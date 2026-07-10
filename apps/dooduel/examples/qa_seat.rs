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

use dooduel::paint::CanvasKind;
use dooduel::{CANVAS_H, CANVAS_W, Dooduel, Msg};

/// The res-Q2 desktop size: renders the full 3-pane layout without clipping. Overridable
/// via `--size WxH` (spec §3.3); the default is the size all the layout reasoning assumes.
const DEFAULT_W: u32 = 1280;
const DEFAULT_H: u32 = 800;

/// Build the real client on a headless render stack + picking (spec §2.1). The
/// primary Window is created by `WindowPlugin`; the two cameras + pointer are spawned
/// in `spawn_view`. Returns the app (not yet `finish`ed).
fn build_app(w: u32, h: u32) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                resolution: bevy::window::WindowResolution::new(w, h),
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
fn spawn_view(app: &mut App, w: u32, h: u32) -> (Handle<Image>, Entity, Entity) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("WindowPlugin created a primary window");

    // The offscreen readback texture (capture.rs:228-231 pattern).
    let mut image = Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None);
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

/// One GPU readback of `image` (a `w`×`h` texture) → tight RGBA bytes (row-padding
/// stripped). Pumps up to 60 frames until `ReadbackComplete` fires (capture.rs:377-416).
fn readback_rgba(app: &mut App, image: &Handle<Image>, w: u32, h: u32) -> Vec<u8> {
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

    let unpadded = (w * 4) as usize;
    let padded = unpadded.div_ceil(256) * 256;
    let rows = h as usize;
    if raw.len() == unpadded * rows {
        raw
    } else {
        let mut out = Vec::with_capacity(unpadded * rows);
        for row in 0..rows {
            let start = row * padded;
            out.extend_from_slice(&raw[start..start + unpadded]);
        }
        out
    }
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
    if let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return false;
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

// ---------------------------------------------------------------------------
// Verbs (spec §3.1/§3.2): click / set_value / stroke / shot / quit.
// ---------------------------------------------------------------------------

/// The Game canvas node's window-space rect (top-left + size), or None if not laid out.
fn game_canvas_rect(app: &mut App) -> Option<(Vec2, Vec2)> {
    app.world_mut()
        .query::<(&CanvasKind, &GlobalTransform, &ResolvedLayout)>()
        .iter(app.world())
        .find(|(k, ..)| **k == CanvasKind::Game)
        .map(|(_, gt, layout)| (gt.translation().truncate(), layout.size))
}

/// Map a canvas coord (0..CANVAS_W × 0..CANVAS_H) to a window-space point — the exact
/// inverse of paint.rs::to_pixel (spec §res-Q4). `+0.5` hits the texel center.
fn canvas_to_window(tl: Vec2, size: Vec2, cx: f32, cy: f32) -> Vec2 {
    Vec2::new(
        tl.x + ((cx + 0.5) / CANVAS_W as f32) * size.x,
        tl.y + ((cy + 0.5) / CANVAS_H as f32) * size.y,
    )
}

fn role_from_str(s: &str) -> Option<A11yRole> {
    match s {
        "Button" => Some(A11yRole::Button),
        "TextInput" => Some(A11yRole::TextInput),
        _ => None,
    }
}

/// Set a text field's value via the probe SetValue channel (spec §res-Q5), then settle.
fn set_value_role(
    app: &mut App,
    role: A11yRole,
    name: Option<&str>,
    text: &str,
) -> Result<(), buiy_core::a11y::ActionError> {
    let node = buiy::probe::get_by_role(app.world_mut(), role, name, None)?;
    buiy::probe::set_value(app.world_mut(), node, text)?;
    app.update();
    Ok(())
}

enum Applied {
    Ok(String),
    Quit,
}

/// Apply one command line. Returns the outcome string (logged) or Quit. A malformed line is
/// a non-fatal BadData outcome (never a panic) — spec §2.3.
fn apply_command(app: &mut App, window: Entity, pointer: Entity, line: &str) -> Applied {
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Applied::Ok(format!("BadData (malformed JSON): {e}")),
    };
    let cmd = v.get("cmd").and_then(|c| c.as_str()).unwrap_or("");
    match cmd {
        "click" => {
            let Some(role) = v
                .get("role")
                .and_then(|r| r.as_str())
                .and_then(role_from_str)
            else {
                return Applied::Ok("BadData: click needs a known role".into());
            };
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
            match click_role(app, window, pointer, role, name) {
                Ok(()) => Applied::Ok(format!("click {name:?} → Ok")),
                Err(e) => Applied::Ok(format!("click {name:?} → {e:?}")),
            }
        }
        "set_value" => {
            let Some(role) = v
                .get("role")
                .and_then(|r| r.as_str())
                .and_then(role_from_str)
            else {
                return Applied::Ok("BadData: set_value needs a known role".into());
            };
            let name = v.get("name").and_then(|n| n.as_str());
            let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("");
            match set_value_role(app, role, name, text) {
                Ok(()) => Applied::Ok(format!("set_value {text:?} → Ok")),
                Err(e) => Applied::Ok(format!("set_value → {e:?}")),
            }
        }
        "stroke" => {
            let Some((tl, size)) = game_canvas_rect(app) else {
                return Applied::Ok("stroke → NotFound: no Game canvas on screen".into());
            };
            let pts: Vec<Vec2> = v
                .get("points")
                .and_then(|p| p.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| {
                            let a = p.as_array()?;
                            let cx = a.first()?.as_f64()? as f32;
                            let cy = a.get(1)?.as_f64()? as f32;
                            Some(canvas_to_window(tl, size, cx, cy))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let path: Vec<Vec2> = if pts.len() == 1 {
                vec![pts[0], pts[0] + Vec2::new(1.0, 0.0)] // 1-point tap → micro-stroke
            } else {
                pts
            };
            if path.len() < 2 {
                return Applied::Ok("stroke → BadData: need ≥1 point".into());
            }
            drive_stroke(app, window, pointer, &path);
            app.update();
            Applied::Ok(format!("stroke ({} pts) → Ok", path.len()))
        }
        "shot" => Applied::Ok("shot → Ok (forced refresh)".into()),
        "quit" => Applied::Quit,
        other => Applied::Ok(format!("BadData: unknown cmd {other:?}")),
    }
}

// ---------------------------------------------------------------------------
// CLI, per-seat env isolation, and the real-time command loop (spec §2.2/§3.3).
// ---------------------------------------------------------------------------

struct Args {
    dir: PathBuf,
    url: String,
    name: Option<String>,
    interval: f32,
    w: u32,
    h: u32,
}

/// Parse a `WxH` size string (e.g. `1280x800`); None if malformed.
fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

fn parse_args() -> Args {
    let mut dir = None;
    let mut url = None;
    let mut name = None;
    let mut interval = 1.0_f32;
    let mut w = DEFAULT_W;
    let mut h = DEFAULT_H;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" => dir = it.next().map(PathBuf::from),
            "--url" => url = it.next(),
            "--name" => name = it.next(),
            "--interval" => interval = it.next().and_then(|s| s.parse().ok()).unwrap_or(1.0),
            "--size" => {
                if let Some((pw, ph)) = it.next().as_deref().and_then(parse_size) {
                    w = pw;
                    h = ph;
                }
            }
            other => eprintln!("qa_seat: ignoring unknown arg {other:?}"),
        }
    }
    let dir = dir.expect("--dir <seat_dir> is required");
    let url = url
        .or_else(|| std::env::var("DOODUEL_SERVER_URL").ok())
        .unwrap_or_else(|| "ws://127.0.0.1:7878".to_string());
    Args {
        dir,
        url,
        name,
        interval,
        w,
        h,
    }
}

/// Seed the player name through the MVU funnel (`Msg::SetName`) so it reaches the connect
/// payload (`connect_intent` reads `Dooduel.player_name`, net.rs:565) AND re-renders the
/// Home name field. Dispatched AFTER warmup: the model entity spawns at `Startup`
/// (buiy_view app.rs:126), so it does not exist before the first `app.update()`; and going
/// through the funnel (not a raw field write) keeps the §7.5 single-writer audit clean.
fn seed_name(app: &mut App, name: &str) {
    let Ok(e) = app
        .world_mut()
        .query_filtered::<Entity, With<Dooduel>>()
        .single(app.world())
    else {
        return;
    };
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<buiy_core::mvu::Envelope<Dooduel>>>()
        .write(buiy_core::mvu::Envelope::user(
            e,
            Msg::SetName(name.to_string()),
        ));
    app.update();
}

fn main() {
    let args = parse_args();
    std::fs::create_dir_all(&args.dir).expect("create seat dir");
    // Per-seat isolation (spec §2.1 / item A): the server URL WsClientPlugin reads, and a
    // private state dir so N seats never race ~/.config/dooduel/state.json.
    // SAFETY: set before any Bevy/App thread spawns (single-threaded here).
    unsafe {
        std::env::set_var("DOODUEL_SERVER_URL", &args.url);
        std::env::set_var("DOODUEL_STATE_DIR", args.dir.join("state"));
    }
    std::fs::create_dir_all(args.dir.join("state")).ok();

    let mut app = build_app(args.w, args.h);
    let (image, window, pointer) = spawn_view(&mut app, args.w, args.h);
    app.finish();
    app.cleanup();
    // Warm up: Startup runs (model + cameras spawn), fonts register, tree builds, reshape
    // settles. The model does not exist before this (Startup), so seed the name after.
    for _ in 0..90 {
        app.update();
    }
    if let Some(n) = &args.name {
        seed_name(&mut app, n);
    }

    let screen_png = args.dir.join("screen.png");
    let ui_md = args.dir.join("ui.md");
    let commands = args.dir.join("commands.jsonl");
    let log = args.dir.join("driver.log");
    let mut cursor: u64 = 0;
    let mut consumed_k: u64 = 0;
    let mut last_refresh = std::time::Instant::now();
    let interval = std::time::Duration::from_secs_f32(args.interval);
    let frame_budget = std::time::Duration::from_millis(16); // ~60 Hz cap

    refresh(&mut app, &image, args.w, args.h, &screen_png, &ui_md);
    append_line(&log, "qa_seat up");
    println!("qa_seat: {} → {}", args.url, args.dir.display());

    loop {
        let t0 = std::time::Instant::now();
        app.update();

        let mut force_refresh = false;
        for line in tail_commands(&commands, &mut cursor) {
            // Every \n-terminated line consumes a K (spec §2.3), so K stays equal to the
            // agent's appended-line count. A blank line is logged-skipped (no app change, no
            // forced refresh); a malformed non-blank line flows through apply_command and
            // logs a BadData outcome — still one K.
            if line.trim().is_empty() {
                append_line(
                    &log,
                    &format!("consumed: {consumed_k} → skipped (blank line)"),
                );
                consumed_k += 1;
                continue;
            }
            match apply_command(&mut app, window, pointer, &line) {
                Applied::Quit => {
                    append_line(&log, &format!("consumed: {consumed_k} → quit"));
                    println!("qa_seat: quit");
                    return;
                }
                Applied::Ok(outcome) => {
                    append_line(&log, &format!("consumed: {consumed_k} → {outcome}"));
                }
            }
            consumed_k += 1;
            force_refresh = true;
        }

        if force_refresh || last_refresh.elapsed() >= interval {
            refresh(&mut app, &image, args.w, args.h, &screen_png, &ui_md);
            last_refresh = std::time::Instant::now();
        }

        // Pace to the frame budget (real-time ticking; readback bursts absorb their own
        // stall — spec §2.2, don't "fix" it).
        if let Some(rem) = frame_budget.checked_sub(t0.elapsed()) {
            std::thread::sleep(rem);
        }
    }
}

/// Readback → screen.png + snapshot → ui.md, both atomic + change-detected.
fn refresh(app: &mut App, image: &Handle<Image>, w: u32, h: u32, screen_png: &Path, ui_md: &Path) {
    let png = {
        let rgba = readback_rgba(app, image, w, h);
        let img = image::RgbaImage::from_raw(w, h, rgba).expect("w*h*4");
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("encode png");
        buf.into_inner()
    };
    atomic_write_if_changed(screen_png, &png);
    atomic_write_if_changed(ui_md, snapshot_md(app).as_bytes());
}
