//! The render-world vector-icon producer (parity Wave B3, parity-design § 3.5).
//!
//! Per frame, per visible [`Icon`] entity:
//! rasterize the SVG path to an `R8` coverage bitmap (`render::icon_raster`),
//! insert it into the SHARED glyph-alpha atlas keyed by `hash(path_d,
//! stroke_width, size, fill)` (content-addressed dedup — the same icon authored
//! twice is one atlas cell), and emit one [`GlyphAlphaInstance`] tinted by the
//! icon's resolved color token into the [`ExtractedIcons`] carrier.
//!
//! ## Why a SEPARATE carrier (not `ExtractedGlyphs`)
//!
//! The text glyph producer (`text::extract_buiy_glyphs`) OWNS `ExtractedGlyphs`
//! outright: it rebuilds the carrier wholesale on `GlyphDamage::Full` frames and
//! splice-patches it in place on `Patch` frames (the 2026-07-03 keyed partial
//! re-extract, `docs/specs/2026-07-03-glyph-partial-reextract-design.md`) —
//! either way, anything an icon producer pushed into that resource would be
//! clobbered or spliced over. And `buckets::partition_glyph_ranges` debug-asserts
//! the entity runs are contiguous-from-0 covering every instance, so two
//! independently built sources cannot share one partition. So icons get their OWN
//! carrier + their OWN GPU buffer (`BuiyInstanceBuffers::icon`) + their OWN draw
//! — but that draw reuses the EXISTING coverage pipeline + atlas bind group + the
//! `coverage.wgsl` shader VERBATIM (the icon record IS a `GlyphAlphaInstance`),
//! so there is **no new GPU shader/pipeline code**. The two producers stay fully
//! decoupled; the text path is untouched. This ICON tier itself still rebuilds
//! `ExtractedIcons` wholesale on its dirty frames — a candidate for the same
//! Full/Patch treatment (follow-up filed in `docs/plans/follow-ups.md`).
//!
//! ## Live re-tint
//!
//! The atlas cell is monochrome coverage; the color rides the instance. A
//! `SetAccent` swatch click mutates `Res<Theme>` → `theme.is_changed()` → this
//! producer re-resolves the token and re-emits with the new color, no atlas
//! mutation (parity-design § 3.5 "live recolor"). That is exactly the
//! alpha-as-color trick text glyphs use.
//!
//! ## Scheduling + residency
//!
//! Runs in `ExtractSchedule` `.after(maintain_atlas)` (the glyph-producer
//! precedent) so inserts/touches use the just-advanced atlas frame clock. Every
//! emitted key joins [`ResidentIconKeys`], touched un-gated each frame so a
//! retained-but-painted icon never grace-evicts its cell out from under a live
//! instance (the `ResidentTextKeys` § 6.3 hazard, mirrored).

use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::render::Extract;

use crate::components::{Node, ResolvedLayout};
use crate::layout::Stacking;
use crate::render::atlas::{AtlasEntryKind, AtlasFormat, AtlasKey, BuiyAtlas, GlyphAlphaInstance};
use crate::render::color::resolve_token;
use crate::render::components::{AncestorClip, ClipRect, ComputedPaintSkip, Icon};
use crate::render::extract::effective_clip;
use crate::render::icon_raster::{IconPaint, rasterize_icon};
use crate::theme::Theme;

/// The icon equivalent of [`ExtractedGlyphs`](crate::render::prepare::ExtractedGlyphs):
/// the per-frame list of icon coverage instances, retained across steady frames
/// so `is_changed()` is the prepare-side damage signal. Each instance is a
/// `GlyphAlphaInstance` — byte-identical to a text glyph — so the prepare splice
/// packs it into the shared coverage buffer and `coverage.wgsl` draws it with no
/// new GPU code.
#[derive(Resource, Default)]
pub struct ExtractedIcons {
    /// One instance per visible icon, in entity-iteration order. (Icons paint at
    /// the coverage tier, right after text glyphs — both are coverage stamps.)
    pub icons: Vec<GlyphAlphaInstance>,
    /// One run per emitting entity, contiguous-from-0 covering `icons`, for the
    /// effect-group partition (`partition_glyph_ranges`) — its own self-consistent
    /// source, so the contiguity assert holds trivially.
    pub entity_runs: Vec<IconEntityRun>,
}

/// One emitting entity's contiguous slice of [`ExtractedIcons::icons`] — the
/// group-partition lookup key (the `GlyphEntityRun` mirror). v1 emits exactly one
/// icon instance per `Icon` entity, so each run is a single index, but the run
/// shape keeps the partition path identical to the glyph tier.
#[derive(Clone, Debug, PartialEq)]
pub struct IconEntityRun {
    pub entity: Entity,
    pub instances: std::ops::Range<u32>,
}

/// The producer's retained touch-key set (the [`ResidentTextKeys`] mirror): every
/// `AtlasKey` the current `ExtractedIcons` samples, touched UN-gated each frame so
/// a retained-but-painted icon stays LRU-warm and never grace-evicts its cell.
/// Also caches the last-seen scale factor — the value-compare damage probe (a
/// scale change re-rasterizes at the new physical size).
///
/// [`ResidentTextKeys`]: crate::text::ResidentTextKeys
#[derive(Resource, Default)]
pub struct ResidentIconKeys {
    /// One entry per emitted instance, in emission order.
    pub keys: Vec<AtlasKey>,
    /// `None` until the first rebuild seeds it.
    pub last_scale_factor: Option<f32>,
}

/// Build the content-addressed [`AtlasKey`] for an icon coverage cell:
/// `[Mask kind byte, sub-id 1, <16-byte FNV-1a hash of (path_d, stroke_width,
/// size, fill)>]`. The `Mask` kind byte is the reserved "sampled exactly like a
/// glyph" coverage kind (atlas types.rs); sub-id **1** distinguishes icon keys
/// from the solid stamp's `[Mask, 0]` (text/stamp.rs) so the two never alias.
/// The hash content-addresses the icon so the same `(d, width, size, fill)`
/// authored twice resolves to ONE atlas cell (dedup), and a different stroke
/// width or size is a distinct cell.
pub fn icon_atlas_key(
    path_d: &str,
    stroke_width: f32,
    size_px: u16,
    viewbox: f32,
    fill: bool,
) -> AtlasKey {
    // FNV-1a over the canonical content tuple. f32 hashed by its bit pattern
    // (`to_bits`) so the key is exactly content-addressed — two `Icon`s with
    // bit-identical inputs produce the same key, which is the dedup contract.
    // `viewbox` rides the hash: the same `path_d`/`size` at a different author
    // viewBox rasterizes at a different scale, so it MUST be a distinct cell.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    mix(path_d.as_bytes());
    mix(&stroke_width.to_bits().to_le_bytes());
    mix(&size_px.to_le_bytes());
    mix(&viewbox.to_bits().to_le_bytes());
    mix(&[fill as u8]);
    // Two independent 64-bit folds → a 16-byte content hash (collision-resistant
    // enough for a ≤ few-dozen-icon atlas; the atlas is content-addressed, so a
    // theoretical collision would only share a cell, never corrupt — but the
    // 128-bit width makes even that astronomically unlikely).
    let h2 = h.rotate_left(32) ^ 0x9e37_79b9_7f4a_7c15u64.wrapping_mul(h | 1);
    let mut bytes = Vec::with_capacity(18);
    bytes.push(AtlasEntryKind::Mask.key_byte());
    bytes.push(1); // sub-id 1 = icon (0 = solid stamp)
    bytes.extend_from_slice(&h.to_le_bytes());
    bytes.extend_from_slice(&h2.to_le_bytes());
    AtlasKey::from_bytes(&bytes)
}

/// sRGB `Color` → linear-light straight-alpha `[f32; 4]` — the alpha-as-color
/// tint the coverage shader expects (the `text::extract` `linear_color` mirror;
/// straight alpha, NOT premultiplied — `coverage.wgsl` scales only alpha).
fn linear_color(color: Color) -> [f32; 4] {
    let lin = LinearRgba::from(color);
    [lin.red, lin.green, lin.blue, lin.alpha]
}

/// Whether a visible `Icon` entity emits a coverage instance this frame — the
/// pure per-entity skip predicate the producer loop consults (factored out so
/// the headless test can pin the decisions without an adapter / render world).
///
/// An icon is suppressed (emits NOTHING) when:
///   - it carries the computed paint-skip marker (`has_paint_skip` — the
///     `CssVisibility::Hidden` / `OffscreenAuto` subtree drop, the single skip
///     source the bg-quad/text paths also honor); OR
///   - it authors nothing (`path_d` empty or `size_px == 0`); OR
///   - its resolved tint is fully transparent; OR
///   - **its layout box is zero-area** (finding M5).
///
/// The zero-area-box rule is the M5 root cause: an `Icon` paints at its NATIVE
/// `size_px`, NOT the box size (`extract_buiy_icons` centers a `size_px`-wide
/// glyph in the box). So a node collapsed to `0×0` — a `Display::None` subtree
/// retains a STALE zero/old `ResolvedLayout` because `write_resolved_layout`
/// only inserts, never removes — would otherwise still rasterize its glyph at
/// the collapsed origin (a stray tofu/glyph). The bg-quad path is immune (a
/// `0×0` rect rasterizes no pixels; `extract.rs` § shadow + the SDF clamp a
/// zero rect away), so honoring the SAME zero-rect skip here aligns the icon
/// path with every other paint channel. `Display::None` alone (no
/// `CssVisibility::Hidden`) is now sufficient to hide an icon, so the gallery's
/// belt-and-suspenders `CssVisibility::Hidden` paint-skip is no longer
/// load-bearing.
pub fn icon_paints(
    has_paint_skip: bool,
    path_d: &str,
    size_px: u16,
    tint_alpha: f32,
    box_size: Vec2,
) -> bool {
    if has_paint_skip {
        return false; // the single computed paint-skip source
    }
    if path_d.is_empty() || size_px == 0 {
        return false; // nothing authored
    }
    if tint_alpha == 0.0 {
        return false; // fully transparent: nothing to paint
    }
    if box_size.x <= 0.0 || box_size.y <= 0.0 {
        return false; // zero-area box (a collapsed / Display::None node) — M5
    }
    true
}

/// The render-world icon producer. For every visible `Icon` entity it
/// rasterizes and atlas-inserts the coverage, then emits one tinted
/// `GlyphAlphaInstance` into the [`ExtractedIcons`] carrier. It is a wholesale
/// rebuild under a damage gate
/// (a theme swap, a scale change, or any `Changed` icon paint input); steady
/// frames return after only the un-gated touch pass, so the prepare-side icon
/// buffer is retained (the glyph-producer O(0)-steady contract).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn extract_buiy_icons(
    mut atlas: ResMut<BuiyAtlas>,
    mut extracted: ResMut<ExtractedIcons>,
    mut resident: ResMut<ResidentIconKeys>,
    theme: Extract<Res<Theme>>,
    icons: Extract<
        Query<
            (
                Entity,
                &GlobalTransform,
                &ResolvedLayout,
                &Icon,
                Option<&ComputedPaintSkip>,
                Option<&ClipRect>,
                Option<&AncestorClip>,
                Option<&Stacking>,
            ),
            With<Node>,
        >,
    >,
    changed: Extract<
        Query<
            Entity,
            (
                With<Icon>,
                Or<(
                    Changed<Icon>,
                    Changed<GlobalTransform>,
                    Changed<ResolvedLayout>,
                    Changed<ClipRect>,
                    Changed<AncestorClip>,
                    Changed<ComputedPaintSkip>,
                    Changed<Stacking>,
                )>,
            ),
        >,
    >,
    mut removed: Extract<RemovedComponents<Icon>>,
    primary: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
) {
    // Drain the removal stream every frame (cursor must advance even on an early
    // return) — an icon despawn/hide must repaint the cleared cell away.
    let removed_any = removed.read().count() > 0;

    let scale_factor = primary
        .single()
        .map(|w| w.resolution.scale_factor())
        .unwrap_or(1.0);
    let scale_changed = resident.last_scale_factor != Some(scale_factor);

    let dirty = !changed.is_empty() || removed_any || theme.is_changed() || scale_changed;
    if !dirty {
        // Steady state: retain the carrier (so prepare's `is_changed()` stays
        // false and the icon buffer is not re-uploaded), but run the un-gated
        // touch pass so a retained-but-painted icon never grace-evicts.
        for key in &resident.keys {
            atlas.touch_existing(key);
        }
        return;
    }
    resident.last_scale_factor = Some(scale_factor);

    let mut new_icons: Vec<GlyphAlphaInstance> = Vec::new();
    let mut new_runs: Vec<IconEntityRun> = Vec::new();
    let mut new_keys: Vec<AtlasKey> = Vec::new();
    let theme: &Theme = &theme;

    for (entity, gt, layout, icon, skip, clip_rect, ancestor_clip, stacking) in icons.iter() {
        let color = resolve_token(&icon.color, theme);
        // The single per-entity skip decision (paint-skip marker, nothing
        // authored, transparent tint, OR a zero-area box — M5). Factored into a
        // pure predicate so the headless test pins it without an adapter.
        if !icon_paints(
            skip.is_some(),
            &icon.path_d,
            icon.size_px,
            color.alpha(),
            layout.size,
        ) {
            continue;
        }

        // Rasterize + insert (idempotent: a hit returns the resident cell and the
        // closure never runs — no re-raster). Content-addressed by the icon's
        // (d, width, size, fill), so a second identical icon dedups.
        let paint = if icon.fill {
            IconPaint::Fill
        } else {
            IconPaint::Stroke
        };
        let key = icon_atlas_key(
            &icon.path_d,
            icon.stroke_width,
            icon.size_px,
            icon.viewbox,
            icon.fill,
        );
        // Clone the rasterizer inputs so the lazy closure (run ONLY on an atlas
        // miss) owns them — a hit never rasterizes.
        let (path_d, stroke_width, size_px, viewbox) =
            (icon.path_d.clone(), icon.stroke_width, icon.size_px, icon.viewbox);
        let entry = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, move || {
            rasterize_icon(&path_d, paint, stroke_width, size_px, viewbox)
        });

        // A degenerate/empty coverage cell (e.g. an unparseable `d`) carries a
        // zero-area pixel rect — skip it rather than emit an invisible instance.
        if entry.px.size().x == 0 || entry.px.size().y == 0 {
            continue;
        }

        // Place the icon CENTERED in the entity's resolved box (the design sizes
        // each icon's box to the glyph). The icon's box-local top-left is the
        // centering offset; `gt.transform_point` folds the translation AND the 2D
        // linear part (rotation/scale about the 6e-baked transform-origin), so a
        // rotated/scaled icon spins in place. Under identity this equals the old
        // `translation + offset` (bit-identical). `coverage.wgsl` then applies the
        // same 2D affine (below) to the quad corners about this origin.
        let size = Vec2::splat(icon.size_px as f32);
        let local_tl = (layout.size - size) * 0.5;
        let pos = gt.transform_point(local_tl.extend(0.0)).truncate();
        // The 2D affine basis (GlobalTransform's linear part), columns
        // [m00,m10,m01,m11] — same convention as the quad path (render::extract).
        let m = gt.affine().matrix3;
        let affine = [m.x_axis.x, m.x_axis.y, m.y_axis.x, m.y_axis.y];

        // Self-inclusive clip (own ClipRect ∩ ancestors; a top-layer member forces
        // the unclipped sentinel) — same resolution as a glyph's clip.
        let eff_clip = effective_clip(stacking, clip_rect, ancestor_clip);
        let clip = match eff_clip {
            Some(c) => [c.min.x, c.min.y, c.max.x, c.max.y],
            None => [
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::INFINITY,
            ],
        };

        let start = new_icons.len() as u32;
        new_icons.push(GlyphAlphaInstance {
            rect: [pos.x, pos.y, size.x, size.y],
            uv: [
                entry.uv.min.x,
                entry.uv.min.y,
                entry.uv.max.x,
                entry.uv.max.y,
            ],
            color: linear_color(color),
            clip,
            page: entry.page as u32,
            affine,
        });
        new_keys.push(key);
        new_runs.push(IconEntityRun {
            entity,
            instances: start..new_icons.len() as u32,
        });
    }

    // Value-compared publish (the glyph-carrier discipline): equal inputs produce
    // bit-identical instances, so a steady rebuild keeps the tick and prepare
    // retains the buffer. Publish (icons, runs) together — group membership keys
    // on the entity, so a runs change must republish even if instances match.
    if extracted.icons != new_icons || extracted.entity_runs != new_runs {
        extracted.icons = new_icons;
        extracted.entity_runs = new_runs;
    }
    resident.keys = new_keys;
    for key in &resident.keys {
        atlas.touch_existing(key);
    }
}
