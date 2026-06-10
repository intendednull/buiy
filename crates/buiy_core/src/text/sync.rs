//! `BuiyLayoutStep::TextSync` — the dirty path (measure-and-layout § 4.1;
//! architecture §§ 4.1, 5.1).
//!
//! Creates/updates `TextBuffer` from the authored components via the 0.19
//! LAZY setters — no `FontSystem`, no lock (architecture § 1.2): mutation
//! is recorded, shaping deferred to the next lock-bearing site (T3's
//! measure closure / `TextCommit`). Invalidates the cached intrinsics and
//! (Task 7 / T3 consumers) dirty-marks the Taffy node.
//!
//! **The trigger-union ledger (architecture § 5.1 row 1).** As specced:
//! `Or<(Changed<Text>, Changed<text-style carriers>, Added<TextBuffer>,
//! Changed<WritingModeResolved>)>` ∪ `FontsGeneration` bump ∪ theme
//! font-token swap. Carriers from T2: `Text`, `FontFamily`, `FontSize`,
//! `FontWeight`, `WritingModeResolved`, `FontsGeneration`; joined in T3:
//! `LineHeight` / `WhiteSpace` / `TextWrap` / `TextAlign` (measure
//! §§ 5.1–5.3). Members that join with their carriers: `TextDirection`
//! (**T5**, with the § 5.4 strong-mark prepend), the theme font-token swap
//! (**buiy-theme-tokens-design**, font-assets § 9).

use bevy::prelude::*;
use cosmic_text::{Attrs, Family, Metrics, Weight};

use crate::layout::{LayoutTree, WritingModeResolved};

use super::components::{
    ComputedTextLayout, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, LineHeight,
    ResolvedBaseline, TEXT_SHAPING, Text, TextAlign, TextBuffer, TextStyleDefaults, TextWrap,
    WhiteSpace, resolve_wrap,
};
use super::font_system::FontsGeneration;
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

/// The § 5.1 row-1 union over the T2 + T3 carriers (the module doc is the
/// ledger for members that join later).
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
        ),
        Without<TextBuffer>,
    >,
) {
    applied.0 = 0;
    let mut ctx = SyncContext {
        defaults: &defaults,
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
    for (entity, text, family, size, weight, line_height, white_space, text_wrap) in &unsynced {
        let style = AuthoredStyle::resolve(
            ctx.defaults,
            family,
            size,
            weight,
            line_height,
            white_space,
            text_wrap,
        );
        let mut buffer = TextBuffer::new(style.metrics());
        apply_authored(&mut buffer, text, &style);
        commands.entity(entity).insert(buffer);
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
            sync_one(item, &mut ctx);
        }
    } else {
        let mut triggered = synced.p0();
        for item in triggered.iter_mut() {
            sync_one(item, &mut ctx);
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
            entity_commands.remove::<(TextBuffer, ComputedTextLayout, ResolvedBaseline)>();
        }
    }
}

struct SyncContext<'a> {
    defaults: &'a TextStyleDefaults,
    tree: Option<&'a mut LayoutTree>,
    applied: &'a mut TextSyncAppliedCount,
}

fn sync_one(item: SyncedTextItem<'_>, ctx: &mut SyncContext<'_>) {
    let (entity, text, mut buffer, family, size, weight, line_height, white_space, text_wrap) =
        item;
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
    );
    apply_authored(buffer, text, &style);
    if let Some(tree) = ctx.tree.as_deref_mut() {
        // Absent tree (standalone BuiyTextPlugin, no LayoutPlugin): nothing
        // measures, nothing to dirty.
        tree.mark_dirty_for_entity(entity);
    }
    ctx.applied.0 += 1;
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
}

impl<'a> AuthoredStyle<'a> {
    fn resolve(
        defaults: &'a TextStyleDefaults,
        family: Option<&'a FontFamily>,
        size: Option<&FontSize>,
        weight: Option<&FontWeight>,
        line_height: Option<&LineHeight>,
        white_space: Option<&WhiteSpace>,
        text_wrap: Option<&TextWrap>,
    ) -> Self {
        Self {
            family: family.map_or(&defaults.family, |component| &component.0),
            size: size.map_or(defaults.size, |component| component.0),
            weight: weight.map_or(defaults.weight, |component| component.0),
            line_height: line_height.copied().unwrap_or_default(),
            white_space: white_space.copied().unwrap_or_default(),
            text_wrap: text_wrap.copied().unwrap_or_default(),
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

    /// Lower to the per-buffer `Attrs`. T2 INTERIM: the stack's FIRST entry
    /// only — the Buiy-owned resolver (fontdb `Query` walk, coverage
    /// span-splitting, `unicode-range`) is T5's (font-assets § 6); until it
    /// lands, misses fall through to cosmic-text's `FontFallbackIter` and
    /// the deterministic `BuiyFallback`.
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
/// FontSystem, no lock; shaping deferred (architecture §§ 1.2, 3.2).
///
/// `alignment: None` stays even with the `TextAlign` carrier landed
/// (decision 8): `set_text` with `None` leaves reused lines' align
/// untouched, so the `Some→None` transition is only correct in the pass
/// that calls `set_align` on EVERY line — `TextCommit` (§ 5.3, a finalize
/// concern). The § 5.4 direction strong-mark prepend (T5) slots between
/// the collapse transform and `set_text`, AFTER the trim.
fn apply_authored(buffer: &mut TextBuffer, text: &Text, style: &AuthoredStyle<'_>) {
    let collapsed = collapse_whitespace(&text.0, style.white_space.collapse_mode());
    buffer.buffer.set_metrics(style.metrics());
    buffer
        .buffer
        .set_wrap(resolve_wrap(style.white_space, style.text_wrap));
    buffer.buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    buffer
        .buffer
        .set_text(&collapsed, &style.attrs(), TEXT_SHAPING, None);
    buffer.invalidate_intrinsics();
}
