//! Offscreen **composites showcase** capture (parity Wave C2 verification artifact).
//! Renders the `composites_showcase` grid (the
//! [`buiy_gallery::composites`] builder) — one of each composite widget
//! (stepper / segmented / search / meter /
//! badge / chip / kbd / status-dots / stat-row / table header+rows) plus a live
//! `show_toast` — to an offscreen texture on a real wgpu adapter (no on-screen
//! window), reads the pixels back, and writes
//! `docs/reports/parity-proto-assets/c2-composites.png` so the composites can be
//! eyeballed against the design. Mirrors `capture_shell`'s render-to-texture + GPU
//! readback path (the canonical `buiy_core` GPU golden path).
//!
//! Run on a GPU host (this prototype's RX 6700 XT / lavapipe):
//!
//! ```sh
//! cargo run -p buiy_gallery --bin capture_composites
//! ```
//!
//! Not a CI gate — the headless `composites_layout` test is the regression guard;
//! this is the human-eyeball artifact.

use std::sync::{Arc, Mutex};

use bevy::asset::{AssetApp, RenderAssetUsages};
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;

use buiy_core::animation::AnimationPlugin;
use buiy_core::theme::default_dark_theme;
use buiy_gallery::composites::{ToastPlugin, composites_showcase, show_toast};
// `set_meter` was promoted to the framework (Wave 5 refinement).
use buiy_widgets::composites::set_meter;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 760;
const OUT: &str = "docs/reports/parity-proto-assets/c2-composites.png";

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // A window sized to the capture target so the primary-window-derived view
        // uniform matches the offscreen pixel grid; the page root's `100%` also
        // resolves against this window.
        .add_plugins(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                resolution: bevy::window::WindowResolution::new(WIDTH, HEIGHT),
                ..default()
            }),
            ..default()
        })
        .add_plugins(bevy::asset::AssetPlugin::default())
        // `composites_showcase` → `search_input` spawns a text-input scene, which
        // needs the scene infrastructure.
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::render::RenderPlugin::default())
        .add_plugins(bevy::image::ImagePlugin::default())
        .add_plugins(CameraPlugin)
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(bevy::input::InputPlugin)
        // The Buiy headless render subset (theme → layout → core → text → focus →
        // a11y → widgets → render) the composites PAINT path needs, as ONE plugin,
        // PLUS the animation + toast plugins on top (so the meter `ScaleTween` + the
        // toast entrance tweens + the toast auto-dismiss timer tick settle in the
        // captured frame — `AnimationPlugin` is intentionally NOT in the headless
        // subset, since a static capture does not need it).
        .add_plugins(buiy::BuiyHeadlessPlugin)
        .add_plugins(AnimationPlugin)
        .add_plugins(ToastPlugin);

    app.init_asset::<Mesh>();
    // Bevy 0.19 `CameraPlugin` reads `Assets<SkinnedMeshInverseBindposes>` as a
    // non-`Option` param (panics if absent); real apps get it via `MeshPlugin`.
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();

    // Dark theme so the design tokens resolve.
    app.insert_resource(default_dark_theme());

    // Offscreen `Rgba8UnormSrgb` target with `COPY_SRC` for readback.
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image.asset_usage = RenderAssetUsages::all();
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

    // Capture camera → offscreen target, clearing to the app background.
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::from(target.clone()),
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(0x0b, 0x0c, 0x0e)),
            ..default()
        },
    ));

    // Build the showcase grid + animate the meter (so the captured frame shows the
    // tween settled to the new fraction) + raise a toast (so the top-layer card is
    // in frame).
    {
        let world = app.world_mut();
        let (_page, meter_fill) = composites_showcase(world);
        // Re-target the meter to a different fraction; the 64-frame settle below
        // runs the 0.3s tween to completion, so the captured fill shows 0.42.
        set_meter(world, meter_fill, 0.42);
        show_toast(world, "Build complete · 0 errors");
    }

    // Materialize the device + pipelines, then settle enough frames for layout →
    // extract → prepare → paint, the glyph/icon atlas to fill, and the tweens to
    // run (the toast entrance is ~180ms; the meter ~300ms — 64 frames covers both
    // well past completion). NOTE: the toast auto-dismiss is 2.2s, so it is still
    // on-screen at 64 frames.
    app.finish();
    app.cleanup();
    for _ in 0..64 {
        app.update();
    }

    let pixels = readback_rgba(&mut app, target, WIDTH, HEIGHT);
    let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels)
        .expect("readback buffer matches width*height*4");
    if let Some(parent) = std::path::Path::new(OUT).parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    img.save(OUT).unwrap_or_else(|e| panic!("write {OUT}: {e}"));
    println!("wrote {OUT} ({WIDTH}x{HEIGHT})");
}

/// Spawn a `Readback`, observe its completion, poll frames until the bytes arrive,
/// then strip wgpu's 256-byte row padding. Returns un-padded RGBA8. (Mirrors
/// `capture_shell::readback_rgba`.)
fn readback_rgba(app: &mut App, target: Handle<Image>, width: u32, height: u32) -> Vec<u8> {
    let slot: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let sink = slot.clone();
    app.world_mut().spawn(Readback::texture(target)).observe(
        move |trigger: On<ReadbackComplete>| {
            let mut s = sink.lock().expect("readback sink");
            if s.is_none() {
                s.replace(trigger.event().data.clone());
            }
        },
    );

    for _ in 0..60 {
        app.update();
        if slot.lock().expect("readback sink").is_some() {
            break;
        }
    }
    let raw = slot
        .lock()
        .expect("readback sink")
        .take()
        .expect("GPU readback delivered bytes within 60 frames");

    let unpadded = (width * 4) as usize;
    let padded = unpadded.div_ceil(256) * 256;
    let h = height as usize;
    if raw.len() == unpadded * h {
        raw
    } else if raw.len() == padded * h {
        let mut out = Vec::with_capacity(unpadded * h);
        for row in 0..h {
            let start = row * padded;
            out.extend_from_slice(&raw[start..start + unpadded]);
        }
        out
    } else {
        panic!(
            "readback returned {} bytes for {width}x{height} — expected {} or {}",
            raw.len(),
            unpadded * h,
            padded * h
        );
    }
}
