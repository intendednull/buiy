use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{FlexDirection, Node, ResolvedLayout, Style},
    layout::{LayoutPlugin, LayoutTree},
};

#[test]
fn layout_resolves_a_simple_flex_row() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style {
                width: 200.0,
                height: 100.0,
                flex_direction: FlexDirection::Row,
                ..default()
            },
        ))
        .id();

    let child = app
        .world_mut()
        .spawn((
            Node,
            Style {
                width: 50.0,
                height: 50.0,
                ..default()
            },
        ))
        .id();

    app.world_mut().entity_mut(parent).add_child(child);

    app.update(); // run BuiySet::Layout

    let layout = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("child has ResolvedLayout after Update");
    assert!((layout.size.x - 50.0).abs() < 0.5, "child width ~ 50");
    assert!((layout.size.y - 50.0).abs() < 0.5, "child height ~ 50");
}

#[test]
fn layout_tree_garbage_collects_despawned_entities() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style {
                width: 100.0,
                height: 100.0,
                ..default()
            },
        ))
        .id();

    app.update();
    assert_eq!(
        app.world().non_send_resource::<LayoutTree>().len(),
        1,
        "spawned entity registered in LayoutTree after first update",
    );

    app.world_mut().entity_mut(entity).despawn();
    app.update();

    assert!(
        app.world().non_send_resource::<LayoutTree>().is_empty(),
        "despawned entity dropped from LayoutTree by gc system",
    );
}
