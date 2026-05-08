use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{FlexDirection, Node, ResolvedLayout, Style},
    layout::LayoutPlugin,
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
