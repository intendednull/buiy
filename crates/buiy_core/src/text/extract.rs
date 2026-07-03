//! The glyph producer (glyph-pipeline; architecture § 4.4): per-frame, per
//! visible text entity in `painters_z` order — quantize via `physical()`,
//! key via the `FontKeyInterner`, rasterize on miss (lock site #3), emit
//! straight-alpha `GlyphAlphaInstance`s into the GPU-verified
//! `ExtractedGlyphs` slot. This file owns the producer; the atlas/consumer
//! seam it fills is frozen (render/atlas, prepare.rs, node.rs).
//!
//! This module is the cosmic-text boundary: no cosmic type crosses into
//! `render::atlas`'s seam types (pinned by tests/text_touch_pass.rs's seam
//! contract test).

use std::ops::Range;
use std::sync::MutexGuard;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::math::{UVec2, Vec2};
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::window::PrimaryWindow;
use cosmic_text::{CacheKey, FontSystem, SwashContent};
use smallvec::SmallVec;

use crate::components::{Node, ResolvedLayout, StackingContext};
use crate::layout::Stacking;
use crate::render::atlas::{
    AtlasBitmap, AtlasEntry, AtlasFormat, AtlasKey, BuiyAtlas, GLYPH_IDENTITY_AFFINE,
    GlyphAlphaInstance,
};
use crate::render::color::{
    resolve_caret_color, resolve_preedit_underline, resolve_selection_bg, resolve_selection_fg,
    resolve_token,
};
use crate::render::components::{
    AncestorClip, CaretColor, ClipRect, ComputedPaintSkip, EffectGroup, Opacity, TextColor,
};
use crate::render::extract::{
    ExtractedTextQuads, TextQuad, context_roots, context_tree_paint_order, effective_clip,
};
use crate::render::prepare::{ExtractedGlyphs, GlyphEntityRun};
use crate::theme::Theme;

use super::atlas_key::{FontKeyInterner, glyph_atlas_key};
use super::components::{
    CaretVisual, ComputedTextLayout, FontSize, PreeditVisual, SelectionVisual, TextBuffer,
    TextDecorations,
};
use super::decoration::{DecorationKind, span_decoration_rects, span_x_extent};
use super::edit::{
    Placeholder, PlaceholderActive, PlaceholderBuffer, TextBufferAccessReadOnly,
    TextBufferAccessReadOnlyItem,
};
use super::font_system::{FontDbLineage, FontsGeneration, SharedFontSystem};
use super::registry::PendingFontBlock;
use super::stamp::{solid_stamp_bitmap, solid_stamp_key, stamp_uv};
use super::swash::BuiySwashCache;
use super::visual::caret_stamp_rect;

/// A rasterized glyph's bearing — `Placement{left, top}` (top points UP),
/// the § 5.2 terms `AtlasEntry` does not carry. Cached per `AtlasKey`
/// (bearings are a pure function of the `CacheKey`, so the cache can never
/// go stale) in [`GlyphMetaCache`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphBearing {
    /// Horizontal offset from the glyph origin, physical px.
    pub left: i32,
    /// Vertical offset from the glyph origin, physical px, top-up.
    pub top: i32,
}

/// The `physical()` offset for one run (glyph-pipeline § 5.1): the entity's
/// content-box origin folded with the run baseline, in PHYSICAL px —
/// `physical()` applies its offset after scale (layout.rs:98–99 verified).
pub fn physical_offset(content_origin: Vec2, line_y: f32, scale_factor: f32) -> (f32, f32) {
    (
        content_origin.x * scale_factor,
        (content_origin.y + line_y) * scale_factor,
    )
}

/// The § 5.2 rect formula — rasterize physical, position logical:
/// `rect_px = (phys.x + left, phys.y - top, w, h)`, divided by the scale
/// factor into `GlyphAlphaInstance.rect`'s logical px. `size` is the atlas
/// cell's pixel extent (`AtlasEntry.px.size()` == the rasterized
/// `Placement{width, height}`). A physical-grid-aligned rect divided by the
/// scale lands back on the same physical texels under the exact-linear view
/// transform — crisp text under the pinned Nearest sampler.
/// Re-pivot a window-space glyph/decoration rect's ORIGIN by the entity's 2D
/// affine about `translation`, so a rotated/scaled text run rotates rigidly about
/// the entity's transform-origin (the pivot 6e baked into `GlobalTransform`):
/// `rect.xy -> translation + A·(rect.xy - translation)` = `transform_point` of the
/// box-local corner. The size (`zw`) is left untouched — `coverage.wgsl` applies
/// the SAME affine to the box-local extent, so `rect.xy + A·(v.uv·size)` becomes
/// `transform_point(box_local_corner)`. Identity affine early-returns the rect
/// UNCHANGED, so untransformed text is byte-identical.
fn repivot_origin(rect: [f32; 4], translation: Vec2, affine: [f32; 4]) -> [f32; 4] {
    if affine == GLYPH_IDENTITY_AFFINE {
        return rect;
    }
    let lx = rect[0] - translation.x;
    let ly = rect[1] - translation.y;
    // affine cols [m00, m10, m01, m11]: world = A·local.
    let wx = affine[0] * lx + affine[2] * ly;
    let wy = affine[1] * lx + affine[3] * ly;
    [translation.x + wx, translation.y + wy, rect[2], rect[3]]
}

pub fn glyph_rect_logical(
    phys_x: i32,
    phys_y: i32,
    bearing: GlyphBearing,
    size: UVec2,
    scale_factor: f32,
) -> [f32; 4] {
    // Direct division, NOT multiply-by-reciprocal: 1/1.25 is inexact in f32
    // and the extra rounding step drifts a ULP off the correctly-rounded
    // quotient the fixtures pin (e.g. 9.0 * (1.0/1.25) ≠ 9.0 / 1.25).
    [
        (phys_x + bearing.left) as f32 / scale_factor,
        (phys_y - bearing.top) as f32 / scale_factor,
        size.x as f32 / scale_factor,
        size.y as f32 / scale_factor,
    ]
}

/// Pack the resolved per-glyph clip (glyph-pipeline § 8) into the instance
/// slot: logical-px AABB, `±INFINITY` sentinel when unclipped — the SAME
/// encoding as `PackedInstance` (the coverage shader's discard reads it).
pub fn pack_clip(clip: Option<&ClipRect>) -> [f32; 4] {
    match clip {
        Some(c) => [c.min.x, c.min.y, c.max.x, c.max.y],
        None => [
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::INFINITY,
        ],
    }
}

/// The producer's retained state (glyph-pipeline § 6.3 + § 6.2): every
/// `AtlasKey` the current `ExtractedGlyphs` samples (rebuilt alongside it on
/// damage; touched UN-gated every frame so retained-but-painted glyphs stay
/// LRU-warm), plus the cached last-seen primary-window scale factor — the
/// § 6.2 VALUE-COMPARE probe (never `Changed<Window>`: bevy_winit writes
/// `Window.physical_cursor_position` per CursorMoved, so a tick probe would
/// rebuild on every mouse-move frame).
#[derive(Resource, Default)]
pub struct ResidentTextKeys {
    /// One entry per emitted instance, in emission order.
    pub keys: Vec<AtlasKey>,
    /// Per-entity key attribution: one contiguous range into `keys` per
    /// emitting entity, in emission order — recorded AT emission and rebuilt
    /// in lockstep with `keys` on every rebuild (partial-reextract D2). Keys
    /// are NOT 1:1 with instances (the bidi secondary caret stamp pushes a
    /// second instance but deliberately no second key), so these ranges can
    /// NEVER be derived from `ExtractedGlyphs::entity_runs`. Lives HERE, in
    /// the producer's retained state, NOT on [`GlyphEntityRun`]: the publish
    /// block value-compares `(glyphs, entity_runs)` under ONE tick (T8 D4),
    /// and key attribution is atlas bookkeeping, not published paint data —
    /// folding it into `GlyphEntityRun` would add a non-paint input to that
    /// compare.
    pub key_runs: Vec<(Entity, Range<u32>)>,
    /// `None` until the first rebuild seeds it (the first frame rebuilds
    /// regardless via the Added/Changed fan).
    pub last_scale_factor: Option<f32>,
    /// Last-seen `FontsGeneration` — the § 6.2 font-set probe (T5): the
    /// same value-compare idiom (main-world change ticks are meaningless
    /// across the extract boundary). `None` until the first rebuild.
    pub last_generation: Option<u64>,
}

/// Producer-owned bearing cache (T4 decision 3): `Placement{left, top}` per
/// glyph key — the § 5.2 terms a cache HIT cannot recover from `AtlasEntry`
/// (which carries uv + pixel rect only). Bearings are a pure function of the
/// `CacheKey`, so entries can never go stale; the map is pruned to atlas
/// residency after every rebuild, bounding it by the atlas's own audited
/// budget (no second eviction policy).
#[derive(Resource, Default)]
pub struct GlyphMetaCache(pub std::collections::HashMap<AtlasKey, GlyphBearing>);

/// How this frame re-extracted the glyph carrier — the glyph tier's
/// `NodeDamage` mirror (partial-reextract D1), published by
/// [`extract_buiy_glyphs`] on EVERY dirty frame and consumed by prepare +
/// the compositor from Stage C/D. `Full` = the whole set was rebuilt (cold
/// frame, global trigger, structural change, or a Patch-ineligible change).
/// `Patch { changed, removed }` = the damage is confined to the named
/// RESIDENT entities: re-emit + splice-replace `changed`, splice-delete
/// `removed` (D3: despawns ARE patchable, keyed by the
/// `RemovedComponents<ResolvedLayout>` stream's ids). Untouched on clean
/// frames (the § 6.2 O(0) contract), so consumers read freshness off the
/// resource tick exactly like the carriers.
///
/// Stage B is OBSERVATION-ONLY: the verdict is published (and counted via
/// `RenderWorkCounters`) while the producer still always executes the Full
/// rebuild — Patch EXECUTION is Stage C.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub enum GlyphDamage {
    /// Rebuild/consume the whole carrier.
    #[default]
    Full,
    /// Value-tier damage confined to the named resident entities.
    Patch {
        /// Resident entities to re-emit + splice-replace (Stage C).
        changed: SmallVec<[Entity; 8]>,
        /// Resident despawned entities to splice-delete (Stage C).
        removed: SmallVec<[Entity; 4]>,
    },
}

/// The D3 changed-set-fraction bail threshold: a dirty frame whose changed
/// set exceeds this percentage of the retained runs classifies Full — a
/// scroll ticks `GlobalTransform` on every text descendant, and splice-all
/// must never cost more than the wholesale walk it replaces. 50 % is the
/// design default (partial-reextract D3, "~50 % of runs, tuned in Stage B");
/// the Stage C `mt_ceiling`/gallery measurements own any retune.
const GLYPH_PATCH_MAX_CHANGED_PERCENT: u64 = 50;

/// The Stage B Full/Patch verdict (design D3: "any uncertainty → Full").
/// PURE — every rule branch is unit-testable headless (the
/// [`classify_glyph_content`] discipline); the producer owns gathering the
/// inputs. Rule order:
///
/// 1. Whole-set by NATURE — `global_trigger` (theme / scale-factor /
///    `FontsGeneration` / `FontDbLineage`, all whole-set re-resolves) — or
///    whole-set by UNCERTAINTY — `structural_changed` (the un-scoped probes:
///    paint order or group membership may have moved), `skip_lifted`
///    (hide→show re-insertion: the re-shown subtree's order position is
///    unknown without the walk), `degradation_live` (the alpha-fold's
///    "whole buffer repacked from source" invariant, compositor.rs
///    `fold_degraded_groups`) — → Full.
/// 2. A changed entity ABSENT from the retained runs → Full. Absence-from-
///    retained-runs is the Added detection (D3 "no `Added` text entities"),
///    chosen over `Added<TextBuffer>` because one rule also catches every
///    other way an entity can NEWLY emit (first non-whitespace edit,
///    placeholder activation, entity-tier hide→show) — its order position
///    is equally unknown in all of them.
/// 3. The changed-set-fraction bail: strictly more than
///    [`GLYPH_PATCH_MAX_CHANGED_PERCENT`] of the retained runs → Full.
///    (A 1-entity scene therefore always classifies Full — harmless: with
///    one run, Patch and Full walk the same entity.)
/// 4. Else → Patch: the changed residents to re-emit, plus the RESIDENT
///    despawns to splice-delete. A non-resident removal id is dropped —
///    EVERY node despawn rides `RemovedComponents<ResolvedLayout>`, so a
///    text-free despawn must classify as a no-op Patch, not escalate.
fn classify_glyph_damage(
    global_trigger: bool,
    structural_changed: bool,
    skip_lifted: bool,
    degradation_live: bool,
    changed: &[Entity],
    despawned: &[Entity],
    retained_runs: &[GlyphEntityRun],
) -> GlyphDamage {
    if global_trigger || structural_changed || skip_lifted || degradation_live {
        return GlyphDamage::Full;
    }
    let resident: std::collections::HashSet<Entity> =
        retained_runs.iter().map(|r| r.entity).collect();
    // A despawned id can also arrive through a value-clear stream (a despawn
    // removes EVERY component) — it is a delete, never a re-emit.
    let changed_live: SmallVec<[Entity; 8]> = changed
        .iter()
        .copied()
        .filter(|e| !despawned.contains(e))
        .collect();
    if changed_live.iter().any(|e| !resident.contains(e)) {
        return GlyphDamage::Full;
    }
    if changed_live.len() as u64 * 100
        > retained_runs.len() as u64 * GLYPH_PATCH_MAX_CHANGED_PERCENT
    {
        return GlyphDamage::Full;
    }
    let removed: SmallVec<[Entity; 4]> = despawned
        .iter()
        .copied()
        .filter(|e| resident.contains(e))
        .collect();
    GlyphDamage::Patch {
        changed: changed_live,
        removed,
    }
}

/// The render-world glyph producer (architecture § 4.4; glyph-pipeline § 6).
/// Runs in `ExtractSchedule` `.after(maintain_atlas)` so inserts and touches
/// use the just-advanced frame clock.
///
/// Binds the read-only `TextBufferAccess` form (E1): an editor entity emits
/// glyphs from its editor-owned buffer, a display entity from
/// `TextBuffer.buffer` — the seam is transparent (§ 6.1's accessor mention,
/// realized). `Edit::with_buffer` / `layout_runs` are `&self`, so the
/// main-world read stays read-only.
///
/// § 6.2 ledger — the `FontsGeneration`/`FontDbLineage` value-compare
/// probes joined in T5 (a generation bump rebuilds; a lineage advance
/// additionally reseats the interner), as did `Changed<PendingFontBlock>`
/// with its removal stream (the font-display Block zero-alpha arm: onset,
/// deadline rewrite, and the load/timeout lift). `Changed<TextDecorations>`
/// and the `ExtractedTextQuads` co-carrier joined in T6 (same producer,
/// same probe, one damage decision — both carriers rebuild and republish
/// together on every dirty frame, and neither is touched on a steady one).
/// `Changed<SelectionVisual>` (+ its removal stream — removal IS the clear
/// mechanism) joined in T7's selection emission; `Changed<CaretVisual>`
/// (+ its removal stream — focus loss hides) and `Changed<CaretColor>`
/// joined with T7's caret emission. The un-scoped `Changed<StackingContext>`
/// ORDER probe joined in partial-reextract Stage B (D3) — the scoped union
/// is blind to ancestor-driven paint reorders. The ledger names zero open
/// joins.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn extract_buiy_glyphs(
    mut atlas: ResMut<BuiyAtlas>,
    // The two co-published carriers, tupled into ONE system param (params
    // nest — the `removed` precedent): the producer sits at Bevy's 16-param
    // cap, and T7's probe members need the headroom.
    carriers: (ResMut<ExtractedGlyphs>, ResMut<ExtractedTextQuads>),
    mut interner: ResMut<FontKeyInterner>,
    mut resident: ResMut<ResidentTextKeys>,
    mut meta: ResMut<GlyphMetaCache>,
    fonts: Res<SharedFontSystem>,
    mut swash: ResMut<BuiySwashCache>,
    // The un-gated full fan (the extract_buiy_nodes discipline): WHETHER to
    // rebuild is the probe below; WHAT to include is always the full set.
    texts: Extract<
        Query<
            (
                &GlobalTransform,
                // The editor-first read-only accessor (E1): an editor entity
                // emits glyphs from its editor-owned buffer; a display entity
                // from `TextBuffer.buffer`. `Edit::with_buffer` is `&self`, so
                // the main-world read stays read-only (architecture § 4.4).
                TextBufferAccessReadOnly,
                &ComputedTextLayout,
                Option<&TextColor>,
                Option<&ComputedPaintSkip>,
                Option<&ClipRect>,
                Option<&AncestorClip>,
                Option<&Stacking>,
                Option<&PendingFontBlock>,
                // T6: the decoration carrier — the `-color` token resolves
                // here (decision 1; the line bits already shaped the spans
                // via TextSync).
                Option<&TextDecorations>,
                // T7: the editor visual paint-inputs (decoration-and-paint
                // §§ 5–6) — selection endpoints in, rects + re-tint derived
                // here — plus the caret rect/phase and the explicit
                // caret-color tier.
                Option<&SelectionVisual>,
                Option<&CaretVisual>,
                Option<&CaretColor>,
                // E5: the preedit underline span (editing-and-ime § 6.2). Its
                // own seat — distinct color + underline geometry — on the same
                // quad carrier. 14 of Bevy's 15-tuple limit.
                Option<&PreeditVisual>,
                // E6 (editing-and-ime § 10): the display-only placeholder buffer
                // (M2 — exactly ONE slot, the 15th, at Bevy's tuple cap). Present
                // iff the editor value is empty (sync_placeholder inserts it only
                // when active), so presence IS the "paint the placeholder" signal
                // — the PlaceholderActive marker stays OUT of the tuple (it would
                // be the 16th) and drives only the damage gate below.
                Option<&PlaceholderBuffer>,
            ),
            With<Node>,
        >,
    >,
    // § 6.2 — THE normative trigger union (architecture § 5.1 row 3 defers
    // here). `Changed<TextBuffer>` is deliberately ABSENT: measure/commit
    // writes bypass its ticks; the idempotent ComputedTextLayout write is
    // the text-changed signal.
    changed: Extract<
        Query<
            Entity,
            (
                With<TextBuffer>,
                Or<(
                    Changed<ComputedTextLayout>,
                    Changed<GlobalTransform>,
                    Changed<ResolvedLayout>,
                    Changed<TextColor>,
                    Changed<ClipRect>,
                    Changed<AncestorClip>,
                    Changed<ComputedPaintSkip>,
                    Changed<Stacking>,
                    // T5 (font-assets § 7): Block onset / deadline rewrite.
                    Changed<PendingFontBlock>,
                    // T6 (decoration-and-paint § 2.2): the line bits change
                    // shaping via TextSync; the COLOR tier resolves HERE, so
                    // a color-only edit must re-emit even though
                    // ComputedTextLayout is idempotent.
                    Changed<TextDecorations>,
                    // T7 (decoration-and-paint § 6.3): the render-prep-
                    // written editor visual state — a caret-blink edge or
                    // a selection endpoint change re-emits; steady phases
                    // rebuild nothing.
                    Changed<SelectionVisual>,
                    Changed<CaretVisual>,
                    Changed<CaretColor>,
                    // E5 (editing-and-ime § 6.2): a composition start/update/
                    // end re-emits the preedit underline; steady frames
                    // rebuild nothing.
                    Changed<PreeditVisual>,
                    // E6 (editing-and-ime § 10): the placeholder triggers,
                    // nested in their own Or so the outer union stays at Bevy's
                    // 15-element filter-tuple cap (this group is the 15th slot).
                    //  • PlaceholderActive — the empty↔non-empty TOGGLE: the
                    //    cheap Copy gate signal whose presence the producer's
                    //    PlaceholderBuffer paint branch mirrors.
                    //  • Placeholder / FontSize — a reshape WHILE already
                    //    active: editing the string (or size) re-shapes the
                    //    PlaceholderBuffer with NO PlaceholderActive insert (the
                    //    marker stays present) and NO ComputedTextLayout tick
                    //    (the empty editor value is unchanged), so without these
                    //    the screen keeps the stale placeholder glyphs. Both are
                    //    small runtime-mutable components — cheap, exact gates.
                    Or<(
                        Changed<PlaceholderActive>,
                        Changed<Placeholder>,
                        Changed<FontSize>,
                    )>,
                )>,
            ),
        >,
    >,
    // Despawn + hide→show + Block lift: the damage sources Changed cannot
    // see. Tupled into ONE system param (params nest): the producer hit
    // Bevy's 16-param function-system cap when the Block stream joined.
    mut removed: (
        Extract<RemovedComponents<ResolvedLayout>>,
        Extract<RemovedComponents<ComputedPaintSkip>>,
        Extract<RemovedComponents<PendingFontBlock>>,
        Extract<RemovedComponents<SelectionVisual>>,
        Extract<RemovedComponents<CaretVisual>>,
        // E5: a preedit clear (commit / cancel / focus-loss) is a REMOVAL —
        // removal IS the clear, so the entity must re-emit (with no preedit
        // quad) on commit/cancel.
        Extract<RemovedComponents<PreeditVisual>>,
    ),
    // Paint-order STRUCTURE (partial-reextract D3), tupled into ONE param
    // (params nest — the `carriers`/`removed` precedent; the producer sits at
    // Bevy's 16-param cap):
    //  .0 — every forming entity's StackingContext: the walk's order source.
    //  .1 — the ORDER probe, un-scoped `Changed<StackingContext>`: the § 6.2
    //       union is `With<TextBuffer>`-scoped and therefore BLIND to an
    //       ancestor-driven paint reorder (a wrapper z flip ticks nothing on
    //       the text entities). `StackingContext` is written value-gated
    //       (layout 6f's idempotent insert), so this term is O(0) on steady
    //       frames and fires EXACTLY when painters_z / cross_root_rank — the
    //       only order inputs this walk reads — actually changed. It JOINS
    //       the dirty union below (the Stage B bug fix) and escalates the
    //       classifier to Full.
    //  .2 — the STRUCTURAL probe (classifier-only escalation; deliberately
    //       NOT in the dirty union — see the per-member notes on the query).
    structure: (
        Extract<Query<(Entity, &StackingContext)>>,
        Extract<Query<(), Changed<StackingContext>>>,
        Extract<
            Query<
                (),
                Or<(
                    // A z / top-layer / isolation edit ANYWHERE: order-affecting
                    // by definition. Every glyph-visible reorder also flows into
                    // a `StackingContext` value change (probe .1 — order is
                    // derived solely from painters_z), so this is conservative
                    // defense-in-depth for the CLASSIFIER, mirroring the node
                    // probe, not a dirty trigger.
                    Changed<Stacking>,
                    // A hierarchy mutation (insert/remove/reorder): the retained
                    // run ORDER a Patch would splice into can no longer be
                    // trusted without the walk. Same node-probe member, same
                    // classifier-only role (the order consequence itself rides
                    // probe .1).
                    Changed<Children>,
                    // An effect group forming/dropping re-partitions the glyph
                    // GROUP ranges prepare derives (T8: entity→group off the
                    // node list) — Patch-unsafe. NB `write_effect_groups`
                    // re-inserts the marker EVERY frame a former holds, so any
                    // dirty frame in a group-bearing scene classifies Full —
                    // matching the node tier's `group.is_some()` Patch bail;
                    // this is also exactly why it must NOT join the dirty gate.
                    Changed<EffectGroup>,
                    // The group-DROP edge `Changed<EffectGroup>` cannot see
                    // (opacity → 1.0 REMOVES the marker; removal never ticks
                    // Changed) — the node probe's `Opacity` member, same role.
                    Changed<Opacity>,
                    // Evaluated and EXCLUDED from the node-probe list
                    // (render/extract.rs structural_changed):
                    //  • ClipRect / AncestorClip — an ancestor clip edit
                    //    propagates into each affected TEXT entity's own
                    //    `AncestorClip` (render/clip.rs `reconcile_one`,
                    //    value-gated), which the scoped § 6.2 union already
                    //    watches; clip is per-instance VALUE data (pack_clip),
                    //    never order or footprint → Patch-safe.
                    //  • ComputedPaintSkip — the ADD direction marks every
                    //    suppressed subtree entity individually (visibility.rs
                    //    walk), so affected text rides the scoped union; the
                    //    LIFT direction is the `skip_lifted` removal stream,
                    //    already a Full escalation.
                    //  • Outline / Border / BoxShadow — node-tier FOOTPRINT
                    //    terms (band slot counts in the node buffers); the
                    //    glyph producer reads none of them and glyph slices
                    //    have no fixed footprint (splice semantics, D2).
                    // No glyph-only additions: every other walk input is
                    // either With<TextBuffer>-scoped in the union or derived
                    // from painters_z (probe .1).
                )>,
            >,
        >,
    ),
    theme: Extract<Res<Theme>>,
    // The main-world font-set counters (T5): VALUE-compared against the
    // retained state below — the `theme` main-world-resource extraction
    // precedent, the `last_scale_factor` compare idiom.
    generation: Extract<Res<FontsGeneration>>,
    lineage: Extract<Res<FontDbLineage>>,
    primary: Extract<Query<&Window, With<PrimaryWindow>>>,
    // Stage B (partial-reextract D1/D3): the published Full|Patch verdict.
    // `Option` per the `NodeDamage`/`RenderWorkCounters` precedent — a harness
    // that does not register it (buiy_verify's content-presence census) still
    // runs the producer; no registration drift, no missing-resource skip.
    mut damage: Option<ResMut<GlyphDamage>>,
) {
    let (mut glyphs, mut text_quads) = carriers;
    let (contexts, order_probe, structural_probe) = structure;
    // Drain the removal streams FIRST so the cursors advance on every frame,
    // including early returns (the extract.rs:409 discipline). Stage B
    // collects the IDS (previously just presence bits): despawns are the
    // classifier's splice-delete candidates; the four value-tier clear
    // streams name the entity to re-emit (D3).
    let despawned_ids: SmallVec<[Entity; 8]> = removed.0.read().collect();
    let despawned = !despawned_ids.is_empty();
    let skip_lifted = removed.1.read().count() > 0;
    // The § 7 swap-to-visible: a lifted Block (load or timeout) repaints at
    // full alpha through a normal rebuild.
    let block_lifted_ids: SmallVec<[Entity; 4]> = removed.2.read().collect();
    let block_lifted = !block_lifted_ids.is_empty();
    // T7: a selection clear is a REMOVAL — unlike the style carriers,
    // removal here IS the hide mechanism (the T2-erratum-1 exclusion does
    // not apply), so the cleared rects must repaint away.
    let selection_cleared_ids: SmallVec<[Entity; 4]> = removed.3.read().collect();
    let selection_cleared = !selection_cleared_ids.is_empty();
    // T7: same for the caret — removal = focus loss, the stamp must
    // repaint away.
    let caret_removed_ids: SmallVec<[Entity; 4]> = removed.4.read().collect();
    let caret_removed = !caret_removed_ids.is_empty();
    // E5: a preedit clear (commit / cancel / focus-loss) removes the
    // component — removal IS the clear, so the underline must repaint away.
    let preedit_removed_ids: SmallVec<[Entity; 4]> = removed.5.read().collect();
    let preedit_removed = !preedit_removed_ids.is_empty();

    let Ok(window) = primary.single() else {
        // Vanished window: clear the carriers ONCE (an unconditional clear
        // would mark them changed and re-upload empty buffers every frame).
        if !glyphs.glyphs.is_empty()
            || !glyphs.entity_runs.is_empty()
            || !text_quads.quads.is_empty()
        {
            glyphs.glyphs.clear();
            glyphs.entity_runs.clear();
            text_quads.quads.clear();
            resident.keys.clear();
            resident.key_runs.clear();
            // The whole set vanished — whole-set damage, once (inside the
            // once-clear so retained no-window frames stay tick-quiet).
            if let Some(d) = damage.as_deref_mut() {
                *d = GlyphDamage::Full;
            }
        }
        return;
    };
    let scale_factor = window.resolution.scale_factor();
    let scale_changed = resident.last_scale_factor != Some(scale_factor);
    let fonts_changed = resident.last_generation != Some(generation.0);
    // Partial-reextract D3 / Stage B: an ancestor-driven paint reorder JOINS
    // the § 6.2 union. The scoped `changed` union cannot see it (a wrapper z
    // flip ticks nothing on any text entity), so pre-Stage-B the carrier kept
    // STALE glyph paint order — the pre-existing under-trigger the
    // reorder-escalation pin (`ancestor_z_reorder_rebuilds_and_flips_glyph_
    // paint_order`) reproduces. Value-gated writes keep this O(0) on steady
    // frames; the classifier-only `structural_probe` deliberately does NOT
    // join (its `EffectGroup` member re-ticks every frame a former holds).
    let paint_reordered = !order_probe.is_empty();

    let dirty = !changed.is_empty()
        || despawned
        || skip_lifted
        || block_lifted
        || selection_cleared
        || caret_removed
        || preedit_removed
        || paint_reordered
        || theme.is_changed()
        || scale_changed
        || fonts_changed;
    if !dirty {
        // Steady state: return WITHOUT touching ExtractedGlyphs (so
        // `glyphs.is_changed()` stays false in prepare and the GPU glyph
        // buffer is retained — the O(0) contract)… except for the § 6.3
        // UN-gated touch pass: retained instances embed uv/page, and an
        // untouched key would grace-evict while still painted — the
        // stale-uv corruption hazard. O(visible keys) hash lookups.
        for key in &resident.keys {
            atlas.touch_existing(key);
        }
        return;
    }

    // ---- Stage B classification (OBSERVATION-ONLY; partial-reextract D3) --
    // Publish the Full|Patch verdict against the RETAINED runs (pre-rebuild
    // state — exactly what a Stage C Patch would splice into), then fall
    // through to the wholesale rebuild regardless. `record_text_work_counters`
    // reads the verdict off this write's tick.
    if let Some(d) = damage.as_deref_mut() {
        // The value-tier changed set: the § 6.2 union ∪ the four clear
        // streams (removal IS the paint change on that entity — the cleared
        // selection/caret/preedit/Block must repaint away/at-full-alpha).
        let mut changed_set: Vec<Entity> = changed.iter().collect();
        for &e in block_lifted_ids
            .iter()
            .chain(&selection_cleared_ids)
            .chain(&caret_removed_ids)
            .chain(&preedit_removed_ids)
        {
            if !changed_set.contains(&e) {
                changed_set.push(e);
            }
        }
        *d = classify_glyph_damage(
            theme.is_changed() || scale_changed || fonts_changed,
            paint_reordered || !structural_probe.is_empty(),
            skip_lifted,
            // Live effect-group degradation is a PREPARE-side decision
            // (`plan_allocation` inside `prepare_effect_groups` runs AFTER
            // extract each frame; only last frame's `RtPoolStats` echo exists
            // here) — the producer CANNOT see the current frame's bit, so the
            // D3 degradation bail is enforced by the CONSUMER: Stage C's
            // prepare reads the fold state and falls back to Full there. In
            // practice `Changed<EffectGroup>` (probe .2) already forces Full
            // for every group-bearing dirty frame, and degradation cannot
            // exist without a group.
            false,
            &changed_set,
            &despawned_ids,
            &glyphs.entity_runs,
        );
    }

    resident.last_scale_factor = Some(scale_factor);
    resident.last_generation = Some(generation.0);
    // § 3.2 (T5): a lineage advance means every fontdb ID was reissued —
    // clear the interner's ID map BEFORE any interning so keys re-seat
    // MONOTONICALLY (old entries grace-evict on their own; `GlyphMetaCache`
    // prunes via the residency retain below). In-lineage rebuilds no-op.
    interner.begin_lineage(lineage.0);

    // ---- Rebuild (wholesale, § 6.2 v1) -------------------------------
    let mut new_glyphs: Vec<GlyphAlphaInstance> = Vec::new();
    // The quad-tier co-carrier (T6): rebuilt alongside the glyphs on every
    // dirty frame (decision 12), PUBLISHED value-compared (T7 decision 4 —
    // see the publish block). ENTITY-GROUPED by construction — the walk
    // below emits one entity at a time, and the pack debug_asserts the
    // grouping (§ 4.6).
    let mut new_quads: Vec<TextQuad> = Vec::new();
    // Per-entity instance attribution (T8 D1): one contiguous run per
    // emitting entity, in emission order — the prepare-time group
    // partition's lookup key.
    let mut new_runs: Vec<GlyphEntityRun> = Vec::new();
    let mut new_keys: Vec<AtlasKey> = Vec::new();
    // Per-entity key attribution (partial-reextract D2), rebuilt in lockstep
    // with `new_keys` — see [`ResidentTextKeys::key_runs`].
    let mut new_key_runs: Vec<(Entity, Range<u32>)> = Vec::new();
    // The per-frame shared emission context (the lazily-taken font guard and
    // the stamp-residency probe live on it — see the field docs). ONE context
    // threads through the whole walk; a future per-entity Patch path
    // constructs its own and re-emits through the same [`emit_one_entity`]
    // (partial-reextract D2).
    let mut ctx = EmitContext {
        atlas: &mut atlas,
        meta: &mut meta,
        fonts: &fonts,
        font_guard: None,
        swash: &mut swash,
        interner: &mut interner,
        stamp_entry: None,
        theme: &theme,
        scale_factor,
    };

    // painters_z order — the same walk extract_buiy_nodes runs (§ 2), including the
    // cross-root rank so a parentless top-layer root (modal/popover) glyphs paint
    // over the whole window (the M6 fix — same root ordering the node walk uses).
    let sc_by_entity: std::collections::HashMap<Entity, &[Entity]> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.painters_z.as_slice()))
        .collect();
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { sc_by_entity.get(&e).copied() };
    let rank_by_entity: std::collections::HashMap<Entity, u8> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.cross_root_rank))
        .collect();
    let mut order = Vec::new();
    for root in context_roots(&sc_by_entity, |e| {
        rank_by_entity.get(&e).copied().unwrap_or(0)
    }) {
        context_tree_paint_order(root, &painters_z_of, &mut order);
    }

    for entity in order {
        let Ok(item) = texts.get(entity) else {
            continue; // not a text painter
        };
        emit_one_entity(
            entity,
            item,
            &mut ctx,
            &mut new_glyphs,
            &mut new_quads,
            &mut new_keys,
            &mut new_runs,
            &mut new_key_runs,
        );
    }
    // Dropping the context releases lock site #3's guard (if taken) and the
    // producer borrows before the residency prune + publish below.
    drop(ctx);

    // Bearing-cache hygiene (decision 3): prune to atlas residency, so the
    // map is bounded by the atlas's own budget — invariant: every resident
    // glyph key has a bearing.
    meta.0.retain(|key, _| atlas.get(key).is_some());

    // Publish — wholesale REBUILD under the one § 6.2 damage decision
    // (unchanged), VALUE-COMPARED publication per carrier (T7 decision 4,
    // refining T6 decision 12): a blink edge changes only the glyph
    // content, so the quad carrier keeps its tick and the GPU quad buffer
    // is retained (decoration-and-paint § 6.3's damage property; prepare
    // gates each buffer independently). Equal inputs produce bit-identical
    // f32 outputs, so derive-PartialEq equality is deterministic. One
    // O(instances) compare per DIRTY frame — steady frames return above.
    // Then the § 6.3 touch pass over the NEW visible set (covers this
    // frame's hits — `atlas.get` deliberately does not touch the LRU).
    // The glyph carrier compares (instances, entity_runs) TOGETHER under one
    // tick (T8 D4): instance bytes can coincide across different entity sets
    // (despawn + respawn of an identical fixture), and group membership keys
    // on the ENTITY — runs inequality must republish even when instances
    // compare equal, or prepare would never re-derive the group partition.
    if glyphs.glyphs != new_glyphs || glyphs.entity_runs != new_runs {
        let glyphs = &mut *glyphs;
        glyphs.glyphs = new_glyphs;
        glyphs.entity_runs = new_runs;
    }
    if text_quads.quads != new_quads {
        text_quads.quads = new_quads;
    }
    resident.keys = new_keys;
    resident.key_runs = new_key_runs;
    for key in &resident.keys {
        atlas.touch_existing(key);
    }
}

/// The `texts` paint-query row, spelled once — [`extract_buiy_glyphs`]'s walk
/// fetches it and [`emit_one_entity`] destructures it. The tuple itself (at
/// Bevy's 15-member cap) lives on the query, which carries the per-member
/// ledger; this is its read-only item form.
type TextPaintItem<'w, 's> = (
    &'w GlobalTransform,
    TextBufferAccessReadOnlyItem<'w, 's>,
    &'w ComputedTextLayout,
    Option<&'w TextColor>,
    Option<&'w ComputedPaintSkip>,
    Option<&'w ClipRect>,
    Option<&'w AncestorClip>,
    Option<&'w Stacking>,
    Option<&'w PendingFontBlock>,
    Option<&'w TextDecorations>,
    Option<&'w SelectionVisual>,
    Option<&'w CaretVisual>,
    Option<&'w CaretColor>,
    Option<&'w PreeditVisual>,
    Option<&'w PlaceholderBuffer>,
);

/// The per-frame shared context [`emit_one_entity`] reads (partial-reextract
/// D2): the producer state every entity's emission touches, bundled so the
/// Full walk and a future per-entity Patch path call ONE signature (the node
/// tier's `resolve_one` discipline — Full and Patch byte-identical by
/// construction). `'w` is the system-param borrow the font guard hangs off;
/// `'a` the per-frame resource borrows.
struct EmitContext<'w, 'a> {
    atlas: &'a mut BuiyAtlas,
    meta: &'a mut GlyphMetaCache,
    fonts: &'w SharedFontSystem,
    /// Lock site #3 (architecture § 1.2 — the LAST of the exhaustive three):
    /// taken LAZILY, once per frame, only when at least one glyph misses the
    /// atlas (the text_commit guard pattern). A hit-only rebuild takes ZERO
    /// locks; extract runs in the pipelining sync window, so the lock is
    /// uncontended by construction. Starts `None`; `resolve_glyph` fills it
    /// on the first miss.
    font_guard: Option<MutexGuard<'w, FontSystem>>,
    swash: &'a mut BuiySwashCache,
    interner: &'a mut FontKeyInterner,
    /// One stamp-residency probe per frame (T6, decoration-and-paint § 4.3):
    /// the solid stamp's closure never touches the `FontSystem`, so this
    /// stays outside lock site #3; a grace-evicted stamp self-heals here on
    /// the next line-through ("re-inserted on miss like any
    /// content-addressed entry").
    stamp_entry: Option<AtlasEntry>,
    theme: &'a Theme,
    scale_factor: f32,
}

/// Emit ONE text entity's records — the walk-loop body of
/// [`extract_buiy_glyphs`], factored out (partial-reextract D2, the node
/// tier's `resolve_one` discipline) so a future Patch path can re-emit a
/// single changed entity through the SAME code against separate output vecs.
/// Appends, in the § 4.4 seat order: selection quads (seat 2), the preedit
/// underline, then per run decoration quads + glyphs + buffered line-through
/// stamps (seat 5), placeholder glyphs, the caret stamp(s) (seat 6) — then
/// attributes the appended instance range (`new_runs`) and key range
/// (`new_key_runs`) to the entity. A paint-skipped entity appends nothing.
#[allow(clippy::too_many_arguments)]
fn emit_one_entity(
    entity: Entity,
    item: TextPaintItem<'_, '_>,
    ctx: &mut EmitContext<'_, '_>,
    new_glyphs: &mut Vec<GlyphAlphaInstance>,
    new_quads: &mut Vec<TextQuad>,
    new_keys: &mut Vec<AtlasKey>,
    new_runs: &mut Vec<GlyphEntityRun>,
    new_key_runs: &mut Vec<(Entity, Range<u32>)>,
) {
    let (
        gt,
        access,
        computed,
        color,
        skip,
        clip_rect,
        ancestor_clip,
        stacking,
        pending_block,
        decorations,
        selection_visual,
        caret_visual,
        caret_color,
        preedit_visual,
        placeholder_buffer,
    ) = item;
    if skip.is_some() {
        return; // the single computed skip source (§ 5.3/§ 5.4)
    }
    // Re-borrow the context under the names the emission body has always
    // used — the body below is the pre-factor walk-loop body, moved verbatim
    // (Stage A's byte-identical discipline).
    let atlas = &mut *ctx.atlas;
    let meta = &mut *ctx.meta;
    let fonts = ctx.fonts;
    let font_guard = &mut ctx.font_guard;
    let swash = &mut *ctx.swash;
    let interner = &mut *ctx.interner;
    let stamp_entry = &mut ctx.stamp_entry;
    let theme = ctx.theme;
    let scale_factor = ctx.scale_factor;
    let default_color = TextColor::default();

    let entity_start = new_glyphs.len() as u32;
    // The entity's key-range start (partial-reextract D2): keys append in
    // lockstep with instances through the body below.
    let key_start = new_keys.len() as u32;
    // § 8: glyphs AND decorations are CONTENT — self-inclusive clip
    // (own ClipRect ∩ ancestors); top-layer members force the unclipped
    // sentinel.
    let eff_clip = effective_clip(stacking, clip_rect, ancestor_clip);
    let clip = pack_clip(eff_clip.as_ref());
    // § 7: resolved at extract like Background, CPU-linearized,
    // STRAIGHT alpha (premultiplying would double-dim — primitive.rs).
    let resolved_entity_color = resolve_token(&color.unwrap_or(&default_color).0, theme);
    let entity_color = linear_color(resolved_entity_color);
    let origin = gt.translation().truncate() + computed.content_offset;
    // The entity's 2D affine (GlobalTransform's linear part, cols
    // [m00,m10,m01,m11]) + its translation — so a rotated/scaled text run
    // paints its glyphs/decorations rotated rigidly about the transform-origin
    // (6e-baked). `repivot_origin` moves each window-space rect's ORIGIN by the
    // affine about `translation`; `coverage.wgsl` then applies the same affine
    // to the box-local extent. Identity affine leaves every rect byte-identical
    // (repivot early-returns), so untransformed text is unchanged.
    let translation = gt.translation().truncate();
    let m = gt.affine().matrix3;
    let glyph_affine = [m.x_axis.x, m.x_axis.y, m.y_axis.x, m.y_axis.y];
    let blocked = pending_block.is_some();
    // § 3.2 tier 1: the -color token, resolved at extract against the
    // live theme (decision 1 — retheme re-emits, never reshapes).
    let deco_override: Option<Color> = decorations
        .and_then(|d| d.color.as_ref())
        .map(|t| resolve_token(t, theme));

    // T7 § 5.1: the selection pre-pass — seat 2 quads for the WHOLE
    // entity before any seat-3 decoration quad (§ 4.4; a per-run
    // interleave could paint an underline under the next line's
    // selection where line boxes touch). Iteration only — no locks.
    // Collapsed selections paint nothing (a collapsed selection is a
    // caret; skipping also removes upstream's mid-grapheme re-tint
    // edge). Paints normally under Block (chrome, not ink —
    // decision 9).
    let selection = selection_visual.filter(|s| !s.is_collapsed());
    let mut selection_fg: Option<[f32; 4]> = None;
    if let Some(sel) = selection {
        let bg = resolve_selection_bg(theme);
        selection_fg = Some(linear_color(resolve_selection_fg(theme)));
        if bg.alpha() > 0.0 {
            // Upstream's reference width source for the line-edge
            // extension + empty-line fill; commit guarantees Some
            // (T3 decision 9) — computed.size.x is defense, not a
            // path (upstream's unwrap_or(0.0) would drop the
            // extension silently).
            let full_w = access
                .with_buffer(|buffer| buffer.size().0)
                .unwrap_or(computed.size.x);
            access.with_buffer(|buffer| {
                for run in buffer.layout_runs() {
                    // THE caller contract (Orientation § 1 of the T7 plan):
                    // highlight's predicate degenerates to all-selected
                    // outside [start.line, end.line] — gate first, like
                    // upstream (edit/editor.rs:103).
                    if run.line_i < sel.start.line || run.line_i > sel.end.line {
                        continue;
                    }
                    let spans: SmallVec<[(f32, f32); 4]> =
                        run.highlight(sel.start, sel.end).collect();
                    if spans.is_empty() && run.glyphs.is_empty() && sel.end.line > run.line_i {
                        // Internal fully-selected empty line: full width.
                        push_selection_quad(
                            new_quads, entity, origin, 0.0, full_w, &run, bg, eff_clip,
                        );
                    } else {
                        let len = spans.len();
                        for (idx, (x, w)) in spans.into_iter().enumerate() {
                            let (mut min_x, mut max_x) = (x, x + w);
                            // Non-final selected line: the last rect
                            // extends to the line edge (newline made
                            // visible) — RTL-aware, upstream verbatim.
                            if idx + 1 == len && sel.end.line > run.line_i {
                                if run.rtl {
                                    min_x = 0.0;
                                } else {
                                    max_x = full_w;
                                }
                            }
                            push_selection_quad(
                                new_quads,
                                entity,
                                origin,
                                min_x,
                                max_x - min_x,
                                &run,
                                bg,
                                eff_clip,
                            );
                        }
                    }
                }
            });
        }
    }

    // E5 (editing-and-ime § 6.2; decoration-and-paint § 8): the preedit
    // underline — a forced single underline over the composing byte
    // range, quad-tier. Mirrors the selection pre-pass: highlight the
    // span per run, then push a THIN underline strip (not a full-height
    // box) at the run baseline. Reuses the quad carrier; no new GPU.
    let preedit = preedit_visual.filter(|p| !p.is_collapsed());
    if let Some(pre) = preedit {
        let color = resolve_preedit_underline(theme, resolved_entity_color);
        let color = if blocked {
            color.with_alpha(0.0)
        } else {
            color
        };
        if color.alpha() > 0.0 {
            access.with_buffer(|buffer| {
                for run in buffer.layout_runs() {
                    if run.line_i < pre.start.line || run.line_i > pre.end.line {
                        continue;
                    }
                    // Underline strip thickness: 1 logical px at the run
                    // baseline bottom (decoration-and-paint § 8 uses the
                    // standard underline metric; a 1px strip is the v1
                    // forced underline — the engine has no per-font
                    // underline metric exposed at this seat).
                    let thickness = 1.0_f32;
                    let strip_top = run.line_top + run.line_height - thickness;
                    for (x, w) in run.highlight(pre.start, pre.end) {
                        if w <= 0.0 {
                            continue;
                        }
                        new_quads.push(TextQuad {
                            entity,
                            position: Vec2::new(origin.x + x, origin.y + strip_top),
                            size: Vec2::new(w, thickness),
                            color,
                            clip: eff_clip,
                        });
                    }
                }
            });
        }
    }

    let runs = access.with_buffer(|buffer| {
        let mut runs = 0usize;
        for run in buffer.layout_runs() {
            runs += 1;
            // The decoration walk (§ 4.6: ONE run walk emits quads AND
            // stamps AND glyphs, under the one damage decision).
            // Underline/overline are quad-tier (§ 4.2) and paint UNDER the
            // text by primitive rank; emitted before the glyph loop for
            // § 4.4 carrier order. Line-through rects (rect, linearized
            // color) buffer through the glyph loop into solid-stamp
            // instances appended after it (§ 4.4 seat 5).
            let mut strikes: SmallVec<[([f32; 4], [f32; 4]); 2]> = SmallVec::new();
            for span in run.decorations {
                let Some((x_start, width)) = span_x_extent(run.glyphs, &span.glyph_range) else {
                    continue;
                };
                for deco in span_decoration_rects(
                    origin,
                    run.line_y,
                    run.line_top,
                    x_start,
                    width,
                    &span.data,
                    span.font_size,
                    span.color_opt,
                    scale_factor,
                ) {
                    // § 3.2 precedence: token override → upstream tiers
                    // (per-kind / span color) → currentColor.
                    let mut color = deco_override
                        .or(deco.color_opt.map(cosmic_color))
                        .unwrap_or(resolved_entity_color);
                    if blocked {
                        // § 7 Block: paint-invisible, layout-identical.
                        color = color.with_alpha(0.0);
                    } else if color.alpha() == 0.0 {
                        continue; // fully transparent: nothing to paint
                    }
                    match deco.kind {
                        DecorationKind::Underline | DecorationKind::Overline => {
                            new_quads.push(TextQuad {
                                entity,
                                position: Vec2::new(deco.rect[0], deco.rect[1]),
                                size: Vec2::new(deco.rect[2], deco.rect[3]),
                                color,
                                clip: eff_clip,
                            });
                        }
                        // § 4.4 seat 5: buffered through the glyph loop —
                        // the solid stamp paints OVER the run's glyphs (the
                        // CSS Text Decoration L3 painting order).
                        DecorationKind::LineThrough => {
                            strikes.push((deco.rect, linear_color(color)));
                        }
                    }
                }
            }
            for glyph in run.glyphs.iter() {
                // § 5.1: cosmic-text's own binning, verbatim.
                let phys = glyph.physical(
                    physical_offset(origin, run.line_y, scale_factor),
                    scale_factor,
                );
                // T7 § 5.2: per-CLUSTER re-tint — a glyph whose bytes
                // intersect the selection paints with the selection fg
                // (over any rich-text span color, upstream-verbatim); the
                // atlas is never touched. Granularity is the cluster: a
                // partially selected ligature re-tints whole while its
                // RECT stays grapheme-accurate (§ 5.2's accepted
                // tradeoff). Upstream's text_color != selected_text_color
                // short-circuit is dropped — equal resolved colors emit
                // identical instances.
                let selected = selection.is_some_and(|sel| {
                    run.line_i >= sel.start.line
                        && run.line_i <= sel.end.line
                        && (sel.start.line != run.line_i || glyph.end > sel.start.index)
                        && (sel.end.line != run.line_i || glyph.start < sel.end.index)
                });
                let mut color = match (selected, selection_fg) {
                    (true, Some(fg)) => fg,
                    // Per-span Attrs color override rides through (§ 7).
                    _ => glyph.color_opt.map(span_color).unwrap_or(entity_color),
                };
                if blocked {
                    // font-assets § 7 Block: identical fallback LAYOUT,
                    // zero-alpha PAINT — instances ARE emitted (the atlas
                    // and the GPU buffer stay warm with the fallback's
                    // glyphs), bypassing the transparent skip below.
                    color[3] = 0.0;
                } else if color[3] == 0.0 {
                    continue; // fully transparent: nothing to paint
                }
                // The SHARED per-glyph emit (the placeholder branch reuses it).
                emit_glyph(
                    atlas,
                    meta,
                    fonts,
                    font_guard,
                    swash,
                    interner,
                    new_glyphs,
                    new_keys,
                    &phys,
                    color,
                    clip,
                    scale_factor,
                    translation,
                    glyph_affine,
                );
            }
            // § 4.4 seat 5: line-through paints OVER the run's glyphs —
            // solid-stamp GlyphAlphaInstances appended after them (quad-tier
            // rects could never paint over glyphs: § 4.1's fixed primitive
            // rank).
            if !strikes.is_empty() {
                let entry = *stamp_entry.get_or_insert_with(|| {
                    atlas.get_or_insert(
                        solid_stamp_key(),
                        AtlasFormat::CoverageR8,
                        solid_stamp_bitmap,
                    )
                });
                if entry.page > 0 {
                    warn_once_page_overflow(); // § 11.1 v1 mitigation
                }
                for (rect, color) in strikes {
                    new_glyphs.push(GlyphAlphaInstance {
                        rect: repivot_origin(rect, translation, glyph_affine),
                        uv: stamp_uv(&entry),
                        color,
                        clip,
                        page: entry.page as u32,
                        affine: glyph_affine,
                    });
                    // § 6.3: one key per instance — the un-gated touch pass
                    // keeps a live stamp LRU-warm through retained frames.
                    new_keys.push(solid_stamp_key());
                }
            }
        }
        runs
    });
    // architecture § 3.2 tripwire: layout_runs TERMINATES at the first
    // unshaped line — a mismatch means something mutated the buffer
    // after TextCommit.
    debug_assert_eq!(
        runs,
        computed.lines.len(),
        "TextBuffer dirty-unshaped at extract (mutated after TextCommit?)"
    );

    // E6 — placeholder paint (editing-and-ime § 10, decoration-and-paint
    // § 7). A SEPARATE additive emission: the placeholder is its own display
    // buffer with its own runs — NOT part of the editor's ComputedTextLayout,
    // so it does NOT feed the § 3.2 run-count assert above (M4). Painted only
    // when a shaped PlaceholderBuffer is present (sync_placeholder inserts it
    // iff the editor value is empty ⇒ the editor loop above emitted no ink),
    // tinted to the placeholder token. Reuses the shared per-glyph emitter.
    if let Some(ph) = placeholder_buffer {
        let ph_color = linear_color(resolve_token(&TextColor::placeholder().0, theme));
        if ph_color[3] > 0.0 {
            for run in ph.buffer.layout_runs() {
                for glyph in run.glyphs.iter() {
                    let phys = glyph.physical(
                        physical_offset(origin, run.line_y, scale_factor),
                        scale_factor,
                    );
                    emit_glyph(
                        atlas,
                        meta,
                        fonts,
                        font_guard,
                        swash,
                        interner,
                        new_glyphs,
                        new_keys,
                        &phys,
                        ph_color,
                        clip,
                        scale_factor,
                        translation,
                        glyph_affine,
                    );
                }
            }
        }
    }

    // T7 § 6.1 — seat 6: the caret paints last, over glyphs and
    // line-through, as a solid-stamp instance (pre-phase decision 2).
    // Painted under Block too (chrome, not ink — decision 9: browsers
    // keep the caret in a focused field whose font is loading). At a
    // bidirectional direction boundary an OPTIONAL second (secondary-
    // indicator) stamp follows the primary (§§ 4.1, 5) — same
    // entry/color/clip/page, CPU geometry only, no new atlas insert.
    if let Some(cv) = caret_visual
        && cv.visible
    {
        // § 6.2: explicit token → theme caret key (presence check) →
        // currentColor. Re-tint only — never an atlas mutation.
        let color = resolve_caret_color(caret_color.map(|c| &c.0), theme, resolved_entity_color);
        if color.alpha() > 0.0 {
            let entry = *stamp_entry.get_or_insert_with(|| {
                atlas.get_or_insert(
                    solid_stamp_key(),
                    AtlasFormat::CoverageR8,
                    solid_stamp_bitmap,
                )
            });
            if entry.page > 0 {
                warn_once_page_overflow(); // § 11.1 v1 mitigation
            }
            new_glyphs.push(GlyphAlphaInstance {
                rect: repivot_origin(
                    caret_stamp_rect(origin, cv.rect, scale_factor),
                    translation,
                    glyph_affine,
                ),
                uv: stamp_uv(&entry),
                color: linear_color(color),
                clip,
                page: entry.page as u32,
                affine: glyph_affine,
            });
            // §§ 4.1, 5: the SECONDARY split-caret indicator — a second
            // solid stamp at the boundary's before-glyph logical-end edge,
            // present only at a bidi direction boundary. Reuse the SAME
            // entry/color/clip/page; the stamp key is already queued below
            // (do NOT push it twice).
            if let Some(sec) = cv.secondary {
                new_glyphs.push(GlyphAlphaInstance {
                    rect: repivot_origin(
                        caret_stamp_rect(origin, sec, scale_factor),
                        translation,
                        glyph_affine,
                    ),
                    uv: stamp_uv(&entry),
                    color: linear_color(color),
                    clip,
                    page: entry.page as u32,
                    affine: glyph_affine,
                });
            }
            // § 6.3: the stamp key joins the un-gated touch pass — a
            // retained caret idling past eviction_grace must not lose
            // its cell.
            new_keys.push(solid_stamp_key());
        }
    }
    // T8 D1: attribute this entity's contiguous instance slice. The
    // prepare partition maps the entity to its ExtractedNode.group off
    // the fresh node list — the producer learns nothing about groups.
    let entity_end = new_glyphs.len() as u32;
    if entity_end > entity_start {
        new_runs.push(GlyphEntityRun {
            entity,
            instances: entity_start..entity_end,
        });
        // Partial-reextract D2: the entity's key range, recorded AT emission
        // under the SAME push condition (every instance-emitting path pushes
        // at least one key, so the run sets coincide). Deliberately NOT
        // derivable from `entity_runs`: the bidi secondary caret stamp above
        // pushes a second instance but no second key, so keys are not 1:1
        // with instances.
        new_key_runs.push((entity, key_start..new_keys.len() as u32));
    } else {
        // No instances ⇒ no keys (a key only ever accompanies an instance);
        // an unattributed key would break `key_runs`' gapless cover of
        // `resident.keys`.
        debug_assert_eq!(key_start as usize, new_keys.len());
    }
}

/// What a rasterized glyph image's `(content, width, height)` triple resolves
/// to — the PURE decision behind [`resolve_glyph`]. The zero-area guard wins
/// over every content kind: a glyph with no covered texels emits nothing
/// regardless of what swash would have produced, so [`classify_glyph_content`]
/// checks dimensions before the content match. Side effects (the color-emoji
/// warn-once, the atlas bake) live in `resolve_glyph`; this stays a total,
/// allocation-free function so the branch table is unit-testable headless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolveAction {
    /// `SwashContent::Mask` with nonzero area: bake the coverage bitmap into
    /// the atlas and cache the bearing (§ 2 — the only emitting arm).
    Bake,
    /// `SwashContent::Color` with nonzero area: skip + warn-once (§ 9 — the
    /// C-tier IconInstance/ColorRgba8 seam, named not built).
    SkipColorEmoji,
    /// `SwashContent::SubpixelMask` with nonzero area: skip (the producer
    /// never requests subpixel-RGB — glyph-pipeline § 5.1).
    SkipSubpixel,
    /// Zero placement width or height, ANY content: zero-coverage
    /// (whitespace) emits no instance, inserts nothing (§ 2).
    SkipZeroArea,
}

/// The pure content→action decision: the zero-area guard first (it dominates
/// every content kind), then the `SwashContent` match. Total and side-effect
/// free — `resolve_glyph` owns the warn-once and the atlas bake.
fn classify_glyph_content(content: SwashContent, width: u32, height: u32) -> ResolveAction {
    if width == 0 || height == 0 {
        return ResolveAction::SkipZeroArea;
    }
    match content {
        SwashContent::Mask => ResolveAction::Bake,
        SwashContent::Color => ResolveAction::SkipColorEmoji,
        SwashContent::SubpixelMask => ResolveAction::SkipSubpixel,
    }
}

/// Residency + bearing for one glyph key. A hit with a cached bearing is
/// lock-free; otherwise rasterize via `SwashCache::get_image_uncached`
/// (lock site #3 — one cache, not two: the atlas is the only bitmap cache,
/// § 3.2) and insert. `None` = emit nothing, insert nothing: zero-coverage
/// (whitespace) or `SwashContent::Color` (§ 9: skip + warn-once — the
/// C-tier IconInstance/ColorRgba8 seam, named not built). The branch table is
/// factored into the pure [`classify_glyph_content`]; only the side effects
/// (warn-once, atlas bake) stay here.
fn resolve_glyph<'a>(
    atlas: &mut BuiyAtlas,
    meta: &mut GlyphMetaCache,
    fonts: &'a SharedFontSystem,
    font_guard: &mut Option<MutexGuard<'a, FontSystem>>,
    swash: &mut BuiySwashCache,
    key: &AtlasKey,
    cache_key: CacheKey,
) -> Option<(AtlasEntry, GlyphBearing)> {
    if let (Some(entry), Some(bearing)) = (atlas.get(key), meta.0.get(key).copied()) {
        return Some((entry, bearing));
    }
    let font_system = font_guard.get_or_insert_with(|| fonts.lock());
    let image = swash.0.get_image_uncached(font_system, cache_key)?;
    debug_assert!(
        image.content != SwashContent::SubpixelMask,
        "the producer never requests subpixel-RGB (glyph-pipeline § 5.1)"
    );
    match classify_glyph_content(image.content, image.placement.width, image.placement.height) {
        ResolveAction::Bake => {
            let bearing = GlyphBearing {
                left: image.placement.left,
                top: image.placement.top,
            };
            let bitmap = AtlasBitmap {
                size: UVec2::new(image.placement.width, image.placement.height),
                format: AtlasFormat::CoverageR8,
                data: image.data,
            };
            // The closure moves the prebuilt bitmap (the drain_warmup
            // precedent) — it still runs only on a miss; on the
            // meta-miss-but-resident edge it is simply not called.
            let entry = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, move || bitmap);
            meta.0.insert(key.clone(), bearing);
            Some((entry, bearing))
        }
        ResolveAction::SkipColorEmoji => {
            warn_once_color_emoji_skipped();
            None
        }
        // § 2: zero-coverage glyphs and the never-requested subpixel-RGB path
        // emit no instance, insert nothing.
        ResolveAction::SkipSubpixel | ResolveAction::SkipZeroArea => None,
    }
}

/// Emit ONE shaped glyph as a `GlyphAlphaInstance` into `new_glyphs` (+ its
/// `AtlasKey` into `new_keys`): resolve the atlas cell (rasterizing on miss —
/// lock site #3), then push the straight-alpha instance with the caller's
/// already-resolved `color` and `clip`. A zero-coverage glyph (whitespace /
/// color-emoji skip) pushes NOTHING. The caller owns the COLOR decision (the
/// editor loop's selection re-tint + Block alpha; the placeholder branch's flat
/// token tint) — this is the SHARED emit body the two paths reuse verbatim (M4:
/// the placeholder is its own buffer, but the per-glyph atlas-and-push is one
/// emitter).
#[allow(clippy::too_many_arguments)]
fn emit_glyph<'a>(
    atlas: &mut BuiyAtlas,
    meta: &mut GlyphMetaCache,
    fonts: &'a SharedFontSystem,
    font_guard: &mut Option<MutexGuard<'a, FontSystem>>,
    swash: &mut BuiySwashCache,
    interner: &mut FontKeyInterner,
    new_glyphs: &mut Vec<GlyphAlphaInstance>,
    new_keys: &mut Vec<AtlasKey>,
    phys: &cosmic_text::PhysicalGlyph,
    color: [f32; 4],
    clip: [f32; 4],
    scale_factor: f32,
    // The entity translation + 2D affine so a rotated/scaled run rotates rigidly
    // about the transform-origin. The full-origin `physical()` binning above is
    // UNCHANGED (subpixel atlas cell identical); we only re-pivot the resulting
    // window rect's origin. Identity affine ⇒ rect byte-identical.
    translation: Vec2,
    glyph_affine: [f32; 4],
) {
    let key = glyph_atlas_key(&phys.cache_key, interner);
    let Some((entry, bearing)) =
        resolve_glyph(atlas, meta, fonts, font_guard, swash, &key, phys.cache_key)
    else {
        return; // zero coverage (whitespace) or color-emoji skip (§ 9)
    };
    if entry.page > 0 {
        warn_once_page_overflow(); // § 11.1 v1 mitigation
    }
    let rect = glyph_rect_logical(phys.x, phys.y, bearing, entry.px.size(), scale_factor);
    new_glyphs.push(GlyphAlphaInstance {
        rect: repivot_origin(rect, translation, glyph_affine),
        uv: [
            entry.uv.min.x,
            entry.uv.min.y,
            entry.uv.max.x,
            entry.uv.max.y,
        ],
        color,
        clip,
        page: entry.page as u32,
        affine: glyph_affine,
    });
    new_keys.push(key);
}

/// One § 5.1 selection rect: a highlight span unioned with the run's
/// (line_top, line_height), origin-folded, at quad seat 2. No § 3.3 snap:
/// selection rects are tall boxes, not hairlines (the snap rule exists
/// for sub-pixel-thin analytics; box-edge AA here matches node
/// backgrounds).
#[allow(clippy::too_many_arguments)]
fn push_selection_quad(
    quads: &mut Vec<TextQuad>,
    entity: Entity,
    origin: Vec2,
    x: f32,
    w: f32,
    run: &cosmic_text::LayoutRun,
    color: Color,
    clip: Option<ClipRect>,
) {
    if w <= 0.0 {
        return;
    }
    quads.push(TextQuad {
        entity,
        position: Vec2::new(origin.x + x, origin.y + run.line_top),
        size: Vec2::new(w, run.line_height),
        color,
        clip,
    });
}

/// CPU-linearize a resolved color into the straight-alpha instance slot —
/// exactly the quad path (`render/instance.rs` `LinearRgba::from`).
fn linear_color(color: Color) -> [f32; 4] {
    let lin = LinearRgba::from(color);
    [lin.red, lin.green, lin.blue, lin.alpha]
}

/// One cosmic sRGB8 color into the Buiy `Color` tier — one conversion, two
/// sinks: the per-span glyph override and the decoration precedence walk.
fn cosmic_color(c: cosmic_text::Color) -> Color {
    Color::srgba_u8(c.r(), c.g(), c.b(), c.a())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` fabricated entities (a scratch `World` mints real ids).
    fn entities(n: usize) -> Vec<Entity> {
        let mut world = World::new();
        (0..n).map(|_| world.spawn_empty().id()).collect()
    }

    /// Retained runs naming `ents`, one 1-instance run each (the classifier
    /// reads only `entity`; the ranges are inert here).
    fn runs(ents: &[Entity]) -> Vec<GlyphEntityRun> {
        ents.iter()
            .enumerate()
            .map(|(i, &entity)| GlyphEntityRun {
                entity,
                instances: i as u32..i as u32 + 1,
            })
            .collect()
    }

    fn classify(
        global: bool,
        structural: bool,
        skip_lifted: bool,
        degraded: bool,
        changed: &[Entity],
        despawned: &[Entity],
        retained: &[GlyphEntityRun],
    ) -> GlyphDamage {
        classify_glyph_damage(
            global,
            structural,
            skip_lifted,
            degraded,
            changed,
            despawned,
            retained,
        )
    }

    // ---- D3 rule 1: whole-set escalations --------------------------------

    #[test]
    fn global_trigger_classifies_full() {
        let e = entities(3);
        let retained = runs(&e);
        assert_eq!(
            classify(true, false, false, false, &e[..1], &[], &retained),
            GlyphDamage::Full,
            "theme/scale/fonts are whole-set by nature (D3)"
        );
    }

    #[test]
    fn structural_change_classifies_full() {
        let e = entities(3);
        let retained = runs(&e);
        assert_eq!(
            classify(false, true, false, false, &e[..1], &[], &retained),
            GlyphDamage::Full,
            "a paint reorder / group-membership change is Patch-unsafe (D3)"
        );
    }

    #[test]
    fn skip_lift_classifies_full() {
        let e = entities(3);
        let retained = runs(&e);
        assert_eq!(
            classify(false, false, true, false, &[], &[], &retained),
            GlyphDamage::Full,
            "hide→show re-insertion: order position unknown without the walk (D3)"
        );
    }

    #[test]
    fn live_degradation_classifies_full() {
        let e = entities(3);
        let retained = runs(&e);
        assert_eq!(
            classify(false, false, false, true, &e[..1], &[], &retained),
            GlyphDamage::Full,
            "the alpha-fold repacks the WHOLE buffer from source (D3)"
        );
    }

    // ---- D3 rule 2: Added / newly-emitting detection ----------------------

    #[test]
    fn changed_entity_absent_from_retained_runs_classifies_full() {
        let e = entities(4);
        // e[3] never emitted (Added, or newly non-whitespace, or re-shown).
        let retained = runs(&e[..3]);
        assert_eq!(
            classify(false, false, false, false, &e[3..], &[], &retained),
            GlyphDamage::Full,
            "absence-from-retained-runs IS the Added detection (D3)"
        );
    }

    // ---- D3 rule 3: the changed-set-fraction bail --------------------------

    #[test]
    fn changed_fraction_above_threshold_classifies_full() {
        let e = entities(3);
        let retained = runs(&e);
        // 2 of 3 = 66 % > GLYPH_PATCH_MAX_CHANGED_PERCENT (50 %): the scroll
        // degeneracy — splice-all must not cost more than wholesale (D3).
        assert_eq!(
            classify(false, false, false, false, &e[..2], &[], &retained),
            GlyphDamage::Full
        );
    }

    #[test]
    fn changed_fraction_at_threshold_stays_patch() {
        let e = entities(4);
        let retained = runs(&e);
        // 2 of 4 = exactly 50 %: the bail is STRICTLY-greater-than.
        assert_eq!(
            classify(false, false, false, false, &e[..2], &[], &retained),
            GlyphDamage::Patch {
                changed: SmallVec::from_slice(&e[..2]),
                removed: SmallVec::new(),
            }
        );
    }

    // ---- D3 rule 4: the Patch verdicts -------------------------------------

    #[test]
    fn plain_resident_value_change_classifies_patch() {
        let e = entities(3);
        let retained = runs(&e);
        assert_eq!(
            classify(false, false, false, false, &e[..1], &[], &retained),
            GlyphDamage::Patch {
                changed: SmallVec::from_slice(&e[..1]),
                removed: SmallVec::new(),
            }
        );
    }

    #[test]
    fn resident_despawn_classifies_patch_delete() {
        let e = entities(3);
        let retained = runs(&e);
        assert_eq!(
            classify(false, false, false, false, &[], &e[..1], &retained),
            GlyphDamage::Patch {
                changed: SmallVec::new(),
                removed: SmallVec::from_slice(&e[..1]),
            },
            "despawns ARE patchable: splice-delete keyed by the removal ids (D3)"
        );
    }

    #[test]
    fn non_resident_despawn_is_a_noop_patch() {
        let e = entities(4);
        let retained = runs(&e[..3]);
        // EVERY node despawn rides RemovedComponents<ResolvedLayout>; a
        // text-free despawn must not escalate (or even splice).
        assert_eq!(
            classify(false, false, false, false, &[], &e[3..], &retained),
            GlyphDamage::Patch {
                changed: SmallVec::new(),
                removed: SmallVec::new(),
            }
        );
    }

    #[test]
    fn despawned_id_in_the_changed_set_is_a_delete_not_a_reemit() {
        let e = entities(3);
        let retained = runs(&e);
        // A despawn also fires the value-clear streams (every component is
        // removed), so the id can arrive in BOTH inputs — delete wins.
        assert_eq!(
            classify(false, false, false, false, &e[..1], &e[..1], &retained),
            GlyphDamage::Patch {
                changed: SmallVec::new(),
                removed: SmallVec::from_slice(&e[..1]),
            }
        );
    }
}

/// Per-span `LayoutGlyph.color_opt` override (§ 7) — cosmic carries sRGB8.
fn span_color(c: cosmic_text::Color) -> [f32; 4] {
    linear_color(cosmic_color(c))
}

static WARNED_COLOR_EMOJI: AtomicBool = AtomicBool::new(false);
static WARNED_PAGE_OVERFLOW: AtomicBool = AtomicBool::new(false);

/// § 9's rate-limited warn (the components.rs warn-once precedent).
fn warn_once_color_emoji_skipped() {
    if !WARNED_COLOR_EMOJI.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: color (emoji) glyphs are skipped in v1 — the ColorRgba8/\
             IconInstance path is a named C-tier seam (glyph-pipeline § 9; \
             warned once)"
        );
    }
}

/// § 11.1's v1 mitigation: the @group(1) bind group samples page 0 only.
fn warn_once_page_overflow() {
    if !WARNED_PAGE_OVERFLOW.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: a glyph allocated on coverage page > 0, but the glyph draw \
             binds page 0 only — those glyphs will sample wrong texels. Time \
             to build the multi-page bind (glyph-pipeline § 11.1; warned once)"
        );
    }
}

#[cfg(test)]
mod classify_glyph_content_tests {
    //! T2.11 (#26): the `SwashContent` → `ResolveAction` decision behind
    //! `resolve_glyph`, unit-tested headless. The only emoji fixture is
    //! monochrome (`Mask`), so the `Color`/`SubpixelMask`/zero-area arms have
    //! no end-to-end coverage; a `Mask`↔`Color` swap in the table would
    //! silently drop or mis-bake glyphs at runtime. These pin every arm,
    //! including the zero-area guard's precedence over every content kind.

    use super::{ResolveAction, classify_glyph_content};
    use cosmic_text::SwashContent;

    #[test]
    fn mask_with_nonzero_area_bakes() {
        // The only emitting arm: a covered monochrome glyph is baked. A
        // Mask↔Color swap in the helper turns this into SkipColorEmoji and
        // fails here (every visible glyph would vanish).
        assert_eq!(
            classify_glyph_content(SwashContent::Mask, 4, 6),
            ResolveAction::Bake
        );
    }

    #[test]
    fn color_with_nonzero_area_skips_as_color_emoji() {
        // The § 9 skip+warn arm. A Mask↔Color swap turns this into Bake and
        // fails here (color emoji would be mis-baked as coverage).
        assert_eq!(
            classify_glyph_content(SwashContent::Color, 8, 8),
            ResolveAction::SkipColorEmoji
        );
    }

    #[test]
    fn subpixel_mask_with_nonzero_area_skips_as_subpixel() {
        assert_eq!(
            classify_glyph_content(SwashContent::SubpixelMask, 8, 8),
            ResolveAction::SkipSubpixel
        );
    }

    #[test]
    fn zero_area_skips_regardless_of_content() {
        // The guard dominates every content kind: zero width OR zero height,
        // for Mask / Color / SubpixelMask alike, resolves to SkipZeroArea —
        // never Bake, never the content-specific skip.
        for content in [
            SwashContent::Mask,
            SwashContent::Color,
            SwashContent::SubpixelMask,
        ] {
            assert_eq!(
                classify_glyph_content(content, 0, 6),
                ResolveAction::SkipZeroArea,
                "zero width must skip for {content:?}"
            );
            assert_eq!(
                classify_glyph_content(content, 4, 0),
                ResolveAction::SkipZeroArea,
                "zero height must skip for {content:?}"
            );
            assert_eq!(
                classify_glyph_content(content, 0, 0),
                ResolveAction::SkipZeroArea,
                "zero width and height must skip for {content:?}"
            );
        }
    }
}
