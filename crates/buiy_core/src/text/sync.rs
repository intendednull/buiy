//! `BuiyLayoutStep::TextSync` — the dirty path (measure-and-layout § 4.1;
//! architecture §§ 4.1, 5.1).
//!
//! Creates/updates `TextBuffer` from the authored components via the 0.19
//! LAZY setters — no `FontSystem`, no lock (architecture § 1.2): mutation
//! is recorded, shaping deferred to the next lock-bearing site (T3's
//! measure closure / `TextCommit`). The T5 `FontStack` resolver runs here
//! too, and keeps the contract — it probes the lock-free `FontMatchIndex`
//! snapshot + `FontRegistry` only (T5 plan decision 2). Invalidates the
//! cached intrinsics and (Task 7 / T3 consumers) dirty-marks the Taffy
//! node.
//!
//! **The trigger-union ledger (architecture § 5.1 row 1).** As specced:
//! `Or<(Changed<Text>, Changed<text-style carriers>, Added<TextBuffer>,
//! Changed<WritingModeResolved>)>` ∪ `FontsGeneration` bump ∪ theme
//! font-token swap. Carriers from T2: `Text`, `FontFamily`, `FontSize`,
//! `FontWeight`, `WritingModeResolved`, `FontsGeneration`; joined in T3:
//! `LineHeight` / `WhiteSpace` / `TextWrap` / `TextAlign` (measure
//! §§ 5.1–5.3); joined in T5: `TextDirection` (with the § 5.4 strong-mark
//! prepend). Carrier REMOVAL is not a member: like every other carrier
//! (T2 erratum 1), a removed `TextDirection` resyncs on the next other
//! trigger — documented here, not special-cased. Members that join with
//! their carriers later: the theme font-token swap
//! (**buiy-theme-tokens-design**, font-assets § 9).

use bevy::prelude::*;
use cosmic_text::{Attrs, Family, Metrics, Weight};

use crate::layout::{LayoutTree, WritingModeResolved};

use super::components::{
    ComputedTextLayout, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, LineHeight,
    ResolvedBaseline, TEXT_SHAPING, Text, TextAlign, TextBuffer, TextDirection, TextStyleDefaults,
    TextWrap, WhiteSpace, resolve_wrap,
};
use super::direction::prepend_strong_marks;
use super::font_system::FontsGeneration;
use super::match_index::FontMatchIndex;
use super::registry::{FontRegistry, PendingFontBlock};
use super::resolver::{ResolvedFamily, resolve_spans};
use super::whitespace::collapse_whitespace;

/// CSS `line-height: normal` — the common UA factor (measure § 5.1; the
/// `LineHeight::Normal` arm of the carrier's `Metrics` mapping).
pub(crate) const DEFAULT_LINE_HEIGHT_SCALE: f32 = 1.2;

/// cosmic-text's `set_metrics` asserts BOTH fields non-zero
/// (buffer.rs:729); authored data must degrade, never panic the app.
const METRICS_FLOOR: f32 = 0.01;

/// CSS `tab-size` initial (measure § 5.2 — "set to 8 … at `TextSync`");
/// the C-tier `tab-size` property later drives the same lazy setter.
const DEFAULT_TAB_WIDTH: u16 = 8;

/// Per-frame count of text entities `text_sync_buffers` applied the lazy
/// setters to — the `SyncStylesIterCount` precedent (layout/systems.rs
/// ~:109). Overwritten (not accumulated) at the top of every invocation.
/// `tests/text_sync.rs` asserts ZERO on a no-change frame (the steady-state
/// half of architecture § 5's contract) and exact counts per § 5.1 trigger.
#[derive(Resource, Default, Debug)]
pub struct TextSyncAppliedCount(pub usize);

/// The § 5.1 row-1 union over the T2 + T3 + T5 carriers (the module doc is
/// the ledger for members that join later).
type TextSyncTriggers = Or<(
    Changed<Text>,
    Changed<FontFamily>,
    Changed<FontSize>,
    Changed<FontWeight>,
    // T3 carriers (measure §§ 5.1–5.3). TextAlign is TRIGGER-ONLY here:
    // its value is applied at TextCommit (§ 5.3 — a finalize concern);
    // union membership is the § 5.1 carrier pin (an align edit must
    // dirty-mark the node like any other text-style change).
    Changed<LineHeight>,
    Changed<WhiteSpace>,
    Changed<TextWrap>,
    Changed<TextAlign>,
    // T5 carrier (measure § 5.4): the strong-mark prepend changes buffer
    // content, so a direction flip must resync like a Text edit.
    Changed<TextDirection>,
    Added<TextBuffer>,
    Changed<WritingModeResolved>,
)>;

type SyncedText = (
    Entity,
    &'static Text,
    &'static mut TextBuffer,
    Option<&'static FontFamily>,
    Option<&'static FontSize>,
    Option<&'static FontWeight>,
    Option<&'static LineHeight>,
    Option<&'static WhiteSpace>,
    Option<&'static TextWrap>,
    Option<&'static TextDirection>,
    // Read-only: the marker's CURRENT state, reconciled (insert/remove)
    // against this sync's resolution by `reconcile_font_block` (decision 9).
    Option<&'static PendingFontBlock>,
);

type SyncedTextItem<'w> = (
    Entity,
    &'w Text,
    Mut<'w, TextBuffer>,
    Option<&'w FontFamily>,
    Option<&'w FontSize>,
    Option<&'w FontWeight>,
    Option<&'w LineHeight>,
    Option<&'w WhiteSpace>,
    Option<&'w TextWrap>,
    Option<&'w TextDirection>,
    Option<&'w PendingFontBlock>,
);

/// The `BuiyLayoutStep::TextSync` body (measure-and-layout § 4.1).
///
/// Registered by `BuiyTextPlugin`; the step set itself is configured
/// (chained) by `LayoutPlugin`'s `configure_pipeline` — standalone
/// `BuiyTextPlugin` apps (the T1 engine tests) run it unordered with empty
/// queries, which is inert.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn text_sync_buffers(
    mut commands: Commands,
    defaults: Res<TextStyleDefaults>,
    fonts_generation: Res<FontsGeneration>,
    registry: Res<FontRegistry>,
    mut index: ResMut<FontMatchIndex>,
    time: Res<Time>,
    mut applied: ResMut<TextSyncAppliedCount>,
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut removed_texts: RemovedComponents<Text>,
    mut synced: ParamSet<(Query<SyncedText, TextSyncTriggers>, Query<SyncedText>)>,
    unsynced: Query<
        (
            Entity,
            &Text,
            Option<&FontFamily>,
            Option<&FontSize>,
            Option<&FontWeight>,
            Option<&LineHeight>,
            Option<&WhiteSpace>,
            Option<&TextWrap>,
            Option<&TextDirection>,
            // A marker surviving a Text remove→re-add cycle reconciles
            // here like any other creation-time state.
            Option<&PendingFontBlock>,
        ),
        Without<TextBuffer>,
    >,
) {
    applied.0 = 0;
    let mut ctx = SyncContext {
        defaults: &defaults,
        registry: &registry,
        index: &mut index,
        now: time.elapsed_secs_f64(),
        tree: tree.as_deref_mut(),
        applied: &mut applied,
    };

    // Creation: a `Text` entity without a buffer gets one built and FULLY
    // populated this frame (TextSync precedes SyncStyles, so the deferred
    // insert is visible to the same frame's style sync — text never appears
    // a frame late). The insertion tick fires next frame's
    // `Added<TextBuffer>` arm once more: an idempotent lazy re-apply,
    // before any shaping consumer exists (documented; tests `settle()`
    // across it).
    for (
        entity,
        text,
        family,
        size,
        weight,
        line_height,
        white_space,
        text_wrap,
        direction,
        pending,
    ) in &unsynced
    {
        let style = AuthoredStyle::resolve(
            ctx.defaults,
            family,
            size,
            weight,
            line_height,
            white_space,
            text_wrap,
            direction,
        );
        let mut buffer = TextBuffer::new(style.metrics());
        let blocked = apply_authored(&mut buffer, text, &style, ctx.registry, ctx.index, ctx.now);
        commands.entity(entity).insert(buffer);
        reconcile_font_block(&mut commands, entity, blocked, pending, &style, &ctx);
        if let Some(tree) = ctx.tree.as_deref_mut() {
            // Text added to an entity that ALREADY has a Taffy node
            // (decision 1's edge (b)); no-op for brand-new entities,
            // whose node is created with its context by sync_styles
            // later this frame.
            tree.set_text_context(entity);
        }
        ctx.applied.0 += 1;
    }

    // A FontsGeneration bump sweeps EVERY buffer — late fonts never leave
    // stale tofu (architecture § 2.2). Otherwise only the trigger set runs.
    // (`is_added` excluded: the plugin-init frame has no buffers to sweep.)
    if fonts_generation.is_changed() && !fonts_generation.is_added() {
        let mut all = synced.p1();
        for item in all.iter_mut() {
            sync_one(item, &mut ctx, &mut commands);
        }
    } else {
        let mut triggered = synced.p0();
        for item in triggered.iter_mut() {
            sync_one(item, &mut ctx, &mut commands);
        }
    }

    // `Text` removed while the entity lives: the leaf stops being a text
    // leaf — drop the buffer and the (T3-written) commit outputs, and
    // unregister the Taffy measure context (measure § 2.2;
    // `clear_text_context`'s internal mark_dirty forces the now-plain
    // leaf to re-measure as zero). Despawned entities clean up for free
    // (`gc_removed_nodes` + the `by_entity` guard); `get_entity` filters
    // them out here.
    for entity in removed_texts.read() {
        if let Some(tree) = ctx.tree.as_deref_mut() {
            tree.clear_text_context(entity);
        }
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<(
                TextBuffer,
                ComputedTextLayout,
                ResolvedBaseline,
                PendingFontBlock,
            )>();
        }
    }
}

struct SyncContext<'a> {
    defaults: &'a TextStyleDefaults,
    /// The resolver's gate state (load states, descriptors, deadlines).
    registry: &'a FontRegistry,
    /// The resolver's lock-free match substrate (decision 2 — `&mut` only
    /// for the lazy coverage cache, never a `FontSystem`).
    index: &'a mut FontMatchIndex,
    /// `Time::elapsed_secs_f64` at system entry — the Block-window probe.
    now: f64,
    tree: Option<&'a mut LayoutTree>,
    applied: &'a mut TextSyncAppliedCount,
}

fn sync_one(item: SyncedTextItem<'_>, ctx: &mut SyncContext<'_>, commands: &mut Commands) {
    let (
        entity,
        text,
        mut buffer,
        family,
        size,
        weight,
        line_height,
        white_space,
        text_wrap,
        direction,
        pending,
    ) = item;
    // EVERY in-place TextBuffer write bypasses change detection
    // (measure-and-layout § 7): a sync write is not a damage signal —
    // damage keys on the commit outputs; `Changed<TextBuffer>` is reserved
    // for nothing (tests/text_sync.rs pins it never fires past insertion).
    let buffer = buffer.bypass_change_detection();
    let style = AuthoredStyle::resolve(
        ctx.defaults,
        family,
        size,
        weight,
        line_height,
        white_space,
        text_wrap,
        direction,
    );
    let blocked = apply_authored(buffer, text, &style, ctx.registry, ctx.index, ctx.now);
    reconcile_font_block(commands, entity, blocked, pending, &style, ctx);
    if let Some(tree) = ctx.tree.as_deref_mut() {
        // Absent tree (standalone BuiyTextPlugin, no LayoutPlugin): nothing
        // measures, nothing to dirty.
        tree.mark_dirty_for_entity(entity);
    }
    ctx.applied.0 += 1;
}

/// Reconcile the entity's `PendingFontBlock` marker with this sync's
/// resolution (font-assets § 7; decision 9, entity-level v1): insert/update
/// while a `Block` family is loading inside its window, remove once it no
/// longer blocks (load completed, registration left, or the window closed),
/// and DON'T touch a marker that already carries the right deadline —
/// idempotent, no tick churn on steady re-syncs. The timeout path is
/// `expire_font_block`'s (a closed window with NO re-sync trigger never
/// reaches here).
fn reconcile_font_block(
    commands: &mut Commands,
    entity: Entity,
    blocked: bool,
    existing: Option<&PendingFontBlock>,
    style: &AuthoredStyle<'_>,
    ctx: &SyncContext<'_>,
) {
    let blocked_until = blocked
        .then(|| ctx.registry.earliest_block_deadline(style.family, ctx.now))
        .flatten();
    match (blocked_until, existing) {
        (Some(until), existing) if existing.map(|pending| pending.until) != Some(until) => {
            commands.entity(entity).insert(PendingFontBlock { until });
        }
        (None, Some(_)) => {
            commands.entity(entity).remove::<PendingFontBlock>();
        }
        _ => {}
    }
}

/// The authored style snapshot TextSync lowers into cosmic-text state.
/// Unset font components fall back to `TextStyleDefaults` (font-assets
/// § 8 pins the resource to the font trio); the T3 carriers fall back to
/// their `Default` impls — the CSS initials (measure §§ 5.1–5.2).
struct AuthoredStyle<'a> {
    family: &'a FontStack,
    size: f32,
    weight: u16,
    line_height: LineHeight,
    white_space: WhiteSpace,
    text_wrap: TextWrap,
    direction: TextDirection,
}

impl<'a> AuthoredStyle<'a> {
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        defaults: &'a TextStyleDefaults,
        family: Option<&'a FontFamily>,
        size: Option<&FontSize>,
        weight: Option<&FontWeight>,
        line_height: Option<&LineHeight>,
        white_space: Option<&WhiteSpace>,
        text_wrap: Option<&TextWrap>,
        direction: Option<&TextDirection>,
    ) -> Self {
        Self {
            family: family.map_or(&defaults.family, |component| &component.0),
            size: size.map_or(defaults.size, |component| component.0),
            weight: weight.map_or(defaults.weight, |component| component.0),
            line_height: line_height.copied().unwrap_or_default(),
            white_space: white_space.copied().unwrap_or_default(),
            text_wrap: text_wrap.copied().unwrap_or_default(),
            direction: direction.copied().unwrap_or_default(),
        }
    }

    /// font-size + line-height → `Metrics` (measure § 5.1).
    fn metrics(&self) -> Metrics {
        let font_size = self.size.max(METRICS_FLOOR);
        let line_height = match self.line_height {
            LineHeight::Normal => font_size * DEFAULT_LINE_HEIGHT_SCALE,
            LineHeight::Scale(scale) => font_size * scale,
            LineHeight::Px(px) => px,
        }
        .max(METRICS_FLOOR);
        Metrics::new(font_size, line_height)
    }

    /// The base/default `Attrs`: the stack's FIRST entry + weight. Since T5
    /// the resolver ([`resolve_spans`]) owns the per-span family lowering;
    /// this remains the `set_rich_text` `default_attrs` and the empty-text
    /// `set_text` path — both deliberately the first-entry shape, matching
    /// the resolver's no-winner rule.
    fn attrs(&self) -> Attrs<'_> {
        Attrs::new()
            .family(self.first_family())
            .weight(Weight(self.weight))
    }

    fn first_family(&self) -> Family<'_> {
        match self.family.0.first() {
            Some(FamilyEntry::Named(name)) => Family::Name(name),
            Some(FamilyEntry::Generic(generic)) => generic.to_cosmic(),
            // An empty authored stack degrades to the pinned sans-serif
            // generic rather than panicking or skipping the entity.
            None => Family::SansSerif,
        }
    }
}

/// Apply authored content + style through the 0.19 LAZY setters — no
/// FontSystem, no lock; shaping deferred (architecture §§ 1.2, 3.2). The T5
/// resolver runs between the § 5.4 pre-passes and the setters; its spans
/// lower via `set_text` (≤ 1 span — the T2 shape, no `AttrsList` churn) or
/// `set_rich_text` (coverage splits), both lazy (decision 8). Returns the
/// `font-display: block` flag ([`reconcile_font_block`] consumes it).
///
/// `alignment: None` stays even with the `TextAlign` carrier landed
/// (decision 8): `set_text` with `None` leaves reused lines' align
/// untouched, so the `Some→None` transition is only correct in the pass
/// that calls `set_align` on EVERY line — `TextCommit` (§ 5.3, a finalize
/// concern).
fn apply_authored(
    buffer: &mut TextBuffer,
    text: &Text,
    style: &AuthoredStyle<'_>,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
) -> bool {
    let collapsed = collapse_whitespace(&text.0, style.white_space.collapse_mode());
    // § 5.4: AFTER collapse (the trim sees authored edges, never the mark).
    // Direction joins the intrinsics content version for free — the wholesale
    // invalidate_intrinsics() below covers every content change.
    let directed = prepend_strong_marks(&collapsed, style.direction);
    let resolution = resolve_spans(&directed, style.family, style.weight, registry, index, now);
    buffer.buffer.set_metrics(style.metrics());
    buffer
        .buffer
        .set_wrap(resolve_wrap(style.white_space, style.text_wrap));
    buffer.buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    match resolution.spans.as_slice() {
        // Empty text: the base attrs (there is nothing to resolve).
        [] => buffer
            .buffer
            .set_text(&directed, &style.attrs(), TEXT_SHAPING, None),
        // One span: the T2 set_text path — identical buffer state, no
        // AttrsList churn.
        [only] => buffer.buffer.set_text(
            &directed,
            &span_attrs(style, &only.family),
            TEXT_SHAPING,
            None,
        ),
        spans => buffer.buffer.set_rich_text(
            spans.iter().map(|span| {
                (
                    &directed[span.range.clone()],
                    span_attrs(style, &span.family),
                )
            }),
            &style.attrs(),
            TEXT_SHAPING,
            None,
        ),
    }
    buffer.invalidate_intrinsics();
    resolution.blocked
}

/// Base attrs + the span's resolved family. Weight rides the committed
/// surface (`Attrs.weight` → `Query.weight` → `get_font(id, weight)` —
/// variable weight works, font-assets § 6); style/stretch stay `Normal`
/// (no carriers — C-tier).
fn span_attrs<'a>(style: &AuthoredStyle<'_>, family: &'a ResolvedFamily) -> Attrs<'a> {
    let base = Attrs::new().weight(Weight(style.weight));
    match family {
        ResolvedFamily::Named(name) => base.family(Family::Name(name)),
        ResolvedFamily::Generic(generic) => base.family(generic.to_cosmic()),
    }
}
