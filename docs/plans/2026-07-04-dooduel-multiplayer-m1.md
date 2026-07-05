# Dooduel production multiplayer — M1 implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` (recommended) or `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Revision 2** — the 3-reviewer plan gate applied (all three CHANGES-REQUIRED; 23 findings confirmed after adversarial verification, 6 refuted, 2 nits — headline fixes: injected entropy, the bot-entry mechanism, base64 avatars + consistent caps, the real game-test location, an automated purity tripwire, `canvas_e2e.rs` in W3, per-IP join rate limiting, port defaults). Spec touched in the same commit (rev-2.1: §3.1 avatar arithmetic, §2.2 op shape).

**Goal:** Realize the [M1 spine design](../specs/2026-07-04-dooduel-multiplayer-m1-design.md) (rev-2.1): humans (native + web) and MCP agents play one authoritative Dooduel match to the podium over a real WebSocket, with solo-vs-bots preserved as the in-process verification path.

**Architecture:** Extract a pure Bevy-free `dooduel_core` (game rules + wire protocol + `Session` authority + transport trait + op-log canvas); refactor the GUI from authoritative to a `RoomReplica` fed by `Msg::Net`; add a tokio-free `dooduel_server` (actor-model rooms) and a headless `dooduel_mcp` client (hand-rolled stdio MCP). Spec rev-2.1 is the contract — **when this plan and the spec disagree, the spec wins; flag the conflict rather than improvising.**

**Tech stack:** existing workspace + `bevy_reflect 0.19.0` (already a lock node), `ewebsock =0.8.0` (client WS, native+wasm), `async-tungstenite 0.34 smol-runtime` + `async-net` (server WS, tokio-free), `getrandom` (server-side entropy; already a transitive lock node), `serde_json` wire + `base64` (existing deps), `image` (existing dep; MCP `get_canvas` only).

**Wave = one PR-sized unit.** Commit freely inside a wave; each wave ends gated + reviewed. **Nothing is pushed/PR'd/merged without the user's explicit go** (the branch is `feat/dooduel-multiplayer-m1` off `origin/main @ 6e07954`).

---

## File structure (locked here; tasks reference it)

```
apps/dooduel_core/              NEW package "dooduel_core" (pure; deps: bevy_reflect, serde, serde_json, base64)
├── Cargo.toml
├── src/
│   ├── lib.rs                  module root + crate doc (the authority/transport split)
│   ├── game.rs                 MOVED from apps/dooduel/src/game.rs (+ §2.3 API delta)
│   ├── canvas.rs               NEW: PaintBuffer + eraser_radius() (pure fns moved from paint.rs)
│   ├── protocol.rs             NEW: ClientIntent / ServerEvent / RoomReplica / CanvasOp / limits
│   ├── session.rs              NEW: Session + SessionOpts (authority; no I/O; entropy INJECTED)
│   └── transport.rs            NEW: ServerTransport / ClientTransport traits + InProcessTransport
│                               + WsClientTransport behind feature "ws-client" (ewebsock; wave 4)
└── tests/purity.rs             NEW: automated bevy-free dep-tree tripwire (W0.5)

apps/dooduel/                   existing package (GUI lib + windowed bin)
├── Cargo.toml                  + dep dooduel_core (feature ws-client from wave 4)
├── src/
│   ├── game.rs                 DELETED (moved; `pub use dooduel_core::game;` keeps paths stable)
│   ├── paint.rs                PaintSurface wraps dooduel_core::canvas::PaintBuffer; re-exports Tool
│   ├── lib.rs                  model split (RoomReplica + local UI state), Msg::Net, intents-out;
│   │                           pure game tests EXTRACTED to dooduel_core in W0 (they live in lib.rs::tests today)
│   ├── net.rs                  NEW: NetPlugin (transport pump) + LocalAuthorityPlugin (solo)
│   ├── view/*.rs               reads re-pointed game::Game → RoomReplica; SwitchSeat affordances removed
│   └── bin/{capture.rs,playtest_host.rs}   capture follows the model shape; playtest_host DELETED in W3
└── tests/canvas_e2e.rs         W3: ported to the LocalAuthorityPlugin solo path

apps/dooduel_server/            NEW package "dooduel_server" (native bin; doc = false)
├── Cargo.toml                  deps: dooduel_core, async-tungstenite(smol-runtime), async-net,
│                               async-executor, async-io, futures-lite, serde_json, getrandom
│                               dev-deps: dooduel_mcp (lib, for the e2e — wave 5)
├── src/
│   ├── main.rs                 args/env (default port 7878; --port 0 supported), stdout port line, accept loop
│   ├── registry.rs             RoomCode → room task registry, Create/Join routing, per-IP rate limit, GC
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
  (M1 touches no render code, so **no RG/GPU lane is required** unless a wave unexpectedly touches `crates/`.)
- **Lockfile discipline:** the first commit that changes `Cargo.lock` (wave 0 adds packages; wave 4 adds deps) is its **own commit** and runs `cargo deny check` first (deps pre-verified against deny.toml in spec §10).
- **TDD:** every behavior lands red-first — write the failing test, see it fail, implement, see it pass. Tests live at the lowest tier that observes the behavior (pure `dooduel_core` tests before probe tests before e2e).
- **No `unwrap()` on wire input.** Malformed/hostile input → `Error` event + log, never a panic (spec §6.1).
- **RUST_MIN_STACK:** **not** currently in ci.yml (rev-2 correction — the earlier "already in CI" claim was false). Export `RUST_MIN_STACK=33554432` locally when running app-package test binaries; if a wave's CI run SIGSEGVs building app binaries, add job-level `RUST_MIN_STACK: "33554432"` to the test jobs as that wave's CI delta (W4.6 is the natural home).
- **Run the artifact:** waves 3, 4, 5 end with a real run (solo GUI match / networked GUI match / MCP match) — green gates alone do not close a wave.
- **Spec conflicts:** if implementation reveals a spec error, stop, note it in the wave's commit body, and update the spec in the same commit (supersede, don't silently drift).

---

### Wave 0 — extract `dooduel_core` (pure move, zero behavior change)

**Files:** Create `apps/dooduel_core/{Cargo.toml,src/lib.rs,src/game.rs,src/canvas.rs,tests/purity.rs}`. Modify root `Cargo.toml` (members + `bevy_reflect` workspace dep), `apps/dooduel/Cargo.toml`, `apps/dooduel/src/{lib.rs,paint.rs}`. Delete `apps/dooduel/src/game.rs` (moved).

- [x] **W0.1 Package skeleton.** `apps/dooduel_core/Cargo.toml`:

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
base64 = { workspace = true }        # WireAvatar::Custom png_base64 (W1)

[features]
# wave 4 adds: ws-client = ["dep:ewebsock"]
```

Root `Cargo.toml`: add `"apps/dooduel_core"` to members (before `"apps/dooduel"`); add `bevy_reflect = "0.19.0"` to `[workspace.dependencies]` (matches the existing lock node — **no new lock entry**).

- [x] **W0.2 Move `game.rs` + extract its tests.** `git mv apps/dooduel/src/game.rs apps/dooduel_core/src/game.rs`; change `use bevy::prelude::Reflect` → `use bevy_reflect::Reflect`. In `apps/dooduel/src/lib.rs` replace `pub mod game;` with `pub use dooduel_core::game;` (path-stable for `view/`, tests, bins). **The pure game tests do NOT live in game.rs** (rev-2 correction) — they live in `apps/dooduel/src/lib.rs`'s `tests` module: extract the pure ones (`normalize_*`, `close_matches_*`, `guesser_points_*`, `drawer_points_*`, `match_starts_*`, `pick_timeout_*`, `choosing_a_word_*`, the tick/countdown suite — everything using only `Game`/`Config`, no App/probe) into a `#[cfg(test)] mod tests` in `dooduel_core/src/game.rs`, moving the `started()`/`tick_to()` helpers with them; probe/ECS tests stay in `apps/dooduel`.
- [x] **W0.3 Extract the pure canvas.** Create `apps/dooduel_core/src/canvas.rs`: move from `paint.rs` the pure items — `stamp_circle`, `stroke_segment`, `flood_fill`, `PAPER`, `PALETTE`, `BRUSH_SIZES`, `Tool` — and add `pub fn eraser_radius(base: i32) -> i32` (the ×1.6 rule extracted as a helper — today it is inline in the Bevy-coupled `sync_tools_to_canvases`; the wire encoder needs it too, spec §3.5). New bevy-free buffer extracted from `PaintSurface`'s pure half:

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

`paint.rs` re-exports `Tool` (`pub use dooduel_core::canvas::Tool;`) so `Msg::SelectTool(paint::Tool)` and view code stay untouched.
- [x] **W0.4 Re-point `paint.rs`.** `PaintSurface` delegates pixels/brush/undo to an inner `PaintBuffer`; keeps `enabled`, `to_pixel`, Bevy `Image` mirroring, observers. All existing paint tests must pass unchanged.
- [x] **W0.5 Automated purity tripwire** (rev-2: the wasm check does NOT guard bevy-freeness — bevy/buiy are wasm-safe). `apps/dooduel_core/tests/purity.rs`:

```rust
/// dooduel_core must stay Bevy-free (spec §2.1): the only bevy-family dep is
/// bevy_reflect, and no buiy crate may appear. Runs `cargo tree` (metadata only,
/// no build) so it rides the normal workspace nextest gate on all 3 OSes.
#[test]
fn dep_tree_is_bevy_free() {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["tree", "-p", "dooduel_core", "-e", "normal", "--locked"])
        .output().expect("cargo tree runs");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));
    let tree = String::from_utf8_lossy(&out.stdout);
    for line in tree.lines() {
        assert!(!line.contains("buiy"), "buiy crate leaked into dooduel_core: {line}");
        if line.contains("bevy") {
            assert!(line.contains("bevy_reflect") || line.contains("bevy_ptr") || line.contains("bevy_utils") || line.contains("bevy_platform"),
                "non-reflect bevy dep leaked into dooduel_core: {line}");
        }
    }
}
```

(`bevy_ptr`/`bevy_utils`/`bevy_platform` are `bevy_reflect`'s own transitive family — verify the exact allow-list against `cargo tree` output when implementing and pin what's printed.)
- [x] **W0.6 Gate + commit.** Full SG. Expected: total workspace test count unchanged ± the relocated pure game tests (now reported under `dooduel_core`; nothing lost — compare `nextest` totals before/after). Two commits: (1) lockfile-only member addition + `cargo deny check`; (2) the move + extraction.

### Wave 1 — protocol types + the `game::Game` API delta

**Files:** Create `apps/dooduel_core/src/protocol.rs`. Modify `apps/dooduel_core/src/{lib.rs,game.rs}`, `apps/dooduel/src/lib.rs`, `apps/dooduel/src/bin/{playtest_host.rs,capture.rs}` (all `start_match` call sites — rev-2: capture.rs calls it too).

- [x] **W1.1 Protocol types (red-first: round-trip tests).** `protocol.rs` per spec §3 (rev-2.1) — the exact wire surface:

```rust
pub const PROTOCOL_VERSION: u32 = 1;
// Limits (spec §3.1 rev-2.1) — also the DoS guard:
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_STROKE_POINTS: usize = 256;
pub const MAX_GUESS_LEN: usize = 128;
pub const MAX_NAME_LEN: usize = 32;
pub const ROOM_CODE_LEN: usize = 6;
pub const MAX_AVATAR_PNG: usize = 32 * 1024;   // RAW bytes; base64 on the wire (~43 KiB) —
                                               // a max-avatar Join frame fits MAX_FRAME_BYTES.

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum WireAvatar { Default, Preset { icon: usize, tint: usize },
                      Custom { png_base64: String } }   // rev-2: base64 String, NOT Vec<u8>
                                                        // (serde_json Vec<u8> = number array, ~3.6×)

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum CanvasOp {
    // rev-2: effective radius pre-applied (eraser = PAPER color + eraser_radius()); no tool
    // enum, no erase flag on the wire — color+radius fully determine the stamp (spec §2.2 rev-2.1).
    Stroke { id: u64, points: Vec<(i32, i32)>, color: [u8; 4], radius: i32 },
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
    Stroke { stroke_id: u64, points: Vec<(i32, i32)>, color: [u8; 4], radius: i32, done: bool },
    Fill { seed: (i32, i32), color: [u8; 4] },
    Undo, Clear, Continue, Leave,
}

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub enum ServerEvent {
    Welcome { seat: usize, room_code: String, reconnect_token: String, protocol_version: u32 },
    RoomState(RoomReplica),
    Roster { players: Vec<ReplicaPlayer> },     // carries `guessed` too (deliberate superset of spec §3.3's sketch)
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
impl RoomReplica {
    /// Per-letter slots derived from word_display ('_' ⇒ unrevealed) — the view's
    /// word-row data source (replaces Game::word_slots, which keyed off viewing_as).
    pub fn word_slots(&self) -> Vec<(char, bool)>;
}

#[derive(Serialize, Deserialize, Reflect, Clone, Debug, PartialEq)]
pub struct ReplicaPlayer { pub name: String, pub avatar: WireAvatar, pub connected: bool,
                           pub is_bot: bool, pub score: i64, pub guessed: bool }
```

`Phase`, `ChatMsg`, `ChatKind` (rev-2: `ChatMsg` contains it), `TurnResult` gain `Serialize/Deserialize` in `game.rs`.
Tests: serde_json round-trip for a populated value of EVERY variant; **the cap-consistency boundary test (rev-2, red-first):** a `Join` carrying a `MAX_AVATAR_PNG`-sized (raw) avatar serializes to `≤ MAX_FRAME_BYTES` — this is what forced the 32 KiB cap; `word_slots()` derivation cases (blanks, hint-revealed, full word).
- [x] **W1.2 Run round-trips red→green; commit** `feat(dooduel_core): wire protocol types (M1 spec §3)`.
- [x] **W1.3 Game API delta (red-first, one sub-step each; spec §2.3).** In `game.rs`:
  1. `pub struct PlayerSpec { pub name: String, pub is_bot: bool }`; new `start_match(roster: &[PlayerSpec], config: Config)` — old signature becomes `start_match_solo(human_name, config)` building `[human, Priya(bot), Theo(bot), Sam(bot)]` and delegating. Call sites (all three): `apps/dooduel/src/lib.rs`, `bin/playtest_host.rs`, `bin/capture.rs`.
  2. `Player` gains `is_bot: bool, occupied: bool`; `seeded_bot_plans`/due-guess drain key off `is_bot` (test: a 2-human roster never bot-guesses on a human seat).
  3. Vacant-seat semantics: `pub fn vacate_seat(&mut self, seat)`; rotation skips `!occupied`; `guesser_count()`/`all_guessed()` count occupied non-drawer seats; occupancy < 2 ⇒ `Final` (tests: rotation-skip, count, early-final).
  4. `pub fn force_end_turn(&mut self)` — public path to the existing turn-end (test: mid-Drawing force ends to Reveal with results).
  5. `pub fn knows(&self, seat) -> bool` (three-way, spec §5.1) + `pub fn word_display_for(&self, seat) -> String`; existing `word_display()` delegates to `word_display_for(self.viewing_as)` (removed in W3). The view-side `word_slots` replacement is `RoomReplica::word_slots()` (W1.1) — no core sibling needed.
- [x] **W1.4 Gate + commit** `feat(dooduel_core): Game roster/redaction API delta (M1 spec §2.3)`.

### Wave 2 — `Session` + `InProcessTransport` (the authority, headless)

**Files:** Create `apps/dooduel_core/src/{session.rs,transport.rs}`. Modify `lib.rs` (exports).

- [ ] **W2.1 Transport trait + InProcessTransport (red-first).** Per spec §2.4 exactly (non-blocking `try_recv`, addressed `send`, `disconnects`); `InProcessTransport` = paired `VecDeque`s (`new_pair(n_clients) -> (ServerEnd, Vec<ClientEnd>)`). Test: send/recv round-trip, per-recipient addressing isolation.
- [ ] **W2.2 Session core.** `session.rs`:

```rust
/// Injected policy — keeps dooduel_core dep-free and the in-process tests
/// deterministic (rev-2: entropy is INJECTED, never a core dep).
pub struct SessionOpts {
    /// ≥128-bit hex token per call (spec §6.3). Server: getrandom-backed.
    /// Solo/tests: a seeded deterministic generator.
    pub token_gen: Box<dyn FnMut() -> String + Send>,
    /// On StartMatch, pad occupied seats up to this count with PRESET_NAMES bots
    /// (rev-2: THE bot-entry mechanism — spec §8). Solo: 4. Networked M1 rooms: 0.
    pub fill_bots_to: usize,
}

pub struct Session {
    game: Game,
    canvas: PaintBuffer,               // authoritative raster (derived cache)
    canvas_ops: Vec<CanvasOp>,         // the per-turn op log — the sync primitive
    next_op_id: u64,
    seats: Vec<SeatState>,             // conn binding, token, disconnect deadline
    host: usize,
    outbox: Vec<(Recipient, ServerEvent)>,   // Recipient = Seat(usize) | All
    config: Config,
    opts: SessionOpts,
    started: bool,
}
impl Session {
    pub fn new(config: Config, opts: SessionOpts) -> Self;
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

Build order, each red-first: (1) connect/roster/host + `Welcome`/`RoomState`/`Roster` events; (2) `StartMatch` host-gate + **bot padding** (`fill_bots_to` — test: 1 connected client, `fill_bots_to: 4` ⇒ 4-seat roster, the 3 bots guess via `tick`, the human seat is never bot-guessed) + `PhaseChanged`/`WordChoices`; (3) `Pick` + `Guess` through `apply_guess` → `GuessResult`/`ChatLine`/`WordUpdate` upgrades; (4) canvas intents → op validation/append/`CanvasOpApplied`, `Undo` → remove-last + `CanvasUndo`, `Clear`, no-echo-to-originator (spec §3.5); (5) `tick`: countdown, hint flip re-sends, auto-pick, turn end → `TurnEnded`, match end → `MatchEnded`; (6) disconnect/grace/**token rotation via `opts.token_gen`**/`force_end_turn` on drawer-drop, host migration, `Leave`, vacate.
- [ ] **W2.3 The secrecy scan (spec §9.2 — the load-bearing test).** Scripted seeded 2-round match, 4 seats (words chosen from the pool with no substring collisions in scripted guesses); for every guesser seat and every turn: serialize every event addressed to that seat; assert the secret (case-insensitive) absent from the JSON **before** `min(that seat's correct guess, TurnEnded)`; assert the drawer's stream DOES carry it (proves the scan can see).
- [ ] **W2.4 Op-log determinism + late-join equivalence.** (a) two `PaintBuffer`s fed the same op log are pixel-identical; (b) replay-after-undo == incremental application with the undo applied; (c) a "late joiner" seeded by `CanvasLog` mid-turn ends pixel-identical to a from-start replica after subsequent ops + an undo reaching pre-join ops.
- [ ] **W2.5 In-process full match.** One `Session` + 4 scripted `InProcessTransport` clients play to podium (guesses, strokes, continues); assert podium scores against the same script run directly on `Game` (the authority adds no scoring drift).
- [ ] **W2.6 Gate + commit** (likely 3–4 commits along the build order).

### Wave 3 — the client-replica refactor + solo-over-Session (GUI runs on `RoomReplica`)

**Files:** Modify `apps/dooduel/src/{lib.rs,paint.rs}`, `apps/dooduel/src/view/*.rs`, `apps/dooduel_core/src/game.rs` (the deferred deletions), `apps/dooduel/src/bin/capture.rs`, `apps/dooduel/tests/canvas_e2e.rs` (rev-2: it drives `Msg::StartMatch`/`ChooseWord` and hangs without the solo authority). Create `apps/dooduel/src/net.rs`. Delete `apps/dooduel/src/bin/playtest_host.rs`.

**Note (spec §11 sequencing deviation, deliberate):** `playtest_host` drives the pre-replica model directly and cannot survive this wave; it is deleted **here**, and its successor (`dooduel_mcp`) lands in wave 5 — acceptable because the branch merges as one gated series (the capability gap never reaches `main`). Its pure per-seat-view rendering tests were superseded by W2.3's per-recipient replica tests.

- [ ] **W3.1 Model split (spec §4.1).** `Dooduel` drops `game: Game`; gains `replica: RoomReplica`, `net: NetState` (`enum NetState { Offline, Solo, Joining, Connected { .. }, Dropped { .. } }`), `chat_input: String` (moved OUT of `Game` — delete the core field + `viewing_as`/`switch_seat` and the `word_display()`/`word_slots()` shims from `game.rs` now). `Msg` delta: `+ Net(ServerEvent)`, `CreateRoom`/`SubmitJoin` become intent-sending, `- SwitchSeat`, gameplay arms (`ChooseWord`, `SubmitGuess`, `Continue`, `StartMatch`, tool strokes' commit path) **send intents instead of mutating** (spec §4.2); `Msg::Net(MatchEnded)` lifts `Screen::Podium` (not `Tick`).
- [ ] **W3.2 The net pump.** `net.rs`: `NetPlugin` — a `ClientNet` resource holding `Box<dyn ClientTransport>`; system in `MvuSet::Enqueue` draining `try_recv()` → `enqueue(Msg::Net(ev))`; outbound helper the reducer's `Cmd`s call. `LocalAuthorityPlugin` (solo): owns `Session` (constructed with `SessionOpts { token_gen: seeded-deterministic, fill_bots_to: 4 }`) + the in-process pair; a system pumps `session.tick(Time::elapsed())` + routes intents/events (the same loop shape `dooduel_server` runs — spec §8).
- [ ] **W3.3 View re-point.** Mechanical: `s.game.X` → `s.replica.X` / accessor equivalents across `view/*.rs` (the deleted `game` field makes every stale read a compile error — the compiler is the audit); word row from `replica.word_slots()` (W1.1); countdown from the monotonic re-anchored `remaining` (spec §4.3 — anchor on `Msg::Net` receipt, clamp no-upward-jump); pick overlay keyed on `!replica.word_choices.is_empty()`; the lobby becomes the **minimal live lobby** (live `Roster`, host-gated Start, server `room_code`); `SwitchSeat` affordances deleted (roster chips become inert badges; the "Switch to {drawer}" waiting-overlay button is removed).
- [ ] **W3.4 Tests.** (a) Existing `lib.rs` probe tests re-target: solo match flows through `LocalAuthorityPlugin` (start → pick → guess via intents → podium). (b) **`tests/canvas_e2e.rs` ported (rev-2):** install `LocalAuthorityPlugin` in its unified driver + pump frames so the match reaches Drawing through the solo path. (c) New: feed a scripted `ServerEvent` sequence into `Msg::Net`, assert the view via `BuiyProbePlugin` (guesser never shows the word pre-reveal; countdown displays; chat renders). (d) Replay: record a **solo** session, replay byte-identical (spec §3.4; the **networked** replay assertion lands in W5.3 where a real networked stream first exists — rev-2).
- [ ] **W3.5 RUN THE ARTIFACT.** `cargo run -p dooduel` — play a full solo match vs 3 bots to the podium; drawing, guessing, theming, avatar editor all live. Record findings in the commit body.
- [ ] **W3.6 Gate + commit series.**

### Wave 4 — `dooduel_server` + the WS transports + networked GUI

**Files:** Create `apps/dooduel_server/*` (structure above). Modify `apps/dooduel_core/{Cargo.toml,src/transport.rs}` (feature `ws-client` + `WsClientTransport` via ewebsock) **+ spec §2.1 amended in the same commit** (rev-2: the shared-impl refinement goes back into the spec diagram per the plan's own conflict rule). Modify `apps/dooduel/Cargo.toml` (+`ws-client`), `apps/dooduel/src/net.rs` (Create/Join wiring), root `Cargo.toml` + `Cargo.lock`, `.github/workflows/ci.yml`.

- [ ] **W4.1 Deps commit (lock + deny, own commit).** Add `ewebsock = "=0.8.0"`, `async-tungstenite = { version = "0.34", default-features = false, features = ["smol-runtime"] }`, `async-net`, `async-executor`, `async-io`, `futures-lite`, `getrandom` (rev-2: the server's entropy source for tokens + room codes — already a transitive lock node, now a direct dep of `dooduel_server` only) to `[workspace.dependencies]`; run `cargo deny check`; commit the lockfile delta alone.
- [ ] **W4.2 `WsClientTransport`** (`dooduel_core`, `feature = "ws-client"`): ewebsock connect; `try_recv` maps `WsMessage::Text` → `serde_json::from_str::<ServerEvent>` (decode error ⇒ log + skip, never panic); `send` serializes intents; `status()` from ewebsock events. Unit test behind the feature: intent→text→intent round-trip through the framing fns (no live socket at this tier). **Amend spec §2.1's crate diagram in this commit** (WsClientTransport → `dooduel_core` feature `ws-client`).
- [ ] **W4.3 Server skeleton.** `main.rs`: parse `--port` / `DOODUEL_ADDR` — **default 7878** (rev-2: matches the GUI's default `ws://127.0.0.1:7878`; the e2e passes `--port 0` explicitly); print `LISTENING port=<n>` to stdout (the e2e's discovery line, spec §9.5); accept loop on a static `async_executor::Executor` driven by `async_io::block_on`. `wire.rs`: per-conn read task (frame-size cap 64 KiB, intent rate cap ~30/s sustained → `Error{RateLimited}`/disconnect on abuse) forwarding decoded intents into the room channel; write task draining the conn's outbox. `registry.rs`: `Create` → **getrandom-backed** 6-char `[A-Z0-9]` code collision-checked; `Join` unknown ⇒ `Error{RoomNotFound}`; **per-IP connection/Join-attempt rate limit** (rev-2: the room-code brute-force guard, spec §6.2 — a small token-bucket per remote IP at the accept/Join layer); route to the room task; GC on all-empty + grace. `room.rs`: the actor — single owner of `Session` (constructed with `SessionOpts { token_gen: getrandom-backed, fill_bots_to: 0 }`); loop over (intent channel, 100 ms tick timer); after each intake/tick, `drain_events()` → per-recipient conn outboxes; per-turn transcript log line (acceptance evidence, spec §1.4).
- [ ] **W4.4 Server-side tests.** In-package: registry Create/Join/GC semantics with a fake conn channel (no socket); **the per-IP join rate limit** (burst allowed, sustained brute-force rejected — rev-2); room-actor determinism (scripted intents interleaved with ticks == same script against a bare `Session`); token re-attach over reconnect including rotation + old-conn replacement (spec §6.3).
- [ ] **W4.5 Networked GUI.** `net.rs`: `CreateRoom`/`SubmitJoin` open a `WsClientTransport` to `DOODUEL_SERVER_URL` (default `ws://127.0.0.1:7878`); `NetState` drives the Join/Lobby screens (spinner on `Joining`, error toasts on `Error` events, `Dropped` → rejoin-with-token attempt).
- [ ] **W4.6 CI delta.** Add to `ci.yml`: a `dooduel-web-check` step in the web-smoke job (`cargo check --target wasm32-unknown-unknown -p dooduel_web --locked` with the job's existing RUSTFLAGS) — the **wasm-safety** gate for the client graph (bevy-freeness is W0.5's tripwire — rev-2 correction: the wasm check cannot catch a bevy leak, bevy is wasm-safe). New packages ride the existing workspace jobs automatically; verify the MSRV job locally with `RUSTUP_TOOLCHAIN=1.95 cargo check --workspace --locked` before committing.
- [ ] **W4.7 RUN THE ARTIFACT.** Terminal 1: `cargo run -p dooduel_server` (binds 7878). Terminals 2+3: two `cargo run -p dooduel` instances — create a room in one, join by code in the other, play a 2-human match (draw on one screen, watch it appear on the other; guess; podium). Record findings.
- [ ] **W4.8 Gate + commit series.**

### Wave 5 — `dooduel_mcp` + the two-process e2e

**Files:** Create `apps/dooduel_mcp/*` (structure above). Modify `apps/dooduel_server/Cargo.toml` (dev-dep on `dooduel_mcp` lib) + `apps/dooduel_server/tests/e2e.rs`.

- [ ] **W5.1 `HeadlessClient` (lib, red-first).** Wraps `WsClientTransport` + a `RoomReplica`: `pump()` (drain events → replica), `state_report() -> String` (the honest per-seat markdown view — phase/word-display/scores/chat-tail/actions-now), `canvas_png() -> Vec<u8>` (rasterize `replica.canvas_ops` through `PaintBuffer` + `image` PNG encode), plus intent passthroughs (`pick/guess/stroke/fill/undo/clear/continue_turn/leave`). Tests: feed scripted events, assert the report redacts exactly as the replica does; canvas_png round-trips through `image::load_from_memory`.
- [ ] **W5.2 The MCP surface (`mcp.rs`).** Hand-rolled newline-delimited JSON-RPC 2.0 over stdio (spec §7): `initialize` (advertise ONLY `tools`), `notifications/initialized`, `tools/list` (the 11 tools with JSON-schema params), `tools/call` (dispatch → `HeadlessClient`), `ping`. stderr = logs. Unit tests drive it as `fn handle(line: &str) -> Option<String>` — no process spawn at this tier; malformed JSON-RPC ⇒ error response, never a panic.
- [ ] **W5.3 The two-process e2e** (`apps/dooduel_server/tests/e2e.rs`, spec §9.5): spawn `env!("CARGO_BIN_EXE_dooduel_server")` with `--port 0`, parse `LISTENING port=`, connect 4 `dooduel_mcp::HeadlessClient`s; scripted full match to podium with condition-based waits (poll `pump()` for expected events, deadline 60 s, no sleeps-as-sync, no retry reliance); assert podium + a reconnect mid-match (drop one client, rejoin with token, assert `RoomState`+`CanvasLog` resync) + **the networked replay assertion (rev-2, spec §9.6):** record one client's `Msg::Net` stream during the match and replay it byte-identical.
- [ ] **W5.4 RUN THE ARTIFACT.** Launch the server + one real `dooduel_mcp` process; drive it by hand over stdio (`initialize`, `tools/call join_room`, `get_state`) against a live GUI match. Record findings.
- [ ] **W5.5 Gate + commit series.**

### Wave 6 — acceptance + docs closeout

**Files:** Create `docs/reports/2026-07-05-dooduel-m1-acceptance.md` (+assets). Modify `docs/README.md`, `docs/specs/2026-07-04-dooduel-multiplayer-m1-design.md` (status), `docs/plans/follow-ups.md` (anything deferred en route).

- [ ] **W6.1 Acceptance harness.** A documented fish-safe launch recipe (bash script under `apps/dooduel_mcp/examples/` or a README section): start `dooduel_server`, N agent seats (each = one `dooduel_mcp` process an LLM agent drives), the room code handshake. The server's per-turn transcript is the evidence stream.
- [ ] **W6.2 THE ACCEPTANCE RUN (manual, with the user).** The user in the native GUI (and/or web) + 3 MCP-agent seats play a full match to the podium over the live server (spec §1.4). Evidence: server transcript, per-seat reports, screenshots.
- [ ] **W6.3 Close the books.** Acceptance report in `docs/reports/`; spec status `draft` → `active`; docs/README index rows; follow-ups ledger for anything discovered-but-deferred; final full SG + summary for the user's push/PR/merge decision.

---

## Risk register

| Risk | Wave | Mitigation / rollback |
|---|---|---|
| The `game.rs` move breaks hidden `bevy::prelude` leaks | 0 | The move is `git mv` + one import line; SG catches everything; rollback = revert one commit |
| `cargo tree` in the purity test flakes under parallel nextest (lock contention) | 0 | `cargo tree` is metadata-only (no build lock); if it proves flaky on an OS, scope it `cfg(target_os = "linux")` with a stated reason |
| View re-point (W3.3) is wide and mechanical-but-error-prone | 3 | The deleted `game` field makes every stale read a compile error; probe tests per screen; the solo RUN gate is the real check |
| ewebsock 0.8.0 staleness bites on wasm (web-sys drift) | 4 | The W4.6 wasm check catches at CI-time; fallback recorded in spec §10 (tokio stack) is a wave-4-local swap |
| smol-executor lifetime/pinning friction in the server loop | 4 | The room actor is transport-agnostic — worst case the accept loop alone moves to the tokio fallback without touching `Session` |
| e2e flake under CI parallelism | 5 | port-0 + stdout discovery + condition waits + generous deadlines; if an OS proves hostile, platform-gate with a stated reason (spec §9.5) rather than retries |

## Self-review + gate history

- Rev-1 self-review: spec coverage / placeholder scan / type consistency — passed with the three fixes folded in.
- **Rev-2 (2026-07-04): the 3-reviewer plan gate** (spec-alignment, buildability, test-adequacy; 32 agents, 3/3 coverage, zero holes): 23 confirmed findings applied — entropy injection (`SessionOpts.token_gen` + server `getrandom`), the `fill_bots_to` bot-entry mechanism, base64 avatars + the 32 KiB cap (spec §3.1 amended), `CanvasOp` shape (spec §2.2 amended), the real game-test location (lib.rs::tests, extracted in W0.2), `stamp_circle`/`eraser_radius` naming, the automated purity tripwire, `canvas_e2e.rs` + `capture.rs` + `dooduel_core/src/game.rs` in the file lists, per-IP join rate limiting, port-default alignment, the RUST_MIN_STACK correction, `ChatKind` serde, `--locked` flags, networked-replay landing in W5.3, and the spec §2.1 amendment scheduled in W4.2. 6 findings refuted (recorded in the gate ledger; e.g. the secrecy-scan cross-turn false-positive and the solo-clock/replay-suppression claims rested on misread evidence).
