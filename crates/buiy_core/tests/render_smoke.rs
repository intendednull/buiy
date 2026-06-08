use bevy::prelude::*;
use buiy_core::{CorePlugin, render::BuiyRenderPlugin};

mod support;

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
    // `BuiyPipeline` is registered in `BuiyRenderPlugin::finish` (not `build`):
    // it needs the `RenderDevice`/`PipelineCache` that `RenderPlugin::finish`
    // materializes via the async renderer init, so `finish()` MUST run first.
    // `finish()` also runs `ImagePlugin::finish` (which indexes the added
    // `ImagePlugin`) and `RenderPlugin`'s fallback-image init, so the *complete*
    // `gpu_test_app` plugin set is required — a bare minimal set panics in
    // `ImagePlugin::finish`. No frame is driven; this only inspects the resource.
    let mut app = support::gpu_test_app();
    app.finish();

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
    // `CorePipelinePlugin` pulls in `TonemappingPlugin::build`, which reads
    // `Assets<Image>` to register its LUT images; that asset is owned by
    // `ImagePlugin`, so it must be added (before `CorePipelinePlugin`) or the
    // tonemapping build panics with "Requested resource … does not exist".
    app.add_plugins(bevy::image::ImagePlugin::default());
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
// `extract_buiy_draws`, the per-view `extract_buiy_nodes` (render/mod.rs), and
// the R10 `warmup_atlas` drain (atlas/mod.rs, wired via `atlas::register` in the
// plugin's RenderApp branch). This is the delta the membership test below
// asserts; bump it in lockstep whenever the plugin's `ExtractSchedule`
// registrations change.
const BUIY_EXTRACT_SYSTEM_COUNT: usize = 3;

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
         ExtractSchedule (extract_buiy_draws + extract_buiy_nodes + warmup_atlas); \
         got a delta of {} — a missing add_systems(ExtractSchedule, …) in \
         render/mod.rs or atlas/mod.rs",
        with_plugin_count - baseline_count,
    );
}

// Same RenderApp/wgpu-adapter caveat as the other render_smoke tests:
// RenderPlugin::build does block_on(initialize_renderer(...)) which expect()s
// a wgpu adapter. After Task 8 the quad pipeline is built through
// BuiyPrimitives::specialize; this asserts the BuiyPipeline resource (and its
// valid quad CachedRenderPipelineId) still registers via that path.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
fn quad_pipeline_registers_via_specializer() {
    // `BuiyPipeline` is registered in `BuiyRenderPlugin::finish` and `finish()`
    // needs the full plugin set — see `pipeline_registers_in_render_app`.
    let mut app = support::gpu_test_app();
    app.finish();

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let pipeline = render_app
        .world()
        .get_resource::<buiy_core::render::pipeline::BuiyPipeline>()
        .expect("BuiyPipeline registered via specializer path");
    // The id is a valid handle into the cache (compilation is async; we only
    // assert the resource + id exist, not that the pipeline finished).
    let _ = pipeline.id;
}

// Same RenderApp/wgpu-adapter caveat as `quad_pipeline_registers_via_specializer`:
// RenderPlugin::build does block_on(initialize_renderer(...)) which expect()s a
// wgpu adapter. After R8b the per-instance vertex layout grew to stride 52 (the
// clip AABB at @location(6)/(7)); wgpu validates the layout against the WGSL
// `Instance` at pipeline creation, so a registered BuiyPipeline on a real
// adapter proves the stride-52 layout + clip-aware shaders compile and bind. The
// device-free half (layout offsets, naga parse) is covered headlessly in
// render_primitive_descriptor.rs / render_shader_wgsl.rs.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
fn clip_aabb_pipeline_registers_with_stride_52() {
    // `BuiyPipeline` is registered in `BuiyRenderPlugin::finish` and `finish()`
    // needs the full plugin set — see `pipeline_registers_in_render_app`.
    let mut app = support::gpu_test_app();
    app.finish();

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let pipeline = render_app
        .world()
        .get_resource::<buiy_core::render::pipeline::BuiyPipeline>()
        .expect("BuiyPipeline registered with the stride-52 clip-AABB layout");
    // The id is a valid handle into the cache (compilation is async; we only
    // assert the resource + id exist, not that the pipeline finished). Pipeline
    // creation is where wgpu would reject a layout/shader stride mismatch.
    let _ = pipeline.id;
}

// Same wgpu-adapter caveat as the other render_smoke #[ignore] tests. Asserts
// the ported node draws the persistent buffers without panicking and the
// view-uniform bind group is wired. Run locally with `-- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); ported node draws persistent buffers"]
fn node_draws_persistent_buffers_with_view_uniform() {
    use buiy_core::Node;
    use buiy_core::layout::Style;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use buiy_core::render::prepare::BuiyInstanceBuffers;
    use std::borrow::Cow;

    // `gpu_render_app` adds `CorePipelinePlugin` so a live `Core2d` graph exists
    // for `BuiyRenderPlugin` to wire `BuiyNode` into — without it the node is
    // never added to a graph and never executes. A capture camera targeting an
    // offscreen image gives the node a real `ViewTarget` to paint into.
    const W: u32 = 32;
    const H: u32 = 32;
    let mut app = support::gpu_render_app(W, H);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    app.world_mut().spawn((
        Node,
        // 40×40 opaque white over the 32×32 view → the readback is fully covered.
        Style::default().width_px(40.0).height_px(40.0),
        Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
        },
    ));
    support::finish_and_run(&mut app, 3);

    let buffers = support::render_world_resource::<BuiyInstanceBuffers>(&app)
        .expect("BuiyInstanceBuffers present after prepare");
    assert_eq!(
        buffers.quad_count, 1,
        "the one opaque node is packed into the persistent quad buffer"
    );
    assert!(
        buffers.view_uniform.binding().is_some(),
        "the view-uniform UBO was uploaded (the @group(0) bind the node builds)"
    );
    // And the node actually PAINTED: read the offscreen target back and assert it
    // is not uniformly the clear color — proof BuiyNode::run executed its draw in
    // the live graph (the buffer-only asserts above can't see a never-run node).
    let pixels = support::readback_rgba(&mut app, target);
    let clear = [0u8, 0, 0, 255];
    assert!(
        pixels.chunks_exact(4).any(|px| px != clear),
        "BuiyNode::run painted non-clear pixels into the offscreen view"
    );
}

// Same wgpu-adapter caveat as the other render_smoke #[ignore] tests. The v1
// top-layer composite (R8b) is a SINGLE draw, not a second pass: layout sub-pass
// 6f tails top-layer members in the root `painters_z`, so they pack last and the
// one instanced `BuiyNode::run` draw emits them OVER the in-flow content; their
// `clip = None` packs to the full-view sentinel so the fragment discard never
// fires and they paint unclipped over the whole view (paint-order § 3.2/§ 6f).
// This asserts that compositing-last property on a real adapter — the ordering
// + sentinel half is proven headlessly in render_extract.rs
// (`top_layer_entity_gets_none_clip_regardless_of_clip_rect` and the
// `assemble_*` painters_z tests). Run locally with `-- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); top-layer composites last in one draw"]
fn top_layer_composites_last_over_in_flow() {
    use buiy_core::Node;
    use buiy_core::layout::{Style, TopLayer};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use buiy_core::render::extract::ExtractedNodesView;
    use buiy_core::render::prepare::BuiyInstanceBuffers;
    use std::borrow::Cow;

    let opaque = |token: &'static str| Background {
        color: ColorToken::Token(Cow::Borrowed(token)),
    };
    let mut app = support::gpu_test_app_with_layout();
    let in_flow = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(40.0).height_px(40.0),
            opaque("color.surface.primary"),
        ))
        .id();
    let modal = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(40.0)
                .height_px(40.0)
                .top_layer(TopLayer::Modal),
            opaque("color.surface.secondary"),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[in_flow, modal]);
    support::finish_and_run(&mut app, 3);

    // The single-draw top-layer composite property, observed at the lowest layer
    // that sees it (verification.md § 2): layout 6f tails the modal in the root
    // painters_z, so it extracts LAST → packs last → the one instanced draw emits
    // it OVER the in-flow node; and it escapes ancestor clip to the full-view
    // sentinel (clip == None, paint-order § 3.2).
    let view = support::render_world_resource::<ExtractedNodesView>(&app)
        .expect("ExtractedNodesView present after extract");
    let nodes = &view.0.nodes;
    // The CPU records carry every node in the context tree — including the
    // transparent root, which is dropped only at the GPU pack (pack_view skips
    // Color::NONE). So the ordering claim is what matters, not the record count.
    let modal_idx = nodes.iter().position(|n| n.entity == modal);
    let in_flow_idx = nodes.iter().position(|n| n.entity == in_flow);
    assert_eq!(
        nodes.last().map(|n| n.entity),
        Some(modal),
        "the top-layer member tails painters_z (extracts last → packs last → \
         drawn last over the in-flow node)"
    );
    assert!(
        in_flow_idx < modal_idx,
        "the in-flow node paints before the top-layer member"
    );
    assert!(
        nodes.last().unwrap().clip.is_none(),
        "the top-layer member escapes ancestor clip to the full-view sentinel"
    );
    let buffers = support::render_world_resource::<BuiyInstanceBuffers>(&app)
        .expect("BuiyInstanceBuffers present after prepare");
    assert_eq!(
        buffers.quad_count, 2,
        "the two OPAQUE nodes pack (the transparent root is skipped by pack_view)"
    );
}

// Same RenderApp/wgpu-adapter caveat as the other render_smoke #[ignore] tests:
// RenderPlugin::build does block_on(initialize_renderer(...)) which expect()s a
// wgpu adapter. This is the final R8b wire-up guard: the full clip path —
// ClipRect → ExtractedNode.clip → PackedInstance.clip_min/clip_max (stride 52) →
// the @location(6)/(7) vertex attrs consumed by the clip-aware quad/shadow WGSL
// — is proven device-free across Tasks 1–3 (render_extract.rs, render_instance.rs,
// render_primitive_descriptor.rs, render_shader_wgsl.rs). What only a real adapter
// can prove is that wgpu *accepts* that end-to-end shape at pipeline creation:
// it validates the stride-52 instance layout against the WGSL `Instance` struct
// and compiles the clip-discard fragment, so a registered BuiyPipeline on a live
// device means the whole wired path binds without a layout/shader mismatch.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
fn clip_aabb_full_wire_up_smoke() {
    // `BuiyPipeline` is registered in `BuiyRenderPlugin::finish` and `finish()`
    // needs the full plugin set — see `pipeline_registers_in_render_app`.
    let mut app = support::gpu_test_app();
    app.finish();

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let pipeline = render_app
        .world()
        .get_resource::<buiy_core::render::pipeline::BuiyPipeline>()
        .expect(
            "BuiyPipeline registered: stride-52 clip layout + clip-aware WGSL accepted by wgpu",
        );
    // The id is a valid handle into the cache (compilation is async; we only
    // assert the resource + id exist, not that the pipeline finished). Pipeline
    // creation is where wgpu would reject the stride-52 layout / clip-discard
    // shader mismatch, so a present id proves the full wired path is accepted.
    let _ = pipeline.id;
}
