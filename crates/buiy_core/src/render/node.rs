//! Buiy render-graph node. Inserted into the Core2d sub-graph, after the
//! main 2D pass (`Node2d::EndMainPass`) and before tonemapping. Phase 0
//! draws Buiy entities directly into the 2D-pass color attachment.
//!
//! Why pre-tonemap: Buiy widgets share the same color pipeline as 2D scene
//! content, so widget output participates in tonemapping when HDR / advanced
//! color management is enabled in v0.x. Inserting after tonemapping would
//! force Buiy to manage its own color-space matching with the rest of the
//! frame, which is unnecessary complexity for Phase 0.
//!
//! Phase 0 closeout (2026-05-08): this node builds the instance buffer
//! per-frame from `ExtractedDraws` (logical-pixel → clip-space conversion
//! lives in `render::instance::to_instance`) and issues an instanced
//! `draw(0..4, 0..n)` against the static unit-quad VBO held on
//! `BuiyPipeline`. v0.x upgrades to persistent buffers + bind groups for
//! filters / atlas (`buiy-render-pipeline-design`); the conversion will
//! move to a view uniform at that point.

use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{
        NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
    },
    render_resource::{BufferInitDescriptor, BufferUsages, PipelineCache, RenderPassDescriptor},
    renderer::RenderContext,
    view::ViewTarget,
};

use super::{ExtractedDraws, instance::to_instance, pipeline::BuiyPipeline};

#[derive(Default)]
pub struct BuiyNode;

impl ViewNode for BuiyNode {
    type ViewQuery = &'static ViewTarget;

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        view_target: QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let buiy_pipeline = world.resource::<BuiyPipeline>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(buiy_pipeline.id) else {
            return Ok(());
        };
        let draws = world.resource::<ExtractedDraws>();
        if draws.draws.is_empty() || draws.window_size.x <= 0.0 || draws.window_size.y <= 0.0 {
            return Ok(());
        }

        // Pack instances. Phase 0 closeout: per-frame allocation; v0.x
        // (`buiy-render-pipeline-design`) introduces persistent buffers.
        let instances: Vec<_> = draws
            .draws
            .iter()
            .map(|d| to_instance(d, draws.window_size))
            .collect();

        let render_device = render_context.render_device();
        let instance_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("buiy_instance_vbo"),
            contents: bytemuck::cast_slice(&instances),
            usage: BufferUsages::VERTEX,
        });

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("buiy_pass"),
            color_attachments: &[Some(view_target.get_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_vertex_buffer(0, buiy_pipeline.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.draw(0..4, 0..instances.len() as u32);
        Ok(())
    }
}

/// Stable [`RenderLabel`] for the Buiy node inside the [`Core2d`] sub-graph.
#[derive(RenderLabel, Hash, PartialEq, Eq, Debug, Clone)]
pub struct BuiyRenderLabel;

pub(crate) fn register(render_app: &mut SubApp) {
    // Insert the Buiy node into Core2d, then declare an edge from
    // `Node2d::EndMainPass -> BuiyRenderLabel` so we run *after* the main 2D
    // pass and write into the same color attachment. Tonemapping /
    // post-processing run after us via the existing Core2d ordering.
    render_app
        .add_render_graph_node::<ViewNodeRunner<BuiyNode>>(Core2d, BuiyRenderLabel)
        .add_render_graph_edge(Core2d, Node2d::EndMainPass, BuiyRenderLabel);
}
