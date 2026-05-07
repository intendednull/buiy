use bevy::prelude::*;
use buiy_core::{CorePlugin, render::BuiyRenderPlugin};

#[test]
fn render_plugin_loads_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // BuiyRenderPlugin needs a render-app context normally, but Phase 0
    // smoke test asserts the plugin's `build` does not panic when added
    // without RenderApp. Real render assertions happen in the e2e test (Task 19).
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.update();
}
