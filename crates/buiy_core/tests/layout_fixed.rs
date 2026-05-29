//! Phase 10 — Position::Fixed: containing block = layout root.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.1, § 2.2.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout,
    layout::{Inset, LayoutPlugin, Length, PositionKind, Sizing, Style},
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
