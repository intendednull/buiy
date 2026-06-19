use bevy::prelude::*;
use buiy_core::CorePlugin;

#[test]
fn core_plugin_loads_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.update();
}
