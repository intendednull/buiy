use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    picking::{PickingPlugin, hit_test},
};

#[test]
fn hit_test_returns_entity_under_point() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            ResolvedLayout {
                position: Vec2::new(10.0, 10.0),
                size: Vec2::new(100.0, 50.0),
            },
        ))
        .id();

    let world = app.world();
    let hit = hit_test(world, Vec2::new(50.0, 30.0));
    assert_eq!(hit, Some(entity));
    let miss = hit_test(world, Vec2::new(500.0, 500.0));
    assert_eq!(miss, None);
}
