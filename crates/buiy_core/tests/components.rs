use bevy::prelude::*;
use buiy_core::{CorePlugin, components::*};

#[test]
// `Node::default()` is intentional: the test asserts default-constructibility
// per the architecture commitment (Reflect + Default + Clone + Component on
// every Buiy component), even though `Node` is currently a unit struct.
#[allow(clippy::default_constructed_unit_structs)]
fn node_and_style_are_registered_and_default_constructible() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);

    let registry = app.world().resource::<AppTypeRegistry>();
    let registry = registry.read();
    assert!(
        registry.get(std::any::TypeId::of::<Node>()).is_some(),
        "Node not registered"
    );
    assert!(
        registry.get(std::any::TypeId::of::<Style>()).is_some(),
        "Style not registered"
    );
    assert!(
        registry
            .get(std::any::TypeId::of::<ResolvedLayout>())
            .is_some(),
        "ResolvedLayout not registered"
    );

    drop(registry);
    let world = app.world_mut();
    let entity = world.spawn((Node::default(), Style::default())).id();
    assert!(world.get::<Node>(entity).is_some());
    assert!(world.get::<Style>(entity).is_some());
}
