//! Typed render primitives and `(primitive, layer)` instance bucketing (the
//! CPU instance-bucketing module — R6 owns it).
//!
//! architecture.md § 2: `BuiyNode` is a small fixed set of typed SDF
//! primitives, batched per `(primitive, layer)`, each batch a single instanced
//! draw. This module owns the shared `BuiyPrimitiveKind` enum, the batch key,
//! and the per-view bucket store the prepare phase fills. R7's pipeline
//! specialization key (`render/primitive.rs`) **imports** `BuiyPrimitiveKind`
//! from here — it is NOT redefined there. The `layer` is the forward index
//! into `StackingContext.painters_z` (§ 2.2); this phase threads it but
//! defaults to 0 (real layers are the paint-order phase's job).

use std::collections::{BTreeMap, HashMap, hash_map::Entry};
use std::ops::Range;

use crate::render::extract::{ExtractedNode, TextQuad};
use crate::render::instance::{
    BorderBandInstance, GradientInstance, PackedInstance, pack_border, pack_extracted,
    pack_gradient, pack_outline, pack_shadow, pack_text_quad,
};
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::{Color, Entity};
use bytemuck::Pod;

/// A typed render primitive — the shared primitive-kind enum (R6 owns it; R7
/// imports it from `render::buckets`). This is the authoritative primitive set
/// from architecture.md § 2.1: `quad` / `shadow` / `glyph` / `path`. `Border`
/// is **not** a distinct primitive — it folds into `Quad` (background fill +
/// border band + rounded corners, one SDF), and `Outline` is a `Quad` variant
/// (the quad pipeline with the clip rect suppressed), not its own discriminant.
/// v1 only emits `Quad`; `Shadow` is the next F-tier pipeline and `Glyph` /
/// `Path` are the text/path seams — all four are bucket-reserved here so the
/// shared enum matches the design the paint-order and pipeline phases consume.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuiyPrimitiveKind {
    /// Box-shadow SDF (painted ahead of its caster). Lowest paint order.
    Shadow,
    /// Background fill + border band + rounded corners (one SDF rounded-rect).
    /// `Outline` paints through this same pipeline as a clip-suppressed variant.
    Quad,
    /// Single text glyph sampling the alpha atlas (atlas-and-text-seam.md).
    Glyph,
    /// Filled arbitrary 2D path SDF. Highest paint order. (C-tier shader.)
    Path,
}

impl BuiyPrimitiveKind {
    /// Within-layer paint rank: back-to-front `shadow < quad < glyph < path`
    /// (architecture.md § 2.2).
    pub fn paint_order(self) -> u8 {
        match self {
            BuiyPrimitiveKind::Shadow => 0,
            BuiyPrimitiveKind::Quad => 1,
            BuiyPrimitiveKind::Glyph => 2,
            BuiyPrimitiveKind::Path => 3,
        }
    }
}

/// The key a batch is grouped by: `(primitive, layer)`. Ordering is **layer
/// first** (the forward `painters_z` walk), then primitive paint order, so the
/// natural `BTreeMap` iteration is the draw order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PrimitiveBatchKey {
    pub primitive: BuiyPrimitiveKind,
    pub layer: u32,
}

impl PartialOrd for PrimitiveBatchKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PrimitiveBatchKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.layer.cmp(&other.layer).then(
            self.primitive
                .paint_order()
                .cmp(&other.primitive.paint_order()),
        )
    }
}

/// Per-view bucket store: each `(primitive, layer)` maps to its instance
/// vector. Iteration is in draw order (the `BTreeMap` key order). The instance
/// payload is generic over a `Pod` record so the same store can hold quad
/// instances now and other primitive layouts later; this phase uses
/// [`PackedInstance`].
#[derive(Default)]
pub struct InstanceBuckets {
    batches: BTreeMap<PrimitiveBatchKey, Vec<[f32; 17]>>,
}

impl InstanceBuckets {
    /// Push one packed instance (as raw `[f32; 17]` =
    /// pos2+size2+color4+radius1+clip_min2+clip_max2+affine4) into its batch.
    pub fn push(&mut self, key: PrimitiveBatchKey, instance: [f32; 17]) {
        self.batches.entry(key).or_default().push(instance);
    }

    /// Number of instances in a batch (0 if the key was never pushed to).
    pub fn len(&self, key: PrimitiveBatchKey) -> usize {
        self.batches.get(&key).map_or(0, Vec::len)
    }

    /// `true` iff no instances were pushed.
    pub fn is_empty(&self) -> bool {
        self.batches.values().all(Vec::is_empty)
    }

    /// Total instance count across all batches.
    pub fn total_instances(&self) -> usize {
        self.batches.values().map(Vec::len).sum()
    }

    /// Iterate batches in draw order (`(layer, primitive paint order)`).
    pub fn batches(&self) -> impl Iterator<Item = (&PrimitiveBatchKey, &Vec<[f32; 17]>)> {
        self.batches.iter()
    }
}

/// Flatten a [`PackedInstance`] into the raw `[f32; 17]` the bucket store holds.
/// Keeps the bucket store decoupled from the concrete instance struct while
/// the stride is asserted equal in tests.
///
/// LAYOUT INVARIANT (R1 / R2 dependency): indices `0..13` are byte-identical to
/// the pre-R1 layout — color is at [`COLOR_FLOAT_OFFSET`]`..+4` and alpha at
/// [`ALPHA_FLOAT_OFFSET`] (R2's degraded-group re-tint reads alpha there). The
/// 2D affine basis appends at `[13..17]` (`[m00, m10, m01, m11]`); identity
/// `[1, 0, 0, 1]` paints axis-aligned.
///
/// [`COLOR_FLOAT_OFFSET`]: crate::render::instance::COLOR_FLOAT_OFFSET
/// [`ALPHA_FLOAT_OFFSET`]: crate::render::instance::ALPHA_FLOAT_OFFSET
pub fn packed_to_raw(p: &PackedInstance) -> [f32; 17] {
    [
        p.rect_pos[0],
        p.rect_pos[1],
        p.rect_size[0],
        p.rect_size[1],
        // color@COLOR_FLOAT_OFFSET (4..8); alpha@ALPHA_FLOAT_OFFSET (7).
        p.color[0],
        p.color[1],
        p.color[2],
        p.color[3],
        p.radius,
        p.clip_min[0],
        p.clip_min[1],
        p.clip_max[0],
        p.clip_max[1],
        // The 2D affine basis APPENDED after index 13 (offsets 0..13 unchanged).
        p.affine[0],
        p.affine[1],
        p.affine[2],
        p.affine[3],
    ]
}

// Asserts at compile time (via a const fn caller in tests) that the raw layout
// matches the struct stride. `_pod` is here so a future non-f32 record forces
// a conscious change to `packed_to_raw`.
const _ASSERT_POD: fn() = || {
    fn _is_pod<T: Pod>() {}
    _is_pod::<PackedInstance>();
};

/// Tracks the base↔top-layer boundary as a per-node tier packer walks `nodes` in
/// paint order (the top-layer stacking composite, § 3.2). Every packer that
/// partitions its blob at the top-layer boundary drives one of these: call
/// [`observe`](Self::observe) for each node BEFORE pushing that node's
/// instances, passing the tier's current instance count; [`finish`](Self::finish)
/// yields the boundary — the instance index of the first top-layer node's first
/// instance, or the total count when the view has no top-layer node (an empty
/// top-layer block `[count..count)`, the byte-stable path).
///
/// The tail-contiguity `debug_assert` in `observe` is the production tripwire
/// (spec § 3.4): top-layer content is a contiguous suffix of the paint order
/// (`context_tree_paint_order` + `cross_root_rank`), so once a top-layer node is
/// seen no base node may follow. It caught the § 3.1 per-node-vs-ancestor-climb
/// classification bug in one GPU run — a hard panic, not a silent wrong pixel.
#[derive(Default)]
struct TopLayerBoundaryTracker {
    boundary: Option<u32>,
    seen_top_layer: bool,
}

impl TopLayerBoundaryTracker {
    /// Observe `node` (in paint order) with the tier's current `instance_count`
    /// (the index the node's first instance is about to occupy). Records the
    /// boundary at the first top-layer node and trips the tail-contiguity
    /// `debug_assert` on a base node after a top-layer one.
    fn observe(&mut self, node: &ExtractedNode, instance_count: u32) {
        debug_assert!(
            !(self.seen_top_layer && !node.top_layer),
            "top-layer nodes must form a contiguous tail: a base node followed a \
             top-layer node in the paint-order walk (the ancestor-climb classifier \
             drifted from the top-layer materialization, or the tail is not \
             context-contiguous)"
        );
        if node.top_layer {
            self.boundary.get_or_insert(instance_count);
            self.seen_top_layer = true;
        }
    }

    /// The tier's boundary: the first top-layer instance index, or `total` (the
    /// tier's final instance count) when the view has no top-layer node.
    fn finish(self, total: u32) -> u32 {
        self.boundary.unwrap_or(total)
    }
}

/// Pack a per-view node list — R5's [`ExtractedNode`] records — into
/// typed-primitive `(primitive, layer)` buckets. v1 routes every node to
/// `(Quad, layer 0)` — the only primitive the v1 set emits — packing each via
/// [`pack_extracted`]. R6 feeds R5's `ExtractedNodes.nodes` here with no
/// `DrawData` adapter in between (the prepare seam). The `layer` will become the
/// real forward `painters_z` index when the paint-order phase lands; until then
/// it is 0.
///
/// Transparent nodes (`color == Color::NONE`) are skipped here — no quad
/// instance is emitted. This honors [`ExtractedNode::color`]'s contract
/// (`Color::NONE` == transparent, no quad downstream) and mirrors the Phase-0
/// `draw_for_node` skip. Without it every backgroundless layout container
/// (`extracted_node_for` sets `Color::NONE` when `background == None`) would
/// pack a wasted, invisible (alpha-0) quad per frame.
pub fn pack_view(nodes: &[ExtractedNode]) -> InstanceBuckets {
    let mut buckets = InstanceBuckets::default();
    let quad0 = PrimitiveBatchKey {
        primitive: BuiyPrimitiveKind::Quad,
        layer: 0,
    };
    for node in nodes {
        if node.color == Color::NONE {
            continue;
        }
        buckets.push(quad0, packed_to_raw(&pack_extracted(node)));
    }
    buckets
}

/// Pack a view's node list into the flat border/outline BAND instance blob, in
/// paint order (styling-f-tier.md § 2.3 / § 2.4). One [`BorderBandInstance`] per
/// node BORDER (C6-b, drawn AT the box edge) and one per node OUTLINE (C6-a, the
/// ring outside the box); nodes with neither contribute nothing (the byte-stable
/// no-band path). The band buffer draws AFTER the quad/glyph window draw, so a
/// band sits on top of the fill + text within the box; within a node the BORDER
/// is pushed before the OUTLINE so the outline paints over the border (CSS:
/// border paints inside the box, outline outside on top).
///
/// The border uses the entity's OWN clip (it is inside the border box); the
/// outline uses the entity's `AncestorClip` (resolved at extract), so a focus
/// ring survives an `overflow:hidden` ancestor. Both are resolved at extract.
///
/// v1: the band rides the FLAT window draw only — it is not partitioned into
/// effect-group off-screen targets (a ring/border on a grouped element is a
/// follow-up; the common case is a top-level widget).
///
/// Returns the blob PLUS its base↔top-layer boundary (top-layer stacking
/// composite, § 3.2): the instance index of the first top-layer node's first band
/// (border before outline), or the band count when no top-layer node has one. The
/// per-block draw (W2) draws base bands `[0..boundary)` then top-layer bands over
/// the top-layer tier-stack, so a base border no longer bleeds through a scrim. A
/// tail-contiguity `debug_assert` ([`TopLayerBoundaryTracker`]) guards § 3.4.
pub fn pack_band_instances(nodes: &[ExtractedNode]) -> (Vec<BorderBandInstance>, u32) {
    let mut bands = Vec::new();
    let mut top_layer = TopLayerBoundaryTracker::default();
    for n in nodes {
        top_layer.observe(n, bands.len() as u32);
        // Border first (inside the box), then outline (outside, on top).
        if let Some(border) = n.border.as_ref() {
            bands.push(pack_border(border));
        }
        if let Some(outline) = n.outline.as_ref() {
            bands.push(pack_outline(outline));
        }
    }
    let boundary = top_layer.finish(bands.len() as u32);
    (bands, boundary)
}

/// Pack a view's node list into the flat box-shadow instance blob, in paint
/// order (styling-f-tier.md § 2.2 — C6-b). One [`PackedInstance`] per
/// [`ExtractedShadow`](crate::render::extract::ExtractedShadow) term across all
/// nodes, in node-walk order then CSS
/// list order within a node (index 0 frontmost). A node with no shadow
/// contributes nothing. The shadow primitive draws BEHIND the box (the
/// `(Shadow, layer)` bucket has the lowest `paint_order`), so the shadow blob
/// is drawn FIRST in `node.rs`, before the quad/glyph/band draws.
///
/// Reuses the frozen 68 B [`PackedInstance`] (radius slot → blur sigma — no
/// stride change). Like the band, v1 rides the FLAT window draw only (no
/// effect-group partitioning).
///
/// Returns the blob PLUS the base↔top-layer boundary (top-layer stacking
/// composite, § 3.2): the instance index of the first top-layer caster's first
/// SQUARE shadow term, or the shadow count when no top-layer caster has one. The
/// per-block draw (W2) draws base shadows `[0..boundary)` then top-layer shadows
/// `[boundary..)`. A tail-contiguity `debug_assert` ([`TopLayerBoundaryTracker`])
/// guards the § 3.4 invariant.
pub fn pack_shadow_instances(nodes: &[ExtractedNode]) -> (Vec<PackedInstance>, u32) {
    let mut shadows = Vec::new();
    let mut top_layer = TopLayerBoundaryTracker::default();
    for n in nodes {
        top_layer.observe(n, shadows.len() as u32);
        for s in &n.shadows {
            // SQUARE terms only (F4b-6): a `radius > 0` term is a rounded caster's
            // shadow and rides the distinct rounded pipeline instead (a square
            // caster's terms are all `radius == 0` ⇒ byte-identical to before).
            if s.radius <= 0.0 {
                shadows.push(pack_shadow(s));
            }
        }
    }
    let boundary = top_layer.finish(shadows.len() as u32);
    (shadows, boundary)
}

/// Pack the ROUNDED box-shadow instances for a frame (F4b-6): every shadow term
/// whose caster has a corner radius (`radius > 0`), into the distinct
/// [`RoundedShadowInstance`](crate::render::instance::RoundedShadowInstance) the
/// rounded-shadow pipeline draws. The parallel of
/// [`pack_shadow_instances`] for the rounded record; the two partition a node's
/// shadow terms by `radius` so no term is drawn twice and the square path stays
/// byte-stable. Returns the blob PLUS its base↔top-layer boundary (the rounded
/// mirror of [`pack_shadow_instances`]'s boundary — same § 3.2 semantics + § 3.4
/// tripwire, over the ROUNDED-caster terms).
pub fn pack_rounded_shadow_instances(
    nodes: &[ExtractedNode],
) -> (Vec<crate::render::instance::RoundedShadowInstance>, u32) {
    use crate::render::instance::pack_rounded_shadow;
    let mut shadows = Vec::new();
    let mut top_layer = TopLayerBoundaryTracker::default();
    for n in nodes {
        top_layer.observe(n, shadows.len() as u32);
        for s in &n.shadows {
            if s.radius > 0.0 {
                shadows.push(pack_rounded_shadow(s));
            }
        }
    }
    let boundary = top_layer.finish(shadows.len() as u32);
    (shadows, boundary)
}

/// Pack a view's node list into the flat background-gradient instance blob, in
/// paint order (parity Wave B1). One [`GradientInstance`] per
/// [`ExtractedGradient`](crate::render::extract::ExtractedGradient) across all
/// nodes, in node-walk order then the producer's back-to-front layer order
/// within a node. A node with no gradient layers contributes nothing. The
/// gradient draws AFTER the quad (over the solid fill), BEFORE glyphs/bands, so
/// the gradient blob is drawn after the quad in `node.rs`.
///
/// Its OWN `GradientInstance` layout (the 68 B quad stride is untouched). Like
/// the band/shadow, v1 rides the FLAT window draw only (no effect-group
/// partitioning — the common case is a top-level gradient widget).
///
/// Returns the gradient blob PLUS a parallel `anchors` vec (one entry per emitted
/// gradient instance): `anchors[i]` is the quad-blob index `node_quad_anchors`
/// recorded for the node that emitted gradient `i` — its paint-order draw
/// position (after the node's own quad, before its descendants'). `node.rs`
/// interleaves the gradient blob with the flat quad runs by these so an ancestor
/// node's gradient never overpaints a descendant's opaque fill (parity bleed
/// fix). `node_quad_anchors` MUST be the [`PackedPartition::node_quad_anchors`]
/// from the SAME node walk (one entry per input node); a node missing an anchor
/// (length mismatch — never in practice) falls back to anchor `0`.
pub fn pack_gradient_instances(
    nodes: &[ExtractedNode],
    node_quad_anchors: &[u32],
) -> (Vec<GradientInstance>, Vec<u32>) {
    let mut gradients = Vec::new();
    let mut anchors = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        let anchor = node_quad_anchors.get(i).copied().unwrap_or(0);
        for g in &n.gradients {
            gradients.push(pack_gradient(g));
            anchors.push(anchor);
        }
    }
    (gradients, anchors)
}

/// One draw step in the interleaved flat window pass ([`interleave_flat_draw`]).
/// The flat pass binds the quad pipeline for a [`Quads`](Self::Quads) step, the
/// gradient pipeline for a [`Gradients`](Self::Gradients) step, and the raster
/// pipeline for a [`Raster`](Self::Raster) step; the quad/gradient variants hold
/// the instance-index sub-range to `draw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatDrawStep {
    /// Draw this sub-range of the flat quad instance blob (the quad pipeline).
    Quads(Range<u32>),
    /// Draw this sub-range of the background-gradient instance blob (the
    /// gradient pipeline).
    Gradients(Range<u32>),
    /// Draw ONE raster (drawing-canvas) node here — the `u32` indexes the
    /// caller's anchor-sorted raster draw list (`node.rs` binds that draw's
    /// per-node `@group(1)` image + the raster pipeline and draws its single
    /// instance). F4a: a raster splices at its OWN node's `node_quad_anchor`, so
    /// it paints OVER every quad/gradient that precedes its node in paint order
    /// and UNDER every one that follows — its true stacking position, with no
    /// top-layer special case and no contiguous-suffix assumption.
    Raster(u32),
}

/// Interleave the flat quad runs with the background-gradient blob in PAINT
/// ORDER (parity gradient-bleed fix). Each gradient `i` draws at its `anchor`
/// (`gradient_anchors[i]` — the quad-blob index just after its node's own quad),
/// so a node's gradient paints after that node's own fill and BEFORE any
/// descendant's quad: an ancestor's gradient layer (e.g. the viewport
/// dotted-grid) never overpaints a descendant card's opaque fill.
///
/// `flat_ranges` are the non-group quad runs the flat window pass draws
/// (gradients still ride the flat draw only — no effect-group off-screen
/// partition, the documented v1 contract). `gradient_anchors` is non-decreasing
/// (gradients are emitted in node-walk paint order, and all gradients on one
/// node share its anchor), so the walk is a single forward sweep: draw every
/// not-yet-drawn flat quad with blob-index `< anchor`, then the gradient, repeat,
/// then the remaining flat quads. A gradient whose anchor falls inside a group
/// gap (its node's quad is in an off-screen group range, not flat) draws right
/// after the last flat quad before the gap — the existing "gradient on a grouped
/// element" limitation, unchanged. Empty `gradient_anchors` ⇒ the steps are just
/// the flat quad runs (byte-for-byte the pre-fix flat draw).
///
/// Pure (no GPU / ECS) — unit-tested headless; `node.rs` executes the returned
/// schedule against the open render pass.
pub fn interleave_flat_quads_and_gradients(
    flat_ranges: &[Range<u32>],
    gradient_anchors: &[u32],
) -> Vec<FlatDrawStep> {
    // The gradients-only projection of the general interleave: NO raster canvas.
    // Kept as its own entry so the large existing gradient-interleave suite (and
    // any gradient-only caller) reads unchanged; empty raster anchors make it
    // byte-identical to `interleave_flat_draw` by construction (the raster
    // splices never fire — the F4a byte-stability contract for a non-raster view).
    interleave_flat_draw(flat_ranges, gradient_anchors, &[])
}

/// Push `Quads` steps for every not-yet-drawn flat instance with index `< limit`,
/// advancing the `(fi, pos)` cursor across the ascending, disjoint flat runs.
/// Hoisted out of [`interleave_flat_draw`] so the gradient walk AND the raster
/// splices share ONE monotonic cursor (a group gap is jumped once, no flat quad
/// is drawn twice or skipped). `limit == u32::MAX` drains every remaining run.
fn emit_quads_up_to(
    flat_ranges: &[Range<u32>],
    fi: &mut usize,
    pos: &mut u32,
    limit: u32,
    steps: &mut Vec<FlatDrawStep>,
) {
    while *fi < flat_ranges.len() {
        let r = &flat_ranges[*fi];
        if *pos < r.start {
            *pos = r.start; // jump the group gap to the next flat run
        }
        if r.start >= limit {
            break; // this run begins at/after the limit — nothing more yet
        }
        let end = r.end.min(limit);
        if *pos < end {
            steps.push(FlatDrawStep::Quads(*pos..end));
            *pos = end;
        }
        if r.end <= limit {
            *fi += 1; // run fully drawn — advance
        } else {
            break; // run drawn up to the limit — resume here next time
        }
    }
}

/// Interleave the flat quad runs with the background-gradient blob AND the raster
/// (drawing-canvas) draws in PAINT ORDER (the parity gradient-bleed fix EXTENDED
/// by the F4a general per-raster interleave). Each gradient `i` draws at its
/// `gradient_anchors[i]` and each raster `k` at its `raster_anchors[k]` — both the
/// quad-blob index just after that node's own quad, so the primitive paints after
/// its node's own fill and BEFORE any descendant's quad. This retires the
/// prototype's top-layer-suffix split: a raster now paints at its TRUE stacking
/// position, so a non-top-layer overlay draws over the canvas and an OPAQUE
/// top-layer modal panel that contains a raster shows it (no contiguous-suffix
/// assumption). See [`FlatDrawStep::Raster`] for the effect-group boundary this
/// does NOT cross.
///
/// Both anchor slices MUST be non-decreasing: gradients are emitted in node-walk
/// paint order (so they ascend naturally); the caller SORTS the raster draws by
/// anchor before calling. At a SHARED anchor, gradients (a node's background
/// layer) paint BEFORE rasters (a node's content) — a deterministic tie-break for
/// what is a non-case in practice (rasters and gradients live on disjoint nodes).
///
/// `flat_ranges` are the non-group quad runs the flat window pass draws (both
/// gradients and rasters ride the flat draw only — a gradient/raster on a grouped
/// element stays the documented v1 follow-up). A gradient/raster whose anchor
/// falls inside a group gap draws right after the last flat quad before the gap
/// (unchanged). Empty `raster_anchors` ⇒ byte-identical to
/// [`interleave_flat_quads_and_gradients`]; empty both ⇒ the plain flat quad runs.
///
/// Pure (no GPU / ECS) — unit-tested headless; `node.rs` executes the returned
/// schedule against the open render pass.
pub fn interleave_flat_draw(
    flat_ranges: &[Range<u32>],
    gradient_anchors: &[u32],
    raster_anchors: &[u32],
) -> Vec<FlatDrawStep> {
    let mut steps: Vec<FlatDrawStep> = Vec::new();
    // Quad cursor across the ascending, disjoint flat runs: `fi` is the current
    // range index, `pos` the next undrawn instance index within it.
    let mut fi = 0usize;
    let mut pos = flat_ranges.first().map_or(0, |r| r.start);
    // The next raster draw to splice (anchors ascending, caller-sorted).
    let mut ri = 0usize;

    for (gi, &anchor) in gradient_anchors.iter().enumerate() {
        // Splice every raster STRICTLY before this gradient's anchor. A raster at
        // the SAME anchor paints AFTER the gradient (content over background) — it
        // is placed by a later gradient with a greater anchor, or the tail loop.
        while ri < raster_anchors.len() && raster_anchors[ri] < anchor {
            emit_quads_up_to(
                flat_ranges,
                &mut fi,
                &mut pos,
                raster_anchors[ri],
                &mut steps,
            );
            steps.push(FlatDrawStep::Raster(ri as u32));
            ri += 1;
        }
        emit_quads_up_to(flat_ranges, &mut fi, &mut pos, anchor, &mut steps);
        let g = gi as u32;
        // Coalesce consecutive gradients (same or non-increasing anchors) into
        // one run so the pass binds the gradient pipeline once for the group. A
        // raster spliced between two gradients breaks the run (the last step is a
        // Raster, not a Gradients) — correct, and the byte-stable path when
        // `raster_anchors` is empty (the splice loop never runs).
        match steps.last_mut() {
            Some(FlatDrawStep::Gradients(run)) if run.end == g => run.end = g + 1,
            _ => steps.push(FlatDrawStep::Gradients(g..g + 1)),
        }
    }
    // Rasters anchored at/after the last gradient (over every gradient), each
    // before the quads that follow its node in paint order.
    while ri < raster_anchors.len() {
        emit_quads_up_to(
            flat_ranges,
            &mut fi,
            &mut pos,
            raster_anchors[ri],
            &mut steps,
        );
        steps.push(FlatDrawStep::Raster(ri as u32));
        ri += 1;
    }
    emit_quads_up_to(flat_ranges, &mut fi, &mut pos, u32::MAX, &mut steps);
    steps
}

/// The instance-range partition of a packed view (effect-compositor.md § 1.1 /
/// decided fork 3): the flat quad blob plus, per effect group, the contiguous
/// `[start, end)` instance range its members occupy, and the complement
/// (non-group) ranges the flat window draw covers. Drawn off `ExtractedNode.group`:
/// the packer skips transparent nodes (no instance), so the indices here are
/// INSTANCE indices, not node indices — the only correct partition key for the
/// draw. `group_ranges[g]` is the instance range for group index `g`; a group
/// with no opaque members is `start == end` (empty, never drawn). `flat_ranges`
/// is every maximal run of consecutive non-group instances.
///
/// Contiguity holds by construction: every SC-forming effect former is a
/// stacking context (layout 6f — the spec § 2 trigger-5 clause for
/// `opacity`/`filter`/`mix-blend-mode`, trigger 2 for `isolation`), so a
/// group's subtree is one atomic `painters_z` entry; extract emits that
/// subtree as a contiguous paint-order run, and the packer preserves node
/// order within the single `(Quad, 0)` batch, so the instance run stays
/// contiguous. A degenerate interleaving (a group's run split by a
/// non-member) would surface as a non-`start..end`-contiguous range and is
/// asserted against below (a tripwire, not an expected state — see the
/// comment at the assert).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PackedPartition {
    /// The full flat quad blob (every instance, in paint order) — identical to
    /// `pack_view`'s single `(Quad, 0)` batch flattened.
    pub instances: Vec<[f32; 17]>,
    /// `group_ranges[g]` = the `[start, end)` instance range of group `g`'s
    /// members (empty range if the group has no opaque member).
    pub group_ranges: Vec<std::ops::Range<u32>>,
    /// Maximal runs of consecutive NON-group instances — the flat window draw.
    pub flat_ranges: Vec<std::ops::Range<u32>>,
    /// `node_quad_anchors[i]` = the quad-instance-blob index immediately AFTER
    /// input node `i`'s own solid quad (and BEFORE its spliced text quads). This
    /// is the paint-order ANCHOR a node's background gradient draws at: the node
    /// draws (or skips) its own quad, the gradient paints right after it, then
    /// the node's text quads and every descendant follow. `pack_gradient_instances`
    /// tags each gradient with its node's anchor, and `node.rs` interleaves the
    /// gradient blob with [`flat_ranges`](Self::flat_ranges) by these — so an
    /// ANCESTOR's gradient never overpaints a DESCENDANT's opaque fill (parity
    /// gradient-bleed bug). One entry per input node, in node-walk (paint) order,
    /// so it is non-decreasing.
    pub node_quad_anchors: Vec<u32>,
    /// #2 Stage D1: entity -> its quad-instance slot in `instances` (only painting
    /// nodes — a `Color::NONE` node has no quad). Rebuilt every full pack. A Patch
    /// (provably stable paint order — no structural change) reuses this to overwrite
    /// just the changed entity's quad slot via `RawBufferVec::set` +
    /// `write_buffer_range`, instead of re-uploading the whole blob. (The general
    /// rejection of a recorded paint-order index — see the `quads_by_entity` note in
    /// `pack_view_partitioned` — is about REORDER staleness, which a Patch precludes.)
    pub quad_slot_of: EntityHashMap<u32>,
    /// F4a: entity -> its `node_quad_anchor` (the quad-blob index just after the
    /// entity's own quad — the SAME value as its [`node_quad_anchors`] entry).
    /// UNLIKE [`quad_slot_of`] this covers EVERY node, painting or `Color::NONE`,
    /// because a raster node usually paints no background quad yet still needs its
    /// anchor to splice at (the F4a per-raster interleave). `node.rs` joins each
    /// extracted raster (which knows only its entity) to its paint-order position
    /// through this map. Rebuilt every full pack; retained across a Patch (a Patch
    /// never reorders, so the anchors stay valid).
    ///
    /// [`node_quad_anchors`]: Self::node_quad_anchors
    /// [`quad_slot_of`]: Self::quad_slot_of
    pub node_quad_anchor_of: EntityHashMap<u32>,
    /// The base↔top-layer boundary of the flat quad blob (top-layer stacking
    /// composite, § 3.2): the instance index of the first top-layer node's first
    /// quad. `[0..top_layer_boundary)` is the base block, `[top_layer_boundary..
    /// instances.len())` the top-layer block — the per-block draw restructure (W2)
    /// draws the base block's complete tier-stack, then the top-layer block's over
    /// it, so a top-layer subtree occludes base text/icons/borders, not just fills.
    /// Equals `instances.len()` when the view has no top-layer node (an empty
    /// top-layer block — the byte-stable path). The quad packer records it off
    /// `ExtractedNode.top_layer` (the flag rides the record), guarded by a
    /// tail-contiguity `debug_assert` ([`TopLayerBoundaryTracker`]).
    pub top_layer_boundary: u32,
}

/// Pack a view's nodes into the flat quad blob AND its per-group instance-range
/// partition ([`PackedPartition`]). `group_count` is the length of the per-view
/// effect-group list (so a group with zero opaque members still gets an empty
/// range slot at its index). The flat draw uses `flat_ranges`; each group pass
/// uses `group_ranges[g]`. See [`PackedPartition`] for the contiguity contract.
///
/// `text_quads` is text's quad-tier carrier (decoration-and-paint § 4.6 —
/// underline/overline now, selection rects in T7): each entity's quads are
/// spliced into the blob IMMEDIATELY after that entity's own node instance
/// (or its `Color::NONE` skip), adopting the node's group, so within-entity
/// § 4.4 order and the partition contiguity hold by construction. With an
/// empty carrier the output is byte-identical to the pre-T6 pack.
pub fn pack_view_partitioned(
    nodes: &[ExtractedNode],
    group_count: usize,
    text_quads: &[TextQuad],
) -> PackedPartition {
    // Entity → contiguous carrier range (§ 4.6's "entity→quads lookup over
    // the flat carrier"), rebuilt per pack — all ordering derives from the
    // FRESH node walk below, so retained quads land correctly even on
    // frames where the node list rebuilt for non-text reasons (fact (b);
    // a recorded paint-order index would go stale — the rejected round-1
    // merge key).
    let mut quads_by_entity: HashMap<Entity, Range<usize>> = HashMap::new();
    for (i, q) in text_quads.iter().enumerate() {
        match quads_by_entity.entry(q.entity) {
            Entry::Vacant(slot) => {
                slot.insert(i..i + 1);
            }
            Entry::Occupied(mut range) => {
                debug_assert_eq!(
                    range.get().end,
                    i,
                    "ExtractedTextQuads must be entity-grouped (the producer \
                     emits each entity's quads contiguously — § 4.6)"
                );
                range.get_mut().end = i + 1;
            }
        }
    }

    let mut p = Partitioner::new(nodes.len() + text_quads.len(), group_count);
    let mut node_quad_anchors = Vec::with_capacity(nodes.len());
    let mut quad_slot_of: EntityHashMap<u32> = EntityHashMap::default();
    let mut node_quad_anchor_of: EntityHashMap<u32> = EntityHashMap::default();
    let mut top_layer = TopLayerBoundaryTracker::default();
    for node in nodes {
        // Record the base↔top-layer boundary at the first top-layer node's first
        // instance (its own quad, or — for a `Color::NONE` node — its first text
        // quad), before pushing any of this node's instances. The tracker's
        // tail-contiguity `debug_assert` fires if a base node follows a top-layer
        // one (§ 3.4). Text quads inherit their anchoring node's classification
        // (they splice in the same iteration), so no separate handling is needed.
        top_layer.observe(node, p.len());
        let g = node.group.filter(|&g| g < group_count);
        if node.color != Color::NONE {
            // D1: record this painting node's quad slot (the index it is about to
            // occupy) so the partial-upload Patch path can overwrite it in place.
            quad_slot_of.insert(node.entity, p.len());
            p.push(packed_to_raw(&pack_extracted(node)), g);
        }
        // The gradient paint-order anchor (parity gradient-bleed fix): the
        // instance count right AFTER this node's own quad (or its `Color::NONE`
        // skip) and BEFORE its text quads. The node's background gradient draws
        // here — over its own fill, under its own decorations + every descendant.
        let anchor = p.len();
        node_quad_anchors.push(anchor);
        // F4a: the same anchor, keyed by entity, so an extracted raster (which
        // knows only its entity) can join to its paint-order splice position.
        // Every node lands here — a `Color::NONE` raster node has no quad_slot but
        // still needs its anchor.
        node_quad_anchor_of.insert(node.entity, anchor);
        // § 4.6: splice the entity's text quads IMMEDIATELY after its node
        // record, adopting the node's group — partition placement can never
        // disagree with the entity's, so contiguity holds by construction.
        // A quad whose entity has no node record this pack is dropped
        // silently (a transient impossibility — both trigger unions fire on
        // every entity-set change; decision 7).
        if let Some(range) = quads_by_entity.get(&node.entity) {
            for quad in &text_quads[range.clone()] {
                if quad.color == Color::NONE {
                    continue;
                }
                p.push(packed_to_raw(&pack_text_quad(quad)), g);
            }
        }
    }
    let top_layer_boundary = top_layer.finish(p.len());
    let mut partition = p.finish();
    partition.node_quad_anchors = node_quad_anchors;
    partition.quad_slot_of = quad_slot_of;
    partition.node_quad_anchor_of = node_quad_anchor_of;
    partition.top_layer_boundary = top_layer_boundary;
    partition
}

/// The run bookkeeping of [`pack_view_partitioned`], hoisted out of the node
/// loop so a node's own instance and its spliced text quads share it: the
/// instance blob paired with the shared [`RangePartitioner`] that tracks the
/// per-group contiguous ranges (with the contiguity tripwire) and the
/// complement flat runs.
struct Partitioner {
    instances: Vec<[f32; 17]>,
    ranges: RangePartitioner,
}

impl Partitioner {
    fn new(capacity: usize, group_count: usize) -> Self {
        Self {
            instances: Vec::with_capacity(capacity),
            ranges: RangePartitioner::new(group_count),
        }
    }

    /// Append one instance under group `g` (already bounds-filtered by the
    /// caller), extending or starting the group/flat run it belongs to.
    fn push(&mut self, instance: [f32; 17], g: Option<usize>) {
        self.instances.push(instance);
        self.ranges.push(g);
    }

    /// The number of instances pushed so far — the running quad-blob index a
    /// gradient anchor is read from (between a node's own quad and its text quads).
    fn len(&self) -> u32 {
        self.instances.len() as u32
    }

    fn finish(self) -> PackedPartition {
        let (group_ranges, flat_ranges) = self.ranges.finish();
        // The no-top-layer default (empty top-layer block); `pack_view_partitioned`
        // overwrites it with the tracked boundary. `Partitioner` stays top-layer-
        // unaware — the flag lives on `ExtractedNode`, not the range bookkeeping.
        let top_layer_boundary = self.instances.len() as u32;
        PackedPartition {
            instances: self.instances,
            group_ranges,
            flat_ranges,
            // Filled by `pack_view_partitioned` after `finish` (it owns the
            // per-node anchor walk); empty here keeps `Partitioner` blob-only.
            node_quad_anchors: Vec::new(),
            quad_slot_of: EntityHashMap::default(),
            node_quad_anchor_of: EntityHashMap::default(),
            top_layer_boundary,
        }
    }
}

/// The instance-index run bookkeeping shared by the quad packer
/// ([`Partitioner`]) and the glyph partition ([`partition_glyph_ranges`]):
/// per-group contiguous ranges (with the contiguity tripwire) and the
/// complement flat runs. Blob-free — it tracks indices only, so the glyph
/// path (whose instances already live in `ExtractedGlyphs`) reuses the exact
/// quad semantics without copying bytes.
pub(crate) struct RangePartitioner {
    next: u32,
    group_ranges: Vec<Range<u32>>,
    flat_ranges: Vec<Range<u32>>,
    /// Tracks the group of the previous index to coalesce contiguous runs.
    run_group: Option<Option<usize>>,
}

impl RangePartitioner {
    pub(crate) fn new(group_count: usize) -> Self {
        Self {
            next: 0,
            group_ranges: vec![0..0; group_count],
            flat_ranges: Vec::new(),
            run_group: None,
        }
    }

    /// Claim the next instance index under group `g` (already bounds-filtered
    /// by the caller), extending or starting the group/flat run it belongs to.
    pub(crate) fn push(&mut self, g: Option<usize>) {
        let idx = self.next;
        self.next += 1;
        match g {
            Some(gi) => {
                let r = &mut self.group_ranges[gi];
                if r.start == r.end {
                    *r = idx..idx + 1; // first member of this group
                } else {
                    // CONTIGUITY INVARIANT (the off-screen composite's load-bearing
                    // assumption): a group's members must be a contiguous run in
                    // paint order, so the group draws as ONE [start,end) slice into
                    // its target and the flat draw is the exact complement. This
                    // holds when the group is ATOMIC — and it now holds BY
                    // CONSTRUCTION: layout sub-pass 6f forms a stacking context for
                    // every SC-forming effect former (`opacity < 1` / `filter` /
                    // `mix-blend-mode` via the spec § 2 trigger-5 clause, landed;
                    // `isolation` via trigger 2), so a former's subtree is one
                    // atomic painters_z entry and nothing non-descendant can paint
                    // between its members. The SC trigger and the group-former
                    // predicate share one source of truth
                    // (`render::effect::effect_reason_for` /
                    // `forms_render_stacking_context`) precisely so they cannot
                    // drift apart. The assert stays as a TRIPWIRE for the two ways
                    // the invariant could still break: predicate drift, and a
                    // `backdrop-filter`-ONLY group (the one former that is
                    // deliberately EffectGroup-but-not-SC — reserved, no v1 shader)
                    // whose z-indexed member interleaves a non-member. A
                    // single-range partition cannot express interleaving
                    // (supporting it would bake in NON-atomic semantics, which is
                    // wrong) — catch it loudly rather than silently double-painting
                    // the spanned non-member. Text quads cannot trip it: they
                    // adopt their anchoring node's group at the same position.
                    // GPU regression: tests/render_group_contiguity_gpu.rs.
                    debug_assert_eq!(
                        r.end, idx,
                        "effect group {gi} is non-contiguous in paint order (gap \
                         before instance {idx}): a group member painted outside its \
                         former's stacking context — either the trigger-5 SC clause \
                         (layout forms_stacking_context) drifted from the \
                         effect-group former predicate (effect_reason_for), or a \
                         backdrop-filter-only group (EffectGroup-but-not-SC) \
                         interleaved a non-member."
                    );
                    r.end = idx + 1; // contiguous extension
                }
            }
            None => {
                if self.run_group == Some(None) {
                    self.flat_ranges.last_mut().expect("open flat run").end = idx + 1;
                } else {
                    self.flat_ranges.push(idx..idx + 1);
                }
            }
        }
        self.run_group = Some(g);
    }

    pub(crate) fn finish(self) -> (Vec<Range<u32>>, Vec<Range<u32>>) {
        (self.group_ranges, self.flat_ranges)
    }
}

/// Partition the glyph instance buffer into per-effect-group contiguous
/// ranges + the flat complement (T8 — the quad path's
/// [`pack_view_partitioned`] partition applied to glyphs). `runs` is the
/// producer's per-entity attribution (`ExtractedGlyphs::entity_runs` as
/// `(entity, instance range)` pairs — carrier-agnostic so this module stays
/// decoupled from the carrier type), `total` the instance count, and
/// `group_of` resolves an entity to its `ExtractedNode.group` off the FRESH
/// node list (decoration-and-paint § 4.6: membership derives from the node
/// record at pack time, never from recorded indices — stale-proof by
/// construction). Contiguity per group holds because the glyph producer walks
/// the SAME `context_tree_paint_order` as the node extract (an SC-forming
/// group's subtree is one atomic run in both); the `RangePartitioner`
/// tripwire guards the residual drift cases exactly as it does for quads.
///
/// An entity `group_of` cannot resolve maps to FLAT — a transient
/// impossibility (despawn/paint-skip fire BOTH probe unions, so the two
/// carriers rebuild together; fact (a): every painted entity has a node
/// record), kept as the conservative fallback rather than a drop because the
/// instances are already in the buffer.
pub fn partition_glyph_ranges(
    runs: impl IntoIterator<Item = (Entity, Range<u32>)>,
    total: u32,
    group_count: usize,
    group_of: impl Fn(Entity) -> Option<usize>,
) -> (Vec<Range<u32>>, Vec<Range<u32>>) {
    let mut p = RangePartitioner::new(group_count);
    let mut covered = 0u32;
    for (entity, range) in runs {
        debug_assert_eq!(
            range.start, covered,
            "entity runs must be contiguous from 0 (the producer emits one \
             run per entity, gapless, in emission order)"
        );
        covered = range.end;
        let g = group_of(entity).filter(|&g| g < group_count);
        for _ in range {
            p.push(g);
        }
    }
    debug_assert_eq!(
        covered, total,
        "entity runs must cover every glyph instance"
    );
    p.finish()
}
