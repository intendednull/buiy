//! Phase 11 — `content-visibility: auto` / `hidden` layout enforcement.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.2 + § 7.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy_core::{
    CorePlugin, Node, ResolvedLayout,
    layout::{
        ContainIntrinsicSize, Containment, ContentVisibility, Inset, LayoutPlugin, LayoutTree,
        Length, PositionKind, Sizing, Style,
    },
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

/// The number of Taffy children attached to `entity`'s node — the
/// load-bearing detach probe. A skipped entity (Auto-sentinel / Hidden)
/// has an empty Taffy child list even though its descendants' nodes
/// remain in `LayoutTree` (D4). A detached descendant keeps its STALE
/// last-computed Taffy layout (so its `ResolvedLayout` is not zeroed);
/// the verifiable proof of the skip is therefore the parent's child
/// count, not the descendant's resolved size.
fn taffy_child_count(app: &App, entity: Entity) -> usize {
    let tree = app
        .world()
        .get_non_send_resource::<LayoutTree>()
        .expect("LayoutTree present");
    let id = tree.by_entity()[&entity];
    tree.tree_ref()
        .children(id)
        .expect("node present in Taffy tree")
        .len()
}

/// Spawn a primary window so `sync_styles`' viewport read is well-defined
/// (D5). Without this the window-less viewport is `ZERO` and the off-screen
/// test degenerates.
fn with_window(app: &mut App, w: u32, h: u32) {
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(w, h),
            ..Default::default()
        },
        PrimaryWindow,
    ));
}

#[test]
fn hidden_detaches_descendant_from_layout() {
    let mut app = app();
    // child has an explicit 50x50 size; if it were laid out it would resolve to 50x50.
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();
    let hidden = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(200.0)
                .height_px(100.0)
                .containment(Containment {
                    content_visibility: ContentVisibility::Hidden,
                    ..Default::default()
                }),
        ))
        .add_child(child)
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(hidden)
        .id();
    app.update();
    app.update(); // second frame so detach is stable

    // The Hidden entity itself still resolves its own box (D7).
    let hr = app
        .world()
        .get::<ResolvedLayout>(hidden)
        .expect("hidden box resolves");
    assert_eq!(
        hr.size,
        Vec2::new(200.0, 100.0),
        "hidden entity keeps its own box"
    );
    // The descendant is detached from the Hidden node's Taffy child list, so
    // Taffy never lays it out (its own node stays in LayoutTree for cheap
    // snap-back — D4). The verifiable proof is the empty child list.
    assert_eq!(
        taffy_child_count(&app, hidden),
        0,
        "descendant of a content-visibility:hidden node is detached from the Taffy tree"
    );
}

#[test]
fn auto_off_screen_with_hint_applies_sentinel_and_detaches() {
    let mut app = app();
    with_window(&mut app, 800, 600);
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();
    // Auto entity positioned far off-screen (x = 5000, well past 800 + margin),
    // absolutely positioned so its ResolvedLayout.position reflects the inset.
    // `ContainIntrinsicSize` is inserted directly (the `.contain_intrinsic_size`
    // setter lands in T8); the off-screen skip reads it via `Option<&...>`.
    let auto = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Absolute)
                .inset(Inset {
                    left: Sizing::Length(Length::px(5000.0)),
                    top: Sizing::Length(Length::px(0.0)),
                    ..Default::default()
                })
                .containment(Containment {
                    content_visibility: ContentVisibility::Auto,
                    ..Default::default()
                }),
            ContainIntrinsicSize {
                width: Some(120.0),
                height: Some(40.0),
            },
        ))
        .add_child(child)
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(auto)
        .id();
    // Frame 1 establishes the off-screen ResolvedLayout (no skip yet — no last
    // frame). Frame 2 reads frame-1 geometry and applies the skip.
    app.update();
    app.update();

    // Auto entity took the sentinel size (120x40), not its measured size.
    let ar = app
        .world()
        .get::<ResolvedLayout>(auto)
        .expect("auto box resolves");
    assert_eq!(
        ar.size,
        Vec2::new(120.0, 40.0),
        "off-screen auto uses contain-intrinsic-size"
    );
    // child detached from the auto node's Taffy child list → not laid out.
    assert_eq!(
        taffy_child_count(&app, auto),
        0,
        "descendant of an off-screen content-visibility:auto node is detached from the Taffy tree"
    );
}

/// Regression: the skip must hold at STEADY STATE, not just on the
/// transient frames where the entity is still in the `Changed` set.
///
/// The skip classification (`skip_children` / `sentinel_size`) is computed
/// over the FULL tree, not the `Changed`-filtered `nodes` loop. If it were
/// only populated for changed entities, an off-screen `Auto` node (or any
/// `Hidden` node) would lose its `skip_children` membership once it reached
/// steady-state and dropped out of the `Changed` set, and the children-sync
/// pass would silently RE-ATTACH its descendants every steady-state frame —
/// defeating the feature (spec § 5.2 "the big perf win"). The earlier 2-frame
/// tests masked this because the entity is still `Changed` on frame 2 from its
/// frame-1 insertion / `ResolvedLayout` shift; this probe runs well past that.
#[test]
fn skip_holds_at_steady_state() {
    let mut app = app();
    with_window(&mut app, 800, 600);

    // Off-screen content-visibility:auto with a hint + a sized child.
    let auto_child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();
    let auto = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Absolute)
                .inset(Inset {
                    left: Sizing::Length(Length::px(5000.0)),
                    top: Sizing::Length(Length::px(0.0)),
                    ..Default::default()
                })
                .containment(Containment {
                    content_visibility: ContentVisibility::Auto,
                    ..Default::default()
                }),
            ContainIntrinsicSize {
                width: Some(120.0),
                height: Some(40.0),
            },
        ))
        .add_child(auto_child)
        .id();

    // content-visibility:hidden with a sized child (geometry-independent).
    let hidden_child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();
    let hidden = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(200.0)
                .height_px(100.0)
                .containment(Containment {
                    content_visibility: ContentVisibility::Hidden,
                    ..Default::default()
                }),
        ))
        .add_child(hidden_child)
        .id();

    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[auto, hidden])
        .id();

    // Run well past the transient `Changed` frames so both entities reach
    // steady state and drop out of the `Changed`-filtered `nodes` query.
    for _ in 0..10 {
        app.update();
    }

    assert_eq!(
        taffy_child_count(&app, auto),
        0,
        "off-screen content-visibility:auto descendant stays detached at steady state"
    );
    assert_eq!(
        taffy_child_count(&app, hidden),
        0,
        "content-visibility:hidden descendant stays detached at steady state"
    );
    // The auto entity must also keep its sentinel size at steady state.
    let ar = app
        .world()
        .get::<ResolvedLayout>(auto)
        .expect("auto box resolves");
    assert_eq!(
        ar.size,
        Vec2::new(120.0, 40.0),
        "off-screen auto keeps its contain-intrinsic-size at steady state"
    );
}
