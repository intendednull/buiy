//! Buiy render-graph node. Inserted into the Core2d sub-graph in the
//! post-processing window — after `Node2d::StartMainPassPostProcessing` and
//! before `Node2d::Tonemapping` (architecture.md § 1.3). Phase 0 draws Buiy
//! entities directly into the 2D-pass color attachment.
//!
//! Why pre-tonemap: Buiy widgets share the same color pipeline as 2D scene
//! content, so widget output participates in tonemapping when HDR / advanced
//! color management is enabled in v0.x. Inserting after tonemapping would
//! force Buiy to manage its own color-space matching with the rest of the
//! frame, which is unnecessary complexity for Phase 0.
//!
//! View-uniform path (render-pipeline R6/R7): this node draws the PERSISTENT
//! per-view instance buffers maintained by `prepare_buiy_instances`
//! (`render::prepare`), never a per-frame allocation. The instance records are
//! LOGICAL pixels; the per-view `BuiyViewUniform` (bound at `@group(0)
//! @binding(0)`) does the logical→clip transform in the vertex stage, so the
//! Phase-0 per-instance y-flip / `2/min(w,h)` radius hack is retired. The node
//! binds the view uniform, sets the persistent quad buffer as
//! instance VBO 1, and issues `draw(0..4, 0..quad_count)` against the static
//! unit-quad VBO held on `BuiyPipeline`.

use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::{
    render_graph::{
        NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
    },
    render_resource::{BindGroupEntries, PipelineCache, RenderPassDescriptor},
    renderer::RenderContext,
    view::ViewTarget,
};

use super::{pipeline::BuiyPipeline, prepare::BuiyInstanceBuffers};

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

        // The persistent per-view buffers maintained by `prepare_buiy_instances`.
        // v1 carrier is the render-world resource shim (see `render::prepare`);
        // it is absent for one warm-up frame before the prepare system inserts
        // it, so a missing resource is a no-op draw, not an error.
        let Some(buffers) = world.get_resource::<BuiyInstanceBuffers>() else {
            return Ok(());
        };
        // Nothing to draw this frame (empty extract, or buffers not yet
        // uploaded — `buffer()` is `None` until the first `write_buffer`).
        if buffers.quad_count == 0 {
            return Ok(());
        }
        let (Some(instance_buffer), Some(view_binding)) =
            (buffers.quad.buffer(), buffers.view_uniform.binding())
        else {
            return Ok(());
        };

        // Bind group for the view uniform (`@group(0) @binding(0)`), built each
        // frame from the persistent uniform buffer against the layout the
        // pipeline declares.
        let view_bind_group = render_context.render_device().create_bind_group(
            "buiy_view_bind_group",
            &buiy_pipeline.view_layout,
            &BindGroupEntries::single(view_binding),
        );

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("buiy_pass"),
            color_attachments: &[Some(view_target.get_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &view_bind_group, &[]);
        pass.set_vertex_buffer(0, buiy_pipeline.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.draw(0..4, 0..buffers.quad_count);

        // v1 top-layer composite: ONE draw suffices, no second pass.
        //
        // Top-layer members are NOT a separate render pass in v1. Layout
        // sub-pass 6f already places them at the TAIL of the root `painters_z`
        // (paint-order-and-top-layer.md § 6f), so they extract last → pack last
        // → draw last in this single instanced `draw` — painting over all
        // in-flow content for free. Their `ExtractedNode.clip` is forced to
        // `None` (§ 3.2), packed to the `[±INFINITY]` full-view sentinel, so the
        // fragment-discard clip never fires and they paint unclipped over the
        // whole view. `painters_z` order is preserved VERBATIM here — render
        // never groups or re-sorts (pillar 1) — so a second pass is unnecessary.
        //
        // R9 RESERVED — per-`EffectGroup` intermediate passes go here. When an
        // effect group (opacity/blur/blend-isolation, component-model.md § 13)
        // or a top-layer subtree that must escape the sentinel clip needs its
        // own offscreen target, this is where the multi-pass composite lands:
        // render each group to an intermediate texture, then composite the
        // results in paint order after this base draw. `partition_top_layer`
        // (render/top_layer.rs) is the landed helper that splits the in-flow and
        // top-layer instance ranges for that explicit separate pass. v1 has no
        // such active second pass — the single draw above is the whole node.
        Ok(())
    }
}

/// Stable [`RenderLabel`] for the Buiy node inside the [`Core2d`] sub-graph.
#[derive(RenderLabel, Hash, PartialEq, Eq, Debug, Clone)]
pub struct BuiyRenderLabel;

pub(crate) fn register(render_app: &mut SubApp) {
    // Pin the Buiy node inside the Core2d post-processing window, AFTER
    // `Node2d::StartMainPassPostProcessing` and BEFORE `Node2d::Tonemapping`
    // (architecture.md § 1.3). The chained edge gives both a lower bound
    // (post-processing has started) and an UPPER bound (before tonemapping), so
    // widget paint lands in the post-processing window and participates in
    // tonemapping on the opt-in HDR path — unlike a lone `EndMainPass ->`
    // edge, which pins no upper bound and lets the node float past tonemapping.
    // The `Bloom -> BuiyRenderLabel` edge is deliberately NOT wired here: it
    // would force-add the optional `Bloom` node (§ 1.3); a 2D-bloom plugin that
    // adds the node can wire that edge itself.
    render_app
        .add_render_graph_node::<ViewNodeRunner<BuiyNode>>(Core2d, BuiyRenderLabel)
        .add_render_graph_edges(
            Core2d,
            (
                Node2d::StartMainPassPostProcessing,
                BuiyRenderLabel,
                Node2d::Tonemapping,
            ),
        );
}
