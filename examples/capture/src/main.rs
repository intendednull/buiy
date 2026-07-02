//! Headless screenshot capture for the README assets.
//!
//! Renders Buiy scenes **offscreen** on a real wgpu adapter (no window / X
//! server needed) via render-to-texture + GPU readback — the same path the
//! `buiy_core` GPU golden tests exercise (`crates/buiy_core/tests/support/mod.rs`).
//! Every scene is built from real, shipping Buiy components, so the captured
//! images honestly reflect current capabilities.
//!
//! Run (from the workspace root):
//!
//! ```sh
//! cargo run -p capture --release
//! ```
//!
//! Writes `docs/assets/*.png`. No environment variables are required on a host
//! with a working Vulkan/Metal/DX adapter; force a backend with
//! `WGPU_BACKEND=vulkan` only if adapter selection picks the wrong device.

use bevy::asset::{AssetApp, RenderAssetUsages};
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use std::sync::{Arc, Mutex};

use buiy::{AlignItems, JustifyContent};
use buiy::{Background, Border, Corners, FontSize, Node, Radius, Style, Text, TextColor};
use buiy_core::render::color::ColorToken;
use buiy_core::{CorePlugin, render::BuiyRenderPlugin};

// Default light-theme tokens (see `buiy_core::theme::default_light_theme`).
const SURFACE_PRIMARY: ColorToken = ColorToken::SurfacePrimary; // white
const SURFACE_SECONDARY: ColorToken = ColorToken::SurfaceSecondary; // #f5f5f5
const TEXT_PRIMARY: ColorToken = ColorToken::TextPrimary; // #1a1a1a
const TEXT_SECONDARY: ColorToken = ColorToken::TextSecondary; // #666
const ACCENT: ColorToken = ColorToken::Accent; // #3372f2

fn main() {
    // The README hero: layout + text + theming composed from primitives.
    render_scene(
        720,
        430,
        Color::srgb(0.93, 0.94, 0.96),
        "docs/assets/showcase.png",
        scene_showcase,
    );

    // The `hello_text` demo scene: themed title + wrapped paragraph through the
    // cosmic-text shaping → glyph-atlas → coverage-draw pipeline.
    render_scene(
        620,
        300,
        Color::WHITE,
        "docs/assets/hello_text.png",
        scene_hello_text,
    );

    println!("done");
}

/// Spawn a text node carrying one theme-tinted string at `size` px.
fn spawn_text(world: &mut World, s: &str, size: f32, token: ColorToken) -> Entity {
    world
        .spawn((
            Node,
            Style::default(),
            Text(s.to_string()),
            FontSize(size),
            TextColor(token),
        ))
        .id()
}

/// A labelled, themed button box (fixed size, centered label) — what an author
/// composes today from `Node` + `Style` + `Background` + `Border` + a text child.
fn spawn_button(
    world: &mut World,
    label: &str,
    bg: ColorToken,
    fg: ColorToken,
    width: f32,
) -> Entity {
    let label = spawn_text(world, label, 15.0, fg);
    world
        .spawn((
            Node,
            Style::default()
                .width_px(width)
                .height_px(44.0)
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center),
            Background { color: bg },
            Border {
                radius: Corners::all(Radius::circular(8.0)),
                ..Default::default()
            },
        ))
        .add_children(&[label])
        .id()
}

/// Hero card: heading + paragraph + a row of two buttons, centered in the view.
fn scene_showcase(world: &mut World) {
    let heading = spawn_text(world, "Accessible UI for Bevy", 28.0, TEXT_PRIMARY);
    let paragraph = spawn_text(
        world,
        "A parallel, AccessKit-first toolkit: a CSS-subset layout engine over \
         Taffy, cosmic-text shaping, and a custom wgpu render pipeline.",
        15.0,
        TEXT_SECONDARY,
    );
    let primary = spawn_button(world, "Get started", ACCENT, SURFACE_PRIMARY, 150.0);
    let secondary = spawn_button(world, "Docs", SURFACE_SECONDARY, TEXT_PRIMARY, 110.0);
    let buttons = world
        .spawn((Node, Style::default().flex_row().gap_px(12.0)))
        .add_children(&[primary, secondary])
        .id();

    let card = world
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(600.0)
                .padding(32.0)
                .gap_px(18.0),
            Background {
                color: SURFACE_PRIMARY,
            },
            Border {
                radius: Corners::all(Radius::circular(16.0)),
                ..Default::default()
            },
        ))
        .add_children(&[heading, paragraph, buttons])
        .id();

    // Full-viewport container that centers the card both axes.
    world
        .spawn((
            Node,
            Style::default()
                .width_px(720.0)
                .height_px(430.0)
                .flex_column()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center),
        ))
        .add_children(&[card]);
}

/// The `examples/hello_text` scene, verbatim in spirit: a 32px title above a
/// 16px wrapped body in a 560px padded column.
fn scene_hello_text(world: &mut World) {
    let title = spawn_text(world, "Hello, Buiy text!", 32.0, TEXT_PRIMARY);
    let body = spawn_text(
        world,
        "The quick brown fox jumps over the lazy dog. Shaped by cosmic-text at \
         the committed wrap width, rasterized once per (font, size, weight, \
         subpixel-bin) into the shared coverage atlas, tinted per instance — a \
         theme switch never touches the atlas.",
        16.0,
        TEXT_SECONDARY,
    );
    world
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(560.0)
                .padding(24.0)
                .gap_px(12.0),
        ))
        .add_children(&[title, body]);
}

/// Build the headless painting app, run `build` to populate the scene, render
/// it offscreen, read the pixels back, and write a PNG to `out`.
///
/// Plugin stack mirrors `gpu_render_app_with_resolution` in
/// `crates/buiy_core/tests/support/mod.rs` — the canonical headless render path.
fn render_scene(width: u32, height: u32, clear: Color, out: &str, build: impl FnOnce(&mut World)) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // Window sized to the capture target so the primary-window-derived view
        // uniform matches the offscreen image's pixel grid (support/mod.rs note).
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
        .add_plugins(CameraPlugin)
        // The `Core2d` graph `BuiyRenderPlugin` wires its node into — must
        // precede `BuiyRenderPlugin` (plugins build in add order).
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(CorePlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(BuiyRenderPlugin);
    app.init_asset::<Mesh>();
    // Bevy 0.19: `CameraPlugin`'s `update_skinned_mesh_bounds` reads
    // `Res<Assets<SkinnedMeshInverseBindposes>>` (the second asset `MeshPlugin`
    // inits alongside `Mesh`) as a non-`Option` param, which now PANICS if absent
    // (0.18 silently skipped). Real apps get it via `DefaultPlugins` → `MeshPlugin`;
    // this hand-rolled capture stack must init it like `Mesh`.
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();

    // Offscreen `Rgba8UnormSrgb` target: `COPY_SRC` for the readback copy,
    // `RenderAssetUsages::all()` so the `GpuImage` exists in the render world.
    let mut image = Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image.asset_usage = RenderAssetUsages::all();
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

    // Capture camera → offscreen target, clearing to the page background.
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::from(target.clone()),
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(clear),
            ..default()
        },
    ));

    build(app.world_mut());

    // Materialize the device + pipelines, then settle enough frames for layout →
    // extract → prepare → paint and (for text) the glyph atlas to fill.
    app.finish();
    app.cleanup();
    for _ in 0..48 {
        app.update();
    }

    let pixels = readback_rgba(&mut app, target, width, height);
    let img = image::RgbaImage::from_raw(width, height, pixels)
        .expect("readback buffer matches width*height*4");
    if let Some(parent) = std::path::Path::new(out).parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    img.save(out).unwrap_or_else(|e| panic!("write {out}: {e}"));
    println!("wrote {out} ({width}x{height})");
}

/// Spawn a `Readback`, observe its completion, poll frames until the bytes
/// arrive, then strip wgpu's 256-byte row padding. Returns un-padded RGBA8.
/// (Mirrors `support::readback_rgba`.)
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
