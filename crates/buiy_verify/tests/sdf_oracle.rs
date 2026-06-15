//! Pure-CPU (NOT #[ignore]): the full-tile CPU SDF oracle must reproduce the
//! scalar `d` the existing render_instance.rs point-probes assert — center
//! inside (filled), 2× half-extent outside (empty). Pins the full-tile port to
//! the unit-tested shader formula. reftests.md § Verification #5.

use bevy::prelude::*;
use buiy_core::render::DrawData;
use buiy_verify::reftest::sdf_oracle::rasterize_sdf_rect;

#[test]
fn oracle_fills_center_and_clears_far_outside() {
    let inset = DrawData::new(Vec2::new(50.0, 25.0), Vec2::new(40.0, 20.0), Color::WHITE, 0.0);
    let img = rasterize_sdf_rect(&inset, 200, 100);
    assert_eq!(img.dimensions(), (200, 100));
    assert_eq!(img.get_pixel(5, 5).0[3], 0, "far outside the box is empty");
    assert_eq!(
        img.get_pixel(70, 35).0[3],
        255,
        "inside the inset box is filled"
    );
}

#[test]
fn oracle_edge_band_is_partial_alpha() {
    // The AA band must be neither fully 0 nor fully 255 for at least one pixel
    // (proves the smoothstep coverage step is live) — the property the GPU
    // shader's fwidth→smoothstep produces.
    let draw = DrawData::new(Vec2::new(50.0, 25.0), Vec2::new(40.0, 20.0), Color::WHITE, 8.0);
    let img = rasterize_sdf_rect(&draw, 200, 100);
    let has_partial = img.pixels().any(|p| {
        let a = p.0[3];
        a > 0 && a < 255
    });
    assert!(
        has_partial,
        "a rounded-rect edge must produce AA partial-alpha pixels"
    );
}
