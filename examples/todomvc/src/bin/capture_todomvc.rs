//! Headless RUN+view harness for the MVU TodoMVC (prototype verification).
//!
//! Renders the live `TodoPlugin` app offscreen and drives a full interaction
//! sequence through the REAL MVU path (synthesized `OnPress` → route → drain →
//! reconcile bind), capturing a PNG after each step so a human/agent can confirm
//! the structural reconcile (add/remove/filter) actually renders correctly.
//!
//! Run: `cargo run -p todomvc --bin capture_todomvc` (needs a wgpu adapter).

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
use todomvc::{
    AddButton, ClearButton, FilterButton, FilterMode, RowRef, TodoApp, TodoPlugin, ToggleButton,
};

const WIDTH: u32 = 720;
const HEIGHT: u32 = 540;

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
        .add_plugins(buiy::BuiyHeadlessPlugin)
        .add_plugins(TodoPlugin);

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
    settle(&mut app, 48);

    // 0: the three seeded items (one already done).
    capture(
        &mut app,
        target.clone(),
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-todo-0-seed.png",
    );
    assert_eq!(item_count(&mut app), 3, "3 seeded items");

    // 1: add two items via the Add button.
    let add = find::<AddButton>(&mut app);
    press(&mut app, add);
    press(&mut app, add);
    settle(&mut app, 8);
    capture(
        &mut app,
        target.clone(),
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-todo-1-added.png",
    );
    assert_eq!(item_count(&mut app), 5, "5 items after two adds");

    // 2: toggle "Buy milk" (id 0) done.
    let toggle0 = find_row_button::<ToggleButton>(&mut app, 0);
    press(&mut app, toggle0);
    settle(&mut app, 8);
    capture(
        &mut app,
        target.clone(),
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-todo-2-toggled.png",
    );

    // 3: filter to Active (the structural reconcile drops the done rows).
    let active = find_filter(&mut app, FilterMode::Active);
    press(&mut app, active);
    settle(&mut app, 8);
    capture(
        &mut app,
        target.clone(),
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-todo-3-active.png",
    );

    // 4: filter to Completed.
    let completed = find_filter(&mut app, FilterMode::Completed);
    press(&mut app, completed);
    settle(&mut app, 8);
    capture(
        &mut app,
        target.clone(),
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-todo-4-completed.png",
    );

    // 5: clear completed, back to All.
    let clear = find::<ClearButton>(&mut app);
    press(&mut app, clear);
    let all = find_filter(&mut app, FilterMode::All);
    press(&mut app, all);
    settle(&mut app, 8);
    capture(
        &mut app,
        target,
        "docs/reports/2026-06-30-demos-mvu-migration-assets/proto-todo-5-cleared.png",
    );

    println!(
        "OK: todomvc reconcile drove seed→add→toggle→filter→clear; final items = {}",
        item_count(&mut app)
    );
}

fn settle(app: &mut App, n: u32) {
    for _ in 0..n {
        app.update();
    }
}

fn item_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&TodoApp>()
        .iter(app.world())
        .next()
        .expect("model exists")
        .items
        .len()
}

fn press(app: &mut App, target: Entity) {
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(target));
    app.update();
}

fn find<M: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<M>>()
        .iter(app.world())
        .next()
        .expect("marker entity exists")
}

fn find_filter(app: &mut App, mode: FilterMode) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &FilterButton)>();
    q.iter(world)
        .find(|(_, f)| f.0 == mode)
        .map(|(e, _)| e)
        .expect("filter button exists")
}

fn find_row_button<M: Component>(app: &mut App, id: u64) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(Entity, &RowRef), With<M>>();
    q.iter(world)
        .find(|(_, r)| r.0 == id)
        .map(|(e, _)| e)
        .expect("row button exists for id")
}

/// One-shot GPU readback → PNG (mirrors the counter capture).
fn capture(app: &mut App, target: Handle<Image>, out: &str) {
    let slot: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let sink = slot.clone();
    let rb = app
        .world_mut()
        .spawn(Readback::texture(target))
        .observe(move |trigger: On<ReadbackComplete>| {
            let mut s = sink.lock().expect("sink");
            if s.is_none() {
                s.replace(trigger.event().data.clone());
            }
        })
        .id();

    for _ in 0..60 {
        app.update();
        if slot.lock().expect("sink").is_some() {
            break;
        }
    }
    let raw = slot
        .lock()
        .expect("sink")
        .take()
        .expect("GPU readback within 60 frames");
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
    let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels).expect("buffer matches w*h*4");
    if let Some(parent) = std::path::Path::new(out).parent() {
        std::fs::create_dir_all(parent).expect("create dir");
    }
    img.save(out).unwrap_or_else(|e| panic!("write {out}: {e}"));
    println!("wrote {out}");
}
