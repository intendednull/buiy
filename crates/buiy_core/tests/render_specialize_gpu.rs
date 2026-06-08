//! GPU e2e (`#[ignore]`): real specialization through a live PipelineCache.
//! Needs a wgpu adapter (RenderPlugin::build block_on(initialize_renderer)
//! expect()s one) — headless CI has none, so this is ignored and runs only
//! under `-- --ignored` on a GPU/lavapipe host, alongside render_smoke.rs.
//! The device-free counterpart is tests/render_primitive_dedup.rs.

use bevy::render::{
    RenderApp,
    render_resource::{PipelineCache, SpecializedRenderPipelines, TextureFormat},
};
use buiy_core::render::buckets::BuiyPrimitiveKind;
use buiy_core::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};

mod support;

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
fn specialize_allocates_distinct_ids_per_format() {
    // `PipelineCache` is inserted into the RenderApp by `RenderPlugin::finish`
    // (it needs the `RenderDevice`), never `build` — so `finish()` MUST run
    // before the cache can be read. `gpu_test_app` is the canonical complete
    // plugin set; `finish()` materializes the device + cache. No frame is
    // driven: this drives the specialization cache directly.
    let mut app = support::gpu_test_app();
    app.finish();

    let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
    let world = render_app.world_mut();
    let cache = world.resource::<PipelineCache>();
    // Drive the real specialization cache directly.
    let mut specialized = SpecializedRenderPipelines::<BuiyPrimitives>::default();
    let specializer = BuiyPrimitives;

    let id_srgb = specialized.specialize(
        cache,
        &specializer,
        BuiyPrimitiveKey {
            kind: BuiyPrimitiveKind::Quad,
            format: TextureFormat::Rgba8UnormSrgb,
        },
    );
    let id_hdr = specialized.specialize(
        cache,
        &specializer,
        BuiyPrimitiveKey {
            kind: BuiyPrimitiveKind::Quad,
            format: TextureFormat::Rgba16Float,
        },
    );
    // Repeat the srgb key → same id (dedup).
    let id_srgb2 = specialized.specialize(
        cache,
        &specializer,
        BuiyPrimitiveKey {
            kind: BuiyPrimitiveKind::Quad,
            format: TextureFormat::Rgba8UnormSrgb,
        },
    );
    assert_ne!(id_srgb, id_hdr, "distinct format → distinct cached id");
    assert_eq!(id_srgb, id_srgb2, "repeated key → deduped id");
}
