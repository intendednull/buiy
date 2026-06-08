//! Shared test support for Buiy render **GPU** integration tests.
//!
//! [`gpu_test_app`] builds the minimal *complete* plugin set that drives a full
//! Buiy render frame headless on a real wgpu adapter (this host: AMD Radeon RX
//! 6700 XT, RADV/Vulkan — no X server / xvfb needed for render-to-texture).
//!
//! ## Why each plugin is here (the "Message not initialized" cascade)
//!
//! `RenderPlugin::build` pulls in `bevy_render::camera::camera_system`, which
//! reads `MessageReader<WindowResized>`; the minimal probe set never added the
//! sole owner of `add_message::<WindowResized>()`, so the first `app.update()`
//! panicked with *"Parameter `…messages` failed validation: Message not
//! initialized"* (Bevy 0.18 renamed Events→Messages). Resolving that surfaced a
//! short cascade of missing owners — each fixed by adding the **correct owning
//! plugin / init**, not a bare `add_message`:
//!
//! | Missing resource / message            | Owner added                         |
//! |---------------------------------------|-------------------------------------|
//! | `Messages<WindowResized>`             | [`bevy::window::WindowPlugin`]      |
//! | `Assets<Mesh>` + `Messages<AssetEvent<Mesh>>` | `app.init_asset::<Mesh>()` (what the private `MeshPlugin` does internally; `RenderPlugin` extracts meshes but deliberately does **not** add the asset) |
//! | `Res<ClearColor>` (+ visibility/projection) | [`bevy::camera::CameraPlugin`] (the logical-world camera plugin, distinct from `RenderPlugin`'s render-world one) |
//! | `Res<Theme>` / `Res<UserPreferences>` | [`buiy_core::theme::ThemePlugin`] (Buiy's own — intentionally separate from `CorePlugin`; `extract_buiy_nodes` reads `Res<Theme>`) |
//!
//! None of these were Buiy bugs — the panic reproduces with *zero* Buiy plugins.
//! Verified by the panel that established this harness (campaign plan
//! `docs/plans/2026-06-07-render-gpu-verify-campaign.md`).
#![allow(dead_code)]

use bevy::asset::AssetApp;
use bevy::prelude::*;
use buiy_core::{CorePlugin, render::BuiyRenderPlugin};

/// Build the canonical headless-GPU Buiy app. The returned [`App`] is **not yet
/// finished** — the caller must `finish()` it (or use [`finish_and_run`]) before
/// reading any render-world resource: `RenderPlugin` inserts the `RenderDevice` /
/// `PipelineCache` and `BuiyRenderPlugin::finish` registers `BuiyPipeline`
/// **during `finish`**, never `build`.
pub fn gpu_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // Owns `Messages<WindowResized>`, read by `RenderPlugin`'s camera_system.
        .add_plugins(bevy::window::WindowPlugin::default())
        // `Assets<Shader>` lives here (main world); shaders load into it.
        .add_plugins(bevy::asset::AssetPlugin::default())
        // Creates the RenderApp + the wgpu device/adapter (block_on initialize).
        .add_plugins(bevy::render::RenderPlugin::default())
        // Hard transitive requirement of `RenderPlugin`'s GpuImage path.
        .add_plugins(bevy::image::ImagePlugin::default())
        // Logical-world camera: owns `Res<ClearColor>` + visibility/projection.
        .add_plugins(bevy::camera::CameraPlugin)
        // Buiy's `Res<Theme>` / `Res<UserPreferences>`, read by extract.
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(BuiyRenderPlugin);
    // `Assets<Mesh>` + `Messages<AssetEvent<Mesh>>` — `RenderPlugin` extracts
    // meshes but does not add the asset (its doc: "Use MeshPlugin for that").
    app.init_asset::<Mesh>();
    app
}

/// `finish()` + `cleanup()` the app (materializing the render device, pipeline
/// cache, and `BuiyPipeline`), then drive `frames` render frames. Render plugins
/// insert their device-dependent resources only during `finish`, so this MUST
/// run before any render-world resource is read. The first frame may not paint;
/// pass `frames >= 2` when asserting on painted output.
pub fn finish_and_run(app: &mut App, frames: usize) {
    app.finish();
    app.cleanup();
    for _ in 0..frames {
        app.update();
    }
}

/// [`gpu_test_app`] + [`buiy_core::layout::LayoutPlugin`] — for tests that spawn
/// real `(Node, Style)` entities and need the full layout → stacking → transform
/// bridge → extract path. Sub-pass 6f writes the `StackingContext` that
/// `extract_buiy_nodes` walks; without it extract emits nothing, so a painted
/// node never reaches `BuiyInstanceBuffers`. Kept SEPARATE from `gpu_test_app`
/// so the resource/structural GPU tests on the base harness stay untouched.
pub fn gpu_test_app_with_layout() -> App {
    let mut app = gpu_test_app();
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app
}

/// Read a render-world resource back from the `RenderApp` after a frame — `None`
/// if the `RenderApp` or the resource is absent. DRYs the
/// `get_sub_app(RenderApp).world().get_resource::<R>()` idiom the spine / readback
/// tests share.
pub fn render_world_resource<R: Resource>(app: &App) -> Option<&R> {
    app.get_sub_app(bevy::render::RenderApp)?
        .world()
        .get_resource::<R>()
}
