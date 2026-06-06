//! GPU e2e (`#[ignore]`): real specialization through a live PipelineCache.
//! Needs a wgpu adapter (RenderPlugin::build block_on(initialize_renderer)
//! expect()s one) — headless CI has none, so this is ignored and runs only
//! under `-- --ignored` on a GPU/lavapipe host, alongside render_smoke.rs.
//! The device-free counterpart is tests/render_primitive_dedup.rs.

use bevy::prelude::*;
use bevy::render::{
    RenderApp,
    render_resource::{PipelineCache, SpecializedRenderPipelines, TextureFormat},
};
use buiy_core::render::buckets::BuiyPrimitiveKind;
use buiy_core::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
fn specialize_allocates_distinct_ids_per_format() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

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
