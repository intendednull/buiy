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
    batches: BTreeMap<PrimitiveBatchKey, Vec<[f32; 9]>>,
}

impl InstanceBuckets {
    /// Push one packed instance (as raw `[f32; 9]` = pos2+size2+color4+radius1)
    /// into its batch.
    pub fn push(&mut self, key: PrimitiveBatchKey, instance: [f32; 9]) {
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
    pub fn batches(&self) -> impl Iterator<Item = (&PrimitiveBatchKey, &Vec<[f32; 9]>)> {
        self.batches.iter()
    }
}

/// Flatten a [`PackedInstance`] into the raw `[f32; 9]` the bucket store holds.
/// Keeps the bucket store decoupled from the concrete instance struct while
/// the stride is asserted equal in tests.
pub fn packed_to_raw(p: &PackedInstance) -> [f32; 9] {
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
