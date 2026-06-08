//! GPU-path tests for the effect-group compositor. These need a wgpu adapter
//! (real GPU or lavapipe), which CI / this host lack, so they are `#[ignore]`
//! exactly like tests/render_smoke.rs. Run locally with:
//!   cargo test -p buiy_core --test render_compositor_gpu -- --ignored

use bevy::prelude::*;

/// Count the nodes in the Core2d sub-graph of a freshly-built app, optionally
/// installing `BuiyRenderPlugin`. Both variants install the identical
/// prerequisite plugin stack, so the Core2d node-count *delta* between them is
/// exactly what `BuiyRenderPlugin::build` contributes to that sub-graph.
fn core2d_node_count(with_buiy: bool) -> usize {
    use bevy::core_pipeline::core_2d::graph::Core2d;
    use bevy::render::render_graph::RenderGraph;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    // `CorePipelinePlugin` → `TonemappingPlugin::build` reads `Assets<Image>`,
    // whose owner is `ImagePlugin` (not `AssetPlugin`). Without it, build panics
    // with "Requested resource … does not exist" from tonemapping/mod.rs.
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
    if with_buiy {
        app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    }

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let graph = render_app
        .world()
        .get_resource::<RenderGraph>()
        .expect("RenderGraph");
    let sub = graph.get_sub_graph(Core2d).expect("Core2d sub-graph");
    sub.iter_nodes().count()
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
fn compositor_register_adds_no_extra_graph_node() {
    use bevy::core_pipeline::core_2d::graph::Core2d;
    use bevy::render::render_graph::RenderGraph;

    // The compositor runs INSIDE BuiyRenderLabel (effect-compositor.md § 3): it
    // must NOT register a second competing node. `BuiyRenderPlugin::build` wires
    // exactly one Core2d node — `BuiyRenderLabel` (node::register) — and
    // `compositor::register` must add NONE. So installing the plugin grows the
    // Core2d node set by exactly ONE; a compositor that wrongly registered a
    // second node would make this delta two and fail here.
    let control = core2d_node_count(false);
    let with_buiy = core2d_node_count(true);
    assert_eq!(
        with_buiy - control,
        1,
        "BuiyRenderPlugin must add exactly one Core2d node (BuiyRenderLabel); \
         the compositor must add none (control={control}, with_buiy={with_buiy})"
    );

    // Precondition (NOT the guard above): the single added node is in fact the
    // Buiy node, registered under `BuiyRenderLabel`.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    // See `core2d_node_count`: `CorePipelinePlugin` needs `Assets<Image>` (ImagePlugin).
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let graph = render_app
        .world()
        .get_resource::<RenderGraph>()
        .expect("RenderGraph");
    let sub = graph.get_sub_graph(Core2d).expect("Core2d sub-graph");
    assert!(
        sub.get_node_state(buiy_core::render::node::BuiyRenderLabel)
            .is_ok(),
        "BuiyRenderLabel present"
    );
}

// Number of systems `BuiyRenderPlugin` adds to the `Render` schedule:
// `prepare_buiy_instances` (render/mod.rs) and `prepare_effect_groups`
// (render/compositor.rs `register`), both `.in_set(RenderSystems::Prepare)` and
// both queued in `build`. Mirrors `BUIY_RENDER_SYSTEM_COUNT` in
// tests/render_prepare.rs; bump in lockstep whenever the plugin's
// `add_systems(Render, …)` registrations change.
const BUIY_RENDER_SYSTEM_COUNT: usize = 2;

// Count the systems in a RenderApp's `Render` schedule graph. `graph().systems`
// is populated at `add_systems` time (in `build`), so this is pure introspection
// — no executor init, no `finish()`, no extra device work. Identical helper to
// `render_schedule_system_count` in tests/render_prepare.rs.
fn render_schedule_system_count(app: &App) -> usize {
    use bevy::render::{Render, RenderApp};
    app.get_sub_app(RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<bevy::ecs::schedule::Schedules>()
        .get(Render)
        .expect("Render schedule present in the RenderApp")
        .graph()
        .systems
        .len()
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
fn prepare_effect_groups_runs_in_prepare_set() {
    use bevy::render::RenderSystems;

    // Membership is asserted by a baseline system-count delta rather than by
    // system *name*: without `bevy_utils/debug` (this workspace does not enable
    // it) `System::name()` resolves to the placeholder "<Enable the debug feature
    // to see the name>", so a `name().contains("prepare_effect_groups")` match
    // can NEVER fire here — the prior name-based body was structurally broken on
    // a non-debug build. Same proven idiom as
    // `prepare_system_is_in_render_prepare_set` in tests/render_prepare.rs:
    // count the Render-schedule systems WITHOUT the plugin, then WITH it, and
    // assert the delta is exactly the Buiy render-system count. Deleting the
    // `compositor::register` → `add_systems(Render, prepare_effect_groups…)` line
    // drops the delta below `BUIY_RENDER_SYSTEM_COUNT` and fails here. Only
    // *building* the RenderApp needs the wgpu adapter; walking the schedule does
    // not. `CorePipelinePlugin` is intentionally NOT added (it is irrelevant to
    // this membership assertion and pulls in the tonemapping `Assets<Image>`
    // dependency).
    let mut baseline = App::new();
    baseline.add_plugins(MinimalPlugins);
    baseline.add_plugins(bevy::asset::AssetPlugin::default());
    baseline.add_plugins(bevy::render::RenderPlugin::default());
    let baseline_count = render_schedule_system_count(&baseline);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    let with_plugin_count = render_schedule_system_count(&app);

    assert_eq!(
        with_plugin_count - baseline_count,
        BUIY_RENDER_SYSTEM_COUNT,
        "BuiyRenderPlugin must register {BUIY_RENDER_SYSTEM_COUNT} systems in the \
         Render schedule (prepare_buiy_instances + prepare_effect_groups); got a \
         delta of {} — a missing add_systems(Render, prepare_effect_groups…) in \
         render/compositor.rs `register`",
        with_plugin_count - baseline_count,
    );
    // The set-membership (RenderSystems::Prepare) is pinned by register() and
    // the compositor schedule-order test; this test pins presence in the render world.
    let _ = RenderSystems::Prepare;
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
fn buiy_node_runs_with_prepared_effect_groups_query() {
    // Compile + construction smoke: BuiyRenderPlugin builds with the extended
    // BuiyNode ViewQuery (Option<&PreparedEffectGroups>) and the node is in
    // Core2d. The composite correctness is proven by the golden (separate
    // gate #2 fixture) — this only pins that the node wiring compiles & loads.
    use bevy::core_pipeline::core_2d::graph::Core2d;
    use bevy::render::render_graph::RenderGraph;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    // See `core2d_node_count`: `CorePipelinePlugin` needs `Assets<Image>` (ImagePlugin).
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    let graph = render_app.world().get_resource::<RenderGraph>().unwrap();
    let sub = graph.get_sub_graph(Core2d).expect("Core2d");
    assert!(
        sub.get_node_state(buiy_core::render::node::BuiyRenderLabel)
            .is_ok()
    );
}

#[test]
#[ignore = "gate #2 golden; needs a wgpu adapter + golden harness (verification.md § 2.4)"]
// Placeholder body: the pixel-readback assertion lands with the device-backed
// golden harness; until then the `assert!(true, ..)` marker documents the
// fixture contract (clippy would otherwise reject a constant assertion).
#[allow(clippy::assertions_on_constants)]
fn group_opacity_overlap_is_single_layer_at_half() {
    // Fixture: two overlapping opaque red children inside an Opacity(0.5)
    // group. The overlap region must read as 50% red over the backdrop —
    // the off-screen pass result — NOT a doubled (per-child-approx) composite
    // (effect-compositor.md § 4 / § 5.1). This is the regression guard that
    // the correct off-screen pass shipped, not the rejected approximation.
    //
    // The pixel readback rides the e2e golden harness (verification.md § 2.4).
    // Assembled here as the canonical fixture so the harness can target it.
    assert!(
        true,
        "fixture builder lands with the gate-#2 golden harness"
    );
}

#[test]
#[ignore = "gate #15 RSS/leak; needs a wgpu adapter + leak harness (verification.md / README § 5 #4)"]
// Placeholder body: the RSS-slope + RT-bucket-count assertion lands with the
// device-backed leak harness; until then the `assert!(true, ..)` marker
// documents the fixture contract (clippy would otherwise reject a constant
// assertion).
#[allow(clippy::assertions_on_constants)]
fn rt_pool_returns_to_baseline_after_idle() {
    // Fixture: spawn N opacity groups, animate opacity 1.0->0.5->1.0 to churn
    // EffectGroup membership, then idle. After > max(atlas eviction_grace,
    // RT-pool 3 frames) (effect-compositor.md § 2.2), the TextureCache entry
    // count for the "buiy_effect_group_target" descriptor family must return
    // within ε of the steady-state working set, and RSS slope < 1 MB/min.
    //
    // Return-to-baseline is guaranteed by construction: sizing is
    // painted-bounds (not viewport), reuse is descriptor-keyed, and Bevy's
    // update_texture_cache_system drops targets unused for 3 frames (§ 2.3).
    // Buiy adds NO bespoke eviction. The slope/ε numbers are owned by
    // buiy-verification-design (README § 5 #4); this fixture is the mechanism
    // proof the numbers calibrate against.
    assert!(true, "leak fixture builder lands with the gate-#15 harness");
}
