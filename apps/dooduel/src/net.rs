//! The client's network seam (spec §4.2, §8) — how the MVU shell reaches an
//! authoritative [`Session`], solo or (in W4) over a WebSocket.
//!
//! The organizing idea (spec §2): the authority is transport-agnostic. The client
//! never mutates game state; it **sends [`ClientIntent`]s** and **folds
//! [`ServerEvent`]s** back as `Msg::Net`. Two plugins wire that:
//!
//! - [`NetPlugin`] is the client-side pump, used in **both** solo and networked
//!   mode. It holds the [`ClientNet`] transport, drains the reducer's
//!   [`Dooduel::net_outbox`] intents out through it, and drains inbound events into
//!   the funnel as `Msg::Net`. The high-frequency, transient
//!   [`ServerEvent::CanvasStrokeProgress`] is siphoned off to the [`CanvasProgress`]
//!   resource (it has no replica field — the paint subsystem stamps it live).
//! - [`LocalAuthorityPlugin`] is the **solo** in-process authority (spec §8): it
//!   owns a [`Session`] behind an [`InProcessTransport`], ticks it every frame with
//!   the monotonic clock, routes the client's intents into it and its addressed
//!   events back out — the same loop shape `dooduel_server` runs (W4). It rebuilds
//!   the session (fresh match seed) whenever [`Dooduel::solo_epoch`] bumps.
//!
//! Both the transport and the solo authority are `Rc`-backed (single-threaded), so
//! they live as **`NonSend`** resources; a GUI Dooduel app runs their pumps on the
//! main thread (the same discipline the a11y `NonSend` pins use). The W4
//! `WsClientTransport` is `Send`, but a `Send` value in a `NonSend` slot is fine —
//! keeping one [`ClientNet`] type spanning both modes.

use std::time::Duration;

use bevy::prelude::*;
use buiy_core::mvu::{MvuSet, enqueue};

use dooduel_core::game::Config;
use dooduel_core::protocol::{ClientIntent, PROTOCOL_VERSION, ServerEvent, WireAvatar};
use dooduel_core::session::{Recipient, Session, SessionOpts};
use dooduel_core::transport::{
    ClientTransport, ConnId, ConnStatus, InProcClient, InProcServer, InProcessTransport,
    ServerTransport, WsClientTransport,
};

use crate::{Connect, Dooduel, HumanAvatar, Msg, NetState};

/// The client-side transport the reducer's intents leave through and the pump drains
/// events from (spec §2.4). `None` until a session exists (Home / before ▶ Play). A
/// `NonSend` resource — the solo [`InProcClient`] is `Rc`-backed (single-threaded);
/// the W4 `WsClientTransport` is `Send`, but lives here too so one type spans both.
pub struct ClientNet(pub Option<Box<dyn ClientTransport>>);

/// The transient in-progress stroke a guesser is watching grow (spec §3.5, W2-review
/// I6). Kept **off the model** (a resource, not replica state): a
/// [`ServerEvent::CanvasStrokeProgress`] arrives every ~30–60 ms, so folding it
/// through the funnel would churn `view()` per batch. The paint subsystem stamps
/// [`points`](Self::points) live and re-rasterizes the authoritative log on top;
/// it is wiped on any authoritative canvas event or a drawer-disconnect roster.
#[derive(Resource, Default)]
pub struct CanvasProgress {
    /// The client-batching handle of the stroke being accumulated (`None` = idle).
    pub stroke_id: Option<u64>,
    /// The exact points accumulated so far (post-`to_pixel`, integer canvas coords).
    pub points: Vec<(i32, i32)>,
    pub color: [u8; 4],
    pub radius: i32,
    /// Bumped on every change (a batch appended, or a wipe) so the paint subsystem
    /// re-stamps exactly when the overlay changed — never per steady frame.
    pub generation: u64,
}

impl CanvasProgress {
    /// Accumulate one live batch (starting fresh when the `stroke_id` changes).
    fn extend(&mut self, stroke_id: u64, points: &[(i32, i32)], color: [u8; 4], radius: i32) {
        if self.stroke_id != Some(stroke_id) {
            self.stroke_id = Some(stroke_id);
            self.points.clear();
        }
        self.points.extend_from_slice(points);
        self.color = color;
        self.radius = radius;
        self.generation = self.generation.wrapping_add(1);
    }

    /// Wipe the overlay (an authoritative canvas event or a drawer disconnect).
    fn wipe(&mut self) {
        if self.stroke_id.is_some() || !self.points.is_empty() {
            self.stroke_id = None;
            self.points.clear();
            self.generation = self.generation.wrapping_add(1);
        }
    }
}

/// Orders the per-frame net pump inside [`MvuSet::Enqueue`]: stage the reducer's
/// outbound intents, run the (solo) authority, then drain inbound events into the
/// funnel — so a solo intent→event round-trip can complete within a frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum NetSet {
    /// Send [`Dooduel::net_outbox`] intents out through [`ClientNet`].
    Outbox,
    /// The solo authority tick + intent/event routing ([`LocalAuthorityPlugin`]).
    Pump,
    /// Drain inbound [`ServerEvent`]s → `Msg::Net` (+ the [`CanvasProgress`] siphon).
    Drain,
}

/// The client-side transport pump (spec §4.2). Solo and networked both use it.
pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(ClientNet(None));
        app.init_resource::<CanvasProgress>();
        app.configure_sets(
            Update,
            (NetSet::Outbox, NetSet::Pump, NetSet::Drain)
                .chain()
                .in_set(MvuSet::Enqueue),
        );
        app.add_systems(Update, drain_outbox.in_set(NetSet::Outbox));
        app.add_systems(Update, drain_client_net.in_set(NetSet::Drain));
    }
}

/// The in-process solo authority (spec §8). Requires [`NetPlugin`] (it owns the
/// [`ClientNet`] the authority feeds + the pump ordering).
pub struct LocalAuthorityPlugin;

impl Plugin for LocalAuthorityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(SoloAuthority(None));
        app.add_systems(Update, pump_local_authority.in_set(NetSet::Pump));
    }
}

/// Send the reducer's staged intents (spec §4.2). The reducer is pure, so it appends
/// to [`Dooduel::net_outbox`]; this drains new entries (a `Local` cursor) out through
/// the transport. The outbox is append-only within a session, so the cursor advances
/// monotonically — a solo rebuild swaps the transport, and only post-rebuild intents
/// (past the cursor) reach the new one.
fn drain_outbox(mut net: NonSendMut<ClientNet>, model: Query<&Dooduel>, mut cursor: Local<usize>) {
    let Ok(model) = model.single() else {
        return;
    };
    let Some(transport) = net.0.as_mut() else {
        // No transport yet — stay in step with the outbox so nothing double-sends
        // once one appears (pre-session intents are not produced anyway).
        *cursor = model.net_outbox.len();
        return;
    };
    if *cursor > model.net_outbox.len() {
        *cursor = 0; // defensive: the outbox was reset out from under us
    }
    for intent in &model.net_outbox[*cursor..] {
        transport.send(intent);
    }
    *cursor = model.net_outbox.len();
}

/// Drain inbound events into the funnel as `Msg::Net`, siphoning the transient
/// [`ServerEvent::CanvasStrokeProgress`] to [`CanvasProgress`] and wiping that
/// overlay on any authoritative canvas event or a drawer-disconnect roster (the
/// two wipe triggers, spec §3.5 / R1 — the silent-discard paths send no
/// `CanvasUndo`, so the roster is the only signal there).
fn drain_client_net(
    mut net: NonSendMut<ClientNet>,
    mut progress: ResMut<CanvasProgress>,
    model: Query<(Entity, &Dooduel)>,
    mut commands: Commands,
) {
    let Some(transport) = net.0.as_mut() else {
        return;
    };
    let Ok((entity, model)) = model.single() else {
        return;
    };
    while let Some(ev) = transport.try_recv() {
        match &ev {
            ServerEvent::CanvasStrokeProgress {
                stroke_id,
                points,
                color,
                radius,
            } => {
                // Transient — paint it live, never fold it (no replica field).
                progress.extend(*stroke_id, points, *color, *radius);
                continue;
            }
            ServerEvent::CanvasOpApplied { .. }
            | ServerEvent::CanvasUndo { .. }
            | ServerEvent::CanvasCleared
            | ServerEvent::CanvasLog { .. } => progress.wipe(),
            ServerEvent::Roster { players, .. } => {
                // The drawer-drop / vacate discard paths emit no canvas event, so a
                // roster showing the drawer disconnected is the progress wipe there.
                if let Some(drawer) = model.replica.drawer
                    && players.get(drawer).is_some_and(|p| !p.connected)
                {
                    progress.wipe();
                }
            }
            _ => {}
        }
        enqueue::<Dooduel>(&mut commands, entity, Msg::Net(ev));
    }
}

// ---------------------------------------------------------------------------
// The solo in-process authority (spec §8).
// ---------------------------------------------------------------------------

/// The solo authority holder — `None` until the first ▶ Play. A `NonSend` resource
/// (the [`InProcServer`] is `Rc`-backed).
struct SoloAuthority(Option<Solo>);

/// One in-process authority: the [`Session`], the server end of its
/// [`InProcessTransport`], and the seat↔connection binding the pump keeps (the
/// `Session` speaks only in seats, spec §2.3).
struct Solo {
    session: Session,
    server: InProcServer,
    /// `(connection, seat)` for every seated connection — solo has exactly one (the
    /// human); the bots are seats with no connection, so `Recipient::All` reaches
    /// only the human, and per-seat bot events drop at routing.
    seat_of_conn: Vec<(ConnId, usize)>,
    /// The [`Dooduel::solo_epoch`] this session was built for.
    epoch: u64,
}

impl Solo {
    /// Fan the session's staged events out to the client inbox(es). `All` reaches
    /// every seated connection; `Seat(n)` reaches seat `n`'s connection (dropped if
    /// it has none — a bot seat).
    fn route_events(&mut self) {
        let conns: Vec<(ConnId, usize)> = self.seat_of_conn.clone();
        for (recipient, ev) in self.session.drain_events() {
            match recipient {
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

/// Tick the solo authority and shuttle intents/events (spec §8) — the same loop the
/// server runs, embedded. Rebuilds on a fresh [`Dooduel::solo_epoch`]; tears down
/// when the model leaves [`NetState::Solo`].
fn pump_local_authority(
    mut solo: NonSendMut<SoloAuthority>,
    mut net: NonSendMut<ClientNet>,
    model: Query<&Dooduel>,
    time: Res<Time>,
) {
    let Ok(model) = model.single() else {
        return;
    };
    let now = time.elapsed();

    if model.net != NetState::Solo {
        // Left solo (Back / a networked join): drop the authority + its transport.
        if solo.0.is_some() {
            solo.0 = None;
            net.0 = None;
        }
        return;
    }

    // (Re)build on a fresh epoch (▶ Play / Lobby Start / Play again). The connect +
    // StartMatch events are staged into the client inbox here; they flow next frame.
    let rebuild = match &solo.0 {
        Some(s) => s.epoch != model.solo_epoch,
        None => true,
    };
    if rebuild {
        let (built, client) = build_solo(model.solo_epoch, &model.player_name, now);
        solo.0 = Some(built);
        net.0 = Some(Box::new(client));
        return;
    }

    let solo = solo
        .0
        .as_mut()
        .expect("solo authority present after rebuild");
    // 1. Advance the authoritative clock (countdown / hints / auto-pick / bot drain).
    solo.session.tick(now);
    // 2. Intake the client's intents. Solo connects the human directly, so a
    //    Create/Join here would be spurious — ignore it (never panic on input).
    while let Some((conn, intent)) = solo.server.try_recv() {
        let seat = solo
            .seat_of_conn
            .iter()
            .find(|(c, _)| *c == conn)
            .map(|(_, s)| *s);
        match intent {
            ClientIntent::Create { .. } | ClientIntent::Join { .. } => {}
            other => {
                if let Some(seat) = seat {
                    solo.session.handle(seat, other);
                }
            }
        }
    }
    // 3. Fan the resulting events back out to the client inbox.
    solo.route_events();
}

/// Build a fresh solo authority: an [`InProcessTransport`] pair, a [`Session`] with
/// injected policy (a deterministic token generator, a monotonic-clock-derived match seed
/// — solo is not adversarial, but the default constant seed is a redaction target so
/// we avoid it, spec §4.1 — and `fill_bots_to: 4`), the human connected at seat 0,
/// and `StartMatch` fired (solo bypasses the lobby, spec §8). Returns the authority
/// and the client transport (both share one mailbox).
fn build_solo(epoch: u64, human_name: &str, now: Duration) -> (Solo, InProcClient) {
    let (server, mut clients) = InProcessTransport::new_pair(1);
    let client = clients.pop().expect("one client end");
    let conn = client.conn();

    // A deterministic per-call token generator (solo is reproducible; the server
    // uses getrandom instead, spec §6.3).
    let mut token_n: u64 = 0;
    let token_gen = Box::new(move || {
        token_n += 1;
        format!("solo-token-{token_n:016x}")
    });
    let opts = SessionOpts {
        token_gen,
        fill_bots_to: 4,
        room_code: "SOLO".to_string(),
        match_seed: solo_match_seed(epoch, now),
    };
    let mut session = Session::new(Config::default(), opts);

    let name = if human_name.trim().is_empty() {
        // The authority rejects an empty name (R4) — default a blank solo name.
        "You"
    } else {
        human_name.trim()
    };
    let seat = session
        .connect(name, WireAvatar::Default, None, now)
        .expect("solo connect (non-empty name, no reconnect) succeeds");
    session.handle(seat, ClientIntent::StartMatch);

    let mut solo = Solo {
        session,
        server,
        seat_of_conn: vec![(conn, seat)],
        epoch,
    };
    solo.route_events();
    (solo, client)
}

/// A per-match PRNG seed for a solo session. Solo is not adversarial, but the match
/// seed is a redaction target (spec §4.1), so we do NOT reuse the constant
/// `DEFAULT_MATCH_SEED`; we mix the **monotonic** clock at match creation (`now`, the
/// nanos of `Time::elapsed` — which differs per session/press and is wasm-safe, unlike
/// `SystemTime::now()` on `wasm32-unknown-unknown`) with the epoch (distinct per match
/// in a session). This is a **live** value — replay re-feeds the recorded `Msg::Net`
/// stream (never a re-run session), so it does not affect replay determinism.
fn solo_match_seed(epoch: u64, now: Duration) -> u64 {
    (now.as_nanos() as u64) ^ epoch.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

// ---------------------------------------------------------------------------
// The networked WebSocket client (spec §4.2, §6) — the production Create/Join path.
// ---------------------------------------------------------------------------

/// The default `dooduel_server` URL (matches the server's default `127.0.0.1:7878`).
const DEFAULT_SERVER_URL: &str = "ws://127.0.0.1:7878";

/// Opens + tears down the networked [`WsClientTransport`] (spec §4.2). Requires
/// [`NetPlugin`] (it drives the shared [`ClientNet`] the transport lives in). It is the
/// mirror of [`LocalAuthorityPlugin`] for the networked states: when the model is
/// `Joining`/`Connected`/`Dropped` with a staged [`Dooduel::pending_connect`], it opens a
/// socket to `server_url` and sends the `Create`/`Join` first frame; it detects a
/// dropped connection and asks the reducer to re-attach with the reconnect token.
pub struct WsClientPlugin;

impl Plugin for WsClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, manage_ws_client.in_set(NetSet::Pump));
    }
}

/// Bounded auto-rejoin on a dropped connection (spec §6.3): a few tries with a short
/// backoff, then give up gracefully rather than spin forever (I-1).
const MAX_REJOIN_TRIES: u32 = 3;
const REJOIN_BACKOFF: Duration = Duration::from_secs(3);

/// The networked transport's per-frame lifecycle state (a `Local`). `owns` tracks
/// whether THIS system installed the current [`ClientNet`] transport (so teardown never
/// clobbers the solo [`InProcClient`] the [`LocalAuthorityPlugin`] owns); the rejoin
/// fields bound the auto-reconnect.
#[derive(Default)]
struct WsClientState {
    owns: bool,
    rejoin_failures: u32,
    /// The monotonic instant the next rejoin attempt may start (the backoff gate).
    next_attempt_at: Option<Duration>,
}

/// The action [`manage_ws_client`] takes this frame. Pure + exhaustive so every
/// connect/reconnect edge — including the failure paths I-1 flagged — has a defined
/// outcome and a unit test (no live socket needed to exercise them).
#[derive(Debug, PartialEq)]
enum WsAction {
    /// Nothing to do (still connecting, healthy, or backing off).
    Idle,
    /// A `Joining` transport closed without connecting → surface a connect failure.
    ConnectFailed,
    /// A `Connected` transport dropped → go Dropped + stage a token rejoin.
    Dropped,
    /// A `Dropped` rejoin attempt closed, tries remain → tear down + back off.
    RetryRejoin,
    /// A `Dropped` rejoin exhausted its tries → give up gracefully.
    GiveUp,
    /// No transport + a pending connect + the backoff elapsed → open one.
    Connect,
}

/// Decide the transport action from the observable state (spec §4.2/§6.3). `status` is
/// the current transport's [`ConnStatus`], or `None` when there is no transport. A
/// transport WE own reporting `Closed` routes by the state it closed in — that closes
/// the I-1 dead-ends: a `Joining` socket that never opens → `ConnectFailed`, a `Dropped`
/// rejoin that keeps failing → `RetryRejoin` until exhausted → `GiveUp`.
fn ws_decision(
    net: &NetState,
    owns: bool,
    status: Option<ConnStatus>,
    has_pending: bool,
    tries_exhausted: bool,
    backoff_elapsed: bool,
) -> WsAction {
    if !net.is_networked() {
        return WsAction::Idle;
    }
    if owns && status == Some(ConnStatus::Closed) {
        return match net {
            NetState::Connected { .. } => WsAction::Dropped,
            NetState::Joining => WsAction::ConnectFailed,
            NetState::Dropped { .. } if tries_exhausted => WsAction::GiveUp,
            NetState::Dropped { .. } => WsAction::RetryRejoin,
            _ => WsAction::Idle,
        };
    }
    if status.is_none() && has_pending && backoff_elapsed {
        return WsAction::Connect;
    }
    WsAction::Idle
}

/// Own the networked transport's lifecycle (spec §4.2, §6.3): open it for a staged
/// connect, tear it down on leave, and route a closed connection through [`ws_decision`]
/// — a `Joining` failure, a `Connected` drop, or a bounded `Dropped` rejoin — so no
/// connect/reconnect path dead-ends (I-1).
fn manage_ws_client(
    mut net: NonSendMut<ClientNet>,
    model: Query<(Entity, &Dooduel)>,
    time: Res<Time>,
    mut commands: Commands,
    mut state: Local<WsClientState>,
) {
    let Ok((entity, model)) = model.single() else {
        return;
    };
    let now = time.elapsed();

    // Leaving networked play: drop OUR transport (never the solo one) + reset the rejoin
    // budget. Solo's in-process transport is the LocalAuthorityPlugin's business.
    if !model.net.is_networked() {
        if state.owns {
            net.0 = None;
            state.owns = false;
        }
        state.rejoin_failures = 0;
        state.next_attempt_at = None;
        return;
    }

    let status = net.0.as_ref().map(|t| t.status());
    // A healthy live connection resets the rejoin budget (a later drop starts fresh).
    if matches!(model.net, NetState::Connected { .. }) && status == Some(ConnStatus::Open) {
        state.rejoin_failures = 0;
        state.next_attempt_at = None;
    }

    let tries_exhausted = state.rejoin_failures + 1 >= MAX_REJOIN_TRIES;
    let backoff_elapsed = state.next_attempt_at.is_none_or(|t| now >= t);
    match ws_decision(
        &model.net,
        state.owns,
        status,
        model.pending_connect.is_some(),
        tries_exhausted,
        backoff_elapsed,
    ) {
        WsAction::Idle => {}
        WsAction::Dropped => {
            net.0 = None;
            state.owns = false;
            enqueue::<Dooduel>(&mut commands, entity, Msg::NetDropped);
        }
        WsAction::ConnectFailed => {
            net.0 = None;
            state.owns = false;
            enqueue::<Dooduel>(
                &mut commands,
                entity,
                Msg::ConnectFailed("the connection closed".to_string()),
            );
        }
        WsAction::RetryRejoin => {
            net.0 = None;
            state.owns = false;
            state.rejoin_failures += 1;
            state.next_attempt_at = Some(now + REJOIN_BACKOFF);
        }
        WsAction::GiveUp => {
            net.0 = None;
            state.owns = false;
            enqueue::<Dooduel>(&mut commands, entity, Msg::NetGaveUp);
        }
        WsAction::Connect => {
            state.next_attempt_at = None;
            if let Some(req) = &model.pending_connect {
                match WsClientTransport::connect(server_url()) {
                    Ok(mut transport) => {
                        transport.send(&connect_intent(req, model));
                        net.0 = Some(Box::new(transport));
                        state.owns = true;
                    }
                    Err(err) => enqueue::<Dooduel>(&mut commands, entity, Msg::ConnectFailed(err)),
                }
            }
        }
    }
}

/// The server URL: `DOODUEL_SERVER_URL` (native) or the default (wasm has no env — a web
/// deployment configures the URL out of band; native + LAN is the M1 acceptance target).
fn server_url() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("DOODUEL_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        DEFAULT_SERVER_URL.to_string()
    }
}

/// Build the first-frame intent for a staged connect (spec §3.2). The player name
/// defaults to a placeholder if blank (the authority rejects an empty name).
fn connect_intent(req: &Connect, model: &Dooduel) -> ClientIntent {
    let name = {
        let n = model.player_name.trim();
        if n.is_empty() {
            "Player".to_string()
        } else {
            n.to_string()
        }
    };
    let avatar = wire_avatar(model.avatar.kind);
    match req {
        Connect::Create => ClientIntent::Create {
            name,
            avatar,
            protocol_version: PROTOCOL_VERSION,
        },
        Connect::Join { code, reconnect } => ClientIntent::Join {
            room: code.clone(),
            name,
            avatar,
            protocol_version: PROTOCOL_VERSION,
            reconnect: reconnect.clone(),
        },
    }
}

/// Map the client's chosen avatar to its wire form. M1 sends `Default`/`Preset`; the
/// custom drawn PNG is NOT sent over the wire yet (the solo path also uses `Default`, so
/// other players see the name-hashed doodle) — a faithful custom-avatar upload is a
/// deferred M1 tail (it needs the saved PNG bytes off the paint surface).
fn wire_avatar(kind: HumanAvatar) -> WireAvatar {
    match kind {
        HumanAvatar::Default => WireAvatar::Default,
        HumanAvatar::Preset { icon, tint } => WireAvatar::Preset { icon, tint },
        HumanAvatar::Custom => WireAvatar::Default,
    }
}

#[cfg(test)]
mod ws_decision_tests {
    //! The connect/reconnect edge machine (I-1) — every failure path has a defined
    //! outcome, driven by feeding `ws_decision` the transport status directly (no live
    //! socket: a test transport would only ever hand us this same `ConnStatus`).
    use super::*;

    fn connected() -> NetState {
        NetState::Connected {
            room: "ABCDEF".to_string(),
        }
    }
    fn dropped() -> NetState {
        NetState::Dropped {
            token: "tok".to_string(),
        }
    }

    #[test]
    fn a_joining_transport_that_closes_is_a_connect_failure_not_a_dead_end() {
        assert_eq!(
            ws_decision(
                &NetState::Joining,
                true,
                Some(ConnStatus::Closed),
                true,
                false,
                true
            ),
            WsAction::ConnectFailed,
        );
    }

    #[test]
    fn a_connected_transport_that_closes_is_a_drop() {
        assert_eq!(
            ws_decision(
                &connected(),
                true,
                Some(ConnStatus::Closed),
                false,
                false,
                true
            ),
            WsAction::Dropped,
        );
    }

    #[test]
    fn a_dropped_rejoin_retries_until_exhausted_then_gives_up() {
        // Tries remain → back off + retry; the last try's failure → give up.
        assert_eq!(
            ws_decision(
                &dropped(),
                true,
                Some(ConnStatus::Closed),
                true,
                false,
                true
            ),
            WsAction::RetryRejoin,
        );
        assert_eq!(
            ws_decision(&dropped(), true, Some(ConnStatus::Closed), true, true, true),
            WsAction::GiveUp,
        );
    }

    #[test]
    fn no_transport_with_a_pending_connect_opens_one_only_after_backoff() {
        assert_eq!(
            ws_decision(&NetState::Joining, false, None, true, false, true),
            WsAction::Connect,
        );
        // Still backing off → Idle (don't hammer the server between rejoin tries).
        assert_eq!(
            ws_decision(&NetState::Joining, false, None, true, false, false),
            WsAction::Idle,
        );
        // No pending connect → nothing to open.
        assert_eq!(
            ws_decision(&NetState::Joining, false, None, false, false, true),
            WsAction::Idle,
        );
    }

    #[test]
    fn a_healthy_or_still_connecting_transport_is_idle() {
        assert_eq!(
            ws_decision(
                &connected(),
                true,
                Some(ConnStatus::Open),
                false,
                false,
                true
            ),
            WsAction::Idle,
        );
        assert_eq!(
            ws_decision(
                &NetState::Joining,
                true,
                Some(ConnStatus::Connecting),
                false,
                false,
                true
            ),
            WsAction::Idle,
        );
    }

    #[test]
    fn offline_or_solo_is_always_idle() {
        assert_eq!(
            ws_decision(
                &NetState::Offline,
                true,
                Some(ConnStatus::Closed),
                true,
                true,
                true
            ),
            WsAction::Idle,
        );
        assert_eq!(
            ws_decision(
                &NetState::Solo,
                true,
                Some(ConnStatus::Closed),
                true,
                true,
                true
            ),
            WsAction::Idle,
        );
    }
}
