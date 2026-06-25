//! The § 4.6 splice-merge contract (decoration-and-paint.md): text quads
//! merge into the EXISTING quad instance blob, ordered by the FRESH node
//! list every pack, partition contiguity preserved. These tests are the
//! headless half of the contract; the GPU regressions live in
//! tests/text_decoration_gpu.rs.

use bevy::prelude::*;
use buiy_core::render::buckets::pack_view_partitioned;
use buiy_core::render::extract::{ExtractedNode, TextQuad};

fn node(entity: Entity, x: f32, color: Color, group: Option<usize>) -> ExtractedNode {
    ExtractedNode {
        entity,
        position: Vec2::new(x, 0.0),
        size: Vec2::splat(10.0),
        color,
        clip: None,
        group,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
    }
}

fn quad(entity: Entity, x: f32) -> TextQuad {
    TextQuad {
        entity,
        position: Vec2::new(x, 100.0),
        size: Vec2::new(5.0, 1.0),
        color: Color::WHITE,
        clip: None,
    }
}

fn e(i: u32) -> Entity {
    Entity::from_raw_u32(i).unwrap()
}

/// Instance x-positions, the order fingerprint.
fn xs(p: &buiy_core::render::buckets::PackedPartition) -> Vec<f32> {
    p.instances.iter().map(|i| i[0]).collect()
}

#[test]
fn quads_splice_immediately_after_their_entity() {
    let nodes = [
        node(e(1), 1.0, Color::WHITE, None),
        node(e(2), 2.0, Color::WHITE, None),
    ];
    let quads = [quad(e(1), 11.0), quad(e(1), 12.0), quad(e(2), 22.0)];
    let p = pack_view_partitioned(&nodes, 0, &quads);
    // node1, its two quads IN CARRIER ORDER, node2, its quad — § 4.4 holds
    // by construction (background < decorations, per entity).
    assert_eq!(xs(&p), vec![1.0, 11.0, 12.0, 2.0, 22.0]);
}

#[test]
fn transparent_nodes_still_anchor_their_quads() {
    // § 4.6 fact (a): extract emits Color::NONE records and only the pack
    // skips the BACKGROUND quad — the text quads still splice at the
    // entity's paint position.
    let nodes = [
        node(e(1), 1.0, Color::NONE, None),
        node(e(2), 2.0, Color::WHITE, None),
    ];
    let quads = [quad(e(1), 11.0)];
    let p = pack_view_partitioned(&nodes, 0, &quads);
    assert_eq!(xs(&p), vec![11.0, 2.0]);
}

#[test]
fn order_derives_from_the_fresh_node_list_every_pack() {
    // THE round-2 contract (the rejected painters_z merge key): the SAME
    // retained carrier lands correctly when the node walk rebuilt in a NEW
    // order for a non-text reason.
    let quads = [quad(e(1), 11.0), quad(e(2), 22.0)];
    let before = [
        node(e(1), 1.0, Color::WHITE, None),
        node(e(2), 2.0, Color::WHITE, None),
    ];
    let after = [
        node(e(2), 2.0, Color::WHITE, None),
        node(e(1), 1.0, Color::WHITE, None),
    ];
    assert_eq!(
        xs(&pack_view_partitioned(&before, 0, &quads)),
        vec![1.0, 11.0, 2.0, 22.0]
    );
    assert_eq!(
        xs(&pack_view_partitioned(&after, 0, &quads)),
        vec![2.0, 22.0, 1.0, 11.0]
    );
}

#[test]
fn quads_adopt_their_entitys_group_and_keep_contiguity() {
    // Group membership comes from the node record being spliced after —
    // a text quad's partition placement can never disagree with its
    // entity's. debug_assert contiguity must NOT fire (debug test builds).
    let nodes = [
        node(e(1), 1.0, Color::WHITE, None),
        node(e(2), 2.0, Color::WHITE, Some(0)),
        node(e(3), 3.0, Color::WHITE, Some(0)),
        node(e(4), 4.0, Color::WHITE, None),
    ];
    let quads = [quad(e(2), 22.0), quad(e(3), 33.0)];
    let p = pack_view_partitioned(&nodes, 1, &quads);
    assert_eq!(xs(&p), vec![1.0, 2.0, 22.0, 3.0, 33.0, 4.0]);
    // Group 0 = instances 1..5 (node2, quad, node3, quad) — one contiguous
    // range including the spliced quads.
    assert_eq!(p.group_ranges, vec![1..5]);
    assert_eq!(p.flat_ranges, vec![0..1, 5..6]);
}

#[test]
fn group_member_with_only_quads_still_extends_the_group() {
    // A transparent group member contributes ONLY its text quads — they
    // must still carry the group (the § 4.5 underline-dims contract).
    let nodes = [
        node(e(1), 1.0, Color::WHITE, Some(0)),
        node(e(2), 2.0, Color::NONE, Some(0)),
    ];
    let quads = [quad(e(2), 22.0)];
    let p = pack_view_partitioned(&nodes, 1, &quads);
    assert_eq!(p.group_ranges, vec![0..2]);
    assert!(p.flat_ranges.is_empty());
}

#[test]
fn unknown_entities_are_skipped() {
    // Decision 7: a quad whose entity has no node record this pack is
    // dropped (transient impossibility — both unions fire on entity-set
    // changes), never panicked on.
    let nodes = [node(e(1), 1.0, Color::WHITE, None)];
    let quads = [quad(e(9), 99.0)];
    let p = pack_view_partitioned(&nodes, 0, &quads);
    assert_eq!(xs(&p), vec![1.0]);
}

#[test]
fn transparent_text_quads_are_skipped_like_node_quads() {
    let nodes = [node(e(1), 1.0, Color::WHITE, None)];
    let mut q = quad(e(1), 11.0);
    q.color = Color::NONE;
    let p = pack_view_partitioned(&nodes, 0, &[q]);
    assert_eq!(xs(&p), vec![1.0]);
}

#[test]
fn empty_carrier_is_byte_identical_to_the_old_pack() {
    // The no-text regression: with no text quads the partition must be
    // exactly the pre-T6 output (the compositor's flat path stays
    // byte-for-byte).
    let nodes = [
        node(e(1), 1.0, Color::WHITE, None),
        node(e(2), 2.0, Color::WHITE, Some(0)),
        node(e(3), 3.0, Color::NONE, None),
    ];
    let p = pack_view_partitioned(&nodes, 1, &[]);
    assert_eq!(xs(&p), vec![1.0, 2.0]);
    assert_eq!(p.group_ranges, vec![1..2]);
    assert_eq!(p.flat_ranges, vec![0..1]);
}

#[test]
fn text_quad_packs_like_a_node_quad() {
    // pack_text_quad mirrors pack_extracted: linearized color, radius 0,
    // clip sentinel.
    use buiy_core::render::instance::pack_text_quad;
    let q = quad(e(1), 11.0);
    let p = pack_text_quad(&q);
    assert_eq!(p.rect_pos, [11.0, 100.0]);
    assert_eq!(p.rect_size, [5.0, 1.0]);
    assert_eq!(p.radius, 0.0);
    let lin = LinearRgba::from(Color::WHITE);
    assert_eq!(p.color, [lin.red, lin.green, lin.blue, lin.alpha]);
    assert_eq!(p.clip_min, [f32::NEG_INFINITY; 2]);
}
