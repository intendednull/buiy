//! BoxSizing::ContentBox vs BorderBox produce the spec-mandated widths.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 2.2 + § 6.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{LayoutPlugin, Style},
};

#[test]
fn content_box_treats_width_as_content_box() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .padding(10.0)
                .content_box(),
        ))
        .id();

    app.update();
    let rl = app.world().get::<ResolvedLayout>(entity).unwrap();
    // ContentBox: total width = content (100) + padding-left (10) + padding-right (10) = 120
    assert!(
        (rl.size.x - 120.0).abs() < 0.5,
        "ContentBox: total width should be 120 (100 content + 20 padding); got {}",
        rl.size.x
    );
}

#[test]
fn border_box_treats_width_as_border_box() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .padding(10.0)
                .border_box(),
        ))
        .id();

    app.update();
    let rl = app.world().get::<ResolvedLayout>(entity).unwrap();
    // BorderBox: total width = 100 (set value); content box is 100-20 = 80.
    assert!(
        (rl.size.x - 100.0).abs() < 0.5,
        "BorderBox: total width should be 100; got {}",
        rl.size.x
    );
}
