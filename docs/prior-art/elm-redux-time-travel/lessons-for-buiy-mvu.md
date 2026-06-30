**Date:** 2026-06-26
**Status:** active
**Subject:** KEEP / AVOID decision file — what Elm + Redux + Redux-DevTools time-travel teaches Buiy's core `Reflect`-log + `LogicalId` replay

This is the consult-this-when-designing file. [`README.md`](README.md) is the evidence; this is the synthesis for the **MVU-as-core** spike. Code citations are to the proto-2 runtime in the sibling worktree: `examples/mvu_native/src/runtime.rs` (cited as `runtime.rs:NN`). The proto-2 retrospective and draft spec are in the same worktree under `docs/prototypes/` and `docs/specs/`.

## The one finding that should drive the spec

**Replay is a property of the whole system, not of the log.** It holds iff three things hold at once: (1) the transition fn is pure, (2) *every* input that drives it is in the log, and (3) the initial environment is reconstructed identically. Elm and Redux are 10+ years of evidence that miss-any-one → **silent** divergence. Buiy's core-substrate bet is fundamentally a bet on invariant (2): an opt-in app-boundary log structurally cannot satisfy it (widget-internal state is off-log), a core log can. The spec's job is to make (1) and (3) type-/architecture-enforced and to honestly bound the cost of (2).

---

## KEEP — choices Elm/Redux confirm Buiy got right

1. **Effects-as-data, results fold back AS recorded Msgs.** Buiy's `Cmd::{None, Emit, Batch, task}` (`runtime.rs:68-85`) is Elm's `Cmd`/`Sub` and the disciplined-Elm "Effect pattern" (a custom inspectable enum instead of opaque `Cmd`) at once. The load-bearing rule both systems converge on: **replay the *result* of an effect, never re-run the effect.** Elm's runtime does not re-run `Cmd` on rewind; the effect's result re-enters as a new `Msg` and *that* is what's logged. Buiy already does this for `Emit` (the re-fold runs to completion inside one drain pass, `runtime.rs:323-331`) and the draft spec mandates it for async (`Cmd::task` → `InFlight` poll system enqueues the result as a normal `Envelope` → records + replays). **Keep this invariant explicit and inviolable in the core spec:** effect *application* is gated off in replay mode; only the *fold* re-runs.

2. **Purity enforced by type, not by convention.** Redux's #1 chronic failure is that purity is only *recommended* (principle 3 + a style guide); the ecosystem ships time-travel bugs from impure reducers and mutated state. Buiy's sealed `PureEnv` allowlist (`runtime.rs:184-230`) is strictly stronger — a reducer literally cannot name `Commands`/`ResMut`/`Query<&mut>`/`MessageWriter`, the orphan rule stops anyone blessing them, and the only `&mut` in scope is the model. This is the proto-2 REDESIGN that supersedes `ReadOnlySystemParam` (which leaks because `Commands: ReadOnlySystemParam` in 0.19). **Keep it, and make it core-wide** — it is the single biggest advantage Buiy has over Redux on replay correctness. Pair it with the `#[derive(PureEnv)]` for user env structs (still a residual gap).

3. **Serializable log, closures kept OUT of it.** Both systems break the instant a non-serializable value (function, `Promise`, class instance) enters the recorded stream. Buiy's answer is already correct: the `Reflect` log serializes the `Msg` via `TypedReflectSerializer` + RON (`runtime.rs:127-145`), and the Yew-style `Callback<T>` wiring is stored `#[reflect(ignore)]` so the snapshot skips the `Arc<dyn Fn>` and logs only the *resulting parent Msg*. This is the exact Elm/Redux discipline ("`Msg`/action must be plain data") expressed in Rust. **Keep the hard rule: every `Msg` derives `Reflect`; no closures/futures/live-handles in a `Msg` — only serializable data.** This is also what keeps WASM clean (no new obstacle: RON/serde are already in the tree, no reflection-of-closures).

4. **Stable logical identity, not session identity.** Redux keys its log by action *order* in a single tree; Elm by message order from `init`. Buiy is decomposed, so it needs an identity that survives a fresh process — `LogicalId` (`runtime.rs:94-99`), keyed in the log instead of raw `Entity` (`LoggedEntry { lid, seq, type_path, ron }`, `runtime.rs:102-108`). Proto-2 proved cross-*process* byte-identical re-fold with **no ECS and no Entity ids at all** in the replay process. **Keep `LogicalId` as the log key, and unify it with the agent-interface test-id space** (charter item) so there is one identity space for record, replay, test, and agent-driving — the write-side dual of the existing AccessKit read-tree.

5. **One ordered drain owns the record tap; handlers may only enqueue.** Redux's "single ordered log, changes happen one-by-one in strict order, no race conditions" (principle 2) is Buiy's `MvuSet::Drain` as the sole reader/recorder (`runtime.rs:300-336`) behind the enqueue-only rule (`enqueue`, `runtime.rs:168-178`). Determinism lives at the drain, not the queue. **Keep the single-tap; keep the enqueue-only law** (observers/callbacks/press handlers never fold). This is what makes the log a faithful, totally-ordered record.

6. **Import/export the log as a first-class artifact.** Both systems make the recorded history a *portable JSON file* (Elm's history export; DevTools import/export). This is what turns "time-travel" into "send me your session and I'll replay your bug" and into agent-driving (an agent emits a Msg log). Buiy's RON log is already this shape. **Keep export/import as a v1 core capability, not a someday-feature** — it is the payoff that justifies the core-substrate cost, and it is the substrate for cross-process / MCP / cross-session replay the charter wants.

---

## AVOID — failure modes Elm/Redux paid for; Buiy must not repeat

1. **AVOID believing replay restores the world.** *Severity: high (conceptual ceiling).* Time-travel restores the *model*; it never un-sends a request or un-writes a file. Elm doesn't re-run `Cmd` on rewind; Redux doesn't re-run middleware. **Mitigation for Buiy:** make the effect boundary explicit in the spec — `Cmd::task`/external effects are applied **only when recording**, gated `run_if(in_state(Replay)→false)` (draft spec § 7; proto-2 REFINE #3). Document loudly that replay reproduces UI model state, not external side effects. Never let a reducer touch the world (the `PureEnv` gate already enforces this — this is *why* it matters).

2. **AVOID the unbounded, always-on, always-complete log on the hot path.** *Severity: high — direct hit on the 60 Hz floor / weak-machine / thousands-of-widgets charter risk.* Redux DevTools defaults to `maxAge: 50` and ships `actionSanitizer`/`stateSanitizer` *because* full history leaks past 1 GB and OOM-crashes Android. A *core* Buiy log that `Reflect`-serializes every Msg from every widget every frame is exactly this liability, made worse by per-frame interaction streams (pointer-move, scroll, IME composition). **Mitigations the spec must commit to:**
   - **Bounded ring / `maxAge`-equivalent** for the live log; commit/snapshot to compact.
   - **Opt-out / sampling on hot paths** — high-frequency Msgs (drag deltas, scroll) recordable at reduced fidelity or excluded, with a documented determinism caveat.
   - **Serialize lazily:** record the *typed Msg* in memory; only `Reflect`-serialize to RON at export/snapshot time, not every fold. (Proto-2 serializes eagerly in `record`, `runtime.rs:134-143` — fine for a spike, wrong for the hot path. This is a concrete change for the final.)
   - **hw-independent gates (iai-callgrind)** as the charter demands, so the record cost is measured, not hoped.

3. **AVOID non-serializable / mutation-aliased state sneaking into the log or models.** *Severity: med-high.* This is Redux's most-cited breakage (`{}` snapshots, corrupt history). Buiy is partly protected (Rust types + `Reflect` bound on `Msg`), but two real leak points remain: (a) a `Model` field that is `#[reflect(ignore)]`'d for convenience becomes **off-log state** that silently breaks replay — the decomposed analogue of Redux "state outside the store"; (b) the un-reflected `TextEditState` the charter names is *exactly* this. **Mitigation:** treat any `#[reflect(ignore)]` on a `Model` field (not on `Callback` wiring) as a replay-correctness defect; require widget-internal state (focus/edit/IME/selection/scroll) to be `Reflect` and on-log — that is the whole point of going core. A lint/test that asserts "every `Model` round-trips through `Reflect` losslessly" is cheap insurance (Redux's `serializableCheck` analogue).

4. **AVOID the init-env divergence trap.** *Severity: med.* Elm's history export omits flags → replay in a different-flagged session diverges, silently. Any Buiy reducer env (`Res<Settings>`, theme, locale, clock) read during a fold is an *input* not in the Msg log; if it differs at replay, the fold diverges. **Mitigation:** the draft spec's "deterministic given the *seeded* environment" contract (§ 3) must become a hard mechanism, not a sentence — snapshot the `PureEnv` reads (or the resources they come from) at record-start and restore them at replay-start. Forbid wall-clock/RNG reads in reducers except via a seeded, logged env. This is the one place Buiy can do *better* than Elm (which just punted on flags).

5. **AVOID assuming "single source of truth" properties come free over a decomposed model.** *Severity: med-high — architectural.* Redux gets O(1) "jump to state N" because it caches the *single* whole-state object per action (`computedStates`). Buiy has N per-entity Models and no single object — so jump/scrub is O(total folds) to re-derive from `init`, or O(live Models) to snapshot all of them. **Mitigations to evaluate in the spec (this is an open design question, not a settled one):**
   - **Periodic whole-UI snapshots** (`Reflect`-serialize all live Models at marked seqs) as `computedStates`-equivalent keyframes; jump = restore nearest keyframe + re-fold the tail. This is the Redux `computedStates` cache, adapted.
   - **Decide the granularity of "a frame":** Redux's unit is one action; Buiy's drain runs many folds (incl. `Emit` chains) per tick. The log's `seq` (`runtime.rs:137-140`) is per-fold; a user-facing "step" probably wants per-*drain-pass* or per-*input-event* grouping (cf. DevTools grouping consequent actions). Name this explicitly.
   - Structural ops (spawn/despawn on keyed reconcile) are **not** Msgs in the draft (they live in a reconcile system, draft spec § 5) — so they are *off-log*. For whole-UI replay this is a gap: re-folding the log won't recreate spawned child entities. Either record reconcile as log entries, or derive structure deterministically from on-log parent state. **Flag this as a must-resolve for the core spec.**

6. **AVOID effect/async re-ordering surprises during live time-travel.** *Severity: low-med.* Redux time-travel + effects (thunk/saga, ngrx effects) is a long tail of "it doesn't work" issues because async results re-dispatch live during a jump and interleave. Buiy's `takeLatest` single-in-flight model (draft spec § 6) and the gate-effects-off-in-replay rule mostly sidestep this, **provided** the spec pins that during replay no *new* `task` is spawned (only logged results re-enter). **Mitigation:** make "replay never spawns effects; it only re-feeds logged effect-results" a tested invariant, and pin the flush points (`MvuSet::{Enqueue, Drain, Bind}`, `runtime.rs:154-162`) against `BuiySet` so latency is one designed frame, not emergent (proto-2 REFINE #2).

---

## What the DevTools UX teaches whole-UI time-travel

Redux DevTools' verb set is a ready-made spec for what a *complete* time-travel surface needs — Buiy's agent-interface + verification story should treat these as requirements, not inspiration:

- **jump / scrub** — restore state at any seq. (Buiy: needs the keyframe strategy of AVOID-5.)
- **skip (toggle action)** — recompute *as if a Msg never fired*. Powerful for "is *this* Msg the culprit?" Requires deterministic re-fold from a base — Buiy gets this for free from the pure drain, *if* keyframes exist.
- **pause** — stop recording without stopping the app. (Buiy: the `MsgLog.recording` flag, `runtime.rs:116` + `start()`, is already the hook.)
- **lock** — freeze the app, keep time-travel live. (Buiy: a `bevy_state` replay/lock gate, draft spec § 8.)
- **commit / sweep / reset / revert** — squash history to a new base / drop skipped / wipe to init / undo-to-last-commit. These are *log lifecycle* operations; Buiy's bounded-log strategy (AVOID-2) needs `commit` (snapshot + truncate) anyway, so expose it.
- **dispatch** — inject a custom Msg from the tool. This *is* the agent-driving write-path: an agent/test that emits a Msg into the funnel by `LogicalId` is "DevTools dispatch." The charter's "action lowering through `update`" is literally this verb. Unify them.
- **import / export** — the portable session file. Buiy's RON log; ship it v1.

The lesson: **whole-UI time-travel is not "a slider" — it is a small algebra over the log (jump/skip/pause/lock/commit/dispatch/import/export).** Designing the core log to support that algebra from day one is cheaper than retrofitting it, and it is the same surface the agent-interface and verification harness consume.

## How to use this file

1. Writing the proto-3 **core-MVU spec**: walk the KEEP list to confirm inherited shapes, then the AVOID list — each AVOID names a spec obligation (effect-boundary gate, bounded log, lossless-`Reflect` models, seeded env, keyframe/granularity decision, replay-spawns-no-effects). AVOID-2 and AVOID-5 are the two that can sink the core-substrate bet on *performance* and *decomposed-replay feasibility* respectively — give them dedicated spec sections.
2. Designing the **time-travel / agent-driving surface**: the DevTools verb algebra above is the requirements list; map each verb to a log operation.
3. Promote settled decisions into the proto-3 spec (which supersedes `docs/specs/2026-06-26-buiy-state-management-design.md`); this file captures *what Elm/Redux taught*, not Buiy's own decisions.

## Sources

See [`README.md`](README.md) § Sources. Code: `examples/mvu_native/src/runtime.rs`; proto-2 retrospective `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`; draft spec `docs/specs/2026-06-26-buiy-state-management-design.md`; proto-3 charter `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md`.
</content>
