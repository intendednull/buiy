use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yLabel, A11yPlugin, A11yRole, A11yTreeBuilder},
    focus::Focusable,
};

#[test]
fn adapter_plugin_loads_without_panic() {
    use bevy::winit::accessibility::ACCESS_KIT_ADAPTERS;
    use buiy_core::a11y::AccessKitAdapterPlugin;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_plugins(AccessKitAdapterPlugin);
    // `FocusPlugin::handle_tab` reads `Res<ButtonInput<KeyCode>>`; MinimalPlugins
    // doesn't include `InputPlugin`, so we seed the resource manually —
    // same pattern used in `tests/focus.rs`.
    app.init_resource::<ButtonInput<KeyCode>>();
    // The plugin must install `push_tree_updates` without panicking, even
    // when no winit windows exist. Real adapter creation is exercised by
    // running the `hello_button` example end-to-end.
    app.update();
    // bevy_winit's `ACCESS_KIT_ADAPTERS` thread-local is the source of truth
    // for which windows have AccessKit adapters. Under MinimalPlugins no
    // winit windows are spawned, so the map stays empty.
    let bevy_adapters_empty = ACCESS_KIT_ADAPTERS.with_borrow(|m| m.0.is_empty());
    assert!(
        bevy_adapters_empty,
        "no bevy_winit adapters created under MinimalPlugins"
    );
}

#[test]
fn tree_builder_emits_one_node_per_focusable_with_role_and_label() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let _btn = app
        .world_mut()
        .spawn((
            Focusable::default(),
            A11yRole::Button,
            A11yLabel("Save".to_string()),
        ))
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let snapshot = builder.snapshot();
    let count = snapshot
        .iter()
        .filter(|n| n.role == A11yRole::Button)
        .count();
    assert_eq!(count, 1, "exactly one button node in tree");
    let names: Vec<String> = snapshot.iter().map(|n| n.name.clone()).collect();
    assert!(names.contains(&"Save".to_string()), "Save name present");
}
