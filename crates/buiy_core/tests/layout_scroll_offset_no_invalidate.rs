//! Phase 2 invariant: mutating ScrollOffset (or ScrollSnapItem) must
//! NOT cause sync_styles to re-translate the entity in the following
//! frame. Asserted by mirroring sync_styles' trigger query and counting
//! the entities it would yield.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 2.1
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.2

use bevy::ecs::query::Or;
use bevy::prelude::*;
use buiy_core::{
    BoxModel, CorePlugin, Display, FlexItem, FlexParams, Length, Node, Overflow, OverflowMode,
    Position, ResolvedLayout, Scroll, ScrollOffset, ScrollSnapItem, Sizing, SnapAlign, Style,
    layout::LayoutPlugin,
};

/// Mirror of `sync_styles`' trigger filter. If `ScrollOffset` or
/// `ScrollSnapItem` mutation triggered this filter, the test would fail.
type SyncStylesFilter = (
    With<Node>,
    Or<(
        Changed<Display>,
        Changed<BoxModel>,
        Changed<Position>,
        Changed<FlexParams>,
        Changed<FlexItem>,
        Changed<Overflow>,
        Changed<Scroll>,
        Changed<Children>,
        Changed<ChildOf>,
    )>,
);

fn count_changed(world: &mut World) -> usize {
    let mut q = world.query_filtered::<Entity, SyncStylesFilter>();
    q.iter(world).count()
}

#[test]
fn mutating_scroll_offset_does_not_trigger_sync_styles() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(100.0)),
                    height: Sizing::Length(Length::Px(100.0)),
                    ..Default::default()
                },
                overflow: Overflow {
                    y: OverflowMode::Scroll,
                    ..Default::default()
                },
                ..Default::default()
            },
            ScrollOffset::default(),
        ))
        .id();

    // Frame 1: spawn frame; everything is `Changed`. sync_styles fired.
    // Snapshot ResolvedLayout's fields (Vec2 is Copy + PartialEq;
    // ResolvedLayout itself derives only Clone, not Copy or PartialEq —
    // see crates/buiy_core/src/components.rs).
    app.update();
    let pos_after_first_frame = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout written on frame 1")
        .position;
    let size_after_first_frame = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout written on frame 1")
        .size;

    // Frame 2: nothing has changed since frame 1. sync_styles trigger
    // query must yield zero entities.
    app.update();
    let count = count_changed(app.world_mut());
    assert_eq!(
        count, 0,
        "sync_styles trigger should be empty in steady-state frame 2"
    );

    // Mutate ScrollOffset. This is the operation that must NOT invalidate.
    {
        let mut offset = app
            .world_mut()
            .get_mut::<ScrollOffset>(entity)
            .expect("entity has ScrollOffset");
        offset.y = 50.0;
    }

    // Frame 3: ScrollOffset changed; sync_styles trigger query should
    // STILL yield zero entities (ScrollOffset is excluded from the filter).
    let count_after_offset_mutation = count_changed(app.world_mut());
    assert_eq!(
        count_after_offset_mutation, 0,
        "ScrollOffset mutation must not enter sync_styles' trigger set"
    );

    // Run frame 3 and verify ResolvedLayout fields are unchanged from frame 1.
    app.update();
    let pos_after_offset_mutation = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout still present")
        .position;
    let size_after_offset_mutation = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout still present")
        .size;
    assert_eq!(
        pos_after_first_frame, pos_after_offset_mutation,
        "ResolvedLayout.position must be unchanged across a scroll-only frame"
    );
    assert_eq!(
        size_after_first_frame, size_after_offset_mutation,
        "ResolvedLayout.size must be unchanged across a scroll-only frame"
    );
}

#[test]
fn mutating_scroll_snap_item_does_not_trigger_sync_styles() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(100.0)),
                    height: Sizing::Length(Length::Px(100.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(50.0)),
                    height: Sizing::Length(Length::Px(50.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
            ScrollSnapItem::default(),
            ChildOf(parent),
        ))
        .id();

    app.update();
    app.update();
    assert_eq!(count_changed(app.world_mut()), 0, "steady-state");

    {
        let mut item = app
            .world_mut()
            .get_mut::<ScrollSnapItem>(child)
            .expect("child has ScrollSnapItem");
        item.align = SnapAlign::Center;
    }

    assert_eq!(
        count_changed(app.world_mut()),
        0,
        "ScrollSnapItem mutation must not enter sync_styles' trigger set"
    );
}
