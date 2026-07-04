//! Task 2.4 self-tests for the Tier-2 display-list dump. Plain `assert_eq!` /
//! `assert!` (NOT snapshots) so these meta-tests cannot pass vacuously
//! (snapshots.md § Verification #1, #4 + the magenta-sentinel signal).

use bevy::prelude::*;
use buiy_core::render::components::ClipRect;
use buiy_core::render::extract::{ExtractedNode, ExtractedNodes};
use buiy_verify::snapshot::{DISPLAY_LIST_DUMP_VERSION, NameLookup, display_list_dump};

/// Build an `ExtractedNodes` set with two named nodes (a clipped tooltip over a
/// modal) plus the matching `NameLookup`, both derived from REAL entities in a
/// fresh `World`. `swap` flips the order the two entities are spawned, which
/// perturbs their Entity ids — so the determinism self-test proves the dump is
/// Name-keyed (id-invariant), end-to-end through `NameLookup::from_world`.
fn two_node_scene(swap: bool) -> (ExtractedNodes, NameLookup) {
    let mut world = World::new();
    let (modal_e, tooltip_e) = if swap {
        let t = world.spawn(Name::new("tooltip")).id();
        let m = world.spawn(Name::new("modal")).id();
        (m, t)
    } else {
        let m = world.spawn(Name::new("modal")).id();
        let t = world.spawn(Name::new("tooltip")).id();
        (m, t)
    };

    let modal = ExtractedNode {
        entity: modal_e,
        position: Vec2::new(10.0, 20.0),
        size: Vec2::new(100.0, 40.0),
        radius: 0.0,
        color: Color::srgba(0.1, 0.2, 0.3, 1.0),
        clip: None,
        group: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    };
    let tooltip = ExtractedNode {
        entity: tooltip_e,
        position: Vec2::new(0.0, 0.0),
        size: Vec2::new(80.0, 24.0),
        radius: 0.0,
        color: Color::WHITE,
        clip: Some(ClipRect {
            min: Vec2::new(0.0, 0.0),
            max: Vec2::new(80.0, 24.0),
        }),
        group: Some(0),
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    };
    let nodes = ExtractedNodes {
        // Stored paint order is modal (bottom) then tooltip (top); the dump
        // emits this verbatim regardless of the entities' raw ids.
        nodes: vec![modal, tooltip],
        ..Default::default()
    };
    (nodes, NameLookup::from_world(&world))
}

#[test]
fn display_dump_is_entity_order_invariant() {
    // snapshots.md § Verification #1: the dump renders entities by Name, so two
    // scenes whose names map to DIFFERENT Entity ids produce a byte-identical
    // dump (the node ORDER is the same; only the underlying ids differ).
    let (na, la) = two_node_scene(false);
    let (nb, lb) = two_node_scene(true);
    let da = display_list_dump(&na, &la);
    let db = display_list_dump(&nb, &lb);
    assert_eq!(da, db, "display-list dump must be invariant to Entity ids");
    assert!(da.contains("modal rect"), "names the modal node");
    assert!(da.contains("tooltip rect"), "names the tooltip node");
}

#[test]
fn display_dump_has_version_header() {
    // snapshots.md § Verification #4.
    let (nodes, names) = two_node_scene(false);
    let dump = display_list_dump(&nodes, &names);
    assert_eq!(
        dump.lines().next(),
        Some(DISPLAY_LIST_DUMP_VERSION),
        "first line is the display-list dump version header"
    );
}

#[test]
fn nodes_render_in_stored_paint_order() {
    // The dump emits `ExtractedNode.nodes` in STORED order (never re-sorted —
    // extract.rs:141), so a z-sort regression shows as a line reorder. The
    // modal is index 0, the tooltip index 1.
    let (nodes, names) = two_node_scene(false);
    let dump = display_list_dump(&nodes, &names);
    let node_lines: Vec<&str> = dump
        .lines()
        .skip_while(|l| !l.starts_with("[nodes"))
        .skip(1)
        .take_while(|l| !l.starts_with("[buckets"))
        .collect();
    assert_eq!(node_lines.len(), 2);
    assert!(node_lines[0].starts_with("0 modal rect"), "index 0 = modal");
    assert!(
        node_lines[1].starts_with("1 tooltip rect"),
        "index 1 = tooltip"
    );
    // The clipped tooltip renders its clip AABB; the unclipped modal is `none`.
    assert!(node_lines[0].contains("clip=none"));
    assert!(node_lines[1].contains("clip=0,0..80,24"));
    assert!(node_lines[1].contains("group=0"));
    assert!(node_lines[0].contains("group=none"));
}

#[test]
fn resolved_color_renders_as_hex_and_unnamed_entity_falls_back() {
    // snapshots.md § Tier 2: `ExtractedNode.color` is already theme-resolved, so
    // it dumps as its literal `#rrggbbaa` (a color regression shows as a hex
    // diff). The concrete magenta here just makes the hex easy to eyeball — the
    // dump renders any resolved color the same way. (Track B removed the old
    // magenta missing-token *sentinel*: a typo'd token is now a compile error, so
    // there is no runtime miss to surface.)
    let node = ExtractedNode {
        entity: Entity::from_raw_u32(1).unwrap(),
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
        radius: 0.0,
        color: Color::srgb(1.0, 0.0, 1.0),
        clip: None,
        group: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    };
    let nodes = ExtractedNodes {
        nodes: vec![node],
        ..Default::default()
    };
    let dump = display_list_dump(&nodes, &NameLookup::default());
    assert!(
        dump.contains("color=#ff00ffff"),
        "a resolved color must surface as its #rrggbbaa hex, got:\n{dump}"
    );
    // Unnamed entity falls back to entity#<index>.
    assert!(dump.contains("entity#1 rect"), "unnamed fallback in dump");
}

#[test]
fn buckets_appear_in_draw_order_with_counts() {
    // The dump appends the pack_view() InstanceBuckets in BTreeMap (draw) order
    // with per-batch `xN` counts. Two opaque nodes → one (Quad,layer=0) x2.
    let (nodes, names) = two_node_scene(false);
    let dump = display_list_dump(&nodes, &names);
    assert!(
        dump.contains("[buckets draw-order]"),
        "dump has a buckets section"
    );
    assert!(
        dump.contains("(Quad,layer=0) x2"),
        "two opaque nodes pack to one Quad batch of 2, got:\n{dump}"
    );
}
