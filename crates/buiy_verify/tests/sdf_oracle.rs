//! Pure-CPU (NOT #[ignore]): the full-tile CPU SDF oracle must reproduce the
//! geometry the existing render_instance.rs point-probes assert — center
//! inside (filled), 2× half-extent outside (empty). Pins the full-tile port to
//! the unit-tested shader formula. reftests.md § Verification #5.
//!
//! The oracle output is **capture-matched** (`rasterize_sdf_rect` composites
//! the box over the capture camera's opaque-black clear in linear space, then
//! sRGB-encodes — so the CPU-vs-GPU cross-check compares like-for-like). Thus
//! "filled" is opaque WHITE `[255,255,255,255]` and "empty" is opaque BLACK
//! `[0,0,0,255]` (NOT transparent) — the same geometric center-inside /
//! far-outside probe, in the composited convention.

use bevy::prelude::*;
use buiy_core::render::DrawData;
use buiy_verify::reftest::sdf_oracle::rasterize_sdf_rect;

#[test]
fn oracle_fills_center_and_clears_far_outside() {
    let inset = DrawData::new(
        Vec2::new(50.0, 25.0),
        Vec2::new(40.0, 20.0),
        Color::WHITE,
        0.0,
    );
    let img = rasterize_sdf_rect(&inset, 200, 100);
    assert_eq!(img.dimensions(), (200, 100));
    // Far outside the box → the opaque-black clear (composited convention).
    assert_eq!(
        img.get_pixel(5, 5).0,
        [0, 0, 0, 255],
        "far outside the box is the opaque-black background"
    );
    // Deep interior → opaque white (full coverage of the white fill).
    assert_eq!(
        img.get_pixel(70, 35).0,
        [255, 255, 255, 255],
        "inside the inset box is the filled white"
    );
}

#[test]
fn oracle_edge_band_is_partial_coverage() {
    // The AA band must be a partial gray (between the opaque-black background
    // and the opaque-white fill) for at least one pixel — proves the smoothstep
    // coverage step is live, the property the GPU shader's fwidth→smoothstep
    // produces. (Output is opaque, so AA shows in the RGB channels, not alpha.)
    let draw = DrawData::new(
        Vec2::new(50.0, 25.0),
        Vec2::new(40.0, 20.0),
        Color::WHITE,
        8.0,
    );
    let img = rasterize_sdf_rect(&draw, 200, 100);
    let has_partial = img.pixels().any(|p| {
        let lum = p.0[0];
        lum > 0 && lum < 255 && p.0[3] == 255
    });
    assert!(
        has_partial,
        "a rounded-rect edge must produce AA partial-coverage (gray) pixels"
    );
}
