//! Headless RUN+view harness for the `buiy_view` Counter.
//!
//! Renders the live surface offscreen on a real wgpu adapter, captures the seed
//! frame (`Count: 0` + the `- + Reset` row, Reset dimmed/disabled), then
//! synthesizes three `+` presses through the REAL path (write `OnPress` →
//! `route_presses` → drain → reconciler re-patches the label) and captures the
//! `Count: 3` frame (Reset now bright/enabled). The two PNGs let a human/agent
//! confirm the reconciler + router actually work, not just compile.
//!
//! Run: `cargo run -p counter_view --bin capture_counter_view` (needs a wgpu adapter).

use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;

use buiy_core::interaction::OnPress;
use buiy_core::theme::default_dark_theme;
use buiy_view::find_press_target;
use counter_view::{Counter, Msg};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;
// Repo-relative output (under the gitignored `target/`); `capture()` create_dir_all's it.
const OUT_DIR: &str = "target/view-captures";

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
        .add_plugins(buiy::BuiyHeadlessPlugin);
    counter_view::install(&mut app);

    app.init_asset::<Mesh>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.insert_resource(default_dark_theme());

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
    // Settle: Startup (spawn model+camera) → reconcile builds tree → layout →
    // extract → paint + glyph atlas.
    for _ in 0..48 {
        app.update();
    }

    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/buiy-view-counter-0.png"),
    );
    assert_eq!(model_value(&mut app), 0, "counter starts at 0");

    // Three `+` presses through the REAL router (write OnPress on the + button).
    let inc = find_press_target::<Counter>(app.world_mut(), &Msg::Inc)
        .expect("the + button has a press handler");
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<Messages<OnPress>>()
            .write(OnPress(inc));
        app.update(); // route(Enqueue) → drain(Drain) → reconcile(before Layout, next frame)
    }
    // Settle the re-layout/re-paint of the one-frame-later patched label (#10).
    for _ in 0..16 {
        app.update();
    }

    assert_eq!(model_value(&mut app), 3, "three + presses folded to 3");
    capture(
        &mut app,
        target,
        &format!("{OUT_DIR}/buiy-view-counter-3.png"),
    );
    println!(
        "OK: buiy_view counter folded 0 → 3; wrote buiy-view-counter-0.png + buiy-view-counter-3.png"
    );
}

fn model_value(app: &mut App) -> i32 {
    app.world_mut()
        .query::<&Counter>()
        .iter(app.world())
        .next()
        .expect("counter model exists")
        .count
}

/// One-shot `Readback` → strip wgpu row padding → PNG.
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
