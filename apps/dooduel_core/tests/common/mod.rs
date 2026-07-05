//! The in-process transport pump (the "glue") shared by the W2 integration tests.
//!
//! It is the same loop `dooduel_server`'s room actor and the solo `LocalAuthorityPlugin`
//! run (spec §6.1, §8): drain dropped connections → `Session::disconnect`; decode each
//! frame → `Session::connect` (Create/Join) or `Session::handle` (gameplay); flush the
//! `Session`'s per-recipient outbox back to connections. The **connection↔seat map lives
//! here, not in the `Session`** — that is the split that keeps the authority
//! transport-agnostic. Version gating (spec §3.1) is a pump/registry concern and lives
//! here too.

#![allow(dead_code)] // each integration test binary uses a different subset.

use std::collections::BTreeMap;
use std::time::Duration;

use dooduel_core::game::{Config, DEFAULT_MATCH_SEED};
use dooduel_core::protocol::{
    CanvasOp, ClientIntent, ErrorCode, PROTOCOL_VERSION, ServerEvent, WireAvatar,
};
use dooduel_core::session::{Recipient, Session, SessionOpts};
use dooduel_core::transport::{
    ClientTransport, ConnId, InProcClient, InProcServer, InProcessTransport, ServerTransport,
};

/// The room code every harness session uses.
pub const ROOM: &str = "ROOM01";

/// A deterministic, counter-based token generator (tests never use entropy, spec §6.3).
pub fn counter_tokens() -> Box<dyn FnMut() -> String + Send> {
    let mut n = 0u64;
    Box::new(move || {
        n += 1;
        format!("tok-{n}")
    })
}

/// A `Session` + `InProcessTransport` driven by the shared pump, recording every event
/// each connection receives (so the tests can scan per-recipient streams).
pub struct Harness {
    session: Session,
    server: InProcServer,
    clients: BTreeMap<ConnId, InProcClient>,
    conn_seat: BTreeMap<ConnId, usize>,
    seat_conn: BTreeMap<usize, ConnId>,
    log: BTreeMap<ConnId, Vec<ServerEvent>>,
    now: Duration,
    match_seed: u64,
}

impl Harness {
    /// A fresh lobby harness. `fill` is `SessionOpts::fill_bots_to`.
    pub fn new(config: Config, fill: usize) -> Self {
        let match_seed = DEFAULT_MATCH_SEED;
        let (server, _none) = InProcessTransport::new_pair(0);
        let session = Session::new(
            config,
            SessionOpts {
                token_gen: counter_tokens(),
                fill_bots_to: fill,
                room_code: ROOM.to_string(),
                match_seed,
            },
        );
        Harness {
            session,
            server,
            clients: BTreeMap::new(),
            conn_seat: BTreeMap::new(),
            seat_conn: BTreeMap::new(),
            log: BTreeMap::new(),
            now: Duration::ZERO,
            match_seed,
        }
    }

    /// The PRNG seed this harness's session started from — the W2.5 oracle replays it.
    pub fn match_seed(&self) -> u64 {
        self.match_seed
    }

    /// A new connection Creates (the first one) or Joins the room; a `reconnect` token
    /// re-attaches to a held seat. Returns the connection id.
    pub fn connect(&mut self, name: &str, reconnect: Option<String>) -> ConnId {
        let mut client = self.server.accept();
        let conn = client.conn();
        let fresh_room = self.seat_conn.is_empty() && reconnect.is_none();
        let intent = if fresh_room {
            ClientIntent::Create {
                name: name.to_string(),
                avatar: WireAvatar::Default,
                protocol_version: PROTOCOL_VERSION,
            }
        } else {
            ClientIntent::Join {
                room: ROOM.to_string(),
                name: name.to_string(),
                avatar: WireAvatar::Default,
                protocol_version: PROTOCOL_VERSION,
                reconnect,
            }
        };
        client.send(&intent);
        self.clients.insert(conn, client);
        self.log.entry(conn).or_default();
        self.pump();
        conn
    }

    /// Send a gameplay intent from a connection.
    pub fn send(&mut self, conn: ConnId, intent: ClientIntent) {
        if let Some(c) = self.clients.get_mut(&conn) {
            c.send(&intent);
        }
        self.pump();
    }

    /// Drop a connection (a simulated socket close): starts its seat's grace window.
    pub fn drop_client(&mut self, conn: ConnId) {
        if let Some(c) = self.clients.get_mut(&conn) {
            c.drop_conn();
        }
        self.pump();
    }

    /// Advance the authoritative clock to `now` (virtual time) and flush.
    pub fn tick(&mut self, now: Duration) {
        self.now = now;
        self.session.tick(now);
        self.flush();
        self.collect();
    }

    /// The events connection `conn` has received so far, in order.
    pub fn log_for(&self, conn: ConnId) -> &[ServerEvent] {
        self.log.get(&conn).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The seat a connection currently maps to (if any).
    pub fn seat_of(&self, conn: ConnId) -> Option<usize> {
        self.conn_seat.get(&conn).copied()
    }

    // --- the pump ----------------------------------------------------------

    fn pump(&mut self) {
        for conn in self.server.disconnects() {
            if let Some(seat) = self.conn_seat.remove(&conn) {
                // Only disconnect if this conn still owns the seat (a live-token
                // replacement already rebound the seat to a new conn).
                if self.seat_conn.get(&seat) == Some(&conn) {
                    self.session.disconnect(seat, self.now);
                    self.seat_conn.remove(&seat);
                }
            }
        }
        while let Some((conn, intent)) = self.server.try_recv() {
            self.route(conn, intent);
        }
        self.flush();
        self.collect();
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
                if let Some(seat) = self.conn_seat.remove(&conn) {
                    self.session.handle(seat, ClientIntent::Leave);
                    self.seat_conn.remove(&seat);
                }
            }
            other => match self.conn_seat.get(&conn).copied() {
                Some(seat) => self.session.handle(seat, other),
                None => self.server.send(
                    conn,
                    &ServerEvent::Error {
                        code: ErrorCode::Malformed,
                        message: "gameplay before join".to_string(),
                    },
                ),
            },
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
            self.server.send(
                conn,
                &ServerEvent::Error {
                    code: ErrorCode::VersionMismatch,
                    message: "protocol version mismatch".to_string(),
                },
            );
            return;
        }
        match self
            .session
            .connect(name, avatar, reconnect.as_deref(), self.now)
        {
            Ok(seat) => {
                // A live-token rejoin replaces the old connection (spec §6.3).
                if let Some(old) = self.seat_conn.insert(seat, conn)
                    && old != conn
                {
                    self.conn_seat.remove(&old);
                    self.server.close(old);
                }
                self.conn_seat.insert(conn, seat);
            }
            Err(code) => self.server.send(
                conn,
                &ServerEvent::Error {
                    code,
                    message: "join rejected".to_string(),
                },
            ),
        }
    }

    fn flush(&mut self) {
        for (recip, ev) in self.session.drain_events() {
            match recip {
                Recipient::All => {
                    let conns: Vec<ConnId> = self.seat_conn.values().copied().collect();
                    for conn in conns {
                        self.server.send(conn, &ev);
                    }
                }
                Recipient::Seat(s) => {
                    if let Some(conn) = self.seat_conn.get(&s).copied() {
                        self.server.send(conn, &ev);
                    }
                }
            }
        }
    }

    fn collect(&mut self) {
        let conns: Vec<ConnId> = self.clients.keys().copied().collect();
        for conn in conns {
            if let Some(c) = self.clients.get_mut(&conn) {
                while let Some(ev) = c.try_recv() {
                    self.log.get_mut(&conn).expect("log exists").push(ev);
                }
            }
        }
    }
}

// --- shared scan/extract helpers ------------------------------------------

/// Serialize an event to lowercase JSON (the secrecy scan's string haystack).
pub fn json_lower(ev: &ServerEvent) -> String {
    serde_json::to_string(ev)
        .expect("event serializes")
        .to_lowercase()
}

/// The drawer's word choices as last seen by connection `conn` (the drawer), if any.
pub fn last_word_choices(evs: &[ServerEvent]) -> Option<Vec<String>> {
    evs.iter().rev().find_map(|e| match e {
        ServerEvent::WordChoices { words } => Some(words.clone()),
        _ => None,
    })
}

/// The reconnect token from a connection's first `Welcome`, if any.
pub fn welcome_token(evs: &[ServerEvent]) -> Option<String> {
    evs.iter().find_map(|e| match e {
        ServerEvent::Welcome {
            reconnect_token, ..
        } => Some(reconnect_token.clone()),
        _ => None,
    })
}

/// A [`CanvasOp`]'s server-assigned id.
pub fn canvas_op_id(op: &CanvasOp) -> u64 {
    match op {
        CanvasOp::Stroke { id, .. } | CanvasOp::Fill { id, .. } => *id,
    }
}

/// Fold a connection's received events into the canvas op log a replica derives —
/// the client-side reduction (spec §2.2/§4.4) Wave 3 implements: `RoomState`/`CanvasLog`
/// seed the log, `CanvasOpApplied` appends, `CanvasUndo` removes by id, `CanvasCleared`
/// truncates. The raster is then derived from this log.
pub fn fold_canvas(evs: &[ServerEvent]) -> Vec<CanvasOp> {
    let mut ops: Vec<CanvasOp> = Vec::new();
    for e in evs {
        match e {
            ServerEvent::RoomState(r) => ops = r.canvas_ops.clone(),
            ServerEvent::CanvasLog { ops: log } => ops = log.clone(),
            ServerEvent::CanvasOpApplied { op } => ops.push(op.clone()),
            ServerEvent::CanvasUndo { removed_id } => {
                ops.retain(|o| canvas_op_id(o) != *removed_id)
            }
            ServerEvent::CanvasCleared => ops.clear(),
            _ => {}
        }
    }
    ops
}

/// Whether `ev` is the point at which `seat` *earns* the right to see the word — its
/// own correct `GuessResult` (spec §5.1's `guessed_correctly(seat)`) or the turn's
/// broadcast `TurnEnded` reveal. This is the scan's per-turn cutoff, tied to the seat
/// having actually earned it — **not** to the event carrying the word (a redaction bug
/// leaks the full word to a seat that has *not* earned it, and must be caught, so the
/// cutoff cannot key off word content).
pub fn earned_reveal(ev: &ServerEvent, seat: usize) -> bool {
    match ev {
        ServerEvent::TurnEnded { .. } => true,
        ServerEvent::GuessResult {
            seat: s,
            correct: true,
            ..
        } => *s == seat,
        _ => false,
    }
}
