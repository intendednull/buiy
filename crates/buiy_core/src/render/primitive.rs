//! Typed-primitive pipeline specialization: the device-free key that selects
//! one `SpecializedRenderPipeline` variant per `(primitive, target format)`.
//!
//! A wgpu `RenderPipeline`'s fragment `ColorTargetState.format` is fixed at
//! creation, so each typed primitive (quad / shadow; border folds into quad,
//! outline is a clip-suppressed quad) is a `SpecializedRenderPipeline` keyed on
//! the target format; Buiy builds each for both the view format
//! (`Rgba8UnormSrgb` by default) and the `Rgba16Float` effect-group target
//! format. See
//! `docs/specs/2026-06-03-buiy-render-pipeline-design/architecture.md` § 1.4.
//!
//! `BuiyPrimitiveKind` is **owned by `crate::render::buckets`** (R6); this
//! module imports it and adds only the `(kind, format)` specialization key.

use bevy::mesh::VertexBufferLayout;
use bevy::render::render_resource::{
    BlendState, ColorTargetState, ColorWrites, FragmentState, FrontFace, MultisampleState,
    PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipelineDescriptor,
    SpecializedRenderPipeline, TextureFormat, VertexAttribute, VertexFormat, VertexState,
    VertexStepMode,
};

// Owned by R6 (render::buckets) — imported, not redefined.
use crate::render::buckets::BuiyPrimitiveKind;
use crate::render::pipeline::{
    shader_handle, shadow_shader_handle, view_uniform_layout_descriptor,
};

/// One `SpecializedRenderPipeline` variant: a primitive built for a specific
/// target color-attachment format. `Key` for the typed-primitive
/// `SpecializedRenderPipeline` (architecture.md § 1.4).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiyPrimitiveKey {
    /// The typed primitive this variant draws. Owned by `render::buckets` (R6).
    pub kind: BuiyPrimitiveKind,
    /// The bound attachment's format: the view format for the main pass
    /// (`Rgba8UnormSrgb` default / `Rgba16Float` HDR) or the fixed
    /// `Rgba16Float` for effect-group targets.
    pub format: TextureFormat,
}

/// The typed-primitive `SpecializedRenderPipeline`. One specializer builds
/// every `(kind, format)` variant; `SpecializedRenderPipelines<BuiyPrimitives>`
/// (render world) dedupes identical keys into one `CachedRenderPipelineId`.
#[derive(Default)]
pub struct BuiyPrimitives;

impl BuiyPrimitives {
    /// The two interleaved vertex-buffer layouts shared by every quad-family
    /// primitive (static unit quad, stride 16; per-instance record, stride 36).
    fn quad_family_vertex_buffers() -> Vec<VertexBufferLayout> {
        vec![
            VertexBufferLayout {
                array_stride: 16,
                step_mode: VertexStepMode::Vertex,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                ],
            },
            VertexBufferLayout {
                array_stride: 36,
                step_mode: VertexStepMode::Instance,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 2,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 3,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 4,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32,
                        offset: 32,
                        shader_location: 5,
                    },
                ],
            },
        ]
    }

    /// The shader handle for a primitive kind. Border folds into the quad
    /// SDF and outline is a clip-suppressed quad, so both paint through the
    /// quad shader — neither is a distinct `BuiyPrimitiveKind` variant.
    /// `Glyph` / `Path` shaders are sibling-phase work (octets `..03` /
    /// `..04`), not built here; this phase ships only `Quad` and `Shadow`.
    fn shader_for(kind: BuiyPrimitiveKind) -> bevy::asset::Handle<bevy::shader::Shader> {
        match kind {
            BuiyPrimitiveKind::Quad => shader_handle(),
            BuiyPrimitiveKind::Shadow => shadow_shader_handle(),
            BuiyPrimitiveKind::Glyph | BuiyPrimitiveKind::Path => shader_handle(),
        }
    }
}

impl SpecializedRenderPipeline for BuiyPrimitives {
    type Key = BuiyPrimitiveKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let shader = Self::shader_for(key.kind);
        RenderPipelineDescriptor {
            label: Some(format!("buiy_{:?}_pipeline", key.kind).into()),
            // Every quad-family shader binds the view uniform at
            // `@group(0) @binding(0)` (see `shader.wgsl`); the pipeline layout
            // must declare a matching group or wgpu validation rejects the
            // pipeline. This is the same `@group(0)` descriptor the Phase-0
            // `register` path supplies, shared so the two cannot drift.
            layout: vec![view_uniform_layout_descriptor()],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: Self::quad_family_vertex_buffers(),
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    // The format/edge seam: keyed off the bound attachment,
                    // not hard-coded (architecture.md § 1.4).
                    format: key.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}
