# MVU keyed Subscription — external-input ingest seam (design)

**Date:** 2026-07-11
**Status:** draft
**Realizes / advances:** [`2026-06-29-mvu-as-core-design.md` § 8](2026-06-29-mvu-as-core-design.md) (the keyed `Subscription` roadmap item; `Origin::Subscription` log-format hook already baked in)

> **This is the framework half of issue #143 Item 6 ("WebSocket-into-ECS").** The research
> ([Track B brief], audit `reports/2026-07-11-web-first-class-audit.md`) found the audit's Item-6
> framing internally contradictory: its §6 said "build the WebSocket→ECS bridge in `buiy_core`",
> its §3.C said that would breach the foundation non-goal ("networking … is the consuming app's
> concern"). Both cannot hold. This spec resolves it: **the framework provides a transport-agnostic
> external-input *ingest* seam (the MVU keyed `Subscription`); the app owns the transport.** A live
> WebSocket stream is the archetypal Subscription source — so Item 6 is the forcing function that
> lands the already-specified §8 Subscription seam, and it adds **zero networking dependencies to
> the framework.**

## Purpose

Give an MVU app a first-class, replay-safe way to fold a **long-lived external input stream**
(a socket, a timer, an OS event source, a channel) into `Model` state — without any `await` in a
system and without the framework taking on transport, protocol, or reconnect. Today MVU has only
`Cmd::task` (PR #110), which is **one-shot** (request → one result → done). A socket is a *stream*
with a start/stop lifecycle; nothing in the shipped surface models it, so `apps/dooduel` hand-rolled
a per-frame drain system (`net.rs`). This spec generalizes that pattern into the seam MVU §8 already
committed to.

## 1. Why this is MVU completeness, not networking

The foundation excludes transport (`2026-05-07-buiy-foundation/cross-cutting.md:96` — *"Networking,
fetch, XHR, WebSocket, WebRTC, WebTransport. **O**"*; `README.md:49` non-goal). That exclusion is
correct and stays. But an app that owns a transport still needs a *sanctioned way to get inbound
data into `Model`* — and that ingress is pure MVU: it flows through the same `enqueue → drain`
funnel, is logged with an origin tag, and must obey replay determinism. MVU §8 already names this
(`2026-06-29-mvu-as-core-design.md:219-228`) and already reserved `Origin::Subscription`
(`crates/buiy_core/src/mvu/mod.rs:322`, defined early *"so adding it later is not a format change"*)
and `enqueue_with_origin` (`mvu/mod.rs:638`). This spec ships that reserved seam. **No socket, no
protocol, no networking crate enters `buiy_core`.**

## 2. Decisions

### D1 — Ship the keyed `Subscription` seam in `buiy_core` (realize MVU § 8)
A `Subscription` is a keyed, long-lived source of `Msg` values owned by a `Model`. Each frame the
runtime diffs the active sub-set (computed from the owning `Model`, exactly as §8:224 specifies):
**start** new keys, **drop** vanished keys (drop = cancel the source), leave unchanged keys running.
Every emission enters via `enqueue_with_origin::<M>(cmds, target, msg, Origin::Subscription)` — the
same funnel as every other Msg. The source's async work is spawned **once** at start
(`wasm_bindgen_futures::spawn_local` on wasm / Bevy task pool native) into a single-thread-safe
channel; a per-frame drain system pops the channel and enqueues. **Zero `await` in any system** —
the load-bearing wasm-safety invariant (single-threaded scheduler, never block the browser main
thread; `prior-art/web-rendering/{threading,lessons,winit-web}.md`).

The async primitives this needs are **already in `buiy_core`'s dependency graph** — `spawn_local` is
used today (`text/edit/clipboard.rs:224`) and `crossbeam-channel` / `async-channel` are already
transitive via Bevy — so the seam adds **no new framework dependency** (separate from the "no
*networking* dep" point above), regardless of which spawn-ownership model is chosen (Open Q3). Two
models exist and must be pinned at B0: **(A)** the framework spawns an app-provided `Future`/source
into a framework-owned channel (more ergonomic for simple sources like a timer), vs **(B)** the app
owns the source's spawn + channel and the Subscription is a thin keyed per-frame drain over an
app-provided receiver/poll-fn (matches the § 3 Dooduel reconciliation, where `drain_client_net`
already owns its `try_recv`; the leaner default). Both are zero-new-dep; the choice sets how thin the
Dooduel adapter is.

*Rejected:* extend `Cmd::Task` to be stream-shaped. Rejected — §8 deliberately pairs one-shot
`Cmd::task` *with* keyed `Subscription` as two distinct primitives; overloading Task with lifecycle
+ keying reproduces the Subscription design badly and muddies the one-shot contract.

### D2 — Replay never starts a subscription; it re-feeds logged Msgs
Per §8:226/228, replay is deterministic: it **does not** open the source or re-run the effect — it
re-injects the recorded `Msg` values stamped `Origin::Subscription`. A recorded inbound frame thus
replays without reconnecting. This is already how Dooduel's solo path behaves (`net.rs`
`solo_match_seed`: "replay re-feeds the recorded `Msg::Net` stream, never a re-run session"); the
seam makes it a framework guarantee, not an app convention.

### D3 — The seam is inbound-only; outbound stays an effect/app-resource drain
A Subscription ingests. Sending is a separate concern the app already models as a `Cmd`/effect or a
resource drain (Dooduel: `net_outbox` + `drain_outbox`, `net.rs:136`). Keeping outbound out of the
Subscription keeps the primitive small and its replay story clean (outbound is a side-effect replay
skips anyway).

*Rejected:* a bidirectional `Subscription` that also owns `send`. Rejected — couples ingest lifecycle
to send policy, and send has no replay meaning.

### D4 — Transport stays 100% app-owned; ship a reference pattern, not a transport
The socket, wire protocol, framing, and reconnect are the app's, per the foundation non-goal. The
deliverable beside the seam is a **documented pattern + a worked example** modeled on Dooduel's
`net.rs` (transport trait with non-blocking `try_recv`/`send`/`status`; `ewebsock` as the
Dooduel-proven **unified native+wasm** transport; an in-memory `InProcess` transport for headless
tests). The framework blesses no transport crate.

*Rejected:* (b) a transport provider inside `buiy_core` — breaches the non-goal and forces
ewebsock/tungstenite into every UI consumer's binary + cargo-deny surface for a capability ~none use.
*Deferred:* (c) an **opt-in, feature-gated `buiy_net` sibling crate** (ewebsock-based, cfg-selected
native/wasm, a shared `ConnStatus`) — legitimate *only* if the widget-catalog multiplayer demo
(issue #142) actually materializes and wants a reusable transport. **Demand-gated; not built now.**

### D5 — Reconnect/backoff + connection-state stay app-owned
Reconnect is transport *policy*. Keep the app-side pure state-machine shape Dooduel proved
(`ws_decision`/`WsAction`, bounded tries + monotonic backoff, `ConnStatus` resource; `net.rs:411`).
The framework's only guarantee: a Subscription **re-keys cleanly** — dropping and re-adding a key
starts a fresh source with no leaked task.

### D6 — Scope-record annotation (no non-goal reversal)
Unlike the URL router (Item 7, which reverses a bundled exclusion — see the
[router spec](2026-07-11-buiy-url-router-design.md)), this needs **no reversal**: transport stays
`O`. Add one clarifying line to `cross-cutting.md:96` / `README.md:49`: *"transport remains
app-owned; the MVU external-input ingest seam (keyed `Subscription`, `Origin::Subscription`) is the
sanctioned in-framework way an app-owned transport folds inbound data into `Model` state."* This
records the boundary; it does not move it.

## 3. Reconciliation with `apps/dooduel` (purely additive)

`feat/dooduel-multiplayer-m1` already runs the target pattern (QA cycle-3 clean). Landing this seam
moves **no** code into the framework:
- `dooduel_core/src/{transport,session,protocol}.rs` — stay app-side verbatim (transport traits,
  `InProcess`, `WsClientTransport`/ewebsock, the room-actor server, the `serde_json` wire).
- `dooduel/src/net.rs` `drain_client_net` (`:160`) — becomes a thin adapter that registers a
  Subscription keyed on the connection, mapping inbound frames → `Msg::Net`, replacing the hand-rolled
  drain. Outbound (`drain_outbox`) and reconnect (`ws_decision`) stay app-side unchanged.
- Net effect: Dooduel *gains the option* to express its inbound bridge as a first-class Subscription;
  nothing it owns migrates. This spec is validated against, and reconciled with, that running code.

**Dependency:** the Dooduel M1 branch must land on `main` first (it carries the reference impl this
seam reconciles with + the framework fixes the web campaign sits on). Prep (this spec) proceeds now;
execution waits on that merge.

## 4. Verification

- **Headless unit tests (no socket):** drive the Subscription seam with an in-memory source that
  emits scripted `Msg`s; assert start/drop-on-rekey, `Origin::Subscription` stamping, no leaked task,
  and that **replay re-feeds the logged Msgs without invoking the source** (D2). This is the lowest
  tier that observes the behavior — it needs no transport.
- **Native integration (real transport):** an echo-server fixture (Dooduel's
  `apps/dooduel_server/tests/e2e.rs` spawns the real server via `CARGO_BIN_EXE_*` and drives real
  WebSockets) — proves the ewebsock reference pattern end-to-end, **native only**.
- **Wasm socket path:** CI likely cannot exercise a real `web-sys` WebSocket without a browser+server
  harness → **manual/browser smoke**, consistent with foundation § 2.9 (web = manual release gate).

## 5. Open questions (for plan / review)
1. Confirm landing the §8 Subscription seam *now* (vs. keeping Item 6 a documented pattern with the
   seam still deferred). Recommendation: land it — already spec'd, and this is its natural forcing
   function.
2. `buiy_net` opt-in crate: build now, or defer to widget-catalog-multiplayer demand (recommend
   defer)?
3. **Spawn-ownership boundary (pin at B0):** model (A) framework-owns-the-channel (spawns an
   app-provided `Future`/source) vs model (B) app-owns-spawn-and-channel + thin keyed drain (matches
   the Dooduel reconciliation; leaner). Recommendation: support (B) at minimum — it is the thinnest
   framework surface and the one the Dooduel adapter reduces to. Either way the dep footprint is zero
   (§ 2 D1: `spawn_local` + `crossbeam`/`async-channel` already in-graph). This also fixes the exact
   `Subscription` public API shape (key type, source trait vs receiver, registration ext), pinned
   against the §8 text and the `Cmd::task` precedent (PR #110).

## 6. References
- Realizes: [`2026-06-29-mvu-as-core-design.md` § 8](2026-06-29-mvu-as-core-design.md).
- Audit: [`reports/2026-07-11-web-first-class-audit.md`](../reports/2026-07-11-web-first-class-audit.md) (Item 6).
- Reference impl (unmerged): `feat/dooduel-multiplayer-m1` — `apps/dooduel_core/src/{transport,session,protocol}.rs`, `apps/dooduel/src/net.rs`.
- Wasm constraints: [`prior-art/web-rendering/`](../prior-art/web-rendering/) (`threading.md`, `lessons.md`, `winit-web.md`).
- Code anchors: `crates/buiy_core/src/mvu/mod.rs` (`Origin::Subscription` :322, `enqueue_with_origin` :638, `Cmd::task` :239).
- Sibling campaign track: [URL router design](2026-07-11-buiy-url-router-design.md); campaign roadmap: `plans/2026-07-11-web-first-class-campaign.md`.
