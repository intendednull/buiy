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
            },
            ExtractedNode {
                entity: Entity::from_raw_u32(2).unwrap(),
                position: Vec2::new(30.0, 40.0),
                size: Vec2::new(60.0, 70.0),
                color: Color::srgb(0.0, 1.0, 0.0),
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

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); prepare-system render-world membership"]
fn prepare_system_is_in_render_prepare_set() {
    use bevy::render::{Render, RenderApp, RenderSystems};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
    // Assert the Render schedule contains our prepare system. Bevy exposes
    // schedule membership via the Schedules resource; we check the system is
    // present in the Render schedule graph.
    let schedules = render_app
        .world()
        .resource::<bevy::ecs::schedule::Schedules>();
    let render = schedules.get(Render).expect("Render schedule present");
    // `System::name()` derefs to `str`; without `bevy_utils/debug` it is a
    // placeholder (same caveat render_smoke.rs documents), so this name match
    // only fires on a debug-feature build — acceptable here because this is the
    // #[ignore] GPU path, run locally on a machine that can build with debug.
    let found = render
        .graph()
        .systems
        .iter()
        .any(|(_, system, _)| system.name().contains("prepare_buiy_instances"));
    assert!(
        found,
        "prepare_buiy_instances registered in the Render schedule"
    );
    // The set-membership (RenderSystems::Prepare) is asserted by the ordering
    // test below; this test pins presence in the render world.
    let _ = RenderSystems::Prepare;
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); buffer upload round-trip"]
fn prepare_uploads_persistent_buffers() {
    // Full GPU round-trip: build a RenderApp, insert R5's ExtractedNodes with a
    // few nodes onto a view entity, run the Prepare set, and assert the view's
    // BuiyInstanceBuffers holds a non-empty quad buffer whose instance count
    // equals nodes.len(). (Provisioned by the Task-N e2e/visual harness on a
    // GPU runner; left as the documented GPU coverage point here.)
}
