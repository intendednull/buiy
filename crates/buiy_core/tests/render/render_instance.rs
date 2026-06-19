//! Unit tests for the logical-pixel instance-data layout (the view-uniform
//! path). These are pure-CPU tests; no GPU adapter required. The clip-space
//! transform now lives in the per-view `BuiyViewUniform` (vertex stage), so the
//! per-instance records here stay in LOGICAL pixels — `pack_instance` packs
//! them and the SDF evaluates in logical px (positive half-extent, no abs hack).

use bevy::prelude::*;
use buiy_core::render::DrawData;
use buiy_verify::snapshot::assert_instance_hex_snapshot;

// Pure-CPU port of `shader.wgsl::sdf_rounded_rect` (logical px). The view-uniform
// path keeps the SDF in logical px with a POSITIVE half_size — no abs() hack.
fn sdf_rounded_rect(p: Vec2, half_size: Vec2, r: f32) -> f32 {
    let q = p.abs() - half_size + Vec2::splat(r);
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
}

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
    // clip_max(2*4) = 52, in LOGICAL px (not clip). The clip AABB rides every
    // instance (R8b fragment discard); the const must equal the struct stride.
    assert_eq!(
        std::mem::size_of::<PackedInstance>(),
        PACKED_INSTANCE_STRIDE_BYTES
    );
    assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 52);
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
        color: Color::WHITE,
        clip,
        group: None,
    }
}

#[test]
fn packed_instance_stride_is_52() {
    // R8b: pos(2)+size(2)+color(4)+radius(1)+clip_min(2)+clip_max(2) = 13 f32 = 52 B.
    // The struct stride, the const, and the raw [f32;13] must all agree (52 B);
    // any drift makes the instanced draw read garbage.
    assert_eq!(std::mem::size_of::<PackedInstance>(), 52);
    assert_eq!(
        std::mem::size_of::<PackedInstance>(),
        std::mem::size_of::<[f32; 13]>()
    );
    assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 52);
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
fn packed_raw_stride_agrees_with_thirteen_floats() {
    // The raw bucket layout is [f32;13] and byte-equal to PackedInstance's stride.
    assert!(buiy_core::render::instance::packed_raw_stride_agrees());
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
