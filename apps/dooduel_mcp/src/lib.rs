//! `dooduel_mcp` — the headless MCP agent client (spec §7).
//!
//! An LLM agent plays Dooduel as a **headless protocol client**, never by driving a
//! GUI: this crate wraps the shared [`dooduel_core::transport::ClientTransport`] around a
//! [`RoomReplica`] and exposes the game as game-semantic tools over a hand-rolled stdio
//! JSON-RPC MCP surface ([`mcp`]). At the authoritative `Session` an agent is
//! indistinguishable from a GUI client — one process per seat, exactly as the acceptance
//! playtest ran (spec §1.4).
//!
//! Two pieces:
//! - [`ReplicaFold`] — the **pure, transport-free** client-side fold: the mirror of the
//!   GUI's `apply_event` (`apps/dooduel/src/lib.rs`). It folds one [`ServerEvent`] into a
//!   [`RoomReplica`] plus the small client-only state the GUI keeps beside its replica
//!   (the canvas reseed counter, the reconnect token, the transient stroke-progress
//!   overlay, the last error). Kept separate from the transport so it is unit- **and**
//!   replay-testable (spec §3.4/§9.6) with no socket.
//! - [`HeadlessClient`] — [`ReplicaFold`] + a transport: `pump()` drains events into the
//!   fold, the intent passthroughs send, and [`HeadlessClient::state_report`] /
//!   [`HeadlessClient::canvas_png`] render the honest per-seat view the MCP `get_state` /
//!   `get_canvas` tools return.

use dooduel_core::canvas::{CANVAS_H, CANVAS_W, PAPER, flood_fill, stamp_circle, stroke_segment};
use dooduel_core::game::Phase;
use dooduel_core::protocol::{
    CanvasOp, ClientIntent, ErrorCode, PROTOCOL_VERSION, RoomReplica, ServerEvent, WireAvatar,
};
use dooduel_core::transport::{ClientTransport, ConnStatus};

pub mod mcp;

// ---------------------------------------------------------------------------
// ReplicaFold — the pure client-side fold (mirror of the GUI's apply_event).
// ---------------------------------------------------------------------------

/// A live in-progress stroke batch buffered off the op log (spec §3.5): the guesser
/// paints it immediately, and it is wiped on any authoritative canvas event or a `Roster`
/// showing the drawer disconnected. It carries no id and never enters the op log.
///
/// **Coalesced per `stroke_id` (W5-review minor 4):** batches of one stroke arrive one per
/// frame carrying only *their* new points; the fold **extends** the current progress with
/// each same-id batch (so `stamp_points` interpolates across batch boundaries — no gap at
/// the seam) and **replaces** it when a new `stroke_id` arrives (dropping any stale,
/// abandoned batch). There is only ever one in-progress stroke (the sole drawer), so this
/// is an `Option`, not a list — mirroring the GUI's live-relay paint.
#[derive(Clone, Debug, Default, PartialEq)]
struct ProgressStroke {
    stroke_id: u64,
    points: Vec<(i32, i32)>,
    color: [u8; 4],
    radius: i32,
}

/// The drawer's own in-progress stroke, accumulated on the OUTBOUND side (spec §3.5): the
/// drawer is never echoed its own ops, so its optimistic overlay is built from the batches
/// it sends. Finalized into an [`CanvasOp::Stroke`] on `done` / a `stroke_id` change / a
/// fill — mirroring the server's own open-stroke accumulation, so the drawer's overlay
/// matches the log the server builds (sole-producer order is provably identical).
#[derive(Clone, Debug)]
struct OwnOpenStroke {
    stroke_id: u64,
    points: Vec<(i32, i32)>,
    color: [u8; 4],
    radius: i32,
}

/// The pure client-side fold — the transport-free mirror of the GUI's `apply_event`
/// (`apps/dooduel/src/lib.rs`).
///
/// It folds one [`ServerEvent`] into a [`RoomReplica`] plus the small client-only state
/// the GUI keeps *beside* its replica: the canvas reseed counter (bumped on a wholesale
/// log replace — `RoomState`/`CanvasLog` — so a cross-turn coincidence still re-renders,
/// GUI parity), the rotating reconnect token, the transient stroke-progress overlay, and
/// the last error. The word secret, another drawer's pre-pick choices, the RNG seed, and
/// other seats' private chat have **no field to land in** (the negative invariant, spec
/// §4.1) — the fold cannot manufacture what the server redacted.
///
/// **Deliberate deviation from the GUI fold (documented, W5):** the GUI re-anchors the
/// phase clock to its *monotonic per-frame* `Msg::Tick` and interpolates a live countdown
/// (`Countdown`, spec §4.3). A headless client is pumped on demand, not per frame, so it
/// has no continuous clock to interpolate against; it simply carries the server's
/// last-reported `remaining` in [`RoomReplica::remaining`] (honest — the last value the
/// authority sent). The reset-vs-clamp anchor *rules* only matter for the interpolated
/// display the GUI owns; the raw value the fold keeps is deterministic, so the §9.6 replay
/// property holds regardless.
#[derive(Clone, Debug, Default)]
pub struct ReplicaFold {
    /// The client-side mirror of the room (the redaction-safe subset, spec §4.1).
    pub replica: RoomReplica,
    /// Monotonic count of wholesale canvas-log replacements (`RoomState`/`CanvasLog`) —
    /// GUI parity (`Dooduel::canvas_reseeds`): a reseed always re-renders the raster even
    /// when `(len, last_op_id)` coincidentally matches across turns (ids reset per turn).
    pub canvas_reseeds: u64,
    /// The current reconnect token (from the latest `Welcome`; rotated every
    /// (re)connection, spec §6.3). The reconnect path re-attaches with it.
    pub reconnect_token: String,
    /// The transient in-progress stroke overlay (spec §3.5) — painted by `canvas_png`,
    /// coalesced per `stroke_id`, wiped on any authoritative canvas event or a
    /// drawer-disconnected `Roster`.
    live_progress: Option<ProgressStroke>,
    /// The drawer's OWN optimistic op overlay (W5-review Important 1): the drawer is not
    /// echoed its own ops (spec §3.5 no-echo), so `replica.canvas_ops` stays empty during
    /// its turn — this holds the strokes/fills it drew so `canvas_png` shows the drawer its
    /// own ink. Finalized ops only; the open batch lives in [`Self::own_open`]. Reconciled
    /// with the echoed `CanvasUndo`/`CanvasCleared`, replaced wholesale by a `CanvasLog`/
    /// `RoomState` reseed, and cleared at each turn start (`PhaseChanged`→Picking).
    own_ops: Vec<CanvasOp>,
    /// The drawer's accumulating own stroke (across `done: false` batches).
    own_open: Option<OwnOpenStroke>,
    /// The dense per-turn id the drawer assigns its own finalized ops (spec §3.5 — the
    /// drawer derives its ids by counting its own finalizations), so an echoed
    /// `CanvasUndo { removed_id }` resolves against [`Self::own_ops`]. Resets each turn.
    next_own_op_id: u64,
    /// The last rejected-intent / protocol error (the GUI surfaces this as a toast;
    /// off-replica by the negative invariant). Rendered in the seat view.
    last_error: Option<(ErrorCode, String)>,
}

impl ReplicaFold {
    /// Fold one authoritative [`ServerEvent`] — the sole state mutator, mirroring the
    /// GUI's `apply_event` event→replica mapping (spec §3.3). Each arm sets exactly the
    /// fields the event carries.
    pub fn apply(&mut self, ev: ServerEvent) {
        match ev {
            ServerEvent::Welcome {
                seat,
                room_code,
                reconnect_token,
                ..
            } => {
                self.replica.my_seat = seat;
                self.replica.room_code = room_code;
                self.reconnect_token = reconnect_token;
                self.last_error = None;
            }
            ServerEvent::RoomState(replica) => {
                self.replica = replica;
                // A full seed is a wholesale canvas replace (a reseed) — the raster must
                // re-render even if the log coincides, any transient progress is stale, and
                // the drawer's optimistic overlay is superseded by the authoritative log.
                self.canvas_reseeds = self.canvas_reseeds.wrapping_add(1);
                self.live_progress = None;
                self.reseed_own_overlay();
            }
            ServerEvent::Roster { players, host } => {
                self.replica.players = players;
                self.replica.host = host;
                // R1: the drawer-drop/vacate discard paths emit no canvas event, so a
                // `Roster` showing the drawer disconnected is the progress-wipe trigger.
                if let Some(d) = self.replica.drawer
                    && self.replica.players.get(d).is_some_and(|p| !p.connected)
                {
                    self.live_progress = None;
                }
            }
            ServerEvent::PhaseChanged {
                phase,
                drawer,
                round,
                total_rounds,
                remaining,
            } => {
                self.replica.phase = phase;
                self.replica.drawer = drawer;
                self.replica.round = round;
                self.replica.total_rounds = total_rounds;
                self.replica.remaining = remaining;
                if phase == Phase::Picking {
                    // A fresh turn: clear last turn's word row / choices / reveal rows
                    // (nothing else re-blanks a guesser's row — GUI parity).
                    self.replica.word_display.clear();
                    self.replica.word_len = 0;
                    self.replica.hints_revealed = 0;
                    self.replica.word_choices.clear();
                    self.replica.turn_results.clear();
                    // A fresh canvas: drop the drawer's optimistic overlay + its per-turn
                    // op-id counter (the log clears server-side via CanvasCleared too).
                    self.own_ops.clear();
                    self.own_open = None;
                    self.next_own_op_id = 0;
                    self.live_progress = None;
                }
            }
            ServerEvent::CountdownSync { remaining } => self.replica.remaining = remaining,
            ServerEvent::WordUpdate {
                display,
                len,
                hints_revealed,
            } => {
                self.replica.word_display = display;
                self.replica.word_len = len;
                self.replica.hints_revealed = hints_revealed;
            }
            ServerEvent::WordChoices { words } => self.replica.word_choices = words,
            ServerEvent::CanvasOpApplied { op } => {
                self.replica.canvas_ops.push(op);
                self.live_progress = None;
            }
            ServerEvent::CanvasStrokeProgress {
                stroke_id,
                points,
                color,
                radius,
            } => match &mut self.live_progress {
                // Same stroke: EXTEND (so stamp_points interpolates across the seam).
                Some(p) if p.stroke_id == stroke_id => p.points.extend_from_slice(&points),
                // A new (or first) stroke: REPLACE — drop any stale abandoned batch.
                _ => {
                    self.live_progress = Some(ProgressStroke {
                        stroke_id,
                        points,
                        color,
                        radius,
                    })
                }
            },
            ServerEvent::CanvasUndo { removed_id } => {
                // The undo confirmation reaches every seat (spec §3.5): remove it from the
                // guesser's log AND the drawer's own overlay (whichever holds it).
                self.replica.canvas_ops.retain(|op| op_id(op) != removed_id);
                self.own_ops.retain(|op| op_id(op) != removed_id);
                self.live_progress = None;
            }
            ServerEvent::CanvasCleared => {
                self.replica.canvas_ops.clear();
                self.own_ops.clear();
                self.own_open = None;
                self.live_progress = None;
            }
            ServerEvent::CanvasLog { ops } => {
                self.replica.canvas_ops = ops;
                self.canvas_reseeds = self.canvas_reseeds.wrapping_add(1);
                self.live_progress = None;
                self.reseed_own_overlay();
            }
            ServerEvent::ChatLine { line } => self.replica.chat.push(line),
            ServerEvent::GuessResult { seat, correct, .. } => {
                // The score + a full Roster follow; reflect the guessed flag at once.
                if correct && let Some(p) = self.replica.players.get_mut(seat) {
                    p.guessed = true;
                }
            }
            ServerEvent::TurnEnded { results, word } => {
                self.replica.turn_results = results;
                // The reveal legitimately broadcasts the word (spec §3.3): show it as the
                // full, space-joined row (GUI parity).
                self.replica.word_len = word.chars().count();
                self.replica.word_display = word
                    .chars()
                    .map(|c| c.to_ascii_uppercase().to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            ServerEvent::MatchEnded { podium } => {
                self.replica.podium = Some(podium);
                self.replica.phase = Phase::Final;
            }
            ServerEvent::Error { code, message } => self.last_error = Some((code, message)),
        }
    }

    // --- The drawer's own optimistic overlay (spec §3.5, W5-review Important 1) ---
    //
    // The drawer is never echoed its own ops, so these are built from the OUTBOUND intents
    // the `HeadlessClient` passthroughs relay here after sending. Sole-producer order is
    // provably identical to the server's, so the dense ids match and an echoed `CanvasUndo`
    // resolves against `own_ops`.

    /// A reseed (`CanvasLog`/`RoomState`) supersedes the overlay: the drawer's surviving
    /// ops are now in the authoritative `replica.canvas_ops`, so drop the overlay and
    /// realign the per-turn id counter to the reseeded log length.
    fn reseed_own_overlay(&mut self) {
        self.own_ops.clear();
        self.own_open = None;
        self.next_own_op_id = self.replica.canvas_ops.len() as u64;
    }

    /// Accumulate one outbound stroke batch (finalizing on `done` / a `stroke_id` change).
    fn own_stroke_batch(
        &mut self,
        stroke_id: u64,
        points: &[(i32, i32)],
        color: [u8; 4],
        radius: i32,
        done: bool,
    ) {
        if self
            .own_open
            .as_ref()
            .is_some_and(|o| o.stroke_id != stroke_id)
        {
            self.finalize_own_open();
        }
        match &mut self.own_open {
            Some(o) => o.points.extend_from_slice(points),
            None => {
                self.own_open = Some(OwnOpenStroke {
                    stroke_id,
                    points: points.to_vec(),
                    color,
                    radius,
                })
            }
        }
        if done {
            self.finalize_own_open();
        }
    }

    /// Finalize an outbound fill into the overlay (closing any open stroke first).
    fn own_fill(&mut self, seed: (i32, i32), color: [u8; 4]) {
        self.finalize_own_open();
        let id = self.next_own_op_id;
        self.next_own_op_id += 1;
        self.own_ops.push(CanvasOp::Fill { id, seed, color });
    }

    /// An outbound `Undo`: the server cancels an un-finalized open stroke (mirror it); a
    /// finalized op is removed when its `CanvasUndo` echo folds (spec §3.5).
    fn own_undo(&mut self) {
        self.own_open = None;
    }

    /// Close the accumulating own stroke into a finalized [`CanvasOp::Stroke`] (an empty
    /// stroke mints no op / no id — matching the server).
    fn finalize_own_open(&mut self) {
        let Some(o) = self.own_open.take() else {
            return;
        };
        if o.points.is_empty() {
            return;
        }
        let id = self.next_own_op_id;
        self.next_own_op_id += 1;
        self.own_ops.push(CanvasOp::Stroke {
            id,
            points: o.points,
            color: o.color,
            radius: o.radius,
        });
    }
}

/// The stable id of a canvas op (spec §3.5) — a `CanvasUndo { removed_id }` resolves
/// against the log by this. (Same shape as the GUI's `op_id`.)
fn op_id(op: &CanvasOp) -> u64 {
    match op {
        CanvasOp::Stroke { id, .. } | CanvasOp::Fill { id, .. } => *id,
    }
}

// ---------------------------------------------------------------------------
// HeadlessClient — the fold + a transport (send intents / pump events / render).
// ---------------------------------------------------------------------------

/// A headless protocol client: a [`ReplicaFold`] fed by a [`ClientTransport`] (spec §7).
/// Generic over the transport so the production path uses
/// [`dooduel_core::transport::WsClientTransport`] while unit tests drive it through an
/// in-process pair (the real `pump` path, no socket).
pub struct HeadlessClient<T: ClientTransport> {
    transport: T,
    fold: ReplicaFold,
    /// This seat's name (for the outbound `Create`/`Join` frames + the report fallback
    /// before the roster seats it).
    name: String,
    avatar: WireAvatar,
    /// A monotonic client batching handle for `draw_stroke` (spec §3.5 — groups a
    /// drawer's batches; never travels in a logged op).
    next_stroke_id: u64,
}

impl<T: ClientTransport> HeadlessClient<T> {
    /// Wrap a transport in a fresh, unseated client.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            fold: ReplicaFold::default(),
            name: String::new(),
            avatar: WireAvatar::Default,
            next_stroke_id: 1,
        }
    }

    /// Drain every queued server event into the fold (spec §4.2 — the pump), returning
    /// them in order so a caller can search for a condition or record the stream (the
    /// §9.6 replay recording). Non-blocking: it yields whatever has arrived.
    pub fn pump(&mut self) -> Vec<ServerEvent> {
        let mut drained = Vec::new();
        while let Some(ev) = self.transport.try_recv() {
            self.fold.apply(ev.clone());
            drained.push(ev);
        }
        drained
    }

    /// Fold one event directly (bypassing the transport) — for scripted tests + replay.
    pub fn apply(&mut self, ev: ServerEvent) {
        self.fold.apply(ev);
    }

    /// The client-side room mirror (redaction-safe, spec §4.1).
    pub fn replica(&self) -> &RoomReplica {
        &self.fold.replica
    }

    /// The full fold state (replica + reseed counter + token) — for tests/replay.
    pub fn fold(&self) -> &ReplicaFold {
        &self.fold
    }

    /// The transport's connection status (for the reconnect / displaced-socket UX).
    pub fn status(&self) -> ConnStatus {
        self.transport.status()
    }

    /// The current (rotating, single-use) reconnect token (spec §6.3).
    pub fn reconnect_token(&self) -> &str {
        &self.fold.reconnect_token
    }

    // --- Intent passthroughs (spec §3.2) ------------------------------------

    /// Open a fresh room (creator = host); the server issues the code in `Welcome`.
    pub fn create(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.send(ClientIntent::Create {
            name: self.name.clone(),
            avatar: self.avatar.clone(),
            protocol_version: PROTOCOL_VERSION,
        });
    }

    /// Join a room by code, optionally re-attaching a held seat with a token (spec §6.3).
    pub fn join(
        &mut self,
        room: impl Into<String>,
        name: impl Into<String>,
        reconnect: Option<String>,
    ) {
        self.name = name.into();
        self.send(ClientIntent::Join {
            room: room.into(),
            name: self.name.clone(),
            avatar: self.avatar.clone(),
            protocol_version: PROTOCOL_VERSION,
            reconnect,
        });
    }

    /// Host-only: start the match (spec §3.2).
    pub fn start_match(&mut self) {
        self.send(ClientIntent::StartMatch);
    }

    /// Drawer-only, in Picking: commit word choice `index`.
    pub fn pick(&mut self, index: usize) {
        self.send(ClientIntent::Pick { index });
    }

    /// Guess `text` (non-drawer, in Drawing).
    pub fn guess(&mut self, text: impl Into<String>) {
        self.send(ClientIntent::Guess { text: text.into() });
    }

    /// Send one stroke batch under an explicit `stroke_id` (spec §3.5) — the multi-batch
    /// path the e2e drives (`done: false` grows the stroke; `done: true` finalizes it).
    pub fn stroke(
        &mut self,
        stroke_id: u64,
        points: Vec<(i32, i32)>,
        color: [u8; 4],
        radius: i32,
        done: bool,
    ) {
        // Feed the drawer's own optimistic overlay before the points move onto the wire
        // (the drawer is never echoed its own ops, so this is its only local record).
        self.fold
            .own_stroke_batch(stroke_id, &points, color, radius, done);
        self.send(ClientIntent::Stroke {
            stroke_id,
            points,
            color,
            radius,
            done,
        });
    }

    /// Draw one complete stroke (a single finalized batch) — the MCP `draw_stroke` tool's
    /// shape (an agent draws crude shapes, one call per stroke). Auto-mints a `stroke_id`.
    pub fn draw_stroke(&mut self, points: Vec<(i32, i32)>, color: [u8; 4], radius: i32) {
        let id = self.next_stroke_id;
        self.next_stroke_id += 1;
        self.stroke(id, points, color, radius, true);
    }

    /// Flood-fill from `seed` with `color` (drawer-only, in Drawing).
    pub fn fill(&mut self, seed: (i32, i32), color: [u8; 4]) {
        self.fold.own_fill(seed, color);
        self.send(ClientIntent::Fill { seed, color });
    }

    /// Undo the last op (drawer-only, in Drawing). A finalized op is removed from the
    /// overlay when its `CanvasUndo` echo folds; an un-finalized open stroke is cancelled
    /// now (the server does the same, spec §3.5).
    pub fn undo(&mut self) {
        self.fold.own_undo();
        self.send(ClientIntent::Undo);
    }

    /// Clear the canvas (drawer-only, in Drawing).
    pub fn clear(&mut self) {
        self.send(ClientIntent::Clear);
    }

    /// Advance out of the reveal (any seat, in Reveal).
    pub fn continue_turn(&mut self) {
        self.send(ClientIntent::Continue);
    }

    /// Graceful seat release (skips grace, spec §3.2).
    pub fn leave(&mut self) {
        self.send(ClientIntent::Leave);
    }

    /// Send an arbitrary intent verbatim — the escape hatch the e2e uses to drive
    /// off-nominal frames (e.g. a `Create` carrying a deliberately-wrong
    /// `protocol_version` for the version-gate test). The tool-facing passthroughs above
    /// always send the current [`PROTOCOL_VERSION`].
    pub fn send_raw(&mut self, intent: ClientIntent) {
        self.send(intent);
    }

    fn send(&mut self, intent: ClientIntent) {
        self.transport.send(&intent);
    }

    // --- The honest per-seat view (spec §7 `get_state` / `get_canvas`) -------

    /// The drawer's offered word choices (empty unless this seat is the picking drawer).
    /// The MCP `list_choices` tool's payload.
    pub fn word_choices(&self) -> &[String] {
        &self.fold.replica.word_choices
    }

    /// The honest per-seat view as markdown (spec §7 `get_state`) — phase, the word row
    /// (redacted exactly as the replica is), the roster + scores, the chat tail, and the
    /// actions available to this seat *right now*. Fed entirely from the replica, so a
    /// guesser's report cannot contain the secret before it is revealed (the negative
    /// invariant, spec §4.1/§5).
    pub fn state_report(&self) -> String {
        let r = &self.fold.replica;
        let me = r.my_seat;
        let my_name = r
            .players
            .get(me)
            .map(|p| p.name.as_str())
            .filter(|n| !n.is_empty())
            .unwrap_or(if self.name.is_empty() {
                "you"
            } else {
                &self.name
            });

        let mut out = String::new();
        // Lead with a connection banner whenever the socket is not live (W5-review
        // Important 2): an unattended agent must know its replica is frozen and act (rejoin)
        // instead of grinding a dead state that will never update.
        match self.transport.status() {
            ConnStatus::Open => {}
            ConnStatus::Closed => {
                out.push_str("> ⚠ CONNECTION LOST — reports are frozen; rejoin required.\n\n")
            }
            ConnStatus::Connecting => {
                out.push_str("> ⏳ CONNECTING — not yet live; this view may be empty or stale.\n\n")
            }
        }
        out.push_str(&format!("# Dooduel — you are seat {me} ({my_name})\n"));
        let room = if r.room_code.is_empty() {
            "(not in a room)".to_string()
        } else {
            r.room_code.clone()
        };
        let mut line = format!("Room {room} · {}", phase_label(r.phase));
        if r.round > 0
            && r.total_rounds > 0
            && matches!(r.phase, Phase::Picking | Phase::Drawing | Phase::Reveal)
        {
            line.push_str(&format!(" · round {}/{}", r.round, r.total_rounds));
        }
        let secs = r.remaining.as_secs();
        if secs > 0 && matches!(r.phase, Phase::Picking | Phase::Drawing | Phase::Reveal) {
            line.push_str(&format!(" · ~{secs}s left"));
        }
        out.push_str(&line);
        out.push_str("\n\n");

        // The word row — redacted exactly as the replica is (the load-bearing property).
        out.push_str(&self.word_section());

        // The roster + scores.
        out.push_str("\n## Players\n");
        if r.players.is_empty() {
            out.push_str("- (waiting for players)\n");
        }
        for (i, p) in r.players.iter().enumerate() {
            let mut tags = Vec::new();
            if i == me {
                tags.push("you".to_string());
            }
            if i == r.host {
                tags.push("host".to_string());
            }
            if r.drawer == Some(i) {
                tags.push("drawing".to_string());
            }
            if p.is_bot {
                tags.push("bot".to_string());
            }
            if p.guessed {
                tags.push("guessed ✓".to_string());
            }
            if !p.connected {
                tags.push("disconnected".to_string());
            }
            let tag = if tags.is_empty() {
                String::new()
            } else {
                format!("  [{}]", tags.join("] ["))
            };
            out.push_str(&format!("- #{i} {} — {} pts{tag}\n", p.name, p.score));
        }

        // The chat tail (the last handful of lines).
        out.push_str("\n## Chat\n");
        let tail = r.chat.iter().rev().take(8).collect::<Vec<_>>();
        if tail.is_empty() {
            out.push_str("- (no messages yet)\n");
        }
        for m in tail.into_iter().rev() {
            out.push_str(&format!("- {}\n", m.text));
        }

        // The podium, if the match is over.
        if let Some(podium) = &r.podium {
            out.push_str("\n## Podium\n");
            for (rank, (_seat, name, score)) in podium.iter().enumerate() {
                out.push_str(&format!("{}. {name} — {score} pts\n", rank + 1));
            }
        }

        // The actions available to this seat right now.
        out.push_str("\n## You can now\n");
        for a in self.actions_now() {
            out.push_str(&format!("- {a}\n"));
        }

        if let Some((code, message)) = &self.fold.last_error {
            out.push_str(&format!("\n> ⚠ last error: {code:?} — {message}\n"));
        }
        out
    }

    /// The word row, rendered as this seat is allowed to see it. A seat that knows the
    /// word (the drawer, a correct guesser, or the reveal) gets the contiguous word; a
    /// guesser gets the blanked row (`_ _ B _ _`) — never the secret.
    fn word_section(&self) -> String {
        let r = &self.fold.replica;
        if !matches!(r.phase, Phase::Drawing | Phase::Reveal) {
            return String::new();
        }
        let slots = r.word_slots();
        if slots.is_empty() {
            return String::new();
        }
        let all_revealed = slots.iter().all(|(_, revealed)| *revealed);
        if all_revealed {
            let word: String = slots.iter().map(|(c, _)| *c).collect();
            format!("## Word\nYou know the word: **{word}**\n")
        } else {
            let row = slots
                .iter()
                .map(|(c, _)| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let revealed = slots.iter().filter(|(_, r)| *r).count();
            format!(
                "## Word\n`{row}`  ({revealed}/{} letters revealed)\n",
                slots.len()
            )
        }
    }

    /// What this seat can do at this moment (spec §7 — the actionable-now flags).
    fn actions_now(&self) -> Vec<String> {
        let r = &self.fold.replica;
        let me = r.my_seat;
        let is_drawer = r.drawer == Some(me);
        let is_host = !r.players.is_empty() && me == r.host;
        let guessed = r.players.get(me).is_some_and(|p| p.guessed);
        let mut a = Vec::new();
        match r.phase {
            Phase::Idle => {
                if is_host {
                    a.push("start the match (you are the host)".to_string());
                } else {
                    a.push("wait for the host to start the match".to_string());
                }
            }
            Phase::Picking => {
                if is_drawer {
                    a.push("list_choices, then pick_word(index) — you draw this turn".to_string());
                } else {
                    a.push("wait for the drawer to pick a word".to_string());
                }
            }
            Phase::Drawing => {
                if is_drawer {
                    a.push("draw_stroke(points,color,size) / fill / undo / clear".to_string());
                } else if guessed {
                    a.push("you guessed it — wait for the turn to end".to_string());
                } else {
                    a.push("guess(text) — read the drawing with get_canvas".to_string());
                }
            }
            Phase::Reveal => a.push("continue_turn — advance to the next turn".to_string()),
            Phase::Final => a.push("the match is over — see the podium above".to_string()),
        }
        a
    }

    /// The current canvas as a PNG (spec §7 `get_canvas`): the op log rasterized locally
    /// through the same integer paint math every replica uses (`dooduel_core::canvas`),
    /// plus any buffered live stroke-progress stamped on top (agents guess from
    /// in-progress drawings). No PNG ever travels on the wire (spec §2.2).
    ///
    /// A GUESSER sees the **authoritative** op log plus any in-progress stroke (agents
    /// guess from live ink). A DRAWER is not echoed its own ops (spec §3.5 no-echo), so its
    /// authoritative log is empty during its turn — but its own optimistic overlay
    /// (built from the strokes/fills it sent, W5-review Important 1) is rasterized here, so
    /// a drawer agent sees its own ink. A reconnect `CanvasLog` reseed folds the surviving
    /// ops into the authoritative log and clears the overlay, so no op is drawn twice.
    pub fn canvas_png(&self) -> Vec<u8> {
        let pixels = self.rasterize();
        encode_png(CANVAS_W as u32, CANVAS_H as u32, &pixels)
    }

    /// Rasterize the authoritative log + the drawer's own overlay + the buffered progress
    /// onto a fresh `PAPER` RGBA8 buffer, mirroring the GUI's raster
    /// (`apps/dooduel/src/paint.rs` `apply_op` / `stamp_points`) so identical ops produce
    /// identical pixels. For any given seat one of the two op sources is empty (a guesser
    /// has only the authoritative log; a drawer has only its overlay until a reseed), so
    /// nothing is stamped twice.
    fn rasterize(&self) -> Vec<u8> {
        let (w, h) = (CANVAS_W, CANVAS_H);
        let mut px: Vec<u8> = PAPER.iter().copied().cycle().take(w * h * 4).collect();
        for op in &self.fold.replica.canvas_ops {
            apply_op(&mut px, w, h, op);
        }
        // The drawer's own optimistic overlay (empty for a guesser).
        for op in &self.fold.own_ops {
            apply_op(&mut px, w, h, op);
        }
        // The transient in-progress overlay grows the guesser's view before the finalize.
        if let Some(p) = &self.fold.live_progress {
            stamp_points(&mut px, w, h, &p.points, p.color, p.radius);
        }
        px
    }
}

impl HeadlessClient<dooduel_core::transport::WsClientTransport> {
    /// Open a `WsClientTransport` to `url` (e.g. `ws://127.0.0.1:7878`) and wrap it — the
    /// production MCP-agent constructor (spec §7). `WsClientTransport` is always available
    /// here (this crate pins `dooduel_core` with the `ws-client` feature).
    pub fn connect(url: impl Into<String>) -> Result<Self, String> {
        let t = dooduel_core::transport::WsClientTransport::connect(url)?;
        Ok(Self::new(t))
    }
}

/// A human-readable phase label for the seat view.
fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Idle => "lobby",
        Phase::Picking => "picking a word",
        Phase::Drawing => "drawing",
        Phase::Reveal => "turn over",
        Phase::Final => "match over",
    }
}

// --- Rasterization (mirrors apps/dooduel/src/paint.rs, spec §2.2) -----------

/// Stamp a stroke's exact sample sequence (interpolating between samples) — the pure
/// integer op the op-log sync stands on. Mirrors `paint.rs::stamp_points`.
fn stamp_points(
    px: &mut [u8],
    w: usize,
    h: usize,
    points: &[(i32, i32)],
    color: [u8; 4],
    radius: i32,
) {
    let mut last: Option<(i32, i32)> = None;
    for &(x, y) in points {
        match last {
            Some(l) => stroke_segment(px, w, h, l, (x, y), radius, color),
            None => stamp_circle(px, w, h, x, y, radius, color),
        }
        last = Some((x, y));
    }
}

/// Replay one [`CanvasOp`] onto a buffer — the same shape as the GUI's guesser rasterizer
/// (`paint.rs::apply_op`), so identical ops ⇒ identical pixels.
fn apply_op(px: &mut [u8], w: usize, h: usize, op: &CanvasOp) {
    match op {
        CanvasOp::Stroke {
            points,
            color,
            radius,
            ..
        } => stamp_points(px, w, h, points, *color, *radius),
        CanvasOp::Fill { seed, color, .. } => flood_fill(px, w, h, seed.0, seed.1, *color),
    }
}

/// Encode an RGBA8 buffer to a PNG. The buffer is our own (never wire input), and a
/// `Vec` sink never fails I/O — a dimension mismatch degrades to an empty `Vec` rather
/// than a panic (no `unwrap` on any path a caller reaches).
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use image::ImageEncoder as _;
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    if encoder
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .is_err()
    {
        out.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dooduel_core::game::{ChatKind, ChatMsg};
    use dooduel_core::protocol::{ReplicaPlayer, RoomReplica};
    use dooduel_core::transport::{
        InProcClient, InProcServer, InProcessTransport, ServerTransport,
    };
    use std::time::Duration;

    /// A HeadlessClient over an in-process transport pair — the real `pump` path, no
    /// socket. The returned server lets a test feed scripted events to seat 0's client.
    fn client_pair() -> (InProcServer, HeadlessClient<InProcClient>) {
        let (server, mut clients) = InProcessTransport::new_pair(1);
        (server, HeadlessClient::new(clients.remove(0)))
    }

    fn player(name: &str, is_bot: bool) -> ReplicaPlayer {
        ReplicaPlayer {
            name: name.to_string(),
            avatar: WireAvatar::Default,
            connected: true,
            is_bot,
            score: 0,
            guessed: false,
        }
    }

    fn four_players() -> Vec<ReplicaPlayer> {
        vec![
            player("Ada", false),
            player("Bo", false),
            player("Cy", false),
            player("Di", false),
        ]
    }

    /// Seed a client to Drawing with `my_seat` a guesser (drawer = 0) and a blanked word.
    fn seed_guesser(hc: &mut HeadlessClient<InProcClient>, my_seat: usize) {
        hc.apply(ServerEvent::Welcome {
            seat: my_seat,
            room_code: "ROOM01".to_string(),
            reconnect_token: "tok".to_string(),
            protocol_version: PROTOCOL_VERSION,
        });
        hc.apply(ServerEvent::Roster {
            players: four_players(),
            host: 0,
        });
        hc.apply(ServerEvent::PhaseChanged {
            phase: Phase::Drawing,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(80),
        });
    }

    // --- The load-bearing redaction property (spec §5) ----------------------

    #[test]
    fn a_guesser_report_never_contains_the_secret_pre_reveal() {
        // A drawer (knows-path) and a guesser (redacted) are fed the SAME turn: the drawer
        // gets the full word, the guesser gets blanks. The reports must reflect exactly
        // that — the guesser's view cannot contain the secret before the reveal.
        let secret = "robot";

        let mut drawer = HeadlessClient::new(InProcessTransport::new_pair(1).1.remove(0));
        seed_guesser(&mut drawer, 0); // seat 0 IS the drawer here
        drawer.apply(ServerEvent::WordUpdate {
            display: "R O B O T".to_string(),
            len: 5,
            hints_revealed: 5,
        });
        let drawer_report = drawer.state_report().to_lowercase();

        let mut guesser = HeadlessClient::new(InProcessTransport::new_pair(1).1.remove(0));
        seed_guesser(&mut guesser, 1); // seat 1 is a guesser (drawer is 0)
        guesser.apply(ServerEvent::WordUpdate {
            display: "_ _ _ _ _".to_string(),
            len: 5,
            hints_revealed: 0,
        });
        let guesser_report = guesser.state_report().to_lowercase();

        assert!(
            drawer_report.contains(secret),
            "the knows-path drawer report DOES carry the secret (proving the scan can see it):\n{drawer_report}"
        );
        assert!(
            !guesser_report.contains(secret),
            "the guesser report must NOT contain the secret pre-reveal:\n{guesser_report}"
        );
    }

    #[test]
    fn a_hint_reveal_does_not_leak_the_whole_word() {
        // A single revealed hint letter appears; the rest stay blank; the secret is not
        // reconstructable from the guesser's row.
        let mut guesser = HeadlessClient::new(InProcessTransport::new_pair(1).1.remove(0));
        seed_guesser(&mut guesser, 1);
        guesser.apply(ServerEvent::WordUpdate {
            display: "_ _ B _ _".to_string(),
            len: 5,
            hints_revealed: 1,
        });
        let report = guesser.state_report();
        assert!(
            report.contains("_ _ B _ _"),
            "the hint row is shown: {report}"
        );
        assert!(
            !report.to_lowercase().contains("robot"),
            "one hint does not leak the word"
        );
    }

    #[test]
    fn the_pump_folds_events_through_the_transport() {
        // The real pump path: an event queued on the server end reaches the fold.
        let (mut server, mut hc) = client_pair();
        server.send(
            0,
            &ServerEvent::Welcome {
                seat: 2,
                room_code: "ABC123".to_string(),
                reconnect_token: "rotated".to_string(),
                protocol_version: PROTOCOL_VERSION,
            },
        );
        let drained = hc.pump();
        assert_eq!(drained.len(), 1, "one event drained");
        assert_eq!(hc.replica().my_seat, 2);
        assert_eq!(hc.replica().room_code, "ABC123");
        assert_eq!(hc.reconnect_token(), "rotated");
    }

    // --- The canvas raster (spec §2.2/§7) -----------------------------------

    #[test]
    fn canvas_png_round_trips_through_image_load() {
        // A populated op log rasterizes to a decodable PNG of the canvas size.
        let mut hc = HeadlessClient::new(InProcessTransport::new_pair(1).1.remove(0));
        hc.apply(ServerEvent::CanvasOpApplied {
            op: CanvasOp::Stroke {
                id: 0,
                points: vec![(10, 10), (200, 120), (400, 300)],
                color: [20, 20, 24, 255],
                radius: 5,
            },
        });
        hc.apply(ServerEvent::CanvasOpApplied {
            op: CanvasOp::Fill {
                id: 1,
                seed: (600, 400),
                color: [0xf4, 0xc2, 0x0d, 255],
            },
        });
        let png = hc.canvas_png();
        assert!(!png.is_empty(), "a PNG was encoded");
        let img = image::load_from_memory(&png).expect("the PNG decodes");
        assert_eq!(img.width(), CANVAS_W as u32);
        assert_eq!(img.height(), CANVAS_H as u32);
    }

    #[test]
    fn buffered_stroke_progress_appears_in_the_png() {
        // A guesser watching an in-progress stroke: the transient progress is stamped into
        // the PNG even though it is not yet in the op log (agents guess from live ink).
        let mut hc = HeadlessClient::new(InProcessTransport::new_pair(1).1.remove(0));
        // A dark stroke across a blank (PAPER-white) canvas.
        hc.apply(ServerEvent::CanvasStrokeProgress {
            stroke_id: 1,
            points: vec![(50, 50), (300, 200), (500, 350)],
            color: [10, 10, 12, 255],
            radius: 6,
        });
        let png = hc.canvas_png();
        let img = image::load_from_memory(&png).expect("decodes").to_rgba8();
        let inked = img.pixels().any(|p| p.0 != PAPER);
        assert!(inked, "the buffered progress stroke left ink on the canvas");
    }

    #[test]
    fn an_authoritative_canvas_event_wipes_the_progress_overlay() {
        // R1: progress is transient — a finalize (CanvasOpApplied) wipes the overlay (the
        // finalize re-stamps the same pixels), and a clear empties everything.
        let mut hc = HeadlessClient::new(InProcessTransport::new_pair(1).1.remove(0));
        hc.apply(ServerEvent::CanvasStrokeProgress {
            stroke_id: 1,
            points: vec![(50, 50), (300, 200)],
            color: [10, 10, 12, 255],
            radius: 6,
        });
        hc.apply(ServerEvent::CanvasCleared);
        let png = hc.canvas_png();
        let img = image::load_from_memory(&png).expect("decodes").to_rgba8();
        assert!(
            img.pixels().all(|p| p.0 == PAPER),
            "a clear wipes both the op log and the transient progress"
        );
    }

    // --- The report renders the seat view (spec §7) -------------------------

    #[test]
    fn the_report_shows_roster_scores_and_actions() {
        let mut hc = client_pair().1;
        hc.apply(ServerEvent::Welcome {
            seat: 0,
            room_code: "ROOM01".to_string(),
            reconnect_token: "t".to_string(),
            protocol_version: PROTOCOL_VERSION,
        });
        let mut players = four_players();
        players[1].score = 410;
        players[1].guessed = true;
        hc.apply(ServerEvent::Roster { players, host: 0 });
        hc.apply(ServerEvent::PhaseChanged {
            phase: Phase::Drawing,
            drawer: Some(0),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(53),
        });
        let report = hc.state_report();
        assert!(report.contains("Ada"), "the roster is shown");
        assert!(report.contains("410 pts"), "scores are shown");
        assert!(report.contains("guessed ✓"), "the guessed flag is shown");
        // Seat 0 is the drawer: its actions offer drawing.
        assert!(
            report.contains("draw_stroke"),
            "the drawer's actions are shown: {report}"
        );
    }

    #[test]
    fn a_guesser_is_offered_the_guess_action() {
        let mut hc = client_pair().1;
        seed_guesser(&mut hc, 2); // seat 2 guesser, drawer 0
        hc.apply(ServerEvent::WordUpdate {
            display: "_ _ _ _ _".to_string(),
            len: 5,
            hints_revealed: 0,
        });
        let report = hc.state_report();
        assert!(
            report.contains("guess(text)"),
            "a guesser can guess: {report}"
        );
    }

    // --- The fold mirrors the GUI apply_event semantics ---------------------

    #[test]
    fn a_correct_guess_flags_the_seat_and_turn_end_reveals_the_word() {
        let mut hc = client_pair().1;
        seed_guesser(&mut hc, 1);
        hc.apply(ServerEvent::Roster {
            players: four_players(),
            host: 0,
        });
        hc.apply(ServerEvent::GuessResult {
            seat: 2,
            correct: true,
            points: 300,
        });
        assert!(
            hc.replica().players[2].guessed,
            "a correct guess flags the seat"
        );

        hc.apply(ServerEvent::TurnEnded {
            results: vec![],
            word: "robot".to_string(),
        });
        assert_eq!(
            hc.replica().word_slots(),
            vec![
                ('R', true),
                ('O', true),
                ('B', true),
                ('O', true),
                ('T', true)
            ],
            "TurnEnded reveals the full word"
        );
    }

    #[test]
    fn a_canvas_log_reseed_bumps_the_reseed_counter() {
        let mut hc = client_pair().1;
        assert_eq!(hc.fold().canvas_reseeds, 0);
        hc.apply(ServerEvent::CanvasLog {
            ops: vec![CanvasOp::Fill {
                id: 0,
                seed: (1, 1),
                color: [1, 2, 3, 255],
            }],
        });
        assert_eq!(hc.fold().canvas_reseeds, 1, "a CanvasLog is a reseed");
        hc.apply(ServerEvent::RoomState(RoomReplica::default()));
        assert_eq!(hc.fold().canvas_reseeds, 2, "a RoomState is a reseed");
    }

    #[test]
    fn picking_clears_the_previous_turns_word_row() {
        let mut hc = client_pair().1;
        seed_guesser(&mut hc, 1);
        hc.apply(ServerEvent::WordUpdate {
            display: "R O B O T".to_string(),
            len: 5,
            hints_revealed: 5,
        });
        assert_eq!(hc.replica().word_len, 5);
        hc.apply(ServerEvent::PhaseChanged {
            phase: Phase::Picking,
            drawer: Some(1),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(15),
        });
        assert_eq!(hc.replica().word_len, 0, "Picking blanks the word row");
        assert!(hc.replica().word_display.is_empty());
    }

    #[test]
    fn a_roster_with_a_disconnected_drawer_wipes_the_progress_overlay() {
        // R1: the drawer-drop discard path emits no canvas event, so a Roster showing the
        // drawer disconnected is the wipe trigger.
        let mut hc = client_pair().1;
        seed_guesser(&mut hc, 1); // drawer = 0
        hc.apply(ServerEvent::CanvasStrokeProgress {
            stroke_id: 1,
            points: vec![(50, 50), (300, 200)],
            color: [10, 10, 12, 255],
            radius: 6,
        });
        // The drawer (seat 0) drops.
        let mut players = four_players();
        players[0].connected = false;
        hc.apply(ServerEvent::Roster { players, host: 1 });
        let img = image::load_from_memory(&hc.canvas_png())
            .expect("decodes")
            .to_rgba8();
        assert!(
            img.pixels().all(|p| p.0 == PAPER),
            "a disconnected-drawer Roster wipes the transient progress"
        );
    }

    // --- The chat tail --------------------------------------------------------

    #[test]
    fn the_report_shows_the_chat_tail() {
        let mut hc = client_pair().1;
        hc.apply(ServerEvent::Welcome {
            seat: 0,
            room_code: "ROOM01".to_string(),
            reconnect_token: "t".to_string(),
            protocol_version: PROTOCOL_VERSION,
        });
        for i in 0..3 {
            hc.apply(ServerEvent::ChatLine {
                line: ChatMsg {
                    seq: i,
                    kind: ChatKind::Guess,
                    text: format!("message {i}"),
                    to: None,
                },
            });
        }
        let report = hc.state_report();
        assert!(report.contains("message 0"));
        assert!(report.contains("message 2"));
    }

    // --- The drawer's own optimistic overlay (W5-review Important 1) ---------

    /// Seat `seat` as the drawer in Drawing.
    fn seed_drawer(hc: &mut HeadlessClient<InProcClient>, seat: usize) {
        hc.apply(ServerEvent::Welcome {
            seat,
            room_code: "ROOM01".to_string(),
            reconnect_token: "t".to_string(),
            protocol_version: PROTOCOL_VERSION,
        });
        hc.apply(ServerEvent::Roster {
            players: four_players(),
            host: 0,
        });
        hc.apply(ServerEvent::PhaseChanged {
            phase: Phase::Drawing,
            drawer: Some(seat),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(80),
        });
    }

    #[test]
    fn a_drawer_sees_its_own_optimistic_ink_and_it_clears_at_turn_end() {
        let mut hc = client_pair().1;
        seed_drawer(&mut hc, 0);
        // The drawer's authoritative log stays empty (no-echo); without the overlay,
        // canvas_png would be blank. The overlay makes the drawer see its own ink.
        hc.draw_stroke(vec![(50, 50), (300, 200), (500, 350)], [10, 10, 12, 255], 6);
        hc.fill((650, 400), [244, 194, 13, 255]);
        assert!(
            hc.replica().canvas_ops.is_empty(),
            "the drawer is never echoed its own ops (authoritative log empty)"
        );
        let img = image::load_from_memory(&hc.canvas_png())
            .expect("decodes")
            .to_rgba8();
        assert!(
            img.pixels().any(|p| p.0 != PAPER),
            "the drawer sees its own optimistic ink mid-turn"
        );

        // Turn end: a fresh Picking clears the overlay — the next turn starts blank.
        hc.apply(ServerEvent::PhaseChanged {
            phase: Phase::Picking,
            drawer: Some(1),
            round: 1,
            total_rounds: 2,
            remaining: Duration::from_secs(15),
        });
        let img2 = image::load_from_memory(&hc.canvas_png())
            .expect("decodes")
            .to_rgba8();
        assert!(
            img2.pixels().all(|p| p.0 == PAPER),
            "the overlay is gone once the drawer's turn ends"
        );
    }

    #[test]
    fn the_drawer_overlay_reconciles_an_echoed_undo() {
        let mut hc = client_pair().1;
        seed_drawer(&mut hc, 0);
        hc.draw_stroke(vec![(10, 10), (100, 100)], [1, 2, 3, 255], 4); // own op id 0
        hc.draw_stroke(vec![(200, 200), (300, 300)], [1, 2, 3, 255], 4); // own op id 1
        assert_eq!(hc.fold().own_ops.len(), 2);
        // The server pops the last op (dense id 1) and echoes CanvasUndo to ALL incl the
        // drawer; it resolves against the drawer's own overlay by that id.
        hc.apply(ServerEvent::CanvasUndo { removed_id: 1 });
        assert_eq!(
            hc.fold().own_ops.len(),
            1,
            "the echoed undo removed the drawer's op 1"
        );
        assert!(matches!(
            hc.fold().own_ops[0],
            CanvasOp::Stroke { id: 0, .. }
        ));
    }

    #[test]
    fn a_canvas_log_reseed_replaces_the_drawer_overlay_without_double_drawing() {
        let mut hc = client_pair().1;
        seed_drawer(&mut hc, 0);
        hc.draw_stroke(vec![(10, 10), (100, 100)], [1, 2, 3, 255], 4); // own op id 0
        assert_eq!(hc.fold().own_ops.len(), 1);
        // A reconnect reseed: the surviving op is now in the AUTHORITATIVE log; the overlay
        // is dropped so the op is never drawn twice, and the id counter realigns.
        hc.apply(ServerEvent::CanvasLog {
            ops: vec![CanvasOp::Stroke {
                id: 0,
                points: vec![(10, 10), (100, 100)],
                color: [1, 2, 3, 255],
                radius: 4,
            }],
        });
        assert!(
            hc.fold().own_ops.is_empty(),
            "the reseed replaced the overlay"
        );
        assert_eq!(
            hc.replica().canvas_ops.len(),
            1,
            "the op is in the authoritative log now"
        );
        assert_eq!(
            hc.fold().next_own_op_id,
            1,
            "the per-turn id counter realigned to the log"
        );
    }

    // --- The connection banner (W5-review Important 2) -----------------------

    /// A HeadlessClient over a CLOSED in-process client (`status() == Closed`).
    fn closed_client() -> HeadlessClient<InProcClient> {
        let (_server, mut clients) = InProcessTransport::new_pair(1);
        clients[0].drop_conn();
        HeadlessClient::new(clients.remove(0))
    }

    #[test]
    fn a_closed_transport_report_leads_with_a_connection_banner() {
        let mut hc = closed_client();
        assert_eq!(hc.status(), ConnStatus::Closed);
        hc.apply(ServerEvent::Welcome {
            seat: 0,
            room_code: "ROOM01".to_string(),
            reconnect_token: "t".to_string(),
            protocol_version: PROTOCOL_VERSION,
        });
        let report = hc.state_report();
        assert!(
            report.starts_with("> ⚠ CONNECTION LOST"),
            "a closed report leads with the connection banner: {report}"
        );
        assert!(report.contains("rejoin required"));
    }

    // --- Progress coalescing per stroke_id (W5-review minor 4) ---------------

    #[test]
    fn progress_batches_of_one_stroke_are_coalesced_across_the_seam() {
        let mut hc = client_pair().1;
        // Two batches of ONE stroke with a gap between them (150→250 at y=50), each
        // carrying only its own points.
        hc.apply(ServerEvent::CanvasStrokeProgress {
            stroke_id: 7,
            points: vec![(50, 50), (150, 50)],
            color: [10, 10, 12, 255],
            radius: 6,
        });
        hc.apply(ServerEvent::CanvasStrokeProgress {
            stroke_id: 7,
            points: vec![(250, 50), (350, 50)],
            color: [10, 10, 12, 255],
            radius: 6,
        });
        let img = image::load_from_memory(&hc.canvas_png())
            .expect("decodes")
            .to_rgba8();
        // The seam pixel (200,50) is inked ONLY if the batches were coalesced (stamp_points
        // interpolated across the boundary); two separately-stamped batches leave a gap.
        assert_ne!(
            img.get_pixel(200, 50).0,
            PAPER,
            "the batch seam is bridged (the stroke was coalesced per stroke_id)"
        );
    }

    #[test]
    fn a_new_stroke_id_replaces_stale_progress() {
        let mut hc = client_pair().1;
        // Stroke A top-left; then a NEW stroke_id B elsewhere — A is abandoned (replaced).
        hc.apply(ServerEvent::CanvasStrokeProgress {
            stroke_id: 1,
            points: vec![(20, 20), (40, 20)],
            color: [10, 10, 12, 255],
            radius: 4,
        });
        hc.apply(ServerEvent::CanvasStrokeProgress {
            stroke_id: 2,
            points: vec![(600, 400)],
            color: [10, 10, 12, 255],
            radius: 4,
        });
        let img = image::load_from_memory(&hc.canvas_png())
            .expect("decodes")
            .to_rgba8();
        assert_eq!(
            img.get_pixel(30, 20).0,
            PAPER,
            "the abandoned stroke A was dropped when B replaced it"
        );
        assert_ne!(
            img.get_pixel(600, 400).0,
            PAPER,
            "the current stroke B is painted"
        );
    }
}
