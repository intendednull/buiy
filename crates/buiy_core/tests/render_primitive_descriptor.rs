//! Device-free tests over the `RenderPipelineDescriptor` that
//! `SpecializedRenderPipeline::specialize` builds. `specialize` is a pure fn
//! (no PipelineCache, no RenderDevice), so the descriptor — its target
//! format, blend, shader handle, and entry points — is fully assertable
//! without a wgpu adapter (architecture.md § 1.4).

// `BuiyPrimitives` is a `Default`-derived unit struct (the render-world
// `SpecializedRenderPipelines<BuiyPrimitives>` constructs it via `FromWorld` →
// `Default`); these tests use the same `::default()` ctor. Same repo idiom as
// `tests/components.rs` for `Node::default()`.
#![allow(clippy::default_constructed_unit_structs)]

use bevy::render::render_resource::{
    BlendState, SpecializedRenderPipeline, TextureFormat, VertexFormat,
};
use buiy_core::render::buckets::BuiyPrimitiveKind;
use buiy_core::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};

fn descriptor_for(kind: BuiyPrimitiveKind, format: TextureFormat) {
    // exercised via the asserts in each test; helper kept for readability
    let _ = (kind, format);
}

#[test]
fn quad_descriptor_uses_key_format_not_hardcoded() {
    let specializer = BuiyPrimitives::default();
    let srgb = specializer.specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    let hdr = specializer.specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba16Float,
    });
    let srgb_fmt = srgb.fragment.as_ref().unwrap().targets[0]
        .as_ref()
        .unwrap()
        .format;
    let hdr_fmt = hdr.fragment.as_ref().unwrap().targets[0]
        .as_ref()
        .unwrap()
        .format;
    assert_eq!(srgb_fmt, TextureFormat::Rgba8UnormSrgb);
    assert_eq!(hdr_fmt, TextureFormat::Rgba16Float);
    descriptor_for(BuiyPrimitiveKind::Quad, srgb_fmt);
}

#[test]
fn quad_descriptor_keeps_alpha_blending_and_entry_points() {
    let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    let frag = d.fragment.as_ref().unwrap();
    assert_eq!(
        frag.targets[0].as_ref().unwrap().blend,
        Some(BlendState::ALPHA_BLENDING),
        "alpha blending preserved (the Phase-0 setting; blend-space seam is \
         a format consequence, not a blend change)"
    );
    assert_eq!(d.vertex.entry_point.as_deref(), Some("vertex"));
    assert_eq!(frag.entry_point.as_deref(), Some("fragment"));
}

#[test]
fn quad_descriptor_has_two_vertex_buffers_with_phase0_strides() {
    // Static unit-quad VBO (stride 16) + per-instance buffer (stride 52 after
    // R8b appends the clip AABB at @location(6)/(7)); the unit-quad VBO is
    // untouched.
    let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    let buffers = &d.vertex.buffers;
    assert_eq!(buffers.len(), 2, "vertex + instance buffer layouts");
    assert_eq!(buffers[0].array_stride, 16);
    assert_eq!(buffers[1].array_stride, 52);
}

#[test]
fn instance_buffer_stride_is_52_with_clip_fields() {
    // The per-instance record grew from 36 B (R7) to 52 B (R8b) when the clip
    // AABB (`clip_min`/`clip_max`, two Float32x2) was appended; the vertex
    // layout's `array_stride` must track `PackedInstance`'s 52-byte stride or
    // wgpu mis-strides the instance buffer.
    let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    assert_eq!(d.vertex.buffers[1].array_stride, 52);
}

#[test]
fn instance_has_clip_min_at_location_6_offset_36() {
    // `clip_min` is the first appended clip field: Float32x2 at byte offset 36
    // (immediately after `radius`/`blur` @ 32) bound to `@location(6)` — the
    // WGSL `Instance.clip_min`.
    let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    let attrs = &d.vertex.buffers[1].attributes;
    let clip_min = attrs
        .iter()
        .find(|a| a.shader_location == 6)
        .expect("instance layout has @location(6) clip_min");
    assert_eq!(clip_min.format, VertexFormat::Float32x2);
    assert_eq!(clip_min.offset, 36);
}

#[test]
fn instance_has_clip_max_at_location_7_offset_44() {
    // `clip_max` follows `clip_min`: Float32x2 at byte offset 44 bound to
    // `@location(7)` — the WGSL `Instance.clip_max`.
    let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    let attrs = &d.vertex.buffers[1].attributes;
    let clip_max = attrs
        .iter()
        .find(|a| a.shader_location == 7)
        .expect("instance layout has @location(7) clip_max");
    assert_eq!(clip_max.format, VertexFormat::Float32x2);
    assert_eq!(clip_max.offset, 44);
}

#[test]
fn quad_descriptor_declares_the_view_uniform_bind_group_layout() {
    // The quad shader binds `@group(0) @binding(0) var<uniform> view`, so the
    // pipeline layout MUST declare exactly one bind-group layout — an empty
    // `layout` makes wgpu reject the pipeline at creation. This guards the
    // Phase-0 equivalence the quad primitive preserves: the inline `register`
    // descriptor declared the view-uniform `@group(0)`, and `specialize` must
    // too (architecture.md § 1.4).
    let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    assert_eq!(
        d.layout.len(),
        1,
        "quad pipeline declares the view-uniform @group(0) bind-group layout"
    );
    assert_eq!(
        d.layout[0].entries.len(),
        1,
        "one binding: @binding(0) view"
    );
}

#[test]
fn quad_and_shadow_use_distinct_shaders() {
    let s = BuiyPrimitives::default();
    let quad = s.specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    let shadow = s.specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Shadow,
        format: TextureFormat::Rgba8UnormSrgb,
    });
    // The two F-tier pipelines this phase ships reference different shaders;
    // border (folded into quad) and outline (clip-suppressed quad) add no
    // third shader UUID — they are not BuiyPrimitiveKind variants.
    assert_ne!(
        quad.vertex.shader, shadow.vertex.shader,
        "quad and shadow must use distinct vertex shaders"
    );
    assert_ne!(
        quad.fragment.as_ref().unwrap().shader,
        shadow.fragment.as_ref().unwrap().shader,
        "quad and shadow must use distinct fragment shaders (..01 vs ..02)"
    );
}
