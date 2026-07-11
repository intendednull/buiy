//! W8 §D1 (part 2) — the drawing canvas driven END-TO-END through the REAL
//! pointer pipeline, headless. This is the "unified headless driver" answer: a
//! single App runs the **GPU-free probe preset** (`BuiyProbePlugin` — the MVU
//! funnel + reconciler + a11y projection) AND the **picking stack** (bevy's
//! `PickingPlugin`, Buiy's `PickingPlugin` + backend, a `Camera2d` + synthetic
//! window) — they compose because the probe preset simply *omits* picking, so
//! adding it back conflicts with nothing. On that App we boot Dooduel + the
//! drawing canvas, navigate into the Drawing phase as the drawer, and drag a real
//! synthetic pointer across the canvas node's laid-out rect. `pointer_events`
//! derives `Pointer<Press>` → `Drag` → `Release`, the app's own paint observers
//! (`on_canvas_press`/`on_canvas_drag`) map each to canvas pixels, and we assert
//! INK landed in the `PaintCanvases` CPU buffer at the stroked location — the full
//! input → funnel → canvas path, with no GPU.

use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use buiy_core::components::ResolvedLayout;
use buiy_core::mvu::Envelope;
use buiy_verify::pointer::drive_stroke;
use dooduel::net::{CanvasProgress, ClientNet};
use dooduel::paint::{CANVAS_H, CANVAS_W, CanvasKind, PAPER, PaintCanvases};
use dooduel::{CanvasOp, Dooduel, Msg, ReplicaPlayer, ServerEvent, WireAvatar};
use dooduel_core::protocol::ClientIntent;
use dooduel_core::transport::{ClientTransport, ConnStatus};

/// A test transport wired to two shared queues — the intents the client SENT (for
/// asserting the outbound stream) and the events to hand it on `try_recv` (for driving
/// the pump). `Rc`-backed, single-threaded, like `InProcClient`.
#[derive(Clone, Default)]
struct TestWires {
    sent: Rc<RefCell<Vec<ClientIntent>>>,
    recv: Rc<RefCell<VecDeque<ServerEvent>>>,
}

struct TestTransport {
    wires: TestWires,
}

impl ClientTransport for TestTransport {
    fn send(&mut self, intent: &ClientIntent) {
        self.wires.sent.borrow_mut().push(intent.clone());
    }
    fn try_recv(&mut self) -> Option<ServerEvent> {
        self.wires.recv.borrow_mut().pop_front()
    }
    fn status(&self) -> ConnStatus {
        ConnStatus::Open
    }
}

/// Build the unified headless driver: the GPU-free probe preset + the real
/// picking stack + a synthetic window/camera/pointer, then Dooduel + the canvas.
/// Returns `(app, window, pointer)`.
fn unified_driver() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        // The core bevy_picking infrastructure (PointerInput::receive + hit
        // scheduling + Messages<PointerHits>).
        .add_plugins(bevy::picking::PickingPlugin)
        // The GPU-free Buiy stack: MVU funnel + reconciler + layout + a11y + text
        // + widgets. It OMITS picking — which is exactly why we can add it back.
        .add_plugins(buiy::BuiyProbePlugin)
        // Buiy's picking half: the InteractionPlugin hover stage (Pointer<E>
        // taxonomy) + the Buiy hit-test backend. (Scroll/animation are NOT needed
        // — the recipe is deliberately minimal, confirmed by removing them.)
        .add_plugins(buiy_core::picking::PickingPlugin)
        .add_plugins(buiy_core::picking::BuiyPickingBackendPlugin);

    // A synthetic primary window so the layout solver has a viewport and the
    // pointer has a target (desktop 3-pane in-game layout).
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(1280, 800),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    // A Camera2d targeting the window so `emit_picks` resolves a camera (else the
    // backend drops every hit).
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));
    // The synthetic mouse pointer (drive_stroke rewrites its PointerLocation).
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

    // The `Image` asset type: `CanvasPlugin` creates + mirrors the canvas image,
    // but the GPU-free probe preset never adds `ImagePlugin`/`RenderPlugin` (which
    // normally `init_asset::<Image>()`), so a headless canvas host must register it
    // itself — the missing piece of the unified-driver recipe.
    app.init_asset::<Image>();

    // Dooduel MVU + the in-process solo authority (M1 W3 — `Msg::StartMatch` now
    // drives a real `Session` through `LocalAuthorityPlugin`, so the match reaches
    // Drawing over the intent/event path, not a local `Game` mutation) + the drawing
    // canvas (CPU paint surfaces + the paint observers).
    dooduel::install(&mut app);
    app.add_plugins(dooduel::net::NetPlugin);
    app.add_plugins(dooduel::net::LocalAuthorityPlugin);
    app.add_plugins(dooduel::paint::CanvasPlugin);

    (app, window, pointer)
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
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

/// The Game drawing canvas node's window-space top-left + size (set by the
/// reconciler + wire_canvas_node once the in-game screen is up).
fn canvas_rect(app: &mut App) -> (Vec2, Vec2) {
    app.world_mut()
        .query::<(&CanvasKind, &GlobalTransform, &ResolvedLayout)>()
        .iter(app.world())
        .find(|(k, ..)| **k == CanvasKind::Game)
        .map(|(_, gt, layout)| (gt.translation().truncate(), layout.size))
        .expect("the Game canvas node is laid out in the in-game screen")
}

fn game_pixel(app: &mut App, x: usize, y: usize) -> [u8; 4] {
    let canvases = app.world().resource::<PaintCanvases>();
    let s = canvases.surface(CanvasKind::Game);
    let i = (y * s.width + x) * 4;
    [
        s.pixels[i],
        s.pixels[i + 1],
        s.pixels[i + 2],
        s.pixels[i + 3],
    ]
}

#[test]
fn dragging_the_canvas_lands_ink_in_the_paint_buffer() {
    let (mut app, window, pointer) = unified_driver();
    // Boot: Startup creates the canvases; announce_canvases folds the handles;
    // the reconciler settles.
    settle(&mut app, 12);

    // Into the Drawing phase as the drawer: `StartMatch` spins up the solo `Session`
    // (seat 0 is the human drawer this turn), then `ChooseWord(0)` sends the `Pick`
    // intent → `PhaseChanged(Drawing)` flows back → the canvas `enabled` becomes true.
    // The extra settle frames cover the intent→event round-trips through the pump.
    enqueue(&mut app, Msg::StartMatch);
    settle(&mut app, 16);
    enqueue(&mut app, Msg::ChooseWord(0));
    settle(&mut app, 16);

    // The canvas node is laid out with a real 720×450 rect (shrink(false) keeps it
    // at its natural size, so window px → canvas texel is 1:1).
    let (tl, size) = canvas_rect(&mut app);
    assert!(
        size.x > 100.0 && size.y > 100.0,
        "the canvas has a real rect: {size}"
    );

    // The buffer starts blank (the per-turn auto-clear on the Picking→Drawing edge).
    let mid = (CANVAS_W / 2, CANVAS_H / 2);
    assert_eq!(
        game_pixel(&mut app, mid.0, mid.1),
        PAPER,
        "the canvas is blank paper before we draw"
    );

    // Drag a horizontal line across the middle third of the canvas rect.
    let y = tl.y + size.y * 0.5;
    let from = Vec2::new(tl.x + size.x * 0.3, y);
    let to = Vec2::new(tl.x + size.x * 0.7, y);
    let path: Vec<Vec2> = (0..=6).map(|i| from.lerp(to, i as f32 / 6.0)).collect();
    drive_stroke(&mut app, window, pointer, &path);

    // INK landed: the center pixel (on the stroked line) is no longer paper, and a
    // meaningful band of pixels was painted (the brush stamped + interpolated).
    assert_ne!(
        game_pixel(&mut app, mid.0, mid.1),
        PAPER,
        "the stroke inked the canvas center pixel"
    );
    let inked = {
        let canvases = app.world().resource::<PaintCanvases>();
        let s = canvases.surface(CanvasKind::Game);
        s.pixels.chunks_exact(4).filter(|p| *p != PAPER).count()
    };
    assert!(
        inked > 500,
        "the stroke painted a visible line (inked pixels = {inked})"
    );
}

fn inked_pixels(app: &App) -> usize {
    let canvases = app.world().resource::<PaintCanvases>();
    let s = canvases.surface(CanvasKind::Game);
    s.pixels.chunks_exact(4).filter(|p| *p != PAPER).count()
}

/// The drawer's canvas render is UNIFORM (no per-role filter) yet must not be blanked
/// by an incoming canvas event: under no-echo the drawer's `replica.canvas_ops` stays
/// empty during its own turn, so a `CanvasUndo` / `CanvasCleared` it receives (which
/// it already applied optimistically) is idempotent — the uniform re-render is never
/// triggered (empty log unchanged) and the optimistic ink survives. This is the
/// concrete "fights the optimistic state" case a naive uniform re-raster would break.
#[test]
fn drawer_optimistic_ink_survives_an_incoming_canvas_event() {
    let (mut app, window, pointer) = unified_driver();
    settle(&mut app, 12);
    enqueue(&mut app, Msg::StartMatch);
    settle(&mut app, 16);
    enqueue(&mut app, Msg::ChooseWord(0));
    settle(&mut app, 16);

    // The human (seat 0) is the drawer this turn — draw a line.
    let (tl, size) = canvas_rect(&mut app);
    let y = tl.y + size.y * 0.5;
    let from = Vec2::new(tl.x + size.x * 0.3, y);
    let to = Vec2::new(tl.x + size.x * 0.7, y);
    let path: Vec<Vec2> = (0..=6).map(|i| from.lerp(to, i as f32 / 6.0)).collect();
    drive_stroke(&mut app, window, pointer, &path);
    let drawn = inked_pixels(&app);
    assert!(drawn > 500, "the drawer inked its optimistic canvas");

    // A spurious incoming CanvasUndo/CanvasCleared (the drawer's own confirmations are
    // idempotent — its optimistic buffer is the truth) must NOT blank the canvas.
    enqueue(
        &mut app,
        Msg::Net(ServerEvent::CanvasUndo { removed_id: 0 }),
    );
    settle(&mut app, 6);
    assert_eq!(
        inked_pixels(&app),
        drawn,
        "an incoming CanvasUndo did not touch the drawer's optimistic ink"
    );
}

/// A drawer-role app (this client is seat 0, the drawer, in Drawing) whose canvas has
/// been reseeded from a scripted `CanvasLog` — the mid-turn reconnect shape. No live
/// session; the events are exactly what the server sends on reconnect. Returns the app
/// with ink already on the Game canvas (asserted here).
fn drawer_role_app_reseeded() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(buiy::BuiyProbePlugin);
    app.init_asset::<Image>();
    dooduel::install(&mut app);
    app.add_plugins(dooduel::paint::CanvasPlugin);
    settle(&mut app, 12);

    let net = |app: &mut App, ev: ServerEvent| enqueue(app, Msg::Net(ev));
    net(
        &mut app,
        ServerEvent::Welcome {
            seat: 0,
            room_code: "SOLO".to_string(),
            reconnect_token: String::new(),
            protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
        },
    );
    net(
        &mut app,
        ServerEvent::PhaseChanged {
            phase: dooduel::game::Phase::Drawing,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: std::time::Duration::from_secs(60),
        },
    );
    settle(&mut app, 8);
    assert_eq!(inked_pixels(&app), 0, "the drawer's canvas starts blank");

    // The reconnect reseed: the full current-turn op log (a stroke across the middle).
    let mid_y = (CANVAS_H / 2) as i32;
    let ops = vec![CanvasOp::Stroke {
        id: 0,
        points: (0..CANVAS_W as i32)
            .step_by(4)
            .map(|x| (x, mid_y))
            .collect(),
        color: [20, 20, 24, 255],
        radius: 4,
    }];
    net(&mut app, ServerEvent::CanvasLog { ops });
    settle(&mut app, 8);
    app
}

/// A drawer's mid-turn reconnect reseed (`CanvasLog`) DOES re-render its canvas from
/// the authoritative log — the uniform render restores the reconnected drawer's canvas
/// (the case a blanket drawer-ignores-canvas-events filter would break).
#[test]
fn drawer_canvas_reseeds_from_a_canvas_log() {
    let mut app = drawer_role_app_reseeded();
    assert_ne!(
        game_pixel(&mut app, CANVAS_W / 2, CANVAS_H / 2),
        PAPER,
        "the drawer's canvas re-rendered from the CanvasLog reseed"
    );
    assert!(
        inked_pixels(&app) > 500,
        "the reseeded stroke is rasterized onto the drawer's canvas"
    );
}

/// A `CanvasCleared` reaching a DRAWER-role replica clears its raster — proving the
/// drawer applies canvas events uniformly (not just `CanvasLog`). After a reseed
/// populated the drawer's log, a `CanvasCleared` truncates it and the uniform render
/// blanks the canvas.
#[test]
fn canvas_cleared_clears_a_drawer_role_raster() {
    let mut app = drawer_role_app_reseeded();
    assert!(
        inked_pixels(&app) > 500,
        "reseeded ink present before clear"
    );

    enqueue(&mut app, Msg::Net(ServerEvent::CanvasCleared));
    settle(&mut app, 8);
    assert_eq!(
        inked_pixels(&app),
        0,
        "CanvasCleared cleared the drawer-role client's raster (uniform application)"
    );
}

// ---------------------------------------------------------------------------
// Drawer undo/clear intent-relay hygiene (I-1/2) — a canvas-only app with a
// TestTransport `ClientNet` so we can assert the outbound intent stream.
// ---------------------------------------------------------------------------

/// A GPU-free canvas app (no session) with a [`TestTransport`] `ClientNet`, seated as
/// the drawer (seat 0) in Drawing so the toolbar is `enabled` and relays intents.
fn drawer_canvas_app_with_wires() -> (App, TestWires) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(buiy::BuiyProbePlugin);
    app.init_asset::<Image>();
    dooduel::install(&mut app);
    app.add_plugins(dooduel::paint::CanvasPlugin);
    let wires = TestWires::default();
    app.insert_non_send(ClientNet(Some(Box::new(TestTransport {
        wires: wires.clone(),
    }))));
    settle(&mut app, 12);

    enqueue(
        &mut app,
        Msg::Net(ServerEvent::Welcome {
            seat: 0,
            room_code: "SOLO".to_string(),
            reconnect_token: String::new(),
            protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
        }),
    );
    enqueue(
        &mut app,
        Msg::Net(ServerEvent::PhaseChanged {
            phase: dooduel::game::Phase::Drawing,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: std::time::Duration::from_secs(60),
        }),
    );
    settle(&mut app, 8);
    (app, wires)
}

/// Directly stamp one undoable stroke into the Game buffer (each `begin` snapshots the
/// undo ring), simulating the drawer's optimistic paint without the pointer stack.
fn stamp_optimistic_stroke(app: &mut App, y: i32) {
    let mut canvases = app.world_mut().resource_mut::<PaintCanvases>();
    let g = canvases.surface_mut(CanvasKind::Game);
    g.begin(80, y);
    for x in (90..640).step_by(6) {
        g.extend(x, y);
    }
    g.end();
}

fn undo_intent_count(wires: &TestWires) -> usize {
    wires
        .sent
        .borrow()
        .iter()
        .filter(|i| matches!(i, ClientIntent::Undo))
        .count()
}

/// I-1: a local Clear is NON-undoable (matches the server, where Clear mints no op),
/// so a following Undo neither resurrects the drawing NOR relays an Undo intent.
#[test]
fn clear_then_undo_leaves_buffer_and_intent_stream_clean() {
    let (mut app, wires) = drawer_canvas_app_with_wires();
    stamp_optimistic_stroke(&mut app, (CANVAS_H / 2) as i32);
    assert!(
        inked_pixels(&app) > 0,
        "optimistic ink present before clear"
    );

    enqueue(&mut app, Msg::ClearCanvas);
    settle(&mut app, 4);
    assert_eq!(inked_pixels(&app), 0, "Clear blanked the local buffer");

    enqueue(&mut app, Msg::UndoStroke);
    settle(&mut app, 4);
    assert_eq!(
        inked_pixels(&app),
        0,
        "Undo after a non-undoable Clear did NOT resurrect the drawing"
    );
    let sent = wires.sent.borrow();
    let clears = sent
        .iter()
        .filter(|i| matches!(i, ClientIntent::Clear))
        .count();
    let undos = sent
        .iter()
        .filter(|i| matches!(i, ClientIntent::Undo))
        .count();
    assert_eq!(clears, 1, "exactly one Clear intent reached the wire");
    assert_eq!(
        undos, 0,
        "no Undo intent reached the wire (the local pop found an empty ring)"
    );
}

/// I-2: a mash-undo past the undo-ring floor stops relaying — the Undo intent count
/// equals the number of successful local pops (the ring depth), so the local pixels
/// stay consistent with what the intent stream implies (no over-removal on the server).
#[test]
fn mash_undo_stops_relaying_at_the_ring_floor() {
    let (mut app, wires) = drawer_canvas_app_with_wires();
    // 13 undoable strokes; the ring holds UNDO_DEPTH (12), so the oldest is dropped.
    for i in 0..13 {
        stamp_optimistic_stroke(&mut app, 40 + i * 10);
    }
    // 13 separate Undo clicks (the seq counter collapses same-frame bumps into one
    // pop, so each Undo needs its own frame).
    for _ in 0..13 {
        enqueue(&mut app, Msg::UndoStroke);
        settle(&mut app, 3);
    }
    assert_eq!(
        undo_intent_count(&wires),
        12,
        "exactly UNDO_DEPTH (12) Undo intents relayed — the 13th (empty ring) sent none"
    );
}

// ---------------------------------------------------------------------------
// The net pump (drain_client_net) — progress-overlay wipe triggers (test gap 1).
// ---------------------------------------------------------------------------

/// A GPU-free app with `NetPlugin` (the pump) fed by a [`TestTransport`], so
/// `drain_client_net` runs against a scripted `ServerEvent` queue.
fn pump_app_with_wires() -> (App, TestWires) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(buiy::BuiyProbePlugin);
    app.init_asset::<Image>();
    dooduel::install(&mut app);
    app.add_plugins(dooduel::net::NetPlugin);
    let wires = TestWires::default();
    // Replace NetPlugin's None transport with the scripted one.
    app.insert_non_send(ClientNet(Some(Box::new(TestTransport {
        wires: wires.clone(),
    }))));
    settle(&mut app, 8);
    (app, wires)
}

fn push_event(wires: &TestWires, ev: ServerEvent) {
    wires.recv.borrow_mut().push_back(ev);
}

fn progress_active(app: &App) -> bool {
    !app.world().resource::<CanvasProgress>().points.is_empty()
}

fn four_players_drawer1_disconnected(disconnect_drawer: bool) -> Vec<ReplicaPlayer> {
    (0..4)
        .map(|i| ReplicaPlayer {
            name: format!("P{i}"),
            avatar: WireAvatar::Default,
            connected: !(disconnect_drawer && i == 1),
            is_bot: i != 0,
            score: 0,
            guessed: false,
        })
        .collect()
}

/// The pump fills the [`CanvasProgress`] overlay from `CanvasStrokeProgress` and wipes
/// it on BOTH triggers (test gap 1): any authoritative canvas event, AND a `Roster`
/// showing the drawer disconnected (the silent-discard path — previously uncovered).
#[test]
fn pump_fills_and_wipes_the_progress_overlay() {
    let (mut app, wires) = pump_app_with_wires();
    // Establish the drawer as seat 1 (so the Roster-disconnect check has a target).
    push_event(
        &wires,
        ServerEvent::PhaseChanged {
            phase: dooduel::game::Phase::Drawing,
            drawer: Some(1),
            round: 1,
            total_rounds: 2,
            remaining: std::time::Duration::from_secs(60),
        },
    );
    settle(&mut app, 4);

    let progress = |sid: u64| ServerEvent::CanvasStrokeProgress {
        stroke_id: sid,
        points: vec![(10, 10), (20, 20), (30, 30)],
        color: [0, 0, 0, 255],
        radius: 3,
    };

    // Trigger A — an authoritative canvas event wipes the live overlay.
    push_event(&wires, progress(1));
    settle(&mut app, 2);
    assert!(progress_active(&app), "the overlay filled from progress");
    push_event(&wires, ServerEvent::CanvasCleared);
    settle(&mut app, 2);
    assert!(
        !progress_active(&app),
        "an authoritative canvas event wiped the overlay"
    );

    // Trigger B — a Roster showing the drawer (seat 1) disconnected wipes it (the
    // silent-discard path sends no CanvasUndo, so the Roster is the only signal).
    push_event(&wires, progress(2));
    settle(&mut app, 2);
    assert!(progress_active(&app), "the overlay re-filled from progress");
    push_event(
        &wires,
        ServerEvent::Roster {
            players: four_players_drawer1_disconnected(true),
            host: 0,
        },
    );
    settle(&mut app, 2);
    assert!(
        !progress_active(&app),
        "a Roster showing the drawer disconnected wiped the overlay"
    );
}

// ---------------------------------------------------------------------------
// Guesser re-raster on CanvasUndo (test gap 2).
// ---------------------------------------------------------------------------

/// A GPU-free canvas app seated as a guesser (seat 1; seat 0 draws) in Drawing.
fn guesser_drawing_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(buiy::BuiyProbePlugin);
    app.init_asset::<Image>();
    dooduel::install(&mut app);
    app.add_plugins(dooduel::paint::CanvasPlugin);
    settle(&mut app, 12);
    enqueue(
        &mut app,
        Msg::Net(ServerEvent::Welcome {
            seat: 1,
            room_code: "SOLO".to_string(),
            reconnect_token: String::new(),
            protocol_version: dooduel_core::protocol::PROTOCOL_VERSION,
        }),
    );
    enqueue(
        &mut app,
        Msg::Net(ServerEvent::PhaseChanged {
            phase: dooduel::game::Phase::Drawing,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: std::time::Duration::from_secs(60),
        }),
    );
    settle(&mut app, 4);
    app
}

/// A full-width horizontal stroke op at row `y` (dense id `id`).
fn band_op(id: u64, y: i32) -> CanvasOp {
    CanvasOp::Stroke {
        id,
        points: (40..680).step_by(6).map(|x| (x, y)).collect(),
        color: [20, 20, 24, 255],
        radius: 4,
    }
}

/// A guesser (not the drawer) re-renders its canvas from the authoritative log on a
/// `CanvasUndo`: two ops paint the top + bottom bands; undoing the bottom op removes
/// its ink (the client re-rasters the shortened log).
#[test]
fn guesser_re_rasters_on_canvas_undo() {
    let mut app = guesser_drawing_app();
    let net = |app: &mut App, ev: ServerEvent| enqueue(app, Msg::Net(ev));

    let band = |id: u64, y: i32| CanvasOp::Stroke {
        id,
        points: (40..680).step_by(6).map(|x| (x, y)).collect(),
        color: [20, 20, 24, 255],
        radius: 4,
    };
    let top_y = CANVAS_H / 4;
    let bottom_y = 3 * CANVAS_H / 4;

    net(
        &mut app,
        ServerEvent::CanvasOpApplied {
            op: band(0, top_y as i32),
        },
    );
    net(
        &mut app,
        ServerEvent::CanvasOpApplied {
            op: band(1, bottom_y as i32),
        },
    );
    settle(&mut app, 4);
    assert_ne!(
        game_pixel(&mut app, CANVAS_W / 2, top_y),
        PAPER,
        "the top band is inked"
    );
    assert_ne!(
        game_pixel(&mut app, CANVAS_W / 2, bottom_y),
        PAPER,
        "the bottom band is inked"
    );

    // Undo the bottom op → the guesser re-rasters the shortened log.
    net(&mut app, ServerEvent::CanvasUndo { removed_id: 1 });
    settle(&mut app, 4);
    assert_ne!(
        game_pixel(&mut app, CANVAS_W / 2, top_y),
        PAPER,
        "the top band survives the undo"
    );
    assert_eq!(
        game_pixel(&mut app, CANVAS_W / 2, bottom_y),
        PAPER,
        "the undone bottom band is gone (the guesser re-rastered on CanvasUndo)"
    );
}

// ---------------------------------------------------------------------------
// Reseed-visible raster signature — the cross-turn (len, last_op_id) degeneracy.
// ---------------------------------------------------------------------------

/// The raster re-render signature folds in the `canvas_reseeds` counter, so a mid-turn
/// reseed (`RoomState` / `CanvasLog`) re-renders EVEN when the new log coincidentally
/// shares the current log's `(len, last_op_id)`. Op ids reset per turn (dense `0,1,…`),
/// so two equal-length no-undo logs from DIFFERENT turns share that pair — a W4
/// reconnect that missed the Picking boundary would otherwise keep stale turn-N ink
/// over the turn-N+1 replica. Red evidence: without the reseed counter, the second
/// CanvasLog leaves the top (turn-N) band on screen and the bottom (turn-N+1) blank.
#[test]
fn reseed_re_renders_even_when_len_and_ids_coincide() {
    let mut app = guesser_drawing_app();
    let net = |app: &mut App, ev: ServerEvent| enqueue(app, Msg::Net(ev));
    let top_y = CANVAS_H / 4;
    let bottom_y = 3 * CANVAS_H / 4;

    // Turn-N log: two ops (dense ids 0, 1) in the TOP band.
    net(
        &mut app,
        ServerEvent::CanvasLog {
            ops: vec![band_op(0, top_y as i32), band_op(1, top_y as i32 + 8)],
        },
    );
    settle(&mut app, 4);
    assert_ne!(
        game_pixel(&mut app, CANVAS_W / 2, top_y),
        PAPER,
        "turn-N top ink is present"
    );
    assert_eq!(
        game_pixel(&mut app, CANVAS_W / 2, bottom_y),
        PAPER,
        "the bottom band is blank before the reseed"
    );

    // A reseed of the CROSS-TURN shape: two DIFFERENT ops with the SAME length + SAME
    // dense ids (0, 1) in the BOTTOM band, WITHOUT a phase boundary. The
    // `(len, last_op_id)` pair coincides with turn N — only `canvas_reseeds` differs.
    net(
        &mut app,
        ServerEvent::CanvasLog {
            ops: vec![band_op(0, bottom_y as i32), band_op(1, bottom_y as i32 + 8)],
        },
    );
    settle(&mut app, 4);
    assert_ne!(
        game_pixel(&mut app, CANVAS_W / 2, bottom_y),
        PAPER,
        "the reseed re-rendered the new (bottom) ops despite the coinciding sig"
    );
    assert_eq!(
        game_pixel(&mut app, CANVAS_W / 2, top_y),
        PAPER,
        "the stale turn-N (top) ink is gone"
    );
}

// ---------------------------------------------------------------------------
// The local canvas blanks on the turn boundary (QA cycle-1 F1b).
// ---------------------------------------------------------------------------

/// F1b: the previous turn's drawing must NOT linger through the next drawer's Picking
/// phase. The local raster used to clear only on the Drawing-phase entry
/// (`sync_tools_to_canvases`'s `drawing && !was_drawing` edge), while
/// `rerender_canvas_from_log` early-returns during Picking — so the stale drawing showed
/// (mostly under the waiting scrim) through the whole Picking phase. The fix blanks the
/// local buffer ONCE on the turn boundary (Reveal/Drawing → Picking/Idle/Final).
///
/// This is a purely LOCAL display reset: the authoritative op log (`replica.canvas_ops`)
/// is already emptied by the server's per-turn `CanvasCleared` and is untouched here, so
/// nothing is relayed and replay/late-join are unaffected. The reveal still shows the
/// finished drawing (blanking fires on leaving Reveal, not on entering it).
#[test]
fn local_canvas_blanks_on_the_turn_boundary() {
    let mut app = guesser_drawing_app();
    let net = |app: &mut App, ev: ServerEvent| enqueue(app, Msg::Net(ev));
    let mid_y = CANVAS_H / 2;

    // Ink the current turn's canvas (the drawer's stroke, echoed to this guesser).
    net(
        &mut app,
        ServerEvent::CanvasOpApplied {
            op: band_op(0, mid_y as i32),
        },
    );
    settle(&mut app, 4);
    assert!(
        inked_pixels(&app) > 500,
        "the current turn's drawing is inked"
    );

    // Turn ends → Reveal: the finished drawing STILL shows (the reveal exposes it).
    net(
        &mut app,
        ServerEvent::PhaseChanged {
            phase: dooduel::game::Phase::Reveal,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: std::time::Duration::from_secs(6),
        },
    );
    settle(&mut app, 4);
    assert!(
        inked_pixels(&app) > 500,
        "the drawing persists through the reveal"
    );

    // Next turn → Picking (a new drawer), mirroring the server's per-turn bundle
    // (`PhaseChanged(Picking)` + `CanvasCleared`). The local canvas must be blank for
    // this pick — not the previous turn's lingering drawing (F1b).
    net(
        &mut app,
        ServerEvent::PhaseChanged {
            phase: dooduel::game::Phase::Picking,
            drawer: Some(2),
            round: 1,
            total_rounds: 2,
            remaining: std::time::Duration::from_secs(12),
        },
    );
    net(&mut app, ServerEvent::CanvasCleared);
    settle(&mut app, 4);
    assert_eq!(
        inked_pixels(&app),
        0,
        "the local canvas blanked on the turn boundary — the next Picking shows a clean sheet"
    );
}
