**Date:** 2026-06-26
**Status:** active
**Subject:** Druid — KEEP / AVOID decision file for Buiy core-MVU (prototype-3)

This is the consult-this-when-designing decision file. [`README.md`](README.md) is the evidence (Druid's `Data`/`Lens`/`Widget`/`Command`/`AppDelegate`/`Scope` model and the why-superseded record); this file is the synthesis: concrete **KEEP** (what Druid got right that the proto-3 core-MVU should carry), **AVOID** (the mechanisms that drove the Druid → Xilem rewrite, mapped to Buiy mitigations), and a dedicated section on the charter's hardest open question — **"does every widget become a Model?"** — because Druid is the one prior-art system that ran that exact experiment.

## Top of file: the single most important finding

**Druid is the proof that single-source-of-truth is necessary-but-not-sufficient for time-travel — and that the missing ingredient is exactly what proto-3 makes core.** Druid had one `Data` value as the source of truth, shipped Psst and Runebender on it for ~5 years, and *never* had record/replay/time-travel. The reason is structural, not incidental: Druid **never reified state changes as messages** (the transition is an opaque `&mut data` closure inside `event()`), and it **deliberately pushed widget-internal state — cursor, selection, scroll, IME — out of `Data` and into the retained `Widget` object** (see [`README.md`](README.md) §7). So Druid simultaneously *validates* the proto-3 core ("a data-first model core is viable and ships real apps") and *diagnoses* the two things that must be true for the headline thesis to pay off: **(1) reify every change as a logged `Msg`, and (2) route widget-internal state through that same funnel so the log is complete.** Buiy's proto-2 KEEP set (`Messages` inbox + ordered drain + `Reflect` record tap + `LogicalId`) is ingredient (1); making the substrate *core* (so `buiy_core`'s focus/text-edit/IME flow through it) is ingredient (2). **Druid is the natural-experiment that says: do both, or you get Druid's result — a store with no time-travel.**

A second-order finding for skeptics: **Druid never attempted "every widget is a reducer/actor."** It had *one* `Data` model, imperative widgets, and an opt-in `Scope` for local subtrees. That point on the design line shipped. The maximal proto-3 reading ("every widget a `Model` + reducer + mailbox") is *unprecedented*, and the place it would cost the most — whole-program per-change structural work at thousands of widgets — is exactly where Druid's `same()` diffing tax already hurt. Treat Druid as endorsing the *core* and the *tiered/opt-in* shape, not the maximal one.

---

## KEEP — what Druid got right that core-MVU should carry

1. **A data-first single source of truth as the core posture.** Druid's three-pillar framing ("`Data` is your model; `Widget` is your UI; `Lens` glues them") shipped real apps and is the closest existing-art endorsement of putting the model at the centre. Buiy-MVU's per-entity model + the `Reflect` log is the ECS-native version. *Druid says the posture is sound; it does not say the model must be a single global value (that is Druid's, and iced's, scaling pain — see AVOID #3).*

2. **A single central choke-point with `&mut model` access is the right place to record/intercept.** `AppDelegate::command` is called for **every** `Command` before tree delivery, with `&mut T`, returning `Handled` — i.e. a single ordered point that sees every message and can mutate the model. This is structurally Buiy's `MvuSet::Drain` + record tap (proto-2 KEEP #1/#2). Druid *validates the shape*: a central command-funnel is the natural seam for recording, replay-gating, and supervision. **KEEP** Buiy's single ordered drain; **generalize** Druid's seam from "only `Command`s" to "all state changes" (that generalization *is* the proto-3 bet — see AVOID #1).

3. **Read-only ambient context, threaded and subtree-overridable.** `Env` (typed `Key<T>`, `EnvScope` subtree override) is a clean pattern for "ambient read-only data every node can read but not mutate." It is a loose structural ally of Buiy's sealed `PureEnv` (the read-only env a reducer may read). **KEEP** the discipline: ambient config/theme is read-only context, distinct from the mutable model — and the determinism guarantee lives at the *drain over the model*, not in the env.

4. **Async results re-enter as messages on the main thread.** `ExtEventSink::submit_command` lets a background thread submit a `Command` back onto the UI thread — the one place Druid is message-mediated, and the right shape: **async completions become ordinary messages in the funnel.** This is precisely Buiy's planned `Cmd::task`/`InFlight` poll-and-enqueue (proto-2 REFINE #4): the poll system enqueues the result as a normal `Envelope`, so it records and replays. **KEEP** — but unlike Druid, design it into the core drain from day one (see AVOID #4).

5. **Compile-time, zero-cost sub-state access via closure-passing.** `Lens::with`/`with_mut` pass a closure to the field (rather than returning `&U`) so both the lens and the closure inline to nothing, and the parent keeps ownership of `T`. **KEEP the *technique*** wherever Buiy needs to hand a reducer/`bind` a scoped mutable view of a slice (closure-passing beats returning borrows under Rust's aliasing rules). **Do not** keep `Lens` as the *user-facing addressing model* (AVOID #2) — keep the inlining trick, drop the global-projection ergonomics.

---

## AVOID — the mechanisms that drove the Druid → Xilem rewrite (with Buiy mitigations)

| Pitfall | Druid evidence | Buiy core-MVU mitigation |
|---|---|---|
| **A partial / optional log.** Druid had *two* state-change paths — direct `&mut data` in `event()` (unrecorded, not message-shaped) and `Command` (message-shaped but never logged) — and *neither* was recorded. Result: a single-source-of-truth app with **no** time-travel. | [`README.md`](README.md) §4–§7; no Druid replay tooling exists. | **The funnel must be the *only* path and must be core.** Make the recordable `Msg` substrate live in `buiy_core` so widget-internal mutations cannot bypass it (the proto-3 thesis). An optional/app-boundary log reproduces Druid's incompleteness exactly — which is the charter's whole motivation. Keep proto-2's "the determinism lives at the drain, not the queue." |
| **State transitions are opaque, not reified.** Druid's transition is an arbitrary `&mut data` closure — there is no value to log or re-fold. SSoT without reification ⇒ no replay. | Raph's posts + the `event()` contract ("the only place your model can change"). | **Reify every change as a typed `Msg` + record at the drain** (proto-2 KEEP: `Messages` inbox, ordered drain, `Reflect` log, `LogicalId`, run-to-completion `Emit`). The `Msg` *is* the loggable, replayable value Druid lacked. |
| **Whole-tree structural work on every change.** `Data::same()` drives a whole-tree `update` pass; the model is cloned/compared each frame; `Vec`/`HashMap` are excluded from `Data` (too costly), large data must be `Arc`-wrapped. Raph: *"heavy reliance on diffing creates its own problems."* This is the diffing tax Xilem's id-path routing + `PartialEq` memoization replaced. | [`README.md`](README.md) §1; Raph "Towards principled reactive UI". | **Do not pay Reflect-serialize on every frame for everything.** This is the direct analog of the charter's PERFORMANCE risk. Make recording *targeted*: per-widget opt-out, sampling on hot paths, record at the `Msg` granularity (cheap) not via whole-world `Reflect` snapshots. Gate cost with **hw-independent iai-callgrind** benches (the perf-campaign constraint). Druid + Xilem both *moved away* from unconditional whole-program per-change work — Buiy should not re-adopt it under a new name. |
| **`Lens` as the user-facing state-addressing model confuses people and doubles the work.** Raph: components integrate as *"each a half-lens, requiring the writing out of two pieces of logic"*; *"a lot of people coming to Druid find them confusing."* Xilem kept the idea (`Adapt`) but it remains the sharp edge. | Raph "Towards principled reactive UI"; [`../xilem-masonry/xilem-architecture.md`](../xilem-masonry/xilem-architecture.md) (`Adapt`). | **Use ECS entity identity (`LogicalId`) as the addressing primitive, not a lens path.** The entity *is* the address; routing is `EntityEvent`/observer bubbling to the nearest model owner (proto-2 KEEP #3). This sidesteps the half-lens boilerplate entirely. Unify `LogicalId` with the agent-interface **test-id space** (charter RE-DECIDE) so there is one identity space, not Druid's parallel `Selector("string")` namespace. |
| **Async bolted on after the fact.** Raph: async is *"something the existing Druid architecture struggles with"*; `ExtEventSink` was retrofitted. | Raph "Towards principled reactive UI". | **Design `Cmd::task`/`InFlight`/`takeLatest` into the core drain from the start** (proto-2 REFINE #4): poll system enqueues results as normal `Envelope`s so they record + replay. Add dead-letter (loud, typed) + `catch_unwind` reducer supervision as *core* concerns (charter `Cmd` algebra item). |
| **A heavy OOP per-widget contract.** Every widget implements a five-method `Widget<T>` trait (`event`/`lifecycle`/`update`/`layout`/`paint`) — significant boilerplate, named as a rewrite driver. | [`README.md`](README.md) §2; Xilem's view-fn model is the reaction. | **Do not make "be a reducer" a heavy per-widget tax.** Leaf widgets should *route*, not each carry a 5-method actor contract (see "Every widget a Model?" below). Invest the ergonomics budget in the variadic reducer macro (Bevy-`IntoSystem`-style bare-param signatures; proto-2 REFINE #1) so a reducer is `fn(&mut M, Msg, &Env) -> Cmd`, not a trait impl. |
| **Stringly-typed cross-cutting identity.** `Selector::new("process_rows")` is a global string id (the payload type is checked, the name is not). | [`README.md`](README.md) §4. | **Typed `LogicalId`** unified with the agent test-id space; avoid free-form string ids for routing/record keys (they undermine the determinism + agent-driving guarantees). |
| **Widget-internal state silently excluded from the model.** Druid's documented rule "lift important state to `Data`, keep implementation details in the widget" pushed cursor/selection/scroll/IME *off-model* (`TextBox` private fields; `Scope` local models) — so they were unrecordable even in principle. | [`README.md`](README.md) §7; `druid/src/widget/textbox.rs`. | **This is the crux to close, not repeat.** For replay-critical widgets (text editor: caret, selection, IME, scroll), route the internal state *through the funnel* so it is in the log (closes the un-reflected `TextEditState` crux). Tier it (next section) so non-critical ephemeral state can stay imperative for perf — but make that an explicit, opt-in tier, not Druid's silent default-exclude. |

---

## Every widget a Model? — Druid is the one system that ran the experiment

The charter's hardest open question: *"does **every** widget become a `Model` + reducer, or do leaf widgets stay imperative and only route?"* Druid is the most direct prior art, because it tried a **tiered** answer and shipped it:

1. **App-level `Data`** — one recorded(-shaped) single source of truth. *(Maps to: Buiy's app/screen `Model`.)*
2. **`Scope`/`ScopePolicy`** — opt-in *local* model for a reusable subtree (tabs' selected index, a table's sort/filter/scroll), with a two-way transfer to the outer state, *deliberately* kept out of app `Data`. *(Maps to: Buiy's "a widget gets its own `Model` only when it needs cohesive internal state.")*
3. **Private `Widget` fields** — ephemeral imperative state (cursor blink timer, `was_focused_from_click`), never modelled at all. *(Maps to: Buiy's imperative escape hatch / leaf widgets that only route.)*

Two lessons fall straight out:

- **A tiered model is the proven shape; "every widget a full actor" is not.** Druid shipped Psst/Runebender on tiers 1+3 with tier-2 opt-in. So Buiy should **default to: app/screen `Model` + opt-in per-widget `Model` (for genuinely stateful widgets like the text editor) + an imperative escape hatch for leaves** — *not* a uniform "every entity is a reducer with a mailbox." This is also the cheaper answer for the SCALE/PERFORMANCE risk (per-entity model + mailbox + drain at thousands of widgets is the charter's named cost; Druid's `same()` tax is the warning that whole-tree-per-change is where 60 Hz dies).
- **But Druid's tiering is *why it had no replay* — so Buiy must change the recording boundary, not the tiering.** Druid put tiers 2 and 3 *outside* the recorded model on purpose. Buiy's innovation is orthogonal to the tiering: keep the tiers, but **make the funnel the write-path for whichever tier needs to be in the log** (tier-2 for the text editor's `TextEditState`; tier-3 stays out by explicit opt-out). The escape hatch the charter demands ("MVU-primary must still allow raw ECS") is Druid's tier-3, blessed and named — power users drop a leaf widget to imperative `&mut` ECS exactly where Druid kept private fields. **Do not trap every widget in the paradigm; do make the paradigm the *only recorded path*.**

The net recommendation: **adopt Druid's tiering, invert Druid's recording boundary.** Tiers stay (app model / opt-in widget model / imperative leaf); the difference is that the replay-critical tiers route through the core `Msg` funnel so the log is complete — the one thing Druid never did.

## Mapping to the charter's RE-DECIDE items

| Charter RE-DECIDE item | Druid evidence | Recommendation |
|---|---|---|
| Authoring-surface placement (core/default vs opt-in) | Druid's `Command` channel was optional + partial ⇒ no complete log | Substrate **core**; funnel is the only recorded write-path. **Druid endorses this.** |
| Reducer ergonomics (variadic macro vs `&Env`) | Druid's 5-method OOP trait was a rewrite driver | Invest in the bare-param variadic macro so "be a reducer" is light; don't re-create a heavy per-widget contract. |
| Widget granularity (every widget a `Model`?) | Druid shipped a *tiered* answer (Data / `Scope` / private fields) | **Tiered**, not maximal: app model + opt-in widget model + imperative leaf escape hatch. |
| `PureEnv` enforcement | `Env` = read-only ambient, threaded everywhere | Structural ally; keep `PureEnv` sealed (proto-2 REDESIGN #1), distinct from the model. |
| `Cmd` algebra (+ dead-letter, supervision) | `ExtEventSink` async re-entry = right shape, bolted on late | Design `task`/`done`/`batch` + dead-letter + `catch_unwind` into the core drain from day one. |
| `LogicalId` ↔ agent test-id space | `Selector::new("string")` stringly-typed parallel namespace | Single typed identity space; avoid free-form string routing keys. |
| Migration model | Druid is a *complete rewrite that succeeded on architecture but cost years* ([`../xilem-masonry/history.md`](../xilem-masonry/history.md)) | Favor **incremental adoption** over big-bang; `buiy_core` is mature (charter MIGRATION-COST risk). The Druid→Xilem 7-year arc is the warning against a from-scratch rewrite. |

## How to use this file

1. Find the **AVOID** row closest to the seam you're designing; read the linked [`README.md`](README.md) section for the original Druid mechanism and the rewrite that replaced it.
2. Find the **KEEP** entry closest to a primitive you're adding; carry the *shape*, adapt to Buiy's ECS-native, `Reflect`-logged, `LogicalId`-addressed model.
3. For the granularity decision, start at "Every widget a Model?" — Druid is the only system that shipped the tiered experiment.
4. Promote any decision into the proto-3 **spec** (`docs/specs/`, superseding `2026-06-26-buiy-state-management-design.md`); this file captures what we learn from Druid, not Buiy's own decisions.

## Sources

- All evidence + citations: [`README.md`](README.md) (Druid `Data`/`Lens`/`Widget`/`Command`/`AppDelegate`/`Env`/`Scope`, `textbox.rs` fields, crates.io facts, Raph's posts).
- Druid → Xilem lineage (the `Adapt`=`Lens` and id-path=`same()`-targeting successors): [`../xilem-masonry/history.md`](../xilem-masonry/history.md), [`../xilem-masonry/xilem-architecture.md`](../xilem-masonry/xilem-architecture.md), [`../xilem-masonry/linebender-stack.md`](../xilem-masonry/linebender-stack.md).
- Structural/tone reference for this folder: [`../iced/lessons.md`](../iced/lessons.md) (Elm-architecture peer; the "stateless widgets + reconciliation tree" negative reference cross-applies to Druid's `Lens`).
- Buiy proto-3 charter — `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md`.
- Buiy proto-2 retrospective (KEEP / REDESIGN / REFINE set) — `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`.
- Draft state-management spec (to be superseded) — `docs/specs/2026-06-26-buiy-state-management-design.md`.
