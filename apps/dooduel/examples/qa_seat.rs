//! QA seat-driver (dev tool) — runs the REAL Dooduel client headless, renders it
//! offscreen to `screen.png`, snapshots the semantic tree to `ui.md`, and drives it
//! through real widget interactions from `commands.jsonl`. Spec:
//! `docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md`.
//!
//! Run (needs a real wgpu adapter; no display required):
//!   RUST_MIN_STACK=33554432 cargo run -p dooduel --example qa_seat -- --dir /tmp/qa-seat-1

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::camera::{NormalizedRenderTarget, RenderTarget};
use bevy::image::Image;
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::window::{PrimaryWindow, WindowRef};

use buiy_core::ResolvedLayout;
use buiy_core::a11y::A11yRole;
use buiy_core::a11y::report::snapshot_report;
use buiy_core::a11y::translate::entity_for_node_id;
use buiy_verify::pointer::drive_stroke;

use dooduel::{Dooduel, Screen};

const W: u32 = 1280;
const H: u32 = 800;

/// Build the real client on a headless render stack + picking (spec §2.1). The
/// primary Window is created by `WindowPlugin`; the two cameras + pointer are spawned
/// in `spawn_view`. Returns the app (not yet `finish`ed).
fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                resolution: bevy::window::WindowResolution::new(W, H),
                ..default()
            }),
            ..default()
        })
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::render::RenderPlugin::default())
        .add_plugins(bevy::image::ImagePlugin::default())
        .add_plugins(bevy::camera::CameraPlugin)
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(bevy::input::InputPlugin)
        // Picking core (bevy). NOT DefaultPickingPlugins → no winit PointerInputPlugin,
        // so our synthetic PointerId::Mouse is the only pointer.
        .add_plugins(bevy::picking::PickingPlugin)
        // Buiy headless: core (+GlobalTransform bridge) · theme · a11y TREE · focus ·
        // layout · text · widgets · render. Omits the winit AccessKit adapter + picking.
        .add_plugins(buiy::BuiyHeadlessPlugin)
        // Re-add picking (BuiyHeadlessPlugin omits it) + fidelity plugins (scroll/anim
        // the live app runs — chat stick-to-bottom, button press-dips).
        .add_plugins(buiy_core::picking::PickingPlugin)
        .add_plugins(buiy_core::picking::BuiyPickingBackendPlugin)
        .add_plugins(buiy_core::scroll::ScrollInputPlugin)
        .add_plugins(buiy_core::animation::AnimationPlugin);
    app.init_asset::<Mesh>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    // The real app's runtime bundle: theme · ClockPlugin (real-time Tick) · Viewport ·
    // Net · WsClient · Canvas · Confetti · Storage.
    dooduel::install_runtime(&mut app);
    app
}

/// Handles the view needs after `WindowPlugin` created the primary window: the offscreen
/// readback Image, the picking camera (Window target), the readback camera (Image target),
/// and the synthetic mouse pointer (targets the primary window). Returns
/// `(image, window, pointer)`.
fn spawn_view(app: &mut App) -> (Handle<Image>, Entity, Entity) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("WindowPlugin created a primary window");

    // The offscreen readback texture (capture.rs:228-231 pattern).
    let mut image = Image::new_target_texture(W, H, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image.asset_usage = RenderAssetUsages::all();
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

    // Picking camera → Window (backend.rs:129-138 requires a Window-target active camera
    // for the pointer; it never gets a swapchain headless, so it renders nothing — C1).
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::Window(WindowRef::Primary),
        Camera {
            order: -1,
            ..default()
        },
    ));
    // Readback camera → Image (renders the tree for the screenshot).
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::from(target.clone()),
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(0xf4, 0xf5, 0xf8)),
            ..default()
        },
    ));

    // The synthetic pointer, targeting the primary window (pointer.rs:197-208 shape).
    let norm = WindowRef::Entity(window)
        .normalize(Some(window))
        .expect("normalize primary window");
    let pointer = app
        .world_mut()
        .spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target: NormalizedRenderTarget::Window(norm),
                position: Vec2::ZERO,
            }),
        ))
        .id();

    (target, window, pointer)
}

/// One GPU readback of `image` → tight RGBA bytes (row-padding stripped). Pumps up to 60
/// frames until `ReadbackComplete` fires (capture.rs:377-416).
fn readback_rgba(app: &mut App, image: &Handle<Image>) -> Vec<u8> {
    let slot: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let sink = slot.clone();
    let rb = app
        .world_mut()
        .spawn(Readback::texture(image.clone()))
        .observe(move |trigger: On<ReadbackComplete>| {
            let mut s = sink.lock().expect("readback sink");
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

    let unpadded = (W * 4) as usize;
    let padded = unpadded.div_ceil(256) * 256;
    let h = H as usize;
    if raw.len() == unpadded * h {
        raw
    } else {
        let mut out = Vec::with_capacity(unpadded * h);
        for row in 0..h {
            let start = row * padded;
            out.extend_from_slice(&raw[start..start + unpadded]);
        }
        out
    }
}

/// Save the readback as a PNG.
fn save_png(bytes: Vec<u8>, path: &std::path::Path) {
    let img = image::RgbaImage::from_raw(W, H, bytes).expect("W*H*4 bytes");
    img.save(path)
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// The semantic snapshot (role tree + text/layout dump).
fn snapshot_md(app: &mut App) -> String {
    snapshot_report(app.world_mut())
}

/// Resolve a node by role+name and click it through a REAL synthetic pointer at its center
/// (spec §res-Q6 — the pointer path, not the a11y typed click). Returns the typed error on
/// a miss (a genuine QA signal).
fn click_role(
    app: &mut App,
    window: Entity,
    pointer: Entity,
    role: A11yRole,
    name: &str,
) -> Result<(), buiy_core::a11y::ActionError> {
    let node = buiy::probe::get_by_role(app.world_mut(), role, Some(name), None)?;
    let e =
        entity_for_node_id(node).ok_or(buiy_core::a11y::ActionError::NotFound { target: node })?;
    let (tl, size) = {
        let w = app.world();
        let tl = w
            .get::<GlobalTransform>(e)
            .expect("laid-out node has GlobalTransform")
            .translation()
            .truncate();
        let size = w
            .get::<ResolvedLayout>(e)
            .expect("node has ResolvedLayout")
            .size;
        (tl, size)
    };
    let center = tl + size * 0.5;
    drive_stroke(
        app,
        window,
        pointer,
        &[center, center + Vec2::new(1.0, 0.0)],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// File protocol (spec §2.3): atomic tmp+rename writes, byte-cursor command tail.
// ---------------------------------------------------------------------------

/// Write `bytes` atomically (tmp + rename) unless byte-identical to what's already there.
/// Returns true if it wrote.
fn atomic_write_if_changed(path: &Path, bytes: &[u8]) -> bool {
    if let Ok(existing) = fs::read(path) {
        if existing == bytes {
            return false;
        }
    }
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(&tmp, bytes).unwrap_or_else(|e| panic!("write {}: {e}", tmp.display()));
    fs::rename(&tmp, path).unwrap_or_else(|e| panic!("rename {}: {e}", path.display()));
    true
}

/// Append a line to `path` (driver.log), creating it if absent.
fn append_line(path: &Path, line: &str) {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    writeln!(f, "{line}").expect("append driver.log");
}

/// Read new complete lines from `commands.jsonl` past `*cursor`, advancing `*cursor` by the
/// bytes consumed. EVERY whole `\n`-terminated line is returned — **including blank ones** —
/// so the caller's `consumed: K` index equals the number of lines the agent appended
/// (spec §2.3); blank/malformed lines are logged-and-skipped by the caller, not dropped
/// here. A mid-write partial (no trailing `\n`) stays buffered. Empty vec if absent/no new
/// complete line.
fn tail_commands(path: &Path, cursor: &mut u64) -> Vec<String> {
    let Ok(mut f) = fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len <= *cursor {
        return Vec::new();
    }
    f.seek(SeekFrom::Start(*cursor)).expect("seek commands");
    let mut buf = String::new();
    f.read_to_string(&mut buf).expect("read commands");
    let Some(last_nl) = buf.rfind('\n') else {
        return Vec::new(); // no complete line yet
    };
    let consumed = &buf[..=last_nl];
    *cursor += consumed.len() as u64;
    consumed.lines().map(|s| s.to_string()).collect()
}

fn model_screen(app: &mut App) -> Screen {
    app.world_mut()
        .query::<&Dooduel>()
        .single(app.world())
        .expect("model exists")
        .screen
        .clone()
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .filter(|a| a == "--dir")
            .and(std::env::args().nth(2))
            .unwrap_or_else(|| "/tmp/qa-seat-spike".to_string()),
    );
    std::fs::create_dir_all(&dir).expect("create seat dir");

    let mut app = build_app();
    let (image, window, pointer) = spawn_view(&mut app);
    app.finish();
    app.cleanup();
    // Warm up: model + cameras spawn, fonts register, tree builds, reshape settles.
    for _ in 0..90 {
        app.update();
    }

    // Checkpoint 1a — one readback lands real pixels.
    let px = readback_rgba(&mut app, &image);
    let first = &px[0..4];
    let differing = px.chunks_exact(4).filter(|p| *p != first).count();
    assert!(
        differing > 1000,
        "screen.png is not uniform/black (differing px = {differing})"
    );
    save_png(px, &dir.join("screen.png"));

    // Checkpoint 1b — one non-empty snapshot mentions a known Home label.
    let ui = snapshot_md(&mut app);
    assert!(
        ui.contains("Create a room"),
        "ui.md shows the Home CTA. Report:\n{ui}"
    );
    std::fs::write(dir.join("ui.md"), &ui).expect("write ui.md");

    // Checkpoint 1c — one click resolves and lands its Msg. Click "Join a room" →
    // Msg::GoJoin → Screen::Join: PURE reducer navigation, NO net (capture.rs:57-60
    // asserts exactly this). Do NOT click "Create a room": under install_runtime it calls
    // start_connect → NetState::Joining + pending_connect (lib.rs:808-817), which is
    // networked (is_networked, lib.rs:239-244), so WsClientPlugin opens a real socket; with
    // no server that fails (ECONNREFUSED → ConnStatus::Closed → Msg::ConnectFailed → back to
    // Home + toast, net.rs:445,508-515 / transport.rs:325-326 / lib.rs:695-700). The
    // assertion would be racy AND a fully-rendered Home+toast would masquerade as a render
    // failure. GoJoin has none of that — it only sets Screen::Join.
    assert_eq!(model_screen(&mut app), Screen::Home, "starts on Home");
    click_role(&mut app, window, pointer, A11yRole::Button, "Join a room")
        .expect("Join a room is clickable");
    for _ in 0..12 {
        app.update();
    }
    assert_eq!(
        model_screen(&mut app),
        Screen::Join,
        "the click's Msg::GoJoin navigated to the Join screen (pure nav, no server)"
    );
    let post = snapshot_md(&mut app);
    assert!(
        post.contains("Join room"),
        "the Join screen rendered — its 'Join room' CTA is present. Report:\n{post}"
    );
    save_png(readback_rgba(&mut app, &image), &dir.join("screen.png"));
    std::fs::write(dir.join("ui.md"), &post).expect("write ui.md");
    std::fs::write(
        dir.join("driver.log"),
        "consumed: 0 → click Join a room → Ok (screen: Home → Join)\n",
    )
    .expect("write driver.log");

    println!(
        "W0 spike OK — wrote screen.png / ui.md / driver.log to {}",
        dir.display()
    );
}
