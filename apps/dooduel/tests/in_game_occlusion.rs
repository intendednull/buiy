//! The in-game chat **Send** button must not be occluded by the floating theme
//! toggle (Track 1 of the playtest cycle). At 1280×800 the desktop 3-pane puts the
//! chat pane's Send row in the bottom-right corner — the same corner the `.fixed()
//! .top_layer()` theme toggle floats in. A real pointer click at Send's center used
//! to fold `SetTheme` (the top-layer toggle won the hit-test) instead of submitting
//! the guess.
//!
//! Uses the `canvas_e2e.rs` unified headless driver (the GPU-free probe preset + the
//! real `bevy_picking` stack + a synthetic 1280×800 window/camera/pointer), so both
//! the layout rect and the real pointer route are exercised headless, and navigates
//! into the Drawing phase (`StartMatch` → `ChooseWord(0)`) where the chat pane is
//! laid out with no overlay in front of it.

use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};

use buiy_core::ResolvedLayout;
use buiy_core::a11y::A11yRole;
use buiy_core::a11y::translate::entity_for_node_id;
use buiy_core::mvu::Envelope;
use buiy_verify::invariant::no_transparent_top_layer_occluder;
use buiy_verify::pointer::drive_stroke;

use dooduel::{Dooduel, Msg, Screen};

/// The 1280×800 viewport the bug was found at (desktop 3-pane).
const VW: f32 = 1280.0;
const VH: f32 = 800.0;

/// The unified headless driver: the GPU-free probe preset + the real picking stack +
/// a synthetic 1280×800 window/camera/pointer, then Dooduel + the solo authority +
/// the drawing canvas (so `StartMatch` → `ChooseWord` reaches Drawing over the real
/// intent/event path). Mirrors `canvas_e2e.rs::unified_driver`.
fn driver() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(bevy::picking::PickingPlugin)
        .add_plugins(buiy::BuiyProbePlugin)
        .add_plugins(buiy_core::picking::PickingPlugin)
        .add_plugins(buiy_core::picking::BuiyPickingBackendPlugin);

    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(VW as u32, VH as u32),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));
    let target = WindowRef::Entity(window)
        .normalize(Some(window))
        .expect("normalize window target");
    let pointer = app
        .world_mut()
        .spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target: NormalizedRenderTarget::Window(target),
                position: Vec2::ZERO,
            }),
        ))
        .id();
    app.init_asset::<Image>();

    dooduel::install(&mut app);
    app.add_plugins(dooduel::net::NetPlugin);
    app.add_plugins(dooduel::net::LocalAuthorityPlugin);
    app.add_plugins(dooduel::paint::CanvasPlugin);
    (app, window, pointer)
}

fn settle(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn enqueue(app: &mut App, msg: Msg) {
    let e = app
        .world_mut()
        .query_filtered::<Entity, With<Dooduel>>()
        .iter(app.world())
        .next()
        .expect("model entity exists");
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<Envelope<Dooduel>>>()
        .write(Envelope::user(e, msg));
}

fn model(app: &mut App) -> Dooduel {
    app.world_mut()
        .query::<&Dooduel>()
        .iter(app.world())
        .next()
        .cloned()
        .expect("model exists")
}

/// A button's window-space top-left + size, located by its accessible name.
fn button_rect(app: &mut App, label: &str) -> (Vec2, Vec2) {
    let node = buiy::probe::get_by_role(app.world_mut(), A11yRole::Button, Some(label), None)
        .unwrap_or_else(|_| panic!("the '{label}' button is a locatable button"));
    let e = entity_for_node_id(node).expect("the button node maps to an entity");
    let world = app.world();
    let tl = world
        .get::<GlobalTransform>(e)
        .expect("button has GlobalTransform")
        .translation()
        .truncate();
    let size = world
        .get::<ResolvedLayout>(e)
        .expect("button has ResolvedLayout")
        .size;
    (tl, size)
}

/// Navigate a fresh driver into the in-game Drawing phase as the drawer (seat 0), with
/// the chat pane laid out and no overlay in front of it.
fn into_drawing() -> (App, Entity, Entity) {
    let (mut app, window, pointer) = driver();
    settle(&mut app, 12);
    enqueue(&mut app, Msg::StartMatch);
    settle(&mut app, 16);
    enqueue(&mut app, Msg::ChooseWord(0));
    settle(&mut app, 16);
    assert_eq!(
        model(&mut app).screen,
        Screen::InGame,
        "navigated onto the in-game screen"
    );
    (app, window, pointer)
}

#[test]
fn in_game_chat_send_is_not_occluded_by_the_theme_toggle() {
    let (mut app, window, pointer) = into_drawing();

    // Seed the guess field so a click that actually reaches Send leaves a POSITIVE
    // trace: `SubmitGuess` does `mem::take(&mut chat_input)` UNCONDITIONALLY before its
    // drawer/empty gate (lib.rs), so even for this drawing seat (chat disabled) a
    // Send-routed click clears the field. This makes the test catch ANY future widget
    // occluding Send — not just the theme toggle returning.
    enqueue(&mut app, Msg::SetChatInput("banana".to_string()));
    settle(&mut app, 4);
    assert_eq!(
        model(&mut app).chat_input,
        "banana",
        "the guess field is seeded before the click"
    );

    // A real synthetic pointer click at the Send button's resolved center.
    let theme_before = model(&mut app).theme;
    let (tl, size) = button_rect(&mut app, "Send");
    let center = tl + size * 0.5;
    let path = [center, center + Vec2::new(1.0, 0.0)];
    drive_stroke(&mut app, window, pointer, &path);
    settle(&mut app, 6);

    // POSITIVE signal — the click reached Send: the guess field was cleared. Before the
    // fix, the `.top_layer()` toggle floating in the same bottom-right corner won the
    // hit-test at Send's center, so Send never saw the click and the field stayed
    // "banana". This is the assertion that generalizes to any occluder.
    assert!(
        model(&mut app).chat_input.is_empty(),
        "a real click at the Send button's center must route to Send (clearing the \
         guess field) — a widget is occluding the chat Send control"
    );
    // NEGATIVE signal — the click did NOT land on the toggle: the theme is unchanged
    // (before the fix it folded `SetTheme(Dark)`).
    assert_eq!(
        model(&mut app).theme,
        theme_before,
        "a real click at the Send button's center must not flip the theme"
    );

    // And the fix itself: the floating theme toggle is suppressed on the in-game
    // screen (it would otherwise float over the bottom-right chat pane). The default
    // theme is Light, so its label is "Light" — it must not be in the tree in-game.
    assert!(
        buiy::probe::get_by_role(app.world_mut(), A11yRole::Button, Some("Light"), None).is_err(),
        "the floating theme toggle must be absent on the in-game screen"
    );
}

/// Structural companion to the click-route test above (Tier-3 invariant, F6 /
/// app-author-ergonomics 4b-invariant): sweep the reusable
/// `no_transparent_top_layer_occluder` predicate over the reconciled dooduel world.
/// This is **additive** to the native-pointer assertion — a structural check that
/// no screen leaves the invisible-occluder class at all, not just that Send is
/// reachable. Dooduel's `.top_layer()` nodes are all `buiy_view`-reconciled: the
/// floating theme toggle is `.ignore_picking()` (auto-`Pickable::IGNORE`) and the
/// modal scrims paint a translucent `background(SCRIM)` fill, so the sweep is green.
#[test]
fn dooduel_screens_have_no_transparent_top_layer_occluder() {
    // Home: the floating theme toggle is a transparent `.top_layer()` container,
    // safe ONLY because it carries `Pickable::IGNORE` — exactly the case the sweep
    // must PASS (a reconciler-auto-ignored transparent top-layer node).
    let (mut app, _window, _pointer) = driver();
    settle(&mut app, 12);
    no_transparent_top_layer_occluder(app.world()).unwrap_or_else(|v| {
        panic!("the dooduel Home screen leaves a transparent top-layer occluder: {v}")
    });

    // In-game Drawing (the toggle is suppressed here; any word-pick scrim paints a
    // fill). A distinct reconciled screen state, swept the same way.
    let (app, _window, _pointer) = into_drawing();
    no_transparent_top_layer_occluder(app.world()).unwrap_or_else(|v| {
        panic!("the dooduel in-game screen leaves a transparent top-layer occluder: {v}")
    });
}
