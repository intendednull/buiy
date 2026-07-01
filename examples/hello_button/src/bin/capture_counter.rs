//! Headless RUN+view harness for the MVU counter (prototype verification).
//!
//! Renders the live `CounterPlugin` app offscreen on a real wgpu adapter, captures
//! the initial state, synthesizes three `+` presses through the REAL MVU path
//! (write `OnPress` → `route_counter_press` → drain → `bind_counter_label`),
//! captures again, and asserts the model. The two PNGs let a human (or a
//! multimodal agent) confirm the counter is actually visible and updates — the
//! "invisible output" class a headless logic test misses.
//!
//! Run: `cargo run -p hello_button --bin capture_counter` (needs a wgpu adapter).

use std::sync::{Arc, Mutex};

use bevy::asset::{AssetApp, RenderAssetUsages};
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;

use buiy::OnPress;
use buiy_core::theme::default_dark_theme;
use hello_button::{Counter, CounterPlugin, IncButton};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                resolution: bevy::window::WindowResolution::new(WIDTH, HEIGHT),
                ..default()
            }),
            ..default()
        })
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::render::RenderPlugin::default())
        .add_plugins(bevy::image::ImagePlugin::default())
        .add_plugins(CameraPlugin)
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(bevy::input::InputPlugin)
        // The Buiy headless render subset (theme→layout→core→text→focus→a11y→
        // widgets→render); pulls MvuCorePlugin in transitively via widgets.
        .add_plugins(buiy::BuiyHeadlessPlugin)
        // The demo feature under test.
        .add_plugins(CounterPlugin);

    app.init_asset::<Mesh>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.insert_resource(default_dark_theme());

    // Offscreen Rgba8 target with COPY_SRC for readback.
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image.asset_usage = RenderAssetUsages::all();
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

    app.world_mut().spawn((
        Camera2d,
        RenderTarget::from(target.clone()),
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(0x0b, 0x0c, 0x0e)),
            ..default()
        },
    ));

    app.finish();
    app.cleanup();
    // Settle: Startup (setup) + layout → extract → paint + glyph atlas.
    for _ in 0..48 {
        app.update();
    }

    capture(
        &mut app,
        target.clone(),
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-counter-0.png",
    );
    assert_eq!(model_value(&mut app), 0, "counter starts at 0");

    // Synthesize three `+` presses through the real MVU path.
    let inc = app
        .world_mut()
        .query_filtered::<Entity, With<IncButton>>()
        .iter(app.world())
        .next()
        .expect("the + button exists");
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<Messages<OnPress>>()
            .write(OnPress(inc));
        app.update(); // route → enqueue → drain → bind, one frame each
    }
    for _ in 0..16 {
        app.update(); // settle the re-layout/re-paint of the changed label
    }

    assert_eq!(
        model_value(&mut app),
        3,
        "three + presses folded to value 3"
    );
    capture(
        &mut app,
        target,
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-counter-3.png",
    );
    println!("OK: counter folded 0 → 3; wrote proto-counter-0.png + proto-counter-3.png");
}

fn model_value(app: &mut App) -> i64 {
    app.world_mut()
        .query::<&Counter>()
        .iter(app.world())
        .next()
        .expect("counter model exists")
        .value
}

/// Spawn a one-shot `Readback`, poll until the bytes arrive, strip wgpu's 256-byte
/// row padding, write a PNG, then despawn the readback entity.
fn capture(app: &mut App, target: Handle<Image>, out: &str) {
    let slot: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let sink = slot.clone();
    let rb = app
        .world_mut()
        .spawn(Readback::texture(target))
        .observe(move |trigger: On<ReadbackComplete>| {
            let mut s = sink.lock().expect("readback sink");
            if s.is_none() {
                s.replace(trigger.event().data.clone());
            }
        })
        .id();

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
    app.world_mut().despawn(rb);

    let unpadded = (WIDTH * 4) as usize;
    let padded = unpadded.div_ceil(256) * 256;
    let h = HEIGHT as usize;
    let pixels = if raw.len() == unpadded * h {
        raw
    } else {
        let mut out = Vec::with_capacity(unpadded * h);
        for row in 0..h {
            let start = row * padded;
            out.extend_from_slice(&raw[start..start + unpadded]);
        }
        out
    };

    let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels)
        .expect("readback buffer matches width*height*4");
    if let Some(parent) = std::path::Path::new(out).parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    img.save(out).unwrap_or_else(|e| panic!("write {out}: {e}"));
    println!("wrote {out} ({WIDTH}x{HEIGHT})");
}
