//! Phase 8 — containment layout effects.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout,
    layout::{ContainFlags, LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession, Style},
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
