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

#[test]
fn to_instance_offsets_position_to_clip() {
    // Locks down the `* inv_w - 1.0` / `1.0 - * inv_h` offset arithmetic that
    // the all-zero-position tests above leave un-exercised. A 0×0 rect at the
    // window center (px) should land at clip origin (0, 0).
    let window = Vec2::new(800.0, 600.0);
    let draw = DrawData::new(window * 0.5, Vec2::ZERO, Color::WHITE, 0.0);
    let i = to_instance(&draw, window);
    assert!(i.rect_pos[0].abs() < 1e-6);
    assert!(i.rect_pos[1].abs() < 1e-6);
}

// Pure-CPU port of `shader.wgsl::sdf_rounded_rect`. Mirrors the GPU SDF 1:1
// (only `abs` / `length` / `min` / `max` — no platform-specific intrinsics).
fn sdf_rounded_rect(p: Vec2, half_size: Vec2, r: f32) -> f32 {
    let q = p.abs() - half_size + Vec2::splat(r);
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
}

// Pure-CPU port of `shader.wgsl::vertex` for `half_size`. The bug fix lives
// here: the fragment SDF expects a *positive* half-extent, so we abs() the
// signed `rect_size` before halving. Without the abs, the SDF receives a
// negative y half-extent and every interior fragment computes a positive
// distance ⇒ alpha collapses to 0 across the whole rect.
fn shader_half_size(rect_size: [f32; 2]) -> Vec2 {
    Vec2::new(rect_size[0].abs(), rect_size[1].abs()) * 0.5
}

#[test]
fn shader_sdf_inside_is_filled_outside_is_empty() {
    // Regression test for the half_size sign bug. Builds an `InstanceData`
    // for a centered rect and walks the SDF for two `local_uv` samples:
    // (0, 0) — rect center, must be inside (negative SDF, alpha → 1).
    // (2, 2) — well outside the rect, must be outside (positive SDF, alpha → 0).
    let window = Vec2::new(800.0, 600.0);
    let draw = DrawData::new(
        Vec2::new(100.0, 100.0),
        Vec2::new(200.0, 100.0),
        Color::WHITE,
        0.0,
    );
    let i = to_instance(&draw, window);

    let half = shader_half_size(i.rect_size);

    // Center: local_uv = (0, 0) → p = (0, 0).
    let d_center = sdf_rounded_rect(Vec2::ZERO * half, half, i.radius);
    assert!(
        d_center < 0.0,
        "rect center must be inside the SDF (got d = {d_center}); regression of the \
         half_size sign bug — fragment shader was passing a signed half_size into the \
         SDF, putting every interior point outside the rect."
    );

    // Well outside: local_uv = (2, 2) → p = (2 * half.x, 2 * half.y).
    let d_outside = sdf_rounded_rect(Vec2::splat(2.0) * half, half, i.radius);
    assert!(
        d_outside > 0.0,
        "point at 2x the rect's half-extent must be outside the SDF (got d = {d_outside})"
    );
}

#[test]
fn signed_rect_size_breaks_sdf_without_abs() {
    // Documents WHY `shader.wgsl` must `abs(i.rect_size)` before halving. If
    // we feed the SDF the signed `rect_size * 0.5` (the pre-fix shader
    // behavior), the rect center reports a *positive* SDF — the rect renders
    // invisible. This test pins that property so a regression that drops the
    // abs back out fails loudly.
    let window = Vec2::new(800.0, 600.0);
    let draw = DrawData::new(
        Vec2::new(100.0, 100.0),
        Vec2::new(200.0, 100.0),
        Color::WHITE,
        0.0,
    );
    let i = to_instance(&draw, window);

    // The buggy half_size: signed, with negative y from to_instance's y-flip.
    let bad_half = Vec2::new(i.rect_size[0], i.rect_size[1]) * 0.5;
    assert!(
        bad_half.y < 0.0,
        "test precondition: y-flip leaves rect_size.y negative"
    );

    let d_center_buggy = sdf_rounded_rect(Vec2::ZERO * bad_half, bad_half, i.radius);
    assert!(
        d_center_buggy > 0.0,
        "expected the *buggy* signed half_size path to put the rect center *outside* \
         the SDF (got d = {d_center_buggy}); if this assertion fails, either the SDF \
         or to_instance changed semantics — re-derive the bug first."
    );
}
