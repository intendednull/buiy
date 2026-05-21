//! Phase 5 integration tests — container queries and container units.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.5.

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::ResolvedLayout;
use buiy_core::layout::{
    ContainerQuery, ContainerQueryActive, ContainerQueryInactive, LayoutPlugin,
    LayoutTaffyComputeCount, Length, QueryCondition, Sizing, Style,
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

#[test]
fn container_unit_cqw_resolves_against_queried_ancestor() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(800.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::Length(Length::Cqw(50.0))),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Two frames: 1) parent ResolvedLayout populated. 2) child reads
    // it for Cqw resolution.
    app.update();
    app.update();

    let child_layout = app.world().get::<ResolvedLayout>(child).unwrap();
    assert!(
        (child_layout.size.x - 400.0).abs() < 0.5,
        "child width should resolve to 50% of parent width 800 = 400, got {}",
        child_layout.size.x
    );
}

#[test]
fn cq_same_frame_relayout_caps_at_2x_taffy() {
    let mut app = app();

    // Establish a rule whose activation flips when the container
    // crosses 600 px. Spawn with a container width that starts at
    // 500 px (rule inactive last frame) and is set to 700 px this
    // frame (rule active, flip detected at step 4).
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(500.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
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

    app.update(); // Frame 1: ResolvedLayout populated for parent at 500.
    app.update(); // Frame 2: settle — cq_activate sees parent's 500-px
    // ResolvedLayout from frame 1 and marks child Inactive.
    assert!(
        app.world().get::<ContainerQueryInactive>(child).is_some(),
        "after the settle frame, child should be Inactive (parent 500 < 600)"
    );

    // Frame 3: bump parent to 700. cq_activate (step 2) reads the
    // previous frame's ResolvedLayout (500) and still sees Inactive.
    // taffy_compute (step 3) resolves parent to 700. cq_flip_check
    // (step 4) reads fresh `tree.layout(parent_id)` -> 700 -> rule
    // active_now=true, was=false -> flip child to Active + signal
    // re-run. cq_flip_rerun (step 5) re-runs sync_styles +
    // taffy_compute. Net: at end of frame 3, child is Active AND
    // LayoutTaffyComputeCount == 2.
    app.world_mut().entity_mut(parent).insert(
        Style::default()
            .width_px(700.0)
            .height_px(400.0)
            .container_size(),
    );

    app.update();

    assert!(
        app.world().get::<ContainerQueryActive>(child).is_some(),
        "after same-frame re-layout, child should be Active"
    );
    assert!(app.world().get::<ContainerQueryInactive>(child).is_none());

    let count = app
        .world()
        .get_resource::<LayoutTaffyComputeCount>()
        .expect("LayoutTaffyComputeCount registered")
        .0;
    assert_eq!(
        count, 2,
        "flip frame must run Taffy exactly twice (cap), got {count}"
    );
}

#[test]
fn cq_non_flip_frame_runs_taffy_exactly_once() {
    let mut app = app();
    // Scenario with no active container query — every frame should
    // run Taffy exactly once.
    app.world_mut()
        .spawn((Node, Style::default().width_px(100.0)));
    app.update();
    app.update(); // steady-state

    let count = app
        .world()
        .get_resource::<LayoutTaffyComputeCount>()
        .expect("LayoutTaffyComputeCount registered")
        .0;
    assert_eq!(
        count, 1,
        "non-flip frame must run Taffy exactly once, got {count}"
    );
}
