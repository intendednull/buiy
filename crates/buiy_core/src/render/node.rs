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

use super::{
    atlas::AtlasGpu, compositor::PreparedEffectGroups, pipeline::BuiyPipeline,
    prepare::BuiyInstanceBuffers,
};

#[derive(Default)]
pub struct BuiyNode;

impl ViewNode for BuiyNode {
    // The per-view `PreparedEffectGroups` (effect-compositor.md § 1.1) is read
    // as `Option<&...>` so a view with NO effect groups (the absence of the
    // component, or one with an empty `groups` vec) runs the existing flat pass
    // byte-for-byte unchanged. The prepare pass attaches it on views that have
    // live `EffectGroup`s (architecture § 4 / effect-compositor.md § 1.1).
    type ViewQuery = (&'static ViewTarget, Option<&'static PreparedEffectGroups>);

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_target, prepared): QueryItem<'w, '_, Self::ViewQuery>,
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
        // uploaded). Glyphs draw even with zero quads (a pure-text frame), so the
        // skip checks BOTH counts.
        if buffers.quad_count == 0 && buffers.glyph_count == 0 {
            return Ok(());
        }
        // The view uniform is required for any draw (both pipelines bind it at
        // `@group(0)`); it is `None` until the first `write_buffer`.
        let Some(view_binding) = buffers.view_uniform.binding() else {
            return Ok(());
        };

        // Effect-group composite — step 1: group-subtree passes, innermost-first
        // (effect-compositor.md § 3 step 1). When the prepared per-view store
        // (§ 1.1) carries live groups, EACH group's subtree rasterizes into its
        // own off-screen `Rgba16Float` target BEFORE the in-flow window draw
        // below, in the precomputed post-order (children before parents, so an
        // ancestor samples a child's *composited* result, not its raw subtree).
        //
        // Target RESIDENCY (§ 3): every group target is acquired up-front and
        // held for the whole run — a child target filled here is sampled by its
        // parent at composite (step 2 / the tail below), so it must NOT be
        // recycled mid-run. The `CachedTexture`s ride the prepared per-view
        // store: `TextureCache::get` needs `&mut TextureCache`, which a render
        // node cannot obtain from `&World`, so acquisition lives in the prepare
        // pass (`prepare_effect_groups`, `RenderSystems::Prepare`) exactly as
        // Bevy's own `prepare_core_2d_depth_textures` does — the spec permits
        // "in the prepare pass OR at the very start of `run`" (§ 3), and the
        // prepare side is the only one with the mutable cache handle.
        //
        // The per-group typed-primitive draw consumes the group's `painters_z`
        // instance range and a `Rgba16Float`-targeted pipeline specialization
        // (effect-compositor.md § 2.2 / architecture § 1.4) — neither lands in
        // this phase: `PreparedEffectGroup` carries `bounds`/`extent`/`opacity`/
        // `reason`/`parent`/`index`, and the prepare body that fills the store +
        // the per-format composite pipeline are the deferred upstream seams
        // (Task 9 body / architecture § 1.4). So `prepared.groups` is empty in
        // v1 and this loop is inert; it is the structural seam the draws slot
        // into, mirroring `prepare_effect_groups`'s documented skeleton body.
        if let Some(prepared) = prepared {
            for group in &prepared.groups {
                // clear `targets[group.index]` transparent, then run the
                // typed-primitive pass over `group`'s `painters_z` slice into it
                // (effect-compositor.md § 3 step 1). Nested groups appear as a
                // single composited sample — handled by their earlier iteration.
                let _ = group;
            }
        }

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
        // The view uniform `@group(0)` is shared by both the quad and glyph
        // pipelines, so it is bound once for the whole pass.
        pass.set_bind_group(0, &view_bind_group, &[]);
        pass.set_vertex_buffer(0, buiy_pipeline.vertex_buffer.slice(..));

        // --- Quad draw (paint order: quad after shadow, before glyph) --------
        // `buffer()` is `None` until the first `write_buffer`; a zero-count or
        // not-yet-uploaded quad buffer simply skips the quad draw (glyphs may
        // still draw below).
        if buffers.quad_count > 0
            && let Some(instance_buffer) = buffers.quad.buffer()
        {
            pass.set_render_pipeline(pipeline);
            pass.set_vertex_buffer(1, instance_buffer.slice(..));
            // v1: draws the whole instance buffer (no effect groups are live).
            // TODO(R9 prepare-body wiring): once `prepare_effect_groups`
            // populates `prepared.groups`, this flat draw MUST exclude the
            // instance ranges that belong to effect-group subtrees — those are
            // rasterized into their own off-screen targets in step 1 above and
            // composited back in step 2 below. Drawing them here too would
            // double-paint them (once flat, once composited). The exclusion
            // mechanism depends on how the prepare body partitions the instance
            // buffer into per-group ranges (effect-compositor § 3); it is a no-op
            // while `prepared.groups` is empty.
            pass.draw(0..4, 0..buffers.quad_count);
        }

        // --- Glyph draw (paint order: glyph after quad) ----------------------
        // The coverage-glyph (alpha-as-color) primitive, drawn AFTER the quad so
        // text paints over fills (shadow < quad < glyph < path). Requires: the
        // glyph pipeline compiled, the atlas `@group(1)` bind group built by
        // `prepare_atlas_textures` (a coverage page exists), and a non-empty
        // uploaded glyph buffer. Any missing piece skips the glyph draw without
        // disturbing the quad draw above (e.g. before the pipeline async-compiles
        // or before the first glyph warms an atlas page).
        if buffers.glyph_count > 0
            && let Some(glyph_pipeline) = pipeline_cache.get_render_pipeline(buiy_pipeline.glyph_id)
            && let Some(atlas_gpu) = world.get_resource::<AtlasGpu>()
            && let Some(atlas_bind_group) = atlas_gpu.coverage_bind_group()
            && let Some(glyph_buffer) = buffers.glyph.buffer()
        {
            pass.set_render_pipeline(glyph_pipeline);
            // `@group(0)` (view) is already bound for the pass; add the atlas
            // `@group(1)` (texture + sampler) the coverage shader samples.
            pass.set_bind_group(1, atlas_bind_group, &[]);
            pass.set_vertex_buffer(1, glyph_buffer.slice(..));
            pass.draw(0..4, 0..buffers.glyph_count);
        }

        // Effect-group composite — step 2: composite each group target into its
        // parent target, bottom-up (effect-compositor.md § 3 step 2). In the
        // same precomputed post-order, each filled group target draws as one
        // textured quad into its PARENT target — the enclosing group's target
        // for a nested group (`group.parent == Some(i)`), or the window
        // `ViewTarget::main_texture_view()` for a root group (`group.parent ==
        // None`), which is why this composite-into-window runs AFTER the in-flow
        // flat draw above (the root group paints over the in-flow content). The
        // composite applies `EffectReason`: v1 multiplies the sampled alpha by
        // the prepared `group.opacity` and blends `SrcOver` in LINEAR space (the
        // group target is pinned `Rgba16Float`, § 2.2) — the `composite_src_over`
        // arithmetic run on the GPU, so overlapping children inside an
        // `opacity < 1` group composite once as a unit and do not double-darken
        // (the correct semantics, § 4 / § 5.1; the rejected per-child
        // approximation is only the under-budget degradation fallback, § 2.3).
        // `ISOLATION` alone changes nothing about this composite math (§ 4) — its
        // effect is structural, scoping descendants' blending within the target.
        //
        // Inert in v1 for the same reason as step 1: `prepared.groups` is empty
        // until the prepare body + composite pipeline land (Task 9 / architecture
        // § 1.4). The targets stay resident through here; `update_texture_cache_system`
        // (render `Cleanup`, under `DefaultPlugins`) un-`taken`s them next frame
        // (§ 2.2). Buiy adds NO copy of that system.
        if let Some(prepared) = prepared {
            for group in &prepared.groups {
                // composite `targets[group.index]` into its parent target
                // (`group.parent` → enclosing group, or the window ViewTarget at
                // the root), applying `group.opacity` × `SrcOver` (§ 3 step 2).
                let _ = group;
            }
        }

        // Effect-group composite — step 3 (top-layer): ONE draw suffices in v1,
        // no second pass (effect-compositor.md § 3 step 3).
        //
        // Top-layer members are NOT a separate render pass in v1. Layout
        // sub-pass 6f already places them at the TAIL of the root `painters_z`
        // (paint-order-and-top-layer.md § 6f), so they extract last → pack last
        // → draw last in the single instanced flat `draw` above — painting over
        // all in-flow content for free. Their `ExtractedNode.clip` is forced to
        // `None` (§ 3.2), packed to the `[±INFINITY]` full-view sentinel, so the
        // fragment-discard clip never fires and they paint unclipped over the
        // whole view. `painters_z` order is preserved VERBATIM here — render
        // never groups or re-sorts (pillar 1) — so a second pass is unnecessary.
        // A top-layer entry that is *itself* an effect group (a modal at
        // `opacity: 0.9`) composites its own target in steps 1–2 like any other
        // group, then draws here in top-layer order — the two mechanisms compose
        // without special-casing (§ 3 step 3). `partition_top_layer`
        // (render/top_layer.rs) is the landed helper that splits the in-flow and
        // top-layer instance ranges should a top-layer subtree ever need an
        // explicit separate pass.
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
