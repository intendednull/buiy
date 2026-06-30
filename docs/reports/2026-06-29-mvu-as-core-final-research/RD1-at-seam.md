# RD1 — AT synchronous act-then-observe seam

**Decision: adopt (b) inline mini-drain.** Extract the batch drain's per-message
body into one shared `fold_one_inline` primitive that both the batch drain AND
`dispatch_action_request` call. Reject (a) funnel-in-core / relocate-machine —
it contradicts machine = widget-author placement and collapses into (b) for the
core-typed set-verbs anyway.

Confidence: **high** on the directional choice. One load-bearing *justification*
in the original finding is **factually wrong** and is corrected below (the
adversarial refuter caught it; verified first-hand against code + existing tests).

---

## The correction the spec MUST absorb (refuter-confirmed, verified here)

The original finding claimed `(b)` "restores **synchronous read-back**" such that
"a slider AT Increment's new `A11yValue` is **visible in the SAME `perform()`
snapshot**." **That is false for every `build_tree`-projected field** (value,
expand, text). Only `focus` is genuinely synchronous in the snapshot.

Verified mechanism:

- `perform` calls `dispatch_action_request` then `snapshot` with **no interceding
  `app.update()`** — `crates/buiy_core/src/a11y/inprocess.rs:387-388`.
- `snapshot` reads the **CACHED** `A11yTreeBuilder` views
  (`inprocess.rs:328-331`), feeds them through `consume` →
  `build_tree_update` (`inprocess.rs:340`, `:229-233`), and `project_node` reads
  `numeric_value`/`is_expanded` off that consumer tree (`inprocess.rs:259`,
  `:241-276`) — **NOT off the live component**.
- The cache is refreshed only by `build_tree` during `BuiySet::A11yUpdate`, i.e.
  inside `app.update()`.
- `focus` is the one exception: re-read live from `FocusedEntity` at
  `inprocess.rs:336-339`.

The existing test proves it: `driver_increment_on_slider_raises_now_by_step`
(`crates/buiy_core/tests/crosscut/a11y_inprocess.rs:462-501`) asserts the **live**
`A11yValue.now == 35.0` *synchronously* right after `increment` (lines 489-494,
direct `world().get::<A11yValue>`) **but requires `app.update()` (line 495)**
before the `snapshot` `numeric_value == Some(35.0)` assertion (lines 496-500). The
module doc names this the **"perform-then-update contract"** (`a11y_inprocess.rs:454-458`).

### What this changes, and what it does NOT

- It does **NOT** overturn `(b)`. There IS a genuine synchronous requirement: the
  **live component** must be mutated the instant `dispatch_action_request` returns
  (the slider test reads the live `A11yValue` with no update). An
  enqueue→batch-drain seam would **defer** that live mutation to the next
  update's drain — breaking the contract. `fold_one_inline` mutates the live
  component synchronously via `set_if_neq` and also records the Msg (closes L5).
  So inline-drain remains correct.
- The spec must (1) replace the false "synchronous snapshot read-back" rationale
  with the real **"perform-then-update + live-component-synchronous"** contract,
  and (2) rewrite the prescribed acceptance test: assert the **live** `A11yValue`
  synchronously, and the **snapshot** value **after** an `app.update()` — mirroring
  `driver_increment_on_slider_raises_now_by_step`, NOT in the same `perform()` snapshot.

The refuter found **no** divergence in replay, no incorrect cross-entity/async Cmd
at the seam, and no gate-ordering fault — only this stale-snapshot mislabel.

---

## The substrate primitive

Extract the drain's per-message loop (prototype `crates/buiy_core/src/mvu/mod.rs:607-653`)
into a free fn called by BOTH the batch drain and the seam — one reducer, two call
sites at the fold-step grain. Env-free form (the only form the AT/keyboard reducers
need; `toggle_reducer` `leaf.rs:90`, `menu_reducer` `menu.rs:586` are both
`FnMut(&mut M, M::Msg) -> Cmd`):

```rust
pub fn fold_one_inline<M, F>(world: &mut World, target: Entity, msg: M::Msg, reducer: &mut F) -> bool
where M: Model, F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg> + Send + Sync + 'static
```

Returns `changed` (the `set_if_neq` result). Body identical to the drain:

1. resolve `LogicalId` (`mod.rs:608`);
2. `RecordSession::tick_seq` + `MsgLog.record` so the AT action becomes a recorded
   Msg (`mod.rs:613-618` — **closes L5**);
3. `get_mut::<M>` → clone → `reducer.fold(&mut next)` → `set_if_neq(next)`
   (`mod.rs:628-630`);
4. run the Cmd stack with `Emit` pushed to a **LOCAL** `VecDeque` run-to-completion
   inline, NOT into `Messages` (`mod.rs:635-644` — essential for menu
   `Toggle → Emit(Open)`);
5. bump `MvuWorkCounters`.

**Critical departure from the drain:** it does NOT read `Messages<Envelope<M>>`
(inbox bypass) — it folds exactly the one supplied msg. The drain's own loop body
is then re-expressed as `while let Some(env) = …{ fold_one_inline_with_env(…) }`,
proving structural single-source. Env-reading reducers get a sibling that fetches
`E` via a `SystemState<E>`, but the AT seam keeps its reducers env-free (see
determinism risk).

---

## Replay determinism

An AT-originated Msg recorded at the seam re-folds identically because (1) it is
recorded with the **shared global seq** via `RecordSession::tick_seq`
(`mod.rs:230`), so `replay::merge` orders it correctly relative to
keyboard/editor msgs; (2) replay re-enqueues it through the `ReplayRegistry`
applier (`mod.rs:527-545`) into `Messages<Envelope<M>>`, where the normal batch
drain folds it — a pure env-free reducer applied to the same prior model state
yields the same result regardless of inline-vs-drain timing.

**The risk:** record-time the seam folds inline at a specific mid-frame point
(inside `BuiySet::A11yUpdate`/Input where `dispatch_action_request` runs),
replay-time it folds at `MvuSet::Drain`. Timings differ, so any reducer reading
**world state other than its own model** (an env read) can diverge.
**Mitigation:** AT-seam reducers must be **env-free** (toggle/menu already are). A
`Cmd::Emit` chain is safe because it runs to completion within the single
`fold_one_inline` call at both record and replay.

**Second risk:** if other msgs for the same `M` are pending in the inbox the same
frame, the inline fold mutates `M` before the batch drain processes them — total
order is still preserved by seq, but the inline msg is observed first. Document
this as the seam's **sequencing rule**, not a silent surprise.

---

## Hard cases

1. **`Cmd::Emit` at the seam during a synchronous read-back** — MUST run to
   completion inside `fold_one_inline` (local `VecDeque`, `mod.rs:635-644`). Menu
   `Toggle → Emit(Open)` (`menu_reducer` `menu.rs:598-604`) is exactly this.
2. **`Cmd::task` at the seam** — an async task can NOT complete synchronously, so
   its result is invisible to `perform`'s snapshot **by nature** (correct). The
   inline fold must spawn/queue the task exactly as the batch drain would; its
   completion Msg folds later through the normal inbox path and is recorded there
   at its own seq. **Never await at the seam.**
3. **Disabled/read-only gate ordering** — the §3 live filter
   (`crates/buiy_core/src/a11y/action.rs:205-220`) runs BEFORE dispatch today and
   MUST stay before `fold_one_inline` — fold only on gate-pass. Naturally preserved
   because the gate is in `dispatch_action_request` and `fold_one_inline` is called
   from the §4 dispatch arm (`action.rs:222+`). **Do NOT push the gate into the
   reducer** (reducers are pure/env-free; replay re-folds without live
   `A11yDisabled`/`A11yReadOnly`) — keep gating at the seam so the log holds only
   honored actions.

---

## L5 / H4 closure — precise boundary

- **L5** (AT actions become recorded Msgs): **CLOSED** for every core-typed model
  via the seam's `tick_seq` + `MsgLog.record` (leaf `A11yToggled`, value
  `A11yValue` once given a reducer, `A11yExpanded`). For the machine tier, AT
  Click is **already** recorded today (converges on `OnPress` →
  `route_menu_press` enqueues `MenuMsg::Toggle` → drain folds + records; proven by
  `menu_machine_w5.rs`).
- **H4 signal-2** (write-side unification — one reducer for both AT and batch
  paths): **ACHIEVED** by the shared `fold_one_inline` for core-typed models.
- **RESIDUAL not closed by (b) alone:** machine-tier **absolute** set-verbs honored
  generically in core (AT Expand/Collapse, `action.rs:259-271`) cannot reach a
  `buiy_widgets` reducer by crate direction — the W5 "advertised but inert" gap.
  Closing it needs a type-erased **`InlineActionRegistry`** resource (mirror
  `ReplayRegistry` `mod.rs:287`) populated by `buiy_widgets`: a per-role/per-marker
  boxed `Fn(&mut World, Entity, Action, Option<&ActionData>) -> Option<Result<(), ActionError>>`
  the generic Expand honor consults BEFORE its default direct `A11yExpanded`
  write, returning `Some` when it folded inline (via `fold_one_inline::<MenuModel>`).
  This registry IS the agent-interface write-side unification mechanism for the
  machine tier; it is small + additive, NOT option (a)'s relocation.

---

## Residual open-for-spec

- **Env-determinism invariant is unenforced.** Today only a doc note. The spec
  needs a compile-time or tested guard that the seam path uses the env-free
  `fold_one_inline` form.
- **Rewrite the acceptance test** per the correction above: live-synchronous +
  snapshot-after-update, NOT same-snapshot.
- **The `InlineActionRegistry` hook design** (the W5 machine-tier set-verb gap) is
  net-new surface to spec.

---

## Risks (carried forward)

- Env-reading reducer at the seam diverges on replay — mitigate: env-free only.
- Same-frame inbox contention: inline seam fold observed before queued msgs;
  document as the sequencing rule.
- `fold_one_inline` takes the reducer as an argument, so the honor fn must name the
  concrete reducer (fine for core; widgets machines store the boxed reducer in
  `InlineActionRegistry`).
- Leaving AT Expand/Collapse writing `A11yExpanded` directly after a model
  migration produces "advertised but inert" — the registry hook is REQUIRED, or
  un-advertise the verb on migrated machines.
- `Cmd::task` must queue, never run inline (would block the frame, break
  effects-as-values purity).
- Double-record hazard: a verb that BOTH writes `OnPress` and inline-folds must
  record at exactly one site. The leaf/value/expand set-verbs do NOT go through
  `OnPress`, so no overlap today.

## Key evidence

- `inprocess.rs:387-388` — `perform` = dispatch then snapshot, no `app.update()`.
- `inprocess.rs:328-331`, `:340`, `:259` — snapshot reads CACHED views via consumer
  tree (the staleness).
- `inprocess.rs:336-339` — `focus` is the one live-read field.
- `a11y_inprocess.rs:462-501` — the perform-then-update contract, live-synchronous
  component + snapshot-after-update.
- `action.rs:259-271` (Expand), `contract.rs:384-405` (Slider), `contract.rs:483-493`
  (text SetValue) — direct synchronous set-verb mutations.
- `action.rs:205-220` — §3 gate before §4 dispatch.
- prototype `mvu/mod.rs:607-653` — the loop body to extract; `:613-618` — record
  tap; `:287-297`/`:527-549` — `ReplayRegistry` pattern for the registry hook.
- prototype `menu.rs:598-604` — `Toggle → Emit(Open)` requiring inline Emit.
