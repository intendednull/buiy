//! Pure-CPU tests for the Buiy view uniform: the logical-pixel -> clip-space
//! affine that replaces the Phase-0 per-instance y-flip / radius hack. No GPU
//! adapter required (this is the HEADLESS half of the prepare phase).

use bevy::prelude::*;
use buiy_core::render::view_uniform::{BuiyViewUniform, VIEW_UNIFORM_SIZE_BYTES};

#[test]
fn view_uniform_size_is_std140_friendly() {
    // The uniform is uploaded to a UBO; its CPU size must be a multiple of 16
    // (std140 alignment) so the GPU layout is unambiguous. logical_to_clip is a
    // mat4 surrogate packed as 2 columns of vec4 (32 B) + scale_factor + 3 pad
    // (16 B) = 48 B.
    assert_eq!(
        std::mem::size_of::<BuiyViewUniform>(),
        VIEW_UNIFORM_SIZE_BYTES
    );
    assert_eq!(VIEW_UNIFORM_SIZE_BYTES % 16, 0);
}

#[test]
fn origin_maps_to_clip_top_left() {
    // Logical (0,0) is the window top-left; in clip space (y-up) that is
    // (-1, +1). The y-flip lives ENTIRELY in the uniform now.
    let u = BuiyViewUniform::for_view(Vec2::new(800.0, 600.0), 1.0);
    let p = u.apply(Vec2::ZERO);
    assert!((p.x - -1.0).abs() < 1e-6, "x={}", p.x);
    assert!((p.y - 1.0).abs() < 1e-6, "y={}", p.y);
}

#[test]
fn bottom_right_maps_to_clip_bottom_right() {
    // Logical (w,h) -> clip (+1, -1).
    let w = Vec2::new(800.0, 600.0);
    let u = BuiyViewUniform::for_view(w, 1.0);
    let p = u.apply(w);
    assert!((p.x - 1.0).abs() < 1e-6, "x={}", p.x);
    assert!((p.y - -1.0).abs() < 1e-6, "y={}", p.y);
}

#[test]
fn center_maps_to_clip_origin() {
    let w = Vec2::new(800.0, 600.0);
    let u = BuiyViewUniform::for_view(w, 1.0);
    let p = u.apply(w * 0.5);
    assert!(p.x.abs() < 1e-6 && p.y.abs() < 1e-6, "p={p:?}");
}

#[test]
fn scale_factor_is_carried_verbatim() {
    // The uniform carries scale_factor so the SDF/radius can stay in logical
    // px on the GPU. The logical->clip affine itself is in LOGICAL px (the
    // window size passed in is logical), so scale_factor does NOT scale the
    // affine; it is a separate field the shader uses for px-space AA.
    let u = BuiyViewUniform::for_view(Vec2::new(800.0, 600.0), 2.0);
    assert!((u.scale_factor() - 2.0).abs() < 1e-6);
    // Same logical window, different scale_factor => SAME logical->clip mapping.
    let u1 = BuiyViewUniform::for_view(Vec2::new(800.0, 600.0), 1.0);
    assert!((u.apply(Vec2::new(400.0, 300.0)) - u1.apply(Vec2::new(400.0, 300.0))).length() < 1e-6);
}

#[test]
fn as_std140_array_packs_col0_col1_scale_factor() {
    // The std140 UBO payload the prepare phase uploads (the sole production
    // path: prepare.rs `view_uniform.set(uniform.as_std140_array())`) must pack
    // col0 ++ col1 ++ [scale_factor, 0, 0, 0]. A swapped col0/col1 or a
    // misplaced scale_factor compiles + passes the gate but renders wrong
    // geometry on a GPU; this pure-CPU test pins the byte order. Non-square size
    // so col0 != col1 and a swap is observable.
    let logical = Vec2::new(1000.0, 500.0);
    let scale_factor = 2.0_f32;
    let u = BuiyViewUniform::for_view(logical, scale_factor);
    let a = u.as_std140_array();

    // The affine is diagonal-scale + translate: col0 = [2/w, 0, 0, -1],
    // col1 = [0, -2/h, 0, 1]. Assert slot-by-slot against for_view's contract.
    let sx = 2.0 / logical.x;
    let sy = -2.0 / logical.y;
    let expected_col0 = [sx, 0.0, 0.0, -1.0];
    let expected_col1 = [0.0, sy, 0.0, 1.0];
    for i in 0..4 {
        assert!(
            (a[i] - expected_col0[i]).abs() < 1e-6,
            "col0[{i}] = {} (want {})",
            a[i],
            expected_col0[i]
        );
        assert!(
            (a[4 + i] - expected_col1[i]).abs() < 1e-6,
            "col1[{i}] = {} (want {})",
            a[4 + i],
            expected_col1[i]
        );
    }
    // scale_factor at slot 8, then three zero-pad slots.
    assert!(
        (a[8] - scale_factor).abs() < 1e-6,
        "scale_factor at slot 8, got {}",
        a[8]
    );
    assert_eq!([a[9], a[10], a[11]], [0.0, 0.0, 0.0], "slots 9..12 are pad");

    // col0 and col1 are distinct on a non-square view: a swap would be caught.
    assert!(
        (a[0] - a[5]).abs() > 1e-6,
        "col0[0] != col1[1] (non-square)"
    );
}

#[test]
fn no_per_axis_radius_distortion() {
    // The Phase-0 hack approximated px->clip radius with 2/min(w,h), which
    // distorts on non-square windows. The view uniform removes that: a logical
    // delta maps to clip with INDEPENDENT per-axis scale, so a square in px is
    // a square in px on the GPU (radius stays in logical px). Assert the per-
    // axis clip scale differs on a non-square window (the thing the old hack
    // collapsed to a single min()).
    let u = BuiyViewUniform::for_view(Vec2::new(1000.0, 500.0), 1.0);
    let dx = (u.apply(Vec2::new(1.0, 0.0)) - u.apply(Vec2::ZERO)).x;
    let dy = (u.apply(Vec2::new(0.0, 1.0)) - u.apply(Vec2::ZERO)).y;
    // 1 logical px maps to 2/1000 in x, -2/500 in y: magnitudes differ. The
    // per-axis scale is recovered as a difference of two near-(-1.0)/near-(+1.0)
    // clip coords, so f32 subtractive cancellation bounds the achievable
    // precision at ~1e-8 (the ULP near 1.0); 1e-6 matches the rest of this file.
    assert!((dx - (2.0 / 1000.0)).abs() < 1e-6);
    assert!((dy - (-2.0 / 500.0)).abs() < 1e-6);
    assert!(
        (dx.abs() - dy.abs()).abs() > 1e-6,
        "per-axis scale must differ"
    );
}

// CPU mirror of the NEW vertex transform in shader.wgsl: a logical-px point
// transformed by the view uniform must equal BuiyViewUniform::apply. This is
// the device-free proof that the WGSL vertex math and the CPU uniform agree
// (the shader itself only runs on GPU; this pins the math the GPU executes).
fn wgsl_vertex_logical_to_clip(u: &BuiyViewUniform, logical: Vec2) -> Vec2 {
    // Mirror of: clip.x = col0.x*l.x + col0.w; clip.y = col1.y*l.y + col1.w
    let a = u.as_std140_array();
    Vec2::new(a[0] * logical.x + a[3], a[5] * logical.y + a[7])
}

#[test]
fn wgsl_vertex_mirror_matches_apply() {
    let u = BuiyViewUniform::for_view(Vec2::new(1000.0, 500.0), 1.0);
    for p in [
        Vec2::ZERO,
        Vec2::new(500.0, 250.0),
        Vec2::new(1000.0, 500.0),
        Vec2::new(123.0, 456.0),
    ] {
        let m = wgsl_vertex_logical_to_clip(&u, p);
        let a = u.apply(p);
        assert!((m - a).length() < 1e-6, "p={p:?} mirror={m:?} apply={a:?}");
    }
}
