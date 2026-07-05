//! The authoritative session (spec §2, §5, §6) — the transport-agnostic heart that
//! owns the game and does **all** rule enforcement and per-recipient redaction, with
//! no I/O and no wall-clock reads.
//!
//! ## The seat / connection split
//!
//! The `Session` speaks only in **seats**. A [`crate::transport::ConnId`] is a
//! connection identity that the transport pump (the room actor / the solo plugin /
//! the test harness) maps to a seat: it decodes a `Create`/`Join` frame, calls
//! [`Session::connect`] to allocate or re-attach a seat, and remembers the
//! connection↔seat binding; every later gameplay frame it maps `ConnId → seat` and
//! calls [`Session::handle`]. Draining ([`Session::drain_events`]) yields
//! [`Recipient`]-addressed events the pump fans back out to connections. Keeping the
//! `ConnId` map out of the `Session` is what lets the same authority run behind the
//! in-process, WebSocket, and (M6) P2P transports unchanged.
//!
//! ## The canvas is an op log, not pixels (spec §2.2)
//!
//! The `Session` holds the per-turn [`CanvasOp`] log and **rasterizes nothing** — the
//! op log is the sync primitive; each replica (and `dooduel_mcp::get_canvas`) derives
//! its own raster via [`crate::canvas::PaintBuffer`]. A client `Stroke` intent carries
//! a client-chosen `stroke_id` that can span several `done: false` batches; the
//! server reconciles it to one monotonic server op id — see [`Session::on_stroke`].
//!
//! ## Redaction (spec §5)
//!
//! Every word-bearing event is rendered per recipient through
//! [`crate::game::Game::knows`] / [`crate::game::Game::word_display_for`]: the secret
//! string only ever reaches a seat that already knows it (the drawer, a correct
//! guesser mid-turn) plus the legitimate `TurnEnded` broadcast at the reveal. The
//! secrecy scan (`tests/session_secrecy.rs`) is the load-bearing guard.

use std::time::Duration;

use crate::game::{
    ChatMsg, Config, Game, GuessOutcome, PRESET_NAMES, Phase, PlayerSpec,
};
use crate::protocol::{
    CanvasOp, ClientIntent, ErrorCode, MAX_STROKE_POINTS, PROTOCOL_VERSION, ReplicaPlayer,
    RoomReplica, ServerEvent, WireAvatar,
};

/// The grace window a disconnected seat is held before it is vacated (spec §6.3). A
/// valid-token `Join` inside this window re-attaches; past it the seat frees.
pub const GRACE: Duration = Duration::from_secs(45);

/// The maximum number of seats a lobby will admit (M1 cap; M2 makes it a host
/// setting). A fresh `Join` past this gets [`ErrorCode::RoomFull`].
pub const MAX_SEATS: usize = 8;

/// Where a staged [`ServerEvent`] is addressed (spec §3.3). The pump resolves this to
/// concrete connections: `All` fans to every seated connection; `Seat(n)` goes to
/// seat `n`'s connection only (dropped if it currently has none). Redacted events
/// (`WordUpdate`, `WordChoices`, `RoomState`) are staged **per seat** with
/// already-redacted content, so the addressing carries no secret an addressee may not
/// see.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Recipient {
    /// Every seated connection (an event identical for all — no per-seat secret).
    All,
    /// One seat's connection only.
    Seat(usize),
}

/// Injected policy (spec §6.3, §8) — keeps `dooduel_core` entropy-free and the tests
/// deterministic. The server supplies a `getrandom`-backed token generator and
/// `fill_bots_to: 0`; the solo path and tests supply a seeded generator and (solo)
/// `fill_bots_to: 4`.
pub struct SessionOpts {
    /// Yields a fresh ≥128-bit token per call (spec §6.3). Rotated on every
    /// (re)connection. Server: `getrandom`-backed. Solo/tests: seeded + deterministic.
    pub token_gen: Box<dyn FnMut() -> String + Send>,
    /// On `StartMatch`, pad the roster up to this **total** seat count with
    /// [`PRESET_NAMES`] bots (spec §8). Solo: 4. Networked M1 rooms: 0.
    pub fill_bots_to: usize,
    /// The room's invite code (server-generated; spec §6.2). Reported in `Welcome` /
    /// `RoomState`. The `Session` never generates it (that is the registry's job).
    pub room_code: String,
}

/// A seat's connection + identity state, held alongside (and index-aligned with) the
/// game's `players`. Identity (name/avatar) lives here because it exists in the lobby
/// before `game.players` does; score/occupancy live in `game` once the match starts.
struct SeatState {
    name: String,
    avatar: WireAvatar,
    is_bot: bool,
    /// A live connection is bound (the pump knows which one).
    connected: bool,
    /// `Some(t)`: disconnected at `t`, held in grace until `t + GRACE`. A grace seat
    /// still counts as host-eligible (spec §6.2).
    away_since: Option<Duration>,
    /// The current reconnect token (rotated on every (re)connection; empty for bots
    /// and freed seats).
    token: String,
    /// A monotonic join ordinal for host migration (earliest-joined wins), preserved
    /// across reconnect.
    join_ord: u64,
    /// `false` once the seat is fully gone (grace expired / `Leave`).
    present: bool,
}

/// A stroke being accumulated across `done: false` batches (spec §3.5). The first
/// batch of a `stroke_id` opens it with a fresh monotonic server op id; matching
/// batches append points; `done: true` (or any other op / a differing `stroke_id`)
/// closes it into one [`CanvasOp::Stroke`].
struct OpenStroke {
    server_id: u64,
    stroke_id: u64,
    points: Vec<(i32, i32)>,
    color: [u8; 4],
    radius: i32,
}

/// A pre-mutation snapshot used to diff the game after a `handle`/`tick` and stage the
/// resulting phase/hint/countdown/chat events exactly once (see [`Session::resync`]).
struct Pre {
    phase: Phase,
    hints_revealed: usize,
    chat_len: usize,
    remaining_secs: u64,
}

/// The authoritative networked session (spec §2). One per room. Owns the game, the
/// seat roster, and the per-turn canvas op log; validates every intent (spec §3.2),
/// mutates, and stages per-recipient events. Performs no I/O and reads no wall clock —
/// `now: Duration` arrives via `tick`/`connect`/`disconnect` (tests use virtual time).
pub struct Session {
    room_code: String,
    config: Config,
    game: Game,
    started: bool,
    seats: Vec<SeatState>,
    host: usize,
    /// The per-turn op log — the sync primitive (spec §2.2). Cleared at each turn
    /// start; the raster is never held here (each replica derives its own).
    canvas_ops: Vec<CanvasOp>,
    open_stroke: Option<OpenStroke>,
    /// Monotonic op-id source (spec §2.2 "server-assigned monotonic ids"). Never
    /// resets; the log is what clears, so ids are unambiguous within any turn's log.
    next_op_id: u64,
    next_join_ord: u64,
    outbox: Vec<(Recipient, ServerEvent)>,
    opts: SessionOpts,
}

impl Session {
    /// A fresh lobby session (no seats, no match). `config` is applied at `StartMatch`.
    pub fn new(config: Config, opts: SessionOpts) -> Self {
        let room_code = opts.room_code.clone();
        Session {
            room_code,
            config,
            game: Game::default(),
            started: false,
            seats: Vec::new(),
            host: 0,
            canvas_ops: Vec::new(),
            open_stroke: None,
            next_op_id: 0,
            next_join_ord: 0,
            outbox: Vec::new(),
            opts,
        }
    }

    // --- Connection lifecycle (spec §6.3) -----------------------------------

    /// Admit a connection: a fresh join (lobby only) allocates a new seat, or a valid
    /// `reconnect` token re-attaches to the held seat. Rotates the seat's token on
    /// every (re)connection and stages `Welcome` + `RoomState` (+ `CanvasLog` mid-match)
    /// to the seat and a `Roster` to all. Returns the seat index (the pump binds it to
    /// the connection). Version/room-code gating is the pump/registry's job (spec §3.1);
    /// `connect` owns seat allocation + reconnection.
    pub fn connect(
        &mut self,
        name: &str,
        avatar: WireAvatar,
        reconnect: Option<&str>,
        _now: Duration,
    ) -> Result<usize, ErrorCode> {
        if let Some(tok) = reconnect {
            let found = self.seats.iter().position(|s| {
                s.present && !s.is_bot && !s.token.is_empty() && s.token == tok
            });
            let Some(seat) = found else {
                return Err(ErrorCode::BadToken);
            };
            self.seats[seat].connected = true;
            self.seats[seat].away_since = None;
            self.seats[seat].token = (self.opts.token_gen)();
            self.seats[seat].name = name.to_string();
            self.seats[seat].avatar = avatar;
            self.recompute_host();
            self.stage_welcome(seat);
            self.stage_room_state(seat);
            if self.started {
                self.stage_canvas_log(seat);
            }
            self.broadcast_roster();
            return Ok(seat);
        }

        // A fresh join. M1 seats new players only in the lobby; mid-match arrival is
        // reconnect-only (token-based). A typo'd token that missed the branch above
        // falls here and is treated as a fresh join in the lobby.
        if self.started {
            return Err(ErrorCode::RoomFull);
        }
        if self.seats.iter().filter(|s| s.present).count() >= MAX_SEATS {
            return Err(ErrorCode::RoomFull);
        }
        let seat = self.seats.len();
        let join_ord = self.next_join_ord;
        self.next_join_ord += 1;
        let token = (self.opts.token_gen)();
        self.seats.push(SeatState {
            name: name.to_string(),
            avatar,
            is_bot: false,
            connected: true,
            away_since: None,
            token,
            join_ord,
            present: true,
        });
        self.recompute_host();
        self.stage_welcome(seat);
        self.stage_room_state(seat);
        self.broadcast_roster();
        Ok(seat)
    }

    /// A connection dropped: hold the seat "away" and start its grace window (spec
    /// §6.3). Does not vacate or end the turn — [`Session::tick`] does that on grace
    /// expiry. A host that drops into grace stays host until the window elapses.
    pub fn disconnect(&mut self, seat: usize, now: Duration) {
        if let Some(s) = self.seats.get_mut(seat) {
            if s.present && s.connected {
                s.connected = false;
                s.away_since = Some(now);
            }
        }
        self.recompute_host();
        self.broadcast_roster();
    }

    // --- Intent intake (spec §3.2 gate table) -------------------------------

    /// The sole entry for a **seated** connection's intent (the pump has already
    /// resolved `from` = seat). Validates per the §3.2 gate table; a violation stages
    /// an `Error` to `from` only and leaves state untouched. `Create`/`Join` never
    /// arrive here (the pump routes them to [`Session::connect`]).
    pub fn handle(&mut self, from: usize, intent: ClientIntent) {
        let pre = self.snapshot();
        match intent {
            ClientIntent::Create { .. } | ClientIntent::Join { .. } => {
                self.error(from, ErrorCode::Malformed, "connection intent on a seated connection");
            }
            ClientIntent::StartMatch => self.on_start_match(from),
            ClientIntent::Pick { index } => self.on_pick(from, index),
            ClientIntent::Guess { text } => self.on_guess(from, &text),
            ClientIntent::Stroke {
                stroke_id,
                points,
                color,
                radius,
                done,
            } => self.on_stroke(from, stroke_id, points, color, radius, done),
            ClientIntent::Fill { seed, color } => self.on_fill(from, seed, color),
            ClientIntent::Undo => self.on_undo(from),
            ClientIntent::Clear => self.on_clear(from),
            ClientIntent::Continue => self.on_continue(from),
            ClientIntent::Leave => self.on_leave(from),
        }
        self.resync(pre);
    }

    fn on_start_match(&mut self, from: usize) {
        if self.game.phase != Phase::Idle {
            return self.error(from, ErrorCode::WrongPhase, "the match already started");
        }
        if from != self.host {
            return self.error(from, ErrorCode::NotHost, "only the host can start the match");
        }
        // Roster = the current seats (index-stable), padded with bots up to fill_bots_to.
        let mut roster: Vec<PlayerSpec> = self
            .seats
            .iter()
            .map(|s| PlayerSpec {
                name: s.name.clone(),
                is_bot: s.is_bot,
            })
            .collect();
        let mut bot_names = PRESET_NAMES.iter().cycle();
        while roster.len() < self.opts.fill_bots_to {
            let name = bot_names.next().expect("PRESET_NAMES cycles forever").to_string();
            let join_ord = self.next_join_ord;
            self.next_join_ord += 1;
            self.seats.push(SeatState {
                name: name.clone(),
                avatar: WireAvatar::Default,
                is_bot: true,
                connected: true,
                away_since: None,
                token: String::new(),
                join_ord,
                present: true,
            });
            roster.push(PlayerSpec { name, is_bot: true });
        }
        self.game.start_match(&roster, self.config.clone());
        self.started = true;
        // A lobby seat that left before the start becomes a vacant game seat (indices
        // stay 1:1 with the roster so the pump's connection map never renumbers).
        let gone: Vec<usize> = self
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.present)
            .map(|(i, _)| i)
            .collect();
        for i in gone {
            self.game.vacate_seat(i);
        }
        self.recompute_host();
        self.broadcast_roster();
        // handle()'s trailing resync emits PhaseChanged(Picking)+CanvasCleared+
        // WordChoices(drawer)+the round banner.
    }

    fn on_pick(&mut self, from: usize, index: usize) {
        if self.game.phase != Phase::Picking {
            return self.error(from, ErrorCode::WrongPhase, "not the picking phase");
        }
        if from != self.game.seat_index {
            return self.error(from, ErrorCode::NotDrawer, "only the drawer may pick");
        }
        let Some(word) = self.game.word_choices.get(index).cloned() else {
            return self.error(from, ErrorCode::Malformed, "word choice index out of range");
        };
        self.game.choose_word(word);
        // resync emits PhaseChanged(Drawing) + per-seat WordUpdate.
    }

    fn on_guess(&mut self, from: usize, text: &str) {
        if self.game.phase != Phase::Drawing {
            return self.error(from, ErrorCode::WrongPhase, "not the drawing phase");
        }
        // The drawer / an already-correct guesser are Ignored by apply_guess (a no-op,
        // no event) — the pure core's semantics; no error code exists for them.
        self.apply_guess_and_stage(from, text);
        // resync emits the (broadcast / private) chat line and, if all guessers now
        // have it, PhaseChanged→Reveal + TurnEnded.
    }

    fn on_stroke(
        &mut self,
        from: usize,
        stroke_id: u64,
        points: Vec<(i32, i32)>,
        color: [u8; 4],
        radius: i32,
        done: bool,
    ) {
        if !self.require_drawer_drawing(from) {
            return;
        }
        if points.len() > MAX_STROKE_POINTS {
            return self.error(from, ErrorCode::TooLarge, "stroke batch exceeds MAX_STROKE_POINTS");
        }
        // A batch for a *different* stroke closes the open one first.
        let mismatched = self
            .open_stroke
            .as_ref()
            .is_some_and(|op| op.stroke_id != stroke_id);
        if mismatched {
            self.finalize_open_stroke();
        }
        match &mut self.open_stroke {
            Some(op) => op.points.extend_from_slice(&points),
            None => {
                let id = self.next_op_id;
                self.next_op_id += 1;
                self.open_stroke = Some(OpenStroke {
                    server_id: id,
                    stroke_id,
                    points,
                    color,
                    radius,
                });
            }
        }
        if done {
            self.finalize_open_stroke();
        }
    }

    fn on_fill(&mut self, from: usize, seed: (i32, i32), color: [u8; 4]) {
        if !self.require_drawer_drawing(from) {
            return;
        }
        self.finalize_open_stroke();
        let id = self.next_op_id;
        self.next_op_id += 1;
        let op = CanvasOp::Fill { id, seed, color };
        self.canvas_ops.push(op.clone());
        self.broadcast_op_except(self.game.seat_index, op);
    }

    fn on_undo(&mut self, from: usize) {
        if !self.require_drawer_drawing(from) {
            return;
        }
        // An in-progress (un-broadcast) stroke is cancelled silently — it matches the
        // drawer's own optimistic undo (nobody else saw it).
        if self.open_stroke.take().is_some() {
            return;
        }
        if let Some(op) = self.canvas_ops.pop() {
            let removed_id = match op {
                CanvasOp::Stroke { id, .. } | CanvasOp::Fill { id, .. } => id,
            };
            // Undo confirmations DO go to everyone including the drawer (spec §3.5).
            self.broadcast(ServerEvent::CanvasUndo { removed_id });
        }
        // Nothing to undo ⇒ a benign no-op (no event).
    }

    fn on_clear(&mut self, from: usize) {
        if !self.require_drawer_drawing(from) {
            return;
        }
        self.open_stroke = None;
        self.canvas_ops.clear();
        self.broadcast(ServerEvent::CanvasCleared);
    }

    fn on_continue(&mut self, from: usize) {
        if self.game.phase != Phase::Reveal {
            return self.error(from, ErrorCode::WrongPhase, "not the reveal phase");
        }
        // Any seat may advance (spec §3.2 — accepted M1 semantics).
        self.game.continue_now();
        // resync emits PhaseChanged(next turn / Final).
    }

    fn on_leave(&mut self, from: usize) {
        // Graceful release — skips grace (spec §3.2). The pump drops the connection
        // binding after routing this.
        self.vacate(from);
    }

    /// The shared "drawer, in Drawing" gate for canvas intents (spec §3.2). Stages the
    /// precise error to `from` and returns `false` on a violation.
    fn require_drawer_drawing(&mut self, from: usize) -> bool {
        if self.game.phase != Phase::Drawing {
            self.error(from, ErrorCode::WrongPhase, "not the drawing phase");
            return false;
        }
        if from != self.game.seat_index {
            self.error(from, ErrorCode::NotDrawer, "only the drawer may draw");
            return false;
        }
        true
    }

    // --- The clock (spec §6.1) ----------------------------------------------

    /// One authoritative tick: expire any elapsed grace windows (vacating seats,
    /// force-ending a dropped drawer's turn), advance the game clock (countdown, hint
    /// flips, auto-pick, turn/match end), drain due bot guesses, and stage every
    /// resulting event. `now` is virtual — the caller owns the clock.
    pub fn tick(&mut self, now: Duration) {
        if !self.started {
            self.expire_grace(now);
            return;
        }
        let pre = self.snapshot();
        self.expire_grace(now);
        let pending = self.game.tick(now);
        for pg in pending {
            self.apply_guess_and_stage(pg.player, &pg.text);
        }
        self.resync(pre);
    }

    /// Vacate every seat whose grace window has elapsed (spec §6.3).
    fn expire_grace(&mut self, now: Duration) {
        let expired: Vec<usize> = self
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.present
                    && !s.connected
                    && s.away_since
                        .is_some_and(|t| now.saturating_sub(t) >= GRACE)
            })
            .map(|(i, _)| i)
            .collect();
        for seat in expired {
            self.vacate(seat);
        }
    }

    /// Free a seat: mark it gone, invalidate its token, vacate it in the game (if the
    /// match is running), migrate the host if it was the host, force-end the turn if it
    /// was the drawer mid-Drawing (spec §6.3 — Picking is covered by auto-pick), and
    /// broadcast the new roster.
    fn vacate(&mut self, seat: usize) {
        let Some(s) = self.seats.get_mut(seat) else {
            return;
        };
        if !s.present {
            return;
        }
        s.present = false;
        s.connected = false;
        s.away_since = None;
        s.token = String::new();
        let was_drawer = self.started && self.game.current_drawer() == Some(seat);
        if self.started {
            self.game.vacate_seat(seat);
        }
        self.recompute_host();
        self.broadcast_roster();
        if was_drawer && self.game.phase == Phase::Drawing {
            self.game.force_end_turn();
            // The caller's resync (handle/tick) turns this Drawing→Reveal into TurnEnded.
        }
    }

    fn recompute_host(&mut self) {
        if let Some((i, _)) = self
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| s.present && !s.is_bot && (s.connected || s.away_since.is_some()))
            .min_by_key(|(_, s)| s.join_ord)
        {
            self.host = i;
        }
        // No eligible human ⇒ host unchanged; the match ends via occupancy < 2.
    }

    // --- Guess application --------------------------------------------------

    /// Apply one guess (human or bot) and stage its result events. The chat line(s) the
    /// pure core pushes are picked up by [`Session::resync`]'s chat diff; here we stage
    /// only the guesser-facing `GuessResult` and, on a correct guess, the guesser's
    /// word-row upgrade to the full word (they now `knows` it).
    fn apply_guess_and_stage(&mut self, player: usize, text: &str) {
        let before = self.game.players.get(player).map(|p| p.score).unwrap_or(0);
        let outcome = self.game.apply_guess(player, text);
        match outcome {
            GuessOutcome::Correct => {
                let delta = self.game.players[player].score - before;
                self.outbox.push((
                    Recipient::All,
                    ServerEvent::GuessResult {
                        seat: player,
                        correct: true,
                        points: delta,
                    },
                ));
                let display = self.game.word_display_for(player);
                let len = self.game.word_length();
                let hints_revealed = self.game.hints_revealed();
                self.outbox.push((
                    Recipient::Seat(player),
                    ServerEvent::WordUpdate {
                        display,
                        len,
                        hints_revealed,
                    },
                ));
            }
            GuessOutcome::Wrong | GuessOutcome::Close => {
                self.outbox.push((
                    Recipient::Seat(player),
                    ServerEvent::GuessResult {
                        seat: player,
                        correct: false,
                        points: 0,
                    },
                ));
            }
            GuessOutcome::Ignored => {}
        }
    }

    // --- Canvas op staging --------------------------------------------------

    /// Close the open stroke into one [`CanvasOp::Stroke`], append it to the log, and
    /// broadcast `CanvasOpApplied` to everyone **except** the drawer (no-echo, spec §3.5).
    fn finalize_open_stroke(&mut self) {
        if let Some(op) = self.open_stroke.take() {
            let canvas_op = CanvasOp::Stroke {
                id: op.server_id,
                points: op.points,
                color: op.color,
                radius: op.radius,
            };
            self.canvas_ops.push(canvas_op.clone());
            self.broadcast_op_except(self.game.seat_index, canvas_op);
        }
    }

    /// Stage `CanvasOpApplied{op}` to every seat except `except` (the drawer). Dead
    /// seats (no connection / bots) are dropped at drain time.
    fn broadcast_op_except(&mut self, except: usize, op: CanvasOp) {
        for i in 0..self.seats.len() {
            if i != except {
                self.outbox.push((
                    Recipient::Seat(i),
                    ServerEvent::CanvasOpApplied { op: op.clone() },
                ));
            }
        }
    }

    // --- Diff-based event synthesis -----------------------------------------

    fn snapshot(&self) -> Pre {
        Pre {
            phase: self.game.phase,
            hints_revealed: self.game.hints_revealed(),
            chat_len: self.game.chat.len(),
            remaining_secs: self.remaining_secs(),
        }
    }

    /// After a mutation, stage the events its state change implies: newly-pushed chat
    /// lines (addressed by their `to`), a phase transition's bundle, an in-phase hint
    /// flip's per-seat word updates, or a countdown tick.
    fn resync(&mut self, pre: Pre) {
        let new_chat: Vec<ChatMsg> = self.game.chat.iter().skip(pre.chat_len).cloned().collect();
        for m in new_chat {
            match m.to {
                None => self.broadcast(ServerEvent::ChatLine { line: m }),
                Some(s) => self.outbox.push((Recipient::Seat(s), ServerEvent::ChatLine { line: m })),
            }
        }

        if self.game.phase != pre.phase {
            self.emit_phase_change();
            return;
        }
        if self.game.phase == Phase::Drawing && self.game.hints_revealed() != pre.hints_revealed {
            self.emit_word_updates();
        }
        let now_secs = self.remaining_secs();
        if now_secs != pre.remaining_secs
            && matches!(self.game.phase, Phase::Picking | Phase::Drawing | Phase::Reveal)
        {
            self.broadcast(ServerEvent::CountdownSync {
                remaining: Duration::from_secs(now_secs),
            });
        }
    }

    /// Stage a phase transition's full bundle: the `PhaseChanged` clock event plus the
    /// phase-specific payload (word choices on Picking, per-seat word rows on Drawing,
    /// the reveal on Reveal, the podium on Final).
    fn emit_phase_change(&mut self) {
        let phase = self.game.phase;
        self.broadcast(ServerEvent::PhaseChanged {
            phase,
            drawer: self.game.current_drawer(),
            round: self.game.round_display(),
            total_rounds: self.config.total_rounds,
            remaining: Duration::from_secs(self.remaining_secs()),
        });
        match phase {
            Phase::Picking => {
                // The op log resets at each turn start (spec §2.2).
                self.canvas_ops.clear();
                self.open_stroke = None;
                self.broadcast(ServerEvent::CanvasCleared);
                let drawer = self.game.seat_index;
                let words = self.game.word_choices.clone();
                self.outbox
                    .push((Recipient::Seat(drawer), ServerEvent::WordChoices { words }));
            }
            Phase::Drawing => self.emit_word_updates(),
            Phase::Reveal => {
                let results = self.game.turn_results.clone();
                let word = self.game.secret_word.clone();
                self.broadcast(ServerEvent::TurnEnded { results, word });
            }
            Phase::Final => {
                let podium = self.podium();
                self.broadcast(ServerEvent::MatchEnded { podium });
            }
            Phase::Idle => {}
        }
    }

    /// Stage a per-seat, per-recipient-redacted `WordUpdate` to every present human
    /// seat (the drawer + correct guessers get the full word; others get blanks + the
    /// revealed hints).
    fn emit_word_updates(&mut self) {
        let seats: Vec<usize> = self
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| s.present && !s.is_bot)
            .map(|(i, _)| i)
            .collect();
        let len = self.game.word_length();
        let hints_revealed = self.game.hints_revealed();
        for seat in seats {
            let display = self.game.word_display_for(seat);
            self.outbox.push((
                Recipient::Seat(seat),
                ServerEvent::WordUpdate {
                    display,
                    len,
                    hints_revealed,
                },
            ));
        }
    }

    // --- Per-recipient replica rendering (spec §4.1, §5) --------------------

    /// The replica as `seat` is allowed to see it — the single redaction call site
    /// (`word_display_for(seat)` + drawer-only `word_choices` + `chat_for(seat)`).
    fn replica_for(&self, seat: usize) -> RoomReplica {
        RoomReplica {
            room_code: self.room_code.clone(),
            my_seat: seat,
            host: self.host,
            players: self.replica_players(),
            phase: self.game.phase,
            drawer: self.game.current_drawer(),
            round: self.game.round_display(),
            total_rounds: self.config.total_rounds,
            remaining: Duration::from_secs(self.remaining_secs()),
            word_display: self.game.word_display_for(seat),
            word_len: self.game.word_length(),
            hints_revealed: self.game.hints_revealed(),
            word_choices: if self.game.phase == Phase::Picking && seat == self.game.seat_index {
                self.game.word_choices.clone()
            } else {
                Vec::new()
            },
            chat: self.game.chat_for(seat).cloned().collect(),
            canvas_ops: self.canvas_ops.clone(),
            turn_results: self.game.turn_results.clone(),
            podium: if self.game.phase == Phase::Final {
                Some(self.podium())
            } else {
                None
            },
        }
    }

    /// The roster as every recipient sees it (no secret): seat-index-aligned, gone
    /// seats included as disconnected so `players[i]` always maps to seat `i`.
    fn replica_players(&self) -> Vec<ReplicaPlayer> {
        self.seats
            .iter()
            .enumerate()
            .map(|(i, s)| ReplicaPlayer {
                name: s.name.clone(),
                avatar: s.avatar.clone(),
                connected: s.connected,
                is_bot: s.is_bot,
                score: self.game.players.get(i).map(|p| p.score).unwrap_or(0),
                guessed: self.game.turn_guesses.iter().any(|g| g.player == i),
            })
            .collect()
    }

    fn podium(&self) -> Vec<(usize, String, i64)> {
        self.game
            .standings()
            .into_iter()
            .map(|(i, p)| (i, p.name, p.score))
            .collect()
    }

    /// Seconds remaining in the current phase (0 outside a timed phase).
    fn remaining_secs(&self) -> u64 {
        match self.game.phase {
            Phase::Picking => self.game.pick_seconds_left,
            Phase::Drawing => self.game.draw_seconds_left,
            Phase::Reveal => self.game.reveal_seconds_left,
            _ => 0,
        }
    }

    // --- Event staging helpers ----------------------------------------------

    /// Drain the staged per-recipient events (the pump sends them and re-polls).
    pub fn drain_events(&mut self) -> Vec<(Recipient, ServerEvent)> {
        std::mem::take(&mut self.outbox)
    }

    fn broadcast(&mut self, ev: ServerEvent) {
        self.outbox.push((Recipient::All, ev));
    }

    fn error(&mut self, seat: usize, code: ErrorCode, message: &str) {
        self.outbox.push((
            Recipient::Seat(seat),
            ServerEvent::Error {
                code,
                message: message.to_string(),
            },
        ));
    }

    fn broadcast_roster(&mut self) {
        let players = self.replica_players();
        self.broadcast(ServerEvent::Roster { players });
    }

    fn stage_welcome(&mut self, seat: usize) {
        let reconnect_token = self.seats[seat].token.clone();
        let room_code = self.room_code.clone();
        self.outbox.push((
            Recipient::Seat(seat),
            ServerEvent::Welcome {
                seat,
                room_code,
                reconnect_token,
                protocol_version: PROTOCOL_VERSION,
            },
        ));
    }

    fn stage_room_state(&mut self, seat: usize) {
        let replica = self.replica_for(seat);
        self.outbox
            .push((Recipient::Seat(seat), ServerEvent::RoomState(replica)));
    }

    fn stage_canvas_log(&mut self, seat: usize) {
        let ops = self.canvas_ops.clone();
        self.outbox
            .push((Recipient::Seat(seat), ServerEvent::CanvasLog { ops }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deterministic, counter-based token generator (spec §6.3 tests use a seeded
    // generator, never entropy).
    fn counter_tokens() -> Box<dyn FnMut() -> String + Send> {
        let mut n = 0u64;
        Box::new(move || {
            n += 1;
            format!("tok-{n}")
        })
    }

    fn opts(fill: usize) -> SessionOpts {
        SessionOpts {
            token_gen: counter_tokens(),
            fill_bots_to: fill,
            room_code: "ABC123".to_string(),
        }
    }

    fn new_session(fill: usize) -> Session {
        Session::new(Config::default(), opts(fill))
    }

    fn d(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    type Events = Vec<(Recipient, ServerEvent)>;

    fn error_to(evs: &Events, seat: usize) -> Option<ErrorCode> {
        evs.iter().find_map(|(r, e)| match (r, e) {
            (Recipient::Seat(s), ServerEvent::Error { code, .. }) if *s == seat => Some(code.clone()),
            _ => None,
        })
    }

    fn welcome_token(evs: &Events) -> Option<String> {
        evs.iter().find_map(|(_, e)| match e {
            ServerEvent::Welcome {
                reconnect_token, ..
            } => Some(reconnect_token.clone()),
            _ => None,
        })
    }

    /// Two connected humans (seats 0 host, 1). Match not yet started.
    fn two_humans() -> Session {
        let mut s = new_session(0);
        s.connect("Ada", WireAvatar::Default, None, d(0)).unwrap();
        s.connect("Bo", WireAvatar::Default, None, d(0)).unwrap();
        s.drain_events();
        s
    }

    /// Two humans, match started, seat 0 (host) picked word 0 ⇒ phase Drawing.
    /// Returns the session and the secret word (white-box read for guess tests).
    fn drawing_two_humans() -> (Session, String) {
        let mut s = two_humans();
        s.handle(0, ClientIntent::StartMatch);
        s.handle(0, ClientIntent::Pick { index: 0 });
        s.drain_events();
        let secret = s.game.secret_word.clone();
        assert_eq!(s.game.phase, Phase::Drawing);
        (s, secret)
    }

    // --- Connection lifecycle (spec §6.3) -----------------------------------

    #[test]
    fn first_connect_seats_the_host_and_stages_welcome_state_roster() {
        let mut s = new_session(0);
        let seat = s.connect("Ada", WireAvatar::Default, None, d(0)).unwrap();
        assert_eq!(seat, 0);
        assert_eq!(s.host, 0);
        let evs = s.drain_events();
        assert!(matches!(
            evs.iter().find(|(r, _)| *r == Recipient::Seat(0)).map(|(_, e)| e),
            Some(ServerEvent::Welcome { seat: 0, .. })
        ));
        assert!(evs
            .iter()
            .any(|(r, e)| matches!((r, e), (Recipient::Seat(0), ServerEvent::RoomState(_)))));
        assert!(evs
            .iter()
            .any(|(r, e)| matches!((r, e), (Recipient::All, ServerEvent::Roster { .. }))));
    }

    #[test]
    fn a_second_connect_takes_seat_one_host_unchanged() {
        let s = two_humans();
        assert_eq!(s.seats.len(), 2);
        assert_eq!(s.host, 0, "the earliest-joined seat is host");
        assert!(s.seats.iter().all(|x| x.connected && x.present && !x.is_bot));
    }

    #[test]
    fn a_fresh_join_mid_match_is_rejected_room_full() {
        let (mut s, _) = drawing_two_humans();
        assert_eq!(
            s.connect("Cy", WireAvatar::Default, None, d(1)),
            Err(ErrorCode::RoomFull),
            "M1 seats fresh players only in the lobby; mid-match is reconnect-only"
        );
    }

    // --- Reconnect + token rotation (spec §6.3) -----------------------------

    #[test]
    fn reconnect_within_grace_reattaches_and_rotates_the_token() {
        let mut s = new_session(0);
        s.connect("Ada", WireAvatar::Default, None, d(0)).unwrap();
        s.connect("Bo", WireAvatar::Default, None, d(0)).unwrap();
        let evs = s.drain_events();
        // Bo's Welcome is the second one; capture its token by reconnecting Bo's seat.
        // (Seat 0's token is tok-1, seat 1's is tok-2 — the counter is deterministic.)
        let _ = evs;
        let t1 = "tok-2".to_string();

        s.disconnect(1, d(5));
        s.drain_events();
        let seat = s.connect("Bo", WireAvatar::Default, Some(&t1), d(10)).unwrap();
        assert_eq!(seat, 1, "the valid token re-attaches the same seat");
        assert!(s.seats[1].connected);
        assert!(s.seats[1].away_since.is_none(), "grace cleared");
        let evs = s.drain_events();
        let rotated = welcome_token(&evs).unwrap();
        assert_ne!(rotated, t1, "the token rotates on every (re)connection");
        assert!(evs
            .iter()
            .any(|(r, e)| matches!((r, e), (Recipient::Seat(1), ServerEvent::RoomState(_)))));
    }

    #[test]
    fn a_used_or_bogus_token_is_bad_token() {
        let mut s = new_session(0);
        s.connect("Ada", WireAvatar::Default, None, d(0)).unwrap(); // seat 0, tok-1
        s.connect("Bo", WireAvatar::Default, None, d(0)).unwrap(); // seat 1, tok-2
        s.drain_events();
        s.disconnect(1, d(5));
        // Re-attach with tok-2 ⇒ rotates to tok-3.
        s.connect("Bo", WireAvatar::Default, Some("tok-2"), d(6)).unwrap();
        s.drain_events();
        // The old token is single-use now.
        assert_eq!(
            s.connect("Bo", WireAvatar::Default, Some("tok-2"), d(7)),
            Err(ErrorCode::BadToken),
            "a rotated token is not accepted again"
        );
        assert_eq!(
            s.connect("Zed", WireAvatar::Default, Some("nope"), d(8)),
            Err(ErrorCode::BadToken),
            "a bogus token is rejected"
        );
    }

    #[test]
    fn reconnect_mid_match_reseeds_with_room_state_and_canvas_log() {
        let (mut s, _) = drawing_two_humans();
        // Draw one stroke so the log is non-empty.
        s.handle(
            0,
            ClientIntent::Stroke {
                stroke_id: 1,
                points: vec![(1, 1), (2, 2)],
                color: [0, 0, 0, 255],
                radius: 3,
                done: true,
            },
        );
        s.drain_events();
        // Seat 1 drops and rejoins with its token (tok-2).
        s.disconnect(1, d(3));
        s.drain_events();
        s.connect("Bo", WireAvatar::Default, Some("tok-2"), d(4)).unwrap();
        let evs = s.drain_events();
        assert!(evs
            .iter()
            .any(|(r, e)| matches!((r, e), (Recipient::Seat(1), ServerEvent::RoomState(_)))));
        assert!(
            evs.iter().any(|(r, e)| matches!(
                (r, e),
                (Recipient::Seat(1), ServerEvent::CanvasLog { ops }) if !ops.is_empty()
            )),
            "a mid-match reconnect gets the full current-turn op log"
        );
    }

    // --- Host migration (spec §6.2) -----------------------------------------

    #[test]
    fn host_stays_in_grace_then_migrates_on_expiry() {
        let mut s = new_session(0);
        s.connect("Ada", WireAvatar::Default, None, d(0)).unwrap(); // seat 0 host
        s.connect("Bo", WireAvatar::Default, None, d(0)).unwrap(); // seat 1
        assert_eq!(s.host, 0);
        s.disconnect(0, d(10));
        assert_eq!(s.host, 0, "a host in grace keeps the host");
        s.tick(d(10 + 46)); // grace (45s) elapsed ⇒ seat 0 vacated
        assert_eq!(s.host, 1, "host migrates to the next earliest-joined seat");
        assert!(!s.seats[0].present);
    }

    // --- StartMatch + bot padding (spec §8) ---------------------------------

    #[test]
    fn start_match_is_host_and_lobby_gated() {
        let mut s = two_humans();
        // Non-host ⇒ NotHost.
        s.handle(1, ClientIntent::StartMatch);
        assert_eq!(error_to(&s.drain_events(), 1), Some(ErrorCode::NotHost));
        // Host ⇒ starts (Picking).
        s.handle(0, ClientIntent::StartMatch);
        assert_eq!(s.game.phase, Phase::Picking);
        let evs = s.drain_events();
        assert!(evs.iter().any(|(r, e)| matches!(
            (r, e),
            (Recipient::All, ServerEvent::PhaseChanged { phase: Phase::Picking, .. })
        )));
        assert!(evs.iter().any(|(r, e)| matches!(
            (r, e),
            (Recipient::Seat(0), ServerEvent::WordChoices { .. })
        )));
        // A second StartMatch ⇒ WrongPhase.
        s.handle(0, ClientIntent::StartMatch);
        assert_eq!(error_to(&s.drain_events(), 0), Some(ErrorCode::WrongPhase));
    }

    #[test]
    fn fill_bots_to_pads_the_roster_and_only_bots_auto_guess() {
        let mut s = new_session(4);
        s.connect("Ada", WireAvatar::Default, None, d(0)).unwrap(); // seat 0 human
        s.drain_events();
        s.handle(0, ClientIntent::StartMatch);
        assert_eq!(s.seats.len(), 4, "1 human + 3 bots = fill_bots_to");
        assert!(!s.seats[0].is_bot);
        assert!(s.seats[1..].iter().all(|x| x.is_bot));
        assert_eq!(s.game.players.len(), 4);
        // Seat 0 draws turn 1; pick a word and run the draw window.
        s.handle(0, ClientIntent::Pick { index: 0 });
        s.drain_events();
        let mut correct_seats: Vec<usize> = Vec::new();
        for sec in 0..=80 {
            s.tick(d(sec));
            for (_, e) in s.drain_events() {
                if let ServerEvent::GuessResult {
                    seat, correct: true, ..
                } = e
                {
                    correct_seats.push(seat);
                }
            }
            if s.game.phase != Phase::Drawing {
                break;
            }
        }
        correct_seats.sort_unstable();
        assert_eq!(correct_seats, vec![1, 2, 3], "the 3 bots guessed");
        assert!(
            !correct_seats.contains(&0),
            "the human seat is never bot-guessed"
        );
    }

    // --- The §3.2 gate matrix (every row: allowed + a rejected case) ---------

    #[test]
    fn pick_gate_phase_and_drawer() {
        let mut s = two_humans();
        s.handle(0, ClientIntent::StartMatch); // Picking, seat 0 drawer
        s.drain_events();
        // Non-drawer ⇒ NotDrawer.
        s.handle(1, ClientIntent::Pick { index: 0 });
        assert_eq!(error_to(&s.drain_events(), 1), Some(ErrorCode::NotDrawer));
        // Bad index ⇒ Malformed.
        s.handle(0, ClientIntent::Pick { index: 99 });
        assert_eq!(error_to(&s.drain_events(), 0), Some(ErrorCode::Malformed));
        // Drawer, valid index ⇒ Drawing.
        s.handle(0, ClientIntent::Pick { index: 0 });
        assert_eq!(s.game.phase, Phase::Drawing);
        // Pick again (wrong phase) ⇒ WrongPhase.
        s.handle(0, ClientIntent::Pick { index: 0 });
        assert_eq!(error_to(&s.drain_events(), 0), Some(ErrorCode::WrongPhase));
    }

    #[test]
    fn guess_gate_phase_drawer_and_correct_path() {
        let (mut s, secret) = drawing_two_humans();
        // Wrong phase (before Drawing) is covered elsewhere; here test drawer + correct.
        // The drawer cannot guess ⇒ Ignored (no GuessResult, no score change).
        s.handle(0, ClientIntent::Guess { text: secret.clone() });
        let evs = s.drain_events();
        assert!(
            !evs.iter().any(|(_, e)| matches!(e, ServerEvent::GuessResult { .. })),
            "a drawer's guess is ignored"
        );
        assert!(s.game.turn_guesses.is_empty());
        // A non-drawer's correct guess scores and upgrades their word row.
        s.handle(1, ClientIntent::Guess { text: secret.clone() });
        let evs = s.drain_events();
        assert!(evs.iter().any(|(r, e)| matches!(
            (r, e),
            (Recipient::All, ServerEvent::GuessResult { seat: 1, correct: true, .. })
        )));
        assert!(evs.iter().any(|(r, e)| matches!(
            (r, e),
            (Recipient::Seat(1), ServerEvent::WordUpdate { .. })
        )));
    }

    #[test]
    fn guess_before_drawing_is_wrong_phase() {
        let mut s = two_humans();
        s.handle(0, ClientIntent::StartMatch); // Picking
        s.drain_events();
        s.handle(1, ClientIntent::Guess { text: "robot".into() });
        assert_eq!(error_to(&s.drain_events(), 1), Some(ErrorCode::WrongPhase));
    }

    #[test]
    fn canvas_gate_drawer_and_drawing_plus_no_echo() {
        let (mut s, _) = drawing_two_humans();
        // Non-drawer stroke ⇒ NotDrawer.
        s.handle(
            1,
            ClientIntent::Stroke {
                stroke_id: 1,
                points: vec![(0, 0)],
                color: [0, 0, 0, 255],
                radius: 2,
                done: true,
            },
        );
        assert_eq!(error_to(&s.drain_events(), 1), Some(ErrorCode::NotDrawer));
        // Oversized batch ⇒ TooLarge.
        s.handle(
            0,
            ClientIntent::Stroke {
                stroke_id: 2,
                points: vec![(0, 0); MAX_STROKE_POINTS + 1],
                color: [0, 0, 0, 255],
                radius: 2,
                done: true,
            },
        );
        assert_eq!(error_to(&s.drain_events(), 0), Some(ErrorCode::TooLarge));
        // A valid drawer stroke ⇒ CanvasOpApplied to the guesser (seat 1), NOT the
        // drawer (no-echo, spec §3.5).
        s.handle(
            0,
            ClientIntent::Stroke {
                stroke_id: 3,
                points: vec![(1, 1), (5, 5)],
                color: [10, 10, 10, 255],
                radius: 3,
                done: true,
            },
        );
        let evs = s.drain_events();
        assert!(evs.iter().any(|(r, e)| matches!(
            (r, e),
            (Recipient::Seat(1), ServerEvent::CanvasOpApplied { .. })
        )));
        assert!(
            !evs.iter().any(|(r, e)| matches!(
                (r, e),
                (Recipient::Seat(0), ServerEvent::CanvasOpApplied { .. })
            )),
            "the drawer is not echoed its own op"
        );
        assert_eq!(s.canvas_ops.len(), 1);
    }

    #[test]
    fn undo_and_clear_are_drawer_confirmed_to_all() {
        let (mut s, _) = drawing_two_humans();
        let stroke = |id: u64| ClientIntent::Stroke {
            stroke_id: id,
            points: vec![(id as i32, id as i32)],
            color: [0, 0, 0, 255],
            radius: 2,
            done: true,
        };
        s.handle(0, stroke(1));
        s.handle(0, stroke(2));
        s.drain_events();
        assert_eq!(s.canvas_ops.len(), 2);
        // Undo removes the last complete op and confirms to ALL (including the drawer).
        s.handle(0, ClientIntent::Undo);
        let evs = s.drain_events();
        assert_eq!(s.canvas_ops.len(), 1);
        assert!(evs.iter().any(|(r, e)| matches!(
            (r, e),
            (Recipient::All, ServerEvent::CanvasUndo { .. })
        )));
        // Clear truncates the log and confirms CanvasCleared to ALL.
        s.handle(0, ClientIntent::Clear);
        let evs = s.drain_events();
        assert!(s.canvas_ops.is_empty());
        assert!(evs
            .iter()
            .any(|(r, e)| matches!((r, e), (Recipient::All, ServerEvent::CanvasCleared))));
    }

    #[test]
    fn continue_gate_reveal_only() {
        let (mut s, _) = drawing_two_humans();
        // Continue during Drawing ⇒ WrongPhase.
        s.handle(1, ClientIntent::Continue);
        assert_eq!(error_to(&s.drain_events(), 1), Some(ErrorCode::WrongPhase));
        // Force to Reveal, then any seat may Continue.
        s.game.force_end_turn();
        s.drain_events();
        assert_eq!(s.game.phase, Phase::Reveal);
        s.handle(1, ClientIntent::Continue);
        assert_ne!(s.game.phase, Phase::Reveal, "Continue advanced the turn");
    }

    #[test]
    fn leave_vacates_the_seat_and_reroster() {
        let (mut s, _) = drawing_two_humans();
        // Seat 1 (a guesser) leaves gracefully.
        s.handle(1, ClientIntent::Leave);
        assert!(!s.seats[1].present, "the seat is vacated immediately (no grace)");
        assert!(!s.game.players[1].occupied);
        let evs = s.drain_events();
        assert!(evs
            .iter()
            .any(|(r, e)| matches!((r, e), (Recipient::All, ServerEvent::Roster { .. }))));
    }

    #[test]
    fn drawer_drop_past_grace_force_ends_the_turn() {
        let (mut s, _) = drawing_two_humans(); // seat 0 drawer, Drawing
        s.disconnect(0, d(5));
        s.tick(d(5)); // not expired
        assert_eq!(s.game.phase, Phase::Drawing);
        s.tick(d(5 + 46)); // grace elapsed ⇒ drawer vacated ⇒ force_end_turn
        assert_eq!(s.game.phase, Phase::Reveal);
        let evs = s.drain_events();
        assert!(
            evs.iter().any(|(_, e)| matches!(e, ServerEvent::TurnEnded { .. })),
            "the dropped drawer's turn ends with a TurnEnded broadcast"
        );
    }

    // --- Tick-driven transitions --------------------------------------------

    #[test]
    fn auto_pick_and_countdown_emit_events() {
        let mut s = two_humans();
        s.handle(0, ClientIntent::StartMatch); // Picking
        s.drain_events();
        // Run the pick timeout: the game auto-picks ⇒ PhaseChanged(Drawing) + WordUpdate.
        let mut saw_drawing = false;
        for sec in 0..=crate::game::PICK_SECS {
            s.tick(d(sec));
            for (r, e) in s.drain_events() {
                if let (Recipient::All, ServerEvent::PhaseChanged { phase: Phase::Drawing, .. }) =
                    (r, &e)
                {
                    saw_drawing = true;
                }
            }
        }
        assert!(saw_drawing, "the pick timeout auto-advanced to Drawing");
        assert_eq!(s.game.phase, Phase::Drawing);
    }
}
