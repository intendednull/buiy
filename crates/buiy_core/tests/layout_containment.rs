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
fn content_visibility_auto_warns_once() {
    use buiy_core::layout::ContentVisibility;
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().containment(Containment {
                content_visibility: ContentVisibility::Auto,
                ..Default::default()
            }),
        ))
        .id();
    app.update();
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e))
    );
}

#[test]
fn content_visibility_hidden_also_warns() {
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
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(e))
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
fn content_visibility_deferred_warns_once_per_entity_across_three() {
    use buiy_core::layout::ContentVisibility;
    let mut app = app();
    let mk = |app: &mut App| {
        app.world_mut()
            .spawn((
                Node,
                Style::default().containment(Containment {
                    content_visibility: ContentVisibility::Auto,
                    ..Default::default()
                }),
            ))
            .id()
    };
    let a = mk(&mut app);
    let b = mk(&mut app);
    let c = mk(&mut app);
    app.update();
    // run a second frame — dedup must hold (no panic / re-warn observable
    // via the set, which persists per session).
    app.update();

    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(a))
    );
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(b))
    );
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::ContentVisibilityDeferred(c))
    );
    // Exactly three content-vis keys (one per entity), no duplicates.
    let count = warned
        .set
        .iter()
        .filter(|k| matches!(k, LayoutWarnOnceKey::ContentVisibilityDeferred(_)))
        .count();
    assert_eq!(
        count, 3,
        "one warn-once key per entity, deduped across frames"
    );
}
