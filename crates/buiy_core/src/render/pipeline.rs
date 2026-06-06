//! Buiy render pipeline. The render-graph node in `node.rs` references
//! `BuiyPipeline::id` to dispatch draws.
//!
//! Full pipeline (multi-pass top-layer compositing, atlas binding,
//! filter/blend mode passes) lives in `buiy-render-pipeline-design`.

use core::marker::PhantomData;

use bevy::asset::uuid::Uuid;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState, Buffer,
    BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites,
    FragmentState, FrontFace, MultisampleState, PipelineCache, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPipelineDescriptor, ShaderStages, TextureFormat, VertexAttribute,
    VertexFormat, VertexState, VertexStepMode, binding_types::uniform_buffer,
};
use bevy::render::renderer::RenderDevice;
use bevy::shader::Shader;

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

    // Load WGSL shader into the render world's Shader asset store.
    // `Assets::insert` returns the previous asset at this id (always `None`
    // here — `register` runs once during plugin finish). Explicit `_prev`
    // documents that we are knowingly discarding it, not a fallible result.
    {
        let mut shaders = world.resource_mut::<Assets<Shader>>();
        let _prev = shaders.insert(
            shader_handle().id(),
            Shader::from_wgsl(include_str!("shader.wgsl"), "buiy/render/shader.wgsl"),
        );
    }

    // Bind-group layout for the per-view view uniform: one `var<uniform>` at
    // `@group(0) @binding(0)`, visible to the vertex stage (the logical->clip
    // transform happens in `vertex`). `[Vec4; 3]` is the `BuiyViewUniform`
    // std140 payload the prepare phase uploads (`as_std140_array`, regrouped into
    // the three `vec4` columns of the WGSL `BuiyView`); its min binding size is
    // 48 B, matching the WGSL struct. A bare `[f32; 12]` is NOT a valid uniform
    // payload (4-byte scalar-array stride violates std140's 16-byte rule), so the
    // carrier and this layout both use `[Vec4; 3]`.
    //
    // The SAME entries feed two consumers: the pipeline descriptor (a
    // `BindGroupLayoutDescriptor` the cache materializes + dedups) and the
    // concrete `BindGroupLayout` stored on `BuiyPipeline` for the node to build
    // the per-frame bind group. Both are byte-identical entries, so the bind
    // group is layout-compatible with the pipeline. Built from the render
    // device, a separate immutable borrow from the `PipelineCache` below — both
    // coexist.
    let view_layout_entries =
        BindGroupLayoutEntries::single(ShaderStages::VERTEX, uniform_buffer::<[Vec4; 3]>(false));
    let view_layout = world
        .resource::<RenderDevice>()
        .create_bind_group_layout("buiy_view_uniform_layout", &view_layout_entries);
    let view_layout_descriptor =
        BindGroupLayoutDescriptor::new("buiy_view_uniform_layout", &view_layout_entries);

    // Build pipeline descriptor and queue it.
    let pipeline_cache = world.resource::<PipelineCache>();

    let descriptor = RenderPipelineDescriptor {
        label: Some("buiy_rounded_rect_pipeline".into()),
        layout: vec![view_layout_descriptor],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader_handle(),
            shader_defs: vec![],
            entry_point: Some("vertex".into()),
            buffers: vec![
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
            ],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            front_face: FrontFace::Ccw,
            // cull_mode: None — Phase 0 closeout deliberate choice. The
            // TL/BL/TR/BR strip order produces consistent winding; tightening
            // to Some(Face::Back) is deferred to v0.x.
            cull_mode: None,
            polygon_mode: PolygonMode::Fill,
            ..default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            shader: shader_handle(),
            shader_defs: vec![],
            entry_point: Some("fragment".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        zero_initialize_workgroup_memory: false,
    };

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
