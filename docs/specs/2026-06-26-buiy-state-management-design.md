**Date:** 2026-06-26
**Status:** [SUPERSEDED — historical] — superseded by the merged FINAL `docs/specs/2026-06-29-mvu-as-core-design.md` (placement re-decided from this draft's opt-in crate to **core**). Preserved as design lineage; do not treat as current.
**Subject:** Buiy state management — the Elm-bevyified MVU surface (per-entity reducers on Bevy 0.19), brainstormed target design

> **Draft / pre-final.** This captures the locked design decisions from the
> 2026-06-26 brainstorm. It is validated *in part* by prototype-1 (the bespoke
> MVU runtime, `examples/mvu_spike/`, DO-NOT-MERGE — 33/33 green + byte-identical
> replay). The **Bevy-native / leveraged** half of this design (observers/
> `EntityEvent` transport, `Messages` inbox, the V-B constrained reducer, Yew-style
> callbacks, the `Reflect` log, LogicalId + agent-interface coupling) has **not yet
> been built or run** — a **second prototype** validates it before this becomes the
> final design. See § 13.

---

## 1. Thesis

Each stateful widget is an **actor**: a per-entity **Model** (its components), an
inbound **Msg** mailbox, and a **reducer** (`update`) that is a *pure* function of
(model, msg) returning **effects as values** (`Cmd`). Composition is the ECS entity
tree — many small MVU loops, no god-Model, no `Msg.map`. **Bevy is the actor
runtime**: we build *on* Bevy's primitives (Messages, observers, `EntityEvent`,
`Changed`, `#[require]`, `bevy_state`, `AsyncComputeTaskPool`) and keep bespoke only
the three things Bevy has no equivalent for — the **purity boundary**, the **single
ordered record/replay funnel**, and the **MVU semantics**.

The payoff of purity: a recordable Msg stream gives **deterministic tests +
time-travel + LLM-agent driving** for free (prototype-1 proved byte-identical
replay). This is the write-side dual of the agent-interface's "one tree, N
consumers" → **"one Msg log, N consumers."**

## 2. Goals / non-goals

- **Goal:** ergonomic, easy-to-reason-about per-entity state that *builds on* Bevy
  and benefits fully from the ecosystem; deterministic replay/agent-driving.
- **Goal:** a developer-facing surface that *feels like* defining Bevy components +
  systems, not a parallel runtime.
- **Non-goal:** a fine-grained signal/reactivity layer (rejected in the wider sweep;
  ECS change-detection + this MVU layer is the model). A second store / god-Model.
  Supervision/restart of panicking reducers (v1 non-goal).

## 3. Core model

- **Model** = a per-entity `#[derive(Component, Reflect)]`. Reducer is **per type**,
  state is **per instance** (one behavior, many actors — like a struct's methods).
- **Msg** = a per-model `#[derive(Message, Reflect)] enum`. **Hard invariant: one
  Msg type ↔ exactly one Model type** (makes routing unambiguous). `Reflect` is
  required (§ 7 — cross-process/agent replay).
- **Reducer (purity = "V-B", read-only environment).** A **free function**
  registered via `add_reducer`, whose params are `&mut Model` + `Msg` + any Bevy
  **`ReadOnlySystemParam`** (`Res<T>`, `Query<&T>`, `Local`, …):
  ```rust
  fn update(c: &mut Counter, msg: CounterMsg, settings: Res<Settings>) -> Cmd<CounterMsg> { … }
  app.add_reducer(update);   // Commands / ResMut / Query<&mut _> → DOES NOT COMPILE
  ```
  > ⚠️ **Corrected by prototype-2 (2026-06-26):** `ReadOnlySystemParam` is
  > **insufficient** — in Bevy 0.19 `Commands: ReadOnlySystemParam` (its spawn/despawn
  > is *deferred*), so the comment above is wrong: `Commands` *would* compile and could
  > mutate structurally outside the funnel → replay diverges. Enforcement must be a
  > **sealed `PureEnv` allowlist** (`Res`/read-only `Query`/`Local`/`()`/tuples blessed,
  > `Commands` excluded); proven in `examples/mvu_native`. The final supersedes this line.
  > See `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`.

  Purity is **enforced by the type system**: the only mutable thing is the model;
  a sealed `PureEnv` allowlist (NOT `ReadOnlySystemParam`) forbids impure env params.
  The `Model` trait declares only
  `type Msg` (+ `type Out` when it emits, § 5) — **not** an `update` method (a
  method can't take SystemParams). Replay contract: "deterministic given the seeded
  environment" (env reads are reconstructed/seeded at replay-start, like Elm's init
  flags).
- **Cmd** = effects-as-values. v1: `none` / `done(msg)` (next-frame fold) / `task`
  (§ 6) / `batch`. Deferred: `stream`, `sequence`, structural.

## 4. The architecture: a hybrid recorder (three layers)

The determinism lives at **one place** — the single ordered drain that calls
`update` and records. Everything around it is idiomatic Bevy, on **one hard rule:
observers / `EntityEvent` handlers / `Messages` readers may ONLY enqueue
`(entity, msg)` into the funnel — they NEVER call `update` or mutate a model.**

1. **Bevy-native edge (transport + scaffolding):** a press/output → a propagating
   `EntityEvent` bubbled up `ChildOf` by an `On<E>` observer that *enqueues*;
   `#[require(...)]` drags a model's substrate; a relationship/stored-`Entity`
   carries bind/adoption edges; `bind` rides `Changed<M>` + `set_if_neq`.
2. **The funnel (bespoke — the determinism tap):** drains in order → records to the
   log → calls the reducer. The inbox transport is **Bevy `Messages<M>`**, but **our
   drain owns the read tap** (reads via `MessageReader`, records, runs `update`).
3. **The constrained reducer (§ 3):** the only place state changes.

Observers-as-transport: yes. Observers-as-the-reducer: **no** (that hands the
reducer a `World`, dissolving purity + replay).

## 5. Composition

- **Component params** = constructor fn args → component fields (per-instance
  config/state); **shared** config = a `Res`; **runtime** changes from a parent =
  a `Msg` (no React-style prop re-passing — that would be an untracked mutation).
- **Routing:** `on_press(Msg)` → an `EntityEvent` bubbles up `ChildOf` to the
  nearest ancestor owning that Msg's Model and enqueues; explicit-address dispatch
  is the cross-tree escape hatch. Unhandled = loud typed dead-letter.
- **Child→parent = Yew-style typed callback params (PRIMARY surface).** A reusable
  widget declares per-event typed callbacks as construction params; the parent
  supplies them inline, mapping the widget's payload → its own Msg into the funnel:
  ```rust
  pub fn editor(text: impl Into<String>, on_commit: Callback<String>, on_cancel: Callback<()>) -> impl Bundle { … }
  parent.spawn(editor(t, cx.callback(move |s| TodoMsg::Commit(id, s)), cx.callback(move |_| TodoMsg::Cancel(id))));
  ```
  `Callback<T>` is a cloneable sink that resolves to a **funnel dispatch** (a parent
  Msg), never a direct mutation. Stored as `#[reflect(ignore)]` wiring (reconstructed
  at spawn), so it is **replay-safe** — the logged thing is the resulting parent Msg.
  Desugars to the lower-level `Out`-type + entity-`observe` translator (one closure
  per adoption edge, not per nesting depth).
- **Dynamic collections:** parent owns ordered child entities; **keyed reconcile by
  DOMAIN id** (e.g. `TodoId`), not Entity/position. add=spawn / remove=despawn /
  reorder=move-entity (preserves per-child transient state — the entity *is* the
  identity). Structural ops live in a reconcile system, **never inside `update`**.
- **Derived views:** `bind(|m| …)` (`Changed`-gated, `set_if_neq`) for descendant
  props; a dedicated `Changed`-gated system for sibling/cross-subtree derivations.

## 6. Effects & async

`Cmd::task(future)` → spawned on `AsyncComputeTaskPool`, the `Task<Msg>` stored as an
`InFlight<M>` **component on the originating entity** (entity = fold-back address). A
poll system folds the result **back through `update`** (so `Changed` trips binds).
**Single-in-flight per model = takeLatest** (a new task replaces/cancels the old; v1
default). Cancellation is free on despawn/supersede (drop = cancel, cooperative).

## 7. Record / replay & agent-interface coupling

- The funnel records every drained `(LogicalId, Msg, seq)`; `ReplayMode` makes the
  drain re-run `update` but **drop all Cmds** (every consequent Msg — folded `done`,
  async `Results` — is itself a later log entry); replay re-folds from init.
  Self-contained because **effect results fold back AS Msgs**.
- **Reflect-serialized log (in scope for v1).** Because we chose *full coupling*
  (durable + agent-driven replay), messages are stored `Reflect`-serialized and
  re-emitted via `MessageWriter` — enabling cross-process / MCP / cross-session
  replay. Consequence: **every `Msg` type derives `Reflect`** (no boxed closures/
  futures *in messages*; callbacks are `#[reflect(ignore)]` wiring, § 5).
- **Identity = a stable `LogicalId` aligned to the agent-interface test-id space**
  (raw `Entity` is session-stable only).
- **REDESIGN — Action lowering through `update`.** The agent-interface in-process
  driver currently pokes Focus/OnPress/EditCommand sinks directly
  (`dispatch_action_request`, `set_focus`, `set_value`), bypassing the Msg path —
  which would make the agent/test write-path unrecorded. **This spec defines the
  seam contract + the LogicalId space; the actual driver rewrite lands as a
  companion change in the agent-interface campaign** (touching
  `docs/specs/2026-06-18-buiy-agent-interface-design/`).

## 8. Modes

`bevy_state` (`States`/`SubStates`/`ComputedStates` + `run_if(in_state(Replay))`) for
genuinely **global** modes only (screen router, the replay gate). Per-entity mode
stays a plain field on the model (States is a global singleton — wrong for
multi-instance). Costs enabling the `bevy_state` feature (a real opt-in dep).

## 9. Leverage vs bespoke (summary)

| Leverage Bevy | Stays bespoke |
|---|---|
| `Messages<M>` (inbox), observers + `EntityEvent` (routing/upward), `#[require]` + hooks (scaffolding), `Changed`+`set_if_neq` (bind), `AsyncComputeTaskPool`+`Task` (effects), `bevy_state` (global modes), `Reflect` (log), `SystemParam`/`StaticSystemParam` (the env plumbing) | the `update`-purity boundary, **the sealed `PureEnv` allowlist (purity enforcement — `ReadOnlySystemParam` is insufficient: Commands is read-only in 0.19; proto-2 finding)**, the single ordered drain + record tap + `ReplayMode` re-fold, the MVU semantics (one-Msg↔one-Model, fold-back-through-update, takeLatest, keyed reconcile), the LogicalId addressing |

## 10. Placement

An **opt-in `buiy_mvu` crate**, layered on `buiy_core`'s change-detection / observer
/ `#[require]` conventions. Fits the "Buiy provides tools over the ECS; it does not
own app state" stance.

## 11. Phasing (impl outline — to be detailed in a plan)

P1 model+reducer (V-B `add_reducer`) + `Messages` inbox + the ordered drain. ·
P2 transport: `EntityEvent`/observer routing + `bind`. · P3 effects (`Cmd::task` +
`InFlight` + takeLatest). · P4 composition: Yew callbacks + keyed reconcile +
`#[require]` scaffolding. · P5 record/replay: `Reflect` log + `LogicalId` +
`ReplayMode`. · P6 agent-interface seam (companion change). · P7 `bevy_state` modes.

## 12. Decision log (locked 2026-06-26)

Reducer purity = **V-B** (read-only env). Reducer form = **free fn via
`add_reducer`**; `Model` trait = associated types only. Effects = `-> Cmd`. Transport
= **`Messages<M>`, our drain owns the tap**. Child→parent = **Yew typed callbacks**
(desugars to Out+observe, funnel-routed). Replay = **full coupling now** (`Reflect`
log + `LogicalId` aligned to agent-interface test-ids + Action-through-`update`).
Placement = **opt-in `buiy_mvu`**. Spec scope = **full runtime, phased**. Defaults:
stored-`Entity` bind edge, hand-written derived views, `task`/`done`/`batch`,
takeLatest, per-widget controlled/self-updating (suppress `advance_toggle_on_press`
when controlled).

## 13. Status & what prototype-2 must validate

Prototype-1 (`examples/mvu_spike/`) validated the **bespoke** runtime (Inbox/drain/
OnOutput/InFlight/MsgLog) — 33/33 green, byte-identical replay, clean GUI. It did
**not** build the Bevy-native decisions above. **Prototype-2 (next, post-compaction)
builds & runs the leveraged version** to confirm it before this becomes final:
- V-B **reducer-as-constrained-system** with `ReadOnlySystemParam`-enforced purity
  (does the type-level enforcement actually compile + feel right?).
- **`Messages<M>` inbox with our drain owning the record tap** (timing vs Bevy's
  `MessageUpdates` GC).
- **`EntityEvent`/observer routing + the upward channel as `observe`** (does the
  enqueue-only rule hold; does it match the prototype-1 routing semantics?).
- **Yew-style `Callback<T>`** as a funnel-dispatch (replay-safe, `#[reflect(ignore)]`).
- **`Reflect` log + `LogicalId`** (cross-process re-fold byte-identical).

**Prototype-2 RESULT (2026-06-26 — DONE):** all five items built & ran in
`examples/mvu_native/` — 10/10 tests green (9 headless + 1 `compile_fail`) + a
**byte-identical cross-process** record→replay + a **real GUI** driving a press through
the full render loop on an RX 6700 XT with no crash. **One correction:** purity
enforcement = a **sealed `PureEnv` allowlist**, NOT `ReadOnlySystemParam` (Commands is
read-only in 0.19 — see the ⚠️ in § 3 and the § 9 table). Full keep/refine/redesign +
the spec edits the final must make: `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`.
Then: structured retrospective (done) → **final** design (supersedes this draft) → plan → impl.

## 14. Provenance

Prototype-1 + journal: `docs/prototypes/2026-06-26-elm-bevyified-state-*.md`.
Audits/research: the state-management prior-art sweep, the Elm/MVU deep-dive, and the
Bevy-leverage audit (run 2026-06-26). Prior-art follow-ups queued (non-blocking):
NEW `docs/prior-art/relm4/` (highest value), refresh `iced/`, capture Elm/Redux
time-travel, optional `gpui/actor-model.md`.
