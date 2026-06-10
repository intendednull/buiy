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

use crate::components::{Node, ResolvedLayout, StackingContext};
use crate::layout::Stacking;
use crate::render::atlas::{
    AtlasBitmap, AtlasEntry, AtlasFormat, AtlasKey, BuiyAtlas, GlyphAlphaInstance,
};
use crate::render::color::resolve_token;
use crate::render::components::{AncestorClip, ClipRect, ComputedPaintSkip, TextColor};
use crate::render::extract::{context_roots, context_tree_paint_order, effective_clip};
use crate::render::prepare::ExtractedGlyphs;
use crate::theme::Theme;

use super::atlas_key::{FontKeyInterner, glyph_atlas_key};
use super::components::{ComputedTextLayout, TextBuffer};
use super::font_system::SharedFontSystem;
use super::swash::BuiySwashCache;

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
/// Binds `&TextBuffer` directly — `TextBufferAccess` is deferred to the
/// editing campaign (T3 decision 12 supersedes § 6.1's accessor mention;
/// the swap is mechanical when `TextEditState` exists). `layout_runs` is
/// `&self`, so the main-world read stays read-only.
///
/// § 6.2 ledger — union members that join later, in lockstep with their
/// carriers: `Changed<CaretVisual>` / `Changed<SelectionVisual>` (T7);
/// `ExtractedTextQuads` rebuilt alongside `ExtractedGlyphs` (T6, same
/// producer, same probe, one damage decision).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn extract_buiy_glyphs(
    mut atlas: ResMut<BuiyAtlas>,
    mut glyphs: ResMut<ExtractedGlyphs>,
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
                &TextBuffer,
                &ComputedTextLayout,
                Option<&TextColor>,
                Option<&ComputedPaintSkip>,
                Option<&ClipRect>,
                Option<&AncestorClip>,
                Option<&Stacking>,
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
                )>,
            ),
        >,
    >,
    // Despawn + hide→show: the two damage sources Changed cannot see.
    mut removed: Extract<RemovedComponents<ResolvedLayout>>,
    mut removed_skip: Extract<RemovedComponents<ComputedPaintSkip>>,
    contexts: Extract<Query<(Entity, &StackingContext)>>,
    theme: Extract<Res<Theme>>,
    primary: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    // Drain the removal streams FIRST so the cursors advance on every frame,
    // including early returns (the extract.rs:409 discipline).
    let despawned = removed.read().count() > 0;
    let skip_lifted = removed_skip.read().count() > 0;

    let Ok(window) = primary.single() else {
        // Vanished window: clear the carrier ONCE (an unconditional clear
        // would mark ExtractedGlyphs changed and re-upload an empty buffer
        // every frame).
        if !glyphs.glyphs.is_empty() {
            glyphs.glyphs.clear();
            resident.keys.clear();
        }
        return;
    };
    let scale_factor = window.resolution.scale_factor();
    let scale_changed = resident.last_scale_factor != Some(scale_factor);

    let dirty =
        !changed.is_empty() || despawned || skip_lifted || theme.is_changed() || scale_changed;
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

    // ---- Rebuild (wholesale, § 6.2 v1) -------------------------------
    let mut new_glyphs: Vec<GlyphAlphaInstance> = Vec::new();
    let mut new_keys: Vec<AtlasKey> = Vec::new();
    // Lock site #3 (architecture § 1.2 — the LAST of the exhaustive three):
    // taken LAZILY, once per frame, only when at least one glyph misses the
    // atlas (the text_commit guard pattern). A hit-only rebuild takes ZERO
    // locks; extract runs in the pipelining sync window, so the lock is
    // uncontended by construction.
    let mut font_guard: Option<MutexGuard<'_, FontSystem>> = None;
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
        let Ok((gt, buffer, computed, color, skip, clip_rect, ancestor_clip, stacking)) =
            texts.get(entity)
        else {
            continue; // not a text painter
        };
        if skip.is_some() {
            continue; // the single computed skip source (§ 5.3/§ 5.4)
        }
        // § 8: glyphs are CONTENT — self-inclusive clip (own ClipRect ∩
        // ancestors); top-layer members force the unclipped sentinel.
        let clip = pack_clip(effective_clip(stacking, clip_rect, ancestor_clip).as_ref());
        // § 7: resolved at extract like Background, CPU-linearized,
        // STRAIGHT alpha (premultiplying would double-dim — primitive.rs).
        let entity_color = linear_color(resolve_token(&color.unwrap_or(&default_color).0, theme));
        let origin = gt.translation().truncate() + computed.content_offset;

        let mut runs = 0usize;
        for run in buffer.buffer.layout_runs() {
            runs += 1;
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
                // Per-span Attrs color override rides through (§ 7).
                let color = glyph.color_opt.map(span_color).unwrap_or(entity_color);
                if color[3] == 0.0 {
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
        }
        // architecture § 3.2 tripwire: layout_runs TERMINATES at the first
        // unshaped line — a mismatch means something mutated the buffer
        // after TextCommit.
        debug_assert_eq!(
            runs,
            computed.lines.len(),
            "TextBuffer dirty-unshaped at extract (mutated after TextCommit?)"
        );
    }
    drop(font_guard);

    // Bearing-cache hygiene (decision 3): prune to atlas residency, so the
    // map is bounded by the atlas's own budget — invariant: every resident
    // glyph key has a bearing.
    meta.0.retain(|key, _| atlas.get(key).is_some());

    // Publish, then the § 6.3 touch pass over the NEW visible set (covers
    // this frame's hits — `atlas.get` deliberately does not touch the LRU).
    glyphs.glyphs = new_glyphs;
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

/// CPU-linearize a resolved color into the straight-alpha instance slot —
/// exactly the quad path (`render/instance.rs` `LinearRgba::from`).
fn linear_color(color: Color) -> [f32; 4] {
    let lin = LinearRgba::from(color);
    [lin.red, lin.green, lin.blue, lin.alpha]
}

/// Per-span `LayoutGlyph.color_opt` override (§ 7) — cosmic carries sRGB8.
fn span_color(c: cosmic_text::Color) -> [f32; 4] {
    linear_color(Color::srgba_u8(c.r(), c.g(), c.b(), c.a()))
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
