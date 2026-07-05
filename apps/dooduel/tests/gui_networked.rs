//! The **networked GUI lifecycle**, driven end-to-end through REAL synthetic pointer
//! clicks against a REAL authoritative [`Session`] — the closest automated proxy for a
//! human sitting at the keyboard in the acceptance run.
//!
//! Why this tier exists: the first human playtest hit a dead screen — the host pressed
//! *Start game*, the server started the match, but the shell stayed welded to the Lobby
//! (the host became the AFK first drawer). No test caught it, because every prior test
//! either enqueued `Msg` directly (bypassing the view + picking) or rode the SOLO
//! `Msg::Play` shortcut (which sets `Screen::InGame` itself). The exact click path a
//! human performs — *Create a room* → wait in the lobby → *Start game* → expect the
//! board — was never exercised.
//!
//! This composes the three real layers together, headless (no GPU/window needed — a
//! synthetic `Window` entity + `Camera2d` + pointer, per `theme_toggle.rs`):
//!   1. the real Dooduel view + `bevy_picking` (clicks land on real widgets),
//!   2. the real MVU reducer + `NetPlugin` pump (intents out, `Msg::Net` events in),
//!   3. a real [`Session`] authority behind an [`InProcessTransport`] — the SAME
//!      loop `dooduel_server`'s room actor and the solo authority run, but honoring
//!      the Create/Join/StartMatch intents the lobby produces (solo skips them).
//!
//! The server side is [`RoomServer`], a port of `dooduel_core`'s test `Harness`: it
//! keeps the connection↔seat map (the split that keeps the `Session` transport-agnostic)
//! and is pumped by hand between `app.update()`s, so the test controls both frame
//! ordering and the authoritative clock.

use std::time::Duration;

use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};

use buiy_core::ResolvedLayout;
use buiy_core::a11y::A11yRole;
use buiy_core::a11y::translate::entity_for_node_id;
use buiy_verify::pointer::drive_stroke;

use dooduel_core::game::{Config, Phase};
use dooduel_core::protocol::{ClientIntent, PROTOCOL_VERSION, WireAvatar};
use dooduel_core::session::{Recipient, Session, SessionOpts};
use dooduel_core::transport::{
    ClientTransport, ConnId, InProcClient, InProcServer, InProcessTransport, ServerTransport,
};

use dooduel::net::{ClientNet, NetPlugin};
use dooduel::{Connect, Dooduel, Screen};

const VW: f32 = 1200.0;
const VH: f32 = 760.0;
/// The virtual-clock step the hand-pump advances per settle iteration.
const DT: Duration = Duration::from_millis(100);

// ---------------------------------------------------------------------------
// The in-process authoritative room (a port of dooduel_core's test `Harness`).
// ---------------------------------------------------------------------------

/// A real [`Session`] + the server end of an [`InProcessTransport`], driven by the same
/// pump `dooduel_server` runs: drain dropped conns → `Session::disconnect`; decode each
/// frame → `Session::connect` (Create/Join) or `Session::handle` (gameplay); flush the
/// per-recipient outbox back to connections. The connection↔seat map lives HERE, not in
/// the `Session` (spec §2.3 — the authority speaks only in seats). Version gating is a
/// pump concern and lives here too (spec §3.1).
struct RoomServer {
    session: Session,
    server: InProcServer,
    conn_seat: Vec<(ConnId, usize)>,
    now: Duration,
}

impl RoomServer {
    /// A fresh room. `fill` fills the roster to N with bots (so a lone human host still
    /// has opponents, exactly as the acceptance run's solo-plus-bots fallback does).
    fn new(fill: usize) -> Self {
        let (server, _none) = InProcessTransport::new_pair(0);
        let mut token_n: u64 = 0;
        let token_gen = Box::new(move || {
            token_n += 1;
            format!("test-token-{token_n:016x}")
        });
        let session = Session::new(
            Config::default(),
            SessionOpts {
                token_gen,
                fill_bots_to: fill,
                room_code: "TESTRM".to_string(),
                match_seed: 0x51ED_C0DE,
            },
        );
        RoomServer {
            session,
            server,
            conn_seat: Vec::new(),
            now: Duration::ZERO,
        }
    }

    /// Mint a fresh client end of the transport (a new "socket"), for a GUI client to
    /// send its Create/Join first frame through and receive events on.
    fn accept(&mut self) -> InProcClient {
        self.server.accept()
    }

    /// One pump cycle at virtual time `now`: advance the clock, intake every queued
    /// intent (admitting Create/Join, routing gameplay by seat), then fan the resulting
    /// events back to their recipients.
    fn pump(&mut self, now: Duration) {
        self.now = now;
        self.session.tick(now);
        for conn in self.server.disconnects() {
            if let Some(pos) = self.conn_seat.iter().position(|(c, _)| *c == conn) {
                let (_, seat) = self.conn_seat.remove(pos);
                self.session.disconnect(seat, self.now);
            }
        }
        while let Some((conn, intent)) = self.server.try_recv() {
            self.route(conn, intent);
        }
        self.flush();
    }

    fn route(&mut self, conn: ConnId, intent: ClientIntent) {
        match intent {
            ClientIntent::Create {
                name,
                avatar,
                protocol_version,
            } => self.admit(conn, &name, avatar, None, protocol_version),
            ClientIntent::Join {
                name,
                avatar,
                protocol_version,
                reconnect,
                ..
            } => self.admit(conn, &name, avatar, reconnect, protocol_version),
            ClientIntent::Leave => {
                if let Some(pos) = self.conn_seat.iter().position(|(c, _)| *c == conn) {
                    let (_, seat) = self.conn_seat.remove(pos);
                    self.session.handle(seat, ClientIntent::Leave);
                }
            }
            other => {
                if let Some(&(_, seat)) = self.conn_seat.iter().find(|(c, _)| *c == conn) {
                    self.session.handle(seat, other);
                }
            }
        }
    }

    fn admit(
        &mut self,
        conn: ConnId,
        name: &str,
        avatar: WireAvatar,
        reconnect: Option<String>,
        version: u32,
    ) {
        if version != PROTOCOL_VERSION {
            return; // the real server sends VersionMismatch; the GUI never sends a bad one
        }
        if let Ok(seat) = self
            .session
            .connect(name, avatar, reconnect.as_deref(), self.now)
        {
            // A live-token rejoin rebinds the seat to the new conn (spec §6.3).
            self.conn_seat.retain(|(_, s)| *s != seat);
            self.conn_seat.push((conn, seat));
        }
    }

    fn flush(&mut self) {
        let conns = self.conn_seat.clone();
        for (recip, ev) in self.session.drain_events() {
            match recip {
                Recipient::All => {
                    for (conn, _) in &conns {
                        self.server.send(*conn, &ev);
                    }
                }
                Recipient::Seat(seat) => {
                    if let Some((conn, _)) = conns.iter().find(|(_, s)| *s == seat) {
                        self.server.send(*conn, &ev);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The GUI client harness — the real view + real picking, headless.
// ---------------------------------------------------------------------------

/// Build a headless GUI app with the real view + the real `bevy_picking` stack (the
/// unified probe/picking driver from `theme_toggle.rs`), plus `NetPlugin` — but NOT
/// `LocalAuthorityPlugin`/`WsClientPlugin`, so the injected in-process transport is
/// neither nulled (solo) nor hijacked (a real socket). Returns the app + the synthetic
/// window and pointer entities the click helper drives.
fn gui_client() -> (App, Entity, Entity) {
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
    app.add_plugins(NetPlugin);
    (app, window, pointer)
}

fn model(app: &mut App) -> Dooduel {
    app.world_mut()
        .query::<&Dooduel>()
        .iter(app.world())
        .next()
        .cloned()
        .expect("model exists")
}

fn screen(app: &mut App) -> Screen {
    model(app).screen
}

/// The semantic report (Playwright-style role tree + text/layout) — the "what is on
/// screen" oracle a headless GUI test reads back.
fn report(app: &mut App) -> String {
    buiy_core::a11y::report::snapshot_report(app.world_mut())
}

/// Is a button with this exact label currently on screen (a locatable, laid-out node)?
fn has_button(app: &mut App, label: &str) -> bool {
    buiy::probe::get_by_role(app.world_mut(), A11yRole::Button, Some(label), None).is_ok()
}

/// Click the button labelled `label` through a REAL synthetic pointer click: locate it
/// in the semantic tree, resolve its laid-out center, and drive a 2-point micro-stroke
/// (press → tiny move → release) — `bevy_picking` derives the click on the press target
/// and folds the widget's `OnPress` msg. Panics if the button isn't locatable (a missing
/// button is a real failure, not a skip).
fn click_button(app: &mut App, window: Entity, pointer: Entity, label: &str) {
    let node = buiy::probe::get_by_role(app.world_mut(), A11yRole::Button, Some(label), None)
        .unwrap_or_else(|_| panic!("button {label:?} is locatable on screen"));
    let e = entity_for_node_id(node).expect("the button node maps to an entity");
    let (tl, size) = {
        let world = app.world();
        let tl = world
            .get::<GlobalTransform>(e)
            .expect("button has a GlobalTransform")
            .translation()
            .truncate();
        let size = world
            .get::<ResolvedLayout>(e)
            .expect("button has a ResolvedLayout")
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
}

/// Settle the client alone (no server), advancing enough frames for the funnel + view to
/// re-render after a local action.
fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// Settle the client AND pump the server, advancing the virtual clock — so an
/// intent→authority→event round-trip completes. Returns the clock it reached.
fn settle_net(
    app: &mut App,
    server: &mut RoomServer,
    mut now: Duration,
    frames: usize,
) -> Duration {
    for _ in 0..frames {
        app.update();
        now += DT;
        server.pump(now);
        app.update();
    }
    now
}

/// The Create/Join first frame the client's `WsClientPlugin` would send from a staged
/// `pending_connect` — replicated here (the plugin is socket-bound, so the test plays its
/// connect role). A blank name defaults to "Player" (the authority rejects empty names).
fn first_frame(m: &Dooduel) -> ClientIntent {
    let name = {
        let n = m.player_name.trim();
        if n.is_empty() { "Player" } else { n }.to_string()
    };
    match m.pending_connect.as_ref().expect("a staged connect") {
        Connect::Create => ClientIntent::Create {
            name,
            avatar: WireAvatar::Default,
            protocol_version: PROTOCOL_VERSION,
        },
        Connect::Join { code, reconnect } => ClientIntent::Join {
            room: code.clone(),
            name,
            avatar: WireAvatar::Default,
            protocol_version: PROTOCOL_VERSION,
            reconnect: reconnect.clone(),
        },
    }
}

/// Attach a fresh transport to the client and send its staged first frame — the connect
/// bootstrap the (socket-bound) `WsClientPlugin` performs on a real build.
fn connect(app: &mut App, server: &mut RoomServer) {
    let intent = first_frame(&model(app));
    let mut client = server.accept();
    client.send(&intent);
    app.insert_non_send(ClientNet(Some(Box::new(client))));
}

// ---------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------

#[test]
fn host_creates_starts_and_reaches_the_board_through_real_clicks() {
    let (mut app, window, pointer) = gui_client();
    let mut server = RoomServer::new(4); // lone human host + 3 bots
    settle(&mut app, 12);
    assert_eq!(screen(&mut app), Screen::Home, "starts on Home");

    // 1. Real click: "Create a room" → the shell stages the connect and shows the Lobby's
    //    "Connecting…" state.
    click_button(&mut app, window, pointer, "Create a room");
    settle(&mut app, 6);
    assert_eq!(
        screen(&mut app),
        Screen::Lobby,
        "Create navigates to the lobby"
    );
    assert!(
        model(&mut app).net.is_networked(),
        "the session is networked (Joining), not solo"
    );

    // 2. The connect bootstrap + the round-trip: the server admits the host, issues the
    //    room code + roster; the lobby flips from Connecting to the live room.
    connect(&mut app, &mut server);
    let now = settle_net(&mut app, &mut server, Duration::ZERO, 12);
    let m = model(&mut app);
    assert!(
        matches!(m.net, dooduel::NetState::Connected { .. }),
        "Welcome flipped the session to Connected"
    );
    assert_eq!(
        m.replica.room_code, "TESTRM",
        "the server-issued code shows"
    );
    // Pre-start the lobby shows only the connected players (bots backfill vacant seats
    // at match start, not before) — here just the lone host.
    assert_eq!(
        m.replica.players.len(),
        1,
        "the lobby shows the connected roster (just the host pre-start)"
    );
    assert_eq!(m.replica.my_seat, 0, "the host holds seat 0");
    // The host sees a real, laid-out Start button (host-gated) — proof the lobby actually
    // rendered its connected form, not the connecting spinner.
    assert!(
        has_button(&mut app, "▶ Start game"),
        "the host's Start button is on screen. Report:\n{}",
        report(&mut app)
    );

    // 3. THE REGRESSION: real click "Start game" → the match starts on the authority, and
    //    the shell must LEAVE the lobby for the board. Pre-fix this stayed on the Lobby.
    click_button(&mut app, window, pointer, "▶ Start game");
    let now = settle_net(&mut app, &mut server, now, 12);
    assert_eq!(
        screen(&mut app),
        Screen::InGame,
        "starting the match lifts the host out of the lobby onto the board",
    );
    // The host is the first drawer, mid-match on a live phase, and the bots have now
    // backfilled the vacant seats (the acceptance run's solo-plus-bots fallback).
    let m = model(&mut app);
    assert_eq!(
        m.replica.drawer,
        Some(m.replica.my_seat),
        "host draws first"
    );
    assert_ne!(m.replica.phase, Phase::Idle, "the match phase is live");
    assert_eq!(
        m.replica.players.len(),
        4,
        "the bots backfilled the roster to 4 at match start"
    );

    // And the RENDERED view actually swapped — the lobby's Start button is gone and the
    // in-game shell (its always-present Scoreboard panel) is on screen. This asserts
    // through the real semantic tree, not just the model, so a broken kind-swap (the
    // model advances but the view doesn't) would fail here.
    assert!(
        !has_button(&mut app, "▶ Start game"),
        "the lobby's Start button is gone once in-game"
    );
    assert!(
        report(&mut app).contains("Scoreboard"),
        "the in-game board rendered (its Scoreboard panel is present). Report:\n{}",
        report(&mut app)
    );

    // 4. The drawer's in-game interaction, also through a REAL click: the host is the
    //    drawer in the Picking phase (PICK_SECS is 10s, well beyond the ~1.2s settled),
    //    so its word-choice buttons are on screen. Pick the first one and confirm the
    //    match advances to Drawing with the word now known to the drawer.
    let m = model(&mut app);
    assert_eq!(
        m.replica.phase,
        Phase::Picking,
        "the drawer is choosing a word"
    );
    assert!(
        !m.replica.word_choices.is_empty(),
        "the drawer received its word choices"
    );
    let word = m.replica.word_choices[0].to_uppercase();
    click_button(&mut app, window, pointer, &word);
    settle_net(&mut app, &mut server, now, 12);
    let m = model(&mut app);
    assert_eq!(
        m.replica.phase,
        Phase::Drawing,
        "picking a word begins the drawing phase"
    );
    assert!(
        m.replica.word_len > 0,
        "the drawer now knows the word (its length is set)"
    );
}
