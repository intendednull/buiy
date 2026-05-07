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
    BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites, Face, FragmentState,
    FrontFace, MultisampleState, PipelineCache, PolygonMode, PrimitiveState, PrimitiveTopology,
    RenderPipelineDescriptor, TextureFormat, VertexAttribute, VertexFormat, VertexState,
    VertexStepMode,
};
use bevy::shader::Shader;

/// Stable UUID for the rounded-rect shader asset. Random u128 generated for
/// Phase 0 — keep it constant so the asset ID is stable across runs.
const SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0100_0000_0000_0000_0000_0000_0001u128);

/// Returns the stable weak handle to the rounded-rect WGSL shader.
pub fn shader_handle() -> Handle<Shader> {
    Handle::Uuid(SHADER_UUID, PhantomData)
}

#[derive(Resource)]
pub struct BuiyPipeline {
    pub id: CachedRenderPipelineId,
}

pub fn register(render_app: &mut SubApp) {
    let world = render_app.world_mut();

    // Load WGSL shader into the render world's Shader asset store.
    {
        let mut shaders = world.resource_mut::<Assets<Shader>>();
        let _ = shaders.insert(
            shader_handle().id(),
            Shader::from_wgsl(include_str!("shader.wgsl"), "buiy/render/shader.wgsl"),
        );
    }

    // Build pipeline descriptor and queue it.
    let pipeline_cache = world.resource::<PipelineCache>();

    let descriptor = RenderPipelineDescriptor {
        label: Some("buiy_rounded_rect_pipeline".into()),
        layout: vec![],
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
            cull_mode: Some(Face::Back),
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

    let id = pipeline_cache.queue_render_pipeline(descriptor);
    world.insert_resource(BuiyPipeline { id });
}
