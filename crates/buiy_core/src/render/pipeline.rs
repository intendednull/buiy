//! Buiy render pipeline. The render-graph node in `node.rs` references
//! `BuiyPipeline::id` to dispatch draws.
//!
//! Full pipeline (multi-pass top-layer compositing, atlas binding,
//! filter/blend mode passes) lives in `buiy-render-pipeline-design`.

use core::marker::PhantomData;

use bevy::asset::uuid::Uuid;
use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BindGroupLayoutEntry,
    Buffer, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, PipelineCache,
    SamplerBindingType, ShaderStages, SpecializedRenderPipeline, TextureFormat, TextureSampleType,
    binding_types::{sampler, texture_2d, uniform_buffer},
};
use bevy::render::renderer::RenderDevice;
use bevy::shader::Shader;

use crate::render::buckets::BuiyPrimitiveKind;
use crate::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};

/// Stable UUID for the rounded-rect shader asset.
///
/// **Buiy render-asset UUID convention.** All render-asset UUIDs in `buiy_core`
/// use the prefix `0xB01A_01XX_..` ("BUIY 01") with the trailing octet
/// distinguishing the asset (01 = rounded-rect shader; the view-uniform bind
/// group at `@group(0) @binding(0)` is now part of this rounded-rect pipeline,
/// not a separate asset). When future tasks add shader / atlas / pipeline
/// assets, increment the trailing octet and document in this comment block.
/// Reserved range: `0xB01A_0100_0000_0000_0000_0000_0000_0001`
/// through `0xB01A_01FF_..._FFFF`.
const SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0100_0000_0000_0000_0000_0000_0001u128);

/// Returns the stable weak handle to the rounded-rect WGSL shader.
pub fn shader_handle() -> Handle<Shader> {
    Handle::Uuid(SHADER_UUID, PhantomData)
}

/// Stable UUID for the box-shadow SDF shader (octet `..02`).
const SHADOW_SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0102_0000_0000_0000_0000_0000_0002u128);

/// Weak handle to the box-shadow WGSL shader (octet `..02`).
///
/// Backed by `shadow.wgsl`, which `register` inserts under
/// `SHADOW_SHADER_UUID` at plugin finish.
pub fn shadow_shader_handle() -> Handle<Shader> {
    Handle::Uuid(SHADOW_SHADER_UUID, PhantomData)
}

/// Stable UUID for the coverage-glyph (alpha-as-color) shader (octet `..03`).
const COVERAGE_SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0103_0000_0000_0000_0000_0000_0003u128);

/// Weak handle to the coverage-glyph WGSL shader (octet `..03`).
///
/// Backed by `coverage.wgsl` (the alpha-as-color glyph primitive,
/// atlas-and-text-seam.md § 4.1), loaded under `COVERAGE_SHADER_UUID` into the
/// MAIN world by `BuiyRenderPlugin::build` (`load_internal_asset!`).
pub fn coverage_shader_handle() -> Handle<Shader> {
    Handle::Uuid(COVERAGE_SHADER_UUID, PhantomData)
}

/// The bind-group-layout entries for the per-view view uniform: one
/// `var<uniform>` at `@group(0) @binding(0)`, visible to the vertex stage (the
/// logical->clip transform happens in `vertex`). `[Vec4; 3]` is the
/// `BuiyViewUniform` std140 payload the prepare phase uploads (`as_std140_array`,
/// regrouped into the three `vec4` columns of the WGSL `BuiyView`); its min
/// binding size is 48 B, matching the WGSL struct. A bare `[f32; 12]` is NOT a
/// valid uniform payload (4-byte scalar-array stride violates std140's 16-byte
/// rule), so the carrier and this layout both use `[Vec4; 3]`.
fn view_uniform_layout_entries() -> [BindGroupLayoutEntry; 1] {
    BindGroupLayoutEntries::single(ShaderStages::VERTEX, uniform_buffer::<[Vec4; 3]>(false))
}

/// The pipeline-layout descriptor for the view uniform `@group(0)`. Shared by
/// `register` (the Phase-0 quad pipeline) and `BuiyPrimitives::specialize` (the
/// typed-primitive pipelines) so both declare a `@group(0)` matching the
/// quad-family shaders' `@group(0) @binding(0) var<uniform> view` binding — a
/// `RenderPipelineDescriptor` whose shader binds `@group(0)` but whose `layout`
/// declares zero groups fails wgpu validation. One source of truth keeps the
/// descriptor byte-identical to the concrete `BindGroupLayout` the node binds.
pub(crate) fn view_uniform_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("buiy_view_uniform_layout", &view_uniform_layout_entries())
}

/// The bind-group-layout entries for the atlas `@group(1)`: a fragment-stage
/// `texture_2d<f32>` (binding 0) + a filtering `sampler` (binding 1). This is
/// **additive** — only the atlas-sampling pipelines (coverage glyph) declare it;
/// the non-sampling quad/shadow pipelines keep their single-group `@group(0)`
/// layout byte-identical (GPU-verify design fork #2). The coverage page is
/// `R8Unorm` (a filterable float-sampled format), so `TextureSampleType::Float
/// { filterable: true }` + `SamplerBindingType::Filtering` match the WGSL
/// `texture_2d<f32>` + `sampler`.
fn atlas_layout_entries() -> BindGroupLayoutEntries<2> {
    BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    )
}

/// The pipeline-layout descriptor for the atlas `@group(1)`. Shared by the
/// coverage pipeline's `specialize` (so the descriptor declares the group the
/// shader binds) and the concrete [`build_atlas_layout`] the bind group is built
/// against — one source of truth, so the bind group is layout-compatible.
pub(crate) fn atlas_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("buiy_atlas_layout", &atlas_layout_entries())
}

/// Build the concrete `@group(1)` atlas bind-group layout from the device. The
/// atlas prepare system (`atlas::gpu::prepare_atlas_textures`) builds the
/// coverage bind group against the copy stored on [`BuiyPipeline::atlas_layout`];
/// this is the constructor for it. Same entries as `atlas_layout_descriptor`
/// (the crate-private descriptor the coverage pipeline declares), so the bind
/// group is layout-compatible with the pipeline.
pub fn build_atlas_layout(device: &RenderDevice) -> BindGroupLayout {
    device.create_bind_group_layout("buiy_atlas_layout", &atlas_layout_entries())
}

#[derive(Resource)]
pub struct BuiyPipeline {
    pub id: CachedRenderPipelineId,
    /// Cached pipeline id for the coverage-glyph (alpha-as-color) primitive,
    /// keyed to the view format (`Rgba8UnormSrgb`). The glyph node looks this up
    /// to set the coverage pipeline before its glyph draw. Specialized through
    /// the same `BuiyPrimitives` specializer as `id` (the `Glyph` kind), so it
    /// reuses `coverage.wgsl` + the `@group(0)`/`@group(1)` layout.
    pub glyph_id: CachedRenderPipelineId,
    /// Static unit-quad vertex buffer (4 verts, TriangleStrip). Created once
    /// at pipeline registration and reused every frame. Phase 0 closeout
    /// scope: vertex emission order matches the `cull_mode: None` setting in
    /// the descriptor; v0.x tightens to back-face culling.
    pub vertex_buffer: Buffer,
    /// Bind-group layout for the per-view view uniform (`@group(0) @binding(0)`,
    /// `var<uniform> view: BuiyView`, vertex stage). The node builds the bind
    /// group from this layout against `BuiyInstanceBuffers::view_uniform` each
    /// frame; the layout itself is created once here.
    pub view_layout: BindGroupLayout,
    /// Bind-group layout for the atlas `@group(1)` (`texture_2d<f32>` + a
    /// `sampler`, fragment stage). The atlas prepare system builds the coverage
    /// bind group against this; created once here from the SAME entries the
    /// coverage pipeline descriptor declares, so the bind group is
    /// layout-compatible. Additive — quad/shadow never bind it (design fork #2).
    pub atlas_layout: BindGroupLayout,
}

pub(crate) fn register(render_app: &mut SubApp) {
    let world = render_app.world_mut();

    // NOTE: the WGSL shaders (`shader_handle`/`shadow_shader_handle`) are loaded
    // into the MAIN world's `Assets<Shader>` by `BuiyRenderPlugin::build`
    // (`load_internal_asset!`), NOT here — the render world has no
    // `Assets<Shader>` resource, only the extracted GPU mirror the
    // `PipelineCache` resolves the handle against. This function builds the
    // device-dependent pieces (bind-group layout, vertex buffer, queued
    // pipeline) that genuinely need the render world's `RenderDevice` /
    // `PipelineCache`.

    // The SAME view-uniform layout feeds two consumers: the pipeline descriptor
    // (a `BindGroupLayoutDescriptor` the cache materializes + dedups, built by
    // `view_uniform_layout_descriptor`) and the concrete `BindGroupLayout`
    // stored on `BuiyPipeline` for the node to build the per-frame bind group.
    // Both come from `view_uniform_layout_entries`, so they are byte-identical
    // and the bind group is layout-compatible with the pipeline. The concrete
    // layout is built from the render device, a separate immutable borrow from
    // the `PipelineCache` below — both coexist.
    let view_layout = world
        .resource::<RenderDevice>()
        .create_bind_group_layout("buiy_view_uniform_layout", &view_uniform_layout_entries());

    // The concrete atlas `@group(1)` layout, from the same entries the coverage
    // pipeline descriptor declares (one source of truth — see
    // `atlas_layout_descriptor`). The atlas prepare system builds its coverage
    // bind group against this copy.
    let atlas_layout = build_atlas_layout(world.resource::<RenderDevice>());

    // Build pipeline descriptor and queue it.
    let pipeline_cache = world.resource::<PipelineCache>();

    // Build the quad / view-format pipeline through the typed-primitive
    // specializer rather than an inline descriptor literal, so the Phase-0
    // pipeline and the typed-primitive variants cannot drift (same vertex
    // layout, `@group(0)` view-uniform layout, blend, and entry points). The
    // main pass keys off `TextureFormat::Rgba8UnormSrgb` — exactly what
    // `ViewTarget::main_texture_format()` returns for the default `Camera2d`
    // view (architecture.md § 1.4) and what the Phase-0 descriptor hard-coded.
    // `register` runs at plugin finish, before any `ViewTarget` exists, so the
    // literal stands in for the view's default format here; per-format
    // registration through `SpecializedRenderPipelines` in a prepare pass is a
    // sibling phase.
    let descriptor = BuiyPrimitives.specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Quad,
        format: TextureFormat::Rgba8UnormSrgb,
    });

    // The coverage-glyph pipeline, same view format. Built through the SAME
    // specializer so the glyph vertex layout + `@group(0)`/`@group(1)` cannot
    // drift from the descriptor. Queued alongside the quad pipeline; the glyph
    // node binds `glyph_id` + the atlas `@group(1)` for its glyph draw.
    let glyph_descriptor = BuiyPrimitives.specialize(BuiyPrimitiveKey {
        kind: BuiyPrimitiveKind::Glyph,
        format: TextureFormat::Rgba8UnormSrgb,
    });

    let render_device = world.resource::<RenderDevice>();

    // Unit quad in (pos, uv) interleaved layout, matching the vertex-buffer
    // layout in `descriptor.vertex.buffers[0]`. TriangleStrip order: TL, BL,
    // TR, BR — both triangles wind consistently, which the v0.x backface-cull
    // tightening will rely on.
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
        }, // TL
        QuadVertex {
            pos: [0.0, 1.0],
            uv: [0.0, 1.0],
        }, // BL
        QuadVertex {
            pos: [1.0, 0.0],
            uv: [1.0, 0.0],
        }, // TR
        QuadVertex {
            pos: [1.0, 1.0],
            uv: [1.0, 1.0],
        }, // BR
    ];

    let vertex_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("buiy_unit_quad_vbo"),
        contents: bytemuck::cast_slice(&quad),
        usage: BufferUsages::VERTEX,
    });

    let id = pipeline_cache.queue_render_pipeline(descriptor);
    let glyph_id = pipeline_cache.queue_render_pipeline(glyph_descriptor);
    world.insert_resource(BuiyPipeline {
        id,
        glyph_id,
        vertex_buffer,
        view_layout,
        atlas_layout,
    });
}
