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

// Same RenderApp/wgpu-adapter caveat as `pipeline_registers_in_render_app`.
// We assert that the Buiy node is present in the Core2d sub-graph after the
// main 2D pass has been wired up by Bevy's Core2dPlugin.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
fn render_graph_node_inserted_after_main_2d_pass() {
    use bevy::core_pipeline::core_2d::graph::Core2d;
    use bevy::render::render_graph::RenderGraph;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let render_graph = render_app
        .world()
        .get_resource::<RenderGraph>()
        .expect("RenderGraph resource");
    let sub = render_graph
        .get_sub_graph(Core2d)
        .expect("Core2d sub-graph present");
    assert!(
        sub.get_node_state(buiy_core::render::node::BuiyRenderLabel)
            .is_ok(),
        "BuiyRenderLabel registered in Core2d"
    );
}
