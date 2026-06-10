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

use std::collections::BTreeMap;

use crate::render::extract::ExtractedNode;
use crate::render::instance::{PackedInstance, pack_extracted};
use bevy::prelude::Color;
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
    batches: BTreeMap<PrimitiveBatchKey, Vec<[f32; 13]>>,
}

impl InstanceBuckets {
    /// Push one packed instance (as raw `[f32; 13]` =
    /// pos2+size2+color4+radius1+clip_min2+clip_max2) into its batch.
    pub fn push(&mut self, key: PrimitiveBatchKey, instance: [f32; 13]) {
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
    pub fn batches(&self) -> impl Iterator<Item = (&PrimitiveBatchKey, &Vec<[f32; 13]>)> {
        self.batches.iter()
    }
}

/// Flatten a [`PackedInstance`] into the raw `[f32; 13]` the bucket store holds.
/// Keeps the bucket store decoupled from the concrete instance struct while
/// the stride is asserted equal in tests.
pub fn packed_to_raw(p: &PackedInstance) -> [f32; 13] {
    [
        p.rect_pos[0],
        p.rect_pos[1],
        p.rect_size[0],
        p.rect_size[1],
        p.color[0],
        p.color[1],
        p.color[2],
        p.color[3],
        p.radius,
        p.clip_min[0],
        p.clip_min[1],
        p.clip_max[0],
        p.clip_max[1],
    ]
}

// Asserts at compile time (via a const fn caller in tests) that the raw layout
// matches the struct stride. `_pod` is here so a future non-f32 record forces
// a conscious change to `packed_to_raw`.
const _ASSERT_POD: fn() = || {
    fn _is_pod<T: Pod>() {}
    _is_pod::<PackedInstance>();
};

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
    pub instances: Vec<[f32; 13]>,
    /// `group_ranges[g]` = the `[start, end)` instance range of group `g`'s
    /// members (empty range if the group has no opaque member).
    pub group_ranges: Vec<std::ops::Range<u32>>,
    /// Maximal runs of consecutive NON-group instances — the flat window draw.
    pub flat_ranges: Vec<std::ops::Range<u32>>,
}

/// Pack a view's nodes into the flat quad blob AND its per-group instance-range
/// partition ([`PackedPartition`]). `group_count` is the length of the per-view
/// effect-group list (so a group with zero opaque members still gets an empty
/// range slot at its index). The flat draw uses `flat_ranges`; each group pass
/// uses `group_ranges[g]`. See [`PackedPartition`] for the contiguity contract.
pub fn pack_view_partitioned(nodes: &[ExtractedNode], group_count: usize) -> PackedPartition {
    let mut instances: Vec<[f32; 13]> = Vec::with_capacity(nodes.len());
    let mut group_ranges: Vec<std::ops::Range<u32>> = vec![0..0; group_count];
    let mut flat_ranges: Vec<std::ops::Range<u32>> = Vec::new();
    // Tracks the group of the previous instance to coalesce contiguous runs.
    let mut run_group: Option<Option<usize>> = None;
    for node in nodes {
        if node.color == Color::NONE {
            continue;
        }
        let idx = instances.len() as u32;
        instances.push(packed_to_raw(&pack_extracted(node)));
        let g = node.group.filter(|&g| g < group_count);
        // Extend or start the group/flat run this instance belongs to.
        match g {
            Some(gi) => {
                let r = &mut group_ranges[gi];
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
                    // the spanned non-member. GPU regression:
                    // tests/render_group_contiguity_gpu.rs.
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
                if run_group == Some(None) {
                    flat_ranges.last_mut().expect("open flat run").end = idx + 1;
                } else {
                    flat_ranges.push(idx..idx + 1);
                }
            }
        }
        run_group = Some(g);
    }
    PackedPartition {
        instances,
        group_ranges,
        flat_ranges,
    }
}
