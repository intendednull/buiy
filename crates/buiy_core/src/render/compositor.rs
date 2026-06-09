//! Off-screen effect-group compositor: prepare-phase geometry + pooling.
//!
//! Pillar 6 / spec:
//! docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md
//!
//! This module's *math* (painted-bounds union, next-pow2-capped bucketing,
//! post-order indexing, the `rt_pool_budget` degradation decision) is pure
//! CPU and unit-testable headless. The GPU half (the `RenderSystems::Prepare`
//! system, pooled `Rgba16Float` target acquisition, and the composite draws
//! inside `BuiyNode::run`) needs a wgpu adapter and is `#[ignore]`-gated,
//! exactly like `tests/render_smoke.rs`.

use std::ops::Range;

use bevy::prelude::*;
use bevy::render::render_resource::{
    CachedRenderPipelineId, Extent3d, PipelineCache, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::{CachedTexture, TextureCache};
use bevy::render::view::{Msaa, ViewTarget};

use crate::render::buckets::BuiyPrimitiveKind;
use crate::render::composite::{BuiySpecializedPipelines, CompositeKey, CompositePipeline};
use crate::render::extract::ExtractedEffectGroups;
use crate::render::prepare::BuiyInstanceBuffers;
use crate::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};

// `EffectReason` is the canonical bitflags owned by component-model.md § 10,
// already defined in `super::components` and re-exported from the crate root.
// The compositor's pure math needs it, so re-export it here for a stable
// `render::compositor::EffectReason` path; do NOT redefine it (the bits and
// derives live in one place — `components.rs`).
pub use super::components::EffectReason;

/// One prepared effect group: its post-order composite index, parent group
/// (None == composites into the window target), logical-px painted bounds,
/// bucketed physical-texel target extent, the group `Opacity` to apply at
/// composite, and the reasons it formed.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.1, § 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedEffectGroup {
    /// Post-order composite index (children before parents).
    pub index: usize,
    /// Post-order index of the enclosing group, or `None` for a root group
    /// (composites into the window `ViewTarget`).
    pub parent: Option<usize>,
    /// Logical-px painted bounds (group-local, pre-scale-factor).
    pub bounds: Rect,
    /// Bucketed physical-texel target size (next-pow2 per axis, capped at view).
    pub extent: UVec2,
    /// Group opacity applied at the parent composite (default 1.0).
    pub opacity: f32,
    /// OR of every reason that formed this group.
    pub reason: EffectReason,
}

/// One resolved-px ink-expansion term: a `margin` (px) by which paint
/// escapes the `around` box on every side. Caller resolves the per-term
/// margin upstream (effect-compositor.md § 2.1): `BoxShadow` = blur+spread,
/// `Outline` = max(0, width + offset), reserved `Filter` = blur bleed. The
/// terms are already-resolved px (no `Length` here — § 2.1: percent/`Cq*`
/// resolve before the sizing pass).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkExpansion {
    /// Outset margin in logical px (>= 0; inset offsets clamp to 0 upstream).
    pub margin: f32,
    /// The box this ink expands around (already folded through GlobalTransform).
    pub around: Rect,
}

/// Painted bounds = ( root box ∪ transitive descendant boxes ∪ ink expansion )
/// then clipped by the group's `ClipRect`. The single source of the formula
/// (effect-compositor.md § 2.1). Every input rect is logical-px, already
/// folded through `GlobalTransform` by the caller; the descendant slice is
/// the *transitive* subtree union (a nested group contributes its own
/// composited target's bounds, computed one level down — § 2.1).
pub fn painted_bounds(
    root: Rect,
    descendants: &[Rect],
    ink: &[InkExpansion],
    clip: Option<Rect>,
) -> Rect {
    let mut min = root.min;
    let mut max = root.max;
    let mut grow = |r: Rect| {
        min = min.min(r.min);
        max = max.max(r.max);
    };
    for &d in descendants {
        grow(d);
    }
    for e in ink {
        let m = Vec2::splat(e.margin);
        grow(Rect::from_corners(e.around.min - m, e.around.max + m));
    }
    let unioned = Rect { min, max };
    match clip {
        // Intersect last: clipped descendants cannot enlarge the target.
        // `Rect::intersect` collapses min over max, so a clip disjoint from
        // the painted union yields a degenerate (zero-area) rect rather than
        // an inverted one with negative width()/height().
        Some(c) => unioned.intersect(c),
        None => unioned,
    }
}

/// Bucket a group's logical-px painted-bounds *size* into a physical-texel
/// `Extent` for the pooled `TextureDescriptor` (effect-compositor.md § 2.2).
///
/// Rule (committed): fold in `scale_factor`, snap out to integer texels,
/// round each axis up to the next power of two — but **cap at the view's
/// physical size** on that axis (a group exceeding the viewport collapses
/// onto one shared view-size bucket rather than rounding past the viewport).
/// The numeric pow2 thresholds (smallest bucket, steps) calibrate later
/// (README § 5 #4); the rule shape is fixed here.
pub fn bucket_extent(logical_size: Vec2, scale_factor: f32, view_physical: UVec2) -> UVec2 {
    let phys = (logical_size * scale_factor).ceil();
    let w = bucket_axis(phys.x, view_physical.x);
    let h = bucket_axis(phys.y, view_physical.y);
    UVec2::new(w, h)
}

fn bucket_axis(physical: f32, view: u32) -> u32 {
    let texels = physical.max(0.0) as u32;
    // At least 1 texel; never a 0-sized target.
    let texels = texels.max(1);
    if texels >= view {
        // Cap at the view dimension (one stable shared bucket).
        view.max(1)
    } else {
        // Next power of two, but never past the view cap.
        texels.next_power_of_two().min(view.max(1))
    }
}

/// Produce the bottom-up composite order over the effect-group nesting
/// forest: every child appears before its parent (effect-compositor.md § 3).
/// `parents[i]` is the index of group `i`'s enclosing group, or `None` if
/// `i` is a root group. The returned vec lists group indices in the order
/// `BuiyNode::run` must rasterize + composite them. The prepare pass stores
/// this so the node never walks the main world at run time (§ 3).
pub fn post_order_indices(parents: &[Option<usize>]) -> Vec<usize> {
    // children[p] = groups whose parent is p.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); parents.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (i, p) in parents.iter().enumerate() {
        match p {
            Some(parent) => children[*parent].push(i),
            None => roots.push(i),
        }
    }
    let mut order = Vec::with_capacity(parents.len());
    // Iterative post-order DFS (explicit stack: no recursion-depth risk).
    // Stack frames carry (node, children_emitted?).
    for &root in &roots {
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            if expanded {
                order.push(node);
            } else {
                stack.push((node, true));
                for &c in &children[node] {
                    stack.push((c, false));
                }
            }
        }
    }
    order
}

/// Aggregate live-target budget (bytes) on the concurrent effect-group RT
/// set — committed mechanism (effect-compositor.md § 2.3). v1 default
/// 64 MiB; the *tuned* number defers to `buiy-verification-design`
/// (README § 5 #4), like the atlas `page_budget`. Parallel to, not shared
/// with, the glyph atlas pool.
pub const RT_POOL_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

/// Bytes one pooled target of `extent` consumes. Group targets are pinned
/// `Rgba16Float` (effect-compositor.md § 2.2) = 8 bytes/texel.
pub fn target_bytes(extent: UVec2) -> u64 {
    const BYTES_PER_TEXEL: u64 = 8; // Rgba16Float
    u64::from(extent.x) * u64::from(extent.y) * BYTES_PER_TEXEL
}

/// Decide which groups allocate an off-screen target vs. fall back to
/// direct-to-parent forward compositing under budget pressure
/// (effect-compositor.md § 2.3). Returns one `bool` per input group, in the
/// input order: `true` == allocate a pooled target, `false` == degrade.
///
/// Ranking when the live set would exceed `budget`: eviction is **strict
/// priority**, not a utilization-maximizing bin-pack. The lowest-cost groups
/// degrade first — smallest painted-bounds area and `OPACITY`-only reason
/// first; an `ISOLATION`/reserved group degrades last (its boundary is
/// structural, not just an alpha multiply). A structural group is therefore
/// never degraded while any opacity-only group still holds a target, even when
/// the structural group is individually larger than the remaining budget —
/// dropping it does not "free room" for a lower-priority survivor.
pub fn plan_allocation(groups: &[(UVec2, EffectReason)], budget: u64) -> Vec<bool> {
    // `degrade_rank` orders groups degrade-FIRST: opacity-only before
    // structural, then smaller before larger within each class. Dropping from
    // the front of this order until the survivors fit is exactly the spec's
    // strict-priority eviction (effect-compositor.md § 2.3) — a structural
    // group only degrades once every opacity-only group has already degraded.
    let degrade_rank = |&(extent, reason): &(UVec2, EffectReason)| -> (bool, u64) {
        // `false` (structural) sorts after `true` (opacity-only) below, so
        // structural groups land at the keep-last end of the degrade order.
        let opacity_only = reason == EffectReason::OPACITY;
        (!opacity_only, target_bytes(extent))
    };

    // Indices in degrade-first order (front = degrades first).
    let mut degrade_order: Vec<usize> = (0..groups.len()).collect();
    degrade_order.sort_by_key(|&i| degrade_rank(&groups[i]));

    // Start from "keep everything", then drop from the degrade-first front
    // until the surviving set fits the budget. Strict priority: we never skip
    // a high-priority group to fit a lower-priority one.
    let mut allocate = vec![true; groups.len()];
    let mut live: u64 = degrade_order
        .iter()
        .map(|&i| target_bytes(groups[i].0))
        .sum();
    let mut next_to_drop = 0;
    while live > budget && next_to_drop < degrade_order.len() {
        let i = degrade_order[next_to_drop];
        allocate[i] = false; // degrade to forward compositing
        live -= target_bytes(groups[i].0);
        next_to_drop += 1;
    }
    allocate
}

/// The pinned off-screen group-target descriptor (effect-compositor.md § 2.2):
/// FIXED `Rgba16Float` (linear, NOT the view's SDR format) so group opacity +
/// isolation composite in linear space; `RENDER_ATTACHMENT` (subtree renders
/// into it) | `TEXTURE_BINDING` (composite pass samples it). `extent` is the
/// already-bucketed physical-texel size (`bucket_extent`). Descriptor-keyed
/// `TextureCache` reuse depends on this being byte-identical across frames for
/// a given bucket size — hence the fixed label/format/usage.
pub fn group_target_descriptor(extent: UVec2) -> TextureDescriptor<'static> {
    TextureDescriptor {
        label: Some("buiy_effect_group_target"),
        size: Extent3d {
            width: extent.x,
            height: extent.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba16Float,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

/// Composite one group sample over a destination in **linear** space with
/// `SrcOver`, scaling the source's coverage by the group `opacity`
/// (effect-compositor.md § 3 step 2 / § 4). This is the *correct* group
/// opacity: the whole group is sampled as one already-composed unit and its
/// translucency applies once, so overlapping children inside the group do
/// not double-darken (the rejected per-child approximation, § 4).
///
/// `src`/`dst` are straight-alpha linear colors; `opacity ∈ [0,1]`. The GPU
/// path runs this same arithmetic as a `SrcOver` blend over an `Rgba16Float`
/// target with the sampled alpha pre-multiplied by `opacity`; this CPU port
/// pins the math headless (mirrors render_instance.rs's SDF port).
pub fn composite_src_over(src: LinearRgba, dst: LinearRgba, opacity: f32) -> LinearRgba {
    let a = (src.alpha * opacity).clamp(0.0, 1.0);
    let inv = 1.0 - a;
    LinearRgba::new(
        src.red * a + dst.red * inv,
        src.green * a + dst.green * inv,
        src.blue * a + dst.blue * inv,
        a + dst.alpha * inv,
    )
}

/// Per-view prepared effect groups, stored as a component on the view render
/// entity (parallel to `ExtractedNodes` / `BuiyInstanceBuffers`, architecture
/// § 4 — NOT a global resource). Post-ordered, sized, descriptor-bucketed.
/// `BuiyNode::run` reads this off its matched view entity and never walks the
/// main world from `&World` (effect-compositor.md § 1.1 / § 3).
#[derive(Component, Default, Clone, Debug)]
pub struct PreparedEffectGroups {
    /// One entry per extracted effect group, in EXTRACT order. `groups[g].index`
    /// equals `g` (so `BuiyInstanceBuffers::group_ranges[g]` lines up), and
    /// `groups[g].parent` is the EXTRACT index of the enclosing group. The
    /// bottom-up composite sequence is [`composite_order`] (children before
    /// parents), NOT the natural order of this vec.
    ///
    /// [`composite_order`]: PreparedEffectGroups::composite_order
    pub groups: Vec<PreparedEffectGroup>,
    /// The post-order composite sequence over `groups` (each child's extract
    /// index appears before its parent's) — `post_order_indices` applied to the
    /// parent links. `BuiyNode::run` walks this so a parent samples a child's
    /// already-composited target (effect-compositor.md § 3).
    pub composite_order: Vec<usize>,
    /// The `Quad@Rgba16Float` pipeline id every step-1 group pass binds (the
    /// quad specialization keyed on the group-target format). Specialized in
    /// prepare (`ResMut<SpecializedRenderPipelines>` is unavailable to the node's
    /// `&World`), so the node only reads. `None` until the pipeline async-compiles.
    pub quad_pipeline: Option<CachedRenderPipelineId>,
}

/// Sibling carrier (decided fork 1): the device handles + per-group placement
/// the off-screen passes need, kept OFF the `Copy + PartialEq` pure
/// [`PreparedEffectGroup`] (a `CachedTexture` is neither, and is render-only).
/// Indexed in lockstep with [`PreparedEffectGroups::groups`] (extract order), so
/// `targets[g]` is group `g`'s target. Attached to the SAME view render entity as
/// `PreparedEffectGroups` (so `BuiyNode`'s `ViewQuery` resolves both off one
/// entity — NOT a resource). Held for the whole node run so a child target is not
/// recycled before its parent samples it (the residency rule, § 3).
#[derive(Component, Default, Clone)]
pub struct PreparedEffectTargets {
    /// Per-group off-screen `Rgba16Float` targets (extract order). `None` == the
    /// group degraded under budget (`plan_allocation` == false) and has no target
    /// — the node skips it (v1: degraded groups draw flat, no per-child approx).
    pub targets: Vec<Option<CachedTexture>>,
    /// Per-group placement: the logical→target view-uniform columns (to render
    /// the group's subtree INTO its target), the composite quad's logical bounds,
    /// the uv sub-rect (the used region of the pow2-bucketed target), and the
    /// `opacity` applied at composite. Extract order, in lockstep with `targets`.
    pub placements: Vec<GroupPlacement>,
}

/// One group's off-screen render + composite placement (sibling-carrier payload).
#[derive(Clone, Debug)]
pub struct GroupPlacement {
    /// The quad-instance range (`BuiyInstanceBuffers::group_ranges` index `==`
    /// this group's extract index) the step-1 pass draws into the target.
    pub instance_range: Range<u32>,
    /// The logical→target-clip affine columns (`col0 = [sx,0,0,tx]`,
    /// `col1 = [0,sy,0,ty]`, `params = [scale_factor,0,0,0]`) — the per-group view
    /// uniform that maps logical px into THIS target's bucketed texel grid,
    /// anchored at the painted-bounds min.
    pub target_view_columns: [Vec4; 3],
    /// The composite quad's logical-px bounds in the PARENT's space (where the
    /// sampled target lands at composite).
    pub composite_bounds: Rect,
    /// The PARENT's logical→clip columns (`col0`, `col1`) — the window's view
    /// transform for a root group, or the enclosing group's `target_view_columns`
    /// for a nested group. The composite quad is placed with these so a nested
    /// group lands at the right spot inside its parent's target.
    pub composite_view_columns: [Vec4; 2],
    /// The used sub-rect of the (pow2-bucketed) target in normalized UV
    /// (`[0,0]..[used_w/extent_w, used_h/extent_h]`), so the composite samples
    /// only the painted region, not the pow2 padding.
    pub uv_max: Vec2,
    /// Group opacity applied at composite (`sampled.a * opacity`).
    pub opacity: f32,
    /// The composite pipeline id for THIS group's PARENT format (window
    /// `Rgba8UnormSrgb` for a root group, `Rgba16Float` for a nested group).
    /// Specialized in prepare; `None` until it compiles.
    pub composite_pipeline: Option<CachedRenderPipelineId>,
}

/// Observable render-world stat for the RT-pool leak gate (gate #15): the count
/// of distinct `buiy_effect_group_target` descriptor buckets the prepare pass
/// touched this frame and the live (taken) target count. The `TextureCache`'s own
/// per-descriptor buckets are private, so `prepare_effect_groups` records the
/// working-set size here for the `rt_pool_returns_to_baseline_after_idle` test to
/// observe return-to-baseline after idle (effect-compositor.md § 2.3).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct RtPoolStats {
    /// Distinct `(extent)` bucket descriptors acquired this frame.
    pub distinct_buckets: usize,
    /// Targets acquired (taken) this frame (== non-degraded live group count).
    pub live_targets: usize,
}

/// Per-`EffectGroup` prepare pass (`RenderSystems::Prepare`). Builds each view's
/// [`PreparedEffectGroups`] + the sibling [`PreparedEffectTargets`] from the
/// extracted group list ([`ExtractedEffectGroups`]): it buckets each group's
/// painted bounds ([`bucket_extent`]), runs the budget decision
/// ([`plan_allocation`]), ACQUIRES the pooled `Rgba16Float` targets via
/// `&mut TextureCache` + `&RenderDevice` (the node's `&World` cannot get the
/// mutable cache handle — the `node.rs:88-96` precedent), specializes the
/// `Quad@Rgba16Float` group-pass + the composite pipelines (the node only reads
/// the resulting ids), computes the per-group view/composite transforms +
/// post-order ([`post_order_indices`]), and INSERTS both carriers onto the view
/// render entity (decided fork 2 — NOT a resource, so the node's `ViewQuery`
/// resolves them). Pinned to `Prepare` (after `prepare_buiy_instances`, so the
/// per-group instance ranges in `BuiyInstanceBuffers` are written first) because
/// the view `scale_factor` / `ViewTarget` do not exist until `ManageViews` runs.
/// Records the working-set size into [`RtPoolStats`] for the leak gate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_effect_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    extracted: Res<ExtractedEffectGroups>,
    buffers: Res<BuiyInstanceBuffers>,
    mut stats: ResMut<RtPoolStats>,
    pipeline_cache: Res<PipelineCache>,
    composite_pipeline: Res<CompositePipeline>,
    mut group_pipelines: ResMut<BuiySpecializedPipelines>,
    // The view render entities `BuiyNode` runs on (those with a `ViewTarget`).
    // v1/D2: all groups resolve to the primary view, so the per-view
    // `ExtractedEffectGroups` resource shim is attached to every `ViewTarget`
    // entity — the carrier lands on the SAME entity the node's `ViewQuery`
    // resolves (decided fork 2 — NOT a resource). `ExtractedNodes` carries the
    // view scale_factor used for bucketing. `Msaa` (extracted per view) keys
    // the root-group composite's sample count — that pass draws into the
    // window attachment, which is multisampled under MSAA.
    views: Query<(Entity, &ViewTarget, &Msaa)>,
    nodes: Res<crate::render::extract::ExtractedNodesView>,
) {
    let extracted = &extracted.0;

    // No live groups: clear the carriers off every view so a frame that drops
    // the last group runs the flat path with `Option<&PreparedEffectGroups>` ==
    // the empty groups vec. Reset the stat (no buckets held this frame).
    if extracted.is_empty() {
        *stats = RtPoolStats::default();
        for (view, _, _) in &views {
            commands
                .entity(view)
                .insert(PreparedEffectGroups::default())
                .insert(PreparedEffectTargets::default());
        }
        return;
    }

    // The window (root-group parent) attachment format + sample count — every
    // live view shares them in v1/D2 (single primary view). The composite
    // pipeline is specialized per parent attachment: root groups composite into
    // the window format AT the view's `Msaa` sample count (the window pass
    // attachment is multisampled when MSAA is on), nested groups into the
    // `Rgba16Float` single-sampled group-target format.
    //
    // v1/D2: a single primary view — the composite pipelines are specialized for
    // the FIRST view's format/samples and reused for every view below. A second
    // window with a different attachment format (e.g. HDR) or MSAA mode would
    // need per-view specialization; that rides the per-view-routing follow-up.
    // No-views is unreachable here in practice: with no view entities there are
    // no extracted groups either (the `extracted.is_empty()` early-return above
    // fires first). Return loudly instead of a silent wrong-key fallback so a
    // future multi-view regression cannot composite with a mismatched pipeline.
    let Some((window_format, window_samples)) = views
        .iter()
        .next()
        .map(|(_, vt, msaa)| (vt.main_texture_format(), msaa.samples()))
    else {
        return;
    };

    // Specialize the `Quad@Rgba16Float` group-pass pipeline + the two composite
    // variants HERE (prepare owns the mutable specialization cache; the node's
    // `&World` cannot). The node only reads the resulting ids. The group
    // targets are created single-sampled (`group_target_descriptor`,
    // `sample_count: 1`), so everything drawing INTO a group target — the
    // step-1 quad pass and the step-2a nested composite — keys `samples: 1`;
    // only the root composite (into the window pass) keys the view's samples.
    let quad_pipeline = Some(group_pipelines.primitives.specialize(
        &pipeline_cache,
        &BuiyPrimitives,
        BuiyPrimitiveKey {
            kind: BuiyPrimitiveKind::Quad,
            format: TextureFormat::Rgba16Float,
            samples: 1,
        },
    ));
    let composite_into_window = group_pipelines.composite.specialize(
        &pipeline_cache,
        &composite_pipeline,
        CompositeKey {
            parent_format: window_format,
            samples: window_samples,
        },
    );
    let composite_into_group = group_pipelines.composite.specialize(
        &pipeline_cache,
        &composite_pipeline,
        CompositeKey {
            parent_format: TextureFormat::Rgba16Float,
            samples: 1,
        },
    );

    let scale_factor = nodes.0.scale_factor;
    // The view's physical size caps the per-group bucket (a group larger than the
    // viewport shares one view-size bucket — `bucket_extent`).
    let view_physical = (nodes.0.logical_size * scale_factor).ceil().as_uvec2();

    // Budget decision (effect-compositor.md § 2.3): rank by (extent, reason),
    // degrade lowest-cost first. Inputs in extract order.
    let alloc_inputs: Vec<(UVec2, EffectReason)> = extracted
        .iter()
        .map(|g| {
            let extent = bucket_extent(g.bounds.size(), scale_factor, view_physical);
            (extent, g.reason)
        })
        .collect();
    let allocate = plan_allocation(&alloc_inputs, RT_POOL_BUDGET_BYTES);

    // Build the post-order composite sequence over the parent links.
    let parents: Vec<Option<usize>> = extracted.iter().map(|g| g.parent).collect();
    let composite_order = post_order_indices(&parents);

    // The window's logical→clip columns (the parent transform of a ROOT group's
    // composite — places the sampled target over the in-flow window content).
    let window_uniform =
        crate::render::view_uniform::BuiyViewUniform::for_view(nodes.0.logical_size, scale_factor);
    let window_cols = {
        let a = window_uniform.as_std140_array();
        [
            Vec4::new(a[0], a[1], a[2], a[3]),
            Vec4::new(a[4], a[5], a[6], a[7]),
        ]
    };

    // Phase 1 — per-group target view columns (logical → THIS target's bucketed
    // texel grid, anchored at the painted-bounds min). Computed for ALL groups
    // first so a nested group can read its parent's columns for its composite.
    let target_cols: Vec<[Vec4; 3]> = extracted
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let (extent, _) = alloc_inputs[i];
            let ex = extent.x.max(1) as f32;
            let ey = extent.y.max(1) as f32;
            let sx = 2.0 * scale_factor / ex;
            let tx = -2.0 * g.bounds.min.x * scale_factor / ex - 1.0;
            let sy = -2.0 * scale_factor / ey;
            let ty = 2.0 * g.bounds.min.y * scale_factor / ey + 1.0;
            [
                Vec4::new(sx, 0.0, 0.0, tx),
                Vec4::new(0.0, sy, 0.0, ty),
                Vec4::new(scale_factor, 0.0, 0.0, 0.0),
            ]
        })
        .collect();

    let mut groups: Vec<PreparedEffectGroup> = Vec::with_capacity(extracted.len());
    let mut targets: Vec<Option<CachedTexture>> = Vec::with_capacity(extracted.len());
    let mut placements: Vec<GroupPlacement> = Vec::with_capacity(extracted.len());
    let mut distinct: std::collections::HashSet<UVec2> = std::collections::HashSet::new();
    let mut live_targets = 0usize;

    for (i, g) in extracted.iter().enumerate() {
        let (extent, _reason) = alloc_inputs[i];
        groups.push(PreparedEffectGroup {
            index: i,
            parent: g.parent,
            bounds: g.bounds,
            extent,
            opacity: g.opacity,
            reason: g.reason,
        });

        // The group's quad-instance range (extract index == group_ranges index).
        let instance_range = buffers.group_ranges.get(i).cloned().unwrap_or(0..0);

        // Acquire the pooled target unless the group degraded under budget.
        let target = if allocate[i] {
            distinct.insert(extent);
            live_targets += 1;
            Some(texture_cache.get(&render_device, group_target_descriptor(extent)))
        } else {
            None
        };
        targets.push(target);

        // Composite placement: the used region (physical px of the bounds) sets
        // the uv_max so the pow2 padding is never sampled; the parent columns are
        // the window (root) or the enclosing group's target columns (nested).
        let bounds = g.bounds;
        let used = (bounds.size() * scale_factor).ceil();
        let ex = extent.x.max(1) as f32;
        let ey = extent.y.max(1) as f32;
        // A root group (no parent) composites into the window format; a nested
        // group composites into its parent's `Rgba16Float` target.
        let composite_pipeline = Some(if g.parent.is_some() {
            composite_into_group
        } else {
            composite_into_window
        });
        let composite_view_columns = match g.parent {
            Some(p) => [target_cols[p][0], target_cols[p][1]],
            None => window_cols,
        };
        placements.push(GroupPlacement {
            instance_range,
            target_view_columns: target_cols[i],
            composite_bounds: bounds,
            composite_view_columns,
            uv_max: Vec2::new((used.x / ex).min(1.0), (used.y / ey).min(1.0)),
            opacity: g.opacity,
            composite_pipeline,
        });
    }

    *stats = RtPoolStats {
        distinct_buckets: distinct.len(),
        live_targets,
    };

    let prepared = PreparedEffectGroups {
        groups,
        composite_order,
        quad_pipeline,
    };
    let prepared_targets = PreparedEffectTargets {
        targets,
        placements,
    };
    // Attach BOTH carriers to the SAME view render entity (decided fork 2): the
    // node's `ViewQuery = (&ViewTarget, Option<&PreparedEffectGroups>)` resolves
    // them off this entity. A resource shim would be invisible to the node and
    // leave the loops inert (the false-green risk) — so these are COMPONENTS.
    for (view, _, _) in &views {
        commands
            .entity(view)
            .insert(prepared.clone())
            .insert(prepared_targets.clone());
    }
}

/// Register compositor pipelines/resources in the render app. Per
/// effect-compositor.md § 3 this adds **no** render-graph node and **no**
/// edge — the `BuiyRenderLabel` node group and its edges are owned by
/// architecture.md § 1.3; the compositor's passes run *inside*
/// [`BuiyNode::run`](super::node). It registers the per-`EffectGroup`
/// [`prepare_effect_groups`] system, the [`RtPoolStats`] observable, and (via
/// [`super::composite::register`]) the composite-pipeline specialization cache.
/// The device-owning composite resources (`CompositePipeline`) init in
/// `finish` (`composite::register_gpu`).
///
/// Takes `&mut SubApp` to match the sibling `node::register` /
/// `pipeline::register` signatures (the `RenderApp` sub-app handle).
pub(crate) fn register(render_app: &mut SubApp) {
    use bevy::render::{Render, RenderSystems};
    // The composite pipeline assets (the Quad@Rgba16Float specialization cache +
    // the textured-quad composite pipeline) — device-free to init here; the
    // concrete pipeline ids materialize lazily through the `PipelineCache`.
    render_app.init_resource::<RtPoolStats>();
    super::composite::register(render_app);
    // The per-`EffectGroup` prepare pass (effect-compositor.md § 1.1) attaches
    // in `RenderSystems::Prepare`. It runs AFTER `prepare_buiy_instances` so the
    // per-group instance ranges (`BuiyInstanceBuffers::group_ranges`) are written
    // before this reads them. The view `scale_factor` / `ViewTarget` exist in
    // `Prepare` (after `ManageViews`). This adds a *system*, NOT a render-graph
    // node: the `BuiyRenderLabel` node group + edges remain owned by
    // `node::register`; the composite passes run inside `BuiyNode::run`.
    render_app.add_systems(
        Render,
        prepare_effect_groups
            .in_set(RenderSystems::Prepare)
            .after(super::prepare::prepare_buiy_instances),
    );
}
