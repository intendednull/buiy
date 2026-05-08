//! Unit tests for the instance-data layout and clip-space conversion. These
//! are pure-CPU tests; no GPU adapter required.

use bevy::prelude::*;
use buiy_core::render::DrawData;
use buiy_core::render::instance::{INSTANCE_STRIDE_BYTES, InstanceData, to_instance};

#[test]
fn instance_data_layout_matches_pipeline_descriptor() {
    // pipeline.rs declares the per-instance buffer with array_stride = 36.
    assert_eq!(
        std::mem::size_of::<InstanceData>(),
        INSTANCE_STRIDE_BYTES,
        "InstanceData stride must match pipeline.rs (2*4 + 2*4 + 4*4 + 1*4 = 36)"
    );
    assert_eq!(INSTANCE_STRIDE_BYTES, 36);
}

#[test]
fn to_instance_centers_origin_at_window_center() {
    // A rect at (0,0) of size (window) should map to clip rect_pos = (-1, +1)
    // (top-left in clip after y-flip) and rect_size = (2, 2).
    let window = Vec2::new(800.0, 600.0);
    let draw = DrawData::new(Vec2::ZERO, window, Color::WHITE, 0.0);
    let i = to_instance(&draw, window);
    assert!((i.rect_pos[0] - -1.0).abs() < 1e-6);
    assert!((i.rect_pos[1] - 1.0).abs() < 1e-6);
    assert!((i.rect_size[0] - 2.0).abs() < 1e-6);
    assert!((i.rect_size[1] - -2.0).abs() < 1e-6);
}

#[test]
fn to_instance_packs_color_in_linear_rgba() {
    let window = Vec2::new(100.0, 100.0);
    let draw = DrawData::new(
        Vec2::ZERO,
        Vec2::splat(10.0),
        Color::srgb(1.0, 0.0, 0.0),
        0.0,
    );
    let i = to_instance(&draw, window);
    let lin = LinearRgba::from(Color::srgb(1.0, 0.0, 0.0));
    assert!((i.color[0] - lin.red).abs() < 1e-5);
    assert!((i.color[1] - lin.green).abs() < 1e-5);
    assert!((i.color[2] - lin.blue).abs() < 1e-5);
    assert!((i.color[3] - lin.alpha).abs() < 1e-5);
}

#[test]
fn to_instance_radius_uses_min_window_dim() {
    // Radius is in clip-space units; pixel-to-clip uses 2.0 / min(window dims)
    // so the corner radius stays visually reasonable on non-square windows.
    let window = Vec2::new(1000.0, 500.0);
    let draw = DrawData::new(Vec2::ZERO, Vec2::splat(100.0), Color::WHITE, 25.0);
    let i = to_instance(&draw, window);
    let expected = 25.0 * (2.0 / 500.0);
    assert!((i.radius - expected).abs() < 1e-6);
}
