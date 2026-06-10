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

pub mod extract_harness;

use bevy::asset::{AssetApp, RenderAssetUsages};
use bevy::camera::RenderTarget;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use buiy_core::{CorePlugin, render::BuiyRenderPlugin};
use std::sync::{Arc, Mutex};

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
        // The text engine + the T4 glyph producer (render half registers
        // against the live RenderApp created by RenderPlugin above).
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
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

// ---------------------------------------------------------------------------
// Render-to-texture + readback capture infra (gate-#2 keystone).
//
// `gpu_test_app` proves the harness drives a frame and packs the instance
// buffers, but it deliberately omits `CorePipelinePlugin`, so the `Core2d`
// sub-graph never exists — `node::register`'s `add_render_graph_node(Core2d, …)`
// only *warns* when the sub-graph is missing (bevy_render render_graph/app.rs),
// so the Buiy node is never wired into a graph and never executes. NO pixels are
// painted. This builder is the painting-capable sibling: it adds
// `CorePipelinePlugin` (→ `Core2dPlugin`, which `add_render_sub_graph(Core2d)`s
// the 2D graph) BEFORE `BuiyRenderPlugin`, so the Buiy node lands inside a live
// `Core2d` graph and its `StartMainPassPostProcessing → BuiyRenderLabel →
// Tonemapping` edges resolve. A `CameraDriverNode` then runs that graph for the
// offscreen view, painting into the render-target image read back below.
//
// The primary window resolution is set to the capture size so the per-view
// `BuiyViewUniform` (built in prepare from `ExtractedNodes.logical_size`, which
// `extract_buiy_nodes` fills from the primary window — architecture § 4, D2:
// every Node resolves to the primary view) matches the offscreen target's pixel
// grid. Without that match the logical→clip transform would scale geometry to
// the window, not the image.

/// Painting-capable headless-GPU app: `gpu_test_app`'s stack PLUS
/// `CorePipelinePlugin` (the `Core2d` graph) and `LayoutPlugin`, with the
/// primary window sized to `width`×`height` so the view uniform matches the
/// capture image. Like [`gpu_test_app`] the returned [`App`] is NOT finished —
/// use [`finish_and_run`]. Spawn `(Node, Style, Background)` entities + a
/// capture camera ([`spawn_capture_camera`]) before driving frames.
pub fn gpu_render_app(width: u32, height: u32) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // Sized to the capture target so the primary-window-derived view uniform
        // matches the offscreen image's pixel grid (see module note above).
        .add_plugins(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                resolution: bevy::window::WindowResolution::new(width, height),
                ..default()
            }),
            ..default()
        })
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::render::RenderPlugin::default())
        .add_plugins(bevy::image::ImagePlugin::default())
        .add_plugins(bevy::camera::CameraPlugin)
        // The 2D render graph: `Core2dPlugin` (inside `CorePipelinePlugin`)
        // creates the `Core2d` sub-graph that `BuiyRenderPlugin` wires its node
        // into. MUST precede `BuiyRenderPlugin` (plugins build in add order).
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(CorePlugin)
        // The text engine + the T4 glyph producer (render half registers
        // against the live RenderApp created by RenderPlugin above).
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(BuiyRenderPlugin);
    app.init_asset::<Mesh>();
    app
}

/// Create an offscreen `Rgba8UnormSrgb` render-target image of `width`×`height`,
/// add the `COPY_SRC` usage the readback copy needs (the constructor sets
/// `RENDER_ATTACHMENT | COPY_DST | TEXTURE_BINDING` but not `COPY_SRC`), force
/// `RenderAssetUsages::all()` so the `GpuImage` exists in the render world, and
/// insert it into `Assets<Image>`. Returns the handle.
pub fn render_to_image(app: &mut App, width: u32, height: u32) -> Handle<Image> {
    // `Rgba8UnormSrgb` == `ViewTarget::main_texture_format()` for a non-HDR
    // Camera2d (== `BuiyPipeline`'s target format), so the Buiy pipeline binds
    // to this view without a format mismatch.
    let mut image = Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
    // The GpuReadback copy is a texture→buffer COPY_SRC; the constructor omits it.
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    // Without RENDER_WORLD the GpuImage is never created, so the readback (which
    // looks the handle up in `RenderAssets<GpuImage>`) finds nothing. `all()`
    // keeps MAIN_WORLD too — harmless here, and the documented render-target idiom.
    image.asset_usage = RenderAssetUsages::all();
    app.world_mut().resource_mut::<Assets<Image>>().add(image)
}

/// Spawn a `Camera2d` whose render target is `target`, so `BuiyNode::run` paints
/// into the offscreen image's `ViewTarget`. The clear color is forced opaque
/// black (`ClearColorConfig::Custom`) — a deterministic backdrop the SrcOver
/// composite is asserted against (the global default `ClearColor` is an opaque
/// dark gray, not a clean zero).
///
/// `Msaa::Off` — single-sampled for pixel determinism (most readback tests
/// assert exact pixel values, and a 4x resolve antialiases edges). The Buiy
/// pipelines specialize per-view on the sample count (`prepare_buiy_view_pipelines`),
/// so both `Off` and `Sample4` views work; the multisampled path is covered by
/// `tests/render_msaa.rs` via [`spawn_capture_camera_with_msaa`].
pub fn spawn_capture_camera(app: &mut App, target: Handle<Image>) {
    spawn_capture_camera_with_msaa(app, target, bevy::render::view::Msaa::Off);
}

/// [`spawn_capture_camera`] with an explicit per-view [`Msaa`](bevy::render::view::Msaa)
/// mode — the MSAA regression tests (`tests/render_msaa.rs`) spawn the capture
/// camera at `Msaa::Sample4` (the bare-`Camera2d` default a real app gets) to
/// prove the per-view sample-count pipeline specialization.
pub fn spawn_capture_camera_with_msaa(
    app: &mut App,
    target: Handle<Image>,
    msaa: bevy::render::view::Msaa,
) {
    app.world_mut().spawn((
        Camera2d,
        // `RenderTarget` is a standalone component in Bevy 0.18 (no longer a
        // `Camera` field); spawning it overrides the default primary-window target.
        RenderTarget::from(target),
        msaa,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
    ));
}

/// Resource cell the `ReadbackComplete` observer writes the captured bytes into.
/// `Arc<Mutex<…>>` so the observer (which `move`s its capture) and the test poll
/// loop share one slot; an ECS resource would also work but the shared cell keeps
/// the observer a small closure.
#[derive(Resource, Clone, Default)]
struct CapturedBytes(Arc<Mutex<Option<Vec<u8>>>>);

/// Drive frames until the text fixture's `wait_for_fonts` predicate holds
/// (verification § 3.2): the producer has emitted (`ResidentTextKeys`
/// non-empty), the warmup queue is drained, and every emitted key is
/// resident. Returns frames driven; panics past `max_frames`.
pub fn wait_for_text_ready(app: &mut App, max_frames: usize) -> usize {
    use buiy_core::render::atlas::{AtlasWarmupQueue, BuiyAtlas};
    use buiy_core::render::golden::fonts_ready;
    use buiy_core::text::ResidentTextKeys;

    for frame in 0..max_frames {
        app.update();
        let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
        let world = render_app.world();
        let resident = world.resource::<ResidentTextKeys>();
        if !resident.keys.is_empty()
            && fonts_ready(
                world.resource::<BuiyAtlas>(),
                world.resource::<AtlasWarmupQueue>(),
                &resident.keys,
            )
        {
            return frame + 1;
        }
    }
    panic!("text never became atlas-resident within {max_frames} frames");
}

/// Index one RGBA8 pixel out of an un-padded `w*h*4` readback buffer.
pub fn px(pixels: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// The sRGB8 the target stores for a FULL-coverage texel of linear
/// straight-alpha `color` over the opaque-black clear: SrcOver in linear
/// (dst = 0), then the Rgba8UnormSrgb linear→sRGB encode.
pub fn expected_full_coverage_srgb(color: [f32; 4]) -> [u8; 4] {
    let a = color[3];
    let lin = LinearRgba::new(color[0] * a, color[1] * a, color[2] * a, 1.0);
    let s = Srgba::from(lin);
    [
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        255,
    ]
}

/// Spawn `Readback::texture(target)`, observe its `ReadbackComplete`, and POLL
/// `app.update()` until the bytes arrive — condition-based, NOT a fixed frame
/// count: the pipeline async-compiles, prepares, paints, copies, and maps across
/// several frames, so the number of frames is not knowable up front. Bounded by
/// `MAX_FRAMES`; panics with a clear message if the readback never fires.
///
/// Returns the un-padded `width*height*4` RGBA8 bytes. The raw readback buffer
/// keeps wgpu's 256-byte ROW PADDING whenever `width * 4` is not already
/// 256-aligned (a 32-px-wide target comes back as 256-byte rows = 2× the
/// pixels; every 64-px-wide test was aligned by luck, which hid this). The
/// padding is stripped HERE so callers can index `chunks_exact(4)` safely —
/// padding bytes are `[0,0,0,0]`, which would otherwise satisfy a
/// `px != clear` probe and false-green a "something painted" assertion.
pub fn readback_rgba(app: &mut App, target: Handle<Image>) -> Vec<u8> {
    const MAX_FRAMES: usize = 60;

    // The target's true extent — needed to detect + strip row padding below.
    let (width, height) = {
        let images = app.world().resource::<Assets<Image>>();
        let image = images.get(&target).expect("readback target Image exists");
        (
            image.texture_descriptor.size.width as usize,
            image.texture_descriptor.size.height as usize,
        )
    };

    let cell = CapturedBytes::default();
    app.insert_resource(cell.clone());

    let sink = cell.0.clone();
    app.world_mut().spawn(Readback::texture(target)).observe(
        move |trigger: On<ReadbackComplete>| {
            // `ReadbackComplete` derefs to its `data: Vec<u8>`; clone the raw
            // RGBA8 into the shared slot. First completion wins (the readback
            // re-fires every frame until its entity is despawned, but the poll
            // loop stops at the first non-empty slot).
            let mut slot = sink.lock().expect("readback sink mutex");
            if slot.is_none() {
                slot.replace(trigger.event().data.clone());
            }
        },
    );

    for _ in 0..MAX_FRAMES {
        app.update();
        if cell.0.lock().expect("readback sink mutex").is_some() {
            break;
        }
    }

    let data = cell
        .0
        .lock()
        .expect("readback sink mutex")
        .take()
        .unwrap_or_else(|| {
            panic!(
                "GPU readback never delivered bytes within {MAX_FRAMES} frames — \
                 the texture→buffer copy or buffer map never completed (check that \
                 the image carries COPY_SRC + RenderAssetUsages::all() and that a \
                 capture camera targets it)"
            )
        });

    // Strip wgpu's 256-byte row padding if present (see the doc comment).
    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    if data.len() == unpadded_row * height {
        data
    } else if data.len() == padded_row * height {
        let mut out = Vec::with_capacity(unpadded_row * height);
        for row in 0..height {
            let start = row * padded_row;
            out.extend_from_slice(&data[start..start + unpadded_row]);
        }
        out
    } else {
        panic!(
            "readback returned {} bytes for a {width}x{height} RGBA8 target — \
             expected {} (unpadded) or {} (256-byte-padded rows)",
            data.len(),
            unpadded_row * height,
            padded_row * height,
        );
    }
}
