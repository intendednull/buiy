//! Phase 10 — Position::Fixed: containing block = layout root.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.1, § 2.2.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout,
    layout::{Display, Inset, LayoutPlugin, Length, Position, PositionKind, Sizing, Style},
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

fn inset_top_left(top: f32, left: f32) -> Inset {
    Inset {
        top: Sizing::Length(Length::Px(top)),
        left: Sizing::Length(Length::Px(left)),
        ..Default::default()
    }
}

// A fixed child does not displace its in-flow sibling: the sibling lays
// out as if the fixed child were not a flow participant of the parent
// (the fixed child is removed from the parent's Taffy child list — D1/D4).
#[test]
fn fixed_child_does_not_affect_in_flow_sibling() {
    let mut app = app();
    // Parent is a column flex with two children: one in-flow (height 40),
    // one fixed (height 40). The in-flow sibling must sit at y=0 inside
    // the parent regardless of the fixed child.
    let in_flow = app
        .world_mut()
        .spawn((Node, Style::default().height_px(40.0).width_px(100.0)))
        .id();
    let fixed = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Fixed)
                .inset(inset_top_left(0.0, 0.0))
                .height_px(40.0)
                .width_px(100.0),
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(100.0)
                .height_px(200.0),
        ))
        .add_children(&[in_flow, fixed])
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .add_child(parent)
        .id();
    app.update();

    // The in-flow sibling is the ONLY flow child of `parent`, so it sits
    // at the parent's content origin (relative position y == 0). If the
    // fixed child were still in the parent's flex flow, the column would
    // place the in-flow sibling after/around it and break this.
    let in_flow_layout = app
        .world()
        .get::<ResolvedLayout>(in_flow)
        .expect("in-flow sibling has ResolvedLayout");
    assert_eq!(
        in_flow_layout.position.y,
        in_flow_rel_y_for_parent_origin(&app, parent),
        "in-flow sibling sits at the parent's content origin; fixed child is out of flow",
    );
    assert_eq!(in_flow_layout.size, Vec2::new(100.0, 40.0));
}

// Helper: the parent's own resolved Y (Taffy positions are parent-relative
// for in-flow children, so the in-flow sibling's resolved Y equals the
// parent's resolved Y when it is the first/only flow child at content
// origin with zero padding/border).
fn in_flow_rel_y_for_parent_origin(app: &App, parent: Entity) -> f32 {
    app.world()
        .get::<ResolvedLayout>(parent)
        .unwrap()
        .position
        .y
}

// A fixed entity nested deep under a positioned ancestor resolves its
// inset against the LAYOUT ROOT, not the nearest positioned ancestor.
// This is the sole behavioral difference from Absolute (spec § 2.1).
#[test]
fn fixed_resolves_against_root_not_nearest_ancestor() {
    let mut app = app();
    // root (800x600) > offset_parent (relative, positioned at 100,100,
    // sized 200x200) > fixed (top:0,left:0, size 50x50).
    // Absolute would place `fixed` at the offset_parent origin (100,100);
    // Fixed must place it at the ROOT origin (0,0).
    let fixed = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Fixed)
                .inset(inset_top_left(0.0, 0.0))
                .width_px(50.0)
                .height_px(50.0),
        ))
        .id();
    let offset_parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .inset(inset_top_left(100.0, 100.0))
                .width_px(200.0)
                .height_px(200.0),
        ))
        .add_child(fixed)
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .add_child(offset_parent)
        .id();
    app.update();

    let fixed_layout = app
        .world()
        .get::<ResolvedLayout>(fixed)
        .expect("fixed entity has ResolvedLayout (laid out under the root)");
    assert_eq!(
        fixed_layout.position,
        Vec2::new(0.0, 0.0),
        "fixed resolves top:0/left:0 against the ROOT origin (0,0), not the \
         offset parent at (100,100)",
    );
    assert_eq!(fixed_layout.size, Vec2::new(50.0, 50.0));
}

// A fixed entity with a percentage inset resolves the percentage against
// the ROOT's size (800x600), proving Taffy got the root as the available
// space (the parent-edge re-parent gives this for free — D1 runner-up note).
#[test]
fn fixed_percent_inset_resolves_against_root_size() {
    let mut app = app();
    let fixed = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Fixed)
                .inset(Inset {
                    left: Sizing::Length(Length::Percent(50.0)),
                    top: Sizing::Length(Length::Percent(50.0)),
                    ..Default::default()
                })
                .width_px(10.0)
                .height_px(10.0),
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(200.0)))
        .add_child(fixed)
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .add_child(parent)
        .id();
    app.update();

    let fixed_layout = app.world().get::<ResolvedLayout>(fixed).unwrap();
    // 50% of the ROOT (800x600) = (400, 300), NOT 50% of the 200x200 parent.
    assert_eq!(
        fixed_layout.position,
        Vec2::new(400.0, 300.0),
        "percent inset resolves against the root size (800x600), not the parent (200x200)",
    );
}

// Flipping Position::Fixed -> Absolute on a descendant re-homes it under its
// real parent in the Taffy tree (D3 — the re-parent decision is a pure
// per-frame function of Position.kind, recomputed; no stale flag).
//
// REGRESSION GUARD (reviewer blocker): the children-sync pass must rebuild a
// parent's Taffy child list whenever ANY of its children changes Fixed-status,
// even when the parent itself is unchanged. The pass therefore iterates the
// FULL node set, NOT the `Changed`-filtered `sync_styles` query. To exercise
// that, the tree is first driven to STEADY STATE (`Changed`-filter empty) so
// that flipping the child alone leaves its real parent out of the filtered
// set — under the buggy filtered-rows pass the child stays orphan-attached to
// the root from the prior frame and keeps resolving against the root.
//
// `ResolvedLayout.position` is the Taffy *local* (containing-block-relative)
// location (see `tests/layout_topology.rs`), so percentage insets are used to
// make the containing block observable in that local coordinate:
//   - Fixed: containing block = ROOT (800x600) -> 50% inset -> (400,300)
//   - Absolute: re-homed under the offset parent (200x200) -> (100,100)
// (a zero inset resolves to (0,0) under both and could not distinguish them).
#[test]
fn flipping_fixed_to_absolute_re_homes_under_offset_parent() {
    let mut app = app();
    let percent_50 = Inset {
        top: Sizing::Length(Length::Percent(50.0)),
        left: Sizing::Length(Length::Percent(50.0)),
        ..Default::default()
    };
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Fixed)
                .inset(percent_50)
                .width_px(20.0)
                .height_px(20.0),
        ))
        .id();
    let offset_parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .inset(inset_top_left(100.0, 100.0))
                .width_px(200.0)
                .height_px(200.0),
        ))
        .add_child(child)
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .add_child(offset_parent)
        .id();

    // Drive to steady state so the `Changed`-filter is empty before the flip
    // (otherwise the spawn-frame churn pulls the offset parent into the
    // filtered set and masks the bug).
    for _ in 0..6 {
        app.update();
    }
    // While Fixed: 50% resolves against the ROOT (800x600) -> (400,300).
    assert_eq!(
        app.world().get::<ResolvedLayout>(child).unwrap().position,
        Vec2::new(400.0, 300.0),
        "while Fixed, 50% inset resolves against the root (800x600)",
    );

    // Flip to Absolute: only the child enters the `Changed`-filter; the
    // children-sync pass must still rebuild the (unchanged) offset parent's
    // child list to re-home the child. 50% then resolves against the 200x200
    // parent -> (100,100) local to that parent.
    app.world_mut().entity_mut(child).insert(Position {
        kind: PositionKind::Absolute,
        inset: percent_50,
    });
    app.update();
    assert_eq!(
        app.world().get::<ResolvedLayout>(child).unwrap().position,
        Vec2::new(100.0, 100.0),
        "after flip to Absolute, re-homed under the offset parent: 50% resolves \
         against the 200x200 parent (100,100), not the stale root (400,300)",
    );
}

// A Display::None Fixed entity is removed from the Taffy tree entirely
// (map_display -> taffy::Display::None) and contributes nothing; it must
// not error the root-attach and must produce a zero-size layout.
#[test]
fn display_none_fixed_is_inert() {
    let mut app = app();
    let fixed_none = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::None)
                .position(PositionKind::Fixed)
                .inset(inset_top_left(0.0, 0.0))
                .width_px(30.0)
                .height_px(30.0),
        ))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .add_child(fixed_none)
        .id();
    // Must not panic / error.
    app.update();
    if let Some(rl) = app.world().get::<ResolvedLayout>(fixed_none) {
        assert_eq!(rl.size, Vec2::ZERO, "Display::None Fixed has zero size");
    }
}

// A Fixed child with an EXPLICIT zero inset resolves to the viewport origin
// (0,0) even when the layout ROOT carries padding. This is the property the
// view surface's `.fixed()` modifier depends on (it emits an explicit zero
// inset so an overlay/scrim pins to the viewport, not the padded content box):
// Taffy insets an absolute/fixed child by the containing block's BORDER only —
// root PADDING is excluded for an explicit inset — so `.fixed()` anchors to the
// viewport regardless of root padding without a layout-engine change.
#[test]
fn fixed_explicit_zero_inset_ignores_root_padding() {
    let mut app = app();
    let fixed = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Fixed)
                .inset(inset_top_left(0.0, 0.0))
                .width_px(50.0)
                .height_px(50.0),
        ))
        .id();
    // The root has generous padding on every side. A padded static position
    // (the auto-inset fallback) would place the fixed child at (40,40); an
    // explicit zero inset must ignore it and land at the viewport origin.
    let _root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(800.0)
                .height_px(600.0)
                .padding(40.0),
        ))
        .add_child(fixed)
        .id();
    app.update();

    let fixed_layout = app.world().get::<ResolvedLayout>(fixed).unwrap();
    assert_eq!(
        fixed_layout.position,
        Vec2::new(0.0, 0.0),
        "an explicit-zero-inset Fixed child resolves to the viewport origin \
         (0,0) regardless of the root's 40px padding — the property `.fixed()` \
         relies on for a viewport-anchored overlay",
    );
    assert_eq!(fixed_layout.size, Vec2::new(50.0, 50.0));
}

// Two Fixed siblings both attach to the root and keep their own insets
// (single global root, D2) — neither displaces the other.
#[test]
fn two_fixed_entities_both_resolve_against_root() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Fixed)
                .inset(inset_top_left(10.0, 10.0))
                .width_px(20.0)
                .height_px(20.0),
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Fixed)
                .inset(inset_top_left(50.0, 60.0))
                .width_px(20.0)
                .height_px(20.0),
        ))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .add_children(&[a, b])
        .id();
    app.update();
    assert_eq!(
        app.world().get::<ResolvedLayout>(a).unwrap().position,
        Vec2::new(10.0, 10.0)
    );
    assert_eq!(
        app.world().get::<ResolvedLayout>(b).unwrap().position,
        Vec2::new(60.0, 50.0)
    );
}
