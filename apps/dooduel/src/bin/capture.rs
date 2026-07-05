//! Headless per-screen GPU capture → PNG (offscreen render-to-texture + readback;
//! needs a real wgpu adapter, no display). The eyeball artifact for design parity.
//!
//! Renders the live app offscreen with the LIGHT theme + the purple accent + the
//! Caveat/Shantell fonts (`DooduelThemePlugin`), drives the real MVU funnel to each
//! screen/state, and writes `target/dooduel-captures/<name>.png`.
//!
//! Run: `RUST_MIN_STACK=33554432 cargo run -p dooduel --bin capture` (needs a wgpu
//! adapter; works headless with Vulkan/lavapipe — no display required).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::camera::{CameraPlugin, RenderTarget};
use bevy::ecs::message::Messages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Msaa;

use buiy_core::mvu::Envelope;
use buiy_core::theme::{default_dark_theme, default_light_theme};
use dooduel::game::{ChatKind, ChatMsg, Phase};
use dooduel::theme::DooduelThemePlugin;
use dooduel::{Dooduel, Msg, ReplicaPlayer, Screen, ServerEvent, WireAvatar, theme::ThemePref};

const WIDTH: u32 = 1200;
const HEIGHT: u32 = 760;
const OUT_DIR: &str = "target/dooduel-captures";

fn main() {
    let mut app = build_app(WIDTH, HEIGHT);
    let target = render_target(&mut app, WIDTH, HEIGHT, ThemePref::Light);
    app.finish();
    app.cleanup();
    // Warm up: spawn model + camera, register fonts, build the tree, load + reshape.
    for _ in 0..90 {
        app.update();
    }

    // === Home ===
    capture(&mut app, target.clone(), &format!("{OUT_DIR}/home.png"));
    assert_eq!(screen(&mut app), Screen::Home, "app starts on Home");

    // === Home → Create a room → Lobby (host) ===
    enqueue(&mut app, Msg::SetName("Mara".to_string()));
    enqueue(&mut app, Msg::CreateRoom);
    settle(&mut app);
    assert_eq!(screen(&mut app), Screen::Lobby, "Create a room → Lobby");
    capture(&mut app, target.clone(), &format!("{OUT_DIR}/lobby.png"));

    // === back Home → Join screen ===
    enqueue(&mut app, Msg::Back);
    settle(&mut app);
    enqueue(&mut app, Msg::GoJoin);
    enqueue(&mut app, Msg::SetJoinCode("7xq2kp".to_string()));
    settle(&mut app);
    assert_eq!(screen(&mut app), Screen::Join, "Join a room → Join screen");
    capture(&mut app, target.clone(), &format!("{OUT_DIR}/join.png"));

    // === Avatar editor modal (over Home) ===
    enqueue(&mut app, Msg::Back);
    settle(&mut app);
    enqueue(&mut app, Msg::OpenAvatarEditor);
    settle(&mut app);
    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/avatar_gallery.png"),
    );
    enqueue(&mut app, Msg::SetAvatarTab(dooduel::AvatarTab::Draw));
    settle(&mut app);
    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/avatar_draw.png"),
    );
    enqueue(&mut app, Msg::CloseAvatarEditor);
    settle(&mut app);

    // The in-game + podium screens are seeded through the REAL `Msg::Net` path (a
    // hand-scripted `ServerEvent` stream, exactly what a live `Session` would send)
    // — the least-code faithful port after the client-replica refactor (M1 W3): no
    // GPU-free `Session`/clock plumbing, and it exercises the reducer's event→replica
    // fold. `enter_game` puts the shell on the in-game screen for a fresh turn.

    // === In-game (drawer, mid-draw) ===
    enqueue(&mut app, Msg::Back);
    settle(&mut app);
    enter_game(&mut app);
    drawing(&mut app, 0, "R O B O T", 5, 0); // seat 0 draws; sees the full word
    tick(&mut app);
    settle(&mut app);
    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/in_game_drawer.png"),
    );

    // === In-game (guesser view: chat feedback + hint slot) — bugs #1/#4/#5 ===
    // Seat 1 is a guesser: blanks + one revealed hint, plus the three-way chat (a
    // shared WRONG guess, a private near-miss nudge, a green CORRECT row).
    enqueue(&mut app, Msg::Back);
    settle(&mut app);
    enter_game_as(&mut app, 1);
    drawing(&mut app, 0, "_ _ B _ _", 5, 1);
    net(&mut app, chat(1, ChatKind::Guess, "Theo: windmill", None));
    net(&mut app, chat(2, ChatKind::Close, "So close! 👀", Some(1)));
    net(
        &mut app,
        chat(3, ChatKind::Correct, "🎉 Sam guessed the word!", None),
    );
    tick(&mut app);
    settle(&mut app);
    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/in_game_feedback.png"),
    );

    // === In-game (word-pick overlay) — seat 0 is picking ===
    enqueue(&mut app, Msg::Back);
    settle(&mut app);
    enter_game(&mut app);
    net(
        &mut app,
        ServerEvent::PhaseChanged {
            phase: Phase::Picking,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(15),
        },
    );
    net(
        &mut app,
        ServerEvent::WordChoices {
            words: vec!["robot".into(), "castle".into(), "kite".into()],
        },
    );
    tick(&mut app);
    settle(&mut app);
    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/in_game_picking.png"),
    );

    // === Podium — the `MatchEnded` event lifts the shell to the podium ===
    enqueue(&mut app, Msg::Back);
    settle(&mut app);
    net(&mut app, roster());
    net(
        &mut app,
        ServerEvent::PhaseChanged {
            phase: Phase::Final,
            drawer: None,
            round: 2,
            total_rounds: 2,
            remaining: Duration::ZERO,
        },
    );
    net(
        &mut app,
        ServerEvent::MatchEnded {
            podium: vec![
                (1, "Priya".into(), 1420),
                (0, "Mara".into(), 980),
                (2, "Theo".into(), 610),
                (3, "Sam".into(), 300),
            ],
        },
    );
    settle(&mut app);
    capture(&mut app, target.clone(), &format!("{OUT_DIR}/podium.png"));

    // === Dark theme — Home. `SetTheme(Dark)` folds → `sync_theme_resource` swaps
    // the base ladder; the app canvas fills, so the light camera clear never shows.
    enqueue(&mut app, Msg::Back);
    enqueue(&mut app, Msg::SetTheme(ThemePref::Dark));
    settle(&mut app);
    capture(&mut app, target, &format!("{OUT_DIR}/home_dark.png"));

    println!("OK: wrote Dooduel captures to {OUT_DIR}/");
}

fn build_app(w: u32, h: u32) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
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
        .add_plugins(CameraPlugin)
        .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(buiy::BuiyHeadlessPlugin);
    // The lower-level `install` (NO wall-clock ClockPlugin — the capture drives the
    // match with injected `Tick`s / virtual time), plus just the plugins a capture
    // needs: the theme (fonts + purple accent), the paint canvases (so the in-game +
    // avatar rasters render), and the podium confetti. Persistence + the viewport
    // shell are omitted (no disk, desktop layout).
    dooduel::install(&mut app);
    app.add_plugins(DooduelThemePlugin);
    app.add_plugins(dooduel::paint::CanvasPlugin);
    app.add_plugins(dooduel::confetti::ConfettiPlugin);
    app.init_asset::<Mesh>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app
}

fn render_target(app: &mut App, w: u32, h: u32, theme: ThemePref) -> Handle<Image> {
    let (base, clear) = match theme {
        ThemePref::Light => (default_light_theme(), Color::srgb_u8(0xf4, 0xf5, 0xf8)),
        ThemePref::Dark => (default_dark_theme(), Color::srgb_u8(0x1b, 0x1e, 0x25)),
    };
    app.insert_resource(base);

    let mut image = Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image.asset_usage = RenderAssetUsages::all();
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

    app.world_mut().spawn((
        Camera2d,
        RenderTarget::from(target.clone()),
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(clear),
            ..default()
        },
    ));
    target
}

fn screen(app: &mut App) -> Screen {
    app.world_mut()
        .query::<&Dooduel>()
        .iter(app.world())
        .next()
        .expect("model exists")
        .screen
        .clone()
}

fn enqueue(app: &mut App, msg: Msg) {
    let e = app
        .world_mut()
        .query_filtered::<Entity, With<Dooduel>>()
        .iter(app.world())
        .next()
        .expect("model entity");
    app.world_mut()
        .resource_mut::<Messages<Envelope<Dooduel>>>()
        .write(Envelope::user(e, msg));
}

fn settle(app: &mut App) {
    for _ in 0..10 {
        app.update();
    }
}

/// Enqueue one authoritative event (what a live `Session` would send).
fn net(app: &mut App, ev: ServerEvent) {
    enqueue(app, Msg::Net(ev));
}

/// Anchor + derive the countdown (a `PhaseChanged` staged the anchor; the tick folds
/// it to whole seconds). Capture's virtual clock sits near zero, so `from_secs(0)`
/// derives `secs == remaining`.
fn tick(app: &mut App) {
    enqueue(app, Msg::Tick(Duration::ZERO));
}

/// The four-seat sample roster the in-game + podium captures share.
fn sample_players() -> Vec<ReplicaPlayer> {
    let mk = |name: &str, is_bot: bool, score: i64| ReplicaPlayer {
        name: name.to_string(),
        avatar: WireAvatar::Default,
        connected: true,
        is_bot,
        score,
        guessed: false,
    };
    vec![
        mk("Mara", false, 340),
        mk("Priya", true, 410),
        mk("Theo", true, 180),
        mk("Sam", true, 90),
    ]
}

/// The shared roster event.
fn roster() -> ServerEvent {
    ServerEvent::Roster {
        players: sample_players(),
        host: 0,
    }
}

/// A chat-line event. `seq` must be UNIQUE per line (the chat `keyed_column` keys on
/// it — a real session's `chat_seq` is monotonic).
fn chat(seq: u64, kind: ChatKind, text: &str, to: Option<usize>) -> ServerEvent {
    ServerEvent::ChatLine {
        line: ChatMsg {
            seq,
            kind,
            text: text.to_string(),
            to,
        },
    }
}

/// Seat this client on the in-game screen for a fresh room (as seat `seat`): the
/// `Welcome` + `Roster` seed, plus a direct screen set (capture has no `Session`, so
/// nothing else lifts the shell into the game).
fn enter_game_as(app: &mut App, seat: usize) {
    net(
        app,
        ServerEvent::Welcome {
            seat,
            room_code: "SOLO".to_string(),
            reconnect_token: String::new(),
            protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
        },
    );
    net(app, roster());
    settle(app);
    let e = app
        .world_mut()
        .query_filtered::<Entity, With<Dooduel>>()
        .iter(app.world())
        .next()
        .expect("model entity");
    app.world_mut().get_mut::<Dooduel>(e).expect("model").screen = Screen::InGame;
}

/// Seat this client as the drawer (seat 0) on the in-game screen.
fn enter_game(app: &mut App) {
    enter_game_as(app, 0);
}

/// A `PhaseChanged(Drawing)` for `drawer` + this client's `WordUpdate` (the drawer
/// gets the full word; a guesser gets `display` — blanks + revealed hints).
fn drawing(app: &mut App, drawer: usize, display: &str, len: usize, hints: usize) {
    net(
        app,
        ServerEvent::PhaseChanged {
            phase: Phase::Drawing,
            drawer: Some(drawer),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(60),
        },
    );
    net(
        app,
        ServerEvent::WordUpdate {
            display: display.to_string(),
            len,
            hints_revealed: hints,
        },
    );
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
