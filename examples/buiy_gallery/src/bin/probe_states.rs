//! **Widget-state probe** (audit tooling, not a CI gate). Renders the gallery
//! **Controls / Showcase** screen with its transform- and position-sensitive
//! widgets forced into their NON-resting states — every Switch ON, the Slider at
//! max, the density Segmented moved off its default, the Stepper bumped, the Meter
//! full, and every Disclosure EXPANDED — to an offscreen texture on a real wgpu
//! adapter, then reads the pixels back and writes a PNG for eyeball inspection.
//!
//! The resting states are covered by `capture_shell` (per-screen) and
//! `capture_composites`; this fills the gap those static captures leave: the
//! states a user only reaches by *interacting* (a slid switch thumb, an expanded
//! disclosure caret + body, a moved slider thumb/fill), where render/layout bugs
//! like the recently-fixed chevron-affine class hide.
//!
//! ```sh
//! cargo run -p buiy_gallery --bin probe_states           # default out
//! PROBE_OUT=/tmp/showcase-active.png cargo run -p buiy_gallery --bin probe_states
//! ```

use std::sync::{Arc, Mutex};

use bevy::asset::{AssetApp, RenderAssetUsages};
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::ecs::component::Component;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;

use buiy_core::a11y::{A11yExpanded, A11yToggled, A11yValue, Toggled};
use buiy_core::animation::AnimationPlugin;
use buiy_core::theme::default_dark_theme;
use buiy_gallery::composites::{ToastPlugin, set_segmented, set_stepper};
use buiy_gallery::inspector::{InspectorPlugin, build_inspector_content};
use buiy_gallery::shell::{
    Screen, ScreenRouter, build_shell, mount_screens, reflect_active_screen,
};
use buiy_gallery::{
    ModalPlugin, OverlayMenuPlugin, ScrollListPlugin, ShowcaseDensitySegmented, ShowcasePlugin,
    ShowcaseStepper, TodoMvcPlugin,
};
use buiy_widgets::composites::{MeterFill, set_meter};
use buiy_widgets::{Disclosure, Slider, Switch};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const DEFAULT_OUT: &str = "docs/reports/audit-assets/showcase-active.png";

/// Collect every entity carrying marker `C`.
fn all_with<C: Component>(world: &mut World) -> Vec<Entity> {
    let mut q = world.query_filtered::<Entity, With<C>>();
    q.iter(world).collect()
}

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
        // The showcase drivers repaint on `Changed<A11y*>`; `AnimationPlugin` ticks
        // any tween the meter/switch path uses (harmless when idle).
        .add_plugins(AnimationPlugin)
        .add_plugins(TodoMvcPlugin)
        .add_plugins(ScrollListPlugin)
        .add_plugins(OverlayMenuPlugin)
        .add_plugins(ModalPlugin)
        .add_plugins(ShowcasePlugin)
        .add_plugins(ToastPlugin)
        .add_plugins(InspectorPlugin);

    app.add_systems(bevy::app::Update, reflect_active_screen);

    app.init_asset::<Mesh>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.insert_resource(default_dark_theme());
    app.insert_resource(ScreenRouter(Screen::Showcase));

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

    {
        let world = app.world_mut();
        build_shell(world);
        mount_screens(world);
        build_inspector_content(world);
    }

    app.finish();
    app.cleanup();
    // Settle a couple frames so the tree + a11y reach steady state before forcing.
    for _ in 0..4 {
        app.update();
    }

    // --- Force the Showcase widgets into their NON-resting states ---------------
    {
        let world = app.world_mut();

        // Every Switch ON (drive_showcase_switches slides the thumb on Changed<A11yToggled>).
        for e in all_with::<Switch>(world) {
            world.entity_mut(e).insert(A11yToggled(Toggled::True));
        }

        // Every Slider at MAX (drive_showcase_slider tracks A11yValue.now).
        for e in all_with::<Slider>(world) {
            if let Some(v) = world.get::<A11yValue>(e).cloned() {
                world.entity_mut(e).insert(A11yValue { now: v.max, ..v });
            }
        }

        // Every Disclosure EXPANDED (drive_showcase_disclosures rotates the chevron
        // + reveals the body on Changed<A11yExpanded>).
        for e in all_with::<Disclosure>(world) {
            world.entity_mut(e).insert(A11yExpanded(true));
        }

        // Density segmented → last option (Dense); stepper bumped; meter full.
        for track in all_with::<ShowcaseDensitySegmented>(world) {
            set_segmented(world, track, 2);
        }
        for stepper in all_with::<ShowcaseStepper>(world) {
            set_stepper(world, stepper, 42);
        }
        for fill in all_with::<MeterFill>(world) {
            set_meter(world, fill, 1.0);
        }
    }

    // Settle: layout → extract → prepare → paint, atlas fill, driver repaints, tweens.
    for _ in 0..64 {
        app.update();
    }

    let out = std::env::var("PROBE_OUT").unwrap_or_else(|_| DEFAULT_OUT.to_string());
    let pixels = readback_rgba(&mut app, target, WIDTH, HEIGHT);
    let img = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels)
        .expect("readback buffer matches width*height*4");
    if let Some(parent) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    img.save(&out)
        .unwrap_or_else(|e| panic!("write {out}: {e}"));
    println!("wrote {out} ({WIDTH}x{HEIGHT})");
}

/// Spawn a `Readback`, observe completion, poll frames, strip wgpu row padding.
/// (Mirrors `capture_shell::readback_rgba`.)
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
