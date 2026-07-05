# Dooduel production multiplayer — M1: the authoritative networked session spine

**Date:** 2026-07-04
**Status:** `draft` — **Revision 2.1.** Rev-1 was brainstormed + section-approved (2026-07-04). Rev-2 applies the 5-agent research + 3-reviewer gate (all three reviewers CHANGES-REQUIRED; 33 findings confirmed after adversarial verification — 2 blockers, 24 majors — and 5 refuted; see §13). Rev-2.1 (the plan gate, same day) fixes two spec-internal inconsistencies the plan reviewers caught: the §3.1 avatar-cap arithmetic (64 KiB base64 could not fit the 64 KiB frame cap → 32 KiB raw) and the §2.2 op shape (effective color+radius, no tool enum on the wire). The plan (`../plans/2026-07-04-dooduel-multiplayer-m1.md`, rev-2) additionally pins: entropy is **injected** (`SessionOpts.token_gen`; `getrandom` server-side only), bots enter via `SessionOpts.fill_bots_to` padding at `StartMatch` (§8's mechanism), and `WsClientTransport` lives in `dooduel_core` behind feature `ws-client` (the §2.1 diagram amendment lands with plan W4.2). NO implementation code.
**Kind:** `design` (milestone M1 of the production-multiplayer campaign; §1.2 for the M1–M6 decomposition).
**Base:** `origin/main` @ `6e07954` (the merged Dooduel FINAL — `apps/dooduel` at design parity).
**Worktree:** `dooduel-app2` (branch `feat/dooduel-multiplayer-m1`).
**Supersedes:** nothing. It *reframes* the shipped app: the [Dooduel FINAL](2026-07-03-dooduel-final-design.md) built the game to exact parity with a design bundle that is itself a **single-player simulation** of a multiplayer game. This design takes the features that bundle *mocks* — rooms, joining, real opponents, invite links — into production with real implementations. The FINAL's pure `game::Game` core is reused for its **rules/scoring/clock machinery** (with a bounded, named API delta — §2.3); what changes is *who runs it* and *how clients reach it*.

**Seeds (required reading):**
- The shipped app + its pure core — `apps/dooduel/src/game.rs` (the deterministic rules; depends only on `Duration` + `Reflect`), `apps/dooduel/src/paint.rs` (the integer-deterministic stroke/fill ops the op-log model stands on), `apps/dooduel/src/lib.rs` (the MVU shell), `apps/dooduel/src/bin/playtest_host.rs` (the file-protocol multi-agent host this design supersedes).
- The design bundle + its own honesty about what it mocks — `../reference-designs/dooduel/REQUIREMENTS-DELTA.md` §6 ("Still the solo demo with a dev-only 'playing as' seat-switcher… No bots, no real networking (all deferred)") + §11 (the deferred "full game" list).
- The acceptance playtest that ran on the file protocol — `../reports/2026-07-04-dooduel-acceptance-playtest.md` (the bar this milestone re-hits, now genuinely networked).

---

## §1 Intent

### 1.1 The reframe

The Dooduel design bundle is a **single-player simulation** of a multiplayer game: it runs entirely in one browser tab, the "other players" are seeded bots, the room code is a cosmetic string, "join a room" connects to nothing, and the seat-switcher hot-seats one human across every seat. "Match the design target exactly" faithfully reproduced those *mocks as mocks*. This milestone begins converting the mocks into real implementations, starting with the load-bearing one: **an authoritative networked session that humans (native + web) and LLM agents connect to and play together.**

The pure `game::Game` state machine is the crown jewel — deterministic, side-effect-free, unit-testable without an ECS/GPU/clock. Its **rules, scoring, hint, and clock machinery are reused as-is**; its roster/redaction surface needs a bounded extension (§2.3 — the gate killed the rev-1 "reused verbatim" claim as false). What this milestone changes is architectural: the game stops running *in each client* and starts running in one authoritative `Session`; clients become replicas.

### 1.2 Campaign decomposition (each milestone = its own spec → plan → build)

- **M1 (this spec): the authoritative networked session spine + private-room slice.** Wire protocol, dedicated server, the client refactored from authoritative to replica, in-process + WebSocket transports, invite-code rooms with a **minimal live lobby** (live roster + host-gated start — required by the acceptance bar), native + web + MCP clients, one full match to podium.
- **M2: room configuration + host settings** — the real `Config` surface (rounds / draw-time / hints / max players), the lobby's settings panel + copy-invite-link polish.
- **M3: public matchmaking + hosting hardening** — a quick-match queue behind "▶ Play"; **TLS (`wss://`)** enters here, deliberately (§10: the rustls path has a known cargo-deny landmine — `ring`/`aws-lc-sys` licensing — deferred as its own change).
- **M4: content depth** — word modes (Normal / Hidden / Combination), custom word lists, languages, the full 20+ palette; wire-encoding optimization if measured necessary (§3.1 runner-up).
- **M5: social / moderation** — like/dislike, votekick, live presence status, spoiler-safe post-guess chat, guess-censoring (the §5.3 accepted leak).
- **M6 (eventual, deliberately not designed out): P2P transport** — WebRTC data channels + a peer-hosted `Session` + signaling. M1's transport/authority split is what makes this additive, not a rewrite.

### 1.3 Decisions locked at the brainstorm gate (2026-07-04)

1. **Dedicated authoritative server for v1**, but the transport/authority boundary must keep P2P open ("dedicated server to start, for reliability, but we still want p2p support eventually so don't design that out").
2. **V1 = the private-room vertical slice** with humans (native + web) **and** MCP agents in the *same* milestone — one authoritative match to the podium.
3. **Agents play as headless protocol clients** exposing game-semantic MCP tools — never by driving a GUI through the a11y tree.
4. **Offline solo (in-process local authority + bots) is retained as a demo / verification / test path.**
5. **Server authority = pure-core in a lightweight async server** (no Bevy on the server). The client becomes a replica.
6. **Identity is ephemeral** — name + avatar + reconnect token, no accounts (likely permanently).

### 1.4 Acceptance bar

A real match — the user in the native or web GUI + N MCP agents, each a distinct seat — played end-to-end to the podium over the live `dooduel_server` on a real WebSocket. Operationalized (gate finding `acceptance-run-not-operationalized`): a scripted harness launches `dooduel_server` + N `dooduel_mcp` processes pointed at the room (code + names); the server writes a per-turn transcript log retained as evidence; the run is written up as a `docs/reports/` acceptance report mirroring `2026-07-04-dooduel-acceptance-playtest.md`. This tier is a **manual gate outside CI** (it needs a display and a human).

---

## §2 Architecture — the transport/authority split

The organizing idea: **the authority is transport-agnostic.** A `Session` owns the game and does all rule enforcement and redaction; it performs no I/O. Transports move bytes. A dedicated server and an in-process solo run drive the *same* `Session` behind *different* transports; a peer-hosted P2P session (M6) is a third transport behind the same boundary.

### 2.1 Crate structure (rev-2: the pure core is its own package)

The rev-1 plan ("session/protocol live in the `dooduel` lib") was a **blocker**: the `dooduel` lib is the full Bevy/Buiy GUI app, so a server depending on it links `bevy_render` + `bevy_winit` + x11/wayland + the whole Buiy stack — contradicting locked decision 5. Rev-2 extracts the pure core:

```
apps/dooduel_core   (NEW package `dooduel_core` — pure, Bevy-free, wasm-safe)
│  game        game::Game moved here (rules/scoring/clock; Reflect via bevy_reflect)
│  protocol    ClientIntent / ServerEvent (serde + bevy_reflect derives, §3)
│  session     Session — AUTHORITY: Game + roster + canvas op-log + per-recipient
│              redaction. No I/O. Consumes intents, emits addressed events.
│  transport   the trait (non-blocking try_recv + addressed send, §2.4)
│              + InProcessTransport (channels — solo + tests)
│  canvas      PaintBuffer — the Bevy-free pixel surface + op types extracted from
│              paint.rs (stroke_segment/flood_fill are already pure integer fns)
│  deps: bevy_reflect, serde, serde_json  (NO bevy, NO buiy)

apps/dooduel        (existing package — the GUI lib + windowed bin; wasm-safe)
│  depends on dooduel_core; keeps view/, theme, avatar, storage, confetti;
│  paint.rs's PaintSurface becomes a thin Bevy wrapper (Image mirror + pointer
│  observers) around dooduel_core::canvas::PaintBuffer
│  + net client wiring: WsClientTransport (ewebsock) + NetPlugin (§4.2)

apps/dooduel_server (NEW package, native-only bin)
│  room registry + WS accept loop (async-tungstenite/smol) + per-room actor task
│  depends on dooduel_core only (+ async-net/async-executor/futures-lite/async-tungstenite)

apps/dooduel_mcp    (NEW package, native-only bin)
│  headless client: WsClientTransport + a hand-rolled stdio JSON-RPC MCP server
│  depends on dooduel_core (+ ewebsock, image for get_canvas PNG)

apps/dooduel/dooduel_web (existing wasm crate) — gains the same NetPlugin path
```

Explicitly (gate finding `bins-vs-packages-ambiguous`): `dooduel_server` and `dooduel_mcp` are **separate workspace packages** added to `[workspace] members` — a `[[bin]]` of the `dooduel` package would inherit the full GUI dep tree (bins cannot have their own deps). Both follow the repo's `doc = false` bin convention. They join the headless `cargo test --workspace` gate; the wasm build graph (`dooduel_web`) must never acquire them.

`game.rs`'s `use bevy::prelude::Reflect` migrates to `bevy_reflect::Reflect` — `bevy_reflect` is the **one** Bevy-family dep `dooduel_core` keeps (needed so `Msg::Net(ServerEvent)` stays Reflect-able for MVU record/replay, §3.4).

### 2.2 The canvas authority (rev-2: op-log, not pixels)

`Session` owns the canvas as a **per-turn operation log** — the sync primitive is the op history, not a raster (prior art: skribbl.io's `drawCommands`; gate findings `canvas-authority-extraction-gap`, `undo-semantics-and-late-join-desync`). The raster is a local rendering cache each replica derives.

- `CanvasOp = Stroke { id, points, color, radius } | Fill { id, seed, color }` — integer canvas coordinates throughout (`stroke_segment`/`flood_fill` are already deterministic integer ops — `paint.rs:151-233` — so identical op sequences produce identical pixels on every replica). *Rev-2.1: the op carries the **effective** color+radius only (eraser = PAPER color + the ×1.6 radius pre-applied per §3.5) — no tool enum or erase flag on the wire; color+radius fully determine the stamp.*
- The turn's log is bounded (one drawer, one draw window) and cleared at turn start.
- **Undo = remove the last op from the log** (server-side bookkeeping), broadcast as `CanvasUndo { removed_id }`; every replica re-rasterizes the remaining log (a bounded, cheap replay). This composes with late join by construction and retires the 12-deep pixel-snapshot ring as a *sync* mechanism (it stays client-side only as the drawer's local rendering shortcut if convenient). Clear = an op-log truncation event.
- **Late join / reconnect canvas sync = the full current-turn op log** (`CanvasLog { ops }`), replayed client-side. **No PNG travels on the wire** — rev-1's `CanvasSnapshot`-as-PNG is deleted (it forced a server-side rasterizer + PNG encoder and provably desynced undo for late joiners).
- The server still *rasterizes nothing* in the hot path; `dooduel_mcp::get_canvas` rasterizes the op log locally via `dooduel_core::canvas` + `image` PNG encode.

### 2.3 The `game::Game` API delta (rev-2 — replaces the false "verbatim" claim)

The rules/scoring/hints/clock fold is reused as-is. The roster/redaction surface is extended — a planner implements exactly this list, nothing more:

1. **Roster-parameterized start:** `start_match(roster: Vec<PlayerSpec { name, is_bot }>, config)` replaces the hardcoded "seat 0 human + `PRESET_NAMES` bots".
2. **Per-seat `is_bot`:** the bot machinery (`seeded_bot_plans` / due-guess drain) keys off `is_bot`, replacing the hot-seat `viewing_as` filter (which no longer exists server-side; gate finding `bot-machinery-guards-human-via-viewing-as`).
3. **Vacant-seat semantics** (gate finding `mid-match-seat-departure-unhandled`): a seat freed past grace is marked **vacant** (not removed — seat indices are identity for rotation, `turn_guesses`, private chat `to`). Rotation skips vacant seats; `guesser_count`/`all_guessed` count only occupied non-drawer seats; the match ends (→ `Final`) when occupancy drops below 2.
4. **Public `force_end_turn`** — the drawer-drop-past-grace path (today's turn-end is a private fn gated on `Phase::Drawing`).
5. **Per-seat redaction accessor:** `word_display_for(seat)` and the `knows(seat)` predicate (§5.1) move into `Game` — the single redaction home `Session` calls per recipient (today's `word_display()` keys off the hot-seat `viewing_as`).
6. **Client-state fields leave the core:** `viewing_as` / `switch_seat` / `chat_input` are hot-seat/UI state, not rules state — they move to the client model (§4.1). The solo demo path drives its fixed seat through the same intent surface instead.

### 2.4 The transport trait (rev-2: non-blocking, addressed)

```rust
// dooduel_core::transport — sketch, not final signatures
trait ServerTransport {           // the Session-side face
    fn try_recv(&mut self) -> Option<(ConnId, ClientIntent)>;   // non-blocking
    fn send(&mut self, to: ConnId, ev: &ServerEvent);           // addressed
    fn disconnects(&mut self) -> Vec<ConnId>;
}
trait ClientTransport {           // the client-side face
    fn send(&mut self, intent: &ClientIntent);
    fn try_recv(&mut self) -> Option<ServerEvent>;              // non-blocking
    fn status(&self) -> ConnStatus;                             // for reconnect UX
}
```

Non-blocking `try_recv` is the honest shape (gate finding `client-net-pump-unspecified`): ewebsock is a poll-style receiver, and the MVU funnel's only async primitive (`Cmd::task`) is a **one-shot** future→Msg — a streaming subscription primitive is an explicitly deferred MVU roadmap item. The client integration is therefore a **Bevy system, not `Cmd::task`** (§4.2).

---

## §3 The wire protocol

### 3.1 Encoding + framing (rev-2: pinned)

- **`serde_json`, one protocol message per WebSocket TEXT frame.** Rationale: zero new dependency (workspace dep already), human-debuggable frames, and the §9.1 secrecy scan becomes trivially string-based. With the op-log canvas model there is **no bulk binary payload left** in the protocol (the one image — a custom avatar — is a one-shot, capped, base64 field). Runner-up, named: `postcard` over binary frames (serde-native, varint-compact, allowlist-clean — but a new dep, positional/fragile without golden tests, and its decisive advantage evaporated with `CanvasSnapshot`'s deletion). Revisit as a *measured* M4 optimization. **Rejected:** `bincode` 1.x — unmaintained (RUSTSEC-2025-0141); the existing deny.toml ignore is justified as dev-only, and putting it on the wire would invalidate that triage.
- **Version handshake:** `protocol_version: u32` travels in `Join`/`Create` (the first client frame). Mismatch ⇒ `Error { code: VersionMismatch }` + close, before any `Welcome`.
- **Limits (also the DoS guard):** max frame 64 KiB; `Stroke` ≤ 256 points/batch; guess ≤ 128 chars; name ≤ 32; room code = 6 chars `[A-Z0-9]`; avatar PNG ≤ **32 KiB raw, base64-encoded on the wire** (~43 KiB encoded, so a max-avatar `Join` frame fits the 64 KiB frame cap — *rev-2.1: the rev-2 "64 KiB (base64)" limit was internally inconsistent with the frame cap; caught at the plan gate*); per-connection intent rate cap (server constant, ~30 intents/s sustained) — violations get `Error` and, on repeat, disconnect.

### 3.2 Client → server — `ClientIntent`

| Intent | Phase gate (server-enforced) | Notes |
|---|---|---|
| `Create { name, avatar, protocol_version }` | — | Server **generates** the room code, returns it in `Welcome`. Creator = host. |
| `Join { room, name, avatar, protocol_version, reconnect: Option<Token> }` | — | Unknown code ⇒ `Error { RoomNotFound }` — **never creates** (rev-2; a typo must not spawn an empty room). With a valid token: re-attach (§6.3). |
| `StartMatch` | Lobby, host only | |
| `Pick { index }` | Picking, drawer only | |
| `Guess { text }` | Drawing, non-drawer, not-yet-guessed | Via `Game::apply_guess`. |
| `Stroke { stroke_id, points, color, size, tool, done }` | Drawing, drawer only | See §3.5 framing. |
| `Fill { seed, color }` | Drawing, drawer only | |
| `Undo` / `Clear` | Drawing, drawer only | |
| `Continue` | Reveal, any seat | Any-player advance is the design's semantics; accepted for M1's private rooms (documented; M5 revisits with moderation). |
| `Leave` | any | Graceful seat release (skips grace). |

Client-side `gen_room_code` is retired for networked play (deterministic name-hash codes collide two same-named hosts into one room).

### 3.3 Server → client — `ServerEvent` (addressed per recipient)

`Welcome { seat, room_code, reconnect_token, protocol_version }` · `RoomState { … }` (the full per-recipient replica seed, §4.1 — sent on join/reconnect) · `Roster { players: Vec<{ name, avatar, connected, is_bot, score }> }` · `PhaseChanged { phase, drawer, round, total_rounds, remaining }` · `CountdownSync { remaining }` (periodic, §4.3) · `WordUpdate { display, len, hints_revealed }` (**per-recipient**, §5) · `WordChoices { words }` (**drawer only**, Picking) · `CanvasOpApplied { op }` / `CanvasUndo { removed_id }` / `CanvasCleared` / `CanvasLog { ops }` (late join/reconnect) · `ChatLine { line }` (shared broadcast; private near-miss nudges addressed only to their seat) · `GuessResult { seat, correct, points }` · `TurnEnded { results, word }` (the reveal — word legitimately broadcast here) · `MatchEnded { podium }` · `Error { code, message }`.

**Event → replica mapping** (rev-2; gate finding `replica-update-mapping-undefined`) — each event names exactly the replica fields it sets:

| Event | Replica fields set |
|---|---|
| `RoomState` | everything below, atomically (the seed) |
| `Roster` | `players` (name/avatar/connected/bot/score) |
| `PhaseChanged` | `phase`, `drawer`, `round`, `total_rounds`, countdown re-anchor |
| `CountdownSync` | countdown re-anchor only |
| `WordUpdate` | `word_display`, `word_len`, `hints_revealed` (re-sent on hint flip; upgraded to full word for a seat that guesses correctly) |
| `WordChoices` | `word_choices` (drawer replica only) |
| `CanvasOpApplied`/`CanvasUndo`/`CanvasCleared`/`CanvasLog` | the canvas op log (+ re-rasterize) |
| `ChatLine` | append to `chat` |
| `GuessResult` | `guessed` flags, `scores` |
| `TurnEnded` | `turn_results`, `word_display` (full), phase → Reveal |
| `MatchEnded` | `podium`, phase → Final (**the Screen::Podium lift rides this event**, not `Msg::Tick` — rev-2, §4.1) |

### 3.4 Derives

Every protocol type carries `serde::{Serialize, Deserialize}` **and** `bevy_reflect::Reflect` + `Clone + Debug + PartialEq` — `Msg::Net(ServerEvent)` folds through the MVU funnel, so networked sessions record/replay like any other Msg stream (`bevy_reflect` is the one Bevy dep in `dooduel_core`).

### 3.5 Stroke framing + echo policy (rev-2)

- Points are **post-`to_pixel` integer canvas coordinates**, the *exact, complete* sample sequence the drawer stamped locally — coalescing is transport batching only, **never decimation** (the optimistic-paint ≡ authoritative-replay identity depends on it).
- A stroke spans batches under one client-chosen `stroke_id`, with `done: true` on the final batch — the id gives server and every replica identical stroke boundaries for interpolation anchoring (`last`) and for undo units.
- Color travels as the exact RGBA; the eraser's ×1.6 radius rule (`paint.rs`) is applied *before* the wire (the op carries effective radius) so replicas need no tool-specific knowledge.
- **Echo policy: the server does not echo canvas events to their originator.** The drawer applies its ops optimistically; since the drawer is the *only* op producer in a turn, its local op order provably equals the server's — no reconciliation. (`CanvasUndo`/`CanvasCleared` confirmations DO go to all seats including the drawer, keyed by op id — idempotent to apply on top of the optimistic removal.)

---

## §4 The client-replica refactor

### 4.1 The replica type (rev-2 — the replica is NOT `game::Game`)

Rev-1's "`Msg::Net` applies `ServerEvent` → `Game` replica" was a **blocker**: a guesser's redacted event stream *cannot* populate a `game::Game` (no secret, no rng, no bot plans — §5's own guarantee), and a `Game` on the client would hold every field the architecture exists to hide. The client model instead holds a **`RoomReplica`**:

```rust
// dooduel_core — sketch
struct RoomReplica {
    room_code: String, my_seat: usize, host: usize,
    players: Vec<ReplicaPlayer>,          // name, avatar, connected, is_bot, score, guessed
    phase: Phase, drawer: Option<usize>, round: u32, total_rounds: u32,
    countdown: CountdownAnchor,           // §4.3
    word_display: String, word_len: usize, hints_revealed: usize,
    word_choices: Vec<String>,            // populated only when this seat draws
    chat: Vec<ChatMsg>,                   // per-recipient filtered by the server
    canvas_ops: Vec<CanvasOp>,            // §2.2 — the raster is derived
    turn_results: Vec<TurnResult>, podium: Option<Podium>,
}
```

**Negative invariant (the §5 property, stated structurally):** `secret_word`, pre-pick `word_choices` (non-drawer), the RNG seed, `used_words`, the hint schedule (`hint_positions`/reveal times), bot plans, and other seats' private chat **have no field to land in** — they never appear in any wire type.

The `view/` modules re-point their reads from `game::Game` to `RoomReplica` (mostly mechanical — the accessors were designed for per-seat honesty already). Local-only UI state (tool/color/size, avatar editor, theme, viewport, `chat_input` — which **moves out of `Game`**, rev-2 §2.3.6) stays beside the replica in the model, never on the wire.

### 4.2 The net pump — a Bevy system, not `Cmd::task` (rev-2)

A `NetPlugin` adds a system in `MvuSet::Enqueue` that drains `ClientTransport::try_recv()` each frame and enqueues each event as `Msg::Net(ServerEvent)`; the reducer folds them into the `RoomReplica`. Outbound, gameplay messages (`Pick`/`SubmitGuess`/`Continue`/`StartMatch`/stroke commits) stop mutating game state locally and instead write to `ClientTransport::send` (a thin `Cmd`-like effect at the funnel edge). `Cmd::task` is explicitly **not** the seam — it is one-shot, and the MVU streaming-subscription primitive is a deferred roadmap item this design does not take on.

### 4.3 Clock: duration-remaining + monotonic interpolation (rev-2)

Every `PhaseChanged`/`CountdownSync` carries **`remaining: Duration`, computed by the server at send time**. The client anchors it to its own **monotonic** receipt instant (`Instant::now()`, never wall-clock — wall time jumps on NTP steps/suspend) and counts down locally; error ≈ one-way latency, in the safe direction. Periodic re-syncs re-anchor, **clamped so the displayed countdown never jumps upward**. No absolute timestamp of either machine's clock ever travels. The client never self-advances a phase — phase changes arrive only as events.

### 4.4 Drawing UX

The drawer paints optimistically through the same `PaintBuffer` ops it puts on the wire (§3.5); guessers rasterize the op log incrementally as events arrive. Undo/redo UI affordances stay drawer-only (server enforces regardless).

---

## §5 Anti-cheat / word secrecy — the load-bearing property

### 5.1 The redaction predicate (rev-2: three-way, not two-way)

`knows(seat) = (seat == drawer) ∨ (phase == Reveal/Final) ∨ guessed_correctly(seat)` — a correct guesser's `WordUpdate` upgrades to the full word **mid-turn** (matching today's `word_display` semantics), and `TurnEnded` legitimately broadcasts the word to everyone. `Game::word_display_for(seat)` (§2.3.5) is the single implementation home; `Session` calls it per recipient.

### 5.2 Enforcement

- Per-recipient event rendering (§3.3) + the `RoomReplica` negative invariant (§4.1): the secret has no client-side representation before `knows(seat)`.
- All matching/scoring runs server-side (`Game::apply_guess`); seat/phase gates per the §3.2 table; violations get `Error`, never partial application.

### 5.3 Accepted M1 leak (documented)

A *wrong* guess **containing** the secret as a substring is broadcast literally in chat (today's behavior; skribbl censors these). Accepted for M1 private rooms; M5 adds censoring.

---

## §6 The server — `dooduel_server`

### 6.1 Actor-model rooms

A room registry `RoomCode → Room`; each `Room` is one async task that solely owns its `Session` (no mutex on game state; intake order = mutation order = deterministic). Per-room tick ~10 Hz drives `session.tick(now)` (countdowns/hints/auto-pick/turn-end — and the **bot-plan drain**: `Session::tick` applies due bot guesses via `apply_guess`, replacing the client-side `Msg::Tick` drain); guesses/strokes process event-driven on arrival. After each intake/tick the room flushes addressed events. Runtime: `async-executor`/`async-io` (`smol` family) + `async-net` listener + `async-tungstenite` — **tokio-free** (§10).

### 6.2 Room lifecycle + host migration (rev-2)

- `Create` generates a unique 6-char `[A-Z0-9]` code (~31 bits; CSPRNG, collision-checked against live rooms). `Join` on an unknown code errors — never creates.
- **Host = the earliest-joined seat that is connected or within grace**, recomputed when a seat frees; a grace-window reconnect retains host. (Rev-2; rev-1 left the room unstartable if the host dropped in the lobby.)
- Room GC keys on **all seats empty** (left or grace-expired) — never on host departure alone.
- Join rate limit per connection/IP (also guards room-code brute-force — the room code is the only room-privacy boundary, stated plainly).
- **No TLS in M1** — `ws://` for private/LAN/localhost play. `wss://` lands in M3 with its deliberate deny.toml delta (§10).

### 6.3 Reconnection (rev-2: token semantics pinned)

- Token: **≥128-bit CSPRNG**, per `(room, seat)`, issued in `Welcome`, **rotated on every (re)connection** (single-use — closes the sniffed-token replay hole; prior art: Colyseus), TTL = the grace window + margin, invalidated when the seat frees or the room GCs, never logged.
- Disconnect ⇒ seat held "away" **45 s** (server constant); roster shows it. Valid-token `Join` re-attaches and reseeds via `RoomState` + `CanvasLog`. A token `Join` while the original connection is still live **replaces it** (the old connection is closed) — supports the hung-tab rejoin, and the rotation means a thief racing the owner burns the token observably.
- Drawer drop past grace ⇒ `force_end_turn` (§2.3.4). Mid-`Picking` drop ⇒ the existing auto-pick timeout already advances the turn (verified — no new path needed).

---

## §7 The MCP client — `dooduel_mcp`

A headless protocol client + a **hand-rolled stdio JSON-RPC MCP server** (rev-2: decided — the official `rmcp` SDK hard-requires tokio + tokio-util, defeating the one-async-ecosystem goal; the tools-only stdio surface is genuinely tiny: `initialize` advertising only the `tools` capability, `notifications/initialized`, `tools/list`, `tools/call`, `ping` — newline-delimited JSON-RPC 2.0 on stdin/stdout, stderr free for logging, ~200–400 lines on the existing `serde_json`).

Tools (1:1 onto the protocol): `join_room(room, name)` · `get_state()` (the per-seat honest view — phase, word display, scores, chat tail, actionable-now flags; the `playtest_host` seat-view content, fed from the replica) · `list_choices()` / `pick_word(index)` · `guess(text)` · `draw_stroke(points, color, size)` / `fill(seed, color)` / `undo()` / `clear()` · `continue_turn()` · `get_canvas()` (PNG rasterized locally from the op log via `dooduel_core::canvas` + `image`).

One agent per process; N processes for N seats — exactly how the acceptance playtest ran. The agent is indistinguishable, at the `Session`, from a GUI client.

---

## §8 Offline / in-process local authority (the solo + verification path)

"Solo" = the same `Session` in-process behind `InProcessTransport`, bots filling empty seats via the `is_bot` roster flag (§2.3.2 — the `viewing_as` bot-guard is gone; rev-2). **The authoritative tick driver is a client-side system** (no server process exists): the same ~logical loop `dooduel_server` runs, embedded — pump intents, `session.tick(now)`, drain events into the local transport. The GUI selects at match start: "▶ Play" (solo) ⇒ in-process `Session` + bots; Create/Join ⇒ `WsClientTransport`. The GUI plays through the **same intent/replica path in both modes** — the solo path is the harness that keeps the split honest.

---

## §9 Testing strategy

1. **Pure `Session` tests** (no I/O): scripted intents → authoritative state; phase/seat gate matrix (§3.2); vacant-seat rotation; host migration; token lifecycle.
2. **The secrecy scan** (rev-2, respecified — the rev-1 "entire stream never contains the word" was wrong at Reveal): over a **seeded multi-turn scripted match** with words/guesses chosen to avoid substring collisions, for **every guesser seat and every turn**: decode each server-originated event (it's JSON — string-scan the decoded fields) and assert the secret never appears in that seat's stream **before `min(their correct guess, TurnEnded)`**; drawer stream may contain it; chat echoes of client-typed text are exempt by construction of the script.
3. **Op-log determinism:** identical op logs rasterize to identical pixels (the integer-op fact, §2.2); undo/late-join replay equivalence (join mid-turn ⇒ replay(log) == incremental application).
4. **In-process integration:** one `Session` + N in-process clients play a full match to podium (also the solo path's harness).
5. **Two-process e2e** (rev-2, operationalized): the test lives **inside the `dooduel_server` package** (`env!("CARGO_BIN_EXE_dooduel_server")`); the MCP-client logic is a **lib target** of `dooduel_mcp` that the server package dev-depends on (cargo does not build sibling packages' bins for tests). Server binds **port 0** and prints the bound port as a machine-readable stdout line; all waits are condition-based (poll for `Welcome`/`PhaseChanged`) with deadlines well under nextest's ceilings; **no reliance on retries** (headless profile is retries=0 by policy); must pass 3-OS + the MT lane.
6. **Client replica:** feed `ServerEvent`s into `Msg::Net`, assert the view via the probe (`BuiyProbePlugin`); replay a recorded networked session byte-identically (§3.4).
7. **Existing GPU/visual gates unchanged** — rendering is untouched.
8. **Acceptance:** the §1.4 manual networked match, evidenced per §1.4.

---

## §10 Dependencies + CI delta (rev-2: verified against reality)

All verified against crates.io/advisory-db/deny.toml on 2026-07-04 (gate research R1):

- **`ewebsock` = 0.8.0, pinned** — client transport, native (`tungstenite` on a background thread, no reactor) + wasm (`web-sys` WebSocket) behind one API; MIT/Apache-2; MSRV 1.76 ≤ 1.95. *Caveats recorded:* last release 2024-11 (repo alive, unreleased); pins tungstenite 0.24 ⇒ the lock will carry **two tungstenite versions** (0.24 client / 0.29 server — `multiple-versions = "allow"` permits; self-resolves on their next release); **do not** take git main (`unknown-git = "deny"`).
- **`async-tungstenite` 0.34 with `smol-runtime`** — server WS, runtime-agnostic core on futures-io; MIT; MSRV 1.85. `async-net` is the one genuinely new lock node (async-io/futures-lite/async-executor are already in the lock via bevy/zbus; the server takes direct deps on them).
- **Wire encoding: none** — `serde_json` (existing workspace dep), §3.1.
- **MCP: none** — hand-rolled stdio JSON-RPC (§7); `rmcp` rejected (mandatory tokio/tokio-util + no declared rust-version).
- **`image`** (already in-workspace) — `dooduel_mcp`'s `get_canvas` PNG only. No PNG on the wire.
- **TLS: deliberately absent in M1** (§6.2) — the `wss://` rustls path pulls `ring`/`aws-lc-sys` with known deny.toml friction ("OpenSSL" license expression not allowlisted); that lands as its own reviewed delta in M3.
- **Fallback:** if the tokio-free server path hits real friction, tokio+mio+socket2+tokio-util ≈ 6–10 clean-licensed lock nodes — the cost is a second async ecosystem; named, not chosen.

**CI delta** (rev-2; gate finding `wasm-client-claim-has-no-ci-gate`): (a) a wasm gate for the client graph — at minimum `cargo check --target wasm32-unknown-unknown -p dooduel_web` (with the web-smoke job's RUSTFLAGS), or fold `dooduel_web` into the existing web-smoke job — so an ewebsock/wasm breakage or a `dooduel_core` purity regression cannot pass silently; (b) new packages ride the existing MSRV 1.95 workspace check (dep MSRVs verified above); (c) the `Cargo.lock` change lands with `cargo deny check` in its own commit per house rule; (d) new packages join the headless nextest workspace gate + MT lane; (e) `doc = false` on the new bins (the rustdoc bin-collision precedent).

---

## §11 M1 boundary (explicit deferrals + accepted quirks)

Deferred, each owned by a milestone: room-settings depth + copy-invite-link + lobby polish (M2 — but the **minimal live lobby** — live roster + host-gated start — is **in M1**, the acceptance bar needs it); public matchmaking + TLS (M3); word modes / custom lists / languages / palette / wire-encoding optimization (M4); social/moderation + guess-censoring (M5); P2P (M6). Accounts stay out (likely permanently).

Accepted-and-documented M1 quirks: any-player `Continue` (§3.2); the substring guess leak (§5.3); `ws://`-only (§6.2); solo remains a demo/verification path, not a marketed mode (§8). The hot-seat `SwitchSeat` is removed from networked play; the `playtest_host` bin is retired at the end of the M1 PR series (superseded by `dooduel_mcp` + the in-process path).

---

## §12 Decision log

| # | Decision | Rationale | Runner-up (rejected) |
|---|---|---|---|
| 1 | Dedicated authoritative server for v1 | Reliability; browsers can't accept inbound connections — a server is near-unavoidable once web is a target | Player-hosted (needs signaling/relay for web anyway); thin relay + peer authority (weaker correctness) |
| 2 | Transport/authority split; `Session` transport-agnostic | Keeps P2P (M6) additive; makes solo an in-process transport | Socket baked into the authority |
| 3 | Pure-core authority, lightweight async server; client = replica | Lightest server, cleanest determinism, best P2P seam | Headless Bevy server (heavy; canvas side-surface undercuts the reuse win) |
| 4 | Agents = headless protocol clients w/ game-semantic MCP tools | The literal "connect the same as humans"; extends `playtest_host` | a11y-tree GUI driving (a GUI per agent; awkward for drawing) |
| 5 | Solo kept as in-process local authority | Demo/verification path; exercises the split daily | Networked-only; two code paths |
| 6 | Ephemeral identity (name + avatar + rotated reconnect token) | Account-less by design; reconnection needs a token, not a login | Accounts/profiles/stats |
| 7 | *(rev-2)* Pure core extracted to `dooduel_core` package | The `dooduel` lib links the whole GUI stack — a server dep on it violates decision 5 (gate blocker) | session/protocol inside the `dooduel` lib |
| 8 | *(rev-2)* Canvas = per-turn op log; undo = remove-op; late join = log replay; no PNG on the wire | Deterministic by the integer-op fact; composes with late join by construction; prior art (skribbl `drawCommands`); kills the snapshot/undo desync class + the server rasterizer | Raster `CanvasSnapshot` PNG + per-client snapshot-ring undo (provably desyncs late joiners) |
| 9 | *(rev-2)* `serde_json` text frames for M1 | Zero new dep; debuggable; string-based secrecy scan; no bulk binary left after decision 8 | `postcard` binary (new dep; positional fragility; advantage evaporated) — M4 revisit; `bincode` rejected (RUSTSEC-2025-0141) |
| 10 | *(rev-2)* `Create` vs `Join` split; server-generated codes | A typo'd `Join` must fail, not found an empty room; name-hash codes collide | Join-creates-if-fresh (rev-1) |
| 11 | *(rev-2)* Hand-rolled stdio JSON-RPC MCP | `rmcp` hard-requires tokio (verified) — defeats the one-async-ecosystem goal; the tools-only surface is ~200–400 lines | `rmcp` SDK |
| 12 | *(rev-2)* Duration-remaining-at-send + monotonic client anchor | No cross-machine clock agreement needed; error ≈ one-way latency, safe direction; prior art | Absolute deadline (clock skew); server tick number (needless machinery) |
| 13 | *(rev-2)* Token rotated per (re)connection, single-use; live-token join replaces the old connection | Closes sniffed-token replay; supports hung-tab rejoin (Colyseus precedent) | Static token for seat lifetime |

---

## §13 Rev-2 change log (the 2026-07-04 gate)

Gate shape: 2 research agents (crate-ecosystem reality; netcode prior art) + 3 fresh-context reviewers (architecture-vs-code; protocol/correctness/security; verification/scope) → 35 adversarial verifiers, one per non-nit finding. Coverage 5/5, zero holes. Verdicts: all three reviewers **CHANGES-REQUIRED**; **33 findings confirmed** (2 blockers, 24 majors, 7 minors), **5 refuted**, 1 nit. Every confirmed finding is applied above; the headline structural changes:

- **Blocker `lib-not-pure-server-links-bevy`** → §2.1: the `dooduel_core` package extraction (+ `bevy_reflect` migration, packages-not-bins, workspace members, wasm-graph purity).
- **Blocker `replica-state-undefined-leaks`** + `replica-cannot-be-game` → §4.1 `RoomReplica` with the negative invariant; §3.3 event→replica mapping table; the podium lift moves to `MatchEnded`.
- **The "`Game` reused verbatim" claim was false** (3 findings) → §2.3 API delta: roster-parameterized start, `is_bot` (bot-guard off `viewing_as`), vacant seats, `force_end_turn`, `word_display_for(seat)`, `chat_input`/`viewing_as` leave the core.
- **Canvas redesign** (5 findings + prior art) → §2.2/§3.5: op-log authority, stroke framing (`stroke_id`+`done`, exact samples, integer coords, pre-applied eraser factor), no-echo-to-originator, `CanvasLog` late-join sync, PNG off the wire.
- **Protocol pinned** (4 findings + research) → §3.1 encoding/framing/version/limits; `Create`/`Join` split; avatar wire enum + cap; protocol derives incl. Reflect.
- **Clock representation** → §4.3 duration-remaining + monotonic + no-upward-jump clamp.
- **Server lifecycle** (4 findings) → §6: host migration rule, GC-on-empty, token entropy/rotation/replacement, room-code entropy + join rate limit, no-TLS-in-M1.
- **Secrecy predicate + test respec** (3 findings) → §5.1 three-way `knows(seat)`; §5.3 accepted substring leak; §9.2 scoped scan.
- **Testability** (3 findings) → §9.5 e2e mechanics (CARGO_BIN_EXE, port 0, condition waits, no-retries, MT lane), §1.4 acceptance operationalization, §10 CI delta.
- **Client integration honesty** → §2.4/§4.2 non-blocking transport + NetPlugin system (`Cmd::task` explicitly not the seam); §8 solo bot/tick ownership.
- Refuted (not applied): `drawer-drop-picking-unbuildable` (auto-pick already covers it — now noted in §6.3), `pick-index-validation`, `m1-acceptance-needs-lobby-roster` *as a blocker* (but the minimal-lobby scope is now explicit in §1.2/§11), `switchseat-removal-disposition-unstated`, `continue-semantics-undefined` (Continue now pinned anyway, §3.2). Nit `server-undo-ring-memory` mooted by decision 8 (no server pixel ring).
