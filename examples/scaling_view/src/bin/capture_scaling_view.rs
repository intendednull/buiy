//! Headless RUN+view harness for the `buiy_view` scaling/composition demo.
//!
//! Renders the live surface offscreen on a real wgpu adapter and drives it through the REAL
//! path (write `OnPress` → `route_presses` → drain → reconciler): captures the seed frame, then
//! bumps both embedded Counters (message-lifted), reveals the `when`-gated details panel, and
//! fires the async `Cmd::task` "Load". It runs frames until the async result folds back (the
//! effect is a real 400 ms off-thread wait), asserts the resulting model state, and captures the
//! loaded frame. The two PNGs let a human/agent confirm map + when + async actually render.
//!
//! Run: `cargo run -p scaling_view --bin capture_scaling_view` (needs a wgpu adapter).

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
use scaling_view::{Msg, ScalingApp, counter};

const WIDTH: u32 = 720;
const HEIGHT: u32 = 560;
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
    scaling_view::install(&mut app);

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
    // Settle: Startup (spawn model+camera) → reconcile builds the tree → layout → extract → paint.
    for _ in 0..48 {
        app.update();
    }

    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/buiy-view-scaling-0.png"),
    );

    // Drive the composition paths through the REAL router. Bump left twice + right once
    // (message-lifted child Counters), reveal the details panel (`when`), then fire the async
    // Load (`Cmd::task`).
    fire(&mut app, &Msg::Left(counter::Msg::Inc));
    fire(&mut app, &Msg::Left(counter::Msg::Inc));
    fire(&mut app, &Msg::Right(counter::Msg::Inc));
    fire(&mut app, &Msg::ToggleDetails);
    fire(&mut app, &Msg::Load);

    // The Load button is now disabled (on_press_maybe(None) while loading); run frames until the
    // async task completes and its result folds back through the funnel + reconciles.
    let mut loaded = false;
    for _ in 0..300 {
        app.update();
        if model(&mut app).loaded.is_some() {
            loaded = true;
            break;
        }
    }
    assert!(
        loaded,
        "the async Cmd::task result folded back within the budget"
    );

    let m = model(&mut app);
    assert_eq!(m.left.count, 2, "left Counter bumped twice (map lift)");
    assert_eq!(m.right.count, 1, "right Counter bumped once (map lift)");
    assert!(m.show_details, "the when-gated details panel is shown");
    assert_eq!(m.loaded.as_deref(), Some("42 rows"), "async result folded");
    assert_eq!(m.loads, 1, "exactly one load completed");
    assert!(!m.loading, "loading cleared by the result fold");

    // A couple more frames so the loaded-state kind-swap (text→button) + details panel are laid
    // out and painted before the readback.
    for _ in 0..16 {
        app.update();
    }
    capture(
        &mut app,
        target,
        &format!("{OUT_DIR}/buiy-view-scaling-1.png"),
    );
    println!(
        "OK: buiy_view scaling demo — left=2 right=1 (map), details shown (when), '42 rows' loaded (Cmd::task); \
         wrote buiy-view-scaling-0.png + buiy-view-scaling-1.png"
    );
}

/// Write a real `OnPress` on the button whose handler carries `want`, then run the two frames
/// the reconciler needs (route+drain, then reconcile-before-layout).
fn fire(app: &mut App, want: &Msg) {
    let target = find_press_target::<ScalingApp>(app.world_mut(), want)
        .unwrap_or_else(|| panic!("a button routes {want:?}"));
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(target));
    app.update(); // route(Enqueue) → drain(Drain): model changes
    app.update(); // reconcile(before Layout): the derived tree patches
}

fn model(app: &mut App) -> ScalingApp {
    app.world_mut()
        .query::<&ScalingApp>()
        .iter(app.world())
        .next()
        .expect("scaling model exists")
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
