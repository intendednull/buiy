//! Integration: render consumes the REAL `StackingContext.painters_z` that
//! layout sub-pass 6f produces. Asserts the top-layer tier ORDER (v1 ships no
//! `::backdrop`, so this is order only — paint-order-and-top-layer.md § 3.1, § 4).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md.

use bevy::prelude::*;
use buiy_core::components::StackingContext;
use buiy_core::layout::{LayoutPlugin, Stacking, Style, TopLayer};
use buiy_core::render::extract::{ExtractedNode, ExtractedNodes, assemble_context_tree};
use buiy_core::render::top_layer::partition_top_layer;
use buiy_core::{CorePlugin, Node};
use buiy_verify::snapshot::{NameLookup, assert_display_list_snapshot};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

fn top_layer_of(world: &World, e: Entity) -> TopLayer {
    world
        .get::<Stacking>(e)
        .map(|s| s.top_layer)
        .unwrap_or(TopLayer::None)
}

#[test]
fn top_layer_tail_is_tier_ordered_fullscreen_to_modal() {
    let mut app = app();
    // Spawn one of each non-None tier as children of a single root. Layout 6f
    // escapes them to the root context's tail, tier-sorted. Name-tagged so the
    // display-list snapshot is diff-stable by Name (not raw Entity bits).
    let modal = app
        .world_mut()
        .spawn((
            Node,
            Name::new("modal"),
            Style::default().top_layer(TopLayer::Modal),
        ))
        .id();
    let tooltip = app
        .world_mut()
        .spawn((
            Node,
            Name::new("tooltip"),
            Style::default().top_layer(TopLayer::Tooltip),
        ))
        .id();
    let popover = app
        .world_mut()
        .spawn((
            Node,
            Name::new("popover"),
            Style::default().top_layer(TopLayer::Popover),
        ))
        .id();
    let fullscreen = app
        .world_mut()
        .spawn((
            Node,
            Name::new("fullscreen"),
            Style::default().top_layer(TopLayer::Fullscreen),
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Name::new("root"), Style::default()))
        .add_children(&[modal, tooltip, popover, fullscreen])
        .id();
    app.update();

    let sc = app
        .world()
        .get::<StackingContext>(root)
        .expect("root forms a context")
        .clone();
    let world = app.world();
    let (_in_flow, tail) = partition_top_layer(&sc.painters_z, |e| top_layer_of(world, e));

    // Render reads the tail verbatim; layout pinned the tier order. The
    // `assert_eq!(tail, vec![fullscreen, tooltip, popover, modal])` order check
    // becomes a Name-keyed display-list snapshot: the tail's paint order reads
    // off the node line order (Fullscreen < Tooltip < Popover < Modal,
    // paint-order § 3.1), so a tier-sort regression shows as a line reorder.
    let nodes = ExtractedNodes {
        nodes: tail
            .iter()
            .map(|&e| ExtractedNode {
                entity: e,
                position: Vec2::ZERO,
                size: Vec2::ONE,
                color: Color::WHITE,
                clip: None,
                group: None,
                affine: [[1.0, 0.0], [0.0, 1.0]],
                outline: None,
            })
            .collect(),
        ..Default::default()
    };
    let names = NameLookup::from_world(world);
    assert_display_list_snapshot(&nodes, "top_layer_tail_tier_order", &names);
}

#[test]
fn modal_is_first_hit_candidate_over_popover() {
    // The § 2 / § 3 identity at the integration level: the modal paints last,
    // so it is the FIRST hit-test candidate (why a modal is modal). Asserted
    // against the LIVE extract walk `assemble_context_tree` (the same walk
    // `extract_buiy_nodes` runs) — render owns ONE paint-order walk, and
    // hit-test order is its assembled output reversed (paint-order § 2, the
    // identity also pinned in tests/render_extract.rs).
    let mut app = app();
    let popover = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Popover)))
        .id();
    let modal = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Modal)))
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[popover, modal])
        .id();
    app.update();

    let sc = app.world().get::<StackingContext>(root).unwrap().clone();
    // The root context's painters_z, keyed for the tree walk; the leaves form
    // no nested SCs, so only `root` resolves to a painters_z slice.
    let painters_z_of = |e: Entity| -> Option<&[Entity]> {
        if e == root {
            Some(sc.painters_z.as_slice())
        } else {
            None
        }
    };
    let mut assembled = Vec::new();
    assemble_context_tree(
        root,
        &painters_z_of,
        &mut |e| {
            Some(ExtractedNode {
                entity: e,
                position: Vec2::ZERO,
                size: Vec2::ONE,
                color: Color::WHITE,
                clip: None,
                group: None,
                affine: [[1.0, 0.0], [0.0, 1.0]],
                outline: None,
            })
        },
        &mut assembled,
    );
    // Paint order = assembled forward; hit-test order = it reversed (§ 2).
    let paint: Vec<Entity> = assembled.iter().map(|n| n.entity).collect();
    let hit: Vec<Entity> = assembled.iter().rev().map(|n| n.entity).collect();
    assert_eq!(
        hit.first(),
        Some(&modal),
        "modal is the first hit-test candidate"
    );
    // And popover is below it in paint order (modal painted later).
    let modal_idx = paint.iter().position(|&x| x == modal).unwrap();
    let pop_idx = paint.iter().position(|&x| x == popover).unwrap();
    assert!(modal_idx > pop_idx, "modal paints after (above) popover");
}
