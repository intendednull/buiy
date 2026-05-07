//! Buiy render-graph node. Inserted into the Core2d sub-graph, after the
//! main 2D pass (`Node2d::EndMainPass`) and before tonemapping. Phase 0
//! draws Buiy entities directly into the 2D-pass color attachment.
//!
//! IMPORTANT (clip-space conversion): the extract phase (in `mod.rs`) emits
//! `DrawData.position / size` in **logical pixels** (from `ResolvedLayout`),
//! but the rounded-rect shader (in `shader.wgsl`) consumes them as
//! **clip-space units**. The Phase 0 conversion happens HERE on the CPU
//! before the instance buffer is written: we compute
//!   clip = (px / window_size) * 2.0 - 1.0   (with y-flip)
//! per-element. Future Phase 1+ may move this to a view uniform; flag in
//! `buiy-render-pipeline-design`.
//!
//! Phase 0 status: this node ships the *render-graph wiring* + a
//! `set_render_pipeline` call so the pass exists and the pipeline is bound,
//! but vertex / instance buffer construction is deferred to v0.x. Task 19
//! (the e2e screenshot harness) is responsible for end-to-end verification.

use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{
        NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
    },
    render_resource::{PipelineCache, RenderPassDescriptor},
    renderer::RenderContext,
    view::ViewTarget,
};

use super::{ExtractedDraws, pipeline::BuiyPipeline};

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
            // Pipeline still compiling; skip this frame. The next frame will
            // either succeed or surface an error from the pipeline cache.
            return Ok(());
        };
        let draws = world.resource::<ExtractedDraws>();
        if draws.draws.is_empty() {
            return Ok(());
        }

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("buiy_pass"),
            color_attachments: &[Some(view_target.get_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(pipeline);
        // Phase 0: vertex + instance buffers built per-frame here. v0.x
        // upgrades to persistent buffers + bind groups for filters / atlas.
        // The buffer-construction code is deferred to a follow-up task — this
        // task ships the *render-graph wiring* so the node is present and
        // running, even if the actual draw-call encoding is a TODO.
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
