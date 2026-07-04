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
//! (`clip_min`/`clip_max`) at `@location(6)`/`(7)` and the R1 2D affine basis
//! (`affine_col0`/`affine_col1`) at `@location(8)`/`(9)`, lifting the instance
//! stride to 68 B; the quad-family shaders discard fragments outside the clip
//! and transform each box-local corner by the affine.

use bevy::mesh::VertexBufferLayout;
use bevy::render::render_resource::{
    BlendState, ColorTargetState, ColorWrites, FragmentState, FrontFace, MultisampleState,
    PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipelineDescriptor,
    SpecializedRenderPipeline, TextureFormat, VertexAttribute, VertexFormat, VertexState,
    VertexStepMode,
};

// Owned by R6 (render::buckets) — imported, not redefined.
use crate::render::buckets::BuiyPrimitiveKind;
use crate::render::instance::{
    BORDER_BAND_INSTANCE_STRIDE_BYTES, GRADIENT_INSTANCE_STRIDE_BYTES,
    ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES,
};
use crate::render::pipeline::{
    atlas_layout_descriptor, band_shader_handle, coverage_shader_handle, gradient_shader_handle,
    rounded_shadow_shader_handle, shader_handle, shadow_shader_handle,
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
    /// primitive (static unit quad, stride 16; per-instance record, stride 68).
    /// The instance record carries the per-primitive clip AABB at
    /// `@location(6)`/`(7)` (R8b) and the 2D affine basis at `@location(8)`/`(9)`
    /// (R1); its `array_stride` tracks [`PACKED_INSTANCE_STRIDE_BYTES`] (68 B).
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
                array_stride: 68,
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
                    // R1 2D affine basis: `affine_col0` @ 52, `affine_col1` @ 60
                    // — appended AFTER the clip fields so offsets 0..52 stay
                    // byte-stable (the R2 dependency). See `PackedInstance.affine`
                    // and both quad-family shaders' `Instance.affine_col0/1`.
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 52,
                        shader_location: 8,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 60,
                        shader_location: 9,
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
    /// | affine  | 68  | Float32x4    | 7 |
    ///
    /// Total instance stride 84 B = [`GLYPH_ALPHA_INSTANCE_STRIDE_BYTES`].
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
                array_stride: crate::render::atlas::GLYPH_ALPHA_INSTANCE_STRIDE_BYTES as u64,
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
                    // 2D affine basis (coverage-path rotate/scale) — mirrors the
                    // band's single-Float32x4 fold; keeps the glyph pipeline at
                    // 8/16 vertex attributes (WebGL2-safe).
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 68,
                        shader_location: 7,
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

/// One `SpecializedRenderPipeline` variant of the border/outline BAND pipeline
/// (styling-f-tier.md § 2.3 / § 3.4): a distinct pipeline keyed by record type
/// (NOT a new `BuiyPrimitiveKind`), built for a specific target color-attachment
/// format + sample count exactly like [`BuiyPrimitiveKey`]. The band draws
/// within the `(Quad, layer)` slot (so it paints over the fill within a box),
/// but rides its own `band.wgsl` shader + the distinct `BorderBandInstance`
/// vertex layout, leaving the 68 B quad stride byte-stable.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiyBandKey {
    /// The bound attachment's format (view format / `Rgba16Float` group target).
    pub format: TextureFormat,
    /// The bound attachment's sample count (the per-view `Msaa` count).
    pub samples: u32,
}

/// The border/outline band `SpecializedRenderPipeline`. Its own vertex layout
/// (the `BorderBandInstance` record) + shader (`band.wgsl`), sharing the quad
/// family's `@group(0)` view uniform — no `@group(1)`. `SpecializedRenderPipelines<BuiyBandPipeline>`
/// dedupes identical `(format, samples)` keys onto one `CachedRenderPipelineId`.
#[derive(Default)]
pub struct BuiyBandPipeline;

impl BuiyBandPipeline {
    /// The two interleaved vertex-buffer layouts for the band pipeline: the
    /// shared static unit quad (stride 16, VBO 0) and the per-instance
    /// [`BorderBandInstance`] record (VBO 1, `step_mode: Instance`). The
    /// attribute offsets/formats MUST match `BorderBandInstance`'s `#[repr(C)]`
    /// field offsets byte-for-byte and `band.wgsl`'s `@location`s.
    ///
    /// [`BorderBandInstance`]: crate::render::instance::BorderBandInstance
    fn band_vertex_buffers() -> Vec<VertexBufferLayout> {
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
                array_stride: BORDER_BAND_INSTANCE_STRIDE_BYTES as u64,
                step_mode: VertexStepMode::Instance,
                attributes: vec![
                    // rect_pos @0, rect_size @8
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
                    // color_top @16, color_right @32, color_bottom @48, color_left @64
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 4,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 5,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 48,
                        shader_location: 6,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 64,
                        shader_location: 7,
                    },
                    // width @80 ([t,r,b,l])
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 80,
                        shader_location: 8,
                    },
                    // outer_radius @96 (8 f32 = 32 B) split TL/TR @96, BR/BL @112
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 96,
                        shader_location: 9,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 112,
                        shader_location: 10,
                    },
                    // inner_radius @128 split TL/TR @128, BR/BL @144
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 128,
                        shader_location: 11,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 144,
                        shader_location: 12,
                    },
                    // clip_min @160 + clip_max @168 as ONE Float32x4 (the two
                    // contiguous [f32;2] struct fields read as one attribute) —
                    // folded to FREE a vertex-attribute slot for `style` (F4b-3)
                    // while staying at WebGL2's 16-attribute cap. Byte-identical
                    // upload; band.wgsl reads `.xy`/`.zw`.
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 160,
                        shader_location: 13,
                    },
                    // affine [m00,m10,m01,m11] @176 as ONE Float32x4 (was two
                    // Float32x2 cols). The struct field is already a contiguous
                    // [f32;4], so the stride is unchanged; band.wgsl reads
                    // `.xy`/`.zw`.
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 176,
                        shader_location: 15,
                    },
                    // style [top,right,bottom,left] dash flag @192 (F4b-3),
                    // appended after affine so all prior offsets stay byte-stable.
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 192,
                        shader_location: 14,
                    },
                ],
            },
        ]
    }
}

impl SpecializedRenderPipeline for BuiyBandPipeline {
    type Key = BuiyBandKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let shader = band_shader_handle();
        RenderPipelineDescriptor {
            label: Some("buiy_band_pipeline".into()),
            layout: vec![view_uniform_layout_descriptor()],
            immediate_size: 0,
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: Self::band_vertex_buffers(),
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: key.samples,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

/// One `SpecializedRenderPipeline` variant of the background-GRADIENT pipeline
/// (parity Wave B1): a distinct pipeline keyed by record type (the 2-stop
/// `GradientInstance`), NOT a new `BuiyPrimitiveKind` — the band/shadow
/// precedent. Built for a specific target color-attachment format + sample count
/// exactly like [`BuiyBandKey`]. Painted within the quad paint slot (over the
/// solid fill, under glyphs/bands), riding its own `gradient.wgsl` shader + the
/// distinct vertex layout, leaving the 68 B quad stride byte-stable.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiyGradientKey {
    /// The bound attachment's format (view format / `Rgba16Float` group target).
    pub format: TextureFormat,
    /// The bound attachment's sample count (the per-view `Msaa` count).
    pub samples: u32,
}

/// The background-gradient `SpecializedRenderPipeline`. Its own vertex layout
/// (the `GradientInstance` record) + shader (`gradient.wgsl`), sharing the quad
/// family's `@group(0)` view uniform — no `@group(1)`.
/// `SpecializedRenderPipelines<BuiyGradientPipeline>` dedupes identical
/// `(format, samples)` keys onto one `CachedRenderPipelineId`.
#[derive(Default)]
pub struct BuiyGradientPipeline;

impl BuiyGradientPipeline {
    /// The two interleaved vertex-buffer layouts for the gradient pipeline: the
    /// shared static unit quad (stride 16, VBO 0) and the per-instance
    /// [`GradientInstance`] record (VBO 1, `step_mode: Instance`). The attribute
    /// offsets/formats MUST match `GradientInstance`'s `#[repr(C)]` field offsets
    /// byte-for-byte and `gradient.wgsl`'s `@location`s:
    ///
    /// | field      | offset | format    | `@location` |
    /// |------------|--------|-----------|-------------|
    /// | rect_pos   | 0      | Float32x2 | 2 |
    /// | rect_size  | 8      | Float32x2 | 3 |
    /// | color0     | 16     | Float32x4 | 4 |
    /// | color1     | 32     | Float32x4 | 5 |
    /// | stops      | 48     | Float32x2 | 6 |
    /// | axis       | 56     | Float32x2 | 7 |
    /// | params     | 64     | Float32x2 | 8 |
    /// | clip_min   | 72     | Float32x2 | 9 |
    /// | clip_max   | 80     | Float32x2 | 10 |
    /// | affine_c0  | 88     | Float32x2 | 11 |
    /// | affine_c1  | 96     | Float32x2 | 12 |
    ///
    /// Total instance stride 104 B = [`GRADIENT_INSTANCE_STRIDE_BYTES`].
    ///
    /// [`GradientInstance`]: crate::render::instance::GradientInstance
    /// [`GRADIENT_INSTANCE_STRIDE_BYTES`]: crate::render::instance::GRADIENT_INSTANCE_STRIDE_BYTES
    fn gradient_vertex_buffers() -> Vec<VertexBufferLayout> {
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
                array_stride: GRADIENT_INSTANCE_STRIDE_BYTES as u64,
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
                        format: VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 5,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 48,
                        shader_location: 6,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 56,
                        shader_location: 7,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 64,
                        shader_location: 8,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 72,
                        shader_location: 9,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 80,
                        shader_location: 10,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 88,
                        shader_location: 11,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 96,
                        shader_location: 12,
                    },
                ],
            },
        ]
    }
}

impl SpecializedRenderPipeline for BuiyGradientPipeline {
    type Key = BuiyGradientKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let shader = gradient_shader_handle();
        RenderPipelineDescriptor {
            label: Some("buiy_gradient_pipeline".into()),
            layout: vec![view_uniform_layout_descriptor()],
            immediate_size: 0,
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: Self::gradient_vertex_buffers(),
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: key.samples,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

/// One `SpecializedRenderPipeline` variant of the ROUNDED box-shadow pipeline
/// (F4b-6): a distinct pipeline keyed by record type (the `RoundedShadowInstance`),
/// NOT a new `BuiyPrimitiveKind` — the band/gradient/raster precedent, and the
/// decided Option B (spec §2.5.1). Built for a specific target format + sample
/// count exactly like [`BuiyGradientKey`]. Painted in the SHADOW tier (before the
/// flat quads, alongside the square shadow blob), leaving the 68 B quad stride and
/// the square-shadow path byte-stable.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BuiyRoundedShadowKey {
    /// The bound attachment's format (view format / `Rgba16Float` group target).
    pub format: TextureFormat,
    /// The bound attachment's sample count (the per-view `Msaa` count).
    pub samples: u32,
}

/// The rounded box-shadow `SpecializedRenderPipeline`. Its own vertex layout (the
/// `RoundedShadowInstance` record) + shader (`rounded_shadow.wgsl`), sharing the
/// quad family's `@group(0)` view uniform — no `@group(1)`.
#[derive(Default)]
pub struct BuiyRoundedShadowPipeline;

impl BuiyRoundedShadowPipeline {
    /// The two interleaved vertex-buffer layouts: the shared static unit quad
    /// (stride 16, VBO 0) and the per-instance [`RoundedShadowInstance`] record
    /// (VBO 1). Offsets/formats MUST match its `#[repr(C)]` fields +
    /// `rounded_shadow.wgsl`'s `@location`s:
    ///
    /// | field        | offset | format    | `@location` |
    /// |--------------|--------|-----------|-------------|
    /// | rect_pos     | 0      | Float32x2 | 2 |
    /// | rect_size    | 8      | Float32x2 | 3 |
    /// | color        | 16     | Float32x4 | 4 |
    /// | sigma,radius | 32     | Float32x2 | 5 |
    /// | clip_min/max | 40     | Float32x4 | 6 |
    /// | affine       | 56     | Float32x4 | 7 |
    ///
    /// Total instance stride 72 B = [`ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES`] (8
    /// vertex attributes total — well under the WebGL2 16-attribute cap).
    ///
    /// [`RoundedShadowInstance`]: crate::render::instance::RoundedShadowInstance
    fn rounded_shadow_vertex_buffers() -> Vec<VertexBufferLayout> {
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
                array_stride: ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES as u64,
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
                    // (sigma, radius) folded to one Float32x2 @32.
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 32,
                        shader_location: 5,
                    },
                    // clip_min.xy + clip_max.zw folded to one Float32x4 @40.
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 40,
                        shader_location: 6,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x4,
                        offset: 56,
                        shader_location: 7,
                    },
                ],
            },
        ]
    }
}

impl SpecializedRenderPipeline for BuiyRoundedShadowPipeline {
    type Key = BuiyRoundedShadowKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let shader = rounded_shadow_shader_handle();
        RenderPipelineDescriptor {
            label: Some("buiy_rounded_shadow_pipeline".into()),
            layout: vec![view_uniform_layout_descriptor()],
            immediate_size: 0,
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: Self::rounded_shadow_vertex_buffers(),
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: key.samples,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
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
        // DISTINCT layout from the quad family's `PackedInstance` — even though
        // both strides are now 68 B (R1), the attr sets, raw types ([f32;17] vs
        // GlyphAlphaInstance), and pipelines differ and must not be conflated.
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

#[cfg(test)]
mod band_attr_cap_tests {
    use super::*;

    /// WebGL2 caps `max_vertex_attributes` at 16 (`downlevel_webgl2_defaults`), and
    /// an over-cap band pipeline makes `create_render_pipeline` fail -> the whole
    /// `buiy_pass` CommandEncoder fails to finish -> blank screen. This guards the
    /// affine fold: the band layout must never exceed 16 vertex attributes again.
    #[test]
    fn band_layout_stays_within_webgl2_16_attribute_cap() {
        let buffers = BuiyBandPipeline::band_vertex_buffers();
        let locations: Vec<u32> = buffers
            .iter()
            .flat_map(|b| b.attributes.iter().map(|a| a.shader_location))
            .collect();
        assert!(
            locations.len() <= 16,
            "band pipeline declares {} vertex attributes; WebGL2 caps at 16",
            locations.len()
        );
        let max_loc = locations.iter().copied().max().unwrap_or(0);
        assert!(
            max_loc <= 15,
            "band pipeline max shader_location is {max_loc}; WebGL2's 16-attribute cap needs <= 15"
        );
    }

    /// The coverage (glyph/icon) pipeline gained a 2D affine attribute (loc 7).
    /// Same WebGL2 cap as the band: guard that the layout stays <= 16 attributes /
    /// max location <= 15 so a future field add can't silently break WebGL2.
    #[test]
    fn glyph_layout_stays_within_webgl2_16_attribute_cap() {
        let buffers = BuiyPrimitives::glyph_vertex_buffers();
        let locations: Vec<u32> = buffers
            .iter()
            .flat_map(|b| b.attributes.iter().map(|a| a.shader_location))
            .collect();
        assert!(
            locations.len() <= 16,
            "glyph/coverage pipeline declares {} vertex attributes; WebGL2 caps at 16",
            locations.len()
        );
        let max_loc = locations.iter().copied().max().unwrap_or(0);
        assert!(
            max_loc <= 15,
            "glyph/coverage pipeline max shader_location is {max_loc}; WebGL2's cap needs <= 15"
        );
    }
}
