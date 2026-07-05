# Dooduel production multiplayer — M1 implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` (recommended) or `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Realize the [M1 spine design](../specs/2026-07-04-dooduel-multiplayer-m1-design.md) (rev-2): humans (native + web) and MCP agents play one authoritative Dooduel match to the podium over a real WebSocket, with solo-vs-bots preserved as the in-process verification path.

**Architecture:** Extract a pure Bevy-free `dooduel_core` (game rules + wire protocol + `Session` authority + transport trait + op-log canvas); refactor the GUI from authoritative to a `RoomReplica` fed by `Msg::Net`; add a tokio-free `dooduel_server` (actor-model rooms) and a headless `dooduel_mcp` client (hand-rolled stdio MCP). Spec rev-2 is the contract — **when this plan and the spec disagree, the spec wins; flag the conflict rather than improvising.**

**Tech stack:** existing workspace + `bevy_reflect 0.19.0` (already a lock node), `ewebsock =0.8.0` (client WS, native+wasm), `async-tungstenite 0.34 smol-runtime` + `async-net` (server WS, tokio-free), `serde_json` wire (existing dep), `image` (existing dep; MCP `get_canvas` only).

**Wave = one PR-sized unit.** Commit freely inside a wave; each wave ends gated + reviewed. **Nothing is pushed/PR'd/merged without the user's explicit go** (per-milestone gate; the branch is `feat/dooduel-multiplayer-m1` off `origin/main @ 6e07954`).

---

## File structure (locked here; tasks reference it)

```
apps/dooduel_core/              NEW package "dooduel_core" (pure; deps: bevy_reflect, serde, serde_json)
├── Cargo.toml
└── src/
    ├── lib.rs                  module root + crate doc (the authority/transport split)
    ├── game.rs                 MOVED from apps/dooduel/src/game.rs (+ §2.3 API delta)
    ├── canvas.rs               NEW: PaintBuffer + CanvasOp (pure fns moved from paint.rs)
    ├── protocol.rs             NEW: ClientIntent / ServerEvent / RoomReplica / limits
    ├── session.rs              NEW: Session (authority; no I/O)
    └── transport.rs            NEW: ServerTransport / ClientTransport traits + InProcessTransport
                                + WsClientTransport behind feature "ws-client" (ewebsock; wave 4)

apps/dooduel/                   existing package (GUI lib + windowed bin)
├── Cargo.toml                  + dep dooduel_core (feature ws-client from wave 4)
└── src/
    ├── game.rs                 DELETED (re-export shim in lib.rs during wave 0)
    ├── paint.rs                PaintSurface wraps dooduel_core::canvas::PaintBuffer
    ├── lib.rs                  model split (RoomReplica + local UI state), Msg::Net, intents-out
    ├── net.rs                  NEW: NetPlugin (transport pump) + LocalAuthorityPlugin (solo)
    ├── view/*.rs               reads re-pointed game::Game → RoomReplica; SwitchSeat affordances removed
    └── bin/playtest_host.rs    DELETED in wave 3 (superseded by dooduel_mcp in wave 5 — see W3 note)

apps/dooduel_server/            NEW package "dooduel_server" (native bin; doc = false)
├── Cargo.toml                  deps: dooduel_core, async-tungstenite(smol-runtime), async-net,
│                               async-executor, async-io, futures-lite, serde_json
│                               dev-deps: dooduel_mcp (lib, for the e2e — wave 5)
├── src/
│   ├── main.rs                 args/env, port-0 + stdout port line, accept loop
│   ├── registry.rs             RoomCode → room task registry, Create/Join routing, GC
│   ├── room.rs                 the per-room actor: intake + 10 Hz tick + flush
│   └── wire.rs                 WS conn tasks ↔ room channels; frame/size/rate limits
└── tests/e2e.rs                CARGO_BIN_EXE two-process e2e (wave 5 completes it)

apps/dooduel_mcp/               NEW package "dooduel_mcp" (native bin + LIB target; doc = false)
├── Cargo.toml                  deps: dooduel_core(ws-client), serde_json, image
└── src/
    ├── lib.rs                  HeadlessClient: replica upkeep, honest seat view, canvas raster
    ├── mcp.rs                  hand-rolled stdio JSON-RPC 2.0 (initialize/tools/ping)
    └── main.rs                 bin: stdio loop wiring tools → HeadlessClient

Cargo.toml                      workspace members += the three packages; bevy_reflect in [workspace.dependencies]
.github/workflows/ci.yml        wave 4: dooduel_web wasm check; lock+deny commit discipline
docs/README.md                  index updates ship with each wave that changes doc-visible state
```

## Cross-cutting rules (apply to every wave)

- **SG gate** (the repo's check command; run before every wave-closing commit):
  `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked && cargo nextest run --workspace --locked -j 2`
  (Linux: prefix tests with `xvfb-run -a` if a display-needing test appears; none should — M1 touches no render code, so **no RG/GPU lane is required** unless a wave unexpectedly touches `crates/`.)
- **Lockfile discipline:** the first commit that changes `Cargo.lock` (wave 0 adds packages; wave 4 adds deps) is its **own commit** and runs `cargo deny check` first (house rule; deps were pre-verified against deny.toml in spec §10).
- **TDD:** every behavior lands red-first — write the failing test, see it fail, implement, see it pass. Tests live at the lowest tier that observes the behavior (pure `dooduel_core` tests before probe tests before e2e).
- **No `unwrap()` on wire input.** Malformed/hostile input → `Error` event + log, never a panic (spec §6.1; the `playtest_host` discipline).
- **RUST_MIN_STACK:** dooduel test binaries need `RUST_MIN_STACK=33554432` (existing campaign fact) — already in CI env; export locally when running the app packages' tests.
- **Run the artifact:** waves 3, 4, 5 end with a real run (solo GUI match / networked GUI match / MCP match) — green gates alone do not close a wave.
- **Spec conflicts:** if implementation reveals a spec error, stop, note it in the wave's commit body, and update the spec in the same commit (supersede, don't silently drift).

---

### Wave 0 — extract `dooduel_core` (pure move, zero behavior change)

**Files:** Create `apps/dooduel_core/{Cargo.toml,src/lib.rs,src/game.rs,src/canvas.rs}`. Modify root `Cargo.toml` (members + `bevy_reflect` workspace dep), `apps/dooduel/Cargo.toml`, `apps/dooduel/src/{lib.rs,paint.rs}`. Delete `apps/dooduel/src/game.rs` (moved).

- [ ] **W0.1 Package skeleton.** `apps/dooduel_core/Cargo.toml`:

```toml
[package]
name = "dooduel_core"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
# The ONE Bevy-family dep (Reflect derives for MVU record/replay of Msg::Net).
bevy_reflect = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[features]
# wave 4 adds: ws-client = ["dep:ewebsock"]
```

Root `Cargo.toml`: add `"apps/dooduel_core"` to members (before `"apps/dooduel"`); add `bevy_reflect = "0.19.0"` to `[workspace.dependencies]` (matches the existing lock node — **no new lock entry**).

- [ ] **W0.2 Move `game.rs`.** `git mv apps/dooduel/src/game.rs apps/dooduel_core/src/game.rs`; change `use bevy::prelude::Reflect` → `use bevy_reflect::Reflect`. In `apps/dooduel/src/lib.rs` replace `pub mod game;` with `pub use dooduel_core::game;` (path-stable for `view/`, tests, bins). The `#[cfg(test)]` game tests **move with the file** (they are pure).
- [ ] **W0.3 Extract the pure canvas.** Create `apps/dooduel_core/src/canvas.rs`: move from `paint.rs` the pure items — `stamp_disc`, `stroke_segment`, `flood_fill`, `PAPER`, `PALETTE`, `BRUSH_SIZES`, the eraser ×1.6 rule — plus a new bevy-free buffer extracted from `PaintSurface`'s pure half:

```rust
/// The Bevy-free pixel surface: RGBA8 pixels + brush state + the undo ring.
/// PaintSurface (apps/dooduel/src/paint.rs) becomes a thin Bevy wrapper
/// (Image mirror + pointer observers) around this.
pub struct PaintBuffer {
    pub width: usize, pub height: usize,
    pub pixels: Vec<u8>,
    pub tool: Tool, pub color: [u8; 4], pub radius: i32,
    last: Option<(i32, i32)>,
    undo: Vec<Vec<u8>>,             // ring, UNDO_DEPTH
}
impl PaintBuffer {
    pub fn new(w: usize, h: usize) -> Self;
    pub fn begin(&mut self, x: i32, y: i32);
    pub fn extend(&mut self, x: i32, y: i32);
    pub fn end(&mut self);
    pub fn fill(&mut self, x: i32, y: i32);
    pub fn clear(&mut self);
    pub fn undo(&mut self) -> bool;
}
```

`Tool` moves here too (`paint.rs` re-exports it — `Msg::SelectTool(paint::Tool)` and view code stay untouched).
- [ ] **W0.4 Re-point `paint.rs`.** `PaintSurface` delegates pixels/brush/undo to an inner `PaintBuffer`; keeps `enabled`, `to_pixel`, Bevy `Image` mirroring, observers. All existing paint tests must pass unchanged.
- [ ] **W0.5 Purity guard test.** `apps/dooduel_core/src/lib.rs` doc + a test asserting the dep boundary at the build level is CI's job, but add the cheap tripwire now — `cargo tree -p dooduel_core -e normal` must not contain `bevy ` (umbrella). Run manually and record in the commit body; the CI-visible guard is the wasm check (wave 4).
- [ ] **W0.6 Gate + commit.** Full SG. Expected: identical test counts (the move is behavior-neutral; game tests now run in `dooduel_core`). Two commits: (1) lockfile-only member addition + `cargo deny check` (deny is a no-op — no new deps — but the lock changes with the new package); (2) the move.

### Wave 1 — protocol types + the `game::Game` API delta

**Files:** Create `apps/dooduel_core/src/protocol.rs`. Modify `apps/dooduel_core/src/{lib.rs,game.rs}`, `apps/dooduel/src/lib.rs` (roster-call sites), `apps/dooduel/src/bin/playtest_host.rs` (start call site).

- [ ] **W1.1 Protocol types (red-first: round-trip tests).** `protocol.rs` per spec §3 — the exact wire surface:

```rust
pub const PROTOCOL_VERSION: u32 = 1;
// Limits (spec §3.1) — also the DoS guard:
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_STROKE_POINTS: usize = 256;
pub const MAX_GUESS_LEN: usize = 128;
pub const MAX_NAME_LEN: usize = 32;
pub const ROOM_CODE_LEN: usize = 6;
pub const MAX_AVATAR_PNG: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum WireAvatar { Default, Preset { icon: usize, tint: usize }, Custom { png: Vec<u8> } }

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum CanvasOp {
    Stroke { id: u64, points: Vec<(i32, i32)>, color: [u8; 4], radius: i32, erase: bool },
    Fill { id: u64, seed: (i32, i32), color: [u8; 4] },
}

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum ClientIntent {
    Create { name: String, avatar: WireAvatar, protocol_version: u32 },
    Join { room: String, name: String, avatar: WireAvatar, protocol_version: u32,
           reconnect: Option<String> },
    StartMatch,
    Pick { index: usize },
    Guess { text: String },
    Stroke { stroke_id: u64, points: Vec<(i32, i32)>, color: [u8; 4], size: i32,
             tool: ToolWire, done: bool },
    Fill { seed: (i32, i32), color: [u8; 4] },
    Undo, Clear, Continue, Leave,
}

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum ServerEvent {
    Welcome { seat: usize, room_code: String, reconnect_token: String, protocol_version: u32 },
    RoomState(RoomReplica),
    Roster { players: Vec<ReplicaPlayer> },
    PhaseChanged { phase: Phase, drawer: Option<usize>, round: u32, total_rounds: u32,
                   remaining: Duration },
    CountdownSync { remaining: Duration },
    WordUpdate { display: String, len: usize, hints_revealed: usize },
    WordChoices { words: Vec<String> },                       // drawer only
    CanvasOpApplied { op: CanvasOp },
    CanvasUndo { removed_id: u64 },
    CanvasCleared,
    CanvasLog { ops: Vec<CanvasOp> },                         // late join / reconnect
    ChatLine { line: ChatMsg },
    GuessResult { seat: usize, correct: bool, points: i64 },
    TurnEnded { results: Vec<TurnResult>, word: String },
    MatchEnded { podium: Vec<(usize, String, i64)> },
    Error { code: ErrorCode, message: String },
}

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum ErrorCode { VersionMismatch, RoomNotFound, RoomFull, NotHost, NotDrawer,
                     WrongPhase, BadToken, RateLimited, TooLarge, Malformed }

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq, Default)]
pub struct RoomReplica {
    pub room_code: String, pub my_seat: usize, pub host: usize,
    pub players: Vec<ReplicaPlayer>,
    pub phase: Phase, pub drawer: Option<usize>, pub round: u32, pub total_rounds: u32,
    pub remaining: Duration,                                  // re-anchored on receipt (§4.3)
    pub word_display: String, pub word_len: usize, pub hints_revealed: usize,
    pub word_choices: Vec<String>,
    pub chat: Vec<ChatMsg>,
    pub canvas_ops: Vec<CanvasOp>,
    pub turn_results: Vec<TurnResult>,
    pub podium: Option<Vec<(usize, String, i64)>>,
}

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub struct ReplicaPlayer { pub name: String, pub avatar: WireAvatar, pub connected: bool,
                           pub is_bot: bool, pub score: i64, pub guessed: bool }
```

(`ToolWire` = `{ Brush, Eraser, Fill }` — the wire never carries UI-only state. `Phase`, `ChatMsg`, `TurnResult` gain `Serialize/Deserialize` in `game.rs`.)
Tests (same file, `#[cfg(test)]`): serde_json round-trip for a populated value of EVERY variant (`assert_eq!(orig, serde_json::from_str(&serde_json::to_string(&orig)?)?)`), and a **negative-invariant compile-level check**: `RoomReplica` has no field named `secret_word`/`rng`/`used_words` (grep-style test over `std::any::type_name` is silly — instead this is a doc-comment invariant enforced by the secrecy scan in W2).
- [ ] **W1.2 Run round-trips red→green; commit** `feat(dooduel_core): wire protocol types (M1 spec §3)`.
- [ ] **W1.3 Game API delta (red-first, one sub-step each; spec §2.3).** In `game.rs`:
  1. `pub struct PlayerSpec { pub name: String, pub is_bot: bool }`; new `start_match(roster: &[PlayerSpec], config: Config)` — old signature becomes `start_match_solo(human_name, config)` building `[human, Priya(bot), Theo(bot), Sam(bot)]` and delegating (all existing call sites switch to it mechanically).
  2. `Player` gains `is_bot: bool, occupied: bool`; `seeded_bot_plans`/due-guess drain key off `is_bot` (test: a 2-human roster never bot-guesses on a human seat — the exact bug the gate found).
  3. Vacant-seat semantics: `pub fn vacate_seat(&mut self, seat)`; rotation skips `!occupied`; `guesser_count()`/`all_guessed()` count occupied non-drawer seats; occupancy < 2 ⇒ `Final` (tests for each: rotation-skip, count, early-final).
  4. `pub fn force_end_turn(&mut self)` — public path to the existing turn-end (test: mid-Drawing force ends to Reveal with results).
  5. `pub fn knows(&self, seat) -> bool` (three-way, spec §5.1) + `pub fn word_display_for(&self, seat) -> String`; existing `word_display()` delegates to `word_display_for(self.viewing_as)` (removed in W3).
- [ ] **W1.4 Gate + commit** `feat(dooduel_core): Game roster/redaction API delta (M1 spec §2.3)`.

### Wave 2 — `Session` + `InProcessTransport` (the authority, headless)

**Files:** Create `apps/dooduel_core/src/{session.rs,transport.rs}`. Modify `lib.rs` (exports).

- [ ] **W2.1 Transport trait + InProcessTransport (red-first).** Per spec §2.4 exactly (non-blocking `try_recv`, addressed `send`, `disconnects`); `InProcessTransport` = paired `VecDeque`s (`new_pair(n_clients) -> (ServerEnd, Vec<ClientEnd>)`). Test: send/recv round-trip, per-recipient addressing isolation.
- [ ] **W2.2 Session core.** `session.rs`:

```rust
pub struct Session {
    game: Game,
    canvas: PaintBuffer,               // authoritative raster (derived cache)
    canvas_ops: Vec<CanvasOp>,         // the per-turn op log — the sync primitive
    next_op_id: u64,
    seats: Vec<SeatState>,             // conn binding, token, disconnect deadline
    host: usize,
    outbox: Vec<(Recipient, ServerEvent)>,   // Recipient = Seat(usize) | All
    config: Config,
    started: bool,
}
impl Session {
    pub fn new(config: Config) -> Self;
    /// Sole entry for client input. Validates (spec §3.2 gate table), mutates, stages events.
    pub fn handle(&mut self, from: SeatId, intent: ClientIntent);
    /// Clock + bot drain + hint flips + auto-pick + turn/match end + grace expiry (spec §6.1).
    pub fn tick(&mut self, now: Duration);
    /// Drain staged per-recipient events (the transport layer sends them).
    pub fn drain_events(&mut self) -> Vec<(Recipient, ServerEvent)>;
    pub fn connect(&mut self, name, avatar, reconnect: Option<&str>, now) -> Result<usize, ErrorCode>;
    pub fn disconnect(&mut self, seat: usize, now: Duration);
    fn replica_for(&self, seat: usize) -> RoomReplica;         // per-recipient render (§4.1)
}
```

Build order, each red-first: (1) connect/roster/host + `Welcome`/`RoomState`/`Roster` events; (2) `StartMatch` host-gate + `PhaseChanged`/`WordChoices`; (3) `Pick` + `Guess` through `apply_guess` → `GuessResult`/`ChatLine`/`WordUpdate` upgrades; (4) canvas intents → op validation/append/`CanvasOpApplied`, `Undo` → remove-last + `CanvasUndo`, `Clear`, no-echo-to-originator (spec §3.5); (5) `tick`: countdown, hint flip re-sends, auto-pick, bot drain via `is_bot`, turn end → `TurnEnded`, match end → `MatchEnded`; (6) disconnect/grace/token rotation/`force_end_turn` on drawer-drop, host migration, `Leave`, vacate.
- [ ] **W2.3 The secrecy scan (spec §9.2 — the load-bearing test).** Scripted seeded 2-round match, 4 seats (1 drawer rotating, words chosen from the pool with no substring collisions in scripted guesses); for every guesser seat and every turn: serialize every event addressed to that seat; assert the secret (case-insensitive) absent from the JSON **before** `min(that seat's correct guess, TurnEnded)`; assert the drawer's stream DOES carry it (the test proves the scan can see).
- [ ] **W2.4 Op-log determinism + late-join equivalence.** (a) two `PaintBuffer`s fed the same op log are pixel-identical; (b) replay-after-undo == incremental application with the undo applied; (c) a "late joiner" seeded by `CanvasLog` mid-turn ends pixel-identical to a from-start replica after subsequent ops + an undo reaching pre-join ops.
- [ ] **W2.5 In-process full match.** One `Session` + 4 scripted `InProcessTransport` clients play to podium (guesses, strokes, continues); assert podium scores against the same script run directly on `Game` (the authority adds no scoring drift).
- [ ] **W2.6 Gate + commit** (likely 3–4 commits along the build order).

### Wave 3 — the client-replica refactor + solo-over-Session (GUI runs on `RoomReplica`)

**Files:** Modify `apps/dooduel/src/{lib.rs,paint.rs}`, `apps/dooduel/src/view/*.rs`. Create `apps/dooduel/src/net.rs`. Delete `apps/dooduel/src/bin/playtest_host.rs`. Modify `apps/dooduel/src/bin/capture.rs` (model-shape follow).

**Note (spec §11 sequencing deviation, deliberate):** `playtest_host` drives the pre-replica model directly and cannot survive this wave; it is deleted **here**, and its successor (`dooduel_mcp`) lands in wave 5 — acceptable because the branch merges as one gated series (the capability gap never reaches `main`). Its pure per-seat-view rendering tests were superseded by W2.3's per-recipient replica tests.

- [ ] **W3.1 Model split (spec §4.1).** `Dooduel` drops `game: Game`; gains `replica: RoomReplica`, `net: NetState` (`enum NetState { Offline, Solo, Joining, Connected { .. }, Dropped { .. } }`), `chat_input: String` (moved OUT of `Game` — delete the core field + `viewing_as`/`switch_seat` and the `word_display()` shim from `game.rs` now). `Msg` delta: `+ Net(ServerEvent)`, `+ CreateRoom`/`SubmitJoin` become intent-sending, `- SwitchSeat`, gameplay arms (`ChooseWord`, `SubmitGuess`, `Continue`, `StartMatch`, tool strokes' commit path) **send intents instead of mutating** (spec §4.2); `Msg::Net(MatchEnded)` lifts `Screen::Podium` (not `Tick`).
- [ ] **W3.2 The net pump.** `net.rs`: `NetPlugin` — a `ClientNet` resource holding `Box<dyn ClientTransport>`; system in `MvuSet::Enqueue` draining `try_recv()` → `enqueue(Msg::Net(ev))`; outbound helper the reducer's `Cmd`s call. `LocalAuthorityPlugin` (solo): owns `Session` + the in-process pair; a system pumps `session.tick(Time::elapsed())` + routes intents/events (the same loop shape `dooduel_server` runs — spec §8).
- [ ] **W3.3 View re-point.** Mechanical: `s.game.X` → `s.replica.X` / accessor equivalents across `view/*.rs`; countdown from the monotonic re-anchored `remaining` (spec §4.3 — anchor on `Msg::Net` receipt, clamp no-upward-jump); pick overlay keyed on `!replica.word_choices.is_empty()`; the lobby becomes the **minimal live lobby** (live `Roster`, host-gated Start, server `room_code`); `SwitchSeat` affordances deleted (roster chips become inert badges; the "Switch to {drawer}" waiting-overlay button is removed).
- [ ] **W3.4 Probe tests.** Existing `lib.rs` probe tests re-target: solo match flows through `LocalAuthorityPlugin` (start → pick → guess via intents → podium). New: feed a scripted `ServerEvent` sequence into `Msg::Net`, assert the view via `BuiyProbePlugin` snapshots (guesser never shows the word pre-reveal; countdown displays; chat renders). Replay test: record a solo session, replay byte-identical (spec §3.4).
- [ ] **W3.5 RUN THE ARTIFACT.** `cargo run -p dooduel` — play a full solo match vs 3 bots to the podium; drawing, guessing, theming, avatar editor all live. Record findings in the commit body.
- [ ] **W3.6 Gate + commit series.**

### Wave 4 — `dooduel_server` + the WS transports + networked GUI

**Files:** Create `apps/dooduel_server/*` (structure above). Modify `apps/dooduel_core/{Cargo.toml,src/transport.rs}` (feature `ws-client` + `WsClientTransport` via ewebsock — plan-level refinement of spec §2.1 so `dooduel` and `dooduel_mcp` share one impl). Modify `apps/dooduel/Cargo.toml` (+`ws-client`), `apps/dooduel/src/net.rs` (Create/Join wiring), root `Cargo.toml` + `Cargo.lock`, `.github/workflows/ci.yml`.

- [ ] **W4.1 Deps commit (lock + deny, own commit).** Add `ewebsock = "=0.8.0"`, `async-tungstenite = { version = "0.34", default-features = false, features = ["smol-runtime"] }`, `async-net`, `async-executor`, `async-io`, `futures-lite` to `[workspace.dependencies]`; run `cargo deny check` (expected clean per spec §10 — dual tungstenite 0.24/0.29 is allowed); commit lockfile delta alone.
- [ ] **W4.2 `WsClientTransport`** (`dooduel_core`, `feature = "ws-client"`): ewebsock connect; `try_recv` maps `WsMessage::Text` → `serde_json::from_str::<ServerEvent>` (decode error ⇒ log + skip, never panic); `send` serializes intents; `status()` from ewebsock events. Unit test behind the feature: intent→text→intent round-trip through the framing fns (no live socket at this tier).
- [ ] **W4.3 Server skeleton.** `main.rs`: parse `--port` (default 0) / `DOODUEL_ADDR`; bind `async_net::TcpListener`; print `LISTENING port=<n>` to stdout (machine-readable — the e2e's discovery line, spec §9.5); accept loop spawns conn tasks on a static `async_executor::Executor` driven by `async_io::block_on`. `wire.rs`: per-conn read task (frame-size cap 64 KiB, rate cap ~30 intents/s sustained → `Error{RateLimited}`/disconnect on abuse) forwarding decoded intents into the room channel; write task draining the conn's outbox. `registry.rs`: `Create` → CSPRNG 6-char `[A-Z0-9]` code collision-checked; `Join` unknown ⇒ `Error{RoomNotFound}`; route to the room task; GC on all-empty + grace. `room.rs`: the actor — single owner of `Session`; `select!`-style loop over (intent channel, 100 ms tick timer); after each intake/tick, `drain_events()` → per-recipient conn outboxes; per-turn transcript log line (acceptance evidence, spec §1.4).
- [ ] **W4.4 Server-side tests.** In-package: registry Create/Join/GC semantics with a fake conn channel (no socket); room-actor determinism (scripted intents interleaved with ticks == same script against a bare `Session`); token re-attach over reconnect including rotation + old-conn replacement (spec §6.3).
- [ ] **W4.5 Networked GUI.** `net.rs`: `CreateRoom`/`SubmitJoin` open a `WsClientTransport` to `DOODUEL_SERVER_URL` (default `ws://127.0.0.1:7878`); `NetState` drives the Join/Lobby screens (spinner on `Joining`, error toasts on `Error` events, `Dropped` → rejoin-with-token attempt).
- [ ] **W4.6 CI delta.** Add to `ci.yml`: a `dooduel-web-check` step in the web-smoke job (`cargo check --target wasm32-unknown-unknown -p dooduel_web` with the job's existing RUSTFLAGS) — the wasm-graph purity gate (spec §10 CI delta). New packages ride the existing workspace jobs automatically; verify the MSRV job passes locally with `RUSTUP_TOOLCHAIN=1.95 cargo check --workspace` before committing.
- [ ] **W4.7 RUN THE ARTIFACT.** Terminal 1: `cargo run -p dooduel_server`. Terminals 2+3: two `cargo run -p dooduel` instances — create a room in one, join by code in the other, play a 2-human match (draw on one screen, watch it appear on the other; guess; podium). Record findings.
- [ ] **W4.8 Gate + commit series.**

### Wave 5 — `dooduel_mcp` + the two-process e2e

**Files:** Create `apps/dooduel_mcp/*` (structure above). Modify `apps/dooduel_server/Cargo.toml` (dev-dep on `dooduel_mcp` lib) + `apps/dooduel_server/tests/e2e.rs`.

- [ ] **W5.1 `HeadlessClient` (lib, red-first).** Wraps `WsClientTransport` + a `RoomReplica`: `pump()` (drain events → replica), `state_report() -> String` (the honest per-seat markdown view — phase/word-display/scores/chat-tail/actions-now; the `playtest_host` seat-view content, now replica-fed), `canvas_png() -> Vec<u8>` (rasterize `replica.canvas_ops` through `PaintBuffer` + `image` PNG encode), plus intent passthroughs (`pick/guess/stroke/fill/undo/clear/continue_turn/leave`). Tests: feed scripted events, assert the report redacts exactly as the replica does; canvas_png round-trips through `image::load_from_memory`.
- [ ] **W5.2 The MCP surface (`mcp.rs`).** Hand-rolled newline-delimited JSON-RPC 2.0 over stdio (spec §7): `initialize` (advertise ONLY `tools`), `notifications/initialized`, `tools/list` (the 11 tools with JSON-schema params), `tools/call` (dispatch → `HeadlessClient`), `ping`. stderr = logs. Unit tests drive it as `fn handle(line: &str) -> Option<String>` — no process spawn at this tier; malformed JSON-RPC ⇒ error response, never a panic.
- [ ] **W5.3 The two-process e2e** (`apps/dooduel_server/tests/e2e.rs`, spec §9.5): spawn `env!("CARGO_BIN_EXE_dooduel_server")`, parse `LISTENING port=`, connect 4 `dooduel_mcp::HeadlessClient`s (lib, in-test — the bins are exercised, the MCP stdio layer is W5.2-covered), scripted full match to podium with condition-based waits (poll `pump()` for expected events, deadline 60 s, no sleeps-as-sync, no retry reliance); assert podium + a reconnect mid-match (drop one client, rejoin with token, assert `RoomState`+`CanvasLog` resync).
- [ ] **W5.4 RUN THE ARTIFACT.** Launch the server + one real `dooduel_mcp` process; drive it by hand over stdio (`initialize`, `tools/call join_room`, `get_state`) against a live GUI match. Record findings.
- [ ] **W5.5 Gate + commit series.**

### Wave 6 — acceptance + docs closeout

**Files:** Create `docs/reports/2026-07-05-dooduel-m1-acceptance.md` (+assets). Modify `docs/README.md`, `docs/specs/2026-07-04-dooduel-multiplayer-m1-design.md` (status), `docs/plans/follow-ups.md` (anything deferred en route).

- [ ] **W6.1 Acceptance harness.** A documented fish-safe launch recipe (bash script under `apps/dooduel_mcp/examples/` or a README section): start `dooduel_server`, N agent seats (each = one `dooduel_mcp` process an LLM agent drives), the room code handshake. The server's per-turn transcript is the evidence stream.
- [ ] **W6.2 THE ACCEPTANCE RUN (manual, with the user).** The user in the native GUI (and/or web) + 3 MCP-agent seats play a full match to the podium over the live server (spec §1.4). Evidence: server transcript, per-seat reports, screenshots.
- [ ] **W6.3 Close the books.** Acceptance report in `docs/reports/`; spec status `draft` → `active`; docs/README index rows (spec `[active]`, plan `[landed]`, report); follow-ups ledger for anything discovered-but-deferred; final full SG + summary for the user's push/PR/merge decision.

---

## Risk register

| Risk | Wave | Mitigation / rollback |
|---|---|---|
| The `game.rs` move breaks hidden `bevy::prelude` leaks (e.g. `Duration` re-export differences) | 0 | The move is `git mv` + one import line; SG catches everything; rollback = revert one commit |
| View re-point (W3.3) is wide and mechanical-but-error-prone | 3 | Probe tests per screen before manual run; the solo RUN gate is the real check; commit per screen if needed |
| ewebsock 0.8.0 staleness bites on wasm (web-sys drift) | 4 | The W4.6 wasm check catches at CI-time; fallback recorded in spec §10 (tokio stack) is a wave-4-local swap |
| smol-executor lifetime/pinning friction in the server loop | 4 | The room actor is transport-agnostic — worst case the accept loop alone moves to the tokio fallback without touching `Session` |
| e2e flake under CI parallelism | 5 | port-0 + stdout discovery + condition waits + generous deadlines; if an OS proves hostile, platform-gate with a stated reason (spec §9.5) rather than retries |
| Replica/view drift (a view reads a field the replica doesn't carry) | 3 | The compile does the audit — `game` field is deleted, so every stale read is a build error, not a runtime surprise |

## Self-review (run after writing — done 2026-07-04)

1. **Spec coverage:** every rev-2 section maps to a wave (§2.1→W0, §2.3→W1.3, §3→W1.1, §2.2+§3.5→W2.2/W2.4, §4→W3, §5+§9.2→W2.3, §6→W4.3/W4.4, §7→W5.1/W5.2, §8→W3.2, §9.5→W5.3, §10 CI→W4.1/W4.6, §1.4→W6). Gap check: spec §3.1 rate limits land in W4.3 (`wire.rs`) — noted inline. ✔
2. **Placeholder scan:** no TBDs; the two intentionally-deferred details (exact `SeatState` fields, ewebsock URL config plumbing) are named with their owning step. ✔
3. **Type consistency:** `PaintBuffer`/`CanvasOp`/`RoomReplica`/`Session::handle/tick/drain_events` names match across W0–W5; `start_match_solo` call-site migration named in both W1.3 and W3. ✔
