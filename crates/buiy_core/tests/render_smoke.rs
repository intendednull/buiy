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

// Headless load smoke. Under MinimalPlugins + CorePlugin there is NO RenderApp,
// so the `RenderApp` branch of `BuiyRenderPlugin::build` (where
// `extract_buiy_nodes` + `ExtractedNodesView` are registered) never runs. This
// test therefore does NOT cover the Task 7 extract-system wiring — it only
// proves the plugin's main-world `build` does not panic and that the public
// `render::extract` module path resolves (a compile-time fact). The actual
// registration is guarded by the GPU `#[ignore]` smoke
// `extract_buiy_nodes_registered_in_extract_schedule` below, which builds a
// live RenderApp and asserts `ExtractedNodesView` is present.
#[test]
fn buiy_render_plugin_loads_headless_without_render_app() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.update();

    // No RenderApp here, so only the module path is exercised: this resolves the
    // public `render::extract::ExtractedNodes` type at compile time.
    let _ = buiy_core::render::extract::ExtractedNodes::default();
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

// Number of systems `BuiyRenderPlugin` adds to `ExtractSchedule`: the Phase-0
// `extract_buiy_draws` and the per-view `extract_buiy_nodes` (render/mod.rs).
// This is the delta the membership test below asserts; bump it in lockstep
// whenever the plugin's `ExtractSchedule` registrations change.
const BUIY_EXTRACT_SYSTEM_COUNT: usize = 2;

// Count the systems in a RenderApp's `ExtractSchedule`. Reads the schedule
// graph directly (`graph().systems`), which is populated immediately at
// `add_systems` time — no executor initialization (hence no schedule run, hence
// no extra device work) is required, so this is a pure introspection of which
// systems were registered.
fn extract_schedule_system_count(app: &mut bevy::app::App) -> usize {
    use bevy::prelude::Schedules;
    use bevy::render::{ExtractSchedule, RenderApp};

    let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
    render_app
        .world()
        .resource::<Schedules>()
        .get(ExtractSchedule)
        .expect("ExtractSchedule present in the RenderApp")
        .graph()
        .systems
        .len()
}

// Same RenderApp/wgpu-adapter caveat as `pipeline_registers_in_render_app`:
// constructing the RenderApp needs a wgpu adapter (real GPU or lavapipe), so
// this rides the #[ignore] GPU path (architecture § 7 / verification § 2.1).
// Note: only *building* the RenderApp needs the device — walking the schedule
// graph to assert membership does NOT (the graph is populated at `add_systems`
// time, before any schedule run). This is the guard for the Task 7
// registration: the headless smoke above cannot reach the RenderApp branch, so
// the `extract_buiy_nodes` wiring is exercised only here. The device-free
// behavior of the extract mapping is covered headlessly in
// tests/render_extract.rs.
//
// Membership is asserted by a baseline delta rather than by system *name*:
// `System::name()` resolves to a placeholder ("<Enable the debug feature ...>")
// unless `bevy_utils/debug` is enabled, which this workspace does not enable, so
// a `name().contains("extract_buiy_nodes")` match would never fire. Instead we
// count the `ExtractSchedule` systems in a RenderApp built WITHOUT
// `BuiyRenderPlugin` and assert that adding the plugin grows the schedule by
// exactly the Buiy extract-system count. Deleting an `add_systems(ExtractSchedule, …)`
// line in render/mod.rs (the regression this test name promises to catch) drops
// the delta below `BUIY_EXTRACT_SYSTEM_COUNT` and fails the assertion.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by the e2e harness"]
fn extract_buiy_nodes_registered_in_extract_schedule() {
    use bevy::render::RenderApp;

    // Baseline: RenderApp with Bevy's own extract systems but none of ours.
    let mut baseline = App::new();
    baseline.add_plugins(MinimalPlugins);
    baseline.add_plugins(bevy::asset::AssetPlugin::default());
    baseline.add_plugins(bevy::render::RenderPlugin::default());
    let baseline_count = extract_schedule_system_count(&mut baseline);

    // With BuiyRenderPlugin added, the ExtractSchedule must gain exactly the
    // Buiy extract systems (incl. extract_buiy_nodes).
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    // The carrier resource is still initialized in the RenderApp branch.
    assert!(
        app.get_sub_app(RenderApp)
            .expect("RenderApp")
            .world()
            .get_resource::<buiy_core::render::extract::ExtractedNodesView>()
            .is_some(),
        "ExtractedNodesView initialized in the RenderApp"
    );

    let with_plugin_count = extract_schedule_system_count(&mut app);
    assert_eq!(
        with_plugin_count - baseline_count,
        BUIY_EXTRACT_SYSTEM_COUNT,
        "BuiyRenderPlugin must register {BUIY_EXTRACT_SYSTEM_COUNT} systems in \
         ExtractSchedule (extract_buiy_draws + extract_buiy_nodes); got a delta \
         of {} — a missing add_systems(ExtractSchedule, …) in render/mod.rs",
        with_plugin_count - baseline_count,
    );
}
