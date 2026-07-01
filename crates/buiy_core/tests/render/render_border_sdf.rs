//! Pure-CPU reference for the border band SDF that folds into the quad
//! primitive (architecture.md § 2.1). Mirrors the GPU band fragment 1:1
//! (only abs/length/min/max). No wgpu adapter — this is the oracle the quad
//! shader's outer-minus-inner band is validated against when the component
//! phase grows it. Same idiom as render_instance.rs's sdf port.

use bevy::math::Vec2;
// The canonical CPU twin of `shader.wgsl::sdf_rounded_rect` (negative inside),
// shared across the SDF oracle + render tests so the ports cannot drift.
use buiy_core::render::sdf_rounded_rect;

/// Border band coverage: a fragment is "in the band" iff it is inside the
/// outer rounded rect AND outside the inner (content) rounded rect.
/// Returns (inside_outer, inside_inner); the band is `inside_outer && !inside_inner`.
fn band_membership(
    p: Vec2,
    outer_half: Vec2,
    outer_r: f32,
    width: Vec2, // per-axis border width (left/right collapsed to x, top/bottom to y)
    inner_r: f32,
) -> (bool, bool) {
    let inner_half = outer_half - width;
    let d_outer = sdf_rounded_rect(p, outer_half, outer_r);
    let d_inner = sdf_rounded_rect(p, inner_half, inner_r);
    (d_outer < 0.0, d_inner < 0.0)
}

#[test]
fn point_in_border_band_is_inside_outer_outside_inner() {
    // 100x60 box, 10px uniform border, square corners.
    let outer_half = Vec2::new(50.0, 30.0);
    let width = Vec2::splat(10.0);
    // A point 5px in from the right edge sits inside the 10px band.
    let p = Vec2::new(45.0, 0.0);
    let (in_outer, in_inner) = band_membership(p, outer_half, 0.0, width, 0.0);
    assert!(in_outer, "point is inside the outer box");
    assert!(
        !in_inner,
        "point is in the border band, not the content hole"
    );
}

#[test]
fn point_in_content_hole_is_not_in_band() {
    let outer_half = Vec2::new(50.0, 30.0);
    let width = Vec2::splat(10.0);
    let p = Vec2::ZERO; // dead center → content hole
    let (in_outer, in_inner) = band_membership(p, outer_half, 0.0, width, 0.0);
    assert!(in_outer && in_inner, "center is inside both → not band");
}

#[test]
fn point_outside_outer_is_not_in_band() {
    let outer_half = Vec2::new(50.0, 30.0);
    let width = Vec2::splat(10.0);
    let p = Vec2::new(60.0, 0.0); // 10px past the right edge
    let (in_outer, _) = band_membership(p, outer_half, 0.0, width, 0.0);
    assert!(!in_outer, "point past the outer edge is not in the band");
}

#[test]
fn elliptical_radius_shrinks_corner_band_correctly() {
    // The inner corner radius must shrink with the border width
    // (inner_r = outer_r - min(width)); leaving it equal to outer_r bulges the
    // inner arc outward and mis-classifies hole pixels as band. This sample is
    // chosen so its inner membership is *load-bearing* on that shrink: it sits
    // in the inner-corner quadrant on the boundary between the two arcs.
    let outer_half = Vec2::new(50.0, 30.0);
    let width = Vec2::splat(10.0); // inner half = (40, 20)
    let outer_r = 12.0_f32;
    let inner_r = (outer_r - 10.0).max(0.0); // = 2, the correct shrunk radius

    // (38, 18) is the center of the inner-corner arc when inner_r = 2: it lies
    // comfortably inside the inner rounded rect (content hole), so the band
    // excludes it.
    let corner = Vec2::new(38.0, 18.0);

    let (in_outer, in_inner) = band_membership(corner, outer_half, outer_r, width, inner_r);
    assert!(in_outer, "sample is inside the outer rounded rect");
    assert!(
        in_inner,
        "with the shrunk inner radius the sample falls in the content hole, \
         so the band excludes it"
    );

    // Load-bearing check: leaving inner_r = outer_r (the *unshrunk* radius the
    // production code must avoid) rounds the inner corner away from this point,
    // so the same sample flips to outside the inner arc — i.e. it would be
    // wrongly painted as border band. The shrink computation is what keeps it
    // in the hole.
    let (_, in_inner_unshrunk) = band_membership(corner, outer_half, outer_r, width, outer_r);
    assert!(
        !in_inner_unshrunk,
        "unshrunk inner radius bulges the arc and mis-classifies the hole \
         pixel as band — proves the shrink is load-bearing"
    );
}
