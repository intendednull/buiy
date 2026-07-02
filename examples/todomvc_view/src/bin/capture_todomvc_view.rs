//! Headless RUN+view harness for the `buiy_view` TodoMVC.
//!
//! Renders the live surface offscreen on a real wgpu adapter and captures the
//! seed frame — the card ("todos"), the draft text-input (seeded with a
//! populated draft so the input shows text), the three keyed rows (id 1 checked,
//! the seeded-done item), and the derived "N items left" footer. The PNG lets a
//! human/agent confirm the reconciler (keyed list + the two stateful-leaf
//! widgets) actually renders, not just compiles.
//!
//! Run: `cargo run -p todomvc_view --bin capture_todomvc_view` (needs a wgpu adapter).
//!
//! Known cosmetic issue (out of scope): the checkbox ✓ (U+2713) renders as tofu
//! because the default font lacks the glyph — a pre-existing default-font gap
//! (the widget-catalog finding), NOT introduced by `buiy_view`.

use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;

use buiy_core::theme::default_dark_theme;
use todomvc_view::{TodoApp, install_with, seed};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
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

    // Seed a populated draft so the input renders with text (not the placeholder).
    let init = TodoApp {
        draft: "Buy eggs".into(),
        ..seed()
    };
    install_with(&mut app, init);

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
    // Settle: Startup (spawn model+camera) → reconcile builds the tree (keyed
    // rows + editor + checkboxes) → layout → extract → paint + glyph atlas.
    for _ in 0..64 {
        app.update();
    }

    let remaining = model(&mut app).items.iter().filter(|t| !t.done).count();
    assert_eq!(remaining, 2, "seed: id1 done → 2 items left");
    assert_eq!(model(&mut app).items.len(), 3, "seed has 3 rows");

    capture(
        &mut app,
        target,
        &format!("{OUT_DIR}/buiy-view-todomvc.png"),
    );
    println!("OK: buiy_view TodoMVC rendered (card + draft input + 3 keyed rows + '2 items left')");
}

fn model(app: &mut App) -> TodoApp {
    app.world_mut()
        .query::<&TodoApp>()
        .iter(app.world())
        .next()
        .expect("todo model exists")
        .clone()
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
