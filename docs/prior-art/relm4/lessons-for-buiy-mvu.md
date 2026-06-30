**Date:** 2026-06-26
**Status:** active
**Subject:** Relm4 — KEEP / AVOID / Borrow decision file for Buiy's MVU-as-core (proto-3) state-management re-decision.

This is the consult-this-when-designing decision file. [`README.md`](README.md) is the evidence (Relm4's component/message/effect model in depth); this file is the synthesis aimed at Buiy's specific open decisions. Read it against the proto-3 charter's "what to RE-DECIDE" list and "hard questions / risks."

---

## The headline (the single most important finding)

**Relm4 is the strongest production evidence that Elm-style per-component MVU works in Rust — AND the strongest evidence that the record/replay headline Buiy is chasing is genuinely novel, because Relm4, the closest analog, deliberately does not achieve it.**

Relm4 has Model + typed `Input`/`Output` messages + `update` + child/parent message-passing composition + typed effects. It has shipped these for five years across real apps. So the *ergonomic* shape Buiy's proto-3 wants is de-risked: it is known to work, known to scale, known to be maintainable.

But Relm4 has **no message log, no time-travel, no replay** — and the reasons are structural, not missing-feature:

1. Messages are ephemeral per-component channel sends; nothing taps a global ordered stream.
2. The view stores **live GTK widget handles inside the component**; state is not pure data.
3. `update`/`update_cmd` are side-effecting and return nothing; effects are *performed* mid-update via `sender`, not *returned as values* to be intercepted.
4. Commands are unrecorded background side effects.

Buiy's thesis inverts all four: **one ordered drain owns the global `Reflect` log; state is data not widget handles; the reducer is pure and returns `Cmd` as values; effect results fold back as recorded `Msg`s.** That inversion is *the entire justification for MVU-as-core* (vs. opt-in). Relm4 proves you cannot get replay by adopting Elm ergonomics alone — you must architect for it, core-deep, which is exactly what the charter argues. **Relm4 de-risks the ergonomics and confirms the novelty of the payoff.**

---

## The granularity decision (charter's hardest open question)

> *"Does every widget become a `Model` + reducer, or do leaf widgets stay imperative and only route?"*

**Recommendation: COARSE. Leaf widgets stay plain ECS + route only; `Model` + reducer is reserved for stateful units. Make actor-hood opt-in per widget-type, not the universal default.**

**Rationale (Relm4 evidence).** Relm4 is the actor-per-widget poster child, and it puts an actor on *nothing* below a stateful unit. A `gtk::Button` / `gtk::Label` / `gtk::Entry` inside `view!` is **not** a component — no `Model`, no mailbox, no `update`. The component boundary carries an async runtime task + three channels (`input`/`output`/`command`) + a `Controller`; that weight is multiplied by *tens-to-hundreds* of components in a real app, never thousands. The factory (1000 list items) is the explicit stress case, and even there each item is a *FactoryComponent* (lighter than a full `Component`) and the perf concern is the *widget-update diff*, not the message machinery. Relm4's whole "Efficient UI updates" chapter exists because naive per-element work is too slow — and their fix is **tracker + factory diffing, not more actors.**

This collides directly with the charter's PERFORMANCE + SCALE risks: a per-leaf mailbox + `Reflect`-serialize on the hot path, multiplied by thousands of widgets, against a 60 Hz floor on weak machines, is precisely the cost Relm4's architecture is shaped to avoid. "Every widget is a `Model`" would be a scale mistake by Relm4's lived evidence.

**Runner-up (rejected): uniform actor-per-widget.** Conceptually clean (one model, no two-tier rule) and maximizes the "complete log" (every leaf's internal state in the funnel). Rejected because the per-actor mailbox + record cost does not survive the thousands-of-widgets × 60 Hz constraint, and Relm4 — which had every reason to try it — chose coarse instead. **Mitigation that preserves the charter's "complete log" goal at coarse granularity:** keep the *widget-internal* state Buiy actually needs in the log (focus, text-edit buffer, selection, IME, scroll — the `TextEditState` crux) as `Reflect` components owned by a *small set* of editor/focus actors, not by promoting every button to an actor. The log is complete because the *stateful* surfaces are actors, not because *every* surface is.

---

## KEEP — Buiy proto-2/draft decisions Relm4 confirms

- **Child→parent mapping localized to the connection edge (Buiy's `Callback<T>` / proto-2 `callback.rs`).** Relm4's `.forward(parent.input_sender(), |child_out| ParentInput::…)` is the same move: the child declares an `Output` type; the parent supplies the map *once, at launch*, not threaded through the view (Iced's per-node `Element::map`). KEEP Buiy's `callback::<T,M>(parent, map)` — it is the validated shape, at finer granularity (per-event sink vs one `Output` enum).
- **Keyed reconcile by domain id, not position (Buiy § 5 "keyed reconcile by DOMAIN id, not Entity/position").** Relm4's `DynamicIndex` exists for exactly the bug Buiy is avoiding: a deferred message carrying a `usize` index points to the *wrong* element after a reorder. "If you used `usize` … the index points to another element by the time it is processed." Buiy's entity-identity sidesteps it natively. KEEP with high confidence — this is independently arrived-at, production-confirmed.
- **Structural ops batched and applied at a sync point, never inside `update` (Buiy § 5 "structural ops live in a reconcile system, never inside `update`").** Relm4's factory `guard()` is an RAII batch that syncs widgets on drop; mutation never happens mid-`update`. Buiy's ECS `Commands` (deferred) + reconcile system is the same discipline. KEEP.
- **Pure reducer returning effects-as-values, NOT an async/side-effecting update.** Relm4's `AsyncComponent` (await inside `update`) is a documented footgun — it blocks the component's whole mailbox and, for Buiy, would be non-deterministic + unrecordable. Relm4's blessed path is **Commands** (separate runtime, results via `CommandOutput`), which *is* Buiy's `Cmd::task` + poll-fold-back model. KEEP Buiy's `update -> Cmd` purity; it is the correct half of Relm4's own dichotomy.
- **Cancellation = drop-on-teardown (Buiy § 6 "cancellation is free on despawn (drop = cancel)").** Relm4 binds command futures to the component lifetime via `ShutdownReceiver`; shutdown drops the future. Buiy's `InFlight`-on-entity + despawn=drop=cancel is the same mechanism. KEEP; note Buiy's *takeLatest* (one in-flight per model) is a refinement Relm4 leaves to the author (`oneshot_command` can stack) — Buiy's stricter default is a feature.
- **Change-detection-gated binds over full-view recompute (Buiy's `bind` = `Changed<M>` + `set_if_neq`).** Relm4's whole "efficient UI" answer to the 1000-counters problem is `#[tracker::track]`: a dirty-bit per field, `set_field` marks only on change, the view guards a setter with `#[track = "model.changed(...)"]`, `reset()` each cycle. That is *exactly* `Changed<T>` + `set_if_neq` — which Bevy gives Buiy **for free**. KEEP `bind`; see Borrow #5 for why this is a Buiy *advantage* to name explicitly.
- **A loud, typed dead-letter on unhandled dispatch (Buiy § 5 "unhandled = loud typed dead-letter").** Relm4's `sender.output()` returns `Err(msg)` when all receivers were dropped — a channel-level dead-letter. Confirms Buiy's instinct that an unroutable message must be *surfaced*, not silently dropped. KEEP, and make it loud (the charter's "dead-letter (loud, typed)").

## AVOID — Relm4 pitfalls Buiy must not reproduce

| Pitfall | Source (Relm4) | Buiy mitigation |
|---|---|---|
| **Promoting leaf widgets to actors.** A per-component mailbox + runtime + channels multiplied by thousands of widgets is the scale wall. Relm4 never does it; its "efficient UI" chapter and `FactoryComponent`-is-lighter-than-`Component` split exist precisely to keep per-element cost down. | README § "The granularity verdict"; Book — Efficient UI updates. | Coarse granularity (above). Leaves = plain ECS components + routing. `Reflect`-serialize/record **opt-out or sampled on hot paths**; hw-independent gates (iai-callgrind) per the perf campaign. Make actor-hood a per-widget-type opt-in, not universal. |
| **Treating `forward()` as zero-cost composition / overstating "no `Msg.map`."** Relm4 *relocates* the map to the edge; it does not eliminate it. Deep nesting chains forwards (child→mid→parent each maps). The charter's "compose WITHOUT Elm's `Msg.map` boilerplate" is only *partially* delivered by this pattern — O(edges), not zero. | README § "Parent ↔ child composition." | Keep Buiy's `EntityEvent` **auto-bubbling** for the common "route an event up to the nearest ancestor that owns the `Msg`" case — *no per-edge map at all*, strictly better than Relm4 there. Reserve explicit typed `Callback`/mapping for cross-tree or payload-transforming edges. Do not market away the per-edge cost that remains for those. |
| **A bespoke special-case API for collection→parent routing.** Relm4 *had* `ParentInput` + `forward_to_parent()` on `FactoryComponent` and **removed them** (0.6→0.7), unifying on the same `builder().launch().forward()` as any component. A second routing mechanism was net-negative. | Book — migrations 0.6→0.7; README § "Factories." | Route factory/keyed-reconcile children through the **same** `Callback`/`EntityEvent` path as any child. Buiy's draft already does (reconcile emits normal `Msg`s) — hold that line; resist a special "list child" dispatch API. |
| **Async-in-`update` (`AsyncComponent`).** Awaiting inline blocks the component's mailbox and is non-deterministic — fatal to recording/replay. | README § "AsyncComponent — and why it is a footgun." | Never `await` in the drain/reducer. Only the Commands model: pure `update -> Cmd::task`; a poll system folds the result back **as a recorded `Msg`**. (Buiy's plan — confirmed correct.) |
| **State that is widget handles, not data.** Relm4's `Self::Widgets` stores live GTK objects mutated by side-effecting calls; this is the root reason replay is impossible (you cannot re-fold a GTK tree from `init`). | README § "Why Relm4 cannot replay." | Keep Buiy models as **`Reflect` data only** — *no* render handles, *no* GPU resources, *no* `Entity` ids that aren't `LogicalId`-mapped, in the model. The view is *derived from data by ECS systems*, never stored in the model. This is the non-negotiable that makes the `Reflect`-log + re-fold work. |
| **Effects performed as side effects mid-`update` (returning nothing).** Relm4's `update` issues sends/spawns through `sender` as it runs; there is no value boundary to intercept and record. | README § "The component model in depth." | Keep `update -> Cmd` (effects *returned as values*); the drain is the single place that interprets `Cmd`s and records. The pure boundary (sealed `PureEnv` — proto-2 REDESIGN #1) is what makes this enforceable. |

## Borrow — Relm4 primitives worth studying/adapting

1. **The single `Output` enum per component, as a *complement* to per-event `Callback`s.** Relm4's child declares one `Output` enum; the parent writes one `match` with one arm per variant — ergonomic for a widget that emits *many* events (one map site, one type, one match). Buiy's per-event `Callback<T>` is better for a *fine* typed surface but becomes N closure params for an N-event widget. **Consider supporting both:** typed callbacks for fine control + an optional "emit `Out` enum, parent observes once" desugaring (the draft already mentions `Callback` "desugars to the lower-level `Out`-type + entity-`observe` translator" — Relm4 confirms the `Out`-enum tier is worth exposing directly, not just as an internal desugar).
2. **`CommandOutput` as a *typed-apart* effect-result channel — adapt, don't copy.** Relm4 separates user intent (`Input`→`update`) from effect results (`CommandOutput`→`update_cmd`). Buiy *unifies* them (results fold back as `Msg`s) — correct for a single replay log. But Relm4's separation buys *clarity* ("this message is a network result, not a user click"). **Recover that clarity cheaply:** tag the recorded `Envelope` with an `origin` marker (`User` / `Command` / `Folded`) so the log is self-describing without splitting the message *type* or the reducer. One log, annotated — keeps replay simple, keeps the debugging signal Relm4 gets from the type split.
3. **`DynamicIndex` as a concept for any deferred reference to a moving element.** Beyond keyed reconcile: anywhere Buiy passes a reference to a collection element across a frame boundary (a `Cmd` result targeting "the 3rd item," an agent action addressing a row), use the *stable domain id / `LogicalId`*, never a snapshot position. Relm4 generalized this into a first-class index type — worth a Buiy equivalent if positional addressing ever creeps in.
4. **`ShutdownReceiver` — an explicit cancellation token bound to actor lifetime.** Buiy's "drop = cancel" via despawn is implicit; Relm4's explicit `ShutdownReceiver` lets a *long-running* command observe cancellation cooperatively mid-stream (not just at drop). For Buiy's `Cmd::stream`/long-poll cases (deferred in the draft), an explicit cancel signal threaded into the future — not just drop — is the cleaner shape. Borrow when `stream` lands.
5. **`#[tracker::track]` as the thing Buiy gets for FREE — name it as an advantage.** Relm4 had to build the entire `tracker` crate (dirty bits, `changed()`, `reset()`) to do field-level change detection without a vdom. Buiy's `Changed<T>` + `set_if_neq` *is* that machinery, native to Bevy, zero new code. When writing the proto-3 spec's `bind`/derived-view section, **state explicitly** that Buiy inherits Relm4's hardest-won efficiency primitive from the ECS substrate — it is a concrete reason MVU-on-ECS is a better host for this pattern than MVU-on-GTK.
6. **`Worker`-on-its-own-thread as a distinct "headless actor" tier.** Relm4 separates `Worker` (logic + state, no view, optionally own thread) from `Component` (has view). Buiy's models-without-rendered-widgets (a sync engine, a router, a clock) are the same shape. Consider whether Buiy wants a named "headless model" convention (a model with no `#[require(Node)]`) so the spec/macro can treat view-less actors first-class, rather than as an afterthought.
7. **`MessageBroker` for global cross-tree dispatch.** Relm4 ships a typed global broadcast point for messages that don't fit parent↔child. Buiy's "explicit-address dispatch is the cross-tree escape hatch" is the same need; Relm4's `MessageBroker` is a reference for what a *typed, opt-in* global channel looks like (vs. an untyped god-bus). Decide explicitly whether Buiy needs one or whether `LogicalId`-addressed enqueue covers it. (Open question below.)

## Decisions for the proto-3 spec (recommendation · rationale · runner-up)

1. **Widget granularity → COARSE (Model+reducer for stateful units; leaves route only).** *Rationale:* the actor-per-widget poster child does this, for scale reasons that map onto Buiy's 60 Hz / thousands-of-widgets risk. *Runner-up:* uniform actor-per-widget — rejected on perf + the per-leaf `Reflect` cost Relm4 avoids.
2. **Child→parent → keep typed `Callback`s, ADD an optional per-widget `Out` enum tier.** *Rationale:* `Callback` wins for fine surfaces; Relm4's single-`Output` enum wins for many-event widgets (one match arm). Offer both. *Runner-up:* callbacks-only — loses the one-match ergonomics.
3. **Effect-result typing → UNIFIED log (fold back as `Msg`), but tag `Envelope.origin`.** *Rationale:* one log is required for byte-identical replay; Relm4's `update`/`update_cmd` split doubles surface and isn't replay-shaped. The origin tag recovers Relm4's clarity benefit. *Runner-up:* separate `CommandOutput` type — rejected for replay; clarity recoverable via the tag.
4. **Async → Commands model only (`Cmd::task` + poll-fold-back); NO async reducer.** *Rationale:* Relm4's `AsyncComponent` blocks its mailbox and is non-deterministic; its own guidance prefers Commands. *Runner-up:* async reducer — rejected (unrecordable, blocks the drain, breaks purity).
5. **Collection identity → domain-id / `LogicalId` keyed reconcile.** *Rationale:* `DynamicIndex` is Relm4's documented fix for the exact stale-position bug. *Runner-up:* positional/`usize` index — rejected (Relm4's documented bug).
6. **Up-routing → keep `EntityEvent` auto-bubbling; `Callback` for cross-tree/transform.** *Rationale:* bubbling beats Relm4's per-edge `forward()` for the common case (zero map vs O(edges)). *Runner-up:* mandatory per-edge `forward()` — rejected (reintroduces mapping the bubbling removes).

## Open questions Relm4 surfaces for Buiy

- **Does Buiy need a `MessageBroker`-style typed global dispatch, or does `LogicalId`-addressed enqueue + `EntityEvent` bubbling cover every cross-tree case?** Relm4 found it needed a global broker on top of tree routing. Decide before the spec locks composition.
- **Should Buiy have a distinct "list-item actor" tier cheaper than a full reducer-model?** Relm4 made `FactoryComponent` lighter than `Component` for exactly the thousands-of-items case. If Buiy's keyed-reconcile children are common + numerous, a lighter per-item model (fewer channels, batched record) may be needed to hold the 60 Hz floor.
- **Where exactly is the `Reflect`-record opt-out boundary on hot paths?** Relm4 avoids the cost by not making leaves actors at all; Buiy keeps some stateful leaves as actors (the `TextEditState` crux). Per-message recording of high-frequency streams (pointer-move, IME-preedit, scroll) needs an explicit sampling/opt-out rule, or it breaches the perf floor. Name it in the spec.
- **Does the coarse-granularity default leave any widget-internal state *outside* the log** (the original crux)? Audit which `buiy_core` states (focus, edit buffer, selection, IME, scroll) must be actor-owned `Reflect` components to keep the log "complete," and confirm that set is small enough to stay coarse.

## How to use this file

1. Find the **Decision** above closest to what you are speccing; take the recommendation + its runner-up into the proto-3 spec.
2. For a pitfall, read the **AVOID** row and follow the source link in [`README.md`](README.md) for the original Relm4 incident.
3. For a primitive, read the **Borrow** entry and adapt for Buiy's ECS-native, `Reflect`-logged, pure-reducer model — never copy Relm4's widget-handle-in-model or side-effecting-update shapes (they are why Relm4 can't replay).
4. Promote a settled decision into the proto-3 spec under `docs/specs/`; this file captures what Relm4 teaches, not Buiy's own locked decisions.

## Sources

- All evidence + URLs in [`README.md`](README.md) § Sources.
- Buiy proto-3 charter — `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md`
- Buiy proto-2 retrospective + code (`Callback`, drain, routing) — `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`, `examples/mvu_native/src/`
- Buiy draft state-management spec — `docs/specs/2026-06-26-buiy-state-management-design.md`
- iced prior-art (Elm-architecture peer, structural reference) — [`../iced/lessons.md`](../iced/lessons.md)
