# Dooduel production multiplayer — M1: the authoritative networked session spine

**Date:** 2026-07-04
**Status:** `draft` — brainstormed + section-by-section approved (2026-07-04); awaiting the written-spec review gate before the implementation plan. NO implementation code.
**Kind:** `design` (milestone M1 of the production-multiplayer campaign; see §1.2 for the M1–M6 decomposition).
**Base:** `origin/main` @ `6e07954` (the merged Dooduel FINAL — `apps/dooduel` at design parity).
**Worktree:** `dooduel-app2` (branch `feat/dooduel-multiplayer-m1`).
**Supersedes:** nothing. It *reframes* the shipped app: the [Dooduel FINAL](2026-07-03-dooduel-final-design.md) built the game to exact parity with a design bundle that is itself a **single-player simulation** of a multiplayer game. This design takes the features that bundle *mocks* — rooms, joining, real opponents, invite links — into production with real implementations. The FINAL's pure `game::Game` core is reused verbatim; what changes is *who runs it* and *how clients reach it*.

**Seeds (required reading):**
- The shipped app + its pure core — `apps/dooduel/src/game.rs` (the deterministic rules; depends only on `Duration` + `Reflect`), `apps/dooduel/src/lib.rs` (the MVU shell), `apps/dooduel/src/bin/playtest_host.rs` (the file-protocol multi-agent host this design supersedes).
- The design bundle + its own honesty about what it mocks — `../reference-designs/dooduel/REQUIREMENTS-DELTA.md` §6 ("Still the solo demo with a dev-only 'playing as' seat-switcher… No bots, no real networking (all deferred)") + §11 (the deferred "full game" list).
- The acceptance playtest that ran on the file protocol — `../reports/2026-07-04-dooduel-acceptance-playtest.md` (the bar this milestone re-hits, now genuinely networked).

---

## §1 Intent

### 1.1 The reframe

The Dooduel design bundle is a **single-player simulation** of a multiplayer game: it runs entirely in one browser tab, the "other players" are seeded bots, the room code is a cosmetic string, "join a room" connects to nothing, and the seat-switcher hot-seats one human across every seat. "Match the design target exactly" faithfully reproduced those *mocks as mocks*. This milestone begins converting the mocks into real implementations, starting with the load-bearing one: **an authoritative networked session that humans (native + web) and LLM agents connect to and play together.**

The pure `game::Game` state machine is the crown jewel — deterministic, side-effect-free, unit-testable without an ECS/GPU/clock. It is reused **verbatim**. What this milestone changes is architectural: the game stops running *in each client* and starts running in one authoritative `Session`; clients become replicas.

### 1.2 Campaign decomposition (each milestone = its own spec → plan → build)

- **M1 (this spec): the authoritative networked session spine + private-room slice.** Wire protocol, dedicated server, the client refactored from authoritative to replica, in-process + WebSocket transports, invite-code rooms, native + web + MCP clients, one full match to podium.
- **M2: room configuration + host settings** — the real `Config` surface (rounds / draw-time / hints / max players), host-gated start, live lobby roster from actually-connected players, a real copy-invite-link.
- **M3: public matchmaking** — a quick-match queue behind the "▶ Play" path.
- **M4: content depth** — word modes (Normal / Hidden / Combination), custom word lists, languages, the full 20+ palette.
- **M5: social / moderation** — like/dislike, votekick, live presence status, spoiler-safe post-guess chat over the wire.
- **M6 (eventual, deliberately not designed out): P2P transport** — WebRTC data channels + a peer-hosted `Session` + signaling. M1's transport/authority split is what makes this additive, not a rewrite.

### 1.3 Decisions locked at the brainstorm gate (2026-07-04)

1. **Dedicated authoritative server for v1**, but the transport/authority boundary must keep P2P open (decision-owner: the user; "dedicated server to start, for reliability, but we still want p2p support eventually so don't design that out").
2. **V1 = the private-room vertical slice** with humans (native + web) **and** MCP agents in the *same* milestone — one authoritative match to the podium. No public matchmaking, no settings depth, Normal-mode English, the current 16 colors.
3. **Agents play as headless protocol clients** exposing game-semantic MCP tools — never by driving a GUI through the a11y tree.
4. **Offline solo (in-process local authority + bots) is retained as a demo / verification / test path** — not a marketed offline mode, but the seat that exercises the transport/authority split from day one.
5. **Server authority = pure-core in a lightweight async server** (Approach A), not a headless Bevy app (Approach B). The client becomes a replica.
6. **Identity is ephemeral** — name + avatar + reconnect token, no accounts (confirmed consistent with the bundle's account-less model; accounts stay out of scope, likely permanently).

### 1.4 Acceptance bar

A real match — the user in the native or web GUI + N MCP agents, each a distinct seat — played end-to-end to the podium over the live `dooduel-server` on a real WebSocket. This is the campaign's original bar (`../reports/2026-07-04-dooduel-acceptance-playtest.md`), now genuinely networked instead of file-protocol-mediated.

---

## §2 Architecture — the transport/authority split

The organizing idea: **the authority is transport-agnostic.** A `Session` owns the game and does all rule enforcement and redaction; it performs no I/O. Transports move bytes. A dedicated server and an in-process solo run drive the *same* `Session` behind *different* transports; a peer-hosted P2P session (M6) is a third transport behind the same boundary.

```
              ┌─────────────── dooduel (lib, pure + wasm-safe) ───────────────┐
              │  game::Game        the deterministic rules (reused verbatim)   │
              │  protocol          ClientIntent / ServerEvent (serde)          │
              │  session::Session  AUTHORITY: owns Game + roster + canvas +    │
              │                    per-recipient word redaction. NO I/O.       │
              │  transport (trait) send(to)/recv — the seam                    │
              │    ├ InProcessTransport   channels (solo + tests)              │
              │    └ WsClientTransport    ewebsock (native + wasm)             │
              │  net               applies ServerEvent → Game replica          │
              └───────────────────────────────────────────────────────────────┘
                        ▲ depends on                    ▲ depends on
        ┌───────────────┴───────────┐      ┌────────────┴───────────────┐
        │ dooduel-server (new bin)   │      │ dooduel-mcp (new bin)       │
        │ room registry + WS accept  │      │ headless client + MCP tools │
        │ loop + per-room tick        │      │ (guess/pick/stroke/state)   │
        └────────────────────────────┘      └─────────────────────────────┘
   also depend on the lib: dooduel (GUI bin), dooduel_web (wasm)
```

**Crate / module layout:**
- `dooduel` (existing lib, stays pure + wasm-safe): `game`, `protocol`, `session`, the `transport` trait + `InProcessTransport` + `WsClientTransport`, and the `net` reducer wiring. The protocol and `Session` carry **no transport dependency** (transport is a trait), so the lib gains no server-only deps.
- `dooduel-server` (new native-only bin under `apps/`): the room registry, the WebSocket accept loop, and the per-room tick. Isolated dep tree; never in the wasm graph.
- `dooduel-mcp` (new native-only bin under `apps/`): a headless client + the MCP tool surface.
- `dooduel` GUI bin + `dooduel_web` wasm crate: both keep their MVU app; both gain the client `net` wiring and a transport (`WsClientTransport` for real play; `InProcessTransport` for the solo/demo path).
- `playtest_host` bin: superseded by `dooduel-mcp` + the in-process path; kept until M1 lands, then retired in the M1 PR series' final slice.

---

## §3 The wire protocol

serde-serializable types in `dooduel::protocol`, shared by every client and the server. Versioned with a `protocol_version` handshake field so mismatched builds fail cleanly rather than mis-decode.

**Client → server — `ClientIntent`:**

| Intent | Phase gate (server-enforced) | Notes |
|---|---|---|
| `Join { room, name, avatar, reconnect: Option<Token> }` | any | Creates the room if the code is fresh (joiner ⇒ host); else attaches. |
| `StartMatch` | Lobby, host only | Host-gated. |
| `Pick { index }` | Picking, drawer only | |
| `Guess { text }` | Drawing, non-drawer | Routed through `Game::apply_guess`. |
| `Stroke { points, color, size, tool }` | Drawing, drawer only | Coalesced polyline batch (~30–60 ms of pointer moves). |
| `Fill { seed, color }` | Drawing, drawer only | A command, not pixels (see §4.4). |
| `Undo` / `Clear` | Drawing, drawer only | |
| `Continue` | Reveal | The "Continue →" advance. |
| `Leave` | any | Graceful seat release. |

**Server → client — `ServerEvent`, rendered *per recipient*:**

`Welcome { seat, reconnect_token, room, protocol_version }` · `Snapshot { … room state as this recipient may see it … }` · `Roster { players }` · `Phase { phase, drawer, round, countdown_anchor }` · `Word { display }` (**redacted per recipient** — §5) · `CanvasStroke` / `CanvasFill` / `CanvasUndo` / `CanvasClear` / `CanvasSnapshot` (full canvas for late joiners) · `Chat { line }` (shared broadcast; the private "So close 👀" nudge is addressed only to its seat) · `Scores` / `TurnResult` · `Error { code, message }`.

The redaction discipline is the reason `ServerEvent`s are addressed rather than broadcast-identical: the *same* logical phase change produces a `Word` payload with the full secret for the drawer and a blanks+hints payload for guessers.

---

## §4 The client-replica refactor

The real structural change to `apps/dooduel`.

### 4.1 Model split: replica vs local
The model divides into (a) a **replica** of the authoritative room — the `game::Game` plus roster/canvas mirror, mutated **only** by `Msg::Net(ServerEvent)` — and (b) **local-only UI state** that never crosses the wire: tool/color/size selection, the avatar editor, theme, viewport, `chat_input`. This is a clean boundary and clarifies the existing model (today everything is one authoritative blob).

### 4.2 Intents replace local folds
Today `update()` folds gameplay by mutating `s.game` directly. After M1, the gameplay messages (`Play`/`StartMatch`, `ChooseWord`, `SubmitGuess`, `Continue`, and the stroke path) **stop mutating `game` locally** and instead return a `Cmd` that sends a `ClientIntent`. The replica updates on the server's authoritative echo. `SwitchSeat` (the hot-seat dev switcher) is removed for networked play — a client controls exactly one seat.

### 4.3 Clock: server-truth, client-interpolated
The server owns the authoritative clock and emits `Phase` changes plus a periodic countdown sync. The client keeps a **display** countdown that interpolates from the last anchor via local wall-clock, so the timer animates smoothly without per-second network chatter. The client never self-advances a phase; the existing `ClockPlugin`/`Tick` becomes a *display* tick on the client and the *authoritative* tick on the server's `Session`.

### 4.4 Strokes: optimistic, no reconciliation
The drawer paints optimistically **and** emits a `Stroke` intent. Because the server does not transform strokes, the optimistic paint equals the authoritative echo — no reconciliation. Guessers paint only from `CanvasStroke`/`CanvasFill`/`CanvasUndo`/`CanvasClear` events. **Fill travels as a command** (seed point + color) replayed by each client's existing deterministic flood-fill — not a pixel buffer — so messages stay tiny. Late joiners receive one `CanvasSnapshot` (PNG) to seed their canvas, then follow the event stream.

---

## §5 Anti-cheat / word secrecy — the load-bearing property

The single correctness property that the whole architecture exists to guarantee: **a guesser's client never receives the secret word.**

- `Session` renders each `ServerEvent` *per recipient*. A guesser's `Word`/`Snapshot` carry only blanks + revealed hints. The secret string exists **only** server-side and in the drawer's payload. There is no client-side redaction of a value the client shouldn't hold — today's `word_display()` per-seat redaction moves *into* `Session`, server-side.
- All scoring/matching runs server-side (`Game::apply_guess`); a client cannot fake a correct guess.
- Seat authority is server-enforced: a `Pick`/`Stroke`/`Fill`/`Undo`/`Clear` from a non-drawer, or a `Guess` from the drawer, is rejected (`Error` to the sender), never applied.
- This is directly testable (§9): assert that a guesser's entire event stream, byte-scanned, never contains the secret word.

---

## §6 The server — `dooduel-server`

**Actor-model rooms, no locks.** A room registry `RoomCode → Room`; each `Room` is a single async task that *solely owns* its `Session`. Connections forward `ClientIntent`s into the room task over a channel; the room task is the only mutator, so `Game` mutation ordering is deterministic (no mutex on game state — the pure core's determinism is preserved end-to-end).

**Per-room tick** at ~10 Hz calls `session.tick(now)` for crisp countdowns/hints/auto-pick/turn-end; guesses and strokes are processed event-driven on arrival (not tick-gated). After each intake/tick, the room drains the `Session`'s addressed outbound events to the right connection senders.

**Room lifecycle:** the first `Join` with a fresh code creates the room and makes that joiner the host; the invite code *is* the room code. Empty rooms are garbage-collected after all connections leave plus the reconnection grace window.

**Reconnection (§1.3 decision 6 identity):** `Welcome` hands each client a `reconnect_token`. On disconnect the seat is held "away" for a grace window (**45 s**, a `dooduel-server` constant); the roster shows the player away. A `Join` with a valid token re-attaches to the held seat and resyncs via `Snapshot` + `CanvasSnapshot`. Past grace, the seat is freed. If the **drawer** drops past grace mid-turn, the turn ends early (reusing the existing turn-end machinery).

**Error discipline:** malformed/unauthorized intents are validated, logged, and answered with `ServerEvent::Error` — never a panic (the `playtest_host` discipline, carried forward).

---

## §7 The MCP client — `dooduel-mcp`

A headless client: it opens a `WsClientTransport` to the server, joins a room, and exposes game-semantic MCP tools that map 1:1 onto the protocol:

- `join_room(room, name)` · `list_choices()` / `pick_word(index)` · `guess(text)` · `draw_stroke(points, color, size)` / `fill(seed, color)` / `clear()` · `continue_turn()` · `get_state()` (the per-seat honest view `playtest_host` already produces, now fed from the wire) · `get_canvas()` (PNG of the agent's canvas replica, rebuilt from `CanvasStroke`/`CanvasFill` events — exactly what `playtest_host`'s per-stroke PNG did).

One agent per process (N processes for N agent seats), matching how the acceptance playtest already ran. The MCP transport is either the `rmcp` SDK or a minimal hand-rolled stdio JSON-RPC server (see §10 — a review-gate decision).

This is the literal form of "agents connect the same as humans": the agent is not a special seat in the game — it is an ordinary protocol client whose intents are indistinguishable, at the `Session`, from a GUI client's.

---

## §8 Offline / in-process local authority (the solo + verification path)

"Solo" is the same `Session` running in-process behind an `InProcessTransport` (channels), with bots filling the empty seats via the existing `game::Game` bot machinery (`bots_enabled`). No socket, no server process. This path:
- preserves an instant offline experience and the current single-player game as a **demo / verification harness** (per decision 4 — it is not a marketed feature);
- exercises the transport/authority split from day one, so a bug in the boundary surfaces in fast headless tests, not only over a live socket;
- is the substrate for the in-process integration tests (§9).

The GUI bin selects the transport at match start: "play solo" ⇒ `InProcessTransport` + a local in-process `Session` + bots; "create/join a room" ⇒ `WsClientTransport` to `dooduel-server`.

---

## §9 Testing strategy

Lowest tier that observes each property (the project's testing-anti-patterns discipline):

1. **Pure `Session` tests** (no I/O, `InProcessTransport`): scripted intents → assert authoritative state **and that a guesser's event stream never contains the secret word** (the direct anti-cheat test, §5).
2. **In-process integration:** one `Session` + N in-process clients (incl. a headless one) play a full match to podium; assert scores/podium. Doubles as the solo/demo path.
3. **Two-process e2e:** a real `dooduel-server` + `dooduel-mcp` clients over a localhost WebSocket; full match; assert. Proves the real transport.
4. **Reconnection:** drop + re-`Join` with token → seat + canvas restored.
5. **Client replica:** feed `ServerEvent`s into the app's `Msg::Net` and assert the view via the probe (`BuiyProbePlugin`).
6. **Existing GPU/visual gates unchanged** — rendering is untouched by this milestone.
7. **Acceptance:** the §1.4 real networked match (user GUI + N MCP agents → podium).

---

## §10 Dependency delta (review-gate decisions)

Three new dependencies, all subject to the `cargo deny` gate + manual pin discipline (no dependabot; MSRV 1.95):

- **`ewebsock`** — the client transport, one API over native `tungstenite` + wasm `web-sys` WebSocket, background-threaded (no async reactor needed inside the Bevy app, which has no tokio). Used by the native GUI **and** the wasm client, so it enters the shared graph.
- **`async-tungstenite`** on the existing `async-io`/`futures-lite` reactor — the server accept loop, **tokio-free** to keep the workspace on the one async ecosystem already present. `dooduel-server`-only.
- **An MCP surface** — either the `rmcp` SDK or a minimal hand-rolled stdio JSON-RPC server (zero new SDK surface). `dooduel-mcp`-only. **Open for the review gate;** recommendation: start hand-rolled (the tool set is tiny and stable) unless `rmcp` is already a desired standard.

**Server async runtime** — recommendation is `async-tungstenite` + `async-io`/`smol` over tokio, for `cargo deny` weight and ecosystem unity; tokio is the fallback if we hit friction. Flagged rather than silently chosen.

---

## §11 M1 boundary (explicit deferrals)

Out of scope for M1, each owned by a later milestone: public matchmaking (M3); room-settings depth + real copy-invite-link + live-roster lobby (M2); word modes / custom word lists / languages / full 20+ palette (M4); like-dislike / votekick / presence-status polish / spoiler-safe post-guess chat (M5); P2P / WebRTC (M6). Accounts / persistent stats stay out (likely permanently — account-less by design). Identity in M1 is ephemeral: name + avatar + reconnect token.

---

## §12 Decision log

| # | Decision | Rationale | Runner-up (rejected) |
|---|---|---|---|
| 1 | Dedicated authoritative server for v1 | Reliability; browsers can't accept inbound connections and in-browser P2P needs a signaling/relay server anyway, so a server is near-unavoidable once web is a target — and it is strictly simpler and more correct | Player-hosted authoritative (needs signaling+relay for web anyway; host churn); thin relay + peer authority (weaker correctness) |
| 2 | Transport/authority split; `Session` is transport-agnostic | Keeps P2P (M6) additive rather than a rewrite; makes solo an in-process transport | Bake the socket into the authority (would design P2P out) |
| 3 | Pure-core authority in a lightweight async server (Approach A); client becomes a replica | Lightest server (no Bevy/render), cleanest determinism, best P2P seam; reuses the pure `Game` fold verbatim | Headless Bevy app server (Approach B — heavy; canvas is a side-surface needing its own channel anyway, undercutting the reuse win) |
| 4 | Agents are headless protocol clients w/ game-semantic MCP tools | The literal "connect the same as humans"; fully headless + testable; extends `playtest_host` | Drive a GUI via the a11y semantic tree (heavy — a GUI per agent; awkward for drawing; couples to UI layout) |
| 5 | Keep offline solo as in-process local authority | Preserves the single-player game as a demo/verification path; exercises the split from day one | Networked-only (loses offline + instant play); two separate code paths (drift) |
| 6 | Ephemeral identity (name + avatar + reconnect token) | Matches the bundle's account-less model; reconnection needs a token, not a login | Accounts/profiles/stats (out of scope, likely permanently) |
