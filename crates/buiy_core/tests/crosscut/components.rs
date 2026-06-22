use bevy::prelude::*;
use buiy_core::{CorePlugin, components::*, layout::Style};

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn node_and_resolved_layout_are_registered_and_default_constructible() {
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
        registry
            .get(std::any::TypeId::of::<ResolvedLayout>())
            .is_some(),
        "ResolvedLayout not registered"
    );
    drop(registry);
    let world = app.world_mut();
    let entity = world.spawn((Node::default(), Style::default())).id();
    assert!(world.get::<Node>(entity).is_some());
    // Style is a Bundle (not a reflectable Component); the underlying
    // decomposed components are visible post-spawn.
    assert!(
        world.get::<buiy_core::layout::BoxModel>(entity).is_some(),
        "BoxModel inserted via Style::default()"
    );
}
