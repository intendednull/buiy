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
//!
//! The per-instance vertex layout carries the R8b clip AABB
//! (`clip_min`/`clip_max`) at `@location(6)`/`(7)`, lifting the instance stride
//! to 52 B; the quad-family shaders discard fragments outside it.

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
    atlas_layout_descriptor, coverage_shader_handle, shader_handle, shadow_shader_handle,
    view_uniform_layout_descriptor,
};

/// One `SpecializedRenderPipeline` variant: a primitive built for a specific
/// target color-attachment format and sample count. `Key` for the
/// typed-primitive `SpecializedRenderPipeline` (architecture.md § 1.4).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiyPrimitiveKey {
    /// The typed primitive this variant draws. Owned by `render::buckets` (R6).
    pub kind: BuiyPrimitiveKind,
    /// The bound attachment's format: the view format for the main pass
    /// (`Rgba8UnormSrgb` default / `Rgba16Float` HDR) or the fixed
    /// `Rgba16Float` for effect-group targets.
    pub format: TextureFormat,
    /// The bound attachment's sample count — a wgpu invariant exactly like
    /// `format`: a pipeline's `MultisampleState.count` must equal the pass
    /// attachment's `sample_count` or `set_pipeline` fails validation
    /// ("Render pipeline targets are incompatible with render pass"). The VIEW
    /// pass keys off the per-view [`Msaa`](bevy::render::view::Msaa) (4 for a
    /// bare `Camera2d` — `Msaa::Sample4` is the default), via
    /// `pipeline::prepare_buiy_view_pipelines`; the off-screen effect-group
    /// targets are created single-sampled (`group_target_descriptor`,
    /// `sample_count: 1`), so the group passes always key `samples: 1`.
    pub samples: u32,
}

/// The typed-primitive `SpecializedRenderPipeline`. One specializer builds
/// every `(kind, format)` variant; `SpecializedRenderPipelines<BuiyPrimitives>`
/// (render world) dedupes identical keys into one `CachedRenderPipelineId`.
#[derive(Default)]
pub struct BuiyPrimitives;

impl BuiyPrimitives {
    /// The two interleaved vertex-buffer layouts shared by every quad-family
    /// primitive (static unit quad, stride 16; per-instance record, stride 52).
    /// The instance record carries the per-primitive clip AABB at
    /// `@location(6)`/`(7)` (R8b); its `array_stride` tracks
    /// [`PACKED_INSTANCE_STRIDE_BYTES`] (52 B).
    ///
    /// [`PACKED_INSTANCE_STRIDE_BYTES`]: crate::render::instance::PACKED_INSTANCE_STRIDE_BYTES
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
                array_stride: 52,
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
                    // R8b clip AABB: `clip_min` @ 36, `clip_max` @ 44 — appended
                    // after `radius`/`blur` (@ 32); see `PackedInstance` and both
                    // quad-family shaders' `Instance.clip_min`/`clip_max`.
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 36,
                        shader_location: 6,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 44,
                        shader_location: 7,
                    },
                ],
            },
        ]
    }

    /// The two vertex-buffer layouts for the coverage-glyph primitive: the
    /// static unit quad (stride 16, shared with the quad family — VBO 0) and the
    /// per-instance [`GlyphAlphaInstance`] record (stride 68 — VBO 1,
    /// `step_mode: Instance`). The attribute offsets/formats MUST match
    /// `GlyphAlphaInstance`'s `#[repr(C)]` field offsets byte-for-byte and
    /// `coverage.wgsl`'s `@location`s:
    ///
    /// | field   | offset | format       | `@location` |
    /// |---------|--------|--------------|-------------|
    /// | (vertex) position | 0  | Float32x2 | 0 |
    /// | (vertex) uv       | 8  | Float32x2 | 1 |
    /// | rect    | 0   | Float32x4    | 2 |
    /// | uv      | 16  | Float32x4    | 3 |
    /// | color   | 32  | Float32x4    | 4 |
    /// | clip    | 48  | Float32x4    | 5 |
    /// | page    | 64  | Uint32       | 6 |
    ///
    /// Total instance stride 68 B = [`GLYPH_ALPHA_INSTANCE_STRIDE_BYTES`].
    ///
    /// [`GLYPH_ALPHA_INSTANCE_STRIDE_BYTES`]: crate::render::atlas::GLYPH_ALPHA_INSTANCE_STRIDE_BYTES
    fn glyph_vertex_buffers() -> Vec<VertexBufferLayout> {
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
                array_stride: 68,
                step_mode: VertexStepMode::Instance,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 2,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 3,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 4,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 48,
                        shader_location: 5,
                    },
                    VertexAttribute {
                        format: VertexFormat::Uint32,
                        offset: 64,
                        shader_location: 6,
                    },
                ],
            },
        ]
    }

    /// The shader handle for a primitive kind. Border folds into the quad
    /// SDF and outline is a clip-suppressed quad, so both paint through the
    /// quad shader — neither is a distinct `BuiyPrimitiveKind` variant.
    /// `Glyph` paints through the coverage (alpha-as-color) shader (octet
    /// `..03`); the `Path` shader (octet `..04`) is sibling-phase work, so it
    /// still falls back to the quad shader as a placeholder.
    fn shader_for(kind: BuiyPrimitiveKind) -> bevy::asset::Handle<bevy::shader::Shader> {
        match kind {
            BuiyPrimitiveKind::Quad => shader_handle(),
            BuiyPrimitiveKind::Shadow => shadow_shader_handle(),
            BuiyPrimitiveKind::Glyph => coverage_shader_handle(),
            BuiyPrimitiveKind::Path => shader_handle(),
        }
    }
}

impl SpecializedRenderPipeline for BuiyPrimitives {
    type Key = BuiyPrimitiveKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let shader = Self::shader_for(key.kind);
        let is_glyph = key.kind == BuiyPrimitiveKind::Glyph;
        // The glyph (coverage) pipeline samples the atlas, so it declares the
        // additive `@group(1)` (texture + sampler) ON TOP OF the shared
        // `@group(0)` view uniform. The non-sampling quad-family pipelines keep
        // their single `@group(0)` layout byte-identical (design fork #2): a
        // pipeline that declared a `@group(1)` its shader never binds is just
        // wasted layout, and one whose shader binds a group the layout omits
        // fails wgpu validation — so the layout tracks the shader exactly.
        let layout = if is_glyph {
            vec![view_uniform_layout_descriptor(), atlas_layout_descriptor()]
        } else {
            vec![view_uniform_layout_descriptor()]
        };
        // The glyph instance record is `GlyphAlphaInstance` (stride 68), a
        // different layout from the quad family's `PackedInstance` (stride 52).
        let buffers = if is_glyph {
            Self::glyph_vertex_buffers()
        } else {
            Self::quad_family_vertex_buffers()
        };
        RenderPipelineDescriptor {
            label: Some(format!("buiy_{:?}_pipeline", key.kind).into()),
            // Every quad-family shader binds the view uniform at
            // `@group(0) @binding(0)` (see `shader.wgsl`); the pipeline layout
            // must declare a matching group or wgpu validation rejects the
            // pipeline. This is the same `@group(0)` descriptor the Phase-0
            // `register` path supplies, shared so the two cannot drift. The
            // glyph pipeline appends `@group(1)` for the atlas (see above).
            layout,
            // wgpu 28: push constants → "immediates". Buiy uses none.
            immediate_size: 0,
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers,
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            // The sample-count/edge seam: keyed off the bound attachment's
            // sample count, like `format` below. The shaders are sample-count
            // agnostic — MSAA changes only this state + the pass attachments.
            multisample: MultisampleState {
                count: key.samples,
                ..Default::default()
            },
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
