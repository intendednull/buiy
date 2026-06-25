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
//! prepend); joined in T6: `TextDecorations` — the line bits live in
//! `Attrs`, `has_decoration` gates span creation. Carrier REMOVAL is not a
//! member: like every other carrier (T2 erratum 1), a removed
//! `TextDirection` (or `TextDecorations`) resyncs on the next other
//! trigger — documented here, not special-cased. Members that join with
//! their carriers later: the theme font-token swap
//! (**buiy-theme-tokens-design**, font-assets § 9).

use bevy::prelude::*;
use cosmic_text::{Attrs, AttrsList, Buffer, Family, Metrics, Weight};

use crate::layout::{LayoutTree, WritingModeResolved};

use super::components::{
    ComputedTextLayout, DecorationLineStyle, DecorationLines, FamilyEntry, FontFamily, FontSize,
    FontStack, FontWeight, LetterSpacing, LineHeight, ResolvedBaseline, TEXT_SHAPING, Text,
    TextAlign, TextBuffer, TextDecorations, TextDirection, TextStyleDefaults, TextWrap, WhiteSpace,
    resolve_wrap,
};
use super::direction::prepend_strong_marks;
use super::edit::{TextBufferAccess, TextBufferAccessItem};
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
    // parity-prototype A1: letter-spacing rides Attrs (like weight/family),
    // so an edit must reshape — the per-glyph advance changes (cosmic-text
    // shape.rs adds the lowered em value to each glyph's advance). The lowered
    // value is `px / font_size` (see AuthoredStyle::spaced), so a FontSize edit
    // also changes it — already covered by the Changed<FontSize> trigger above.
    Changed<LetterSpacing>,
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
    // T6 carrier (decoration-and-paint § 2.2): the line bits ride
    // Attrs.text_decoration and gate upstream span creation, so a
    // decoration edit must reshape. (A color-only edit also lands here —
    // component-granular; accepted, see the T6 plan's honesty pins.)
    Changed<TextDecorations>,
    Added<TextBuffer>,
    Changed<WritingModeResolved>,
)>;

type SyncedText = (
    Entity,
    &'static Text,
    // The editor-first accessor (E1): an editor entity's authored text lands
    // in its editor-owned buffer; a display entity's lands in
    // `TextBuffer.buffer` (the `Option<&mut TextEditState>` member is `None`).
    // The accessor's display member still matches display-only entities, so no
    // entity drops out of the sync set.
    TextBufferAccess,
    Option<&'static FontFamily>,
    Option<&'static FontSize>,
    Option<&'static FontWeight>,
    Option<&'static LetterSpacing>,
    Option<&'static LineHeight>,
    Option<&'static WhiteSpace>,
    Option<&'static TextWrap>,
    Option<&'static TextDirection>,
    Option<&'static TextDecorations>,
    // Read-only: the marker's CURRENT state, reconciled (insert/remove)
    // against this sync's resolution by `reconcile_font_block` (decision 9).
    Option<&'static PendingFontBlock>,
    // § 3.3: bind the SingleLine marker (NOT a filter) so `sync_one` lays an
    // editor's buffer with `Wrap::None`. Display-only and multi-line editors
    // get `false` and are unaffected.
    Has<super::edit::SingleLine>,
);

type SyncedTextItem<'w> = (
    Entity,
    &'w Text,
    // The generated item carries TWO lifetimes in 0.18 (`Item<'__w, '__s>`,
    // world + state — see access.rs's note), so the second elides here.
    TextBufferAccessItem<'w, 'w>,
    Option<&'w FontFamily>,
    Option<&'w FontSize>,
    Option<&'w FontWeight>,
    Option<&'w LetterSpacing>,
    Option<&'w LineHeight>,
    Option<&'w WhiteSpace>,
    Option<&'w TextWrap>,
    Option<&'w TextDirection>,
    Option<&'w TextDecorations>,
    Option<&'w PendingFontBlock>,
    bool,
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
            Option<&LetterSpacing>,
            Option<&LineHeight>,
            Option<&WhiteSpace>,
            Option<&TextWrap>,
            Option<&TextDirection>,
            Option<&TextDecorations>,
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
        letter_spacing,
        line_height,
        white_space,
        text_wrap,
        direction,
        decorations,
        pending,
    ) in &unsynced
    {
        let style = AuthoredStyle::resolve(
            ctx.defaults,
            family,
            size,
            weight,
            letter_spacing,
            line_height,
            white_space,
            text_wrap,
            direction,
            decorations,
        );
        // The ONLY direct (non-accessor) write left, and it is correct: a
        // freshly-built `TextBuffer` has no `TextEditState` reachable here
        // (the `unsynced` query is `Without<TextBuffer>` and never binds the
        // editor), so the display buffer IS authoritative at insert time. An
        // entity that ALSO carries `TextEditState` (an editor) re-syncs next
        // frame through the `Added<TextBuffer>` arm — `sync_one` then routes
        // the authored text into the editor-owned buffer via the accessor.
        // (`the_seam_preserves_the_zero_measure_steady_frame` pins that
        // one-frame-later convergence.)
        let mut buffer = TextBuffer::new(style.metrics());
        let blocked = apply_authored_to_buffer(
            &mut buffer.buffer,
            text,
            &style,
            ctx.registry,
            ctx.index,
            ctx.now,
            false, // insert path: no editor here, single-line wrap is moot
        );
        buffer.invalidate_intrinsics();
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
        mut access,
        family,
        size,
        weight,
        letter_spacing,
        line_height,
        white_space,
        text_wrap,
        direction,
        decorations,
        pending,
        single_line,
    ) = item;
    let style = AuthoredStyle::resolve(
        ctx.defaults,
        family,
        size,
        weight,
        letter_spacing,
        line_height,
        white_space,
        text_wrap,
        direction,
        decorations,
    );
    // Write through the accessor: the editor-owned buffer when present (§ 2.2a),
    // else the display `TextBuffer.buffer`. EVERY in-place buffer write bypasses
    // change detection (measure-and-layout § 7) — a sync write is not a damage
    // signal; damage keys on the commit outputs. The accessor's `with_buffer_mut`
    // performs the bypass internally on whichever side is authoritative
    // (`tests/text_edit_substrate.rs` pins the editor arm;
    // `tests/text_sync.rs` pins `Changed<TextBuffer>` never fires past insertion).
    let is_editor = access.has_edit();
    // Whether an explicit `FontSize` carrier drives the metrics. For an editor
    // with NO carrier, the seeded metric (`TextEditState::for_font_size`) is the
    // editor's OWNED baseline — the style-only sweep must preserve it, not reset
    // it to the global default (the C2 editor-style-stays-live contract). When a
    // carrier IS present the sweep re-applies it (the spec's "metrics update on a
    // FontSize change"). Production `TextInput` carries both, so they agree.
    let size_present = size.is_some();
    let blocked = access.with_buffer_mut(|buffer| {
        if is_editor {
            // Bug-3 fix (§ 2.1): editor entities own their content. Re-apply the
            // SAME metrics/wrap/tab-width as the display path and refresh the
            // per-line default attrs, but NEVER set_text — that is the clobber.
            apply_authored_style_to_editor_buffer(
                buffer,
                &style,
                ctx.registry,
                ctx.index,
                ctx.now,
                single_line,
                size_present,
            )
        } else {
            apply_authored_to_buffer(
                buffer,
                text,
                &style,
                ctx.registry,
                ctx.index,
                ctx.now,
                single_line,
            )
        }
    });
    // Invalidate the AUTHORITATIVE cache (the accessor picks the right side) —
    // every content change keys the intrinsics cache off this invalidation.
    access.invalidate_intrinsics();
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
    /// parity-prototype A1: extra inter-glyph tracking, logical px. `0.0`
    /// (the `LetterSpacing` default) = CSS `normal`; lowered (as em —
    /// `px / font_size`) to `Attrs.letter_spacing` only when non-zero (see
    /// [`Self::spaced`] for the px→em conversion and why cosmic-text needs it).
    letter_spacing: f32,
    line_height: LineHeight,
    white_space: WhiteSpace,
    text_wrap: TextWrap,
    direction: TextDirection,
    /// T6: the decoration LINE bits + style (decision 1: only these lower
    /// into `Attrs`; the color token resolves at extract).
    deco_lines: DecorationLines,
    deco_style: DecorationLineStyle,
}

impl<'a> AuthoredStyle<'a> {
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        defaults: &'a TextStyleDefaults,
        family: Option<&'a FontFamily>,
        size: Option<&FontSize>,
        weight: Option<&FontWeight>,
        letter_spacing: Option<&LetterSpacing>,
        line_height: Option<&LineHeight>,
        white_space: Option<&WhiteSpace>,
        text_wrap: Option<&TextWrap>,
        direction: Option<&TextDirection>,
        decorations: Option<&TextDecorations>,
    ) -> Self {
        Self {
            family: family.map_or(&defaults.family, |component| &component.0),
            size: size.map_or(defaults.size, |component| component.0),
            weight: weight.map_or(defaults.weight, |component| component.0),
            // Unset = `0.0` (CSS `normal`); no `TextStyleDefaults` entry —
            // letter-spacing is a per-element parity knob, not an app-wide
            // default like family/size/weight.
            letter_spacing: letter_spacing.map_or(0.0, |component| component.0),
            line_height: line_height.copied().unwrap_or_default(),
            white_space: white_space.copied().unwrap_or_default(),
            text_wrap: text_wrap.copied().unwrap_or_default(),
            direction: direction.copied().unwrap_or_default(),
            deco_lines: decorations.map_or(DecorationLines::empty(), |d| d.line),
            deco_style: decorations.map_or_else(Default::default, |d| d.style),
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
        self.spaced(
            self.decorated(
                Attrs::new()
                    .family(self.first_family())
                    .weight(Weight(self.weight)),
            ),
        )
    }

    /// parity-prototype A1: apply `letter-spacing` (logical px) to the
    /// shared `Attrs` surface. The authored contract is **logical px** of
    /// extra inter-glyph tracking, independent of font size — but cosmic-text
    /// 0.19's `Attrs::letter_spacing` is **em** (a multiple of font-size): its
    /// `shape.rs` adds the value to the per-glyph advance while that advance is
    /// still in em units (`x_advance / units_per_em + letter_spacing`), and the
    /// advance is multiplied by `font_size` only at width time
    /// (`ShapeGlyph::width`). So the on-screen px contribution per glyph is
    /// `letter_spacing × font_size`. To honor the px contract we lower
    /// `px / font_size` (em), making the final advance exactly `px` regardless
    /// of size. We divide by `self.size.max(METRICS_FLOOR)` — the SAME effective
    /// font-size the buffer metrics use (see [`Self::metrics`]) — so the round
    /// trip is exact and a zero/sub-floor size can't divide-by-zero.
    ///
    /// Skipped when zero so a `normal` (unset) run carries
    /// `letter_spacing_opt: None` exactly like before this knob existed (no
    /// spurious `Attrs` inequality, no reshape churn). Shared by BOTH attrs
    /// constructors ([`Self::attrs`] and [`span_attrs`]) so every span of a
    /// tracked node carries the same spacing — the [`Self::decorated`]
    /// precedent. Both constructors lower the node's single font-size into the
    /// buffer metrics (no per-span `metrics_opt`), so the node-level `self.size`
    /// is the correct divisor for every span.
    fn spaced<'b>(&self, attrs: Attrs<'b>) -> Attrs<'b> {
        if self.letter_spacing != 0.0 {
            // px → em: divide by the effective font-size cosmic-text will
            // multiply the advance back by at width time.
            attrs.letter_spacing(self.letter_spacing / self.size.max(METRICS_FLOOR))
        } else {
            attrs
        }
    }

    /// T6: apply the decoration line bits (decision 1 — bits only, never
    /// the `*_color` builders; tokens resolve at extract). Shared by BOTH
    /// attrs constructors ([`Self::attrs`] and [`span_attrs`]) so the two
    /// can never diverge — every span of a decorated node carries the bits,
    /// and upstream merges them back into whole-line spans.
    fn decorated<'b>(&self, attrs: Attrs<'b>) -> Attrs<'b> {
        let mut attrs = attrs;
        if self.deco_lines.contains(DecorationLines::UNDERLINE) {
            attrs = attrs.underline(self.deco_style.to_cosmic_underline());
        }
        if self.deco_lines.contains(DecorationLines::LINE_THROUGH) {
            attrs = attrs.strikethrough();
        }
        if self.deco_lines.contains(DecorationLines::OVERLINE) {
            attrs = attrs.overline();
        }
        attrs
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
/// Takes a bare `&mut Buffer` (E1): the AUTHORITATIVE buffer the accessor
/// hands it — editor-owned when the entity carries `TextEditState`, else the
/// display `TextBuffer.buffer`. The intrinsics-cache invalidation is the
/// CALLER's job now (via `TextBufferAccess::invalidate_intrinsics` /
/// `TextBuffer::invalidate_intrinsics`), because the cache lives on the
/// authoritative side and only the accessor knows which that is.
///
/// `alignment: None` stays even with the `TextAlign` carrier landed
/// (decision 8): `set_text` with `None` leaves reused lines' align
/// untouched, so the `Some→None` transition is only correct in the pass
/// that calls `set_align` on EVERY line — `TextCommit` (§ 5.3, a finalize
/// concern).
fn apply_authored_to_buffer(
    buffer: &mut Buffer,
    text: &Text,
    style: &AuthoredStyle<'_>,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
    single_line: bool,
) -> bool {
    let collapsed = collapse_whitespace(&text.0, style.white_space.collapse_mode());
    // § 5.4: AFTER collapse (the trim sees authored edges, never the mark).
    // Direction joins the intrinsics content version for free — the caller's
    // invalidate_intrinsics() covers every content change.
    let directed = prepend_strong_marks(&collapsed, style.direction);
    let resolution = resolve_spans(&directed, style.family, style.weight, registry, index, now);
    buffer.set_metrics(style.metrics());
    // § 3.3: a SingleLine editor never wraps, regardless of white-space /
    // text-wrap. The marker wins over the resolved wrap.
    let wrap = if single_line {
        cosmic_text::Wrap::None
    } else {
        resolve_wrap(style.white_space, style.text_wrap)
    };
    buffer.set_wrap(wrap);
    buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    match resolution.spans.as_slice() {
        // Empty text: the base attrs (there is nothing to resolve).
        [] => buffer.set_text(&directed, &style.attrs(), TEXT_SHAPING, None),
        // One span: the T2 set_text path — identical buffer state, no
        // AttrsList churn.
        [only] => buffer.set_text(
            &directed,
            &span_attrs(style, &only.family),
            TEXT_SHAPING,
            None,
        ),
        spans => buffer.set_rich_text(
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
    resolution.blocked
}

/// Style-only re-lower onto an editor-owned buffer (Bug 3 fix, § 2.1). Applies
/// the SAME metrics/wrap/tab-width as [`apply_authored_to_buffer`] but
/// PRESERVES the buffer's existing line text — `set_text` is the clobber, so
/// it is never called. The editor owns its content (seeded via the existing
/// `EditCommand::Insert`, programmatic-set via `SelectAll` + `Insert`);
/// `TextSync` remains the sole writer of its STYLE.
///
/// Returns the `font-display: block` flag (derived over the buffer's CURRENT
/// first-line text), so a Block family still gates the editor.
///
/// `size_present` records whether an explicit `FontSize` carrier drives the
/// metrics. With NO carrier the editor's seeded metric
/// (`TextEditState::for_font_size`) is its OWNED baseline and is preserved (the
/// sweep does not downgrade it to the global default); with a carrier the sweep
/// re-applies it (a `FontSize` change reaches the live editor). The other
/// style facets (wrap/tab-width/default-attrs) always re-apply — TextSync is
/// their sole writer (commit only does size/align).
fn apply_authored_style_to_editor_buffer(
    buffer: &mut Buffer,
    style: &AuthoredStyle<'_>,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
    single_line: bool,
    size_present: bool,
) -> bool {
    if size_present {
        buffer.set_metrics(style.metrics());
    }
    // § 3.3: a SingleLine editor never wraps, regardless of white-space /
    // text-wrap. The marker wins over the resolved wrap.
    let wrap = if single_line {
        cosmic_text::Wrap::None
    } else {
        resolve_wrap(style.white_space, style.text_wrap)
    };
    buffer.set_wrap(wrap);
    buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    // Refresh each line's resolved default attrs (weight/family/decoration
    // bits) WITHOUT dropping its text. `set_attrs_list` resets the line's shape
    // cache when it differs, so TextCommit reshapes it at the next lock site (it
    // is also the shape-unset the § 2.2 shape_stale guard catches and reshapes).
    refresh_line_default_attrs(buffer, style);
    // The block flag derives from resolving the CURRENT first-line text against
    // the authored family (a Block family must still gate the editor's paint).
    style_block_flag(buffer, style, registry, index, now)
}

/// Rewrite each `BufferLine`'s default attrs to the authored base attrs,
/// preserving the line text. Reuses the `AttrsList::new(&base)` precedent the
/// rest of the text stack carries to keep resolved attrs across surgery. A v1
/// editor is a single default-attrs run; per-span rich-text editor refresh is a
/// documented follow-up (C2 § 7).
fn refresh_line_default_attrs(buffer: &mut Buffer, style: &AuthoredStyle<'_>) {
    let base = style.attrs();
    for line in buffer.lines.iter_mut() {
        // Replace the line's attrs list with one defaulting to the new base.
        // set_attrs_list resets shaping when it differs (cosmic buffer_line.rs),
        // which is exactly the re-measure trigger we want; an unchanged base is
        // a no-op (no spurious reshape). The returned `bool` is discarded.
        let attrs_list = AttrsList::new(&base);
        line.set_attrs_list(attrs_list);
    }
}

/// The `font-display: block` flag for an editor buffer: resolve the buffer's
/// CURRENT first-line text against the authored family/weight (the content
/// path derives `blocked` from `resolve_spans` over the lowered text — the
/// editor has no `Text` to lower, so resolve over its own first line). An
/// empty buffer (one empty line) resolves to `false` (nothing blocks).
fn style_block_flag(
    buffer: &Buffer,
    style: &AuthoredStyle<'_>,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
) -> bool {
    let first_line = buffer.lines.first().map(|l| l.text()).unwrap_or("");
    if first_line.is_empty() {
        return false;
    }
    resolve_spans(first_line, style.family, style.weight, registry, index, now).blocked
}

/// Base attrs + the span's resolved family. Weight rides the committed
/// surface (`Attrs.weight` → `Query.weight` → `get_font(id, weight)` —
/// variable weight works, font-assets § 6); style/stretch stay `Normal`
/// (no carriers — C-tier). T6: the decoration line bits ride every span
/// (the shared [`AuthoredStyle::decorated`] lowering).
fn span_attrs<'a>(style: &AuthoredStyle<'_>, family: &'a ResolvedFamily) -> Attrs<'a> {
    let base = Attrs::new().weight(Weight(style.weight));
    let attrs = match family {
        ResolvedFamily::Named(name) => base.family(Family::Name(name)),
        ResolvedFamily::Generic(generic) => base.family(generic.to_cosmic()),
    };
    style.spaced(style.decorated(attrs))
}
