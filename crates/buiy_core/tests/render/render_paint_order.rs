//! Integration: render consumes the REAL `StackingContext.painters_z` that
//! layout sub-pass 6f produces. Asserts the top-layer tier ORDER (v1 ships no
//! `::backdrop`, so this is order only — paint-order-and-top-layer.md § 3.1, § 4).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md.

use bevy::prelude::*;
use buiy_core::components::{ResolvedLayout, StackingContext};
use buiy_core::layout::{LayoutPlugin, PositionKind, Stacking, Style, TopLayer};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::extract::{
    ExtractedNode, ExtractedNodes, assemble_context_tree, context_roots, extracted_node_for,
};
use buiy_core::render::top_layer::partition_top_layer;
use buiy_core::theme::Theme;
use buiy_core::{CorePlugin, Node};
use buiy_verify::snapshot::{NameLookup, assert_display_list_snapshot};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

/// Run the REAL extract node walk over the live `StackingContext`s the layout
/// pipeline produced — the exact assembly `extract_buiy_nodes` runs (build the
/// `sc_by_entity` index, find the context roots, `assemble_context_tree` from
/// each) — and return the assembled entities in paint order. Shared by the
/// top-layer descendant-paint regression guards below so they exercise the same
/// `painters_z_of`-keyed descent the production extract does, not a private walk.
fn assembled_paint_order(app: &mut App) -> Vec<Entity> {
    let contexts: Vec<(Entity, StackingContext)> = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &StackingContext)>();
        q.iter(world).map(|(e, sc)| (e, sc.clone())).collect()
    };
    let world = app.world();
    let sc_by_entity: std::collections::HashMap<Entity, &[Entity]> = contexts
        .iter()
        .map(|(e, sc)| (*e, sc.painters_z.as_slice()))
        .collect();
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { sc_by_entity.get(&e).copied() };
    let rank_by_entity: std::collections::HashMap<Entity, u8> = contexts
        .iter()
        .map(|(e, sc)| (*e, sc.cross_root_rank))
        .collect();
    let roots = context_roots(&sc_by_entity, |e| {
        rank_by_entity.get(&e).copied().unwrap_or(0)
    });
    // Color resolution is irrelevant to the WALK (which entities appear); a
    // default `Theme` keeps the build pure (the MinimalPlugins app has no render
    // `Theme` resource — that lives on the render world).
    let theme = Theme::default();
    let build = |e: Entity| -> Option<ExtractedNode> {
        let gt = world.get::<GlobalTransform>(e)?;
        let layout = world.get::<ResolvedLayout>(e)?;
        let bg = world.get::<Background>(e);
        Some(extracted_node_for(e, gt, layout, bg, None, &theme))
    };
    let mut assembled = Vec::new();
    for r in roots {
        assemble_context_tree(r, &painters_z_of, &mut |e| build(e), &mut assembled);
    }
    assembled.iter().map(|n| n.entity).collect()
}

#[test]
fn top_layer_subtree_descendants_are_walked() {
    // Regression (M1/M6 — the parity-prototype render bug): a top-layer node
    // ESCAPES its parent context and is appended to the root context's
    // `painters_z` tail as one atomic entry. The render node walk
    // (`context_tree_paint_order`) descends into a painter's subtree ONLY when
    // that painter owns a `StackingContext`. Before the trigger-7 fix a plain
    // `top_layer(Modal)`/`top_layer(Popover)` node (no transform / isolation /
    // z-index) formed NO stacking context, so the walk treated it as a childless
    // LEAF — its descendants' fills / bands / gradients / shadows / glyphs were
    // dropped (only icons survived, since `icon_producer` iterates entities
    // directly). Trigger 7 makes a top-layer member form its own SC, so its
    // whole subtree is walked. This asserts the live extract walk emits the
    // top-layer node's DESCENDANTS, not just the node itself.
    for tier in [TopLayer::Modal, TopLayer::Popover, TopLayer::Tooltip] {
        let mut app = app();
        // The exact bug shape: a top-layer card with a bg-filled child (the M6
        // "Create" button / selected segment) AND a deeper grandchild (the M1
        // dropdown row text leaf rides the same descend).
        let grandchild = app
            .world_mut()
            .spawn((
                Node,
                Name::new("grandchild"),
                Style::default().width_px(20.0).height_px(10.0),
                Background {
                    color: ColorToken::Token("color.accent".into()),
                },
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((
                Node,
                Name::new("child"),
                Style::default().width_px(40.0).height_px(20.0),
                Background {
                    color: ColorToken::Token("color.surface.card".into()),
                },
            ))
            .add_children(&[grandchild])
            .id();
        let top = app
            .world_mut()
            .spawn((
                Node,
                Name::new("top"),
                Style::default()
                    .top_layer(tier)
                    .width_px(100.0)
                    .height_px(100.0),
                Background {
                    color: ColorToken::Token("color.surface.card".into()),
                },
            ))
            .add_children(&[child])
            .id();
        let _root = app
            .world_mut()
            .spawn((
                Node,
                Name::new("root"),
                Style::default().width_px(200.0).height_px(200.0),
            ))
            .add_children(&[top])
            .id();
        app.update();
        // Two updates: the top-layer escape + SC write lands on frame 1; the
        // global transforms/layout settle by frame 2 (the assemble reads them).
        app.update();

        let order = assembled_paint_order(&mut app);
        assert!(
            order.contains(&top),
            "{tier:?}: the top-layer node itself must be walked",
        );
        assert!(
            order.contains(&child),
            "{tier:?}: a top-layer node's direct child must be walked (its \
             Background fill would otherwise never extract — the M6 bug)",
        );
        assert!(
            order.contains(&grandchild),
            "{tier:?}: a top-layer node's grandchild must be walked too (the \
             descent is recursive, not one level — the M1 dropdown-row case)",
        );
    }
}

#[test]
fn positioned_parent_paints_behind_its_children() {
    // Regression (M6 — the dialog-card-over-its-contents bug): a positioned
    // (`position: relative`/`absolute`) element with `z-index: auto` forms NO real
    // stacking context, but it must still paint as an atomic GROUP — its own
    // background/border BEHIND its descendants. Before the fix the per-context
    // `painters_z` flattened the whole subtree then sorted each entry by
    // `paint_key`: the positioned parent sorted into the LATE positioned tier while
    // its non-positioned children sorted into the EARLY tier, so the parent's fill
    // painted OVER (hid) its own children (the modal card painting over every
    // descendant). The fix gives a positioned element its own `painters_z`, so the
    // walk emits it, THEN its descendants — parent behind children.
    for kind in [PositionKind::Relative, PositionKind::Absolute] {
        let mut app = app();
        let child = app
            .world_mut()
            .spawn((
                Node,
                Name::new("child"),
                Style::default().width_px(20.0).height_px(10.0),
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn((
                Node,
                Name::new("parent"),
                Style::default()
                    .position(kind)
                    .width_px(40.0)
                    .height_px(40.0),
            ))
            .add_children(&[child])
            .id();
        app.world_mut()
            .spawn((
                Node,
                Name::new("root"),
                Style::default().width_px(100.0).height_px(100.0),
            ))
            .add_children(&[parent]);
        app.update();
        app.update();

        let order = assembled_paint_order(&mut app);
        let pi = order.iter().position(|&e| e == parent);
        let ci = order.iter().position(|&e| e == child);
        assert!(
            pi.is_some() && ci.is_some() && pi < ci,
            "{kind:?}: a positioned parent must paint BEFORE (behind) its \
             non-positioned child; got parent_idx={pi:?} child_idx={ci:?}",
        );
    }
}

#[test]
fn parentless_top_layer_root_paints_over_main_content() {
    // Regression (M6 — the modal-under-shell bug): a top-layer node that is its OWN
    // root (a PARENTLESS `TopLayer::Modal` tree — a dialog authored outside the
    // main content tree) cannot escape into a parent root's `painters_z` tail. It
    // becomes a separate root context; `context_roots` orders roots by
    // `(cross_root_rank, entity)`, and layout 6f stamps a top-layer root's rank
    // ABOVE an in-flow root's `0`. Without the rank the dialog sorted by raw entity
    // id and could paint UNDER the whole main-content shell. This asserts the
    // parentless modal root paints LAST (topmost), regardless of entity-id order.
    let mut app = app();
    // The main content root (spawned FIRST → lower entity id; pre-fix it would have
    // sorted, and painted, AFTER a higher-id dialog → covering it).
    let content = app
        .world_mut()
        .spawn((
            Node,
            Name::new("content"),
            Style::default().width_px(200.0).height_px(200.0),
        ))
        .id();
    // The PARENTLESS modal dialog (separate root, higher entity id, top-layer).
    let dialog = app
        .world_mut()
        .spawn((
            Node,
            Name::new("dialog"),
            Style::default()
                .top_layer(TopLayer::Modal)
                .width_px(80.0)
                .height_px(80.0),
        ))
        .id();
    app.update();
    app.update();

    let order = assembled_paint_order(&mut app);
    let content_idx = order.iter().position(|&e| e == content);
    let dialog_idx = order.iter().position(|&e| e == dialog);
    assert!(
        content_idx.is_some() && dialog_idx.is_some() && dialog_idx > content_idx,
        "a parentless TopLayer::Modal root must paint AFTER (over) the main \
         content root; got content_idx={content_idx:?} dialog_idx={dialog_idx:?}",
    );
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
                border: None,
                shadows: Vec::new(),
                gradients: Vec::new(),
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
                border: None,
                shadows: Vec::new(),
                gradients: Vec::new(),
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
