//! Phase 8 — containment layout effects.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout,
    layout::{
        ContainFlags, Containment, LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession, Style,
    },
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

#[test]
fn size_containment_zeroes_auto_width_and_warns() {
    let mut app = app();
    // contain: size, width: auto (default) → Taffy width 0.
    let e = app
        .world_mut()
        .spawn((Node, Style::default().contain(ContainFlags::SIZE)))
        .id();
    app.update();

    let rl = app.world().get::<ResolvedLayout>(e).expect("resolved");
    assert_eq!(rl.size.x, 0.0, "size containment zeroes auto width");
    assert_eq!(rl.size.y, 0.0, "size containment zeroes auto height");

    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::SizeContainmentZeroed(e)),
        "size-containment-zeroed warn recorded"
    );
}

#[test]
fn content_visibility_auto_on_screen_does_not_warn() {
    // Phase 11 D6: the blanket "content-visibility != visible is deferred"
    // warn is gone. An on-screen `auto` entity lays out normally and never
    // warns. (The repurposed `ContentVisibilityDeferred` warn now fires only
    // for off-screen `auto` without a `contain-intrinsic-size` hint — exercised
    // in tests/layout_content_visibility.rs.)
    use bevy::window::{PrimaryWindow, Window, WindowResolution};
    use buiy_core::layout::ContentVisibility;
    let mut app = app();
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(800, 600),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    // Sized + at the origin → squarely inside the (expanded) viewport.
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .containment(Containment {
                    content_visibility: ContentVisibility::Auto,
                    ..Default::default()
                }),
        ))
        .id();
    app.update();
    app.update();
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        !warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)),
        "on-screen content-visibility:auto lays out normally and does not warn"
    );
}

#[test]
fn content_visibility_hidden_does_not_warn() {
    // Phase 11 D6/D7: `hidden` is fully implemented (descendants detached) and
    // never warns.
    use buiy_core::layout::ContentVisibility;
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().containment(Containment {
                content_visibility: ContentVisibility::Hidden,
                ..Default::default()
            }),
        ))
        .id();
    app.update();
    app.update();
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        !warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)),
        "content-visibility:hidden is fully implemented and does not warn"
    );
}

#[test]
fn will_change_does_not_warn() {
    use buiy_core::layout::{WillChange, WillChangeProperty};
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().containment(Containment {
                will_change: WillChange::Properties(vec![WillChangeProperty::Transform]),
                ..Default::default()
            }),
        ))
        .id();
    app.update();
    // will-change is a valid stored hint — no warn-once key for it.
    // (Negative assertion: no ContentVisibilityDeferred / SizeContainmentZeroed
    // fires because content_visibility = Visible and size is not contained.)
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        !warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e))
    );
    assert!(
        !warned
            .set
            .contains(&LayoutWarnOnceKey::SizeContainmentZeroed(e))
    );
}

#[test]
fn content_visibility_auto_off_screen_without_hint_warns_once() {
    // Phase 11 D6: the `ContentVisibilityDeferred` warn-once key is repurposed.
    // It now fires exactly once per entity for the one residual degenerate
    // case — a `content-visibility: auto` entity that is off-screen but carries
    // NO `contain-intrinsic-size` hint, so the requested off-screen layout skip
    // cannot run (D2) and the subtree lays out anyway.
    use bevy::window::{PrimaryWindow, Window, WindowResolution};
    use buiy_core::layout::{ContentVisibility, Inset, Length, PositionKind, Sizing};
    let mut app = app();
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(800, 600),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    // Auto + off-screen + NO contain-intrinsic-size hint → D6 diagnostic.
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(10.0)
                .height_px(10.0)
                .position(PositionKind::Absolute)
                .inset(Inset {
                    left: Sizing::Length(Length::px(5000.0)),
                    ..Default::default()
                })
                .containment(Containment {
                    content_visibility: ContentVisibility::Auto,
                    ..Default::default()
                }),
        ))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(e)
        .id();
    app.update(); // establishes off-screen geometry
    app.update(); // frame 2 sees last-frame off-screen → D6 warn
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e)),
        "off-screen auto without contain-intrinsic-size warns (D6 repurposed)"
    );
    // dedup: a third frame does not add a duplicate.
    app.update();
    let count = app
        .world()
        .resource::<LayoutWarnedOnceSession>()
        .set
        .iter()
        .filter(|k| matches!(k, LayoutWarnOnceKey::ContentVisibilityDeferred(_)))
        .count();
    assert_eq!(count, 1, "one D6 warn per entity, deduped across frames");
}
