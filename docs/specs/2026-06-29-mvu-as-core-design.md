# MVU as core — FINAL design

**Date:** 2026-06-29
**Status:** Spec / target-state design — FINAL (production, merge-targeted) `/staged-development` Stage 2. **Revised once after a 3-reviewer gate** (see §0.1).
**Implementation:** LANDED — all functional waves (W0–W7) built + green on `worktree-mvu-core-final` (full headless workspace gate passing); the finishing wave (cleanup/docs) completes it. Merge-gated on human review.
**Base:** `origin/main` @ `4010753` (includes WASM/WebGPU PR #85).
**Worktree:** `mvu-core-final` (branch `worktree-mvu-core-final`).
**Supersedes:** the unmerged draft `2026-06-26-buiy-state-management-design.md` (opt-in-crate placement; written on the `worktree-state-mgmt-elm-prototype` branch, never merged). This spec re-decides placement to **core** on the strength of prototype-3's evidence.
**Seeds:** prototype-3 retrospective (`docs/prototypes/2026-06-26-mvu-as-core-PROTO3-RETROSPECTIVE.md`) + the FINAL research synthesis (`docs/reports/2026-06-29-mvu-as-core-final-research/SYNTHESIS.md` + RD1–RD5).
**App-author DX (downstream, post-landing):** dogfooding this substrate by migrating the demos (a counter, a TodoMVC, the gallery router) surfaced **zero correctness bugs** but a ranked set of **ergonomic gaps** — no preluded MVU surface, no declarative View / keyed-reconcile, no `OnPress→Model` routing, model-can't-be-a-Resource, and (empirically) unproven structural/keyed-list replay — plus the positive find that `fold_one_inline` makes incremental migration tractable. See the [demos → MVU migration journal + retrospective](../reports/2026-06-30-demos-mvu-migration-journal.md) for the value÷risk-ranked recommendations feeding a *potential* core-ergonomics pass.

> **Prototype lineage.** Prototype-3 built the full maximalist bet, ran it, and measured it (`worktree-mvu-core`, DO NOT MERGE). It **proved the central thesis** (byte-identical editor replay via command-sourcing; whole-UI replay of widget-internal state; `set_if_neq` perf cheap) and **tempered the maximalist framing** (machine-tier cost real/modest; AT-seam needs an inline fold; replay is scoped). This spec is the re-decided, hybrid-port production design: **port the validated, redesign the pressure points.**

---

## §0. Decision log

"KEEP" = ported with a re-derived rationale; "REFINE/REDESIGN" = the FINAL does it differently; "NEW" = net-new surface the prototype did not build.

| # | Decision | Disposition | Where |
|---|----------|-------------|-------|
| D1 | Substrate: `Model`/`Cmd`/`Envelope`/`enqueue`/sealed `PureEnv`/single ordered drain in `buiy_core::mvu` | KEEP | §2 |
| D2 | `set_if_neq` drain discipline + `MvuWorkCounters` (the load-bearing perf rule) | KEEP | §2 |
| D3 | Tiered granularity (router-leaf / stateful-leaf=drain-sole-writer / machine=Model+reducer / raw-ECS hatch) | KEEP | §3 |
| D4 | An a11y-projecting machine folds **AND projects (bind)** in an early caller-chosen window before `A11yUpdate` — to **prevent the regression** the migration would otherwise introduce | REFINE (corrected by gate) | §4 |
| D5 | AT seam = **inline mini-drain** via shared `fold_one_inline` (bare-`fn`, env-free) + type-erased `InlineActionRegistry` (cross-entity); corrected contract; Click vs Expand reconciled | REDESIGN | §5 |
| D6 | Editor command-sourcing; editor = PureEnv-exempt routing leaf; editor seeds are seed-scene initial conditions | KEEP | §6 |
| D7 | `RecordMode` default-OFF + lazy; unified `LogicalId`-keyed log; the **scoped** replay guarantee; debug write-outside-the-funnel auditor | KEEP + REFINE | §7 |
| D8 | `Cmd` algebra + keyed **Subscription** minimal shape; bake the Envelope **origin tag** into the v1 log format now | NEW (roadmap impl) | §8 |
| D9 | Generic dismiss-through-the-funnel hook — a **resource registry**, not a per-entity component | NEW | §9 |
| D10 | Single-writer completeness: close `toggle_all_rows`; document the 3 at-spawn seeds + editor `set_value` seeds | MUST-FIX | §10 |
| D11 | L1 perf **go/no-go gate**: a **can-fail** `BlinkLeaf(Tick(now))` fixture + **headless** `node_rebuilds`(+`node_patches`) test; honest framing; do NOT migrate production caret-blink | NEW (gate) | §11 |
| D12 | Supply chain: two iai dev-only advisory ignores + a **separate** triage of the pre-existing `ttf-parser` base failure | MUST-FIX | §12 |
| D13 | WASM-cleanliness (confirmed clean); preserve base cfg-gates | KEEP | §13 |
| D14 | Derived-structure replay scope: DERIVE not record; resolver-rebuild + loud dead-letter; clause conditional on a proving fixture | REFINE | §7.4 |

### §0.1 What the review gate changed (revision provenance)

A fresh-context 3-reviewer panel (soundness / completeness-alignment / adversarial) reviewed v1 against the code. Two reviewers **independently** caught the headline error and it is verified against the source:

1. **D4 was mis-targeted (CONFIRMED ×2 + verified).** v1 moved only the machine *drain* early. But a machine's a11y state is produced by a **separate projection bind** (`bind_menu_model`, late `MvuSet::Bind`), and the a11y tree reads the *projected* `A11yExpanded` (`translate.rs:59-60`), not the model. v1's "same-frame" claim was false, and worse — **the base is same-frame-correct today** (`A11yExpanded` written at `BuiySet::Input`, base `menu.rs:505`), so the migration *introduces* a one-frame `aria-expanded` regression. D4 is rewritten (§4): move the **drain *and* the bind** (and every enqueue producer) into the early window, with a same-frame acceptance test; or accept the lag *explicitly*.
2. **D11 was a tautology + mis-placed (CONFIRMED + verified).** Under `set_if_neq` the steady-frame all-zero gate passes for any idempotent fold *by construction*; a fixture storing the derived phase passes trivially. Rewritten (§11) to fold a per-frame `Tick(now)` (so it *can* fail), to project a **structural** change (so `node_rebuilds==1` on flip is real — a value change takes the Patch path), to run **headless** (not the `#[ignore]` GPU lane — `node_rebuilds` is CPU-side), and to **frame itself honestly** as a substrate-property + routed-cost gate.
3. **D5 menu Click vs Expand divergence; the cross-entity hop; bare-`fn` purity; the Emit mis-attribution** — all folded into §5.
4. **D9 commit to the resource registry** (a boxed-closure component is non-`Reflect`, fouls seed-scene serialization) — §9.
5. **§15 split into two PRs at the machine boundary** — adopted as the recommendation.
6. Peripheral completeness gaps (variadic-reducer-macro roadmap line; editor `set_value` seed; the 3rd at-spawn seed site; two retro framework findings) — folded into §§6/10/15/16.

---

## §1. The bet, re-stated (placement = core, now evidence-backed)

MVU is the **primary state interface** to Buiy: a recordable message substrate in `buiy_core`; widgets route state changes through one ordered funnel; the message log is **complete over the MVU-governed subtree**, which buys **record/replay**, **agent-drive** (AT actions become recorded messages), and the foundation for **hot-reload** (the replay path doubles as the state-preservation path).

**Why core, not an opt-in crate (the re-decide).** An opt-in crate caps the "one message log, N consumers" thesis at the *app boundary*: it cannot record `buiy_core`'s widget-internal state (the editor's caret/selection, a menu's open index, a toggle's value). The prototype's **killer use case** (W4) — a whole-UI session of real input replaying byte-identically *including widget-internal state* — is exactly what an app-boundary log cannot deliver, and it is only reachable if the substrate is core. The dependency direction confirms it: the AT seam (`dispatch_action_request`), the editor, and the leaf state all already live in core; the funnel must live where they are. Placement = core is **earned for the substrate + leaf + editor**, and **conditional-but-justified for the machine tier** once §4 (early window) and §5 (AT seam) amortize its cost.

**What core does NOT mean.** It is not "every widget is an actor" (rejected, §3) and not "the whole world is recorded" (the guarantee is scoped, §7). The purity boundary + the single ordered drain + the record tap stay bespoke; everything else leverages Bevy primitives (`Messages<M>`, change detection, `Reflect`, the schedule).

---

## §2. The substrate (D1, D2 — KEEP)

`buiy_core::mvu` provides:

- **`Model`** — `trait Model: Component<Mutability = Mutable> + Reflect + Clone + PartialEq + GetTypeRegistration { type Msg: …; }`. The `Reflect`/`Clone`/`PartialEq` bounds make the model loggable and `set_if_neq`-comparable.
- **`Cmd<Msg>`** — effects-as-values: `none` / `done` / `batch` / `Emit` (intra-frame, run-to-completion), with `task` + `Subscription` specified in §8 (roadmap impl). `task` folds its result **back through the drain** as a recorded Msg.
- **`Envelope<M>`** — a Msg + its routing identity + (§8) an **origin tag** (`User` / `Command` / `Folded` / `Subscription`).
- **`enqueue`** — the single ingress; observers/handlers/keymaps/AT all enqueue, never call a reducer directly or mutate a model.
- **Sealed `PureEnv`** allowlist — `Res` / read-only `Query` / `Local` / `()` / tuples are blessed; **`Commands` is excluded by the orphan rule** (in Bevy 0.19 `Commands: ReadOnlySystemParam`, so a `ReadOnlySystemParam` bound would let a reducer defer structural mutation and diverge replay — the prototype-2 finding that retired the `ReadOnlySystemParam` approach). A `&Commands` env fails to compile.
- **The single ordered drain** — a *system*, never an observer. It reads `Messages<Envelope<M>>`, folds each onto a clone, and commits via `Mut::set_if_neq`.

**The load-bearing perf rule (D2).** The drain folds onto a clone and commits with `set_if_neq`, so an **idempotent fold ⇒ `models_mutated == 0` ⇒ no `Changed<M>` cascade ⇒ no downstream bind/extract churn**. The real perf risk was never `Reflect`-serialize on the hot path (recording is default-OFF, §7) — it was *the drain defeating change-detection*. `set_if_neq` fixes it by construction. `MvuWorkCounters` (`drain_folds`, `messages_recorded`, `models_mutated`, `binds_fired`, `emits_refolded`) make the property *gate-able* (§11).

**Identity (`LogicalId`).** One author-assignable `LogicalId` layered over the AT `NodeId` (resolver registry; deterministic structural fallback — never `Entity`, never `uuid`/random). The log is keyed by `LogicalId`; `Entity` never appears in it.

---

## §3. Tiered granularity (D3 — KEEP; "every widget an actor" REJECTED)

| Tier | Examples | State | Writer | Replay |
|------|----------|-------|--------|--------|
| **Router leaf** | `Button` | none | — | routes a Msg, no model |
| **Stateful leaf** | `Checkbox`, `Switch` | the existing single-source component (`A11yToggled`) *is* the model; one shared role-keyed reducer; **no per-widget `Model` struct** | the drain, **sole writer** | folds on-log |
| **Machine** | `Menu` (then `Dialog`/`Popover`, roadmap) | a real `Model` + reducer owning multi-field state; **a11y state is a *projection* (bind), not the model** | the drain, sole writer | folds on-log |
| **Composite / raw-ECS** | escape hatch | arbitrary | the widget author | **outside** the boundary (seed-value only) |

The stateful-leaf tier reuses the existing component as the model precisely because `Checkbox::advance` and `Switch::toggle` fold *identically* — one shared `toggle_reducer`. **The leaf/machine distinction is load-bearing for §4:** in the leaf, the model *is* the consumed component; in the machine, the consumed a11y components (`A11yExpanded`, `active_descendant`) are a *projection* of the model written by a separate bind.

---

## §4. The early-window model for an a11y-projecting machine (D4 — REFINE, corrected by the gate)

### 4.1 The leaf/machine asymmetry (why v1 was wrong)

- **Leaf (W6, VALIDATED — port as-is).** The model *is* the consumed component (`A11yToggled` is both the `Model` and what `build_tree`'s `set_toggled` reads). Folding the leaf drain in `ToggleLeafSet::{Enqueue,Drain}` chained `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)` makes the a11y tree fresh **same-frame** — the drain writes the consumed component directly.
- **Machine.** The model (`MenuModel.open`) is **not** the consumed component. `build_tree` reads the button's `A11yExpanded` (`translate.rs:59-60`), which is written by the **projection bind** `bind_menu_model` — and that bind also writes `CssVisibility`, the menu's `active_descendant`, and `FocusedEntity`/`FocusVisible`, **cross-entity** (proto `menu.rs:696-768`). The prototype installs that bind in the late `MvuSet::Bind` (`.after(A11yUpdate)`), so even with the drain early, `build_tree` reads a stale `A11yExpanded`. **Moving only the drain early does not refresh the tree.**

### 4.2 It is a regression to *prevent*, not a wart to fix

In the base (pre-MVU), the menu's open state lives in `A11yExpanded`, written by `advance_expanded_on_press` at `BuiySet::Input` (base `menu.rs:505`) — **before** `A11yUpdate`, so `aria-expanded` is correct **same-frame today**. The MVU migration moves truth into `MenuModel` + a late projection bind, *introducing* the one-frame lag. §4 must **prevent** that regression, not merely mitigate it.

### 4.3 The decision

**An a11y-projecting machine pins its whole early chain — `Enqueue → ApplyDeferred → Drain → Bind` — into the window `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)`** (the drain *and* the projection bind, not the drain alone). Confirmed legal: nothing in `A11yUpdate` writes back into `MenuModel`, and reducers are env-free, so there is no cycle. `Menu` uses this early window.

**Every enqueue producer for the machine must also live in (or before) that window** — `route_menu_press`, `menu_keyboard_nav`, and the §9 dismiss→`Close` — or its Msg folds one frame late once the drain is early (soundness reviewer). The §14 ledger enumerates these.

**The `ApplyDeferred` in the chain must flush observer-sourced `commands.queue` enqueues** (§9's light-dismiss observer fires during `Picking` and enqueues via a deferred command) before the now-earlier drain reads the inbox — a timing edge the prototype never exercised (it validated dismiss with the *late* drain). The acceptance test (§4.4) covers it.

### 4.4 Acceptance tests (mandatory — the footgun is a silent green-gate regression)

The danger is exactly the half-step v1 took: move the drain, forget the bind → a silent one-frame a11y regression that the headless gate passes. So the FINAL adds, on the **keyboard/pointer** path (not just the AT path of §5.7):
- **Same-frame `aria-expanded`:** open the menu via a synthetic press; assert the built a11y tree shows `expanded == true` in the **same** `app.update()`.
- **Same-frame light-dismiss:** click outside; assert the menu is closed (model + projected tree) in the same `app.update()` the dismiss fires.

### 4.5 Caller-chosen, and the honest fallback

The window is the widget author's choice (the REFINE #1 generalization of `add_reducer_in_set`). A machine whose model projects nothing read same-frame may keep the late slot. **If the bind cannot be moved cleanly for some machine, the honest decision is to ACCEPT the one-frame lag and DROP the same-frame claim for it — and say so in its doc, because the base is better than the port on that axis.** The spec does not let an unachieved same-frame claim stand silently.

- **Runner-up (rejected, §17):** "move the drain early" alone — the v1 half-measure; it does not refresh the projected tree and ships a silent regression.

---

## §5. The AT synchronous act-then-observe seam (D5 — REDESIGN; the pivotal fork)

### 5.1 The corrected contract

`perform` calls `dispatch_action_request` then `snapshot` with **no interceding `app.update()`** (`inprocess.rs:387-388`). `snapshot` reads the **cached** `A11yTreeBuilder` views (`inprocess.rs:328-331`), refreshed only by `build_tree` during `A11yUpdate`; `project_node` reads `numeric_value`/`is_expanded` off the consumer tree built over those cached views — **not** the live component. **Only `focus` is live in the immediate snapshot** (`inprocess.rs:336-339`). So a set-verb is **not** "visible in the same `perform()` snapshot" (the v1 over-claim, refuted). The real contract — named at `a11y_inprocess.rs:454-458`, proven by `driver_increment_on_slider_raises_now_by_step` — is **"live-component-synchronous + perform-then-update"**: the *live component* mutates the instant `dispatch_action_request` returns (the test reads it directly, no update); the **snapshot** reflects it after one `app.update()`.

### 5.2 Why inline-fold is still required

The live component must mutate at dispatch-return. An `enqueue → batch-drain` seam defers that to the next update's drain, breaking the live-component-synchronous half (the slider test would fail). So the AT seam folds **inline**, through the *same reducer* the batch drain uses.

### 5.3 The substrate primitive — `fold_one_inline`

```rust
pub fn fold_one_inline<M>(world: &mut World, target: Entity, msg: M::Msg, reducer: fn(&mut M, M::Msg) -> Cmd<M::Msg>) -> bool
where M: Model;
```

**The reducer is a bare `fn` pointer, not an `FnMut` closure** (soundness reviewer): a closure could capture a `Res` snapshot at registration, which diverges on cross-process replay (a fresh registry); a bare `fn` cannot capture, so the seam path is determinism-safe by type. `toggle_reducer`/`menu_reducer` are free fns and already qualify. Body, identical to the batch drain's per-message body except it **bypasses the inbox** (folds exactly the one supplied msg):
1. resolve `LogicalId`;
2. `RecordSession::tick_seq` + `MsgLog.record` — **the AT action becomes a recorded Msg in the shared global sequence (closes L5)**;
3. `get_mut::<M>` → clone → `reducer(&mut next, msg)` → `set_if_neq(next)` (returns `changed`);
4. run the `Cmd` stack **in one record→fold→push-emit loop identical to the batch drain** (each `Emit` cycles back through the record tap), `Emit` pushed to a **local `VecDeque`, run-to-completion inline** (never into `Messages`). Identical record behavior is what makes "one reducer, two call sites, structural single-source" true — the seam and the batch drain must not diverge in what they log or in `MvuWorkCounters`;
5. bump `MvuWorkCounters`.

The batch drain's loop is re-expressed as `while let Some(env) = inbox.next() { fold_one_inline(world, env.target, env.msg, reducer) }` (its env-reading variant fetches `E` via a `SystemState<E>`; **the AT seam uses only this env-free bare-`fn` form**).

### 5.4 The machine-tier set-verb gap — `InlineActionRegistry` (NEW), and its cross-entity hop

Core's generic Expand/Collapse honor (`action.rs:259-271`) writes `A11yExpanded` *directly*; after a machine migration the real state is `MenuModel.open`, so a direct write is then overwritten by `bind_menu_model` from the unchanged model → **"advertised but inert"** (the W5 gap). Core cannot name a `buiy_widgets` reducer by crate direction. **Solution:** a type-erased `InlineActionRegistry` **resource** (mirroring `ReplayRegistry`), populated once by `buiy_widgets`, keyed by role/marker:

```rust
Box<dyn Fn(&mut World, Entity, Action, Option<&ActionData>) -> Option<Result<(), ActionError>> + Send + Sync>
```

The generic Expand honor consults it **before** its default direct write; a registered hook folds inline and returns `Some(Ok)`; an unregistered node falls through to the default.

**The cross-entity hop the spec spells out (adversarial reviewer):** AT `Expand`/`Collapse` target the **`MenuButton`** (it carries `A11yExpanded`), but the model lives on the **`Menu`**. The hook receives `Entity = button`, resolves `button.A11yRelations.controls[0] → menu`, then `fold_one_inline::<MenuModel>(menu, MenuMsg::Open/Close, menu_reducer)` — **a fold on a different entity than was dispatched.** And because AT `Expand` is **absolute** (not a toggle), the hook folds `MenuMsg::Open`/`Close` directly — *not* `MenuMsg::Toggle` (which would wrongly close an already-open menu). If the hook is not registered, the W5 "inert" bug returns; this is load-bearing, not optional.

This registry **is** the agent-interface write-side unification mechanism for the machine tier (H4 signal-2) — small, additive, crate-direction-clean, *not* relocation of machines into core.

### 5.5 Click vs Expand: two AT entry points, reconciled (adversarial reviewer)

A `MenuButton` advertises **two** AT verbs (`action.rs:195,197`): `Click` (Button-role contract) and `Expand`/`Collapse` (state-keyed off `A11yExpanded`). They have **different timing and different semantics**, by design:
- **`Click` = activation (toggle).** Lowers `honor → OnPress → route_menu_press → enqueue MenuMsg::Toggle → batch drain` — the **shared** path that pointer and keyboard activation also take. It is intentionally **enqueue-async** so all three activation modalities converge on one route (the existing `OnPress` convergence). It is *not* live-component-synchronous, and that is correct — activation is the toggle semantic, shared across modalities.
- **`Expand`/`Collapse` = absolute set-verb.** Rides the §5.3 **inline** path (`InlineActionRegistry`), live-component-synchronous, absolute Open/Close.

**§5's synchronous contract therefore covers the *absolute set-verbs* (`Expand`/`SetValue`/`Increment`/`Decrement`), not `Click`-as-activation** — the spec states this precisely rather than implying all AT verbs are inline-synchronous. A screen reader sending *both* `Click` and `Expand` for one logical "open" is APG-non-idiomatic but possible; the result is a double-fold (`Toggle` then absolute `Open`) — documented as the **dual-advertisement caveat**, with the recommended mitigation noted in the plan (route `Click` through the same absolute inline hook, or drop the `Expand` advertisement on the menu — a one-route simplification to decide at execution).

### 5.6 Hard cases (resolved)

- **`Cmd::Emit` at the seam** — runs to completion inside `fold_one_inline` (local `VecDeque`), recorded in the same loop as the batch drain. **The Emit-at-seam machinery is a substrate-generality requirement, but `Menu` is *not* its witness** (correction): the menu's inline hook folds absolute `Open`/`Close` and emits nothing; the `Toggle → Emit(Open)` path (`menu.rs:598-604`) is the `OnPress`/`Click → batch-drain` path, not the inline seam.
- **`Cmd::task` at the seam** — queued exactly as the batch drain would; **never awaited**. Its async result is invisible to `perform`'s immediate snapshot *by nature* (correct), folding later through the inbox at its own seq.
- **Disabled/read-only gate ordering** — the §3 live filter (`action.rs:205-220`) stays **before** the §4 dispatch arm that calls the inline fold; only honored actions are folded and logged. The gate is **never** pushed into the reducer.
- **Same-frame inbox contention** — the inline fold mutates `M` before any queued msgs for `M`; total order is preserved by seq, the inline msg observed first — documented as the **seam sequencing rule**.
- **Counter-reset ordering (soundness reviewer)** — `reset_mvu_counters` (today `.before(Picking)`) must be pinned **before `route_action_requests`**, or the inline fold's counter bumps (it runs in `BuiySet::Input`) are reset away.
- **Double-record hazard** — a verb that both writes `OnPress` and inline-folds would record twice. The leaf/value/expand set-verbs do **not** go through `OnPress`, so there is no overlap; the spec forbids adding one.

### 5.7 The corrected acceptance test (replaces the false one)

A FINAL test (mirroring `menu_machine_w5.rs` + `driver_increment_on_slider_raises_now_by_step`) proving an AT `Expand`/`SetValue` on a migrated machine: (1) mutates the **live model synchronously** at dispatch-return (direct `world().get::<M>()`, no update); (2) is reflected in the **snapshot after one `app.update()`**; (3) **lands a recorded Msg**; (4) **round-trips byte-identically** on replay. Not "visible in the same `perform()` snapshot."

---

## §6. Editor command-sourcing (D6 — KEEP)

The editor is the documented **PureEnv exemption** (impure: `&mut FontSystem` + clipboard) — an imperative routing leaf, determinism guaranteed at the boundary by **recording the resolved `EditCommand`/IME stream and re-folding from a seed**, *never* serializing `cosmic_text::Editor`. The Buiy-owned `Reflect` mirrors (`MotionMirror` 22-variant zero-loss; the IME mirror carrying its `now: Duration`) make the stream loggable. `Paste` records the *resolved* clipboard text. The record tap is a **pure read-tap** at the apply sites — it observes, does not perturb the schedule, adds no reshape-ordering edge.

**Editor seeds are seed-scene initial conditions (completeness reviewer).** Gallery editor buffers seeded via `set_value` *outside* the record tap (`lib.rs:1256,1332`) are **authored initial state**, reproduced by rebuilding replay from the **same seed scene** (§7.3) — they are *not* re-applied as recorded `EditCommand`s. This is the editor analog of the §10 at-spawn toggle seeds: seeds are seed-scene, runtime edits are on-log.

**Standing rule carried from the prototype (retro framework finding):** *every editor-buffer mutator must make its reshape ordering explicit* (the W2 `apply_intents` topo-sort lesson — activating the MVU chain perturbed the executor order and exposed a mutator that violated the `reshape_edited_editors` contract by luck). The record *tap* is exempt (it is a read, not a mutator); a buffer *mutator* is not.

---

## §7. Record / replay (D7, D14)

### 7.1 Default-OFF + lazy
`RecordMode { Off, Ring(n), Full }`, **default `Off`** — production pays zero (record-off vs full: 243µs vs 686µs criterion, 216k vs 788k instr iai). Typed Msgs in a bounded ring; RON only at export.

### 7.2 Unified log
One `RecordSession` (a shared switch + a global monotonic seq); `crate::replay::{unified_stream (merge-by-seq), replay_into}`. `MsgLog` (folds) + `EditLog` (editor commands) share the seq. **Bake the Envelope `origin` tag into the v1 `LoggedEntry` format now** (`mvu/mod.rs:150-156` has no origin field today) so §8's Subscription/task are not an expensive log-format retrofit.

### 7.3 The scoped guarantee (verbatim — the honest statement)

> Buiy records and replays the MVU-governed subtree, not the whole world. With recording on, every message folded through the single ordered drain — widget activation/value/expand folds and the editor's resolved EditCommand/IME stream — is logged against its stable LogicalId in one global sequence. Replaying that log into a fresh app built from the SAME seed scene reproduces every funneled widget-internal state (toggle/value/expand, focus transitions, and the editor's buffer + caret + selection) BYTE-IDENTICALLY. The guarantee is scoped and conditional, not unconditional whole-UI: (a) it covers entities present in the seed; deterministic keyed-reconcile of on-log Model state into derived structure is *targeted but not yet proven* (a roadmap fixture, not exercised in v1 — see §7.4); imperative spawn/despawn performed outside the funnel is off-log and is NOT reconstructed; (b) state written by escape-hatched raw-ECS systems (entities with no Model, direct component writes) is outside the boundary and is reconstructed only to its seed value; (c) replay re-feeds logged effect/subscription results and never re-runs effects or re-subscribes, so nondeterministic input (time, OS clipboard, async results) is reproduced only insofar as it was captured as a logged Msg payload. A debug-build write-outside-the-funnel auditor makes the boundary detectable rather than silent.

### 7.4 Derived-structure replay (D14 — the conditional clause)
Clause (a)'s "deterministic keyed-reconcile of on-log Model state" is the relm4/Druid-validated shape (reconcile by stable **domain id**, never position). It is **asserted-not-proven** in the prototype and **does not work in the ported code as-is**: `replay.rs:149` computes the `LogicalId → Entity` resolver once up-front, so a fold targeting a replay-spawned child silently dead-letters (`replay.rs:152-153`).
**Decision:** (1) **fix the resolver** — rebuild it after each structural change (or resolve live per-entry via a `Query`); (2) **make the dead-letter loud + typed**; (3) **attempt a minimal keyed-list fixture**. The resolver-rebuild + loud dead-letter are correct **regardless** and ship in v1. **Clause (a) is included in the guarantee only if the fixture proves out**; otherwise it downgrades to *"derived structure is targeted, not yet proven."* **W4 outcome (landed):** the resolver-rebuild (live per-iteration resolution) + the typed `DeadLetter` + `warn!` surfacing shipped; the keyed-list fixture was **deferred** to a roadmap follow-up, so clause (a) carries the downgraded "targeted, not yet proven" wording as shipped.

### 7.5 The auditor
A `cfg(debug_assertions)` system that flags a direct write to a `Model`-bearing component from outside the drain *after spawn-settle* (distinguishing it from a spawn-time seed, §10) — making the single-writer boundary *detectable* rather than silently violated (catches the L6 escape-hatch trap loudly).

**Landed (W8).** The drain (`fold_one_with`, shared by the batch drain and the AT seam) stamps a per-entity "last funnel-write tick" into a debug-only `FunnelWriteStamps` resource on every changing fold; the audit is folded into the per-model `count_binds` system's `cfg(debug_assertions)` arm (no new system → no entity-id-snapshot drift; release `count_binds` is byte-identical), which flags any `Changed<Model>` whose `last_changed()` ≠ its stamp — an entity with no stamp is a pre-first-fold seed, not a violation. Violations `warn!` + collect into a debug-only `FunnelAuditLog`. **Entirely `cfg(debug_assertions)`** (off in bench profile → the iai gate's instruction count is untouched; verified `blink_funneled_node_rebuilds_zero` + the mvu crosscut counters are bit-identical). A 4-case test (`auditor_fires_only_on_runtime_violation`: legit fold / legit AT-seam fold / spawn seed / planted runtime violation) proves it fires only on the violation. Scope: the env-free drain + the AT seam (every shipped model); an env-reading reducer is un-audited (a false negative, never a false positive).

---

## §8. Cmd algebra + keyed Subscription (D8 — NEW; minimal shape now, impl roadmap)

The FINAL **specs** (and bakes the log-format hooks for) two additions, **impl deferred to a follow-up phase** (the W4 killer use case needs neither):

- **`Cmd::task`** — async effect; result folds back as a recorded Msg at its own seq. Replay re-feeds the logged result, never re-runs the task.
- **Keyed `Subscription`** (Iced-validated minimal shape) — a stable hash/key; the runtime diffs the active sub-set each frame from the owning Model, starts new keys, drops vanished keys (drop = cancel); every emission flows through the same `enqueue → drain` funnel, logged with its origin tag.

**The one load-bearing invariant (a TESTED invariant, not a doc note): payload-carries-nondeterminism.** Replay never starts a subscription and never re-runs an effect — it re-feeds the logged Msgs they produced; nondeterministic inputs (time, clipboard, async results) are reproduced *solely from the Msg payload* (the `RecordedEdit.now` precedent).

**The v1 trigger condition:** a Subscription becomes **required** the moment a timer/OS/async source drives **Model** state (not pure visuals). Caret-blink is pure render-prep (mutates no `TextEditState` field), so it stays off-log and v1 ships without Subscription. **Before sequencing the Dialog/Popover machines (roadmap), audit them for timer/OS inputs** (auto-dismiss timer, reposition-on-resize) — those pull Subscription earlier.

---

## §9. Dismiss un-invert (D9 — NEW; a resource registry)

`dismiss.rs`'s `close_overlay` writes `CssVisibility::Hidden` directly, role-agnostically. A migrated `Menu`'s visibility must instead be driven by `MenuModel.open` via an enqueued `MenuMsg::Close`. The prototype's stopgap branched on `With<MenuModel>` — coupling core's dismiss to a widget type.

**Decision: a `DismissRegistry` resource** (mirroring §5.4's `InlineActionRegistry` / the `ReplayRegistry`), keyed by overlay marker, populated once by `buiy_widgets`, mapping to a **close-Msg enqueuer**. The generic `close_overlay`:
- for an overlay whose marker is registered → **enqueue the close-Msg** (the §4 early machine drain folds it same-frame; recorded; single-writer);
- for a raw overlay (plain tooltip/popover) with no registration → the existing **direct `CssVisibility::Hidden`** write.

This keeps `dismiss.rs` model-agnostic. **A resource registry, not a per-entity component (adversarial reviewer):** a `Box<dyn Fn>` *component* on a `MenuModel`-bearing entity is **non-`Reflect`**, which fouls the seed-scene serialization replay rebuilds from (§7.3) and is a per-entity allocation; the resource registry is replay-safe infrastructure (never recorded) and entity-free. **Runner-up (rejected, §17):** the `With<MenuModel>` coupling stopgap. **Note:** `InlineActionRegistry` and `DismissRegistry` share the "core consults a widgets-populated, marker-keyed registry of boxed Msg-routers" pattern; a later `FunnelHooks` unification is a logged simplification follow-up, not forced now.

---

## §10. Single-writer completeness (D10 — MUST-FIX)

The prototype's "single-writer proven" headline is contradicted by a surviving runtime multi-writer: `toggle_all_rows` (`lib.rs:1295`, `:1313` `t.0 = next`) writes `A11yToggled` directly at runtime. **Fix:** reroute it to **enqueue `ToggleMsg::Set(next)` per row**.

**At-spawn seeds are a different category** — authored *initial* state (the Elm-flags model-seed), legitimate provided the seed scene replay rebuilds from carries them (it does). Decision: **at-spawn seeds stay as documented authored initial state**; only *runtime* writers reroute. The three sites (completeness reviewer): `lib.rs:1161` (todo row), `:3730` (modal register switch), **`:4887` (showcase switch)** — plus the editor `set_value` seeds (§6). The §7.5 auditor distinguishes seed (spawn-time) from runtime violation.

---

## §11. The L1 perf go/no-go gate (D11 — NEW; rewritten to be can-fail, structural, and headless)

The prototype proved the *substrate-level* no-cascade property but never measured an **end-to-end funnel-routed high-frequency signal** (open-Q11). The FINAL takes the number — and the gate must be able to **fail informatively** (the v1 gate was tautological under `set_if_neq`).

- **Canonical signal — a *can-fail* fixture.** A `BlinkLeaf` bench model folds a per-frame **`Tick(now)` message whose payload changes every frame**, but stores only the **derived blink phase** (a `bool` flipping every ~500ms). `set_if_neq` then absorbs the steady-frame Ticks to `models_mutated == 0`, while a model that wrongly stored `now` directly (or bypassed `set_if_neq`) would tick `== 1` every frame and **fail the gate**. *This is the real measurement: routing a genuinely-per-frame message through the funnel where only the derived state matters.* The fixture's `Tick` is **bench-only** — it is NOT a production primitive and MUST NOT leak into a production caret-blink expectation (the RD2×RD3 scope trap). **Production caret-blink stays render-prep** (`text/visual.rs` edge-gated `write_caret_blink`).
- **The fixture projects a STRUCTURAL change (soundness reviewer).** The phase flip must drive a **visibility/`ComputedPaintSkip`/footprint** change, not a value-only change: a value-only projection (alpha/`Background`) takes the extract **Patch** path (`extract.rs:1474-1524`: `node_rebuilds = 0, node_patches = 1`), so a `node_rebuilds == 1` flip-frame assertion would *fail on a correct fixture*. Either pin the projection to a visibility flip (forces Full rebuild → `node_rebuilds = 1`, `extract.rs:1439/1475`) **or** assert `node_rebuilds + node_patches == 1` on the flip frame.
- **HARD BINARY gate.** On every *steady* (non-flip) frame: `models_mutated == 0 && binds_fired == 0 && node_rebuilds == 0 (&& node_patches == 0)`. On the *flip* frame: the projected counter `== 1`. **Run HEADLESS** on the adapter-free `buiy_bench_support` harness (`extract_buiy_nodes` via `ExtractSchedule`, no wgpu adapter — `buiy_bench_support/src/lib.rs:99,115`); `node_rebuilds` is set CPU-side in `record_node_counts` *before* any GPU work. **This gate is in the DEFAULT CI gate, not the `#[ignore]` GPU lane** (v1 wrongly put it on the GPU lane).
- **SOFT iai ceiling.** One steady blink tick's funnel fixed cost ≤ ~5K instr (≈ 0.03% of the ~16M weak-machine frame budget), via a `mvu_blink_cadence` iai bench — the per-routed-signal fixed cost that bounds how many such signals fit per weak/wasm-single-threaded frame.
- **Honest framing (adversarial reviewer).** This gate de-risks **the substrate's routed-signal no-cascade property + the per-signal cost**, on the can-fail fixture. It does **not** de-risk production timer-driven *Model* state end-to-end (production caret-blink is not migrated; that is §8's Subscription territory). If the gate fails, the funnel cannot govern per-frame-message signals cheaply → the framing narrows to *"the funnel governs input-sourced state; timer/animation signals stay render-prep (caret-blink stays the edge-gated `write_caret_blink`)."* A failed gate is a **successful** outcome — it kills the over-claim with a number; the substrate + leaf + editor + machine value stands regardless.

`cargo deny check` passing (§12) is a prerequisite for the iai bench's green gate.

---

## §12. Supply chain (D12 — MUST-FIX, two separate concerns)

1. **Pre-existing base failure (independent of this work, verified).** The base @4010753 **already** fails `cargo deny check advisories`: an advisory on `ttf-parser` (transitive via `bevy_winit → winit → sctk-adwaita → ab_glyph → owned_ttf_parser`). **Triage in its OWN commit** (a documented `[advisories] ignore` with the no-upstream-fix-without-a-bevy-bump justification, or a `bevy_winit` bump if one exists) — or the deny gate stays red regardless of the port.
2. **The iai dev-only ignores.** `iai-callgrind` (dev-only, never in the prod/wasm graph) pulls unmaintained transitives: add **two** documented `[advisories] ignore` entries — `proc-macro-error2` **and** `bincode 1.3.3` — with the dev-only justification, and **cfg-gate the iai dev-dep out of the wasm graph**. Confirm bans/licenses/sources stay green. (Exact RUSTSEC IDs verified against the live DB at execution; the stanzas carry the justification + a link.)

---

## §13. WASM-cleanliness (D13 — KEEP; confirmed clean)

The 4 MVU files are wasm-clean: zero `thread`/`Instant`/`SystemTime`/`rayon`; primitives all wasm-safe; `now` derives from bevy `Time::elapsed()`. The port introduces **zero new wasm obstacle**. The reconciliation must **preserve the base's wasm cfg-gates** (`text/edit/clipboard.rs`, `text/mod.rs`). `gallery_web` is re-verified after the gallery reconciliation.

---

## §14. Migration ledger (the secondary-reader rewires)

- **Leaf** — readers untouched; writers reroute (incl. `toggle_all_rows`, §10) + the §4 early schedule ripple.
- **Menu (machine)** — deletes `sync_menu_open` + `sync_menu_dismissed` (~187 LOC), projects via the early bind (§4); **fixes the inspector desync** (`inspector.rs:725` hardcodes Menu "open" = "false" — a **headless-invisible** bug needing a **live-interaction test**, not a headless assertion). **Every menu enqueue producer moves into the early window** (soundness reviewer): `route_menu_press`, `menu_keyboard_nav`, dismiss→`Close` — or each folds one frame late. ~20 menu test sites adjust for the early bind path.
- **Editor** — additive; zero rewire.

**Standing cosmetic tax (retro framework finding):** entity-id-bearing layout snapshots re-bless whenever a plugin adds a resource — give those nodes **stable test-ids** so the MVU plugin's addition does not drift the same 3 snapshots. The standing lesson (memory: "always RUN the GUI"): the desync class is only visible by running the GUI / a live-interaction test — every wave runs the gallery.

---

## §15. Scope: TWO PRs, cut at the machine boundary (adopted from the gate)

The v1 single-PR bundle is too big and buries the novel, most-error-prone surface (where v1 was *wrong*) in a large KEEP diff. **Cut at the machine boundary** so PR1's perf gate + replay land and de-risk before the machine redesign:

**PR1 — substrate + measured KEEP work (no machine):**
substrate (D1/D2) · stateful-leaf + early leaf drain (D3/D4-leaf) · editor command-sourcing + seed handling (D6) · unified in-process record/replay + auditor + resolver-rebuild + loud dead-letter (D7/D14) · the **perf go/no-go gate** (D11 — a *leaf* fixture, machine-independent) · `toggle_all_rows` fix (D10) · the **deny triage + iai ignores** (D12) · wasm-clean confirmation (D13).

**PR2 — the machine tier + AT seam (the novel surface):**
Menu machine + the §4 early **drain *and* bind** window + AT seam (`fold_one_inline` + `InlineActionRegistry` + the cross-entity hop + Click/Expand reconciliation, D5) + the dismiss hook (D9) + the migration rewires + the inspector live-interaction test (§14) + the same-frame acceptance tests (§4.4, §5.7).

**ROADMAP (follow-ups, NOT either PR — log-format hooks baked now):**
`Cmd::task` + keyed Subscription impl (D8) · imperative structural-ops-on-log (DERIVE only; even derived-replay clause conditional on the §7.4 fixture) · cross-process serialized `UnifiedLog` · `Dialog`/`Popover` machine migrations · `PureEnv` `Local` + `#[derive(PureEnv)]` · **the variadic-reducer macro** (deferred until the multi-arg reducer surface is exercised) · dead-letter + `catch_unwind` supervision (native-gated) · the `FunnelHooks` registry unification (simplification).

**Ratified at the human gate (2026-06-29): ONE PR.** The user chose a single PR over the two-PR split. The work is still executed in the verifiable internal **waves W0–W8** of the implementation plan (`docs/plans/2026-06-29-mvu-as-core.md`), each RUN + gated, but lands as one reviewed PR. The machine-boundary cut is retained only as the internal wave seam (substrate/leaf/editor/replay/perf land before the machine/AT-seam waves), not as a PR boundary.

---

## §16. Verification strategy (RUN — don't trust green)

Every wave: **run the gallery** (headless green ≠ works — the standing Buiy lesson). The gates:
- **Headless:** full-workspace `cargo nextest` (`CARGO_BUILD_JOBS=4` — the host OOM discipline). **The §11 perf go/no-go runs here (headless), in the default gate.**
- **GPU `#[ignore]` lane (both legs):** `buiy_core` + `buiy_verify` on the real adapter — the Menu anti-clobber precedent (a reconciliation regression headless cannot catch).
- **Live-interaction tier:** real shell + picking + synthetic clicks — for the routed Menu interaction, the inspector desync, and the §4.4 same-frame `aria-expanded`/light-dismiss tests.
- **`cargo deny check`** green (incl. the documented ignores + the base triage).
- **wasm:** a `gallery_web` build.
- **Docs:** `RUSTDOCFLAGS="-D warnings" cargo doc`, `fmt`, `clippy -D warnings`.

---

## §17. Rejected alternatives (named, with the reason)

- **Opt-in `buiy_mvu` crate** (the draft's placement) — caps the log at the app boundary; cannot record widget-internal state; cannot deliver the proven killer use case.
- **"Every widget is an actor"** — ceremony with no payoff; the tiered model captures the value without a `Model` per widget.
- **AT-seam option (a): relocate machines into core** — contradicts machine = widget-author placement, and collapses into the inline-drain anyway for core-typed set-verbs.
- **AT-seam option B1: enqueue-only, defer to the batch drain** — breaks the live-component-synchronous half of the `perform`-time contract (the slider test reads the live component with no `app.update()`).
- **D4 half-measure: move the machine *drain* early, leave the *bind* late** (the v1 error) — does not refresh the projected a11y tree; ships a silent one-frame `aria-expanded` regression with a green headless gate.
- **`ReadOnlySystemParam` for reducer purity** — `Commands: ReadOnlySystemParam` in Bevy 0.19 would permit deferred structural mutation → replay diverges. The sealed `PureEnv` allowlist excludes `Commands`.
- **`FnMut` closure reducers at the AT seam** — a captured `Res` snapshot diverges on cross-process replay; bare-`fn` reducers cannot capture.
- **Reflect-serialize the editor** — `cosmic_text::Editor` is foreign/un-`Reflect`; command-sourcing re-folds byte-identically without it.
- **`RecordMode` default-ON** — production must pay zero; default-OFF + lazy, measured.
- **Recording raw `Entity`-bearing structural ops** — reintroduces the `Entity`-vs-`LogicalId` portability problem; DERIVE composes with the machine tier instead.
- **A value-only (`Background`/alpha) perf fixture** — takes the extract Patch path, so a `node_rebuilds==1` flip assertion fails on a correct fixture; the fixture must project a structural change (§11).
- **The perf gate on the `#[ignore]` GPU lane** — keeps the go/no-go out of the default CI gate; `node_rebuilds` is CPU-side, so it runs headless (§11).
- **A per-entity boxed-closure dismiss component** — non-`Reflect`, fouls seed-scene serialization; use the resource registry (§9).
- **`With<MenuModel>` dismiss coupling** — couples core to a widget type; does not generalize.

---

## §18. Provenance

- Research synthesis + RD1–RD5: `docs/reports/2026-06-29-mvu-as-core-final-research/`.
- The 3-reviewer spec gate (soundness / completeness / adversarial): summarized in §0.1; transcripts in the session.
- Prototype retrospective / journal / design: `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-*.md` (unmerged reference, `worktree-mvu-core`).
- Prototype research synthesis (D1–D10): `docs/reports/2026-06-26-mvu-as-core-research/SYNTHESIS.md`.
- Memory: `buiy-state-mgmt-design`.
