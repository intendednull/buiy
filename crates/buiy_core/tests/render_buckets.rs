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
    b.push(q0, [0.0; 13]);
    b.push(q0, [1.0; 13]);
    b.push(s0, [2.0; 13]);
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
        [0.0; 13],
    );
    b.push(
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Shadow,
            layer: 0,
        },
        [0.0; 13],
    );
    b.push(
        PrimitiveBatchKey {
            primitive: BuiyPrimitiveKind::Quad,
            layer: 1,
        },
        [0.0; 13],
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

// pack_view consumes R5's ExtractedNode records (the prepare seam, Task 6) — the
// bucketing assertions below are unchanged from the DrawData era; only the input
// record type flipped.
fn node(entity: u32, position: Vec2, size: Vec2, color: Color) -> ExtractedNode {
    ExtractedNode {
        entity: Entity::from_raw_u32(entity).unwrap(),
        position,
        size,
        color,
        clip: None,
    }
}

#[test]
fn raw_layout_stride_agrees_with_struct() {
    // The [f32;13] the bucket holds must be byte-identical in size to the
    // PackedInstance struct the pipeline descriptor declares (52 B). If this
    // ever drifts, the instanced draw reads garbage.
    assert!(packed_raw_stride_agrees());
    assert_eq!(std::mem::size_of::<[f32; 13]>(), 52);
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
    let nodes = vec![node(
        1,
        Vec2::new(7.0, 9.0),
        Vec2::new(3.0, 4.0),
        Color::WHITE,
    )];
    let buckets = pack_view(&nodes);
    let (_, batch) = buckets.batches().next().expect("one batch");
    let expect = buiy_core::render::buckets::packed_to_raw(&pack_extracted(&nodes[0]));
    assert_eq!(batch[0], expect);
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

    assert_eq!(pack_view(&[opaque]).total_instances(), 1);
    assert_eq!(pack_view(&[transparent]).total_instances(), 0);

    // Mixed: only the opaque node survives the pack seam.
    let buckets = pack_view(&[opaque, transparent]);
    assert_eq!(buckets.total_instances(), 1);
    let quad0 = buiy_core::render::buckets::PrimitiveBatchKey {
        primitive: buiy_core::render::buckets::BuiyPrimitiveKind::Quad,
        layer: 0,
    };
    assert_eq!(buckets.len(quad0), 1);
}
