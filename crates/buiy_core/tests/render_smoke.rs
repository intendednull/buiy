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

// Requires a real (or software, e.g. lavapipe) Vulkan/Metal/DX adapter:
// `RenderPlugin::build` does `block_on(initialize_renderer(...))` which
// `expect()`s a wgpu adapter. Headless CI without a GPU/lavapipe panics
// before our pipeline code runs. Keep the test for environments that
// have a GPU; the proper end-to-end render coverage lives in the visual
// regression harness (Task 19), which provisions lavapipe.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
fn pipeline_registers_in_render_app() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // RenderPlugin requires AssetPlugin (Shader is an Asset).
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let pipeline = render_app
        .world()
        .get_resource::<buiy_core::render::pipeline::BuiyPipeline>();
    assert!(pipeline.is_some(), "BuiyPipeline registered");
}
