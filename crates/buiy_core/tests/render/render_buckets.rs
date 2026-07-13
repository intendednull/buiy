//! Pure-CPU tests for typed-primitive `(primitive, layer)` bucketing. No GPU
//! adapter required (HEADLESS half of the prepare phase).

use buiy_core::render::buckets::{BuiyPrimitiveKind, InstanceBuckets, PrimitiveBatchKey};

#[test]
fn primitive_paint_order_is_shadow_quad_glyph_path() {
    // architecture.md § 2.2: within a layer, back-to-front by type.
    assert!(BuiyPrimitiveKind::Shadow.paint_order() < BuiyPrimitiveKind::Quad.paint_order());
    assert!(BuiyPrimitiveKind::Quad.paint_order() < BuiyPrimitiveKind::Glyph.paint_order());
    assert!(BuiyPrimitiveKind::Glyph.paint_order() < BuiyPrimitiveKind::Path.paint_order());
}

#[test]
fn batch_keys_sort_by_layer_then_primitive() {
    let mut keys = [
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Quad,
            layer: 1,
        },
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Shadow,
            layer: 1,
        },
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Path,
            layer: 0,
        },
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Quad,
            layer: 0,
        },
    ];
    keys.sort();
    // layer 0 before layer 1; within a layer, paint order (shadow<quad<...).
    assert_eq!(
        keys[0],
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Quad,
            layer: 0
        }
    );
    assert_eq!(
        keys[1],
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Path,
            layer: 0
        }
    );
    assert_eq!(
        keys[2],
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Shadow,
            layer: 1
        }
    );
    assert_eq!(
        keys[3],
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Quad,
            layer: 1
        }
    );
}

#[test]
fn buckets_group_pushed_instances_by_key() {
    let mut b = InstanceBuckets::default();
    let q0 = PrimitiveBatchKey {
        primitive: BuiyPrimitiveKind::Quad,
        layer: 0,
    };
    let s0 = PrimitiveBatchKey {
        primitive: BuiyPrimitiveKind::Shadow,
        layer: 0,
    };
    b.push(q0, [0.0; 17]);
    b.push(q0, [1.0; 17]);
    b.push(s0, [2.0; 17]);
    assert_eq!(b.len(q0), 2);
    assert_eq!(b.len(s0), 1);
    assert_eq!(b.total_instances(), 3);
    // A key never pushed to has no batch.
    assert_eq!(
        b.len(PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Path,
            layer: 0
        }),
        0
    );
}

#[test]
fn buckets_iterate_in_paint_order() {
    let mut b = InstanceBuckets::default();
    b.push(
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Quad,
            layer: 0,
        },
        [0.0; 17],
    );
    b.push(
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Shadow,
            layer: 0,
        },
        [0.0; 17],
    );
    b.push(
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Quad,
            layer: 1,
        },
        [0.0; 17],
    );
    let order: Vec<_> = b.batches().map(|(k, _)| *k).collect();
    // shadow@0, quad@0, then quad@1 — sorted ascending.
    assert_eq!(order[0].primitive, BuiyPrimitiveKind::Shadow);
    assert_eq!(order[0].layer, 0);
    assert_eq!(order[1].primitive, BuiyPrimitiveKind::Quad);
    assert_eq!(order[1].layer, 0);
    assert_eq!(order[2].layer, 1);
}

use bevy::prelude::*;
use buiy_core::render::buckets::pack_view;
use buiy_core::render::extract::ExtractedNode;
use buiy_core::render::instance::{pack_extracted, packed_raw_stride_agrees};
use buiy_verify::snapshot::assert_instance_hex_snapshot;

// pack_view consumes R5's ExtractedNode records (the prepare seam, Task 6) — the
// bucketing assertions below are unchanged from the DrawData era; only the input
// record type flipped.
fn node(entity: u32, position: Vec2, size: Vec2, color: Color) -> ExtractedNode {
    ExtractedNode {
        entity: Entity::from_raw_u32(entity).unwrap(),
        position,
        size,
        radius: 0.0,
        color,
        clip: None,
        group: None,
        top_layer: false,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    }
}

#[test]
fn raw_layout_stride_agrees_with_struct() {
    // The [f32;17] the bucket holds must be byte-identical in size to the
    // PackedInstance struct the pipeline descriptor declares (68 B). If this
    // ever drifts, the instanced draw reads garbage.
    assert!(packed_raw_stride_agrees());
    assert_eq!(std::mem::size_of::<[f32; 17]>(), 68);
}

#[test]
fn packed_to_raw_appends_affine_via_offset_consts() {
    // packed_to_raw returns 17 floats: the affine basis at [13..17], and the
    // alpha at ALPHA_FLOAT_OFFSET unchanged (the R2 re-tint invariant).
    use buiy_core::render::buckets::packed_to_raw;
    use buiy_core::render::instance::ALPHA_FLOAT_OFFSET;
    let mut n = node(1, Vec2::ZERO, Vec2::splat(10.0), Color::WHITE);
    n.affine = [[2.0, 0.0], [0.0, 3.0]];
    let p = pack_extracted(&n);
    let raw = packed_to_raw(&p);
    assert_eq!(raw.len(), 17);
    assert_eq!(&raw[13..17], &[2.0, 0.0, 0.0, 3.0]);
    assert_eq!(raw[ALPHA_FLOAT_OFFSET], p.color[3]);
}

#[test]
fn pack_view_routes_every_draw_to_quad_layer_0() {
    let nodes = vec![
        node(1, Vec2::ZERO, Vec2::splat(10.0), Color::WHITE),
        node(2, Vec2::splat(5.0), Vec2::splat(20.0), Color::BLACK),
    ];
    let buckets = pack_view(&nodes);
    let quad0 = buiy_core::render::buckets::PrimitiveBatchKey {
        primitive: buiy_core::render::buckets::BuiyPrimitiveKind::Quad,
        layer: 0,
    };
    assert_eq!(buckets.len(quad0), 2);
    assert_eq!(buckets.total_instances(), 2);
}

#[test]
fn pack_view_preserves_packed_values_in_order() {
    // pack_view's single batch holds each node packed verbatim. The old
    // `batch[0] == packed_to_raw(pack_extracted(node))` oracle cross-check
    // becomes a byte-exact hex snapshot of the packed payload: it pins the
    // EXACT instance bytes pack_view emits (snapshots.md § Tier 2 — the bucket
    // dump pins counts, the hex pins the payload). The asserts below still
    // prove the batch's bytes equal the packing-fn output (the preserved
    // oracle), and the hex pins what those bytes ARE.
    let nodes = vec![node(
        1,
        Vec2::new(7.0, 9.0),
        Vec2::new(3.0, 4.0),
        Color::WHITE,
    )];
    let buckets = pack_view(&nodes);
    let (_, batch) = buckets.batches().next().expect("one batch");
    let packed = pack_extracted(&nodes[0]);
    // Preserved oracle: the batch's raw row equals the packing fn's output.
    assert_eq!(batch[0], buiy_core::render::buckets::packed_to_raw(&packed));
    // Pinned payload: snapshot the exact bytes pack_view emits for this node.
    assert_instance_hex_snapshot(&packed, "pack_view_node_payload");
}

#[test]
fn pack_view_empty_input_is_empty() {
    let buckets = pack_view(&[]);
    assert!(buckets.is_empty());
    assert_eq!(buckets.total_instances(), 0);
}

#[test]
fn pack_view_skips_transparent_nodes() {
    // ExtractedNode.color == Color::NONE means transparent — extract emits no
    // quad for it downstream (the ExtractedNode.color contract; mirrors the
    // Phase-0 draw_for_node skip). `extracted_node_for` sets Color::NONE for
    // every backgroundless node, so an opaque node packs to one quad instance
    // while a transparent one packs to zero.
    let opaque = node(1, Vec2::ZERO, Vec2::splat(10.0), Color::WHITE);
    let transparent = node(2, Vec2::splat(5.0), Vec2::splat(20.0), Color::NONE);

    assert_eq!(
        pack_view(std::slice::from_ref(&opaque)).total_instances(),
        1
    );
    assert_eq!(
        pack_view(std::slice::from_ref(&transparent)).total_instances(),
        0
    );

    // Mixed: only the opaque node survives the pack seam.
    let buckets = pack_view(&[opaque, transparent]);
    assert_eq!(buckets.total_instances(), 1);
    let quad0 = buiy_core::render::buckets::PrimitiveBatchKey {
        primitive: buiy_core::render::buckets::BuiyPrimitiveKind::Quad,
        layer: 0,
    };
    assert_eq!(buckets.len(quad0), 1);
}

// --- pack_view_partitioned (effect-group double-paint exclusion) -------------

use buiy_core::render::buckets::pack_view_partitioned;

// A node tagged with an effect group (or `None` for in-flow).
fn grouped(entity: u32, color: Color, group: Option<usize>) -> ExtractedNode {
    ExtractedNode {
        entity: Entity::from_raw_u32(entity).unwrap(),
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
        radius: 0.0,
        color,
        clip: None,
        group,
        top_layer: false,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    }
}

#[test]
fn partition_no_groups_is_one_full_flat_run() {
    // With zero groups every instance is in the single flat run `0..n` — the
    // pre-compositor draw, byte-for-byte. group_ranges is empty.
    let nodes = vec![
        grouped(1, Color::WHITE, None),
        grouped(2, Color::WHITE, None),
        grouped(3, Color::WHITE, None),
    ];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.instances.len(), 3);
    assert!(p.group_ranges.is_empty());
    assert_eq!(p.flat_ranges, vec![0..3]);
}

#[test]
fn partition_all_group_members_leaves_flat_ranges_empty() {
    // The regression that shipped the double-paint: when EVERY instance is a
    // group member, flat_ranges MUST be empty (NOT a full `0..n` run) — the node
    // draws nothing flat, the composite paints the content. group 0 owns 0..2.
    let nodes = vec![
        grouped(1, Color::WHITE, Some(0)),
        grouped(2, Color::WHITE, Some(0)),
    ];
    let p = pack_view_partitioned(&nodes, 1, &[]);
    assert_eq!(p.group_ranges, vec![0..2]);
    assert!(
        p.flat_ranges.is_empty(),
        "all-group input has NO flat draw (not a full 0..n run)"
    );
}

#[test]
fn partition_group_between_flats_splits_into_three_runs() {
    // [flat][group A][flat]: the flat draw is the two outer runs; group A's range
    // is the contiguous middle. Proves the group is excluded from the flat draw.
    let nodes = vec![
        grouped(1, Color::WHITE, None),    // 0 flat
        grouped(2, Color::WHITE, Some(0)), // 1 group 0
        grouped(3, Color::WHITE, Some(0)), // 2 group 0
        grouped(4, Color::WHITE, None),    // 3 flat
    ];
    let p = pack_view_partitioned(&nodes, 1, &[]);
    assert_eq!(p.instances.len(), 4);
    assert_eq!(p.group_ranges, vec![1..3]);
    assert_eq!(p.flat_ranges, vec![0..1, 3..4]);
}

#[test]
fn partition_skips_transparent_so_indices_are_instance_indices() {
    // A transparent group member emits NO instance, so the ranges are INSTANCE
    // indices (not node indices). Here node 2 (transparent) is dropped, so group
    // 0's two opaque members (nodes 1,3) occupy instance range 0..2.
    let nodes = vec![
        grouped(1, Color::WHITE, Some(0)),
        grouped(2, Color::NONE, Some(0)), // transparent → no instance
        grouped(3, Color::WHITE, Some(0)),
        grouped(4, Color::WHITE, None), // flat
    ];
    let p = pack_view_partitioned(&nodes, 1, &[]);
    assert_eq!(p.instances.len(), 3);
    assert_eq!(p.group_ranges, vec![0..2]);
    assert_eq!(p.flat_ranges, vec![2..3]);
}

#[test]
fn partition_group_with_no_opaque_member_is_empty_range() {
    // A group whose only member is transparent has an empty `start == end` range
    // (the node skips it — no off-screen pass for a group that paints nothing).
    let nodes = vec![
        grouped(1, Color::NONE, Some(0)), // transparent group member
        grouped(2, Color::WHITE, None),   // flat
    ];
    let p = pack_view_partitioned(&nodes, 1, &[]);
    assert_eq!(p.instances.len(), 1);
    assert_eq!(p.group_ranges[0].start, p.group_ranges[0].end);
    assert_eq!(p.flat_ranges, vec![0..1]);
}

#[test]
fn partition_two_adjacent_groups_get_distinct_contiguous_ranges() {
    // [group A][group B]: each group is its own contiguous run; no flat draw.
    let nodes = vec![
        grouped(1, Color::WHITE, Some(0)),
        grouped(2, Color::WHITE, Some(0)),
        grouped(3, Color::WHITE, Some(1)),
    ];
    let p = pack_view_partitioned(&nodes, 2, &[]);
    assert_eq!(p.group_ranges, vec![0..2, 2..3]);
    assert!(p.flat_ranges.is_empty());
}

// --- partition_glyph_ranges (T8: the glyph buffer's group/flat partition) ----

use buiy_core::render::buckets::partition_glyph_ranges;
use std::ops::Range;

// One producer entity-run: `(entity, instance range)` — the carrier-agnostic
// shape of `ExtractedGlyphs::entity_runs`.
fn run(entity: u32, range: Range<u32>) -> (Entity, Range<u32>) {
    (Entity::from_raw_u32(entity).unwrap(), range)
}

/// No live group: ONE flat run covering everything — the flat glyph draw
/// stays byte-for-byte the pre-T8 `0..glyph_count` (the quad precedent).
#[test]
fn glyph_partition_no_groups_is_single_full_flat_run() {
    // `|_| false` = no top-layer entity; the 3rd return (the top-layer boundary)
    // is exercised in `toplayer_block_partition.rs` — dropped here.
    let (groups, flat, _) =
        partition_glyph_ranges([run(1, 0..3), run(2, 3..5)], 5, 0, |_| None, |_| false);
    assert!(groups.is_empty());
    assert_eq!(flat, vec![0..5]);
}

/// A grouped middle entity: its range lands in group_ranges[g]; the flat
/// complement is the two surrounding maximal runs.
#[test]
fn glyph_partition_grouped_middle_run() {
    let g = |e: Entity| (e == Entity::from_raw_u32(2).unwrap()).then_some(0);
    let (groups, flat, _) =
        partition_glyph_ranges([run(1, 0..2), run(2, 2..6), run(3, 6..9)], 9, 1, g, |_| {
            false
        });
    assert_eq!(groups, vec![2..6]);
    assert_eq!(flat, vec![0..2, 6..9]);
}

/// A group with no glyph-emitting member keeps its empty `0..0` slot at
/// its index (the group_ranges[g] == prepared group g alignment).
#[test]
fn glyph_partition_empty_group_slot() {
    let (groups, flat, _) = partition_glyph_ranges([run(1, 0..4)], 4, 2, |_| Some(1), |_| false);
    assert_eq!(groups, vec![0..0, 0..4]);
    assert!(flat.is_empty());
}

/// Adjacent same-group entities coalesce into one contiguous group range
/// (two text entities inside one card).
#[test]
fn glyph_partition_coalesces_adjacent_same_group_runs() {
    let (groups, flat, _) =
        partition_glyph_ranges([run(1, 0..2), run(2, 2..5)], 5, 1, |_| Some(0), |_| false);
    assert_eq!(groups, vec![0..5]);
    assert!(flat.is_empty());
}

/// An out-of-bounds group index is filtered to flat — the
/// `pack_view_partitioned` `g < group_count` filter, mirrored.
#[test]
fn glyph_partition_out_of_bounds_group_is_flat() {
    let (groups, flat, _) = partition_glyph_ranges([run(1, 0..3)], 3, 1, |_| Some(7), |_| false);
    assert_eq!(groups, vec![0..0]);
    assert_eq!(flat, vec![0..3]);
}

/// The producer contract is load-bearing: gapless runs from 0 covering
/// `total`. A gap is a producer bug — caught loudly in debug builds.
#[test]
#[should_panic(expected = "entity runs must be contiguous")]
fn glyph_partition_gap_trips_the_debug_assert() {
    let _ = partition_glyph_ranges([run(1, 0..2), run(2, 3..4)], 4, 0, |_| None, |_| false);
}

// --- node_quad_anchors + pack_gradient_instances (parity gradient-bleed fix) -

use buiy_core::render::buckets::pack_gradient_instances;
use buiy_core::render::extract::ExtractedGradient;

/// A minimal 2-stop gradient record at box origin `x` (the fields the bleed-fix
/// tests don't assert on are arbitrary-but-valid).
fn grad(x: f32) -> ExtractedGradient {
    ExtractedGradient {
        rect_pos: Vec2::new(x, 0.0),
        rect_size: Vec2::splat(10.0),
        color0: [1.0, 0.0, 0.0, 1.0],
        color1: [0.0, 1.0, 0.0, 1.0],
        stops: [0.0, 1.0],
        axis: [1.0, 0.0],
        kind: 0.0,
        line_len: 10.0,
        clip: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
    }
}

fn node_with_grads(entity: u32, color: Color, grads: Vec<ExtractedGradient>) -> ExtractedNode {
    let mut n = grouped(entity, color, None);
    n.gradients = grads;
    n
}

/// `node_quad_anchors[i]` is the quad-blob index right AFTER node i's own quad —
/// the paint-order position its background gradient draws at.
#[test]
fn partition_records_node_quad_anchors() {
    let nodes = vec![
        grouped(1, Color::WHITE, None), // own quad at 0 → anchor 1
        grouped(2, Color::WHITE, None), // own quad at 1 → anchor 2
        grouped(3, Color::WHITE, None), // own quad at 2 → anchor 3
    ];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.node_quad_anchors, vec![1, 2, 3]);
}

/// A transparent (`Color::NONE`) node emits NO quad, so its anchor is the running
/// count UNCHANGED — its gradient (the dot-grid viewport case: a solid-less bg
/// with only a gradient layer) draws BEFORE its descendants' quads.
#[test]
fn partition_transparent_node_anchor_is_current_count() {
    let nodes = vec![
        grouped(1, Color::NONE, None),  // no own quad → anchor 0
        grouped(2, Color::WHITE, None), // own quad at 0 → anchor 1
    ];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.node_quad_anchors, vec![0, 1]);
    assert_eq!(p.instances.len(), 1);
}

/// F4a: `node_quad_anchor_of` maps EVERY node's entity to its anchor — INCLUDING
/// a `Color::NONE` node, which `quad_slot_of` (painting nodes only) misses. A
/// raster node usually paints no background quad, so this map is the ONLY one that
/// can join it to its paint-order splice position.
#[test]
fn partition_node_quad_anchor_of_covers_transparent_raster_nodes() {
    let nodes = vec![
        grouped(1, Color::WHITE, None), // own quad at 0 → anchor 1
        grouped(2, Color::NONE, None),  // a raster node: NO quad → anchor 1
        grouped(3, Color::WHITE, None), // own quad at 1 → anchor 2
    ];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    let e = |n: u32| Entity::from_raw_u32(n).unwrap();
    assert_eq!(p.node_quad_anchor_of.get(&e(1)).copied(), Some(1));
    // The transparent (raster) node: present in the anchor map even though it has
    // no quad_slot — it splices at anchor 1 (after node 1's quad, before node 3's).
    assert_eq!(p.node_quad_anchor_of.get(&e(2)).copied(), Some(1));
    assert!(
        !p.quad_slot_of.contains_key(&e(2)),
        "no quad slot for a Color::NONE node"
    );
    assert_eq!(p.node_quad_anchor_of.get(&e(3)).copied(), Some(2));
}

/// `pack_gradient_instances` tags each emitted gradient with its node's anchor;
/// multiple gradients on one node share that anchor (the bleed-fix wiring).
#[test]
fn pack_gradient_instances_tags_each_node_with_its_anchor() {
    let nodes = vec![
        node_with_grads(1, Color::WHITE, vec![grad(0.0), grad(1.0)]), // quad@0 → anchor 1
        node_with_grads(2, Color::WHITE, vec![grad(2.0)]),            // quad@1 → anchor 2
    ];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.node_quad_anchors, vec![1, 2]);
    let (gradients, anchors) = pack_gradient_instances(&nodes, &p.node_quad_anchors);
    assert_eq!(gradients.len(), 3);
    // node0's two gradients both at anchor 1; node1's at anchor 2.
    assert_eq!(anchors, vec![1, 1, 2]);
}

// --- interleave_flat_quads_and_gradients (the paint-order draw schedule) -----

use buiy_core::render::buckets::{
    FlatDrawStep, block_interleave, interleave_flat_draw, interleave_flat_quads_and_gradients,
};

/// Build flat quad runs from `(start, end)` pairs. Routing the ranges through a
/// helper keeps every call uniform and sidesteps clippy's
/// `single_range_in_vec_init` (a bare `&[a..b]` reads as an ambiguous range
/// literal); these are intentional N-element slices of `Range<u32>`.
fn runs(pairs: &[(u32, u32)]) -> Vec<Range<u32>> {
    pairs.iter().map(|&(s, e)| s..e).collect()
}

/// No gradients ⇒ the schedule is exactly the flat quad runs (byte-for-byte the
/// pre-fix flat draw), including across a group gap.
#[test]
fn interleave_no_gradients_is_just_the_flat_runs() {
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 3)]), &[]),
        vec![FlatDrawStep::Quads(0..3)]
    );
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 2), (5, 8)]), &[]),
        vec![FlatDrawStep::Quads(0..2), FlatDrawStep::Quads(5..8)]
    );
}

/// A gradient anchored MID-run splits the flat run: quads before the anchor, the
/// gradient, then the rest (the leaf case — a node's own quad, its gradient over
/// it, then later widgets).
#[test]
fn interleave_gradient_splits_flat_run_at_anchor() {
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 10)]), &[3]),
        vec![
            FlatDrawStep::Quads(0..3),
            FlatDrawStep::Gradients(0..1),
            FlatDrawStep::Quads(3..10),
        ]
    );
}

/// THE BUG FIX: an ANCESTOR gradient (anchor 0 — its node has no solid quad, the
/// viewport dot-grid) draws BEFORE every descendant quad, so the descendants
/// paint over it (dots show only in the gaps), not the reverse.
#[test]
fn interleave_ancestor_gradient_draws_before_descendant_quads() {
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 3)]), &[0]),
        vec![FlatDrawStep::Gradients(0..1), FlatDrawStep::Quads(0..3)]
    );
}

/// Multiple gradients sharing a node (same anchor) coalesce into ONE gradient run
/// (the pass binds the gradient pipeline once).
#[test]
fn interleave_same_anchor_gradients_coalesce() {
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 4)]), &[2, 2, 2]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Gradients(0..3),
            FlatDrawStep::Quads(2..4),
        ]
    );
}

/// A gradient anchored PAST every quad (its node is the last painter) draws after
/// all the flat quads.
#[test]
fn interleave_gradient_after_all_quads() {
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 3)]), &[3]),
        vec![FlatDrawStep::Quads(0..3), FlatDrawStep::Gradients(0..1)]
    );
}

/// Distinct ascending anchors interleave in order; the flat run splits at each.
#[test]
fn interleave_multiple_distinct_anchors_in_order() {
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 6)]), &[1, 4]),
        vec![
            FlatDrawStep::Quads(0..1),
            FlatDrawStep::Gradients(0..1),
            FlatDrawStep::Quads(1..4),
            FlatDrawStep::Gradients(1..2),
            FlatDrawStep::Quads(4..6),
        ]
    );
}

/// A gradient whose anchor lands inside a GROUP GAP (its node's quad is in an
/// off-screen group range, not flat) draws right after the last flat quad before
/// the gap — the documented "gradient on a grouped element" limitation — and
/// never drops a flat quad. flat_ranges = [0..2, 5..8] (3..5 is a group gap).
#[test]
fn interleave_gradient_anchored_in_group_gap() {
    assert_eq!(
        interleave_flat_quads_and_gradients(&runs(&[(0, 2), (5, 8)]), &[4]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Gradients(0..1),
            FlatDrawStep::Quads(5..8),
        ]
    );
}

// --- interleave_flat_draw: the F4a general per-raster-anchor interleave --------
// Each raster splices at its OWN node_quad_anchor (the exact gradient mechanism),
// retiring the prototype's single global top-layer-suffix `Rasters` marker. These
// prove the splice position, the raster/gradient ordering, and — the load-bearing
// F4a contract — that a view with NO raster is byte-identical to the gradient-only
// interleave (empty raster anchors reproduce the old draw exactly).

/// THE F4a BYTE-STABILITY CONTRACT: empty raster anchors ⇒ `interleave_flat_draw`
/// is byte-identical to `interleave_flat_quads_and_gradients` for every gradient
/// config, so a non-raster view's flat draw is unchanged (no golden churn).
#[test]
fn raster_empty_anchors_is_byte_identical_to_the_gradient_interleave() {
    let check = |flat: &[(u32, u32)], grad: &[u32]| {
        assert_eq!(
            interleave_flat_draw(&runs(flat), grad, &[]),
            interleave_flat_quads_and_gradients(&runs(flat), grad),
            "flat={flat:?} grad={grad:?}",
        );
    };
    check(&[(0, 3)], &[]);
    check(&[(0, 10)], &[3]);
    check(&[(0, 3)], &[0]);
    check(&[(0, 4)], &[2, 2, 2]);
    check(&[(0, 6)], &[1, 4]);
    check(&[(0, 2), (5, 8)], &[4]);
}

/// Empty gradients AND rasters ⇒ just the flat quad runs (across a group gap too).
#[test]
fn raster_and_gradient_both_empty_is_the_flat_runs() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 3), (5, 8)]), &[], &[]),
        vec![FlatDrawStep::Quads(0..3), FlatDrawStep::Quads(5..8)]
    );
}

/// THE F4a FIX: a raster anchored MID-run splits the flat draw — quads before its
/// node paint UNDER it, quads after paint OVER it (a non-top-layer overlay, or an
/// opaque modal's own panel quad, now correctly paints over the canvas).
#[test]
fn raster_splices_at_its_anchor_mid_run() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 10)]), &[], &[6]),
        vec![
            FlatDrawStep::Quads(0..6),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Quads(6..10),
        ]
    );
}

/// A raster anchored at 0 (its node is the first painter — a full-view backdrop
/// canvas) draws FIRST; every quad paints over it.
#[test]
fn raster_anchor_zero_draws_before_all_quads() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 3)]), &[], &[0]),
        vec![FlatDrawStep::Raster(0), FlatDrawStep::Quads(0..3)]
    );
}

/// A raster anchored PAST every quad (its node is the last painter) draws after
/// all the flat quads — the pre-F4a fill-tier position, preserved for that case.
#[test]
fn raster_after_all_quads_keeps_the_fill_tier_position() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 3)]), &[], &[3]),
        vec![FlatDrawStep::Quads(0..3), FlatDrawStep::Raster(0)]
    );
}

/// Multiple rasters each splice at their OWN anchor (the general interleave — no
/// single global raster tier): distinct nodes, distinct stacking positions.
#[test]
fn multiple_rasters_each_splice_at_their_anchor() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 8)]), &[], &[2, 6]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Quads(2..6),
            FlatDrawStep::Raster(1),
            FlatDrawStep::Quads(6..8),
        ]
    );
}

/// Two rasters sharing an anchor keep the caller's (stable-sorted) order.
#[test]
fn rasters_sharing_an_anchor_keep_stable_order() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 4)]), &[], &[2, 2]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Raster(1),
            FlatDrawStep::Quads(2..4),
        ]
    );
}

/// A raster and gradient at DISTINCT anchors interleave by ascending anchor,
/// regardless of which comes first (raster-under-gradient and gradient-under-raster
/// are both just paint order).
#[test]
fn raster_and_gradient_interleave_by_ascending_anchor() {
    // gradient (anchor 2) then raster (anchor 6).
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 8)]), &[2], &[6]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Gradients(0..1),
            FlatDrawStep::Quads(2..6),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Quads(6..8),
        ]
    );
    // raster (anchor 2) then gradient (anchor 6).
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 8)]), &[6], &[2]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Quads(2..6),
            FlatDrawStep::Gradients(0..1),
            FlatDrawStep::Quads(6..8),
        ]
    );
}

/// The tie-break at a SHARED anchor: a gradient (a node's BACKGROUND layer) paints
/// BEFORE a raster (a node's CONTENT). A non-case in practice (rasters/gradients
/// live on disjoint nodes) but deterministic.
#[test]
fn raster_paints_over_gradient_at_a_shared_anchor() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 4)]), &[2], &[2]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Gradients(0..1),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Quads(2..4),
        ]
    );
}

/// A raster whose anchor lands inside a GROUP GAP (its node's quad is in an
/// off-screen group range, not flat) draws right after the last flat quad before
/// the gap — the documented "on a grouped element" limitation, matching gradients,
/// and never drops a flat quad. flat_ranges = [0..2, 5..8] (2..5 is a group gap).
#[test]
fn raster_anchored_in_group_gap_draws_after_last_flat_quad_before_gap() {
    assert_eq!(
        interleave_flat_draw(&runs(&[(0, 2), (5, 8)]), &[], &[4]),
        vec![
            FlatDrawStep::Quads(0..2),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Quads(5..8),
        ]
    );
}

// --- block_interleave: the F9 draw-STEP-count stability gate (rev-4/M4) --------
//
// The top-layer block split (§ 3.3, W2) must NOT be an off-screen composite. Two
// properties gate that, both CPU-deterministic (rev-4/M4: a headless FlatDrawStep
// test, NOT an iai bench — iai counts CPU *instructions*, not draw calls):
//
//   (a) an EMPTY top-layer block yields a schedule byte-identical to a single
//       `interleave_flat_draw` (no extra passes for an innocent scene). Proven in
//       `toplayer_block_partition.rs::
//       block_interleave_empty_top_block_is_byte_identical_to_the_single_interleave`
//       — referenced, NOT duplicated here.
//
//   (b) a top-layer scene splits the SAME instance space into a base + a top-layer
//       block: every quad/gradient/raster instance is still drawn EXACTLY once from
//       its ABSOLUTE index (conservation — the deterministic stand-in for "no
//       off-screen-target allocation"), and the step count grows by AT MOST one (a
//       bounded delta — the single straddling quad run cut at the boundary, never a
//       multiplicative blow-up or a second full-scene pass). The tests below are
//       (b): the step-count/conservation gate.

/// Every quad INSTANCE index a schedule draws, sorted. A schedule draws each quad
/// at most once (disjoint flat runs, a monotonic cursor), so this is a set.
fn quad_indices(steps: &[FlatDrawStep]) -> Vec<u32> {
    let mut v: Vec<u32> = steps
        .iter()
        .filter_map(|s| match s {
            FlatDrawStep::Quads(r) => Some(r.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    v.sort_unstable();
    v
}

/// Every gradient index a schedule draws, sorted.
fn gradient_indices(steps: &[FlatDrawStep]) -> Vec<u32> {
    let mut v: Vec<u32> = steps
        .iter()
        .filter_map(|s| match s {
            FlatDrawStep::Gradients(r) => Some(r.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    v.sort_unstable();
    v
}

/// Every raster index a schedule draws, sorted.
fn raster_indices(steps: &[FlatDrawStep]) -> Vec<u32> {
    let mut v: Vec<u32> = steps
        .iter()
        .filter_map(|s| match s {
            FlatDrawStep::Raster(k) => Some(*k),
            _ => None,
        })
        .collect();
    v.sort_unstable();
    v
}

/// The base and top blocks TOGETHER draw exactly the baseline instance space —
/// every quad/gradient/raster index once, no drop, no duplicate, all ABSOLUTE.
/// This is the CPU-deterministic proof of "NO off-screen-target allocation": the
/// split is a re-ordering (partition) of the SAME absolute instance buffers, not a
/// copied off-screen composite (which would re-base indices or re-draw base
/// content).
fn assert_conserves(base: &[FlatDrawStep], top: &[FlatDrawStep], baseline: &[FlatDrawStep]) {
    let mut q = quad_indices(base);
    q.extend(quad_indices(top));
    q.sort_unstable();
    assert_eq!(
        q,
        quad_indices(baseline),
        "every quad instance drawn once from its absolute index — no drop, no off-screen copy"
    );

    let mut g = gradient_indices(base);
    g.extend(gradient_indices(top));
    g.sort_unstable();
    assert_eq!(
        g,
        gradient_indices(baseline),
        "every gradient drawn once, absolute indices (top re-offset back)"
    );

    let mut r = raster_indices(base);
    r.extend(raster_indices(top));
    r.sort_unstable();
    assert_eq!(
        r,
        raster_indices(baseline),
        "every raster drawn once, absolute indices (top re-offset back)"
    );
}

/// (b) An innocent (no-top-layer) scene issues the SAME step count as the
/// pre-refactor single interleave — `node.rs` passes `u32::MAX`, so the base block
/// IS the baseline schedule and the top block is empty: no second pass. (Byte
/// identity is proven in `toplayer_block_partition.rs`; here we pin the step COUNT,
/// which is the F9 no-extra-passes gate.)
#[test]
fn f9_no_top_layer_scene_has_baseline_step_count_and_no_second_block() {
    let flat = runs(&[(0, 8)]);
    let grad = [3u32];
    let raster = [5u32];
    let baseline = interleave_flat_draw(&flat, &grad, &raster);
    let (base, top) = block_interleave(&flat, &grad, &raster, u32::MAX);
    assert_eq!(
        base.len(),
        baseline.len(),
        "a no-top-layer view issues the baseline step count (no extra pass)"
    );
    assert!(top.is_empty(), "no second tier-stack for an innocent scene");
    assert_conserves(&base, &top, &baseline);
}

/// (b) A top-layer boundary that STRADDLES a drawn quad span adds exactly ONE step
/// — the straddling run cut into a base half + a top half — over the baseline. A
/// bounded +1, not a multiplicative blow-up or an off-screen pass.
#[test]
fn f9_top_layer_straddle_adds_exactly_one_step() {
    // flat [0..8], no anchors ⇒ baseline is a single Quads(0..8). Boundary 3 lands
    // strictly inside the run (no anchor there) ⇒ base Quads(0..3) + top Quads(3..8).
    let flat = runs(&[(0, 8)]);
    let baseline = interleave_flat_draw(&flat, &[], &[]);
    assert_eq!(baseline.len(), 1, "one contiguous quad run");
    let (base, top) = block_interleave(&flat, &[], &[], 3);
    assert_eq!(
        base.len() + top.len(),
        baseline.len() + 1,
        "the split adds only the straddle cut (+1)"
    );
    assert_conserves(&base, &top, &baseline);
}

/// (b) A top-layer boundary that lands ON a run edge (a group gap) adds NO step —
/// the cut falls where the schedule already broke.
#[test]
fn f9_top_layer_boundary_at_a_run_edge_adds_no_step() {
    // Two flat runs with a group gap [2..5]; baseline = Quads(0..2) + Quads(5..8).
    // Boundary 5 == the second run's start ⇒ base keeps run 1, top keeps run 2, no
    // straddle to cut.
    let flat = runs(&[(0, 2), (5, 8)]);
    let baseline = interleave_flat_draw(&flat, &[], &[]);
    let (base, top) = block_interleave(&flat, &[], &[], 5);
    assert_eq!(
        base.len() + top.len(),
        baseline.len(),
        "a boundary at a run edge is a zero-cost split (delta 0)"
    );
    assert_conserves(&base, &top, &baseline);
}

/// (b) THE F9 GATE — sweep EVERY boundary over a rich fixture (flat runs across a
/// group gap, interleaved gradients + rasters): the split is ALWAYS a partition of
/// the baseline instance space (conservation — no off-screen duplication) and adds
/// AT MOST one step (a bounded 0..=1 delta — never a multiplicative blow-up or a
/// second full-scene pass). This is the deterministic CPU stand-in for "the
/// top-layer block adds only the per-block second tier-stack, no off-screen
/// target."
#[test]
fn f9_block_split_is_a_bounded_partition_for_every_boundary() {
    let flat = runs(&[(0, 4), (6, 12)]); // a group gap at [4..6]
    let grad = [2u32, 8];
    let raster = [3u32, 10];
    let baseline = interleave_flat_draw(&flat, &grad, &raster);
    // Boundary from before the first instance (0) to past the last (13).
    for boundary in 0..=13u32 {
        let (base, top) = block_interleave(&flat, &grad, &raster, boundary);
        assert_conserves(&base, &top, &baseline);
        let delta = (base.len() + top.len()) as i64 - baseline.len() as i64;
        assert!(
            (0..=1).contains(&delta),
            "boundary {boundary}: step delta {delta} must stay in the bounded range 0..=1 \
             (no off-screen pass)"
        );
    }
    // The no-top-layer sentinel routes everything to the base block, byte-identical.
    let (base, top) = block_interleave(&flat, &grad, &raster, u32::MAX);
    assert_eq!(
        base, baseline,
        "u32::MAX ⇒ base is the whole baseline schedule"
    );
    assert!(top.is_empty(), "u32::MAX ⇒ empty top block");
}
