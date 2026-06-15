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
    AtlasBitmap, AtlasEntry, AtlasFormat, AtlasKey, BuiyAtlas, GlyphAlphaInstance,
};
use crate::render::color::{
    resolve_caret_color, resolve_preedit_underline, resolve_selection_bg, resolve_selection_fg,
    resolve_token,
};
use crate::render::components::{AncestorClip, CaretColor, ClipRect, ComputedPaintSkip, TextColor};
use crate::render::extract::{
    ExtractedTextQuads, TextQuad, context_roots, context_tree_paint_order, effective_clip,
};
use crate::render::prepare::{ExtractedGlyphs, GlyphEntityRun};
use crate::theme::Theme;

use super::atlas_key::{FontKeyInterner, glyph_atlas_key};
use super::components::{
    CaretVisual, ComputedTextLayout, PreeditVisual, SelectionVisual, TextBuffer, TextDecorations,
};
use super::decoration::{DecorationKind, span_decoration_rects, span_x_extent};
use super::edit::TextBufferAccessReadOnly;
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
/// joined with T7's caret emission. The ledger names zero open joins.
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
    contexts: Extract<Query<(Entity, &StackingContext)>>,
    theme: Extract<Res<Theme>>,
    // The main-world font-set counters (T5): VALUE-compared against the
    // retained state below — the `theme` main-world-resource extraction
    // precedent, the `last_scale_factor` compare idiom.
    generation: Extract<Res<FontsGeneration>>,
    lineage: Extract<Res<FontDbLineage>>,
    primary: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    let (mut glyphs, mut text_quads) = carriers;
    // Drain the removal streams FIRST so the cursors advance on every frame,
    // including early returns (the extract.rs:409 discipline).
    let despawned = removed.0.read().count() > 0;
    let skip_lifted = removed.1.read().count() > 0;
    // The § 7 swap-to-visible: a lifted Block (load or timeout) repaints at
    // full alpha through a normal rebuild.
    let block_lifted = removed.2.read().count() > 0;
    // T7: a selection clear is a REMOVAL — unlike the style carriers,
    // removal here IS the hide mechanism (the T2-erratum-1 exclusion does
    // not apply), so the cleared rects must repaint away.
    let selection_cleared = removed.3.read().count() > 0;
    // T7: same for the caret — removal = focus loss, the stamp must
    // repaint away.
    let caret_removed = removed.4.read().count() > 0;
    // E5: a preedit clear (commit / cancel / focus-loss) removes the
    // component — removal IS the clear, so the underline must repaint away.
    let preedit_removed = removed.5.read().count() > 0;

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
        }
        return;
    };
    let scale_factor = window.resolution.scale_factor();
    let scale_changed = resident.last_scale_factor != Some(scale_factor);
    let fonts_changed = resident.last_generation != Some(generation.0);

    let dirty = !changed.is_empty()
        || despawned
        || skip_lifted
        || block_lifted
        || selection_cleared
        || caret_removed
        || preedit_removed
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
    // Lock site #3 (architecture § 1.2 — the LAST of the exhaustive three):
    // taken LAZILY, once per frame, only when at least one glyph misses the
    // atlas (the text_commit guard pattern). A hit-only rebuild takes ZERO
    // locks; extract runs in the pipelining sync window, so the lock is
    // uncontended by construction.
    let mut font_guard: Option<MutexGuard<'_, FontSystem>> = None;
    // One stamp-residency probe per frame (T6, decoration-and-paint § 4.3):
    // the solid stamp's closure never touches the `FontSystem`, so this
    // stays outside lock site #3; a grace-evicted stamp self-heals here on
    // the next line-through ("re-inserted on miss like any
    // content-addressed entry").
    let mut stamp_entry: Option<AtlasEntry> = None;
    let fonts: &SharedFontSystem = &fonts;
    let theme: &Theme = &theme;

    // painters_z order — the same walk extract_buiy_nodes runs (§ 2).
    let sc_by_entity: std::collections::HashMap<Entity, &[Entity]> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.painters_z.as_slice()))
        .collect();
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { sc_by_entity.get(&e).copied() };
    let mut order = Vec::new();
    for root in context_roots(&sc_by_entity) {
        context_tree_paint_order(root, &painters_z_of, &mut order);
    }

    let default_color = TextColor::default();
    for entity in order {
        let Ok((
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
        )) = texts.get(entity)
        else {
            continue; // not a text painter
        };
        if skip.is_some() {
            continue; // the single computed skip source (§ 5.3/§ 5.4)
        }
        let entity_start = new_glyphs.len() as u32;
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
                                &mut new_quads,
                                entity,
                                origin,
                                0.0,
                                full_w,
                                &run,
                                bg,
                                eff_clip,
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
                                    &mut new_quads,
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
                    let Some((x_start, width)) = span_x_extent(run.glyphs, &span.glyph_range)
                    else {
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
                    let key = glyph_atlas_key(&phys.cache_key, &mut interner);
                    let Some((entry, bearing)) = resolve_glyph(
                        &mut atlas,
                        &mut meta,
                        fonts,
                        &mut font_guard,
                        &mut swash,
                        &key,
                        phys.cache_key,
                    ) else {
                        continue; // zero coverage (whitespace) or color-emoji skip (§ 9)
                    };
                    if entry.page > 0 {
                        warn_once_page_overflow(); // § 11.1 v1 mitigation
                    }
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
                    new_glyphs.push(GlyphAlphaInstance {
                        rect: glyph_rect_logical(
                            phys.x,
                            phys.y,
                            bearing,
                            entry.px.size(),
                            scale_factor,
                        ),
                        uv: [
                            entry.uv.min.x,
                            entry.uv.min.y,
                            entry.uv.max.x,
                            entry.uv.max.y,
                        ],
                        color,
                        clip,
                        page: entry.page as u32,
                    });
                    new_keys.push(key);
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
                            rect,
                            uv: stamp_uv(&entry),
                            color,
                            clip,
                            page: entry.page as u32,
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

        // T7 § 6.1 — seat 6: the caret paints last, over glyphs and
        // line-through, as a solid-stamp instance (pre-phase decision 2).
        // Painted under Block too (chrome, not ink — decision 9: browsers
        // keep the caret in a focused field whose font is loading).
        if let Some(cv) = caret_visual
            && cv.visible
        {
            // § 6.2: explicit token → theme caret key (presence check) →
            // currentColor. Re-tint only — never an atlas mutation.
            let color =
                resolve_caret_color(caret_color.map(|c| &c.0), theme, resolved_entity_color);
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
                    rect: caret_stamp_rect(origin, cv.rect, scale_factor),
                    uv: stamp_uv(&entry),
                    color: linear_color(color),
                    clip,
                    page: entry.page as u32,
                });
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
        }
    }
    drop(font_guard);

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
    for key in &resident.keys {
        atlas.touch_existing(key);
    }
}

/// Residency + bearing for one glyph key. A hit with a cached bearing is
/// lock-free; otherwise rasterize via `SwashCache::get_image_uncached`
/// (lock site #3 — one cache, not two: the atlas is the only bitmap cache,
/// § 3.2) and insert. `None` = emit nothing, insert nothing: zero-coverage
/// (whitespace) or `SwashContent::Color` (§ 9: skip + warn-once — the
/// C-tier IconInstance/ColorRgba8 seam, named not built).
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
    if image.placement.width == 0 || image.placement.height == 0 {
        return None; // § 2: zero-coverage glyphs emit no instance, insert nothing
    }
    match image.content {
        SwashContent::Mask => {
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
        SwashContent::Color => {
            warn_once_color_emoji_skipped();
            None
        }
        SwashContent::SubpixelMask => None,
    }
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
