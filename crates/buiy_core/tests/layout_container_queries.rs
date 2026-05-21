//! Phase 5 integration tests — container queries and container units.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.5.

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::{
    ContainerQuery, ContainerQueryActive, ContainerQueryInactive, LayoutPlugin, Length,
    QueryCondition, Style,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

#[test]
fn cq_activate_marks_active_when_container_meets_min_width() {
    let mut app = app();

    // Container: 700 x 400, marked as size-container.
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    // Child carries a rule: activate when min-width >= 600 px.
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Need two frames: frame 1 establishes ResolvedLayout for the
    // container; frame 2 lets cq_activate read it.
    app.update();
    app.update();

    let world = app.world();
    assert!(
        world.get::<ContainerQueryActive>(child).is_some(),
        "child should be marked active because parent width 700 >= 600"
    );
    assert!(world.get::<ContainerQueryInactive>(child).is_none());
}
