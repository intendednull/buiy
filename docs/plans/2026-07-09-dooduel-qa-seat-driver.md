# Dooduel QA seat-driver — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to implement this task-by-task. Steps use
> checkbox (`- [ ]`) syntax. **Every gate is a RUN, not a compile** — this driver is an
> integration binary; "it builds" proves nothing. Commit after every task.

**Date:** 2026-07-09 · **Status:** landed · **Revision:** rev-2 (plan review BLOCK verdict
folded — 1 blocker + 1 major + 4 minors; see the change log at the end) · **Spec:**
`docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md` (rev-2.1/active, `dd485aa` + the §2.3
consumed-K clarification)

**Goal:** Build `apps/dooduel/examples/qa_seat.rs` — a dev-tool that runs the real Dooduel
client headless, renders it offscreen to `screen.png`, snapshots the semantic tree to
`ui.md`, and drives it through real widget interactions from a `commands.jsonl` file — so N
LLM agents can visually QA a real networked match.

**Architecture:** The example composes the real app's `install_runtime` plugin set on a
**headless** Bevy render stack (offscreen `Image` target, no winit window), plus the picking
stack (`BuiyHeadlessPlugin` omits it). Two cameras: one targets the Window (satisfies the
picking backend), one targets the Image (readback). A synthetic `PointerId::Mouse` drives
real `bevy_picking` clicks/strokes. A real-time `app.update()` loop interleaves a
`commands.jsonl` tail with a ~1 Hz `screen.png`+`ui.md` refresh.

**Tech Stack:** Rust · Bevy 0.19 (headless `RenderPlugin`, `gpu_readback::Readback`,
`bevy_picking`) · Buiy (`BuiyHeadlessPlugin`, `buiy::probe`, `buiy_verify::pointer`) ·
`dooduel` lib (`install_runtime`, `WsClientPlugin`) · `serde_json`.

---

## Risk-front-loading & wave shape

- **W0 — checkpoint-1 spike (retires C1 before anything is built on it).** The minimal
  example that boots the full composition headless, spawns the two cameras + pointer, lands
  ONE readback + ONE non-empty snapshot, and resolves ONE real click. **No server needed** —
  the click is "Join a room" → `Msg::GoJoin` → `Screen::Join`, PURE reducer navigation with
  no net (`home.rs:99`; `capture.rs:57-60` asserts exactly this). (Do NOT use "Create a room":
  under `install_runtime` it stages a networked connect that `WsClientPlugin` opens and, with
  no server, fails back to Home — the W0 BLOCKER the review caught. `capture.rs` escapes only
  because it uses bare `dooduel::install`, no `WsClientPlugin`.)
- **W1 — the full driver.** File protocol + all verbs + CLI + env + the 1 Hz loop. Gate:
  manual 2-seat run against a real server.
- **W2 — the committed `#[ignore]` GPU-lane smoke test** (`qa_seat_smoke.rs`), subprocess
  per seat, ≥3 concurrent seats, checkpoints 1-6 black-box.
- **W3 — close-out** (spec status, docs index, journal).

## File structure

| File | Responsibility |
|---|---|
| `apps/dooduel/examples/qa_seat.rs` | **Create.** The whole driver: composition, cameras, readback, snapshot, verbs, file protocol, CLI, main loop. Single file (mirrors `capture.rs`'s single-file shape). |
| `apps/dooduel/Cargo.toml` | **Modify.** No change needed for W0/W1 (examples auto-discover; `buiy_verify` dev-dep + lib are already available). W2 may add `serde_json` to `[dev-dependencies]` if the smoke test needs it (it's already a normal dep, so likely not). |
| `apps/dooduel/tests/qa_seat_smoke.rs` | **Create (W2).** The `#[ignore]` GPU-lane integration test. |
| `docs/specs/…-design.md`, `docs/README.md`, `docs/reports/…` | **Modify (W3).** Status flips + journal. |

## Cross-cutting rules (apply to EVERY task)

- **`RUST_MIN_STACK=33554432` on every build/run/test command** (rustc SIGSEGV on the big
  bevy bins otherwise — spec §C2). Set it inline: `RUST_MIN_STACK=33554432 cargo …`.
- **Mechanical gate after each wave** (mirrors CI, headless — the `#[ignore]` smoke never
  runs here):
  ```sh
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  RUST_MIN_STACK=33554432 cargo test -p dooduel --locked
  ```
  All three must be clean before the wave's commit.
- **Run the artifact at every gate.** W0/W1 gates are manual runs whose output you inspect;
  do not mark them done on a green `cargo build`.
- **Commit per task**, local only — **never push**. Descriptive messages.
- Fish is the user's shell; the commands here are bash-compatible (`RUST_MIN_STACK=… cargo`
  works in both). If you hand a command to the user, keep it fish-safe.

---

## Wave 0 — the checkpoint-1 spike

### Task 0.1: Boot the full composition headless, render, snapshot, and resolve one click

**Files:**
- Create: `apps/dooduel/examples/qa_seat.rs`

This task builds the spike-subset of the final file. W1 extends the same file. Do **not**
add the file protocol or verbs yet — only prove the composition + render + pick path.

- [x] **Step 1: Create the file with imports + the composition builder.**

Write `apps/dooduel/examples/qa_seat.rs`:

```rust
//! QA seat-driver (dev tool) — runs the REAL Dooduel client headless, renders it
//! offscreen to `screen.png`, snapshots the semantic tree to `ui.md`, and drives it
//! through real widget interactions from `commands.jsonl`. Spec:
//! `docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md`.
//!
//! Run (needs a real wgpu adapter; no display required):
//!   RUST_MIN_STACK=33554432 cargo run -p dooduel --example qa_seat -- --dir /tmp/qa-seat-1

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::camera::{RenderTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::camera::NormalizedRenderTarget;
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
```

- [x] **Step 2: Add the two-camera + pointer spawn + the readback Image.**

Append to `qa_seat.rs`:

```rust
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
        Camera { order: -1, ..default() },
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
```

- [x] **Step 3: Add the readback burst + the snapshot writer.**

Append to `qa_seat.rs` (the `capture.rs:377-424` readback, adapted to return bytes):

```rust
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
    img.save(path).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// The semantic snapshot (role tree + text/layout dump).
fn snapshot_md(app: &mut App) -> String {
    snapshot_report(app.world_mut())
}
```

- [x] **Step 4: Add the click helper (address by role → real synthetic-pointer click).**

Append (the `gui_networked.rs:277-301` recipe as a reusable fn):

```rust
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
    let e = entity_for_node_id(node).ok_or(buiy_core::a11y::ActionError::NotFound {
        target: node,
    })?;
    let (tl, size) = {
        let w = app.world();
        let tl = w
            .get::<GlobalTransform>(e)
            .expect("laid-out node has GlobalTransform")
            .translation()
            .truncate();
        let size = w.get::<ResolvedLayout>(e).expect("node has ResolvedLayout").size;
        (tl, size)
    };
    let center = tl + size * 0.5;
    drive_stroke(app, window, pointer, &[center, center + Vec2::new(1.0, 0.0)]);
    Ok(())
}
```

> Note: confirm the exact `ActionError` path — `buiy_core::a11y::ActionError` (re-exported;
> `contract.rs:41`). If the `NotFound { target }` field name differs, match the enum in
> `crates/buiy_core/src/a11y/contract.rs:41-82`. `get_by_role`'s `?` already yields the
> right error type, so the `.ok_or(...)` arm is the only hand-built variant — if awkward,
> replace it with `.expect("node maps to an entity")` (a resolved node always maps).

- [x] **Step 5: Add the spike `main` — boot, readback, snapshot, one click, assert.**

Append:

```rust
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

    println!("W0 spike OK — wrote screen.png / ui.md / driver.log to {}", dir.display());
}
```

- [x] **Step 6: Build the example (compile check only — not the gate).**

Run:
```sh
RUST_MIN_STACK=33554432 cargo build -p dooduel --example qa_seat --locked
```
Expected: `Finished`. If `single(...)` / `Camera::order` / `ActionError` paths differ on
this Bevy/Buiy version, fix against the cited source files (do not stub).

- [x] **Step 7: RUN the spike (THE W0 GATE — needs the real adapter, no server).**

Run:
```sh
RUST_MIN_STACK=33554432 cargo run -p dooduel --example qa_seat --locked -- --dir /tmp/qa-w0
```
Expected stdout: `W0 spike OK — wrote screen.png / ui.md / driver.log to /tmp/qa-w0`
(no panic).

- [x] **Step 8: Inspect the artifacts (the real proof — C1 retired).**

Run:
```sh
file /tmp/qa-w0/screen.png                       # → PNG image data, 1280 x 800
grep -c "Join room" /tmp/qa-w0/ui.md             # → ≥1 (post-click ui.md is the Join screen)
cat /tmp/qa-w0/driver.log                        # → the click resolved, Home → Join
```
Then **open `/tmp/qa-w0/screen.png` and look at it** — it must show the real rendered **Join**
screen (the room-code field + the "Join room" button), not a black/blank frame. (The final
`ui.md`/`screen.png` are the post-click Join screen; the "Create a room" assertion ran against
the pre-click Home snapshot in memory.) **Diagnostic ladder** if it's wrong:
- **Black/blank PNG** → the C1 two-camera coexistence failed. STOP + report (spec §res-Q1
  fallback becomes relevant); do not paper over it.
- **A rendered Home screen with a toast** → you accidentally took the networked path (clicked
  "Create a room" / `Msg::CreateRoom`, not "Join a room" / `Msg::GoJoin`). Render + picking are
  FINE — fix the click target, this is not a C1 failure.
- **Join screen rendered but the model is still `Screen::Home`** → the click did not resolve;
  the picking path (camera resolution / pointer target / hit-test) is the problem, not render.

- [x] **Step 9: Mechanical gate + commit.**

Run the full mechanical gate (cross-cutting rules). Then:
```sh
git add apps/dooduel/examples/qa_seat.rs
git commit -m "feat(dooduel): W0 qa_seat spike — headless render + pick, retires C1

Boots the real client on a headless render stack + picking (two cameras: Window
for picking, Image for readback), lands one GPU readback + one semantic snapshot,
and resolves one real synthetic-pointer click (Home → Join via Msg::GoJoin — pure
navigation, no server). Spec docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md
§res-Q1/§6 checkpoint 1."
```

---

## Wave 1 — the full driver

Wave 1 grows `qa_seat.rs` into the real driver: the file protocol, all verbs, CLI flags,
env isolation, and the real-time command loop. The W0 spike `main` is replaced by the loop.

### Task 1.1: File protocol — atomic writes, byte-cursor tail, malformed-skip

**Files:**
- Modify: `apps/dooduel/examples/qa_seat.rs`

- [x] **Step 1: Add the atomic writer + change-detection.**

Append to `qa_seat.rs`:

```rust
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

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
```

- [x] **Step 2: Add the byte-cursor command tail (only complete `\n`-terminated lines).**

Append:

```rust
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
    consumed
        .lines()
        .map(|s| s.to_string())
        .collect()
}
```

- [x] **Step 3: Commit.**
```sh
git add apps/dooduel/examples/qa_seat.rs
git commit -m "feat(dooduel): qa_seat file protocol — atomic writes + byte-cursor tail"
```

### Task 1.2: Verbs — click / set_value / stroke (with the res-Q4 mapping) / shot / quit

**Files:**
- Modify: `apps/dooduel/examples/qa_seat.rs`

- [x] **Step 1: Add the canvas coordinate mapping (spec §res-Q4).**

Append (imports: add `use dooduel::paint::CanvasKind;` and `use dooduel::{CANVAS_W, CANVAS_H};`
to the top of the file):

```rust
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
```

- [x] **Step 2: Add role parsing + the `set_value` helper.**

Append:

```rust
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
```

- [x] **Step 3: Add the command dispatcher.**

Append (uses `serde_json` — already a `dooduel` dependency, so available to the example):

```rust
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
            let Some(role) = v.get("role").and_then(|r| r.as_str()).and_then(role_from_str)
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
            let Some(role) = v.get("role").and_then(|r| r.as_str()).and_then(role_from_str)
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
```

- [x] **Step 4: Commit.**
```sh
git add apps/dooduel/examples/qa_seat.rs
git commit -m "feat(dooduel): qa_seat verbs — click/set_value/stroke/shot/quit + canvas mapping"
```

### Task 1.3: CLI, env isolation, and the real-time command loop

**Files:**
- Modify: `apps/dooduel/examples/qa_seat.rs`

- [x] **Step 1: Replace the spike `main` with a CLI parser + env setup + the loop.**

Delete the W0 `fn main()` and the W0-only `model_screen` if unused, and replace with:

```rust
struct Args {
    dir: PathBuf,
    url: String,
    name: Option<String>,
    interval: f32,
}

fn parse_args() -> Args {
    let mut dir = None;
    let mut url = None;
    let mut name = None;
    let mut interval = 1.0_f32;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" => dir = it.next().map(PathBuf::from),
            "--url" => url = it.next(),
            "--name" => name = it.next(),
            "--interval" => interval = it.next().and_then(|s| s.parse().ok()).unwrap_or(1.0),
            other => eprintln!("qa_seat: ignoring unknown arg {other:?}"),
        }
    }
    let dir = dir.expect("--dir <seat_dir> is required");
    let url = url
        .or_else(|| std::env::var("DOODUEL_SERVER_URL").ok())
        .unwrap_or_else(|| "ws://127.0.0.1:7878".to_string());
    Args { dir, url, name, interval }
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

    let mut app = build_app();
    // Pre-seed the player name (the reducer reads Dooduel.player_name; set it before the
    // view builds so the Home name field shows it). Optional convenience.
    if let Some(n) = &args.name {
        if let Ok(mut m) = app
            .world_mut()
            .query::<&mut Dooduel>()
            .single_mut(app.world_mut())
        {
            m.player_name = n.clone();
        }
    }
    let (image, window, pointer) = spawn_view(&mut app);
    app.finish();
    app.cleanup();
    for _ in 0..90 {
        app.update();
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

    refresh(&mut app, &image, &screen_png, &ui_md);
    append_line(&log, "qa_seat up");
    println!("qa_seat: {} → {}", args.url, args.dir.display());

    loop {
        let t0 = std::time::Instant::now();
        app.update();

        let mut force_refresh = false;
        for line in tail_commands(&commands, &mut cursor) {
            // Every \n-terminated line consumes a K (spec §2.3), so K stays equal to the
            // agent's appended-line count. A blank line is logged-skipped (no app change,
            // no forced refresh); a malformed non-blank line flows through apply_command
            // and logs a BadData outcome — still one K.
            if line.trim().is_empty() {
                append_line(&log, &format!("consumed: {consumed_k} → skipped (blank line)"));
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
            refresh(&mut app, &image, &screen_png, &ui_md);
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
fn refresh(app: &mut App, image: &Handle<Image>, screen_png: &Path, ui_md: &Path) {
    let png = {
        let rgba = readback_rgba(app, image);
        let img = image::RgbaImage::from_raw(W, H, rgba).expect("W*H*4");
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).expect("encode png");
        buf.into_inner()
    };
    atomic_write_if_changed(screen_png, &png);
    atomic_write_if_changed(ui_md, snapshot_md(app).as_bytes());
}
```

> Note: `unsafe { set_var }` — Rust 2024's `set_var` is unsafe; this is called once at
> startup before threads spawn, which is sound. If the edition here still has safe `set_var`,
> drop the `unsafe`. Also delete the now-unused `save_png` if the loop's `refresh` replaces
> it (keep `readback_rgba`).

- [x] **Step 2: Build + fmt/clippy.**
```sh
RUST_MIN_STACK=33554432 cargo build -p dooduel --example qa_seat --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```
Expected: clean. Fix any warning (unused imports from the W0→W1 refactor are common).

- [x] **Step 3: Commit.**
```sh
git add apps/dooduel/examples/qa_seat.rs
git commit -m "feat(dooduel): qa_seat CLI + real-time command loop + per-seat env isolation"
```

### Task 1.4: Manual 2-seat gate (spec §6 checkpoints 2-6, by hand)

**Files:** none (verification only). This is a RUN gate — do not skip.

> **RESULT (2026-07-10, run on `/tmp/qa-w1`, seats Hosta/Guessa, room `39R5OM`, config
> rounds=1/draw=150/pick=30/reveal=12/hints=2/bots=false).** The gate RAN and did its job —
> it caught two real bugs:
>
> 1. **DRIVER bug FOUND + FIXED (the inherited "settle fix" was a misdiagnosis).** The
>    batched Join flow (`click "Join a room"` → `set_value` code → `click "Join room"`) left
>    the code field empty and SubmitJoin failed validation. Root cause (verified by isolating
>    a standalone set_value + standalone Join-room click, which ALSO failed): `probe::set_value`
>    mutates the editor's `TextEditState` + the a11y-tree value but **never emits `TextChanged`**,
>    so `buiy_view::route_text_input` never fires the field's `on_input` and the edit never folds
>    into the MVU model — the model `SubmitJoin`/`SubmitGuess` read stays empty. `SETTLE_FRAMES`
>    could never fix this (no amount of settling emits the missing signal). Fixed in
>    `set_value_role` by re-emitting the exact `TextChanged` the real keyboard editor emits
>    (`input.rs:741`); re-ran → the batched Join now succeeds (both seats show the 2-player
>    roster). `SETTLE_FRAMES` retained (it serves screen-readiness between batched nav→act
>    commands) with its rationale corrected. **Spec §res-Q5's claim that `set_value` alone
>    "fires the field's on_input binding" is inaccurate — see the W1 report.**
>
> 2. **APP bug FOUND (reported, NOT fixed — production code).** In the in-game screen the
>    floating "Light/Dark" theme toggle (`@1172,730 88x50`) **occludes the chat "Send" button**
>    (`@1194,736 41x56`): Send's resolved center `(1214,764)` is inside the toggle's rect, so
>    the synthetic pointer activates the toggle (it flipped `Light→Dark [focused]`) instead of
>    submitting the guess. This is the occluded-hit / pick≠paint class the driver exists to
>    catch. **It blocks the guess-submission checkpoint at the res-Q2 default 1280×800** (also a
>    W2 checkpoint-6 concern). Guess *value-entry* works (the fixed `set_value` landed the text);
>    only the Send *click* is blocked.
>
> Checkpoints verified: create ✓, join ✓ (after fix), start ✓, pick ✓ (UMBRELLA), draw ✓ (a
> red X rendered on the drawer's canvas AND op-log-synced identically to the guesser's canvas;
> guesser's word slots correctly blank — redaction holds). Guess submission ✗ (blocked by #2).
> Both turns hit the 150s Reveal timeout during the root-cause investigation (podium 0-0) — a
> reminder the real-time server needs the runbook's widened timers for slower LLM-seat cycles.
>
> **RE-GATE (2026-07-10, post-fixes — the previously-blocked guess→chat→score checkpoint is now
> VERIFIED LIVE END-TO-END).** Both bugs above are fixed on this branch — the theme-toggle
> occlusion (`e891000`, suppress the floating toggle in-game) and the `probe::set_value` missing
> `TextChanged` (framework `23540a0`; the driver's local workaround dropped in `7931f22`). A
> fresh 2-seat run (seats Hosta/Guessa, same config rounds=1/draw=150/pick=30/reveal=12/hints=2/
> bots=false) drove the full match through scoring and the podium:
> - **Theme toggle suppressed in-game (both seats):** the in-game `ui.md`/`screen.png` carry NO
>   `Button "Light"`/`"Dark"` (Home/Podium still do); `Send` is unoccluded at `@1194,740`.
> - **Wrong guess routes to chat (room `ADEE2T`):** seat-1 `set_value "banana"` → `Send` →
>   `text="Guessa: banana"` appears in BOTH seats' `ui.md` and seat-1's `TextInput` clears to
>   `[value=""]`. This alone proves the two fixes work together (Send hits Send, not the toggle;
>   the guess text folds into the model and submits).
> - **Correct guess scores + spoiler-safe (room `IYDCA3`, word `BUTTERFLY`):** seat-1 guessed it
>   → chat `"Guessa guessed the word!"` (the word is NOT leaked to observers), reveal
>   `The word was "BUTTERFLY"`, server `seat 1 guessed correctly (+182)` → the turn ended early.
>   Scoreboard: Guessa **182**, Hosta **100** (drawer credit). Redaction held both directions
>   (drawer sees the letters; guesser sees size-0 blanks), including after the seat/role swap in
>   turn 2 (Guessa draws `GUITAR`, Hosta guesses it correctly). **Match podium (both seats +
>   server agree): `Hosta 387 / Guessa 282 — "Hosta wins!"`** The Reveal auto-advanced to the
>   podium after `reveal_seconds`; the `Continue` button is present on the mid-turn reveal card.
> - **QA-driver robustness note (harness, NOT a product bug):** `set_value` immediately followed
>   by `Send` **in the same `commands.jsonl` batch** can submit an EMPTY guess — the
>   `TextChanged`→`on_input`→MVU-model fold needs a beat to settle before the `Send` reducer reads
>   the field, and `set_value_role`'s single `app.update()` sometimes isn't enough (it landed for
>   `banana` but not for a same-batch `butterfly`; separating the two commands, or verifying
>   `[value=…]` before `Send`, is reliable). Worth baking into the W2 smoke / runbook: emit
>   `set_value` and `Send` as separate settled steps. (A real human types over many frames then
>   clicks, so this never bites the product.) Evidence: `/tmp/qa-regate/` (per-seat `ui.md`/
>   `driver.log`, `server.stderr.log`, `evidence-seat{0,1}-{score,podium}.png`).

- [x] **Step 1: Write the wide-timer server config.**

Create `/tmp/qa-server.toml` (agents are slow — spec §6 operational dependency):
```toml
[room]
rounds = 1
draw_seconds = 150
pick_seconds = 30
reveal_seconds = 12
hints = 2
bots = false
```

- [x] **Step 2: Start the server (terminal 1).**
```sh
RUST_MIN_STACK=33554432 cargo run -p dooduel_server --locked -- --port 7878 --config /tmp/qa-server.toml
```
Expected: `LISTENING port=7878` on stdout; per-turn transcript on stderr.

- [x] **Step 3: Start two seats (terminals 2 and 3).**
```sh
# terminal 2 (host):
RUST_MIN_STACK=33554432 cargo run -p dooduel --example qa_seat --locked -- \
    --dir /tmp/qa-host --url ws://127.0.0.1:7878 --name Host
# terminal 3 (guesser):
RUST_MIN_STACK=33554432 cargo run -p dooduel --example qa_seat --locked -- \
    --dir /tmp/qa-p2 --url ws://127.0.0.1:7878 --name Priya
```

- [x] **Step 4: Drive the host: create a room, read the code.**
```sh
echo '{"cmd":"click","role":"Button","name":"Create a room"}' >> /tmp/qa-host/commands.jsonl
sleep 3
grep -Eo 'text="[A-Z0-9]{6}"' /tmp/qa-host/ui.md   # → the room code, e.g. text="7XQ2KP"
cp /tmp/qa-host/ui.md /tmp/qa-lobby-ui.md          # snapshot for the W2 calibration samples (step 8)
```
Open `/tmp/qa-host/screen.png` — the Lobby with the code should be visible.

- [x] **Step 5: Drive the guesser: join by the corrected flow.**
Substitute the code from step 4 for `CODE`:
```sh
printf '%s\n' \
  '{"cmd":"click","role":"Button","name":"Join a room"}' \
  '{"cmd":"set_value","role":"TextInput","text":"CODE"}' \
  '{"cmd":"click","role":"Button","name":"Join room"}' >> /tmp/qa-p2/commands.jsonl
sleep 3
grep -c "Priya" /tmp/qa-host/ui.md    # → host sees the 2nd player in its roster
```

- [x] **Step 6: Start the match, pick a word, draw, guess.**
```sh
echo '{"cmd":"click","role":"Button","name":"▶ Start game"}' >> /tmp/qa-host/commands.jsonl
sleep 3
# The host is the first drawer in Picking — the pick overlay shows the word choices now.
cp /tmp/qa-host/ui.md /tmp/qa-pick-ui.md            # snapshot for the W2 calibration samples (step 8)
grep -Eo 'Button "[A-Z]{2,}"' /tmp/qa-host/ui.md    # → the UPPERCASE word choices; pick one below:
echo '{"cmd":"click","role":"Button","name":"ROBOT"}' >> /tmp/qa-host/commands.jsonl   # UPPERCASE, from the grep
sleep 2
printf '%s\n' \
  '{"cmd":"click","role":"Button","name":"Brush"}' \
  '{"cmd":"click","role":"Button","name":"Color 3"}' \
  '{"cmd":"click","role":"Button","name":"Brush size 6"}' \
  '{"cmd":"stroke","points":[[120,90],[300,110],[480,300]]}' >> /tmp/qa-host/commands.jsonl
sleep 2
printf '%s\n' \
  '{"cmd":"set_value","role":"TextInput","text":"robot"}' \
  '{"cmd":"click","role":"Button","name":"Send"}' >> /tmp/qa-p2/commands.jsonl
sleep 2
```

- [x] **Step 7: VERIFY (the gate).**
- Open `/tmp/qa-host/screen.png` — real ink on the canvas.
- `grep -i robot /tmp/qa-p2/ui.md` — the guess shows in the chat.
- `tail /tmp/qa-host/driver.log` + `/tmp/qa-p2/driver.log` — every command shows
  `consumed: K → … Ok` (no unexpected `NotFound`).
If any button read `NotFound`, cross-check the exact label against `ui.md` (case-sensitive —
spec §3.1) before assuming a driver bug. Quit both: `echo '{"cmd":"quit"}' >> …/commands.jsonl`.

- [x] **Step 8: Save the W2 parser-calibration samples (HARD GATE — W2 does not start without this).**
W2's `room_code`/`first_word_choice` parsers must match `snapshot_report`'s EXACT line format;
this run produced the only real `ui.md`s to calibrate them. Extract the two load-bearing lines
into a committed evidence file W2's Task 2.1 step 2 reads before writing the parsers:
```sh
mkdir -p docs/reports/2026-07-09-qa-seat-driver-assets
{
  echo "# room-code line (from the Lobby ui.md):"
  grep -E 'text="[A-Z0-9]{6}"' /tmp/qa-lobby-ui.md | head -1
  echo "# word-choice Button line (from the Picking-overlay ui.md):"
  grep -E 'Button "[A-Z]{2,}"' /tmp/qa-pick-ui.md | head -1
} | tee docs/reports/2026-07-09-qa-seat-driver-assets/ui-samples.txt
```
Both greps MUST return a line. If either is empty, the report's format differs from the parser
assumption — open the snapshot, copy the real line verbatim into `ui-samples.txt`, and note the
actual shape; W2's parsers key off THESE exact strings (indentation, quoting).

- [x] **Step 9: Commit the calibration samples (+ any fix step 7 surfaced).**
```sh
git add docs/reports/2026-07-09-qa-seat-driver-assets/ui-samples.txt
git commit -m "test(dooduel): W1 manual-gate ui.md calibration samples for the W2 parsers"
```
If step 7 surfaced a driver bug, commit that fix too, with a message naming the bug the manual
run caught.

---

## Wave 2 — the committed `#[ignore]` GPU-lane smoke test

### Task 2.1: `qa_seat_smoke.rs` — self-spawn server + N seats, checkpoints 1-6

**Files:**
- Create: `apps/dooduel/tests/qa_seat_smoke.rs`

The test is `#[ignore]` (needs a real adapter + spawns processes), so it compiles under
`--all-targets` but never runs in the headless gate. It spawns the real `dooduel_server` +
≥3 `qa_seat` example processes and drives them black-box through the file protocol.

> **RESULT (W2, 2026-07-10 — landed green, commit `610a5b3`).** The committed `#[ignore]`
> GPU-lane smoke (`apps/dooduel/tests/qa_seat_smoke.rs`) runs all six checkpoints across **3
> concurrent subprocess seats** on the AMD RX 6700 XT / RADV host and passes. Checkpoints 5–6
> were **kept committed** (not demoted to the manual gate): the canvas-region ink diff
> (`wait_canvas_changed`, `HEADER_SKIP_PX = 220` calibrated against a real drawing-phase
> `screen.png`) and the guesser's guess both prove cleanly cross-process. Two amendments the
> run required are recorded in the test: **AMENDMENT 1** — `set_value` and the following
> submit-click must be **separate settled steps** (each gated on its `consumed: K` ack + a
> `value="…"` on-screen check), never one drain batch, or the guess can submit empty before it
> folds; **AMENDMENT 2** — the `HEADER_SKIP_PX` crop was re-measured against this smoke's own
> screenshot. **Checkpoint 6 (a guesser's `set_value` into the rebuilding in-game chat field
> reaching chat) only passes because of Track 3** — the W2 smoke is exactly what surfaced the
> controlled-input clobber (`d344b1e`), fixed in **`e81b91f`** (`PendingProgrammaticEdit`
> marker); without it the `wait_value` fold never lands on the countdown-rebuilt screen. Track 2
> (`23540a0`, `set_value` emits `TextChanged`) is the prerequisite the earlier checkpoints
> depend on.

- [x] **Step 1: Test scaffolding — binary location + server spawn + seat spawn + file poll.**

Create `apps/dooduel/tests/qa_seat_smoke.rs`:

```rust
//! `#[ignore]` GPU-lane smoke for the qa_seat driver (spec §6). Self-spawns dooduel_server +
//! ≥3 qa_seat example processes; drives create→join→draw→guess through the file protocol.
//! Run: RUST_MIN_STACK=33554432 cargo test -p dooduel --test qa_seat_smoke -- --ignored --test-threads=1

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// `.../target/<profile>` — parent of both `dooduel_server` and `examples/qa_seat`. The test
/// binary lives in `.../target/<profile>/deps/`, so pop twice. (apps/dooduel's tests have no
/// CARGO_BIN_EXE_dooduel_server — that's only set in dooduel_server's own package.)
fn target_dir() -> PathBuf {
    let mut p = std::env::current_exe().expect("current_exe");
    p.pop(); // test bin name
    if p.file_name().is_some_and(|n| n == "deps") {
        p.pop(); // 'deps'
    }
    p
}
fn server_bin() -> PathBuf {
    target_dir().join("dooduel_server")
}
fn qa_seat_bin() -> PathBuf {
    target_dir().join("examples").join("qa_seat")
}

struct Server {
    child: Child,
    port: u16,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn spawn_server(config: &std::path::Path) -> Server {
    let mut child = Command::new(server_bin())
        .args(["--port", "0", "--config"])
        .arg(config)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn dooduel_server (build it: cargo build -p dooduel_server)");
    let stdout = child.stdout.take().expect("server stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(p) = line.strip_prefix("LISTENING port=") {
                let _ = tx.send(p.trim().parse::<u16>().expect("numeric port"));
                break;
            }
        }
    });
    let port = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("server printed LISTENING within 30s");
    Server { child, port }
}

struct Seat {
    child: Child,
    dir: PathBuf,
}
impl Drop for Seat {
    fn drop(&mut self) {
        // Ask it to quit, then kill.
        let _ = append_cmd(&self.dir, r#"{"cmd":"quit"}"#);
        std::thread::sleep(Duration::from_millis(300));
        let _ = self.child.kill();
    }
}

fn spawn_seat(port: u16, name: &str, root: &std::path::Path) -> Seat {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("seat dir");
    let child = Command::new(qa_seat_bin())
        .args(["--dir"])
        .arg(&dir)
        .args(["--url", &format!("ws://127.0.0.1:{port}"), "--name", name])
        .env("RUST_MIN_STACK", "33554432")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn qa_seat (build it: cargo build -p dooduel --example qa_seat)");
    Seat { child, dir }
}

fn append_cmd(dir: &std::path::Path, line: &str) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("commands.jsonl"))?;
    writeln!(f, "{line}")
}

/// Poll `<dir>/ui.md` until it contains `needle` (or deadline). Returns the matched ui.md.
fn wait_ui_contains(dir: &std::path::Path, needle: &str, secs: u64) -> String {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(s) = std::fs::read_to_string(dir.join("ui.md")) {
            if s.contains(needle) {
                return s;
            }
        }
        if Instant::now() > deadline {
            let got = std::fs::read_to_string(dir.join("ui.md")).unwrap_or_default();
            panic!("{:?} ui.md never contained {needle:?} within {secs}s. Got:\n{got}", dir);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
```

- [x] **Step 2: Write the config-file helper + the test body (checkpoints 1-6, 3 seats).**

Append:

```rust
fn write_config(root: &std::path::Path) -> PathBuf {
    let p = root.join("qa-server.toml");
    std::fs::write(
        &p,
        "[room]\nrounds = 1\ndraw_seconds = 150\npick_seconds = 30\nreveal_seconds = 12\nhints = 2\nbots = false\n",
    )
    .expect("write config");
    p
}

/// Parse the 6-char room code out of a Lobby ui.md text section (`text="ABC123"`).
fn room_code(ui: &str) -> String {
    for line in ui.lines() {
        if let Some(rest) = line.split("text=\"").nth(1) {
            let code: String = rest.chars().take_while(|c| *c != '"').collect();
            if code.len() == 6 && code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                return code;
            }
        }
    }
    panic!("no 6-char room code in ui.md:\n{ui}");
}

#[test]
#[ignore = "GPU-lane: needs a real wgpu adapter + spawns processes"]
fn three_seats_create_join_draw_guess() {
    let root = std::env::temp_dir().join(format!("qa-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    // Seat dirs + logs live here; removed on SUCCESS at the end, KEPT on failure (a panic
    // unwinds before the cleanup) for post-mortem. Print the path so a failure is findable.
    eprintln!(
        "qa_seat_smoke: artifacts under {} (removed on success, kept on failure)",
        root.display()
    );
    let cfg = write_config(&root);
    let server = spawn_server(&cfg);

    // Checkpoint 1 is implicit: if a seat boots the composition + renders, its ui.md appears.
    let host = spawn_seat(server.port, "Host", &root);
    let p2 = spawn_seat(server.port, "Priya", &root);
    let p3 = spawn_seat(server.port, "Theo", &root);
    // Each seat wrote a first ui.md (proves boot + snapshot).
    wait_ui_contains(&host.dir, "Create a room", 60);
    wait_ui_contains(&p2.dir, "Create a room", 60);
    wait_ui_contains(&p3.dir, "Create a room", 60);

    // Checkpoint 2 — host creates; read the code.
    append_cmd(&host.dir, r#"{"cmd":"click","role":"Button","name":"Create a room"}"#).unwrap();
    let ui = wait_ui_contains(&host.dir, "Copy", 30); // the Lobby's Copy button
    let code = room_code(&ui);

    // Checkpoint 3 — p2 + p3 join by the corrected flow.
    for seat in [&p2, &p3] {
        append_cmd(&seat.dir, r#"{"cmd":"click","role":"Button","name":"Join a room"}"#).unwrap();
        append_cmd(&seat.dir, &format!(r#"{{"cmd":"set_value","role":"TextInput","text":"{code}"}}"#)).unwrap();
        append_cmd(&seat.dir, r#"{"cmd":"click","role":"Button","name":"Join room"}"#).unwrap();
    }
    wait_ui_contains(&host.dir, "Priya", 30);
    wait_ui_contains(&host.dir, "Theo", 30);

    // Checkpoint 4 — start; host is the drawer in Picking. Word choices are the UPPERCASE
    // words; read one from the pick overlay's ui.md and click it.
    append_cmd(&host.dir, r#"{"cmd":"click","role":"Button","name":"▶ Start game"}"#).unwrap();
    let pick_ui = wait_ui_contains(&host.dir, "Pick a word", 30);
    let word = first_word_choice(&pick_ui);
    append_cmd(&host.dir, &format!(r#"{{"cmd":"click","role":"Button","name":"{word}"}}"#)).unwrap();

    // Checkpoint 5 — select a label-only swatch/size (exercises the .label()-derived
    // accessible-name path), let it settle, THEN capture the pre-stroke frame, THEN stroke.
    // Capturing `before` AFTER the toolbar selection means the only canvas-region change vs.
    // `after` is the stroke itself (the countdown repaint is cropped out — see
    // wait_canvas_changed).
    for c in [
        r#"{"cmd":"click","role":"Button","name":"Brush"}"#,
        r#"{"cmd":"click","role":"Button","name":"Color 3"}"#,
        r#"{"cmd":"click","role":"Button","name":"Brush size 6"}"#,
    ] {
        append_cmd(&host.dir, c).unwrap();
    }
    std::thread::sleep(Duration::from_secs(2)); // let the selection settle into screen.png
    let before = std::fs::read(host.dir.join("screen.png")).expect("pre-stroke screen.png");
    append_cmd(&host.dir, r#"{"cmd":"stroke","points":[[120,90],[300,110],[480,300]]}"#).unwrap();
    wait_canvas_changed(&host.dir, &before, 30);

    // Checkpoint 6 — a guesser guesses; the guess reaches its own chat.
    append_cmd(&p2.dir, &format!(r#"{{"cmd":"set_value","role":"TextInput","text":"{}"}}"#, word.to_lowercase())).unwrap();
    append_cmd(&p2.dir, r#"{"cmd":"click","role":"Button","name":"Send"}"#).unwrap();
    // A correct guess folds a "guessed" chat/roster change; poll for the word or a correct marker.
    wait_ui_contains(&p2.dir, &word.to_lowercase(), 30);

    drop((host, p2, p3, server)); // Drop order kills seats then server.
    // Success only: remove the artifacts. A panic above unwinds past this, keeping them.
    std::fs::remove_dir_all(&root).ok();
}

/// The first word-choice button label (UPPERCASE) from a pick-overlay ui.md.
fn first_word_choice(ui: &str) -> String {
    // Word-choice buttons render their UPPERCASE label as a Button node in the role tree.
    for line in ui.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Button \"") {
            let label: String = rest.chars().take_while(|c| *c != '"').collect();
            if label.len() >= 2 && label.chars().all(|c| c.is_ascii_uppercase()) {
                return label;
            }
        }
    }
    panic!("no UPPERCASE word-choice Button in pick ui.md:\n{ui}");
}

/// The header band (top-bar + header card) where the ~1 Hz countdown + drawer-progress
/// repaint live (`in_game.rs` top_bar/header_card; the timer number at `in_game.rs:392`).
/// At the FIXED 1280×800 desktop layout the canvas + toolbar sit BELOW this, so cropping it
/// out isolates the stroke from the timer confound. Deterministic at a fixed window; if the
/// header grows, bump this (calibrate against the W1 `screen.png`).
const HEADER_SKIP_PX: u32 = 220;

/// Poll until the CANVAS REGION of screen.png differs from `before` — decoding both PNGs and
/// comparing only rows `[HEADER_SKIP_PX..H]`, so the ~1 Hz countdown repaint in the header
/// (`in_game.rs:392`, `text!("{}", secs)`) can't masquerade as ink. A raw PNG byte-diff (a
/// naive `now != before`) is CONFOUNDED: the whole file changes every second regardless of
/// the stroke — the MAJOR the review caught. Requires the `image` crate (a `dooduel` dep).
fn wait_canvas_changed(dir: &std::path::Path, before: &[u8], secs: u64) {
    let base = image::load_from_memory(before)
        .expect("decode pre-stroke png")
        .to_rgba8();
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Ok(bytes) = std::fs::read(dir.join("screen.png")) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let now = img.to_rgba8();
                if now.dimensions() == base.dimensions() {
                    let (w, h) = now.dimensions();
                    let mut diff = 0u32;
                    for y in HEADER_SKIP_PX..h {
                        for x in 0..w {
                            if now.get_pixel(x, y) != base.get_pixel(x, y) {
                                diff += 1;
                            }
                        }
                    }
                    if diff > 200 {
                        return; // the stroke inked the canvas region (timer band excluded)
                    }
                }
            }
        }
        if Instant::now() > deadline {
            panic!(
                "{:?} canvas region (rows {HEADER_SKIP_PX}..) never changed after the stroke within {secs}s",
                dir
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
```

> **Why not the reviewer's "parse the canvas rect from ui.md"?** Verified against code: the
> game canvas is a `Kind::Raster` and a NON-pressable raster gets **no** `A11yRole`
> (`reconcile.rs:281-286` — only a *pressable* raster becomes activatable), so it's absent
> from the role tree; and with no `Text`/`A11yLabel` the report's text section skips it
> (`report.rs:233`, `(None, None) => continue`). So the canvas rect is NOT in `ui.md`. The
> header-crop above achieves the same intent (exclude the sole ~1 Hz repainter) without a
> new driver output. If a more precise crop is ever needed, the driver could emit the rect
> (it already computes `game_canvas_rect`) to a sidecar `meta.json` — deferred (YAGNI).

> **GATE — do NOT write these parsers without the real samples.** `first_word_choice` /
> `room_code` key off `snapshot_report`'s exact line formats (`report.rs:127-201` role lines
> `Role "name" …`; text lines `size=… text="…"`). Before writing them, **paste the two real
> lines saved by Task 1.4 step 8** (the room-code text line + a word-choice Button line) into
> this test as reference comments, and shape the parsing (indentation, quoting) to match them
> exactly. W1's manual gate runs first precisely to produce these calibration samples — W2
> does not start until `<evidence>/ui-samples.txt` exists.

- [x] **Step 3: Build the test (compile under --all-targets) + verify it's ignored.**
```sh
RUST_MIN_STACK=33554432 cargo test -p dooduel --test qa_seat_smoke --locked
```
Expected: compiles; runs 0 tests (the one test is `#[ignore]`d). This proves it stays out of
the headless gate.

- [x] **Step 4: RUN it on this GPU host (THE W2 GATE).**
```sh
RUST_MIN_STACK=33554432 cargo build -p dooduel_server --locked
RUST_MIN_STACK=33554432 cargo build -p dooduel --example qa_seat --locked
RUST_MIN_STACK=33554432 cargo test -p dooduel --test qa_seat_smoke --locked -- --ignored --test-threads=1
```
Expected: `test three_seats_create_join_draw_guess ... ok`. If it hangs at a
`wait_ui_contains`, inspect the seat's `driver.log`/`ui.md` under `$TMPDIR/qa-smoke-*` — the
panic message prints the last `ui.md`.

> **Checkpoints 5-6 — sanctioned demotion covers WEAKNESS *and* flakiness, never silent.**
> These two (draw + guess through 3 subprocess seats over real sockets) are both the most
> timing-sensitive AND the hardest to prove cleanly cross-process. The **human-grade** proof
> of the stroke is the W1 manual gate's eyeball ink inspection (Task 1.4 step 7); this
> committed check is the automatable proxy — the cropped canvas-region diff
> (`wait_canvas_changed`), NOT a raw byte-diff (which the countdown confounds). If **either**
> the proxy proves too weak (e.g. `HEADER_SKIP_PX` mis-calibrated so the timer leaks in, or
> the threshold is wrong) **or** the 3-seat flow proves flaky, the sanctioned fallback is to
> split: keep checkpoints 1-4 (boot→join→start→pick) committed and demote 5-6 to the W1
> manual gate. Do **NOT** silently weaken the assertion (dropping the pixel threshold to 1,
> or reverting to a whole-PNG byte-diff, is a silent weakening); if you split or retune, say
> so in the test doc + the W3 journal.

- [x] **Step 5: Headless gate stays green + commit.**
```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUST_MIN_STACK=33554432 cargo test -p dooduel --locked      # smoke is #[ignore], stays skipped
git add apps/dooduel/tests/qa_seat_smoke.rs
git commit -m "test(dooduel): qa_seat #[ignore] GPU-lane smoke — 3 seats, create→join→draw→guess"
```

---

## Wave 3 — close-out

### Task 3.1: Docs — spec status, index row, journal

**Files:**
- Modify: `docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md`
- Modify: `docs/README.md`
- Create: `docs/reports/2026-07-09-dooduel-qa-seat-driver-report.md` (short landing journal)

> **RESULT (W3 close-out, 2026-07-10).** Statuses flipped to `landed`: the spec header
> (`· Status: landed`), this plan header, and both README rows (spec + plan). The two
> QA-harness-found framework fixes (Track 2 `set_value`→`TextChanged` `23540a0`; Track 3
> controlled-input clobber `e81b91f`) and the Track 1 app fix (theme-toggle occlusion
> `e891000`) each shipped with their own spec/design note + regression test and are logged on
> the known-issues §1 regression-watch. The **landing journal (Step 3) is the orchestrator-owned
> campaign journal** under `docs/prototypes/2026-07-09-dooduel-qa-cycles/` (Steps 3–4 left to the
> campaign close-out); the separate `docs/reports/…-report.md` skeleton file was not authored —
> the campaign journal supersedes it.

- [x] **Step 1: Flip the spec status.**
In `docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md`, change the header
`**Revision:** rev-2 …` line's status context to note it's **landed** (add
`· Status: landed (impl 2026-07-09)` to the Date line, or update the `Status:` field per the
`organizing-buiy-docs` convention). Do not rewrite the body.

- [x] **Step 2: Add the docs index row.**
In `docs/README.md`, add a row for the spec + plan under the Dooduel area (mirror the
existing Dooduel entries' format; mark the plan done). Follow `organizing-buiy-docs`.

- [ ] **Step 3: Write the landing journal.**
Create `docs/reports/2026-07-09-dooduel-qa-seat-driver-report.md`: what shipped
(`apps/dooduel/examples/qa_seat.rs` + `apps/dooduel/tests/qa_seat_smoke.rs`), the W0 C1
result (did the two-camera coexistence work first try?), any deviations from the spec, and
whether checkpoints 5-6 stayed in the committed test or were split (Task 2.1 step 4). Note
that seat briefings are tracked separately (another agent).

- [ ] **Step 4: Final full mechanical gate + commit.** This one mirrors CI in full —
  including the rustdoc leg (the example's doc comments must be warning-clean):
```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
RUST_MIN_STACK=33554432 cargo test -p dooduel --locked
git add docs/
git commit -m "docs(dooduel): land qa_seat-driver — spec status, index row, journal"
```

---

## Plan self-review

**1. Spec coverage** (each spec section → a task):
- §2.1 composition + state isolation + two cameras + pointer → W0 Task 0.1 steps 1-2, W1 Task 1.3 env.
- §2.2 real-time loop + readback pacing → W1 Task 1.3 loop; readback burst W0 step 3.
- §2.3 file protocol (atomic, byte-cursor, malformed-skip, `consumed: K`) → W1 Task 1.1 + Task 1.3 loop.
- §3.1/§3.2 verbs + exact names + toolbar workflow → W1 Task 1.2; names exercised in W1 Task 1.4 + W2.
- §3.3 CLI + `DOODUEL_STATE_DIR`/`DOODUEL_SERVER_URL` + Game-canvas-only scope → W1 Task 1.3.
- §res-Q1 two-camera + no-panic → W0 (the whole point). §res-Q2 1280×800 → `W`/`H` consts.
- §res-Q3 room code from text section → W2 `room_code` parser. §res-Q4 mapping → W1 Task 1.2 `canvas_to_window`.
- §res-Q5 set_value + corrected Join → W1 Task 1.2 + Task 1.4 step 5 + W2 checkpoint 3.
- §res-Q6/Q7 example placement + hygiene → the file is `examples/qa_seat.rs` (no Cargo `[dependencies]` edit).
- §6 verification (6 checkpoints, ≥3 seats, wide timers, `#[ignore]` form, run command) → W2 + W1 Task 1.4.
- §7 out of scope (avatar/reconnect/web) → not implemented (correct). §8 C1-C4 → W0 gate + wide-timer config.

**2. Placeholder scan:** no "TBD"/"similar to task N"/"add error handling" — every code step is
complete. The three `> Note:` callouts flag version-specific API paths to *confirm against
cited source*, not placeholders (the code is written; they say where to look if a signature
drifted). The two flagged risks (C1 failure in W0 step 8; checkpoint 5-6 weakness/flakiness in
W2 step 4) are explicit decision points with sanctioned fallbacks, not gaps.

**3. Type consistency:** `readback_rgba` (returns `Vec<u8>`) used by both `save_png` (W0) and
`refresh` (W1); `click_role`/`set_value_role`/`game_canvas_rect`/`canvas_to_window`/
`apply_command`/`tail_commands`/`atomic_write_if_changed`/`append_line` names are used
consistently across tasks. `Applied` enum (Ok/Quit) defined in Task 1.2, consumed in Task
1.3. `W`/`H` consts (1280/800) shared. Server/seat spawn helpers (`spawn_server`/`spawn_seat`/
`append_cmd`/`wait_ui_contains`/`room_code`/`first_word_choice`/`wait_canvas_changed` +
the `HEADER_SKIP_PX` const) all defined in W2 Task 2.1 and used only there.

**Open items an implementer must resolve at the code face (all flagged inline):** exact
`ActionError` variant/field names (`contract.rs:41-82`); Rust-edition `set_var` unsafe-ness;
`snapshot_report` line format for the `room_code`/`first_word_choice` parsers (calibrate
against the real `ui.md` W1 produces — now a hard gate via Task 1.4 step 8). None block the design.

---

## Change log

**rev-2 (2026-07-09)** — folded the plan review's BLOCK verdict (1 blocker + 1 major + 4 minors;
everything else confirmed correct). Each fix re-verified against code.

- **#1 BLOCKER [W0 false premise].** W0 checkpoint 1c clicked "Create a room", but under
  `install_runtime` that calls `start_connect` → `NetState::Joining` + `pending_connect`
  (`lib.rs:808-817`), which `is_networked` (`lib.rs:239-244`) fires `WsClientPlugin` on → a real
  connect that, with no server, fails back to Home + toast (`net.rs:445,508-515` /
  `transport.rs:325-326` / `lib.rs:695-700`). `capture.rs` escaped only because it uses bare
  `dooduel::install` (no `WsClientPlugin`), not `install_runtime`. **Fix:** retargeted 1c to click
  "Join a room" → `Msg::GoJoin` → `Screen::Join` — pure reducer navigation, no net
  (`home.rs:99`; `capture.rs:57-60`). Updated the Step 5 assertion (Screen::Join + a "Join room"
  ui.md), Step 8's expected screenshot (rendered Join screen) + a 3-rung diagnostic ladder
  (black=C1; Home+toast=you took the networked path; Join rendered but model still Home=picking),
  the commit message, and the wave-shape framing (lines 31-35).
- **#2 MAJOR [W2 confounded diff].** `wait_png_changed` byte-diffed the whole PNG, but the
  Drawing-phase countdown repaints ~1 Hz (`in_game.rs:392`), so any diff fired before the stroke
  proved anything. **Fix:** replaced it with `wait_canvas_changed` — decode both PNGs and compare
  only rows `[HEADER_SKIP_PX..H]`, cropping out the header where the timer/progress repaint;
  capture `before` AFTER the toolbar selection so the stroke is the only canvas-region change.
  Verified the reviewer's suggested mechanism (parse the canvas rect from `ui.md`) is NOT possible
  — the game canvas is a role-less `Kind::Raster` (`reconcile.rs:281-286`) absent from both the
  role tree and the text section (`report.rs:233`); the header-crop achieves the same intent
  test-only (sidecar `meta.json` noted as a deferred more-precise option). Named the W1 manual
  eyeball ink as the human-grade proof and reframed the 5-6 demotion note to cover WEAKNESS
  (confounded proxy) as well as flakiness — never silently weaken.
- **#3 MINOR [temp-root leak].** W2's `qa-smoke-<pid>` root now prints its path up front and
  `remove_dir_all`s on success only; a panic unwinds past the cleanup, deliberately retaining it
  for post-mortem.
- **#4 MINOR [W3 gate].** Added `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  --locked` to Task 3.1's final gate (mirrors CI's rustdoc leg — the example's doc comments).
- **#5 MINOR [consumed:K desync].** `tail_commands` no longer filters blank lines; the loop
  increments K for EVERY `\n`-terminated line (blank → logged "skipped (blank line)" with its K),
  so K always equals the agent's appended-line count. Paired with a **spec rev-2.1** one-line
  §2.3 clarification (the team-lead-sanctioned spec edit).
- **#6 MINOR [parser calibration gate].** Task 1.4 now snapshots the Lobby + Picking `ui.md`s
  (steps 4/6) and step 8 extracts the room-code + word-choice lines into a committed
  `docs/reports/2026-07-09-qa-seat-driver-assets/ui-samples.txt`; W2's parser step is now a HARD
  gate ("do NOT write the parsers without the pasted real samples").
