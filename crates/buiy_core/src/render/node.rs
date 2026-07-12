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
        BindGroupEntries, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, LoadOp,
        Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    },
    renderer::{RenderContext, ViewQuery},
    texture::CachedTexture,
    view::ViewTarget,
};

use super::{
    atlas::AtlasGpu,
    blur::{BlurParams, BlurPipeline, PreparedBackdropBlur, PreparedBackdropBlurs},
    buckets::{FlatDrawStep, interleave_flat_draw},
    composite::CompositePipeline,
    compositor::{EffectReason, PreparedEffectGroups, PreparedEffectTargets},
    pipeline::{BuiyPipeline, BuiyViewPipelines},
    prepare::BuiyInstanceBuffers,
    raster::{RasterBuffers, RasterDraw, build_raster_draws},
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
        Option<&'static PreparedBackdropBlurs>,
    )>,
    mut render_context: RenderContext,
) {
    let (view_target, view_pipelines, prepared, prepared_targets, prepared_blurs) =
        view.into_inner();
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
    // uploaded). Glyphs draw even with zero quads (a pure-text frame), a band
    // draws even with zero quads/glyphs (a focus ring on a transparent
    // focusable — C6-a), a box-shadow draws even with zero of the rest (a
    // shadow-only frame — C6-b), a vector ICON draws even with zero of the
    // rest (parity Wave B3 — an icon-only box, e.g. a bare rail glyph), and a
    // RASTER canvas draws even with zero of the rest (a bare drawing surface),
    // so the skip checks ALL the primitive counts.
    let raster_present = world
        .get_resource::<RasterBuffers>()
        .is_some_and(|b| b.count > 0);
    if buffers.quad_count == 0
        && buffers.glyph_count == 0
        && buffers.icon_count == 0
        && buffers.band_count == 0
        && buffers.shadow_count == 0
        && buffers.gradient_count == 0
        && !raster_present
    {
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
            // Parity Wave B4: a pure backdrop-filter group is NOT an off-screen
            // group — it is an in-place blur of the painted window backdrop
            // (`run_backdrop_blurs` below), and its OWN fill draws flat over the
            // blur. Skip it here (and in the step-2a/2b composite loops); its
            // off-screen target was never acquired (`prepare_backdrop_blurs`
            // owns its scratch instead).
            if is_pure_backdrop_filter(group.reason) {
                continue;
            }
            let Some(target) = targets.targets.get(group.index).and_then(|t| t.as_ref()) else {
                // Degraded group (no target): skip the off-screen pass here — it is
                // NOT lost. `fold_degraded_groups` folded its `opacity` into its
                // members' alpha; a ROOT group also merged its ranges into the flat
                // draw, so the flat WINDOW pass below paints it, and a NESTED group
                // is injected into its parent's target at step-2a for case A (or
                // skipped for the deferred chain) — effect-compositor.md § 2.3.
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
        // Re-fetch the Rgba16Float group pipelines + page-0 atlas bind group for
        // the case-A injection below. Step 1 scopes its own copies locally and this
        // is a separate `if let` block, so they are refetched here (same pipelines
        // step 1 draws groups with). `None` until compiled (async) — the injection
        // mirrors step 1's per-half readiness skip.
        let group_quad_pipeline = prepared
            .quad_pipeline
            .and_then(|id| pipeline_cache.get_render_pipeline(id));
        let group_glyph_pipeline = prepared
            .glyph_pipeline
            .and_then(|id| pipeline_cache.get_render_pipeline(id));
        let atlas_bind_group = world
            .get_resource::<AtlasGpu>()
            .and_then(|a| a.coverage_bind_group());
        for &gi in &prepared.composite_order {
            let group = &prepared.groups[gi];
            if is_pure_backdrop_filter(group.reason) {
                continue; // backdrop-filter: no off-screen target to composite.
            }
            let Some(parent_idx) = group.parent else {
                continue; // root group → composited into the window below.
            };
            // The parent MUST have a target for either path. If the parent is
            // itself degraded, this is a deferred case — `(Some,None)`
            // kept-child-under-degraded-parent or `(None,None)` chain — skipped as
            // today (the child vanishes; no worse than before). See the
            // nested-degraded-forward-composite design § Deferred.
            let Some(parent_tex) = targets.targets.get(parent_idx).and_then(|t| t.as_ref()) else {
                continue;
            };
            let Some(src) = targets.targets.get(gi).and_then(|t| t.as_ref()) else {
                // CASE A (effect-compositor.md § 2.3): the child DEGRADED (no
                // target) but the parent kept one. Forward-composite the child's
                // already-folded members DIRECTLY into the parent's `Rgba16Float`
                // target — the same draw as step 1's group pass, but `LoadOp::Load`
                // into the PARENT target using the PARENT's view columns (the
                // child's members sit at logical positions inside the parent's
                // bounds, so the parent view places them at exactly the position
                // the normal composite would). `fold_degraded_groups` folded the
                // child's opacity into the buffer, so there is no per-draw opacity.
                // Post-order runs this before the parent composites upward, so the
                // child rides along in the parent's composite.
                let child_placement = &targets.placements[gi];
                let parent_placement = &targets.placements[parent_idx];
                let quad_range = child_placement.instance_range.clone();
                let glyph_range = child_placement.glyph_range.clone();
                if quad_range.is_empty() && glyph_range.is_empty() {
                    continue; // nothing of either tier to inject.
                }
                let group_view_buf =
                    render_context
                        .render_device()
                        .create_buffer_with_data(&BufferInitDescriptor {
                            label: Some("buiy_degraded_inject_view_uniform"),
                            contents: bytemuck::cast_slice(&parent_placement.target_view_columns),
                            usage: BufferUsages::UNIFORM,
                        });
                let group_view_bg = render_context.render_device().create_bind_group(
                    "buiy_degraded_inject_view_bind_group",
                    &buiy_pipeline.view_layout,
                    &BindGroupEntries::single(group_view_buf.as_entire_binding()),
                );
                let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                    label: Some("buiy_degraded_inject_pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &parent_tex.default_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: Operations {
                            // Preserve the parent's step-1 content — inject on top.
                            load: LoadOp::Load,
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
                continue;
            };
            // `(Some, Some)`: both kept a target — the existing nested composite.
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

    // Raster (drawing-canvas) `@group(1)` bind groups, built BEFORE the window
    // pass opens (bind-group creation needs the device, which the open pass
    // borrows — the `composite_bindings` precedent). One per raster node whose
    // `GpuImage` is resident; empty when there are no raster nodes (the common
    // case) or none have uploaded yet.
    let raster_draws = build_raster_draws(world, &mut render_context);

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

    // --- Box-shadow draw (paint order: shadow FIRST, behind the quad) ----
    // C6-b: the box-shadow primitive, drawn BEFORE the quad so a shadow paints
    // behind its caster (shadow < quad < glyph < path). It reuses the 68 B quad
    // instance layout (radius slot → blur sigma) but its OWN pipeline
    // (`shadow.wgsl`); the shadow buffer is the distinct `shadow` RawBufferVec.
    // Binds only the shared `@group(0)` view uniform (no atlas `@group(1)`).
    // v1 draws the whole shadow blob flat (no effect-group partition — § 2.2).
    // A zero-count or not-yet-uploaded / not-yet-compiled shadow buffer simply
    // skips this draw without disturbing the quad draw below.
    if buffers.shadow_count > 0
        && let Some(shadow_pipeline) = pipeline_cache.get_render_pipeline(view_pipelines.shadow)
        && let Some(shadow_buffer) = buffers.shadow.buffer()
    {
        pass.set_render_pipeline(shadow_pipeline);
        // `@group(0)` (view) stays bound for the whole pass; the static unit-quad
        // VBO 0 also stays bound. The shadow instance buffer is VBO 1.
        pass.set_vertex_buffer(1, shadow_buffer.slice(..));
        pass.draw(0..4, 0..buffers.shadow_count);
    }

    // --- Rounded box-shadow draw (F4b-6, SHADOW tier — behind the quad) --
    // The rounded caster's shadow: its OWN `RoundedShadowInstance` buffer +
    // `rounded_shadow.wgsl` pipeline (carrying a corner radius the square 68 B
    // layout can't hold — the crisp 3D-press edge). Drawn in the SHADOW tier just
    // after the square shadow blob (both behind all flat content; a rounded and a
    // square shadow rarely overlap, and both being behind the quads makes their
    // relative order visually inert). The square path stays byte-stable; a scene
    // with no rounded shadow uploads a zero-count buffer and skips this draw.
    if buffers.rounded_shadow_count > 0
        && let Some(rs_pipeline) = pipeline_cache.get_render_pipeline(view_pipelines.rounded_shadow)
        && let Some(rs_buffer) = buffers.rounded_shadow.buffer()
    {
        pass.set_render_pipeline(rs_pipeline);
        pass.set_vertex_buffer(1, rs_buffer.slice(..));
        pass.draw(0..4, 0..buffers.rounded_shadow_count);
    }

    // --- Quad + background-gradient draw, INTERLEAVED in paint order -----
    // Paint order shadow < quad < gradient < glyph, with the gradient tier
    // INTERLEAVED per node: each node's background gradient paints right after
    // that node's OWN quad and BEFORE any descendant's quad, so an ANCESTOR's
    // gradient layer (e.g. the viewport dotted-grid `RadialGradient`) never
    // overpaints a descendant card's opaque fill. The pre-fix pass drew the
    // WHOLE quad blob (the `flat_ranges`) then the WHOLE gradient blob (one
    // `draw(0..gradient_count)`), so every gradient painted over every quad — an
    // ancestor gradient bled onto its descendants (parity gradient-bleed bug).
    //
    // `gradient_anchors[i]` is the quad-blob index just after gradient i's node's
    // own quad; gradients are in node-walk (paint) order so anchors ascend, and
    // `interleave_flat_draw` walks the quad `flat_ranges` (the
    // effect-group double-paint complement of `group_ranges` — a group member is
    // rasterized off-screen in step 1 and composited back in step 2, never drawn
    // flat) pausing to emit each gradient at its anchor. Gradients still ride the
    // flat draw ONLY (no effect-group partition; a gradient on a grouped element
    // stays the documented v1 follow-up). A gradient still paints OVER its own
    // node's solid fill (its anchor is AFTER that quad) and UNDER its own
    // text/icons (the glyph/icon tiers draw in their own later passes below).
    // When no group is live, `flat_ranges` is the single full `0..quad_count` run
    // and the only re-sequencing is the gradient interleave; an empty
    // `gradient_anchors` yields exactly the pre-fix flat quad draw.
    //
    // Effect-group safety: this ONLY re-sequences the flat-window draws that
    // already happened here — the same `flat_ranges` quads + the same gradient
    // blob. The off-screen group passes (above) and the backdrop-blur + root/
    // nested composites (below, after `drop(pass)`) are untouched.
    let quad_buffer = (buffers.quad_count > 0)
        .then(|| buffers.quad.buffer())
        .flatten();
    let gradient_ready = buffers.gradient_count > 0;
    let gradient_pipeline = gradient_ready
        .then(|| pipeline_cache.get_render_pipeline(view_pipelines.gradient))
        .flatten();
    let gradient_buffer = gradient_ready.then(|| buffers.gradient.buffer()).flatten();
    // Gradients only ride the schedule when their pipeline + buffer are ready;
    // otherwise the schedule is the plain flat quad runs (this frame paints no
    // gradient, exactly like the pre-fix async-compile / not-yet-uploaded skip).
    let anchors: &[u32] = if gradient_pipeline.is_some() && gradient_buffer.is_some() {
        &buffers.gradient_anchors
    } else {
        &[]
    };

    // --- Raster (drawing-canvas) tier, INTERLEAVED per-raster in paint order ---
    // F4a: retire the fill-tier "raster draws after ALL quads" block. Each
    // `RasterImage` node now splices at its OWN `node_quad_anchor` (the exact
    // gradient-bleed precedent), so a canvas paints over every quad/gradient that
    // precedes its node in paint order and UNDER every one that follows — its true
    // stacking position (a non-top-layer overlay paints over it; an OPAQUE
    // top-layer modal panel that contains a raster shows it). Resolve the raster
    // resources up front; if the pipeline/buffer are not yet ready (async compile
    // / not-yet-uploaded) treat as NO rasters this frame — the interleave gets
    // empty raster anchors and reproduces the byte-stable gradient-only draw, and
    // the rasters land once the resources warm. Each raster is still one bind
    // group (`@group(1)` = its own image + a Nearest sampler) + one instanced
    // `draw`; rasters draw under glyphs (a later pass), so text paints over a
    // canvas.
    let raster_ready = (!raster_draws.is_empty())
        .then(|| world.get_resource::<RasterBuffers>())
        .flatten()
        .filter(|rb| rb.count > 0)
        .and_then(|rb| {
            let pl = pipeline_cache.get_render_pipeline(view_pipelines.raster)?;
            let buf = rb.instances.buffer()?;
            Some((pl, buf))
        });
    // Sort the draws by their node's paint-order anchor so the interleave sweep is
    // monotonic (a stable sort keeps a deterministic order among rasters sharing
    // an anchor). Empty anchors when the resources are not ready — the interleave
    // then emits no `Raster` step (byte-identical to the pre-raster draw).
    let mut sorted_rasters: Vec<&RasterDraw> = raster_draws.iter().collect();
    let raster_anchors: Vec<u32> = if raster_ready.is_some() {
        sorted_rasters.sort_by_key(|d| d.anchor);
        sorted_rasters.iter().map(|d| d.anchor).collect()
    } else {
        sorted_rasters.clear();
        Vec::new()
    };

    for step in interleave_flat_draw(&buffers.flat_ranges, anchors, &raster_anchors) {
        match step {
            FlatDrawStep::Quads(r) => {
                if let Some(qb) = quad_buffer {
                    pass.set_render_pipeline(pipeline);
                    pass.set_vertex_buffer(1, qb.slice(..));
                    pass.draw(0..4, r);
                }
            }
            FlatDrawStep::Gradients(r) => {
                if let (Some(gp), Some(gb)) = (gradient_pipeline, gradient_buffer) {
                    // `@group(0)` (view) + the static unit-quad VBO 0 stay bound;
                    // the gradient instance buffer is VBO 1.
                    pass.set_render_pipeline(gp);
                    pass.set_vertex_buffer(1, gb.slice(..));
                    pass.draw(0..4, r);
                }
            }
            FlatDrawStep::Raster(k) => {
                if let Some((raster_pipeline, raster_buffer)) = raster_ready {
                    // `@group(0)` (view) + the static unit-quad VBO 0 stay bound;
                    // the raster instance buffer is VBO 1, `@group(1)` the per-node
                    // image. Re-bound per raster step (few rasters; a quad/gradient
                    // step between two rasters rebinds VBO 1 + the pipeline anyway).
                    let draw = sorted_rasters[k as usize];
                    pass.set_render_pipeline(raster_pipeline);
                    pass.set_vertex_buffer(1, raster_buffer.slice(..));
                    pass.set_bind_group(1, &draw.bind_group, &[]);
                    pass.draw(0..4, draw.instance..draw.instance + 1);
                }
            }
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

    // --- Vector-icon draw (parity Wave B3) -------------------------------
    // Icons ARE coverage stamps (an icon instance is a `GlyphAlphaInstance`), so
    // they reuse the EXACT glyph pipeline + the EXACT atlas `@group(1)` coverage
    // bind group + `coverage.wgsl` — NO new GPU code (§ 3.5). A SEPARATE buffer +
    // draw (not appended to the glyph buffer) keeps the icon producer decoupled
    // from the wholesale-rebuilt glyph carrier. Drawn right after the glyph draw
    // (both coverage tier, so an icon paints over fills like text). Same
    // effect-group flat/group partition as glyphs (`icon_flat_ranges`); any
    // missing piece skips without disturbing the glyph draw above.
    if buffers.icon_count > 0
        && let Some(glyph_pipeline) = pipeline_cache.get_render_pipeline(view_pipelines.glyph)
        && let Some(atlas_gpu) = world.get_resource::<AtlasGpu>()
        && let Some(atlas_bind_group) = atlas_gpu.coverage_bind_group()
        && let Some(icon_buffer) = buffers.icon.buffer()
    {
        pass.set_render_pipeline(glyph_pipeline);
        pass.set_bind_group(1, atlas_bind_group, &[]);
        pass.set_vertex_buffer(1, icon_buffer.slice(..));
        for r in &buffers.icon_flat_ranges {
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

    // --- Backdrop-blur (parity Wave B4) ----------------------------------
    // Now that the flat window pass painted the BACKDROP (every non-group
    // primitive — the dotted bg, scrolled content, etc.), run the in-place
    // dual-Kawase blur for each backdrop-filter element, then draw the element's
    // OWN fill over the blurred backdrop. This sits between the flat draw and the
    // root-group composites so a modal scrim blurs the content behind it. See
    // `render/blur.rs` for why this is NOT an off-screen effect group.
    if let Some(pb) = prepared_blurs {
        run_backdrop_blurs(
            world,
            view_target,
            &pb.blurs,
            pb.down_pipeline,
            pb.up_pipeline,
            pb.blit_pipeline,
            pipeline_cache,
            &mut render_context,
        );
    }
    if let (Some(prepared), Some(blurs)) = (prepared, prepared_blurs)
        && !blurs.blurs.is_empty()
    {
        // The backdrop-filter elements' OWN fills draw flat over the blurred
        // backdrop (their ranges live in `group_ranges`/`glyph_group_ranges`,
        // EXCLUDED from `flat_ranges`, because they are EffectGroup members; the
        // off-screen loops skip them — `is_pure_backdrop_filter` — so this is
        // where they paint). A LoadOp::Load window pass preserves the blur.
        draw_backdrop_filter_fills(
            world,
            view_target,
            prepared,
            buffers,
            view_pipelines,
            buiy_pipeline,
            pipeline_cache,
            &view_bind_group,
            &mut render_context,
        );
    }

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
            if is_pure_backdrop_filter(group.reason) {
                continue; // backdrop-filter: no off-screen target to composite.
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

/// Does this effect group form ONLY for `backdrop-filter` (no opacity /
/// isolation / filter / blend bit)? Such a group is handled by the in-place
/// backdrop-blur path (`run_backdrop_blurs`), NOT the off-screen-target
/// compositor, so the step-1/2a/2b loops skip it. A group that combines
/// backdrop-filter WITH another former (e.g. a translucent modal that is also a
/// blur) is a documented follow-up — it takes the off-screen path here (its own
/// fill renders into a target), so its backdrop is NOT blurred in v1.
fn is_pure_backdrop_filter(reason: EffectReason) -> bool {
    reason == EffectReason::BACKDROP_FILTER
}

/// Execute the in-place dual-Kawase backdrop blur for every prepared
/// backdrop-filter element (parity Wave B4). For each blur: a DOWN pyramid
/// (window element-region → scratch[0] → … → scratch[N-1]), an UP pyramid
/// (scratch[N-1] → … → scratch[0]), then a final UP blit of scratch[0] back over
/// the element's window region (`LoadOp::Load` preserves the rest of the window).
/// Each pass is its own `begin_tracked_render_pass` on the shared encoder; the
/// read (sample) and write (attachment) target DIFFERENT textures every pass, so
/// there is never a read-write hazard on one texture. A no-op when there are no
/// blurs, the pipelines have not async-compiled, or the `BlurPipeline` resource
/// is absent.
#[allow(clippy::too_many_arguments)]
fn run_backdrop_blurs(
    world: &World,
    view_target: &ViewTarget,
    blurs: &[PreparedBackdropBlur],
    down_pipeline: Option<CachedRenderPipelineId>,
    up_pipeline: Option<CachedRenderPipelineId>,
    blit_pipeline: Option<CachedRenderPipelineId>,
    pipeline_cache: &PipelineCache,
    render_context: &mut RenderContext,
) {
    if blurs.is_empty() {
        return;
    }
    let Some(blur_pipeline) = world.get_resource::<BlurPipeline>() else {
        return;
    };
    // All three pipeline variants must have compiled (the established skip-on-
    // async-compile behavior class — a not-yet-ready frame leaves the backdrop
    // un-blurred, then resolves once the pipelines land). The ids ride the
    // per-view `PreparedBackdropBlurs`; the per-block caller (top-layer stacking
    // composite, § 3.3) passes a filtered `blurs` slice but the SAME ids.
    let (Some(down_id), Some(up_id), Some(blit_id)) = (down_pipeline, up_pipeline, blit_pipeline)
    else {
        return;
    };
    let (Some(down_pl), Some(up_pl), Some(blit_pl)) = (
        pipeline_cache.get_render_pipeline(down_id),
        pipeline_cache.get_render_pipeline(up_id),
        pipeline_cache.get_render_pipeline(blit_id),
    ) else {
        return;
    };

    for blur in blurs {
        if blur.levels.is_empty() {
            continue;
        }
        let n = blur.levels.len();

        // Helper: run ONE blur pass — bind `@group(0)` params + `@group(1)` source
        // (texture + the shared linear sampler), draw the unit quad into `dst`,
        // clearing it (each pyramid level is fully overwritten).
        // The closure is inlined per pass below (it borrows the encoder mutably).

        // 1/source_size for a pass whose source is scratch level `lvl`.
        let level_texel = |lvl: usize| -> [f32; 2] {
            let e = blur.level_extents[lvl]
                .max(bevy::math::UVec2::ONE)
                .as_vec2();
            [1.0 / e.x, 1.0 / e.y]
        };

        // --- DOWN pyramid: window region → scratch[0]; scratch[i-1] → scratch[i].
        for i in 0..n {
            let dst = &blur.levels[i];
            // Source = the WINDOW main texture for level 0 (read the element
            // sub-rect), else the previous scratch level (read full).
            let (src_view, texel, src_rect) = if i == 0 {
                // The window source's texel pitch is `1/window_physical` (the
                // sub-rect read does not change the pitch).
                let w = blur.window_physical.max(bevy::math::UVec2::ONE).as_vec2();
                (
                    view_target.main_texture_view(),
                    [1.0 / w.x, 1.0 / w.y],
                    [
                        blur.src_uv_min.x,
                        blur.src_uv_min.y,
                        blur.src_uv_max.x,
                        blur.src_uv_max.y,
                    ],
                )
            } else {
                (
                    &blur.levels[i - 1].default_view,
                    level_texel(i - 1),
                    [0.0, 0.0, 1.0, 1.0],
                )
            };
            let params = BlurParams {
                texel_and_offset: [texel[0], texel[1], blur.offset, 0.0],
                src_rect,
            };
            blur_one_pass(
                render_context,
                blur_pipeline,
                down_pl,
                src_view,
                &dst.default_view,
                &params,
            );
        }

        // --- UP pyramid: scratch[i+1] → scratch[i] for i = n-2 .. 0.
        for i in (0..n.saturating_sub(1)).rev() {
            let texel = level_texel(i + 1);
            let params = BlurParams {
                texel_and_offset: [texel[0], texel[1], blur.offset, 0.0],
                src_rect: [0.0, 0.0, 1.0, 1.0],
            };
            blur_one_pass(
                render_context,
                blur_pipeline,
                up_pl,
                &blur.levels[i + 1].default_view,
                &blur.levels[i].default_view,
                &params,
            );
        }

        // --- Final blit-back: scratch[0] → the WINDOW element region. A
        // viewport-scissored UP pass writes the blurred backdrop over `region`
        // only; `LoadOp::Load` preserves the rest of the window.
        let texel = level_texel(0);
        let params = BlurParams {
            texel_and_offset: [texel[0], texel[1], blur.offset, 0.0],
            src_rect: [0.0, 0.0, 1.0, 1.0],
        };
        blur_blit_to_window(
            render_context,
            blur_pipeline,
            blit_pl,
            &blur.levels[0].default_view,
            view_target,
            blur.region,
            &params,
        );
    }
}

/// One off-screen blur pass: `@group(0)` params + `@group(1)` (source view +
/// the shared linear sampler), draw the unit quad into `dst`, clearing it.
fn blur_one_pass(
    render_context: &mut RenderContext,
    blur_pipeline: &BlurPipeline,
    pipeline: &bevy::render::render_resource::RenderPipeline,
    src_view: &bevy::render::render_resource::TextureView,
    dst_view: &bevy::render::render_resource::TextureView,
    params: &BlurParams,
) {
    let (params_bg, source_bg) = blur_bindings(render_context, blur_pipeline, src_view, params);
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("buiy_backdrop_blur_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: dst_view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(LinearRgba::NONE.into()),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &params_bg, &[]);
    pass.set_bind_group(1, &source_bg, &[]);
    pass.set_vertex_buffer(0, blur_pipeline.vertex_buffer.slice(..));
    pass.draw(0..4, 0..1);
}

/// The final blit-back: an UP pass that writes the blurred scratch[0] over the
/// element's `region` in the WINDOW (`LoadOp::Load` + a viewport scissor so only
/// the element region is overwritten). The destination is the view's current
/// color attachment.
fn blur_blit_to_window(
    render_context: &mut RenderContext,
    blur_pipeline: &BlurPipeline,
    pipeline: &bevy::render::render_resource::RenderPipeline,
    src_view: &bevy::render::render_resource::TextureView,
    view_target: &ViewTarget,
    region: bevy::math::URect,
    params: &BlurParams,
) {
    let (params_bg, source_bg) = blur_bindings(render_context, blur_pipeline, src_view, params);
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("buiy_backdrop_blur_blit_pass"),
        color_attachments: &[Some(view_target.get_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    // Scissor to the element region (physical px) so the full-target unit quad
    // writes ONLY the backdrop region — the rest of the window is untouched
    // (the quad's NDC covers the whole view, the scissor clips it to `region`).
    pass.set_scissor_rect(region.min.x, region.min.y, region.width(), region.height());
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &params_bg, &[]);
    pass.set_bind_group(1, &source_bg, &[]);
    pass.set_vertex_buffer(0, blur_pipeline.vertex_buffer.slice(..));
    pass.draw(0..4, 0..1);
}

/// Build a blur pass's two bind groups: `@group(0)` the `BlurParams` uniform and
/// `@group(1)` (source view + the shared linear sampler). Created BEFORE the pass
/// (the open pass borrows the device); the transient uniform buffer's bytes are
/// owned by the returned `BindGroup`.
fn blur_bindings(
    render_context: &mut RenderContext,
    blur_pipeline: &BlurPipeline,
    src_view: &bevy::render::render_resource::TextureView,
    params: &BlurParams,
) -> (
    bevy::render::render_resource::BindGroup,
    bevy::render::render_resource::BindGroup,
) {
    let buf = render_context
        .render_device()
        .create_buffer_with_data(&BufferInitDescriptor {
            label: Some("buiy_blur_params_uniform"),
            contents: bytemuck::bytes_of(params),
            usage: BufferUsages::UNIFORM,
        });
    let params_bg = render_context.render_device().create_bind_group(
        "buiy_blur_params_bind_group",
        &blur_pipeline.uniform_layout,
        &BindGroupEntries::single(buf.as_entire_binding()),
    );
    let source_bg = render_context.render_device().create_bind_group(
        "buiy_blur_source_bind_group",
        &blur_pipeline.source_layout,
        &BindGroupEntries::sequential((src_view, &blur_pipeline.sampler)),
    );
    (params_bg, source_bg)
}

/// Draw the backdrop-filter elements' OWN content (the quad members in
/// `group_ranges` + the glyph/icon members in `glyph_group_ranges`/
/// `icon_group_ranges`) over the blurred window backdrop (parity Wave B4). These
/// ranges are EXCLUDED from the flat draw (they are `EffectGroup` members), and
/// the off-screen loops skip them (`is_pure_backdrop_filter`), so this LoadOp::Load
/// pass is where the element + its descendants paint — over the blur. The blur
/// preserves; the draws use the shared view uniform exactly like the flat pass.
///
/// QUADS, then GLYPHS, then ICONS (the global shadow < quad < glyph < icon rank,
/// scoped to the backdrop group's members) — so the header strip's title text
/// (a descendant of the backdrop-filter element, tagged into the SAME group)
/// renders over the blurred backdrop. v1 does NOT re-route the gradient/band
/// tiers (those draw the whole blob flat in the main pass, un-partitioned —
/// styling-f-tier.md § 2.3 / Wave B1), so a gradient/border on a backdrop-filter
/// element paints into the backdrop and is blurred; the gallery's two uses (solid
/// header bg + solid modal scrim) are unaffected. Documented follow-up.
#[allow(clippy::too_many_arguments)]
fn draw_backdrop_filter_fills(
    world: &World,
    view_target: &ViewTarget,
    prepared: &PreparedEffectGroups,
    buffers: &BuiyInstanceBuffers,
    view_pipelines: &BuiyViewPipelines,
    buiy_pipeline: &BuiyPipeline,
    pipeline_cache: &PipelineCache,
    view_bind_group: &bevy::render::render_resource::BindGroup,
    render_context: &mut RenderContext,
) {
    let quad_pl = pipeline_cache.get_render_pipeline(view_pipelines.quad);
    let glyph_pl = pipeline_cache.get_render_pipeline(view_pipelines.glyph);
    let atlas_bg = world
        .get_resource::<AtlasGpu>()
        .and_then(|a| a.coverage_bind_group());
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("buiy_backdrop_filter_fill_pass"),
        color_attachments: &[Some(view_target.get_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_bind_group(0, view_bind_group, &[]);
    pass.set_vertex_buffer(0, buiy_pipeline.vertex_buffer.slice(..));

    // --- QUAD members (the scrim / header bg fill) ---
    if let (Some(quad_pl), Some(quad_buffer)) = (quad_pl, buffers.quad.buffer()) {
        pass.set_render_pipeline(quad_pl);
        pass.set_vertex_buffer(1, quad_buffer.slice(..));
        for group in &prepared.groups {
            if !is_pure_backdrop_filter(group.reason) {
                continue;
            }
            if let Some(range) = buffers.group_ranges.get(group.index)
                && range.start < range.end
            {
                pass.draw(0..4, range.clone());
            }
        }
    }

    // --- GLYPH members (the header title text) + ICON members, sharing the
    // glyph pipeline + the atlas `@group(1)` coverage bind group (icons ARE
    // coverage stamps — Wave B3). Drawn AFTER the quads so text/icons sit over
    // the element's own fill.
    if let (Some(glyph_pl), Some(atlas_bg)) = (glyph_pl, atlas_bg) {
        pass.set_render_pipeline(glyph_pl);
        pass.set_bind_group(1, atlas_bg, &[]);
        if let Some(glyph_buffer) = buffers.glyph.buffer() {
            pass.set_vertex_buffer(1, glyph_buffer.slice(..));
            for group in &prepared.groups {
                if !is_pure_backdrop_filter(group.reason) {
                    continue;
                }
                if let Some(range) = buffers.glyph_group_ranges.get(group.index)
                    && range.start < range.end
                {
                    pass.draw(0..4, range.clone());
                }
            }
        }
        if let Some(icon_buffer) = buffers.icon.buffer() {
            pass.set_vertex_buffer(1, icon_buffer.slice(..));
            for group in &prepared.groups {
                if !is_pure_backdrop_filter(group.reason) {
                    continue;
                }
                if let Some(range) = buffers.icon_group_ranges.get(group.index)
                    && range.start < range.end
                {
                    pass.draw(0..4, range.clone());
                }
            }
        }
    }
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
