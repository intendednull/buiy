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
    let (groups, flat) = partition_glyph_ranges([run(1, 0..3), run(2, 3..5)], 5, 0, |_| None);
    assert!(groups.is_empty());
    assert_eq!(flat, vec![0..5]);
}

/// A grouped middle entity: its range lands in group_ranges[g]; the flat
/// complement is the two surrounding maximal runs.
#[test]
fn glyph_partition_grouped_middle_run() {
    let g = |e: Entity| (e == Entity::from_raw_u32(2).unwrap()).then_some(0);
    let (groups, flat) =
        partition_glyph_ranges([run(1, 0..2), run(2, 2..6), run(3, 6..9)], 9, 1, g);
    assert_eq!(groups, vec![2..6]);
    assert_eq!(flat, vec![0..2, 6..9]);
}

/// A group with no glyph-emitting member keeps its empty `0..0` slot at
/// its index (the group_ranges[g] == prepared group g alignment).
#[test]
fn glyph_partition_empty_group_slot() {
    let (groups, flat) = partition_glyph_ranges([run(1, 0..4)], 4, 2, |_| Some(1));
    assert_eq!(groups, vec![0..0, 0..4]);
    assert!(flat.is_empty());
}

/// Adjacent same-group entities coalesce into one contiguous group range
/// (two text entities inside one card).
#[test]
fn glyph_partition_coalesces_adjacent_same_group_runs() {
    let (groups, flat) = partition_glyph_ranges([run(1, 0..2), run(2, 2..5)], 5, 1, |_| Some(0));
    assert_eq!(groups, vec![0..5]);
    assert!(flat.is_empty());
}

/// An out-of-bounds group index is filtered to flat — the
/// `pack_view_partitioned` `g < group_count` filter, mirrored.
#[test]
fn glyph_partition_out_of_bounds_group_is_flat() {
    let (groups, flat) = partition_glyph_ranges([run(1, 0..3)], 3, 1, |_| Some(7));
    assert_eq!(groups, vec![0..0]);
    assert_eq!(flat, vec![0..3]);
}

/// The producer contract is load-bearing: gapless runs from 0 covering
/// `total`. A gap is a producer bug — caught loudly in debug builds.
#[test]
#[should_panic(expected = "entity runs must be contiguous")]
fn glyph_partition_gap_trips_the_debug_assert() {
    let _ = partition_glyph_ranges([run(1, 0..2), run(2, 3..4)], 4, 0, |_| None);
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

use buiy_core::render::buckets::{FlatDrawStep, interleave_flat_quads_and_gradients};

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
