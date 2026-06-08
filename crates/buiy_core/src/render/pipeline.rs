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
    ShaderStages, SpecializedRenderPipeline, TextureFormat, binding_types::uniform_buffer,
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

#[derive(Resource)]
pub struct BuiyPipeline {
    pub id: CachedRenderPipelineId,
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
    world.insert_resource(BuiyPipeline {
        id,
        vertex_buffer,
        view_layout,
    });
}
