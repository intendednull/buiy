use bevy::prelude::*;
use buiy::BuiyPlugin;

#[test]
fn buiy_plugin_loads_in_correct_order() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // `BuiyPlugin` composes systems that read keyboard / pointer input
    // (focus tab handling, button click). `MinimalPlugins` does not include
    // `InputPlugin`, so we add it here so `app.update()` doesn't panic on
    // missing `ButtonInput<KeyCode>` / `ButtonInput<MouseButton>` resources.
    app.add_plugins(bevy::input::InputPlugin);
    app.add_plugins(BuiyPlugin);
    app.update();

    // Sanity: re-exports are accessible.
    let _ = std::any::TypeId::of::<buiy::Button>();
    let _ = std::any::TypeId::of::<buiy::Focusable>();
    let _ = std::any::TypeId::of::<buiy::A11yRole>();
}

/// `DefaultPlugins` (Bevy 0.18) already includes `bevy::picking::PickingPlugin`,
/// so a real app does `DefaultPlugins` + `BuiyPlugin` — and `BuiyPlugin` must NOT
/// add the Bevy picking plugin a second time ("plugin was already added" panic at
/// startup, hit by `cargo run -p hello_button`). Simulate the DefaultPlugins half
/// by pre-adding the plugin, headlessly.
#[test]
fn buiy_plugin_composes_with_preexisting_bevy_picking() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::input::InputPlugin);
    // What DefaultPlugins contributes (the duplicate-add source).
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(BuiyPlugin);
    app.update();
}

/// The FACADE composition on a real adapter: `BuiyRenderPlugin`'s `finish()`
/// (which registers the device-dependent `BuiyPipeline` + `AtlasGpu`) must
/// actually run when the render plugin is pulled in via `BuiyPlugin`.
///
/// Regression: `BuiyPlugin::finish` used to `add_plugins(BuiyRenderPlugin)` —
/// but Bevy's `App::finish` iterates `0..plugin_registry.len()` with the length
/// captured BEFORE the loop, so a plugin added DURING another plugin's `finish`
/// never gets its own `finish()` called. `pipeline::register`/`register_gpu`
/// therefore never ran in any real app: `prepare_atlas_textures`' `Res<AtlasGpu>`
/// panicked "Resource does not exist" on the first frame
/// (`cargo run -p hello_button`). Headless tests never saw it — no RenderApp.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn facade_render_finish_registers_device_resources() {
    use bevy::asset::AssetApp;

    let mut app = App::new();
    // The order every real app uses: the render stack (DefaultPlugins' relevant
    // subset) BEFORE BuiyPlugin.
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::window::WindowPlugin::default())
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::render::RenderPlugin::default())
        .add_plugins(bevy::image::ImagePlugin::default())
        .add_plugins(bevy::camera::CameraPlugin)
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(BuiyPlugin);
    app.init_asset::<Mesh>();
    app.finish();
    app.cleanup();
    // The first frame is where the missing-resource param validation fired.
    app.update();

    let render_app = app
        .get_sub_app(bevy::render::RenderApp)
        .expect("RenderApp exists");
    assert!(
        render_app
            .world()
            .get_resource::<buiy_core::render::pipeline::BuiyPipeline>()
            .is_some(),
        "BuiyRenderPlugin::finish ran (BuiyPipeline registered) under the facade"
    );
    assert!(
        render_app
            .world()
            .get_resource::<buiy_core::render::atlas::AtlasGpu>()
            .is_some(),
        "atlas::register_gpu ran (AtlasGpu present) under the facade"
    );
}
