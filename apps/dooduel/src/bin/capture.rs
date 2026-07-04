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
use dooduel::theme::DooduelThemePlugin;
use dooduel::{Dooduel, Msg, Screen, theme::ThemePref};

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

    // === In-game (drawer, mid-draw) ===
    enqueue(&mut app, Msg::SetName("Mara".to_string()));
    enqueue(&mut app, Msg::Play);
    settle(&mut app);
    // Pick the first word to enter the Drawing phase, then advance the clock a bit.
    enqueue(&mut app, Msg::ChooseWord(0));
    settle(&mut app);
    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/in_game_drawer.png"),
    );

    // === In-game (guesser view: chat feedback + hint slot) — bugs #1/#4/#5 ===
    // A controlled, bots-off match set directly on the model (like the playtest
    // host's `start`) so the three-way feedback is deterministic: view as a guesser,
    // tick past the first hint threshold (a letter reveals), then inject a shared
    // WRONG guess, a private NEAR-MISS (by the viewing seat), and a green CORRECT row.
    {
        use std::time::Duration;
        let e = app
            .world_mut()
            .query_filtered::<Entity, With<Dooduel>>()
            .iter(app.world())
            .next()
            .expect("model entity");
        // Scope the `Mut<Dooduel>` borrow so it ends before `settle`/`capture` reborrow
        // the world (a no-op `drop()` of a non-Drop `Mut` would not compile-clean).
        {
            let mut d = app.world_mut().get_mut::<Dooduel>(e).expect("model");
            d.game.start_match(
                "Mara",
                dooduel::game::Config {
                    bots_enabled: false,
                    ..Default::default()
                },
            );
            d.screen = Screen::InGame;
            let w = d.game.word_choices[0].clone();
            d.game.choose_word(w);
            d.game.tick(Duration::from_secs(0)); // anchor the draw clock
            d.game.tick(Duration::from_secs(50)); // elapsed 50 > 47 ⇒ first hint reveals
            let secret = d.game.secret_word.clone();
            d.game.switch_seat(1); // view as a guesser (blanks + the revealed hint)
            d.game.apply_guess(2, "windmill"); // a shared WRONG guess (everyone sees it)
            let near = format!("{secret}{}", secret.chars().last().unwrap_or('s')); // one-off ⇒ near-miss
            d.game.apply_guess(1, &near); // PRIVATE "So close!" nudge to the viewing seat
            d.game.apply_guess(3, &secret); // a green CORRECT row
        }
        settle(&mut app);
        capture(
            &mut app,
            target.clone(),
            &format!("{OUT_DIR}/in_game_feedback.png"),
        );
    }

    // === In-game (word-pick overlay) — restart a match, capture Picking ===
    enqueue(&mut app, Msg::Back);
    settle(&mut app);
    enqueue(&mut app, Msg::Play);
    settle(&mut app);
    capture(
        &mut app,
        target.clone(),
        &format!("{OUT_DIR}/in_game_picking.png"),
    );

    // === Podium — drive a full instant match via injected ticks ===
    drive_to_podium(&mut app);
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

/// Drive an instant full match to the podium by injecting `Tick`s far past every
/// phase timeout (the pure clock derives everything from `now`).
fn drive_to_podium(app: &mut App) {
    use std::time::Duration;
    let mut t = 1u64;
    for _ in 0..400 {
        if screen(app) == Screen::Podium {
            break;
        }
        enqueue(app, Msg::Tick(Duration::from_secs(t)));
        t += 5;
        app.update();
    }
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
