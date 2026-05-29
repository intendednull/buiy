//! Phase 9 — stacking + top layer integration tests.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 4, § 6.

use bevy::prelude::*;
use buiy_core::components::StackingContext;
use buiy_core::layout::{LayoutPlugin, Style, TopLayer, TopLayerActivation};
use buiy_core::{CorePlugin, Node};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

#[test]
fn top_layer_modal_escapes_to_root() {
    let mut app = app();
    let modal = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Modal)))
        .id();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(modal)
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(parent)
        .id();
    app.update();
    // Membership = root, not parent (spec § 4.1 / § 6 escape test).
    let root_sc = app.world().get::<StackingContext>(root).unwrap();
    assert!(
        root_sc.painters_z.contains(&modal),
        "modal escapes to root context"
    );
    // It must not also appear in any non-root context that forms one.
    if let Some(parent_sc) = app.world().get::<StackingContext>(parent) {
        assert!(
            !parent_sc.painters_z.contains(&modal),
            "modal must not be counted in its parent's context"
        );
    }
}

#[test]
fn top_layer_activation_tracks_open_order() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Popover)))
        .id();
    let b = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Popover)))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[a, b])
        .id();
    app.update();
    let act = app.world().resource::<TopLayerActivation>();
    let order: Vec<Entity> = act.order.iter().copied().collect();
    assert_eq!(
        order,
        vec![a, b],
        "activation order follows tree/open order; most recent last"
    );
}
