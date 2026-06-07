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

use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

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
    /// Groups in post-order (children before parents); `.index` is the
    /// position used by parent links.
    pub groups: Vec<PreparedEffectGroup>,
}

/// Per-`EffectGroup` prepare pass (`RenderSystems::Prepare`). Builds each
/// view's [`PreparedEffectGroups`] from extracted group members using the pure
/// geometry fns ([`painted_bounds`], [`bucket_extent`], [`post_order_indices`])
/// and the budget decision ([`plan_allocation`]). Pinned to `Prepare` because
/// the view `scale_factor` and `ViewTarget` do not exist until
/// `RenderSystems::ManageViews` runs (after `ExtractSchedule`), and the bounds
/// must fold through the FINAL `GlobalTransform` (effect-compositor.md § 1.1).
///
/// Skeleton: the extract of group members + per-view wiring is owned by
/// architecture § 3/§ 4 (not built in this phase). This system is the consumer
/// seam; its membership in `RenderSystems::Prepare` is asserted by the
/// `#[ignore]` GPU test, and the geometry it computes is covered headless by
/// the `painted_bounds` / `bucket_extent` / `post_order_indices` unit tests.
pub(crate) fn prepare_effect_groups() {
    // Body lands with the extract/per-view phase (architecture § 3/§ 4). It
    // composes painted_bounds → bucket_extent → group_target_descriptor,
    // post_order_indices for the composite order, and plan_allocation for the
    // rt_pool_budget degradation, writing one PreparedEffectGroups per view.
}

/// Register compositor pipelines/resources in the render app. Per
/// effect-compositor.md § 3 this adds **no** render-graph node and **no**
/// edge — the `BuiyRenderLabel` node group and its edges are owned by
/// architecture.md § 1.3; the compositor's passes run *inside*
/// [`BuiyNode::run`](super::node). v1 will register the per-`EffectGroup`
/// prepare system here; the composite pipeline assets slot in alongside the
/// typed-primitive per-format specializations (architecture § 1.4) as they
/// land.
///
/// Takes `&mut SubApp` to match the sibling `node::register` /
/// `pipeline::register` signatures (the `RenderApp` sub-app handle). Until
/// `prepare_effect_groups` lands (Task 9), this is a no-op placeholder that
/// deliberately adds no graph node.
pub(crate) fn register(render_app: &mut SubApp) {
    use bevy::render::{Render, RenderSystems};
    // The per-`EffectGroup` prepare pass (effect-compositor.md § 1.1) attaches
    // in `RenderSystems::Prepare` alongside `prepare_buiy_instances` — the view
    // `scale_factor` / `ViewTarget` do not exist until `ManageViews` runs
    // (architecture § 4). This adds a *system*, NOT a render-graph node: the
    // `BuiyRenderLabel` node group + edges remain owned by `node::register`
    // (architecture § 1.3); the composite passes run inside `BuiyNode::run`.
    render_app.add_systems(Render, prepare_effect_groups.in_set(RenderSystems::Prepare));
}
