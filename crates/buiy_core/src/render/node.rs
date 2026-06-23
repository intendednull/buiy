//! Buiy render pass. Runs as a SYSTEM in the `Core2d` schedule's
//! `Core2dSystems::EarlyPostProcess` set — after the main 2D pass and before
//! tonemapping (`PostProcess`), the verified 0.19 successor to the removed
//! 0.18 `StartMainPassPostProcessing → Buiy → Tonemapping` graph slot
//! (architecture.md § 1.3; bevy `core_2d/mod.rs`). Phase 0 draws Buiy entities
//! directly into the 2D-pass color attachment.
//!
//! Why a system, not a `ViewNode`: Bevy 0.19 removed the `RenderGraph`
//! `Node`/`ViewNode` API. Passes are ordinary systems added to the `Core2d`
//! schedule; per-view data arrives via the `ViewQuery` system param and the
//! command encoder via the `RenderContext` system param (not a `&mut` arg).
//! Pass ordering that the old graph edges expressed is now system ordering
//! (`.in_set(EarlyPostProcess)`), and the pass's OWN multi-step structure
//! (group passes → nested composites → flat window pass → root composites) is
//! preserved by straight-line code in this single system: all
//! `begin_tracked_render_pass` calls share the system's one `RenderContext`
//! encoder, so the `LoadOp::Load` cross-pass loads the composites rely on stay
//! correct (a single command buffer, flushed at system end).
//!
//! Why pre-tonemap: Buiy widgets share the same color pipeline as 2D scene
//! content, so widget output participates in tonemapping when HDR / advanced
//! color management is enabled in v0.x. Inserting after tonemapping would
//! force Buiy to manage its own color-space matching with the rest of the
//! frame, which is unnecessary complexity for Phase 0.
//!
//! View-uniform path (render-pipeline R6/R7): this pass draws the PERSISTENT
//! per-view instance buffers maintained by `prepare_buiy_instances`
//! (`render::prepare`), never a per-frame allocation. The instance records are
//! LOGICAL pixels; the per-view `BuiyViewUniform` (bound at `@group(0)
//! @binding(0)`) does the logical→clip transform in the vertex stage, so the
//! Phase-0 per-instance y-flip / `2/min(w,h)` radius hack is retired. The pass
//! binds the view uniform, sets the persistent quad buffer as
//! instance VBO 1, and issues `draw(0..4, 0..quad_count)` against the static
//! unit-quad VBO held on `BuiyPipeline`.

use bevy::core_pipeline::{Core2d, Core2dSystems};
use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        BindGroupEntries, BufferInitDescriptor, BufferUsages, LoadOp, Operations, PipelineCache,
        RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::{RenderContext, ViewQuery},
    texture::CachedTexture,
    view::ViewTarget,
};

use super::{
    atlas::AtlasGpu,
    composite::CompositePipeline,
    compositor::{PreparedEffectGroups, PreparedEffectTargets},
    pipeline::{BuiyPipeline, BuiyViewPipelines},
    prepare::BuiyInstanceBuffers,
};

/// The Buiy render pass, run as a system in the [`Core2d`] schedule. Per-view
/// data is fetched through [`ViewQuery`] (driven by the render world's
/// `CurrentView`), and the command encoder through the [`RenderContext`] system
/// param. Registered into [`Core2dSystems::EarlyPostProcess`] by `register`.
///
/// The `ViewQuery` tuple mirrors the old `ViewNode::ViewQuery`:
/// - `ViewTarget` — the view's color attachment.
/// - `Option<&BuiyViewPipelines>` — quad/glyph ids specialized to THIS view's
///   attachment format + `Msaa` sample count (`prepare_buiy_view_pipelines`);
///   `Option` because it is absent before the first Prepare touches the view
///   (a skipped draw, not an error).
/// - `Option<&PreparedEffectGroups>` / `Option<&PreparedEffectTargets>` — the
///   per-view effect-compositor carriers (effect-compositor.md § 1.1); `Option`
///   so a view with NO effect groups runs the flat pass byte-for-byte unchanged.
///   The prepare pass attaches them on views that have live `EffectGroup`s.
#[allow(clippy::type_complexity)]
pub fn buiy_pass(
    world: &World,
    view: ViewQuery<(
        &'static ViewTarget,
        Option<&'static BuiyViewPipelines>,
        Option<&'static PreparedEffectGroups>,
        Option<&'static PreparedEffectTargets>,
    )>,
    mut render_context: RenderContext,
) {
    let (view_target, view_pipelines, prepared, prepared_targets) = view.into_inner();
    let pipeline_cache = world.resource::<PipelineCache>();
    let buiy_pipeline = world.resource::<BuiyPipeline>();
    // The view-pass pipelines are the PER-VIEW variants (this view's format
    // + sample count) — never the 1x baseline `BuiyPipeline::id`: a bare
    // `Camera2d` defaults to `Msaa::Sample4`, and a 1x pipeline in its 4x
    // pass fails wgpu validation at `set_pipeline`.
    let Some(view_pipelines) = view_pipelines else {
        return;
    };
    let Some(pipeline) = pipeline_cache.get_render_pipeline(view_pipelines.quad) else {
        return;
    };

    // The persistent per-view buffers maintained by `prepare_buiy_instances`.
    // v1 carrier is the render-world resource shim (see `render::prepare`);
    // it is absent for one warm-up frame before the prepare system inserts
    // it, so a missing resource is a no-op draw, not an error.
    let Some(buffers) = world.get_resource::<BuiyInstanceBuffers>() else {
        return;
    };
    // Nothing to draw this frame (empty extract, or buffers not yet
    // uploaded). Glyphs draw even with zero quads (a pure-text frame) and an
    // outline band draws even with zero quads/glyphs (a focus ring on a
    // transparent focusable — C6-a), so the skip checks ALL THREE counts.
    if buffers.quad_count == 0 && buffers.glyph_count == 0 && buffers.band_count == 0 {
        return;
    }
    // The view uniform is required for any draw (both pipelines bind it at
    // `@group(0)`); it is `None` until the first `write_buffer`.
    let Some(view_binding) = buffers.view_uniform.binding() else {
        return;
    };

    // Effect-group composite — step 1: each group's DIRECT members rasterize
    // into the group's own off-screen `Rgba16Float` target (effect-compositor.md
    // § 3 step 1) — its QUADS, then its GLYPHS (T8: the within-group order
    // mirrors the global shadow < quad < glyph rank, now scoped per region).
    // A nested group's members tag the nested group, so a parent's
    // target receives only its OWN direct members here; the nested child's
    // composited result is blended in at step 2 (post-order, below). Target
    // RESIDENCY (§ 3): the `CachedTexture`s were acquired up-front in
    // `prepare_effect_groups` (the only side with `&mut TextureCache`) and held
    // on `PreparedEffectTargets`, so no child target is recycled before its
    // parent samples it. Both carriers ride the SAME view entity (decided fork
    // 2), so this fires iff prepare attached live groups — never a false-green.
    // Each half gates on its OWN pipeline/buffer readiness (D5) so a pure-text
    // group (empty quad range — a backgroundless Opacity card) still clears +
    // draws its glyphs and composites; an async-compile frame skips that half
    // only (the established behavior class).
    if let (Some(prepared), Some(targets)) = (prepared, prepared_targets) {
        let group_quad_pipeline = prepared
            .quad_pipeline
            .and_then(|id| pipeline_cache.get_render_pipeline(id));
        let group_glyph_pipeline = prepared
            .glyph_pipeline
            .and_then(|id| pipeline_cache.get_render_pipeline(id));
        // The same page-0 atlas bind group the flat glyph draw binds
        // (glyph-pipeline § 11.1 — the multi-page seam is unchanged by T8).
        let atlas_bind_group = world
            .get_resource::<AtlasGpu>()
            .and_then(|a| a.coverage_bind_group());
        for group in &prepared.groups {
            let Some(target) = targets.targets.get(group.index).and_then(|t| t.as_ref()) else {
                // Degraded group (no target): skip the off-screen pass here. A
                // ROOT degraded group is NOT lost — `prepare_effect_groups`
                // folded its `opacity` into its members' alpha and merged its
                // ranges into the flat draw, so the flat WINDOW pass below
                // paints it (effect-compositor.md § 2.3 forward-composite). A
                // NESTED degraded group is still skipped (its correct
                // forward-composite into the parent target is a node-side
                // follow-up; the prepare fold debug-asserts on it).
                continue;
            };
            let placement = &targets.placements[group.index];
            let quad_range = placement.instance_range.clone();
            let glyph_range = placement.glyph_range.clone();
            if quad_range.is_empty() && glyph_range.is_empty() {
                continue; // nothing of EITHER tier to draw (D5).
            }
            // Per-group view uniform: logical px → THIS target's bucketed
            // texel grid, anchored at the painted-bounds min (prepare built
            // the columns). A transient UBO + bind group, valid for this pass.
            let group_view_buf =
                render_context
                    .render_device()
                    .create_buffer_with_data(&BufferInitDescriptor {
                        label: Some("buiy_group_view_uniform"),
                        contents: bytemuck::cast_slice(&placement.target_view_columns),
                        usage: BufferUsages::UNIFORM,
                    });
            let group_view_bg = render_context.render_device().create_bind_group(
                "buiy_group_view_bind_group",
                &buiy_pipeline.view_layout,
                &BindGroupEntries::single(group_view_buf.as_entire_binding()),
            );
            let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("buiy_effect_group_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &target.default_view,
                    // R8b clip + per-target index need bevy 0.18's
                    // `depth_slice`/`resolve_target` defaulted.
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        // Clear transparent so an `opacity < 1` group's
                        // empty texels contribute nothing at composite.
                        load: LoadOp::Clear(LinearRgba::NONE.into()),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &group_view_bg, &[]);
            pass.set_vertex_buffer(0, buiy_pipeline.vertex_buffer.slice(..));
            if !quad_range.is_empty()
                && let Some(pipeline) = group_quad_pipeline
                && let Some(quad_buffer) = buffers.quad.buffer()
            {
                pass.set_render_pipeline(pipeline);
                pass.set_vertex_buffer(1, quad_buffer.slice(..));
                pass.draw(0..4, quad_range);
            }
            // T8: the group's glyphs, into the same target. `@group(0)` stays
            // bound (both pipelines declare the same view layout); the glyph
            // pipeline adds the atlas `@group(1)` exactly like the flat draw.
            if !glyph_range.is_empty()
                && let Some(pipeline) = group_glyph_pipeline
                && let Some(atlas_bg) = atlas_bind_group
                && let Some(glyph_buffer) = buffers.glyph.buffer()
            {
                pass.set_render_pipeline(pipeline);
                pass.set_bind_group(1, atlas_bg, &[]);
                pass.set_vertex_buffer(1, glyph_buffer.slice(..));
                pass.draw(0..4, glyph_range);
            }
        }
    }

    // Effect-group composite — step 2a (NESTED groups, before the window
    // pass): composite each non-root group's target into its PARENT group's
    // target, in post-order (children before parents, so a parent's target
    // holds every nested child's composited result before it composites into
    // the window in step 2b). Root groups (`parent == None`) composite into
    // the window inside the `buiy_pass` below. A separate pass per nested
    // composite because each targets a different attachment (the parent's
    // `Rgba16Float` target). The single-group test has no nested groups, so
    // this loop is a no-op there; it is the seam nesting slots into.
    if let (Some(prepared), Some(targets)) = (prepared, prepared_targets) {
        let composite = world.resource::<CompositePipeline>();
        for &gi in &prepared.composite_order {
            let group = &prepared.groups[gi];
            let Some(parent_idx) = group.parent else {
                continue; // root group → composited into the window below.
            };
            let (Some(src), Some(parent_tex)) = (
                targets.targets.get(gi).and_then(|t| t.as_ref()),
                targets.targets.get(parent_idx).and_then(|t| t.as_ref()),
            ) else {
                continue; // a degraded group on either end: skip (no target).
            };
            let placement = &targets.placements[gi];
            let Some(comp_id) = placement.composite_pipeline else {
                continue;
            };
            let Some(comp_pipeline) = pipeline_cache.get_render_pipeline(comp_id) else {
                continue;
            };
            // Build the composite bind groups BEFORE the pass (they need the
            // device, which the open pass borrows); then composite into the
            // PARENT group's target (LoadOp::Load preserves its step-1 content).
            let (uniform_bg, source_bg) =
                composite_bindings(&mut render_context, composite, src, placement);
            let mut cpass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("buiy_effect_composite_nested_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &parent_tex.default_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            cpass.set_render_pipeline(comp_pipeline);
            cpass.set_bind_group(0, &uniform_bg, &[]);
            cpass.set_bind_group(1, &source_bg, &[]);
            cpass.set_vertex_buffer(0, composite.vertex_buffer.slice(..));
            cpass.draw(0..4, 0..1);
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
        multiview_mask: None,
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
        // Effect-group double-paint exclusion (effect-compositor § 3 / decided
        // fork 3): draw ONLY the non-group instance ranges. A group member is
        // rasterized into its own off-screen target (step 1 above) and
        // composited back into the window (step 2 below), so drawing it flat
        // here too would double-paint it (over-bright overlap). `flat_ranges`
        // is the complement of `group_ranges`: when NO group is live it is the
        // single full `0..quad_count` run (so the flat path is byte-for-byte the
        // pre-compositor draw); when EVERY instance is a group member it is
        // empty, and the flat draw is correctly a no-op (the composite paints
        // the content). It is never wrong to iterate it — an empty `flat_ranges`
        // means "nothing to draw flat", NOT "draw everything".
        for r in &buffers.flat_ranges {
            pass.draw(0..4, r.clone());
        }
    }

    // --- Glyph draw (paint order: glyph after quad) ----------------------
    // The coverage-glyph (alpha-as-color) primitive, drawn AFTER the quad so
    // text paints over fills (shadow < quad < glyph < path). Requires: the
    // glyph pipeline compiled, the atlas `@group(1)` bind group built by
    // `prepare_atlas_textures` (a coverage page exists), and a non-empty
    // uploaded glyph buffer. Any missing piece skips the glyph draw without
    // disturbing the quad draw above (e.g. before the pipeline async-compiles
    // or before the first glyph warms an atlas page).
    //
    // Effect-group double-paint exclusion, glyph tier (T8 — the quad
    // precedent above, verbatim semantics): draw ONLY the non-group glyph
    // ranges. A group member's glyphs rasterized into its off-screen target
    // in step 1 and composite back in step 2. `glyph_flat_ranges` is the
    // complement of `glyph_group_ranges`: with no live group it is the
    // single full `0..glyph_count` run (byte-for-byte the pre-T8 draw);
    // when every glyph is a group member it is empty and this loop is
    // correctly a no-op.
    if buffers.glyph_count > 0
        && let Some(glyph_pipeline) = pipeline_cache.get_render_pipeline(view_pipelines.glyph)
        && let Some(atlas_gpu) = world.get_resource::<AtlasGpu>()
        && let Some(atlas_bind_group) = atlas_gpu.coverage_bind_group()
        && let Some(glyph_buffer) = buffers.glyph.buffer()
    {
        pass.set_render_pipeline(glyph_pipeline);
        // `@group(0)` (view) is already bound for the pass; add the atlas
        // `@group(1)` (texture + sampler) the coverage shader samples.
        pass.set_bind_group(1, atlas_bind_group, &[]);
        pass.set_vertex_buffer(1, glyph_buffer.slice(..));
        for r in &buffers.glyph_flat_ranges {
            pass.draw(0..4, r.clone());
        }
    }

    // --- Border/outline band draw (paint order: outline ON TOP) ----------
    // C6-a: the focus-ring / selection-outline band, drawn AFTER the quad +
    // glyph so the ring sits over the fill and text within the box. Uses the
    // distinct `BorderBandInstance` blob + `band.wgsl` pipeline (the byte-stable
    // 68 B quad stride is untouched). Binds only the shared `@group(0)` view
    // uniform (no atlas `@group(1)`). The outline's clip is the entity's
    // `AncestorClip` (packed at extract), so a ring outside an `overflow:hidden`
    // ancestor still paints (styling-f-tier.md § 2.4). A zero-count or
    // not-yet-uploaded band buffer simply skips this draw.
    if buffers.band_count > 0
        && let Some(band_pipeline) = pipeline_cache.get_render_pipeline(view_pipelines.band)
        && let Some(band_buffer) = buffers.band.buffer()
    {
        pass.set_render_pipeline(band_pipeline);
        // `@group(0)` (view) stays bound for the whole pass; the band binds no
        // additional group. The static unit-quad VBO 0 also stays bound.
        pass.set_vertex_buffer(1, band_buffer.slice(..));
        pass.draw(0..4, 0..buffers.band_count);
    }

    // End the flat window pass before the root-group composites: a composite
    // is a SEPARATE pass into the same window attachment (LoadOp::Load), so it
    // must not overlap the borrow of `pass`.
    drop(pass);

    // Effect-group composite — step 2b (ROOT groups → window): composite each
    // root group's target into the window, in post-order, AFTER the flat draw
    // (the group paints over the in-flow content). The composite samples the
    // group target (`Rgba16Float`, straight-alpha linear) and blends SrcOver
    // with `sampled.a * opacity` in the WINDOW's space (the `Rgba8UnormSrgb`
    // attachment re-encodes linear→sRGB8 on write) — the GPU form of
    // `composite_src_over` (compositor.rs). A nested child's result is already
    // in the parent target (step 2a), so overlapping children inside an
    // `opacity < 1` group composite ONCE as a unit and do not double-darken
    // (the correct semantics, § 4). The targets stay resident through here;
    // `update_texture_cache_system` (render `Cleanup`) un-`taken`s them next
    // frame (§ 2.2).
    if let (Some(prepared), Some(targets)) = (prepared, prepared_targets) {
        let composite = world.resource::<CompositePipeline>();
        for &gi in &prepared.composite_order {
            let group = &prepared.groups[gi];
            if group.parent.is_some() {
                continue; // nested → composited into its parent (step 2a).
            }
            let Some(src) = targets.targets.get(gi).and_then(|t| t.as_ref()) else {
                continue; // degraded root group (no target).
            };
            let placement = &targets.placements[gi];
            let Some(comp_id) = placement.composite_pipeline else {
                continue;
            };
            let Some(comp_pipeline) = pipeline_cache.get_render_pipeline(comp_id) else {
                continue;
            };
            // Bind groups before the pass (device borrow); then composite into
            // the window attachment (LoadOp::Load preserves the flat draw).
            let (uniform_bg, source_bg) =
                composite_bindings(&mut render_context, composite, src, placement);
            let attachment = view_target.get_color_attachment();
            let mut cpass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("buiy_effect_composite_window_pass"),
                color_attachments: &[Some(attachment)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            cpass.set_render_pipeline(comp_pipeline);
            cpass.set_bind_group(0, &uniform_bg, &[]);
            cpass.set_bind_group(1, &source_bg, &[]);
            cpass.set_vertex_buffer(0, composite.vertex_buffer.slice(..));
            cpass.draw(0..4, 0..1);
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
}

/// Build the composite pass's two bind groups (effect-compositor.md § 3 step 2)
/// for one group: `@group(0)` the `Composite` uniform (parent transform + the
/// quad's logical bounds + uv_max + opacity) and `@group(1)` the source target
/// texture + the composite sampler. Created BEFORE the render pass begins (the
/// open pass borrows the device); the transient uniform buffer's bytes are owned
/// by the returned `BindGroup`, so it may drop immediately.
fn composite_bindings(
    render_context: &mut RenderContext,
    composite: &CompositePipeline,
    src: &CachedTexture,
    placement: &super::compositor::GroupPlacement,
) -> (
    bevy::render::render_resource::BindGroup,
    bevy::render::render_resource::BindGroup,
) {
    // The WGSL `Composite` uniform = col0, col1, bounds(min.xy,max.zw),
    // [uv_max.x, uv_max.y, opacity, 0]. 4 × vec4 = 64 B (byte-identical to the
    // `composite.wgsl` struct).
    let b = placement.composite_bounds;
    let uniform: [Vec4; 4] = [
        placement.composite_view_columns[0],
        placement.composite_view_columns[1],
        Vec4::new(b.min.x, b.min.y, b.max.x, b.max.y),
        Vec4::new(
            placement.uv_max.x,
            placement.uv_max.y,
            placement.opacity,
            0.0,
        ),
    ];
    let buf = render_context
        .render_device()
        .create_buffer_with_data(&BufferInitDescriptor {
            label: Some("buiy_composite_uniform"),
            contents: bytemuck::cast_slice(&uniform),
            usage: BufferUsages::UNIFORM,
        });
    let uniform_bg = render_context.render_device().create_bind_group(
        "buiy_composite_uniform_bind_group",
        &composite.uniform_layout,
        &BindGroupEntries::single(buf.as_entire_binding()),
    );
    let source_bg = render_context.render_device().create_bind_group(
        "buiy_composite_source_bind_group",
        &composite.source_layout,
        &BindGroupEntries::sequential((&src.default_view, &composite.sampler)),
    );
    (uniform_bg, source_bg)
}

pub(crate) fn register(render_app: &mut SubApp) {
    // Pin the Buiy pass inside the Core2d post-processing window, AFTER the main
    // 2D pass (`Core2dSystems::MainPass`) and BEFORE tonemapping (which runs in
    // `Core2dSystems::PostProcess`). `EarlyPostProcess` is chained between those
    // two sets (bevy `core_2d/schedule.rs`: `(Prepass, MainPass,
    // EarlyPostProcess, PostProcess).chain()`), so it is the verified 0.19
    // successor to the removed 0.18 `StartMainPassPostProcessing → Buiy →
    // Tonemapping` graph slot (architecture.md § 1.3). Widget paint thus lands
    // in the post-processing window and participates in tonemapping on the
    // opt-in HDR path. A single system: its internal multi-pass structure (group
    // passes → nested composites → flat window pass → root composites) is
    // straight-line code sharing one `RenderContext` encoder, so no inter-system
    // ordering is needed to preserve it (see the module-level note). Optional
    // bloom is a separate concern; a 2D-bloom plugin orders itself relative to
    // these sets.
    render_app.add_systems(Core2d, buiy_pass.in_set(Core2dSystems::EarlyPostProcess));
}
