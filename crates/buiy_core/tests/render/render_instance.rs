//! Unit tests for the logical-pixel instance-data layout (the view-uniform
//! path). These are pure-CPU tests; no GPU adapter required. The clip-space
//! transform now lives in the per-view `BuiyViewUniform` (vertex stage), so the
//! per-instance records here stay in LOGICAL pixels — `pack_instance` packs
//! them and the SDF evaluates in logical px (positive half-extent, no abs hack).

use bevy::prelude::*;
use buiy_core::render::DrawData;
// The canonical CPU twin of `shader.wgsl::sdf_rounded_rect` (logical px, positive
// half_size — no abs() hack), shared across the SDF oracle + render tests.
use buiy_core::render::sdf_rounded_rect;
use buiy_verify::snapshot::assert_instance_hex_snapshot;

#[test]
fn logical_sdf_inside_is_filled_outside_is_empty() {
    let draw = DrawData::new(
        Vec2::new(100.0, 100.0),
        Vec2::new(200.0, 100.0),
        Color::WHITE,
        0.0,
    );
    let p = pack_instance(&draw);
    let half = Vec2::from(p.rect_size) * 0.5; // positive — logical px
    assert!(half.y > 0.0, "logical-px half_size is positive (no y-flip)");

    let d_center = sdf_rounded_rect(Vec2::ZERO, half, p.radius);
    assert!(d_center < 0.0, "rect center inside (d={d_center})");

    let d_out = sdf_rounded_rect(Vec2::splat(2.0) * half, half, p.radius);
    assert!(d_out > 0.0, "2x half-extent outside (d={d_out})");
}

// ----- view-uniform path (prepare phase) -----
// `pack_instance` keeps everything in LOGICAL px; the GPU view uniform does the
// clip transform.
use buiy_core::render::instance::{PACKED_INSTANCE_STRIDE_BYTES, PackedInstance, pack_instance};

#[test]
fn packed_instance_stride_matches_logical_pipeline_descriptor() {
    // pos(2*4) + size(2*4) + color(4*4) + radius(1*4) + clip_min(2*4) +
    // clip_max(2*4) + affine(4*4) = 68, in LOGICAL px (not clip). The clip AABB
    // and the 2D affine basis ride every instance (R8b fragment discard + R1
    // transform paint); the const must equal the struct stride.
    assert_eq!(
        std::mem::size_of::<PackedInstance>(),
        PACKED_INSTANCE_STRIDE_BYTES
    );
    assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 68);
}

#[test]
fn pack_instance_keeps_position_and_size_in_logical_px() {
    // No clip conversion, no y-flip baked into the size. The raw logical box
    // is forwarded; the GPU view uniform (Task 1) does the clip transform.
    // The per-field pos/size/radius asserts collapse into one byte-exact hex
    // snapshot — it pins every f32 of the packed payload (positive height = NO
    // y-flip, radius in logical px = NO 2/min(w,h)), so the half-size sign bug
    // or a radius approximation flips the hex (snapshots.md § byte-exact).
    let draw = DrawData::new(
        Vec2::new(100.0, 50.0),
        Vec2::new(200.0, 80.0),
        Color::WHITE,
        12.0,
    );
    let p = pack_instance(&draw);
    assert_instance_hex_snapshot(&p, "pack_instance_logical_px");
}

#[test]
fn pack_instance_pre_linearizes_color_on_cpu() {
    // color-and-forced-colors.md § 1.1: color stays CPU-pre-linearized; only
    // the COORDINATE packing moves to the view uniform.
    let draw = DrawData::new(
        Vec2::ZERO,
        Vec2::splat(10.0),
        Color::srgb(1.0, 0.0, 0.0),
        0.0,
    );
    let p = pack_instance(&draw);
    let lin = LinearRgba::from(Color::srgb(1.0, 0.0, 0.0));
    assert!((p.color[0] - lin.red).abs() < 1e-5);
    assert!((p.color[1] - lin.green).abs() < 1e-5);
    assert!((p.color[2] - lin.blue).abs() < 1e-5);
    assert!((p.color[3] - lin.alpha).abs() < 1e-5);
}

// ----- R8b: per-instance clip AABB (stride 36 -> 52) -----
// The clip AABB rides every instance (one draw, order-safe). `None` packs to the
// full-view sentinel (±INFINITY) so the fragment discard never fires.
use buiy_core::render::components::ClipRect;
use buiy_core::render::extract::ExtractedNode;
use buiy_core::render::instance::pack_extracted;

fn node_with_clip(clip: Option<ClipRect>) -> ExtractedNode {
    ExtractedNode {
        entity: Entity::from_raw_u32(1).unwrap(),
        position: Vec2::new(10.0, 20.0),
        size: Vec2::new(30.0, 40.0),
        radius: 0.0,
        color: Color::WHITE,
        clip,
        group: None,
        top_layer: false,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    }
}

#[test]
fn packed_instance_stride_is_68() {
    // R8b + R1: pos(2)+size(2)+color(4)+radius(1)+clip_min(2)+clip_max(2)
    // +affine(4) = 17 f32 = 68 B. The struct stride, the const, and the raw
    // [f32;17] must all agree (68 B); any drift makes the instanced draw read
    // garbage.
    assert_eq!(std::mem::size_of::<PackedInstance>(), 68);
    assert_eq!(
        std::mem::size_of::<PackedInstance>(),
        std::mem::size_of::<[f32; 17]>()
    );
    assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 68);
}

#[test]
fn packed_instance_appends_affine_after_existing_thirteen() {
    // R1 HARD CONSTRAINT (campaign-review MAJOR — R2 depends on it): the 2x2
    // affine basis appends AFTER the existing 13 floats so every existing field
    // offset is UNCHANGED (notably color@4 / alpha@7). The raw record carries
    // the flattened basis [m00,m10,m01,m11] at [13..17], and raw[0..13] is
    // byte-identical to the pre-R1 layout.
    use buiy_core::render::buckets::packed_to_raw;
    let mut node = node_with_clip(Some(ClipRect {
        min: Vec2::new(5.0, 6.0),
        max: Vec2::new(105.0, 206.0),
    }));
    node.affine = [[2.0, 3.0], [4.0, 5.0]]; // col0 = [m00,m10], col1 = [m01,m11]
    let p = pack_extracted(&node);
    let raw = packed_to_raw(&p);
    assert_eq!(
        &raw[13..17],
        &[2.0, 3.0, 4.0, 5.0],
        "affine appended at [13..17]"
    );
    // The pre-R1 layout is byte-identical: pos/size/color/radius/clip unchanged.
    assert_eq!(raw[0], 10.0);
    assert_eq!(raw[1], 20.0);
    assert_eq!(raw[2], 30.0);
    assert_eq!(raw[3], 40.0);
    let lin = LinearRgba::from(Color::WHITE);
    assert_eq!(&raw[4..8], &[lin.red, lin.green, lin.blue, lin.alpha]);
    assert_eq!(raw[8], 0.0); // radius
    assert_eq!(&raw[9..13], &[5.0, 6.0, 105.0, 206.0]); // clip min/max
}

#[test]
fn color_and_alpha_offset_consts_point_at_color() {
    // R2 (degraded-group re-tint) reads alpha via ALPHA_FLOAT_OFFSET, so the
    // named consts must point at the color block (color@4, alpha@7) — the
    // invariant the append-after-13 layout exists to preserve.
    use buiy_core::render::buckets::packed_to_raw;
    use buiy_core::render::instance::{ALPHA_FLOAT_OFFSET, COLOR_FLOAT_OFFSET};
    assert_eq!(COLOR_FLOAT_OFFSET, 4);
    assert_eq!(ALPHA_FLOAT_OFFSET, 7);
    let p = pack_extracted(&node_with_clip(None));
    let raw = packed_to_raw(&p);
    assert_eq!(raw[ALPHA_FLOAT_OFFSET], p.color[3]);
    assert_eq!(&raw[COLOR_FLOAT_OFFSET..COLOR_FLOAT_OFFSET + 4], &p.color);
}

#[test]
fn pack_extracted_sets_clip_min_max_from_node_clip() {
    // A node carrying a finite ClipRect packs that box verbatim into
    // clip_min/clip_max (the same logical-px space as ClipRect.min/.max). The
    // per-field clip_min/clip_max asserts become one byte-exact hex snapshot
    // (it pins the whole packed payload, clip bytes included).
    let clip = ClipRect {
        min: Vec2::new(5.0, 6.0),
        max: Vec2::new(105.0, 206.0),
    };
    let p = pack_extracted(&node_with_clip(Some(clip)));
    assert_instance_hex_snapshot(&p, "pack_extracted_finite_clip");
}

#[test]
fn pack_extracted_uses_full_view_sentinel_when_clip_absent() {
    // clip == None packs to clip_min = [-INF; 2], clip_max = [+INF; 2] — for any
    // finite frag_pos the discard never fires, so the node paints unclipped.
    // The hex snapshot pins the ±INFINITY sentinel bytes exactly (so a regression
    // to a finite default flips the hex).
    let p = pack_extracted(&node_with_clip(None));
    assert_instance_hex_snapshot(&p, "pack_extracted_sentinel_clip");
}

#[test]
fn packed_raw_stride_agrees_with_seventeen_floats() {
    // The raw bucket layout is [f32;17] and byte-equal to PackedInstance's stride.
    assert!(buiy_core::render::instance::packed_raw_stride_agrees());
}

#[test]
fn border_band_stride_agrees_and_quad_stride_is_unchanged() {
    // C6-a (styling-f-tier.md § 4): the DISTINCT band/outline record agrees with
    // its declared stride (52 f32 = 208 B since F4b appended the per-side dash
    // `style` lane), AND the frozen 68 B quad stride (`PackedInstance`) stays
    // byte-stable — the whole point of the two-record design (umbrella § 6.7): the
    // band channel grows on its OWN record, it never bumps the quad stride that
    // R2's degraded-group re-tint indexes.
    use buiy_core::render::instance::{
        BORDER_BAND_INSTANCE_STRIDE_BYTES, BorderBandInstance, border_band_stride_agrees,
    };
    assert!(border_band_stride_agrees());
    assert_eq!(BORDER_BAND_INSTANCE_STRIDE_BYTES, 208);
    assert_eq!(std::mem::size_of::<BorderBandInstance>(), 208);
    // The quad stride is untouched — the byte-stability guard, restated next to
    // the new record so a future stride drift on EITHER side reddens here.
    assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 68);
}

#[test]
fn rounded_shadow_stride_agrees_and_quad_stride_is_unchanged() {
    // F4b-6: the DISTINCT rounded-shadow record (72 B = 18 f32) agrees with its
    // declared stride, and the frozen 68 B quad stride stays byte-stable — the
    // Option B guarantee (a dedicated record, never a widen of the shared quad).
    use buiy_core::render::instance::{
        ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES, RoundedShadowInstance, rounded_shadow_stride_agrees,
    };
    assert!(rounded_shadow_stride_agrees());
    assert_eq!(ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES, 72);
    assert_eq!(std::mem::size_of::<RoundedShadowInstance>(), 72);
    assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 68);
}

#[test]
fn gradient_stride_agrees_and_quad_stride_is_unchanged() {
    // Parity Wave B1: the DISTINCT 2-stop gradient record agrees with its
    // declared stride (26 f32 = 104 B), AND the frozen 68 B quad stride
    // (`PackedInstance`) stays byte-stable — the same two-record design as the
    // band: the gradient adds a parallel record, never bumping the quad stride
    // that R2's degraded-group re-tint indexes. A 1000-quad scene carries ZERO
    // gradient bytes.
    use buiy_core::render::instance::{
        GRADIENT_INSTANCE_STRIDE_BYTES, GradientInstance, gradient_stride_agrees,
    };
    assert!(gradient_stride_agrees());
    assert_eq!(GRADIENT_INSTANCE_STRIDE_BYTES, 104);
    assert_eq!(std::mem::size_of::<GradientInstance>(), 104);
    // The quad stride is untouched — the byte-stability guard.
    assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 68);
}

#[test]
fn pack_instance_then_view_uniform_maps_top_left_to_clip() {
    // Packing in logical px and then applying the view uniform on the CPU yields
    // the clip-space top-left (the coordinate seam the GPU vertex stage runs).
    // This is the live view-uniform handoff — the Phase-0 `to_instance` it
    // replaced is gone.
    use buiy_core::render::view_uniform::BuiyViewUniform;
    let window = Vec2::new(800.0, 600.0);
    let draw = DrawData::new(
        Vec2::new(100.0, 100.0),
        Vec2::new(200.0, 100.0),
        Color::WHITE,
        0.0,
    );

    let packed = pack_instance(&draw);
    let u = BuiyViewUniform::for_view(window, 1.0);
    let top_left_clip = u.apply(Vec2::from(packed.rect_pos));

    // Logical (100, 100) on an 800x600 view -> clip (100/400 - 1, 1 - 100/300).
    assert!((top_left_clip.x - (100.0 / 400.0 - 1.0)).abs() < 1e-6);
    assert!((top_left_clip.y - (1.0 - 100.0 / 300.0)).abs() < 1e-6);
}
