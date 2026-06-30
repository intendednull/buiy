# MVU-as-core (prototype-3) — RESEARCH SYNTHESIS

**Date:** 2026-06-26
**Stage:** `/staged-development` RESEARCH synthesis — the gate input that seeds the proto-3 SPEC.
**Base:** current `origin/main` @ `5c0da9f` (perf campaign #84 on top of parity #83 `7752c01`).
**Supersedes-target:** the new spec this seeds will supersede `docs/specs/2026-06-26-buiy-state-management-design.md` (the "opt-in `buiy_mvu`" draft).
**Inputs:** the proto-3 charter, proto-1/2 retrospectives + draft spec, and the 10 research artifacts in this folder + `docs/prior-art/{druid,relm4,elm-redux-time-travel}/` (and the existing `gpui/`, `xilem-masonry/`, `floem/`, `iced/`, `dioxus/` corpus read through the reactive-cores lens).

> **Charter rule honored.** Every decision is re-opened from evidence. Where the research contradicts the draft spec, the charter's literal framing, or a proto KEEP-shape, this synthesis says so and **re-decides**. The biggest re-decisions: (1) placement flips opt-in → **core** (the whole point of proto-3, confirmed by dependency direction); (2) the charter's literal "**every widget is an actor**" is **rejected** in favor of a **tiered** granularity; (3) the editor is **exempt from reducer purity** (an imperative routing leaf, not a `Model`); (4) recording flips eager-always-on → **default-OFF + lazy**; (5) the load-bearing perf risk is **not** Reflect-serialize — it is the **drain defeating change-detection**.

---

## 1. Headline findings

**H1 — The thesis is sound, and the prior art shows *why the alternatives cannot reach it*.** Time-travel/replay is reachable from exactly one architecture: pure reducer + effects-as-values + a single ordered funnel (Elm/Redux/Iced lineage). The signal lineage (Floem/Dioxus) forecloses it (scattered cell writes, no ordered funnel); the production actor lineage (GPUI/Zed) forecloses it (direct `&mut` mutation + direct-call effects, unrecorded). Druid is the natural experiment: it had a single source of truth and shipped real apps for ~5 years and *still* never got time-travel — because it never reified state changes as logged messages and excluded widget-internal state from the model. **Choosing the Elm reducer model *as the core* is therefore not arbitrary; it is the only road to the headline payoff.**

**H2 — "Every widget is an actor" is REJECTED; the answer is tiered.** No prior-art system makes every widget an actor. GPUI (the strongest production Rust actor UI) makes only stateful things `Entity<T>` and keeps leaf UI (buttons, labels) stateless `Div` elements; Relm4 (the actor-per-widget poster child) puts an actor on *nothing* below a stateful unit — a `gtk::Button` inside `view!` has no Model/mailbox/update. Buiy's own widget set already agrees: `Button` owns **no** state (`button.rs:59`); `Checkbox`/`Switch`/`Slider`/`Disclosure` each own a single-source-of-truth component (`A11yToggled`/`A11yValue`/`A11yExpanded`) read by a `Changed`-gated visual — the only defect is **multiple writers**, not the data model. The tiered model: **router leaves** (Button — no model, route only), **stateful leaves** (Checkbox/Switch/Slider/Disclosure — keep the component, make the drain the *sole writer*, no per-entity `Model` trait), **machines** (Menu/Dialog/Popover — real `Model`+reducer where one owning model deletes reconciliation code), **composites/raw-ECS** (escape hatch).

**H3 — The load-bearing perf risk is the drain defeating change-detection, NOT Reflect-serialize.** The charter names "Reflect-serialize cost on the hot path" as the thing that could kill the design. The evidence cuts against that: message rate is `O(interactions/frame)` (single digits), never `O(widgets)`; a small-Msg serialize is ~1–5K instructions, <0.1% of the weak-machine ~16M-instr/frame budget — ~1000× below the render work a single interaction already triggers (a full re-extract at 5000 nodes is ~3.18ms ≈ an entire weak-machine frame). The **real** killer is proto-2's drain doing an unconditional `&mut model` on every fold: that trips `Changed<M>` even on a no-op, which cascades bind → layout → full re-extract, **re-introducing the exact #2/#6 cliffs the perf campaign just removed**. The fix is a solved discipline in this codebase: `set_if_neq` / conditional `deref_mut`, gated by a work-counter (`models_mutated == 0` on an idempotent fold).

**H4 — Placement MUST be core; an opt-in crate is structurally incomplete.** The dependency graph proves it: `buiy_core` is the bottom (depends only on `accesskit/*`); everything depends *down* on it. The activation sink (`Messages<OnPress>`), the AT inbound seam (`dispatch_action_request`), and the consumers that mutate (`advance_toggle_on_press`, the editor apply, the focus writers) **all live in core/widgets**. An opt-in `buiy_mvu` above core *can* read the `OnPress` sink (proto-2's `bridge_press` literally `use buiy_core::interaction::OnPress`) but **cannot re-home the in-core consumers** to enqueue instead of mutate, and **cannot make core's inbound seam lower through it** without a dependency cycle. So an opt-in log is complete only at the app boundary — the precise incompleteness (widget-internal focus/edit/value/expand) the charter exists to fix. **The draft spec's §10 "opt-in `buiy_mvu`" is re-decided to a core `buiy_core::mvu` module.**

**H5 — The `TextEditState` crux is solved by command-sourcing, not by Reflecting the editor.** `TextEditState` (`text/edit/state.rs:92`) is `#[derive(Component)]` only, deliberately non-Reflect because it wraps `cosmic_text::Editor<'static>` + foreign `Change`s. "Make it Reflect" is the wrong framing. The editor is *already* a de-facto reducer: one verb vocabulary (`EditCommand`, `command.rs:21`) lowered at exactly one site (`apply_tracked`, `input.rs:94`). Record the **resolved `EditCommand`/IME stream**; replay re-folds from a seed buffer; the cosmic Editor never serializes. Hot-reload is a *different* problem solved by a small `TextEditSnapshot` logical projection (value + caret-with-affinity + selection + preedit + undo-as-`ChangeItem`s), not replay-from-init. **But the editor's update is intrinsically impure** (motion/click shape against `&mut FontSystem`; Paste reads the OS clipboard) — so the editor is the documented case where "pure actor" breaks: it is an imperative routing leaf, determinism guaranteed at the boundary (same FontSystem + box), not via `PureEnv`.

**H6 — "Complete log" and "unrestricted escape hatch" are mutually exclusive; scope the guarantee.** A raw-ECS write is by construction not a logged Msg, so any escape-hatched entity makes whole-UI replay diverge silently. Do not ship "whole-UI replay" as unconditional. Ship **"complete and byte-identical over the MVU-governed subtree,"** with the boundary made *detectable* (a debug `Changed<T>` "write-outside-the-funnel" auditor) and the framework's own funneled state *encapsulated* (`pub(crate)`, drain-only — the `TextEditState.editor` precedent at `state.rs:96`).

**H7 — Recording's true cost is MEMORY, and it must be default-OFF + lazy.** Proto-2 serializes eagerly to RON per fold into an unbounded `Vec<String>` — fine for a spike, wrong for the hot path and a memory leak acute on 32-bit wasm. Re-decide: a `RecordMode { Off, Ring(n), Full }` defaulting **Off** (production pays zero), and **lazy** serialize (store the typed message / `Box<dyn Reflect>` in a bounded ring; RON only at export/checkpoint). This neutralizes the charter's #1 fear by construction.

**H8 — Two replay-completeness gaps the prior art surfaces that the draft spec leaves open.** (i) **Structural ops are off-log.** Draft §5 puts keyed-reconcile spawn/despawn in a system *outside* `update`; re-folding the Msg log then won't recreate spawned children — the decomposed analogue of Redux's "state outside the store is invisible to time-travel." Structure must be **on-log or provably a pure function of on-log parent state.** (ii) **No subscription primitive.** Timer/IME/OS-event-driven sources are unreplayable unless every event enters the funnel as a logged Msg; the proto `Cmd` algebra has none (the charter's open "+stream?"). Add an Iced-style keyed `Subscription`.

**H9 — Seam, not rewrite.** No prior-art system rewrote a mature imperative core into actors — all layered the paradigm above an imperative toolkit (Masonry⊥Xilem, `floem_reactive`⊥views, `iced_core` widgets are values). `buiy_core` is ~43k LOC, verified (layout/render/text/editing/a11y/widgets + GPU goldens). The charter's Advantage #4 ("redesign `buiy_core`'s subsystems into MVU actors") is the **highest-risk, least-supported** item. The supported move is to keep imperative systems that *route into* the funnel, migrate one subsystem at a time, and **pilot on text-edit** (the crux) before touching focus/interaction/a11y.

---

## 2. Decisions the spec must make (recommendation · rationale · named runner-up)

### D1 — Placement: what is core vs separable

**Recommendation.** Three-way split:
- **Core, always compiled, cheap-when-unused:** the funnel + single ordered drain + `MvuSet` + `Model`/`Cmd` + sealed `PureEnv` + `LogicalId` resolver. Lives in a new `buiy_core::mvu` module composed by `CorePlugin`. The drain no-ops with no `Model` present, so the cost when unused is one empty-inbox check per registered model type.
- **Core but gated / off-hot-path:** the `Reflect`/RON record-and-replay harness (a *consumer* of the drain, not the substrate). Default-OFF (`RecordMode`).
- **Separable / incremental:** the widget migration (reshaping `buiy_widgets` writers through the drain), the `bind`/derive ergonomics, the variadic reducer macro, and the MCP transport (already `buiy_mcp`).

**Rationale.** H4: the inbound seam and the convergent producers/consumers are core-resident; only a core funnel can re-home them. Co-locating the Msg write-log with the AccessKit read-tree (also core) makes them exact duals over one identity space.

**Runner-up (rejected): the draft spec's opt-in `buiy_mvu` crate.** Rejected by the dependency evidence — it caps the log at the app boundary and leaves widget-internal state invisible (proto-2 demonstrated it can bridge `OnPress` *up* but cannot record the direct-sink verbs `Focus`/`Expand`/value/text). This is the locked decision proto-3 exists to overturn.

---

### D2 — Widget granularity (the charter's hardest question)

**Recommendation: TIERED — `Model`+reducer reserved for stateful machines; leaves keep their component and route.**

| Tier | Widgets | Shape |
|---|---|---|
| **Router** | `Button` | No model; converges pointer/keyboard/AT on the activation route; routes a Msg upward. |
| **Stateful leaf** | `Checkbox`, `Switch`, `Slider`, `Disclosure`, `ScrollArea` | Keep the existing single-source-of-truth component (`A11yToggled`/`A11yValue`/`A11yExpanded`/scroll). Route an activation/value Msg; **the drain is the SOLE writer.** No per-entity `Model` trait, no per-entity mailbox — one shared role-keyed reducer. |
| **Machine** | `Menu`, `Dialog`, `Popover`/overlay | A real `Model`+reducer owning the multi-field state (open/active/focus-return). One owning model *deletes* the reconciliation code (e.g. `menu::sync_menu_dismissed`). |
| **Composite / raw-ECS** | app-bespoke, virtualized lists | Imperative escape hatch; outside the replay boundary until/unless ported. |

The `Model` *trait* (associated `Msg`/`Out`) applies to **Machines** (and app screens) only; the leaf tier needs only "the drain is the sole writer of component X." This is a meaningful public-API simplification — settle it early.

**Rationale.** Leaf state is *already* MVU-shaped (single component, `Changed`-gated visual); "make it a `Model`" buys nothing the component+drain doesn't, while a per-leaf `Model`+record-tap is exactly the per-instance ceremony + hot-path serialize cost that threatens the 60Hz floor at slider-drag / 10k-row scale. The complexity (and 100% of the reconciliation pathology) is concentrated in the machines, where one owning model is a genuine simplification. GPUI, Relm4, and Druid all independently shipped this tiered shape; "every widget a full actor" is *unprecedented* and is exactly where the SCALE risk lives.

**Runner-up (rejected): the charter's literal "every widget is a Model+reducer."** Rejected on three grounds: Button has no state (pure ceremony); per-leaf record-tap on hot folds (slider drag, scroll, 10k leaves) threatens the floor; and it conflates *recordable* (record the activation Msg at the sink) with *modeled* (a multi-field machine). Recording at the Msg sink keeps the log complete without per-leaf models.

---

### D3 — The self-update-vs-controlled double-write / flicker (charter advantage #2)

**Recommendation: single writer through one drain.** Light-dismiss, press, Escape, AT, and controlled-parent updates all **enqueue** a Msg; one reducer folds and writes the one state field. This deletes `menu::sync_menu_dismissed` (the live reconciliation system that exists *only* because two writers own the "menu-open" fact and a third keeps them in lock-step, `menu.rs:615`), and collapses "controlled vs self-updating" into one model (a controlled widget is just one whose reducer-owner is an ancestor).

**Rationale.** The visual only ever observes a folded value, so the one-frame flicker cannot occur by construction. The cure is needed: `sync_menu_dismissed` and the gallery's direct `A11yToggled::True` writes racing `advance_toggle_on_press` (`lib.rs:1129`) are live multi-writer bugs in current main.

**Runner-up (rejected): the draft spec's suppression flag** (suppress `advance_toggle_on_press` when a controlled marker `OnPressMsg<M>` is present). Rejected as more fragile and per-widget than making the drain the unique writer; it patches the symptom, not the cause.

---

### D4 — `PureEnv` enforcement core-wide + `#[derive(PureEnv)]`

**Recommendation: keep the sealed `PureEnv` allowlist (NOT `ReadOnlySystemParam`) and add `#[derive(PureEnv)]` for user env structs — but EXEMPT the editor.** The reducer signature stays `fn(&mut M, Msg, &Env) -> Cmd`; `Env` is `()`/`Res`/read-only `Query`/`Local`/tuples, blessed by the sealed trait; `Commands`/`ResMut`/`Query<&mut>`/`MessageWriter` simply aren't, and the orphan rule stops anyone blessing them. `#[derive(PureEnv)]` blesses a user struct of blessed fields (proto-2 blessed only primitives + tuples — this is the residual gap to close).

The **editor is the documented exemption**: its update needs `&mut FontSystem` (shaping is intrinsic to applying motion/click) and reads the OS clipboard — a sealed `PureEnv` cannot bless `FontSystem`. The editor is therefore an **imperative routing leaf**, not a pure `Model`; determinism is guaranteed at the boundary (same FontSystem + box ⇒ same fold), and the clipboard read becomes a logged effect (D6).

**Rationale.** Proto-2 proved `ReadOnlySystemParam` is fatally leaky in Bevy 0.19 (`Commands: ReadOnlySystemParam` — a reducer could `spawn`/`despawn` outside the funnel and diverge replay silently). The sealed allowlist is strictly stronger than Redux purity (mere convention — the #1 source of broken-time-travel bugs) and Elm flags-omission. GPUI is the existence proof that purity buys *nothing* for reentrancy safety (Bevy's command-flush already provides that) — so purity's **sole** load-bearing payoff is the recordable log, which means it must be cheap and escapable where replay isn't wanted, and the editor (impure by nature) must be exempt rather than contorted.

**Runner-up (rejected): refactor the editor so the reducer mutates a pure rope and shaping is a `Cmd` effect.** Rejected for v1 — cosmic's `Editor` couples text mutation to cursor/visual-motion semantics that need shaped runs; splitting it is a deep rewrite of the verified editor, out of proportion to the benefit.

---

### D5 — Reducer ergonomics + the variadic / `IntoSystem`-style macro

**Recommendation: ship `&Env` + `#[derive(PureEnv)]` (option b) for v1; defer the variadic macro.** The reducer is a free fn `fn(&mut M, Msg, &Env) -> Cmd` registered via `add_reducer` / `add_reducer_env`; the env item is reused by `&` across folds (it isn't `Clone`), named via turbofish where inference can't run backward through the `SystemParamItem` projection.

**Rationale.** Proto-2 found the bare-variadic signature (`fn update(&mut M, Msg, Res<Step>)`) is reachable only with Bevy's full `IntoSystem`-style variadic macro + the `for<'a> &'a mut Func: FnMut(P) + FnMut(SystemParamItem<P>)` double-bound — buildable but heavyweight. The `&Env`+derive path is simpler, reads well, and is enough to ship the substrate. Relm4/Druid both warn that a *heavy per-widget authoring contract* (Druid's 5-method `Widget<T>`, Lens's "half-lens × two pieces of logic") was a primary rewrite driver — so spend the ergonomics budget on keeping the reducer a free fn, not on macro machinery.

**Runner-up (deferred, not rejected): the variadic bare-param macro.** Worth building once the substrate is core and the surface is exercised; revisit when `add_reducer` ergonomics demonstrably bite. Naming it a v2 follow-up keeps v1 small.

---

### D6 — `Cmd` algebra (task/done/batch + stream) + dead-letter + `catch_unwind`

**Recommendation: adopt the full effects-as-values algebra as the target — `none`/`done`/`task`/`batch` + `chain`/`sequence` + a keyed `Subscription`-equivalent for long-lived sources — with the hard invariant that ALL effects are values (suppressible in replay).** `task` spawns on `AsyncComputeTaskPool`, the `Task<Msg>` stored as `InFlight<M>` on the originating entity; a poll system folds the result **back through the drain as a recorded Msg** (so it records + replays); single-in-flight = takeLatest (drop = cancel on despawn/supersede). Tag each recorded `Envelope` with an origin marker (`User`/`Command`/`Folded`/`Subscription`) so the log is self-describing. Add **dead-letter** (loud, typed — an unhandled/despawned-target Msg is surfaced, not silently dropped) and **`catch_unwind` reducer supervision** as core concerns, **cfg-gated to native** (wasm is `panic = abort`; keep the dead-letter `continue` path, which works everywhere).

**Rationale.** Effects-as-values is the precondition for "drop Cmds on replay" — proven necessary by *both* Elm (re-running effects is unsafe + double-fires messages, since Cmds generate Msgs) and Redux (effects isolated in middleware, never the reducer). The keyed `Subscription` is the precondition for log *completeness*: a timer/IME/OS-event UI is unreplayable unless every such event enters the funnel as a logged Msg — and the proto currently has no subscription primitive (the charter's open "+stream?"). Relm4's `AsyncComponent` (await-in-update) is a documented footgun that blocks the whole mailbox sequentially; its own guidance prefers Commands — so the spec must **forbid awaiting in the reducer/drain**; only `Cmd::task` + poll-fold-back.

**Runner-up (rejected): a separate `CommandOutput` type + `update_cmd` (Relm4's split), or GPUI direct-call effects.** The Relm4 split doubles the message surface and is not replay-shaped; its only real benefit (debugging clarity — "this came from a network result") is recovered more cheaply by the envelope origin tag. GPUI direct calls can't be suppressed or re-fed in replay.

---

### D7 — `LogicalId` ↔ agent-interface test-id unification

**Recommendation: ONE author-assignable `LogicalId`, layered over the existing AT `NodeId`.** Introduce one core stable id that is simultaneously (i) the Msg-log key, (ii) the `get_by_role` tie-break / agent + MCP addressing id, and (iii) author-assignable with a **deterministic structural fallback** (parent id + local key — so unlabeled widgets get a session-stable id and dynamic lists get keyed-reconcile ids). **Layer it:** keep `node_id_for(entity) = NodeId(bits+1)` as the AT-facing winit wire ref (ATs never compare cross-session), and resolve `LogicalId → Entity` through a registry resource for the agent/test/log path. One resolver, two faces.

**Rationale.** There is no `LogicalId` and no test-id in current core (grep = 0); the only identity is the session-stable `bits+1` `NodeId`. Two would-be stable-id systems are already converging on the same need (the agent-interface's planned author test-id follow-up + the MVU log key). The `bits+1` vs raw `to_bits()` off-by-one that already bit once (two serializers disagreeing on the key) is positive evidence that parallel id derivations drift — collapsing to one is a simplification cascade. Determinism (no `uuid`/random — which would also re-activate `getrandom` on wasm and break cross-process re-fold) is mandatory.

**Runner-up (rejected): unify into `NodeId` itself** (make `LogicalId` *be* the `NodeId`, drop `bits+1`). Conceptually cleaner (one u64) but it rewrites the wire ref the entire verified AT path + every a11y golden keys on, and turns a pure-math inverse into a hot-path registry lookup — higher risk for marginal gain; revisit post-migration. (Second runner-up — keep them separate — rejected: cross-session record/replay, time-travel across respawn, and hot-reload are impossible when the ref is entity bits.)

---

### D8 — Migration model

**Recommendation: incremental, seam-not-rewrite, ~5 staged phases, pilot on text-edit.** (0) Land the funnel in `buiy_core::mvu` behind the existing `OnPress` sink with **zero widget API change**; (1) re-route leaf-control writes through the drain (deletes the controlled/self-update suppression flag, D3); (2) **pilot the seam on text-edit first** (route `EditCommand`s through the funnel + add the `TextEditSnapshot` projection) — it is the crux, prove it before generalizing; (3) convert machines (Menu/Dialog/Popover) to single Models, **one per gated PR, LAST**, each with the live-interaction test tier + the GPU lavapipe lane; (4) focus as a singleton focus-actor (D-focus below); (5) explicit escape hatch (composites + raw ECS stay imperative, outside the replay boundary until ported).

**Rationale.** The state components already exist and are already single-source-of-truth, so the migration *reroutes writers* rather than redesigning data — `#[require]` contracts, `Changed` visuals, and the a11y fold are untouched. A big-bang rewrite throws away ~11.4k LOC verified widget src + ~3.9k LOC widget tests + the gallery + ~43k LOC core, all encoding APG/a11y/visual behavior; no prior-art system rewrote a mature core, and Druid→Xilem (the one that tried a from-scratch successor) took ~7 years. Menu has a documented GPU-lane regression history (an "anti-clobber" change killed the editor Text-seed, caught only by lavapipe on PR) — which is exactly why machines convert one-per-gate with the GPU lane on each.

**Runner-up (rejected): big-bang MVU-ification of `buiy_widgets`/`buiy_core` onto the new runtime.** Rejected — discards verified behavior and risks reconciliation regressions the headless gate can't catch.

---

### D9 — Escape hatch

**Recommendation: the `Model`/funnel membrane is the boundary; tier the replay guarantee; encapsulate framework funneled state; ship a debug auditor.** A power user drops to raw ECS by simply not attaching a `Model` and writing ordinary `Update` systems against still-public components, reading the still-public `Messages<OnPress>` (exactly what `buiy_widgets`' systems are today). Replay is **complete and byte-identical over the MVU-governed subtree**; raw-ECS entities are explicitly outside it and reconstructed at replay-init (Elm-flags model). Make the framework's *own* funneled-state fields `pub(crate)` + drain-only (the `TextEditState.editor` precedent, `state.rs:96`) so the framework's guarantee holds by construction; user state holds by a documented contract. Ship a debug-build `Changed<T>` "write-outside-the-funnel" auditor so the boundary is detectable, not silent.

**Rationale.** H6: "complete log" and "unrestricted hatch" are in direct tension; the honest resolution is a *scoped* guarantee with a detectable boundary, not a pretense that both hold. Every serious UI ships an imperative escape (GPUI `Element`, Iced custom `Widget`, Xilem-via-Masonry); the need is unanimously validated.

**Runner-up (rejected): seal funneled state behind funnel-only access** (private components, `OnPress` consumable only by the drain). Rejected — it traps power users in the paradigm (the charter's explicit anti-goal), breaks the public `OnPress` contract, and is impossible for foreign state (`cosmic_text::Editor`) anyway.

---

### D10 — Schedule / render-extract integration

**Recommendation: fold `MvuSet` into the existing `BuiySet` chain late in `Update`, with a pinned `ApplyDeferred`, and pin the editor drain before the reshape repair.**

```
BuiySet::Layout → Style → Input → Animate → Picking → A11yUpdate
   → MvuSet::Enqueue        (.after(A11yUpdate); observers/systems enqueue)
   → ApplyDeferred          (flush Commands from enqueue + routing observers)
   → MvuSet::Drain          (the single ordered fold + record tap; a SYSTEM, never an observer)
   → MvuSet::Bind           (Changed<Model> → derived Text/Node; set_if_neq)
   → BuiySet::Render
```

Collapse proto-2's `OnPress → Routed<M>` two-hop into a single producer-triggered routed `EntityEvent` (kills the 1–2-frame latency). The **drain MUST be a system in a pinned set**, never an observer (observers fire at unpredictable command-flush points, re-entrantly — fatal for a deterministic ordered fold + single record tap). For the **editor fold specifically**, the drain is a *new post-`TextCommit` buffer mutator*, so it must be pinned so `reshape_edited_editors` runs **after** it and **before** the caret writer / extract (the `mod.rs:202` "any future post-Input editor-buffer mutator MUST be ordered before this system" warning is addressed to exactly this drain), and the fold must **run-to-completion within the frame**. Render extract is in `ExtractSchedule` (render world, after `Update`), so the only hard constraint is "drain + bind finish within `Update`."

**Rationale.** Proto-2 ran `(Enqueue,Drain,Bind).chain()` standalone with no relation to `BuiySet` — the integration gap: drain input (`OnPress`/routing) is produced across Input/Picking and output (model state) is read by Render extract, so the sets must be ordered against `BuiySet` or latency/determinism is emergent (proto-2 REFINE #2). The explicit `ApplyDeferred` makes latency one *designed* frame.

**Runner-up (rejected): keep proto-2's standalone unordered chain,** or a dedicated top-level set replacing `BuiySet::Input` (drain *is* the input stage). The standalone chain leaves ordering emergent (the exact non-determinism the thesis removes); replacing `Input` forces every widget to be an actor before anything works (no incremental path).

---

### D-focus — Focus as a singleton focus-actor (sub-decision under D2/D8)

**Recommendation: model focus as ONE root/app-level singleton focus actor.** `FocusedEntity`/`FocusVisible` are resources mutated from ≥4 sites (`focus_on_click`, `handle_tab`, AT `Focus/Blur`, dialog restore via `FocusReturn`); all focus changes enqueue focus Msgs the singleton folds. This fits the global state AND puts focus transitions in the log (focus drives *which* editor folds, so replay must reproduce it). **Runner-up (rejected):** keep focus as a raw resource with a side-channel record tap — rejected because it forks the log (focus recorded by a different mechanism than widget Msgs) and re-opens incompleteness.

---

## 3. Hard-risk verdicts

### PERFORMANCE / 60Hz hard floor — VERDICT: FEASIBLE within a stated boundary; "every widget a Model" is NOT the boundary to draw

"Every widget is an actor" is perf-infeasible *as literally stated* and, more importantly, **unnecessary** — the tiered model (D2) is both cheaper and the correct design. The boundary that makes the substrate feasible:
1. **Model TYPE = widget KIND** (~20–50 types), never type-per-instance / type-per-row / type-per-variant. Transport is one `Messages<Envelope<M>>` per *type* (5000 buttons share one inbox + one drain; the drain is `O(messages/frame)`, never `O(instances)`). The idle floor is `O(N_model_types)` — a small constant well under the existing `O(N_widgets)` layout/atlas idle cost — and is fully serial on single-threaded wasm, another reason to bound it. Data-driven lists use one *parameterized* model, not codegen-per-row.
2. **`set_if_neq` drain discipline is mandatory and gated** (H3). The drain `deref_mut`s a model only on a real change; binds fire only on `Changed<Model>`. This is the single rule that stops the funnel from re-introducing the #2/#6 full-rebuild cliffs. Enforce with an `MvuWorkCounters { drain_folds, messages_recorded, models_mutated, binds_fired, emits_refolded }` gate (idle: all 0; idempotent fold: `models_mutated == 0`).
3. **Recording default-OFF + lazy** (H7). Production pays zero serialize + zero log growth; even `Full` stores typed messages in a bounded ring and serializes only at export.
4. **Reuse existing widget components AS the funneled state, not wrap** — wrapping doubles per-instance memory at thousands of widgets (`TextEditState` is already heavy).

**Mitigation/gate:** land the substrate's hw-independent gates *with* it — `MvuWorkCounters` (the proven cheapest template), a dhat record-off/on band, and a **net-new iai-callgrind twin** (`mvu_idle/{50 types}`, `mvu_one_message`, `mvu_fold_storm/{1,10,100}`, `mvu_record_off_vs_on`). The iai weak-machine pricer does **not exist yet** (no `pipeline_iai.rs` on main) — building it is net-new work the substrate owns, not inherits.

### MIGRATION COST — VERDICT: bounded if incremental + seam-not-rewrite; a big-bang rewrite is the failure mode

~11.4k LOC widget src + ~3.9k LOC widget tests + ~43k LOC core + ~3.5k LOC editor (behind a facade), all verified incl. GPU goldens. The state components already exist and are single-source-of-truth, so the migration *reroutes writers* — it does not redesign data. **Mitigation:** the D8 5-phase staging (funnel behind `OnPress` with zero API change → leaf writers through drain → text-edit pilot → machines one-per-gate LAST → escape hatch), each machine PR gated by the live-interaction tier + the GPU lavapipe lane. Reconciliation regressions are GPU-lane-sensitive (the Menu anti-clobber precedent) — never land a machine conversion on green-headless alone.

### ESCAPE HATCH — VERDICT: structural and additive, but it forces a scoped (not unconditional) replay guarantee

The hatch already exists (no `Model` ⇒ invisible to the drain) and is exactly what `buiy_widgets`' 22 systems are today. **Mitigation:** D9 — tier the guarantee to the MVU-governed subtree, encapsulate framework funneled state (`pub(crate)` + drain-only), ship a debug write-outside-the-funnel auditor. Do not market "whole-UI replay" as unconditional.

### WASM — VERDICT: zero new COMPILE obstacle (evidence-backed); three runtime items to cfg-gate/bound

`ron` is already in `Cargo.lock` (via `bevy_scene`); `serde`/`serde_json` are workspace deps; `Reflect` is pure Rust; `Messages<T>` is a `Vec`; `AppTypeRegistry` is `Arc<RwLock>` uncontended under the single-threaded web scheduler. The substrate adds **no new crate**, so it cannot reintroduce the `arboard`-class "no wasm backend" failure. **Mitigations (none a *new* obstacle):** (i) cfg-gate `catch_unwind` supervision to native (wasm `panic = abort`; keep dead-letter); (ii) `LogicalId` deterministic (counter/hash, never `uuid`/random — replay determinism demands it anyway, and random re-activates `getrandom`); (iii) `MsgLog` default-OFF + bounded (in-memory `Vec`, never `std::fs`; persistence is a separate transport-agnostic concern — postMessage/IndexedDB).

### SCALE — VERDICT: per-type not per-instance; thousands of widget instances is essentially free

A `Model` is a `Component` Buiy already stores; there is no per-instance mailbox (one inbox per *type*); the drain never iterates idle instances. The cost that scales is `O(N_model_types)` — bound it to widget kinds (D2/perf boundary #1). Virtualized/data-driven lists stay one parameterized model + the keyed-reconcile-by-domain-id primitive (Relm4's `DynamicIndex` independently confirms: never address a moving element by snapshot position across a frame boundary). **Mitigation:** the perf boundary above + an `mvu_idle/{N types}` iai bench proving the idle floor is flat in widget count.

---

## 4. Coverage gaps + open questions for the spec stage

1. **Structural ops on-log vs derivable (must resolve before the spec locks).** Draft §5 keeps keyed-reconcile spawn/despawn *outside* `update`; re-folding the log then won't recreate spawned children — a whole-UI-replay gap. Decide: record structural ops as log entries, or prove structure is a pure function of on-log parent-model state. (elm-redux-time-travel report, open Q.)
2. **Subscription primitive in v1 or deferred?** Required for replay completeness of timer/IME/OS-event sources; absent from the proto `Cmd` algebra. Recommend v1.
3. **Jump/scrub over a decomposed model (the Redux `computedStates` problem).** No single state tree ⇒ naive jump is `O(total folds)` or `O(live Models)`. Spec the keyframe strategy (periodic whole-UI Reflect snapshots + re-fold tail) and the user-facing "step" granularity (per-fold / per-drain-pass / per-input-event). Needs iai measurement before committing.
4. **Replay env-determinism contract.** The V-B reducer reads `Res`/`Query` env, unlike Elm's env-free `update`. Seed env at record-start + restore at replay-start (Elm-flags model) AND add a test-time assertion that reducers are invariant to live env during replay; capture-per-fold only if it fails. (The `elm-redux-time-travel/` folder now exists — use it to settle this.)
5. **Does real-user pointer/keyboard input ALSO enqueue (complete log for real input), or does the funnel additionally record `OnPress`?** Today pointer/keyboard write `OnPress`/edits directly and never mint an `ActionRequest`; "record the `ActionRequest` stream" captures agent/AT actions only. Completeness hinges on this.
6. **Slider `A11yValue` routing through the funnel touches the AT inbound seam the agent-interface campaign depends on** (`dispatch_action_request`'s honor closures). Confirm re-routing doesn't regress the in-process driver's act-then-observe contract.
7. **Editor specifics:** clipboard as a logged effect (Paste carries resolved text vs a separate `Cmd` whose result is logged; are Cut/Copy recorded?); undo on hot-reload (`ChangeItem` mirror vs accept loss-on-reload for v1); mid-composition snapshot policy (seal-on-snapshot recommended); does the focused-editor mailbox sit on the editor entity or a global router addressing the focused entity?
8. **`Motion` enum migration:** `EditCommand` names `cosmic_text::Motion` (foreign, ~20 variants) and `Ime`/`KeyboardInput` are foreign — recording the resolved `EditCommand` needs a Buiy-owned `Reflect` `Motion` + `ImeCommand` mirror. Bounded but real.
9. **MessageBroker-style typed global cross-tree dispatch (Relm4) — needed, or do `LogicalId`-addressed enqueue + `EntityEvent` bubbling cover every cross-tree case?** Relm4 needed a global broker *on top of* tree routing; decide before composition locks.
10. **Per-frame Full/Patch/retain extract mix on an MVU-ified gallery/todomvc.** The substrate changes *what* produces the `Changed` signals; the perf design flags this as audit-open and wants it measured before sizing bind topology.
11. **Does caret-blink-as-a-Msg stay off the re-extract path under `set_if_neq`?** The audit's #6 (caret blink → full re-extract) is the canonical high-frequency widget-internal signal; the spec must demonstrate the blink Msg keeps `node_rebuilds == 0`.
12. **Docs index hygiene (process gap).** `docs/README.md` has no entry yet for this research folder or the new `druid/`, `relm4/`, `elm-redux-time-travel/` prior-art folders (left to a single index-reconciliation pass after the parallel fan-out, to avoid concurrent-edit conflicts). Fold into the spec-stage docs update.

---

## 5. Proposed spec outline

The new spec (`docs/specs/2026-06-26-buiy-mvu-core-design.md` or similar) supersedes the opt-in draft. Suggested structure:

0. **Status / supersession** — supersedes `2026-06-26-buiy-state-management-design.md`; records the placement flip (opt-in → core) and the tiered-granularity re-decide.
1. **Thesis + scoped guarantee** — pure-reducer funnel as the core; replay is complete *over the MVU-governed subtree* (not unconditional whole-UI).
2. **Goals / non-goals** — incl. non-goal: per-widget-instance actors; non-goal: Reflecting the cosmic editor.
3. **Placement** (D1) — the three-way core / core-gated / separable split; the `buiy_core::mvu` module + `CorePlugin` composition.
4. **Core model + tiered granularity** (D2) — `Model`/`Msg`/reducer; the router / stateful-leaf / machine / composite tiers; one-Msg↔one-Model invariant for machines.
5. **`PureEnv`** (D4) — sealed allowlist + `#[derive(PureEnv)]`; the editor exemption (impure routing leaf); determinism-at-the-boundary.
6. **Reducer ergonomics** (D5) — `&Env` + derive for v1; the variadic macro as a named v2 follow-up.
7. **Transport + routing** (D10 routing half) — `EntityEvent` bubble to nearest model owner, two-hop collapse, the enqueue-only rule; `Callback<T>` + the optional `Out`-enum tier; keyed-reconcile-by-domain-id; the structural-ops-on-log decision (open Q1).
8. **The drain + change-detection discipline** (D10, PERF) — single ordered system, `set_if_neq`, the `MvuWorkCounters` gate.
9. **Schedule integration** (D10) — `MvuSet` folded into `BuiySet`, the pinned `ApplyDeferred`, the editor-drain-before-reshape pin.
10. **`Cmd` algebra** (D6) — none/done/task/batch/chain/sequence + keyed `Subscription`; `InFlight`/takeLatest; envelope origin tags; dead-letter; native-only `catch_unwind` supervision.
11. **Composition + focus** (D2, D-focus) — child→parent surfaces; the singleton focus-actor.
12. **Text-edit integration** (H5) — command-sourcing (`EditCommand` + `Motion` enum + `ImeCommand`); `TextEditSnapshot` logical projection for hot-reload; clipboard-as-logged-effect; the drain-ordering pin vs `reshape_edited_editors`.
13. **Record / replay** (H7) — default-OFF lazy `RecordMode`; keyframe jump strategy; replay drops Cmds + re-feeds logged effect-results; env seeding + invariance assertion; the scoped guarantee.
14. **Identity** (D7) — `LogicalId` layered over `NodeId`; resolver registry; hierarchical deterministic fallback.
15. **Agent-interface write-side unification** — action lowering through `update` for model-backed verbs; leaves stay direct; in-process driver in core, MCP transport opt-in above.
16. **Escape hatch + boundary** (D9) — the `Model` membrane; the tiered guarantee; framework funneled-state encapsulation; the debug write-outside-the-funnel auditor.
17. **Modes** — `bevy_state` for genuinely-global modes (screen router, replay gate) only.
18. **WASM posture** (WASM verdict) — the cfg-gates, bounded in-memory log, deterministic id.
19. **Perf gates** — `MvuWorkCounters`, dhat band, the net-new iai-callgrind twin.
20. **Migration plan** (D8) — the 5 phases; pilot text-edit; seam-not-rewrite; per-machine GPU+live-interaction gating.
21. **Decision log** — the re-decides vs the draft (placement, granularity, recording, editor-exemption, the perf load-bearing rule).
22. **Provenance** — this synthesis + the 10 research artifacts + the prior-art corpus.

---

## Provenance

- **Charter:** `…/state-mgmt-elm-prototype/docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md`
- **Retrospectives + draft spec:** proto-1/2 retrospectives + journals + `docs/specs/2026-06-26-buiy-state-management-design.md` (the superseded opt-in draft).
- **Research artifacts (this folder):** `interaction-focus-routing.md`, `text-edit-state-crux.md`, `widget-set-granularity-migration.md`, `a11y-agent-interface-writeside.md`, `perf-hotpath-60hz-scale.md`, `app-arch-wasm-escapehatch.md`, `prior-art-reactive-cores-state-lens.md`.
- **Prior-art folders:** `docs/prior-art/{druid,relm4,elm-redux-time-travel}/` (new) + `gpui/`, `xilem-masonry/`, `floem/`, `iced/`, `dioxus/` (existing corpus, read through the reactive-cores lens).
- **Current-main grounding:** `crates/buiy_core/src/{interaction.rs, lib.rs, a11y/*, text/edit/*, picking/*, focus.rs}`, `crates/buiy_widgets/src/{lib.rs, menu.rs, dialog.rs}`, the perf campaign infra (`render/counters.rs`, `tests/crosscut/work_counters.rs`, `benches/pipeline.rs`); main @ `5c0da9f`.
