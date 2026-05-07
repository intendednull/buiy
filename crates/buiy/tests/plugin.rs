use bevy::prelude::*;
use buiy::BuiyPlugin;

#[test]
fn buiy_plugin_loads_in_correct_order() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // `BuiyPlugin` composes systems that read keyboard / pointer input
    // (focus tab handling, button click). `MinimalPlugins` does not include
    // `InputPlugin`, so we add it here so `app.update()` doesn't panic on
    // missing `ButtonInput<KeyCode>` / `ButtonInput<MouseButton>` resources.
    app.add_plugins(bevy::input::InputPlugin);
    app.add_plugins(BuiyPlugin);
    app.update();

    // Sanity: re-exports are accessible.
    let _ = std::any::TypeId::of::<buiy::Button>();
    let _ = std::any::TypeId::of::<buiy::Focusable>();
    let _ = std::any::TypeId::of::<buiy::A11yRole>();
}
