//! Phase 11 — `content-visibility: auto` / `hidden` layout enforcement.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.2 + § 7.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy_core::{
    CorePlugin, Node, ResolvedLayout,
    layout::{
        BoxModel, ContainerQuery, Containment, ContentVisibility, ContentVisibilityMargin, Inset,
        LayoutPlugin, LayoutTree, Length, Position, PositionKind, QueryCondition, Sizing, Style,
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
                })
                .contain_intrinsic_size(Some(120.0), Some(40.0)),
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
                })
                .contain_intrinsic_size(Some(120.0), Some(40.0)),
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

/// D8: the same-frame container-query re-run (`cq_flip_rerun`) must
/// reproduce the content-visibility skip — sentinel size + descendant
/// detach — or a CQ flip frame would re-lay-out a skipped subtree and undo
/// the skip.
///
/// The fixture nests an off-screen `content-visibility: auto` node with a
/// hint + a sized child alongside a sibling that owns a `ContainerQuery`. We
/// first run several frames so the auto node reaches a steady, skipped state
/// (off-screen last-frame geometry + sentinel size, descendants detached).
/// THEN we flip the query condition so `cq_flip_check` signals a flip and
/// `cq_flip_rerun` runs on a frame where the skip is already active. If the
/// re-run did not honor the skip set (T5's placeholder empty set), it would
/// re-attach the auto node's child during that re-run; the assertions below
/// catch that.
#[test]
fn skip_survives_container_query_flip_frame() {
    let mut app = app();
    with_window(&mut app, 800, 600);

    let child = app
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
                })
                .contain_intrinsic_size(Some(120.0), Some(40.0)),
        ))
        .add_child(child)
        .id();

    // A query container whose condition we flip later to force `cq_flip_rerun`
    // on a frame where the auto node is already in its skipped steady state.
    let queried = app
        .world_mut()
        .spawn((
            Node,
            Style::default().height_px(100.0).container_size(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::px(300.0))],
            },
        ))
        .id();
    // The query resolves to its nearest ancestor container — `root`. Its width
    // gates the rule: 400 >= 300 = active. We later shrink it below 300 so the
    // rule deactivates. Because the size change resolves THIS frame in Taffy
    // while `cq_activate` (step 1.5) reads last-frame `ResolvedLayout`, the two
    // disagree → `cq_flip_check` signals a same-frame `cq_flip_rerun` (D8).
    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(400.0)
                .height_px(600.0)
                .container_size(),
        ))
        .add_children(&[auto, queried])
        .id();

    // Reach steady state: the auto node is off-screen + skipped, the query is
    // active (400 >= 300), and no flip is pending.
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        taffy_child_count(&app, auto),
        0,
        "auto node is skipped at steady state before the flip"
    );

    // Shrink the container below the query threshold. The new size resolves in
    // this frame's Taffy compute, but `cq_activate` already evaluated against
    // the stale (400) last-frame size — so `cq_flip_check` detects a same-frame
    // flip and triggers `cq_flip_rerun` while the auto node's skip is active.
    app.world_mut()
        .get_mut::<BoxModel>(root)
        .expect("root box model present")
        .width = Sizing::Length(Length::px(200.0));
    app.update();

    let ar = app
        .world()
        .get::<ResolvedLayout>(auto)
        .expect("auto resolves");
    assert_eq!(
        ar.size,
        Vec2::new(120.0, 40.0),
        "sentinel survives the CQ flip frame"
    );
    assert_eq!(
        taffy_child_count(&app, auto),
        0,
        "detached child stays detached across the CQ flip re-run"
    );
}

#[test]
fn phase11_types_are_registered() {
    let mut app = app();
    app.update();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    assert!(
        registry
            .get_with_type_path("buiy_core::layout::components::ContainIntrinsicSize")
            .is_some(),
        "ContainIntrinsicSize not registered"
    );
}

/// spec § 5.2: a `content-visibility: auto` subtree "snaps back to full
/// layout when on-screen." We drive the entity off-screen (skipped:
/// sentinel size + detached child), then move it on-screen and assert the
/// skip releases — the child re-attaches to the Taffy tree and is laid out
/// at its real size again.
#[test]
fn auto_snaps_back_on_screen() {
    let mut app = app();
    with_window(&mut app, 800, 600);
    let child = app
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
                    ..Default::default()
                })
                .containment(Containment {
                    content_visibility: ContentVisibility::Auto,
                    ..Default::default()
                })
                .contain_intrinsic_size(Some(120.0), Some(40.0)),
        ))
        .add_child(child)
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(auto)
        .id();
    app.update();
    app.update();
    // Skipped (sentinel) + detached while off-screen.
    assert_eq!(
        app.world().get::<ResolvedLayout>(auto).unwrap().size,
        Vec2::new(120.0, 40.0),
        "off-screen auto uses the sentinel size"
    );
    assert_eq!(
        taffy_child_count(&app, auto),
        0,
        "off-screen auto's descendant is detached"
    );

    // Move it on-screen.
    {
        let mut e = app.world_mut().entity_mut(auto);
        let mut pos = e.get_mut::<Position>().unwrap();
        pos.inset.left = Sizing::Length(Length::px(10.0));
    }
    app.update();
    app.update();

    // Snapped back: the child is re-attached and laid out again at 50x50.
    assert_eq!(
        taffy_child_count(&app, auto),
        1,
        "descendant re-attaches to the Taffy tree on snap-back"
    );
    let cr = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("child re-laid-out");
    assert_eq!(
        cr.size,
        Vec2::new(50.0, 50.0),
        "descendant snaps back to its real size on-screen"
    );
}

/// spec § 7: "content-visibility: auto skips off-screen — off-screen child
/// has `ContentVisibility::Auto`; assert child is not in step 1's
/// translation set when off-screen." We assert the observable consequence:
/// an off-screen auto node's descendants are detached from the Taffy tree
/// (not laid out) while an identical on-screen sibling's descendants are
/// laid out normally.
#[test]
fn auto_skips_off_screen_spec_section_7() {
    let mut app = app();
    with_window(&mut app, 800, 600);

    let mk_auto = |app: &mut App, left: f32| -> (Entity, Entity) {
        let grandchild = app
            .world_mut()
            .spawn((Node, Style::default().width_px(30.0).height_px(30.0)))
            .id();
        let auto = app
            .world_mut()
            .spawn((
                Node,
                Style::default()
                    .position(PositionKind::Absolute)
                    .inset(Inset {
                        left: Sizing::Length(Length::px(left)),
                        ..Default::default()
                    })
                    .containment(Containment {
                        content_visibility: ContentVisibility::Auto,
                        ..Default::default()
                    })
                    .contain_intrinsic_size(Some(60.0), Some(60.0)),
            ))
            .add_child(grandchild)
            .id();
        (auto, grandchild)
    };
    let (off_auto, _off_gc) = mk_auto(&mut app, 5000.0); // off-screen
    let (on_auto, on_gc) = mk_auto(&mut app, 10.0); // on-screen
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[off_auto, on_auto])
        .id();
    app.update();
    app.update();

    // Off-screen: sentinel size + detached grandchild.
    assert_eq!(
        app.world().get::<ResolvedLayout>(off_auto).unwrap().size,
        Vec2::new(60.0, 60.0),
        "off-screen auto uses the sentinel"
    );
    assert_eq!(
        taffy_child_count(&app, off_auto),
        0,
        "off-screen auto's descendant is detached from the Taffy tree"
    );
    // On-screen: descendant attached + laid out at its real size.
    assert_eq!(
        taffy_child_count(&app, on_auto),
        1,
        "on-screen auto keeps its descendant attached"
    );
    assert_eq!(
        app.world().get::<ResolvedLayout>(on_gc).unwrap().size,
        Vec2::new(30.0, 30.0),
        "on-screen auto lays out its descendant"
    );
}

/// D3: the `ContentVisibilityMargin` resource controls the hysteresis
/// dead-band. With the margin shrunk to 0, an entity just past the
/// viewport edge counts as off-screen and skips.
#[test]
fn margin_resource_controls_hysteresis_band() {
    let mut app = app();
    with_window(&mut app, 800, 600);
    // Shrink the margin to 0 so an entity just past the viewport edge skips.
    app.insert_resource(ContentVisibilityMargin(0.0));
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(20.0).height_px(20.0)))
        .id();
    let auto = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Absolute)
                .inset(Inset {
                    left: Sizing::Length(Length::px(900.0)), // past 800, margin 0 → off-screen
                    ..Default::default()
                })
                .containment(Containment {
                    content_visibility: ContentVisibility::Auto,
                    ..Default::default()
                })
                .contain_intrinsic_size(Some(50.0), Some(50.0)),
        ))
        .add_child(child)
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(auto)
        .id();
    app.update();
    app.update();
    assert_eq!(
        app.world().get::<ResolvedLayout>(auto).unwrap().size,
        Vec2::new(50.0, 50.0),
        "with margin 0, an entity past the viewport edge skips"
    );
    assert_eq!(
        taffy_child_count(&app, auto),
        0,
        "with margin 0, the off-screen entity's descendant is detached"
    );
}
