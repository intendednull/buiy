//! Prepare-phase tests. The CONSTRUCTION-purity test is HEADLESS (no GPU); the
//! buffer-upload + render-world-membership tests are #[ignore] because they
//! need a wgpu adapter (RenderPlugin::build `.expect()`s one) — same idiom as
//! render_smoke.rs.

use bevy::prelude::*;
// ExtractedNodes/ExtractedNode are owned by R5 (render::extract); R6 CONSUMES them.
use buiy_core::render::extract::{ExtractedNode, ExtractedNodes};
use buiy_core::render::prepare::{BuiyInstanceBuffers, pack_extracted_nodes};
use buiy_core::render::view_uniform::BuiyViewUniform;

#[test]
fn instance_buffers_default_is_empty_no_device() {
    // The per-view persistent-buffer component R6 owns is plain data: it
    // constructs (Default) with no GPU device present.
    let buffers = BuiyInstanceBuffers::default();
    assert_eq!(buffers.quad_count, 0);
}

#[test]
fn view_uniform_from_extracted_nodes_params() {
    // R6 reads R5's ExtractedNodes (logical_size + scale_factor) and builds the
    // view uniform from them. R5's manual Default sets scale_factor = 1.0.
    let mut nodes = ExtractedNodes::default();
    assert_eq!(nodes.scale_factor, 1.0); // R5's manual Default (not 0.0)
    nodes.logical_size = Vec2::new(800.0, 600.0);
    nodes.scale_factor = 2.0;
    let u = BuiyViewUniform::for_view(nodes.logical_size, nodes.scale_factor);
    let p = u.apply(Vec2::ZERO);
    assert!((p.x - -1.0).abs() < 1e-6 && (p.y - 1.0).abs() < 1e-6);
    assert!((u.scale_factor() - 2.0).abs() < 1e-6);
}

#[test]
fn pack_extracted_nodes_populated_carrier_yields_nonempty_quad_batch() {
    // The carrier→batch wiring (the part that was a silent no-op when prepare
    // queried ExtractedNodes as an absent component): a POPULATED ExtractedNodes
    // must produce one raw quad instance per node, plus the std140 uniform from
    // its logical_size/scale_factor. This is the CPU half of prepare; the GPU
    // `write_buffer` upload is the only part that still needs a device.
    let nodes = ExtractedNodes {
        logical_size: Vec2::new(800.0, 600.0),
        scale_factor: 2.0,
        nodes: vec![
            ExtractedNode {
                entity: Entity::from_raw_u32(1).unwrap(),
                position: Vec2::new(10.0, 20.0),
                size: Vec2::new(100.0, 50.0),
                color: Color::srgb(1.0, 0.0, 0.0),
                clip: None,
                group: None,
            },
            ExtractedNode {
                entity: Entity::from_raw_u32(2).unwrap(),
                position: Vec2::new(30.0, 40.0),
                size: Vec2::new(60.0, 70.0),
                color: Color::srgb(0.0, 1.0, 0.0),
                clip: None,
                group: None,
            },
        ],
    };

    let (instances, uniform) = pack_extracted_nodes(&nodes);

    // One quad instance per painted node — NOT zero (the no-op regression).
    assert_eq!(
        instances.len(),
        2,
        "populated carrier must yield one quad per node"
    );
    // The uniform is built from the carrier's logical_size + scale_factor: the
    // std140 array carries scale_factor at slot 8.
    assert!(
        (uniform[8] - 2.0).abs() < 1e-6,
        "scale_factor threaded into the uniform, got {}",
        uniform[8]
    );

    // An EMPTY carrier yields an empty batch (no stale instances).
    let (empty, _) = pack_extracted_nodes(&ExtractedNodes::default());
    assert!(empty.is_empty(), "empty carrier yields no quads");
}

#[test]
fn extracted_nodes_pack_view_routes_records_to_quad_layer_0() {
    // R6 consumes R5's ExtractedNodes and packs its `nodes` via pack_view — no
    // DrawData adapter in between (the consumption shape after the seam flip).
    use buiy_core::render::buckets::{BuiyPrimitiveKind, PrimitiveBatchKey, pack_view};
    use buiy_core::render::extract::ExtractedNodes;
    let mut view = ExtractedNodes::default();
    // R5's manual Default sets scale_factor = 1.0 (not the derive's 0.0).
    assert_eq!(view.scale_factor, 1.0);
    view.logical_size = Vec2::new(1280.0, 720.0);
    // Push R5's per-painted-entity record directly — pack_view now consumes
    // &[ExtractedNode], so no parallel DrawData carrier is built.
    view.nodes.push(ExtractedNode {
        entity: Entity::from_raw_u32(1).unwrap(),
        position: Vec2::new(10.0, 20.0),
        size: Vec2::new(100.0, 50.0),
        color: Color::srgb(1.0, 0.0, 0.0),
        clip: None,
        group: None,
    });
    let buckets = pack_view(&view.nodes);
    let quad0 = PrimitiveBatchKey {
        primitive: BuiyPrimitiveKind::Quad,
        layer: 0,
    };
    assert_eq!(buckets.len(quad0), view.nodes.len());
}

#[test]
fn extracted_nodes_empty_packs_to_empty_buckets() {
    use buiy_core::render::buckets::pack_view;
    use buiy_core::render::extract::ExtractedNodes;
    let view = ExtractedNodes::default();
    assert!(view.nodes.is_empty());
    assert_eq!(view.scale_factor, 1.0); // R5's manual Default
    let buckets = pack_view(&view.nodes);
    assert!(buckets.is_empty());
}

#[test]
fn view_uniform_carrier_is_a_valid_std140_uniform() {
    // The view-uniform carrier must be a valid std140 UBO payload. encase runs
    // `T::assert_uniform_compat()` inside `UniformBuffer::write_buffer` on the
    // first GPU frame (bevy's `write_buffer` -> `scratch.write(&value).unwrap()`
    // -> encase `assert_uniform_compat`), so an invalid carrier panics on a real
    // adapter. The classic trap is a bare `[f32; 12]`: a scalar array has a
    // 4-byte stride, which violates std140's 16-byte array-stride rule, so encase
    // panics "array stride must be a multiple of 16 (current stride: 4)". This
    // test runs the SAME compat assert headlessly (no device) so that class of
    // panic is caught by the gate, not only on a GPU runner.
    //
    // `[Vec4; 3]` (the carrier `prepare` uses) has a 16-byte stride and packs to
    // a tight 48 B — the WGSL `BuiyView` (3 × vec4). The call below would panic
    // at runtime if the carrier type were ever reverted to a scalar array.
    use bevy::render::render_resource::ShaderType;
    <[Vec4; 3] as ShaderType>::assert_uniform_compat();
    // 3 × vec4 = 48 B, matching VIEW_UNIFORM_SIZE_BYTES and the bind-group
    // layout's min binding size.
    assert_eq!(<[Vec4; 3] as ShaderType>::min_size().get(), 48);
}

// ----- GPU (#[ignore]) — needs a wgpu adapter -----
// Run locally with: `cargo test -p buiy_core --test render_prepare -- --ignored`.

// Number of systems `BuiyRenderPlugin` adds to the `Render` schedule:
// `prepare_buiy_instances` (render/mod.rs), `prepare_buiy_view_pipelines`
// (render/pipeline.rs — the per-view format+Msaa pipeline specialization,
// queued in render/mod.rs), `prepare_effect_groups` (render/compositor.rs
// `register`), and `prepare_atlas_textures` (atlas/mod.rs `register` — the
// dirty-page GPU upload + `@group(1)` bind-group build), all four
// `.in_set(RenderSystems::Prepare)` and queued in `build`. Bump this in
// lockstep (with tests/render_compositor_gpu.rs) whenever the plugin's
// `add_systems(Render, …)` registrations change.
const BUIY_RENDER_SYSTEM_COUNT: usize = 4;

// Count the systems in a RenderApp's `Render` schedule. Reads the schedule graph
// directly (`graph().systems`), which is populated at `add_systems` time — no
// executor initialization (hence no schedule run, hence no extra device work) is
// required, so this is a pure introspection of which systems were registered.
fn render_schedule_system_count(app: &bevy::app::App) -> usize {
    use bevy::render::{Render, RenderApp};
    let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
    render_app
        .world()
        .resource::<bevy::ecs::schedule::Schedules>()
        .get(Render)
        .expect("Render schedule present in the RenderApp")
        .graph()
        .systems
        .len()
}

// Membership is asserted by a baseline delta rather than by system *name*, the
// same idiom as `extract_buiy_nodes_registered_in_extract_schedule` in
// render_smoke.rs: `System::name()` resolves to a placeholder ("<Enable the
// debug feature …>") unless `bevy_utils/debug` is enabled, which this workspace
// does not enable, so a `name().contains("prepare_buiy_instances")` match never
// fires. Instead we count the `Render`-schedule systems in a RenderApp built
// WITHOUT `BuiyRenderPlugin` and assert that adding the plugin grows the schedule
// by exactly the Buiy render-system count. Deleting the
// `add_systems(Render, prepare_buiy_instances…)` line in render/mod.rs (the
// regression this test name promises to catch) drops the delta below
// `BUIY_RENDER_SYSTEM_COUNT` and fails the assertion. Only *building* the
// RenderApp needs the wgpu adapter; walking the schedule graph does not.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); prepare-system render-world membership"]
fn prepare_system_is_in_render_prepare_set() {
    use bevy::render::RenderSystems;

    // Baseline: RenderApp with Bevy's own render systems but none of ours.
    let mut baseline = App::new();
    baseline.add_plugins(MinimalPlugins);
    baseline.add_plugins(bevy::asset::AssetPlugin::default());
    baseline.add_plugins(bevy::render::RenderPlugin::default());
    let baseline_count = render_schedule_system_count(&baseline);

    // With BuiyRenderPlugin added, the Render schedule must gain exactly the Buiy
    // prepare systems (incl. prepare_buiy_instances).
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
         delta of {} — a missing add_systems(Render, …) in render/mod.rs",
        with_plugin_count - baseline_count,
    );
    // The set-membership (RenderSystems::Prepare) is asserted by the ordering
    // test below; this test pins presence in the render world.
    let _ = RenderSystems::Prepare;
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); buffer upload round-trip"]
fn prepare_uploads_persistent_buffers() {
    use buiy_core::Node;
    use buiy_core::layout::Style;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use std::borrow::Cow;

    // The real spine round-trip on a live adapter: spawn one opaque painted node,
    // drive the full layout → stacking → transform-bridge → extract → prepare
    // path, and read the uploaded persistent buffer back from the render world.
    // `color.surface.primary` resolves to WHITE in the default light theme; a
    // transparent/None fill would be skipped by `pack_view` and pack zero quads.
    let mut app = crate::support::gpu_test_app_with_layout();
    app.world_mut().spawn((
        Node,
        Style::default().width_px(50.0).height_px(50.0),
        Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
        },
    ));

    // Drive several frames. The persistent buffer must hold the opaque node's
    // instance on a steady-state frame, not only the frame the node mutated:
    // extract retains the prior set on no-change frames and prepare retains the
    // uploaded buffer (architecture.md § 3.1 — "the persistent buffers from the
    // prior frame are re-bound and re-drawn").
    crate::support::finish_and_run(&mut app, 3);

    let buffers = crate::support::render_world_resource::<BuiyInstanceBuffers>(&app)
        .expect("BuiyInstanceBuffers present in the render world after prepare");
    assert_eq!(
        buffers.quad_count, 1,
        "one opaque node packs exactly one quad instance and survives steady-state \
         frames (the no-op regression this guards is quad_count == 0)"
    );
    assert!(
        buffers.quad.buffer().is_some(),
        "the quad VBO was uploaded via write_buffer"
    );
}

// ----- Damage-retention regressions (2026-06-07-render-extract-retain-damage-design.md) -----
// These pin each branch of the extract damage gate + prepare's retain. They are
// the GPU coverage for the R5 changed-only-replace bug (a static UI extracted once
// then vanished); the bug fails them with quad_count flickering to 0.

/// One opaque static node must keep `quad_count == 1` on EVERY steady-state frame,
/// not just the frame it spawned: extract retains the prior set on no-change frames
/// and prepare skips the re-upload (architecture.md § 3.1), so the persistent buffer
/// is never cleared. The pre-fix bug flickered to 0 from frame 1.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); damage-retention regression"]
fn static_node_survives_steady_state_frames() {
    use buiy_core::Node;
    use buiy_core::layout::Style;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use std::borrow::Cow;

    let mut app = crate::support::gpu_test_app_with_layout();
    app.world_mut().spawn((
        Node,
        Style::default().width_px(30.0).height_px(30.0),
        Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
        },
    ));
    app.finish();
    app.cleanup();
    for frame in 0..5 {
        app.update();
        let qc = crate::support::render_world_resource::<BuiyInstanceBuffers>(&app)
            .map(|b| b.quad_count)
            .unwrap_or(0);
        assert_eq!(
            qc, 1,
            "the static node must remain packed on frame {frame} (no flicker to 0)"
        );
    }
}

/// When ONE of several nodes changes, the FULL set is re-extracted — an unchanged
/// sibling must not be dropped. This is the core R5 bug: the changed-only replace
/// emitted just the mutated node, dropping the others. The fix re-extracts the
/// whole set on any change (the un-gated `nodes` query).
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); damage-retention regression"]
fn one_node_change_keeps_unchanged_siblings() {
    use buiy_core::Node;
    use buiy_core::layout::Style;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use std::borrow::Cow;

    let opaque = |t: &'static str| Background {
        color: ColorToken::Token(Cow::Borrowed(t)),
    };
    let mut app = crate::support::gpu_test_app_with_layout();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            opaque("color.surface.primary"),
        ))
        .id();
    for _ in 0..2 {
        app.world_mut().spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            opaque("color.surface.primary"),
        ));
    }
    crate::support::finish_and_run(&mut app, 2);
    assert_eq!(
        crate::support::render_world_resource::<BuiyInstanceBuffers>(&app)
            .map(|b| b.quad_count)
            .unwrap_or(0),
        3,
        "all three nodes pack before the mutation"
    );

    // Touch ONE node's Background (→ Changed<Background> → full re-extract).
    app.world_mut()
        .get_mut::<Background>(a)
        .expect("node a has Background")
        .set_changed();
    app.update();
    app.update();
    assert_eq!(
        crate::support::render_world_resource::<BuiyInstanceBuffers>(&app)
            .map(|b| b.quad_count)
            .unwrap_or(0),
        3,
        "mutating one node must NOT drop its two unchanged siblings (the R5 bug)"
    );
}

/// Despawning the painted node must drop it from the buffer — the despawn damage
/// path (`RemovedComponents<ResolvedLayout>`), the one signal the `Changed` probe
/// cannot see.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); damage-retention regression"]
fn despawn_drops_the_instance() {
    use buiy_core::Node;
    use buiy_core::layout::Style;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use std::borrow::Cow;

    let mut app = crate::support::gpu_test_app_with_layout();
    let node = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(25.0).height_px(25.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
            },
        ))
        .id();
    crate::support::finish_and_run(&mut app, 2);
    assert_eq!(
        crate::support::render_world_resource::<BuiyInstanceBuffers>(&app)
            .map(|b| b.quad_count)
            .unwrap_or(0),
        1,
        "the node packs before despawn"
    );

    app.world_mut().despawn(node);
    app.update();
    app.update();
    assert_eq!(
        crate::support::render_world_resource::<BuiyInstanceBuffers>(&app)
            .map(|b| b.quad_count)
            .unwrap_or(99),
        0,
        "despawn must re-extract (RemovedComponents) and clear the buffer"
    );
}

/// A theme swap re-resolves every token-bearing fill globally and must bypass the
/// per-entity `Changed` gate (color-and-forced-colors.md § 3): the extracted color
/// must update even though no paint component on the node changed. Without
/// `theme.is_changed()` in the damage gate the new retain would leave stale colors.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); damage-retention regression"]
fn theme_swap_reresolves_extracted_color() {
    use bevy::prelude::Color;
    use buiy_core::Node;
    use buiy_core::layout::Style;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use buiy_core::render::extract::ExtractedNodesView;
    use buiy_core::theme::Theme;
    use std::borrow::Cow;

    let mut app = crate::support::gpu_test_app_with_layout();
    app.world_mut().spawn((
        Node,
        Style::default().width_px(20.0).height_px(20.0),
        Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
        },
    ));
    crate::support::finish_and_run(&mut app, 2);
    let color_before = crate::support::render_world_resource::<ExtractedNodesView>(&app)
        .and_then(|v| {
            v.0.nodes
                .iter()
                .find(|n| n.color != Color::NONE)
                .map(|n| n.color)
        })
        .expect("an opaque node was extracted");

    // Re-point the token to a new color in the live theme (a theme swap edge).
    app.world_mut()
        .resource_mut::<Theme>()
        .colors
        .insert("color.surface.primary".into(), Color::srgb(0.1, 0.2, 0.3));
    app.update();
    app.update();
    let color_after = crate::support::render_world_resource::<ExtractedNodesView>(&app)
        .and_then(|v| {
            v.0.nodes
                .iter()
                .find(|n| n.color != Color::NONE)
                .map(|n| n.color)
        })
        .expect("the node is still extracted after the theme swap");

    assert_ne!(
        color_before, color_after,
        "a theme swap must re-resolve the token (theme.is_changed() bypasses the \
         per-entity Changed gate)"
    );
}
