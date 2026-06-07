//! Atlas registration. The headless half asserts the plugin still builds
//! clean with the atlas wiring added (no RenderApp -> early return, mirrors
//! render_smoke.rs). The RenderApp-resource-presence + GPU draw are #[ignore]
//! (need a wgpu adapter; none on CI/this host).
use bevy::prelude::*;
use buiy_core::{CorePlugin, render::BuiyRenderPlugin};

#[test]
fn render_plugin_with_atlas_builds_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.update();
}

// Needs a wgpu adapter: RenderPlugin::build block_on(initialize_renderer)
// expect()s one; headless CI without a GPU/lavapipe panics before our code
// runs. Same caveat as render_smoke.rs. Run locally with:
//   cargo test -p buiy_core --test atlas_register -- --ignored
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by the gate-#15 e2e harness"]
fn atlas_resources_registered_in_render_app() {
    use buiy_core::render::atlas::{AtlasWarmupQueue, BuiyAtlas};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(BuiyRenderPlugin);

    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    assert!(
        render_app.world().get_resource::<BuiyAtlas>().is_some(),
        "BuiyAtlas registered in the render world"
    );
    assert!(
        render_app
            .world()
            .get_resource::<AtlasWarmupQueue>()
            .is_some(),
        "AtlasWarmupQueue registered in the render world"
    );
}
