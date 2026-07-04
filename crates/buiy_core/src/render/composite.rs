//! The effect-group composite pass resources (effect-compositor.md § 3 step 2):
//! the textured-quad pipeline that samples a group's off-screen `Rgba16Float`
//! target and blends it `SrcOver` into the parent with `sampled.a * opacity`.
//!
//! Registered by `compositor::register` (resources/pipelines only — NO graph
//! node; the composite runs INSIDE `BuiyNode::run`, § 3). Two render-world
//! resources land here:
//! - [`CompositePipeline`] — the composite bind-group layouts, the shared
//!   sampler, the unit-quad VBO, and the per-parent-format `SpecializedRenderPipeline`.
//! - [`BuiySpecializedPipelines`] — the shared `SpecializedRenderPipelines`
//!   caches every Buiy specialization goes through: the per-view view-pass
//!   quad/glyph variants, the `Quad@Rgba16Float@1x` step-1 group-pass variant,
//!   and the per-parent composite variants.

use core::marker::PhantomData;

use bevy::asset::uuid::Uuid;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState, Buffer,
    BufferInitDescriptor, BufferUsages, ColorTargetState, ColorWrites, FragmentState, FrontFace,
    MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, SpecializedRenderPipeline,
    SpecializedRenderPipelines, TextureFormat, TextureSampleType, VertexAttribute, VertexFormat,
    VertexState, VertexStepMode,
    binding_types::{sampler, texture_2d, uniform_buffer},
};
use bevy::render::renderer::RenderDevice;
use bevy::shader::Shader;

use crate::render::primitive::BuiyPrimitives;

/// Stable UUID for the composite shader (octet `..05` — the next free octet after
/// coverage `..03`; `..04` is reserved for the deferred path shader).
const COMPOSITE_SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0105_0000_0000_0000_0000_0000_0005u128);

/// Weak handle to the composite WGSL shader (octet `..05`), backed by
/// `composite.wgsl`, loaded into the MAIN world by `BuiyRenderPlugin::build`.
pub fn composite_shader_handle() -> Handle<Shader> {
    Handle::Uuid(COMPOSITE_SHADER_UUID, PhantomData)
}

/// The composite `Composite` uniform (`@group(0) @binding(0)`): the parent
/// logical→clip columns, the quad's logical bounds, and `[uv_max.x, uv_max.y,
/// opacity, 0]`. Byte-identical to the WGSL `Composite` struct (4 × `vec4` = 64 B).
/// A separate `@group(0)` from the quad/glyph view uniform — the composite pass is
/// its own pipeline, so reusing the quad layout would be wrong (it carries
/// different data); the byte-identical-`@group(0)` rule the design pins applies to
/// the SHARED quad/shadow/glyph view uniform, which this pass does not touch.
pub const COMPOSITE_UNIFORM_VEC4S: usize = 4;

/// The `@group(0)` composite-params uniform layout entries (vertex + fragment):
/// one `var<uniform>` of `[Vec4; 4]` (64 B). Shared by the pipeline descriptor
/// (`specialize`) and the concrete [`BindGroupLayout`] the node binds against, so
/// they stay byte-identical.
fn uniform_layout_entries() -> [bevy::render::render_resource::BindGroupLayoutEntry; 1] {
    BindGroupLayoutEntries::single(
        ShaderStages::VERTEX_FRAGMENT,
        uniform_buffer::<[Vec4; COMPOSITE_UNIFORM_VEC4S]>(false),
    )
}

/// The `@group(1)` source-target layout entries (fragment): a filterable
/// `texture_2d<f32>` + a filtering `sampler` (the group target is `Rgba16Float`).
fn source_layout_entries() -> BindGroupLayoutEntries<2> {
    BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    )
}

fn uniform_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("buiy_composite_uniform_layout", &uniform_layout_entries())
}

fn source_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("buiy_composite_source_layout", &source_layout_entries())
}

#[derive(Resource)]
pub struct CompositePipeline {
    /// `@group(0)` composite-params uniform layout (vertex + fragment).
    pub uniform_layout: BindGroupLayout,
    /// `@group(1)` source-target texture + sampler layout (fragment).
    pub source_layout: BindGroupLayout,
    /// The shared `Rgba16Float`-target sampler (nearest, clamp — the composite
    /// samples integer-snapped used sub-rects, so no filtering bleed across the
    /// pow2 padding seam).
    pub sampler: Sampler,
    /// Static unit-quad VBO (4 verts, TriangleStrip; pos+uv interleaved, stride
    /// 16) shared by every composite draw.
    pub vertex_buffer: Buffer,
}

/// Key for the composite `SpecializedRenderPipeline`: the PARENT attachment
/// format (the window `Rgba8UnormSrgb` for a root group, or `Rgba16Float` for a
/// nested group's target) and its sample count (the view's `Msaa` samples for a
/// root group — the window attachment is multisampled under MSAA — or 1 for a
/// nested group's single-sampled `Rgba16Float` target).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CompositeKey {
    pub parent_format: TextureFormat,
    pub samples: u32,
}

impl SpecializedRenderPipeline for CompositePipeline {
    type Key = CompositeKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("buiy_composite_pipeline".into()),
            layout: vec![uniform_layout_descriptor(), source_layout_descriptor()],
            // wgpu 28: push constants → "immediates". Buiy uses none.
            immediate_size: 0,
            vertex: VertexState {
                shader: composite_shader_handle(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout {
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
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: None,
            // Must match the parent attachment's sample count (a root-group
            // composite draws into the multisampled window pass under MSAA).
            multisample: MultisampleState {
                count: key.samples,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader: composite_shader_handle(),
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.parent_format,
                    // Straight-alpha SrcOver in the parent's space (the group
                    // sample is already-composed straight-alpha linear).
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}

/// Render-world cache of every `BuiyPrimitives` and composite specialization:
/// the per-view view-pass variants (`prepare_buiy_view_pipelines` — quad/glyph
/// keyed on the view's format + `Msaa` samples), the `Quad@Rgba16Float@1x`
/// group-pass variant, and the per-parent composite variants (both
/// `prepare_effect_groups`). One shared cache so identical keys — including the
/// eager `pipeline::register` baseline vs. a 1x view's per-view key — dedup to
/// one `CachedRenderPipelineId` (architecture § 1.4). The prepare systems
/// specialize; the node only reads the resulting ids.
#[derive(Resource, Default)]
pub struct BuiySpecializedPipelines {
    /// Typed-primitive (`BuiyPrimitives`) specializations, every
    /// `(kind, format, samples)` variant.
    pub primitives: SpecializedRenderPipelines<BuiyPrimitives>,
    /// Per-`(parent_format, samples)` composite specializations.
    pub composite: SpecializedRenderPipelines<CompositePipeline>,
    /// Border/outline BAND (`BuiyBandPipeline`) specializations, per
    /// `(format, samples)` (styling-f-tier.md § 2.3 / § 3.4 — C6-a). A distinct
    /// pipeline keyed by record, NOT a new `BuiyPrimitiveKind`.
    pub band: SpecializedRenderPipelines<crate::render::primitive::BuiyBandPipeline>,
    /// Background-GRADIENT (`BuiyGradientPipeline`) specializations, per
    /// `(format, samples)` (parity Wave B1). A distinct pipeline keyed by record
    /// (the 2-stop `GradientInstance`), NOT a new `BuiyPrimitiveKind` — the
    /// band/shadow precedent.
    pub gradient: SpecializedRenderPipelines<crate::render::primitive::BuiyGradientPipeline>,
    /// Raster (textured-quad) (`BuiyRasterPipeline`) specializations, per
    /// `(format, samples)` (the drawing-canvas seam). A distinct pipeline keyed
    /// by record (the [`RasterInstance`](crate::render::raster::RasterInstance)),
    /// NOT a new `BuiyPrimitiveKind` — the band/gradient precedent.
    pub raster: SpecializedRenderPipelines<crate::render::raster::BuiyRasterPipeline>,
}

impl FromWorld for CompositePipeline {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        // Concrete layouts from the SAME entries the pipeline descriptor declares
        // (one source of truth — `*_layout_descriptor`), so the node's bind groups
        // are layout-compatible with the pipeline.
        let uniform_layout = device
            .create_bind_group_layout("buiy_composite_uniform_layout", &uniform_layout_entries());
        let source_layout = device
            .create_bind_group_layout("buiy_composite_source_layout", &source_layout_entries());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("buiy_composite_sampler"),
            address_mode_u: bevy::render::render_resource::AddressMode::ClampToEdge,
            address_mode_v: bevy::render::render_resource::AddressMode::ClampToEdge,
            address_mode_w: bevy::render::render_resource::AddressMode::ClampToEdge,
            mag_filter: bevy::render::render_resource::FilterMode::Nearest,
            min_filter: bevy::render::render_resource::FilterMode::Nearest,
            // wgpu 28+: `mipmap_filter` is `MipmapFilterMode` (mag/min stay `FilterMode`).
            mipmap_filter: bevy::render::render_resource::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Unit quad (pos, uv) interleaved, TriangleStrip TL,BL,TR,BR — the same
        // winding as the main quad VBO.
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVertex {
            pos: [f32; 2],
            uv: [f32; 2],
        }
        let quad: [QuadVertex; 4] = [
            QuadVertex {
                pos: [0.0, 0.0],
                uv: [0.0, 0.0],
            },
            QuadVertex {
                pos: [0.0, 1.0],
                uv: [0.0, 1.0],
            },
            QuadVertex {
                pos: [1.0, 0.0],
                uv: [1.0, 0.0],
            },
            QuadVertex {
                pos: [1.0, 1.0],
                uv: [1.0, 1.0],
            },
        ];
        let vertex_buffer = device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("buiy_composite_unit_quad_vbo"),
            contents: bytemuck::cast_slice(&quad),
            usage: BufferUsages::VERTEX,
        });

        Self {
            uniform_layout,
            source_layout,
            sampler,
            vertex_buffer,
        }
    }
}

/// Register composite resources (NO render-graph system or node). The
/// `CompositePipeline` (its layouts, sampler, VBO) needs the `RenderDevice`, so
/// its init is deferred to `finish` (where `RenderPlugin` has materialized the
/// device) via `register_gpu`; this `build`-time half only inits the device-free
/// specialization cache.
pub(crate) fn register(render_app: &mut bevy::app::SubApp) {
    render_app.init_resource::<BuiySpecializedPipelines>();
}

/// `finish`-time half: the device-owning `CompositePipeline` (`FromWorld` needs
/// the `RenderDevice` that `RenderPlugin::finish` materializes).
pub(crate) fn register_gpu(render_app: &mut bevy::app::SubApp) {
    render_app.init_resource::<CompositePipeline>();
}
