//! glyph-pipeline § 5 — the subpixel/hiDPI math, pinned against
//! hand-computed fixtures (incl. a fractional scale factor) and the
//! upstream 4-bin quantizer (a cosmic-text-bump drift tripwire).

use bevy::math::{UVec2, Vec2};
use buiy_core::render::components::ClipRect;
use buiy_core::text::{GlyphBearing, glyph_rect_logical, pack_clip, physical_offset};
use cosmic_text::{CacheKey, CacheKeyFlags, SubpixelBin, fontdb};

#[test]
fn physical_offset_folds_origin_and_baseline_in_physical_px() {
    // § 5.1: physical() applies offset AFTER scale, so the offset handed in
    // must already be physical px: (origin.x*s, (origin.y + line_y)*s).
    let (x, y) = physical_offset(Vec2::new(10.0, 20.0), 12.8, 1.25);
    assert_eq!((x, y), (12.5, 41.0));
    // Identity at scale 1.
    let (x, y) = physical_offset(Vec2::new(3.0, 4.0), 6.0, 1.0);
    assert_eq!((x, y), (3.0, 10.0));
}

#[test]
fn rect_formula_rasterize_physical_position_logical() {
    // § 5.2 verbatim: rect_px = (phys.x + left, phys.y - top, w, h);
    // rect_logical = rect_px / scale. Placement top points UP — hence the
    // subtraction. Fractional 1.25 scale (the § 11.7 hiDPI case).
    let rect = glyph_rect_logical(
        130,
        56,
        GlyphBearing { left: 2, top: 13 },
        UVec2::new(9, 12),
        1.25,
    );
    assert_eq!(rect, [105.6, 34.4, 7.2, 9.6]);

    // Scale 1, negative left bearing (italic overhang).
    let rect = glyph_rect_logical(
        40,
        30,
        GlyphBearing { left: -1, top: 8 },
        UVec2::new(5, 7),
        1.0,
    );
    assert_eq!(rect, [39.0, 22.0, 5.0, 7.0]);
}

#[test]
fn upstream_quantizer_bins_x_four_ways_and_y_truncation_zeroes_y_bin() {
    // Pins the upstream CacheKey::new quantizer the producer rides
    // (glyph_cache.rs:36–60): fract 0.25 ⇒ One; integer ⇒ Zero. physical()
    // truncf's y BEFORE binning (layout.rs:99), so the y the producer hands
    // the quantizer is always integral ⇒ y_bin structurally Zero — § 5.1's
    // claim, drift-checked on every cosmic-text bump.
    let font_id = buiy_core::text::registered_fonts_db()
        .faces()
        .next()
        .unwrap()
        .id;
    let (key, x, y) = CacheKey::new(
        font_id,
        7,
        20.0,
        (19.25, 33.9_f32.trunc()), // x fract .25; y pre-truncated as physical() does
        fontdb::Weight(400),
        CacheKeyFlags::empty(),
    );
    assert_eq!((x, y), (19, 33));
    assert_eq!(key.x_bin, SubpixelBin::One);
    assert_eq!(key.y_bin, SubpixelBin::Zero);
    // Carry above 0.875 (the bin-table edge).
    let (key, x, _) = CacheKey::new(
        font_id,
        7,
        20.0,
        (19.9, 0.0),
        fontdb::Weight(400),
        CacheKeyFlags::empty(),
    );
    assert_eq!(x, 20);
    assert_eq!(key.x_bin, SubpixelBin::Zero);
}

#[test]
fn clip_packs_aabb_or_infinity_sentinel() {
    // § 8: encoding fixed by the consumer — logical-px AABB, ±INF sentinel
    // (identical to PackedInstance / the coverage.wgsl discard).
    assert_eq!(
        pack_clip(None),
        [
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::INFINITY
        ]
    );
    let clip = ClipRect {
        min: Vec2::new(1.0, 2.0),
        max: Vec2::new(30.0, 40.0),
    };
    assert_eq!(pack_clip(Some(&clip)), [1.0, 2.0, 30.0, 40.0]);
}
