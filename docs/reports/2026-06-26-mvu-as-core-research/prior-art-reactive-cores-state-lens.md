**Date:** 2026-06-26
**Status:** research (proto-3 / MVU-as-core, `/staged-development` RESEARCH stage)
**Subject:** Cross-system state-management comparison through the MVU/reactivity lens — what Buiy's CORE-MVU should learn from xilem, floem, gpui, iced, dioxus
**Author:** prior-art-reactive-state-lens research agent
**Inputs:** existing prior-art folders (read-only) + proto-2 retrospective + draft spec + a current-`main` `buiy_core` spot-check + 2 targeted web searches (Elm/Redux time-travel, since that prior-art folder does not yet exist)

> **Scope.** This synthesizes the *existing* Buiy prior-art corpus through one lens: how each
> system represents state, updates it, models effects, routes messages, scopes state,
> supports time-travel, and integrates with retained/immediate rendering — and what proto-3's
> **core, primary** MVU substrate should adopt or reject. It does not modify any prior-art
> folder. Code claims about external systems cite the corpus `.md` files by line; claims about
> Buiy's current core cite `crates/…` by line. Per the charter, every axis re-decides — and
> where the evidence cuts against the maximalist "MVU-as-core" framing, this report says so.

---

## 0. Bottom line up front

Five findings dominate, in priority order:

1. **The pure-reducer + effects-as-values + single ordered funnel is the *only* prior-art-supported
   path to time-travel/replay.** The signal lineage (Floem, Dioxus) forecloses it (no funnel —
   writes are scattered across cells), and the production actor lineage (GPUI) forecloses it
   (mutations are direct `&mut`, unrecorded). Buiy's choice of the Elm reducer model *as the core*
   is therefore not arbitrary; it is the one architecture from which replay is reachable. **The
   charter's headline thesis is sound, and the prior art shows *why the alternatives cannot get there*.**

2. **No prior-art system makes "every widget an actor."** GPUI — the strongest production
   retained-mode actor UI in Rust — makes only *stateful* things `Entity<T>` views; the bulk of UI
   (buttons, labels) are **stateless `Div` elements** rebuilt in `render`, holding no per-instance
   state (`gpui/element-tree.md:7-13,27-38`). This is decisive evidence for the charter's
   "leaf widgets stay imperative and only route" branch over "every widget is a Model+reducer."

3. **No prior-art system rewrote a mature imperative core into actors.** All of them *layered* the
   reactive/actor paradigm **above** an imperative toolkit (Masonry below Xilem; `floem_reactive`
   beside the view tree; `iced_core` widgets are plain values). This is direct evidence against the
   charter's highest-risk item — "redesign `buiy_core`'s interaction/focus/text-edit/a11y into MVU
   actors" (charter Advantage #4). The supported move is **seam, not rewrite**.

4. **GPUI deliberately rejects reducer purity** ("purity buys nothing further… the discipline lives
   in the effect queue", `gpui/element-tree.md:124`). Bevy's command-flush gives Buiy GPUI's
   reentrancy safety *for free* without purity. So purity's **only** load-bearing payoff in Buiy is
   the recordable log — which means purity must be **cheap and escapable** wherever replay isn't wanted.

5. **A *core, recordable* substrate is genuinely novel** — GPUI's effect queue is core but records
   nothing; Iced's `Task` is core but ships no recorder. Nobody ships "core funnel that records
   widget-internal state." That is the upside *and* the risk: higher novelty, and the single most
   important missing reference (Elm-debugger / Redux-DevTools time-travel) is a prior-art folder
   that **does not yet exist** (queued in draft §14). The two web searches below partially fill it.

The honest reframing: keep the **pure-reducer-funnel core** (it is the only road to replay), but
**temper the maximalism** — "MVU-primary authoring + complete-funnel-for-stateful-widgets + seam-not-
rewrite for the mature core," not "every widget an actor + rewrite `buiy_core`."

---

## 1. The comparison table (7 axes × 5 systems + Buiy target)

| Axis | Iced (Elm/TEA) | Xilem (view-diff) | GPUI (actor) | Floem (signals) | Dioxus (signals+VDOM) | **Buiy core-MVU (proto target)** |
|---|---|---|---|---|---|---|
| **1. Where state lives** | ONE owned global `State` struct, runtime-owned (`iced/elm-architecture.md:18-21`) | ONE `'static` `State` value, runtime-owned, passed via `ViewCtx` (`xilem-architecture.md:11,50-55`) | MANY `Entity<T>` handles into one global `App` arena (`gpui/architecture.md:55-62`) | Per-`RwSignal` cells in a generational arena, scope-bound; view tree built once (`floem/fine-grained-reactivity.md`, `architecture.md:13`) | Per-scope hook arena + `generational-box` Copy signals; Stores for nested (`dioxus/signals-and-state.md:22-48`) | **Per-entity `Model` = `#[derive(Component,Reflect)]`; `World` is the arena; true singletons in `bevy_state`** (draft §3, §8) |
| **2. Update mechanism** | Pure-ish central `update(&mut State, Msg) -> Task` (`iced/elm-architecture.md:19`) | Per-view `message` → callback mutating `&mut State`; coarse view-diff (`xilem-architecture.md:25-27`) | Mutable closures `entity.update(cx,\|s,cx\|…)`; **no purity** (`gpui/element-tree.md:117-124`) | `signal.set()` → tracked effects re-run; **no diff, no reducer** (`floem/fine-grained-reactivity.md:11-14`) | Signal write → scope dirty → fn re-run → VDOM diff → mutations (`dioxus/architecture.md:68-73`) | **Pure per-type reducer `fn(&mut M, Msg, &Env) -> Cmd`, purity = sealed `PureEnv`** (retro REDESIGN #1) |
| **3. Effects/commands** | `Task<Msg>` effects-as-VALUES (`none/done/perform/run/batch/chain/map`) + keyed `Subscription<Msg>` (`iced/elm-architecture.md:63-84`, `architecture.md:109`) | tokio baked in; tasks emit Msgs routed to view path (`xilem-architecture.md:74-75`) | Direct calls inside closures + effect queue drains at end of update (`gpui/architecture.md:60-62`) | Effects subscribe to signals; resources for async | `use_effect`/`use_resource`/`use_future`/`use_coroutine` (`dioxus/architecture.md:37-45`) | **`Cmd` = values (`none/done/task/batch`); `task`→`AsyncComputeTaskPool`+`InFlight<M>`, result folds back AS Msg; takeLatest** (draft §6) |
| **4. Routing / compose / avoid `Msg.map`** | **THE failure**: nested `Element::map`+`Task.map` at every boundary; friction at ~200 variants (`iced/elm-architecture.md:91-95`, `lessons.md:37`) | **id-path routing** (entity-as-path) + **`Adapt` lensing** (parent↔child state projection) — no context/Redux/`@Binding` (`xilem-architecture.md:29-39`) | **No central Msg enum**: per-entity typed `emit`/`subscribe`, subscriber translates (`gpui/element-tree.md:33-38`) | Shared signals + `provide_context`/`use_context` (`floem/architecture.md:95`) | Callback props (`use_callback`) + `use_context_provider`; Stores (`dioxus/signals-and-state.md`) | **`EntityEvent` bubbles `ChildOf`→nearest Model owner+enqueue (= native id-path) + Yew `Callback<T>` props (= callback-translation); explicit-address = cross-tree hatch** (retro KEEP #3,#4; draft §5) |
| **5. Per-component vs global** | Global single-Model (manual substruct split) (`iced/elm-architecture.md:88-97`) | Single root + `Adapt` slices (global-with-lensing) | Per-entity; **leaf UI is STATELESS `Div`** — actor only for the stateful few (`gpui/element-tree.md:36-38`) | Per-cell + context for shared (`floem/architecture.md:95`) | Per-scope hooks; **Stores = per-field subscription = app-scale unlock** (`dioxus/signals-and-state.md:48,95`) | **Per-entity Models (stateful widgets); leaf widgets route only; `Res` for shared; `bevy_state` for singletons** (draft §5, §8) |
| **6. Time-travel / replay** | Architecture *permits* it (pure update + `Task` values); ships no debugger. Elm/Redux are the canonical lineage (web: Elm debugger **drops Cmds** on replay) | None — coarse `&mut` view-diff is not replay-designed | **None** — direct mutation + direct effects are structurally unreplayable | **None** — scattered signal writes, no ordered funnel | None (Subsecond = code hot-reload, not state replay, `dioxus/architecture.md:93`) | **`Reflect` log + `LogicalId` → byte-identical cross-process replay; replay re-folds & DROPS Cmds** (proto-2 proven; draft §7) — matches Elm debugger exactly |
| **7. Retained vs immediate render** | Immediate-ish: `view()` rebuilds tree every event; parallel state `Tree` keyed by pos+type-id; needs `keyed`/`Lazy` (`iced/architecture.md:57-65`) | Pure-fn view → diff → mutate Masonry **retained** widgets (`xilem-architecture.md:11,19-21`) | Hybrid: dirty-view rebuild + immediate paint into GPU cmd list, **no diff**, idle-cost-zero (`gpui/element-tree.md:27-30,135`) | Fully **retained** view tree built ONCE; signals do surgical updates (`floem/architecture.md:13`) | VDOM diff → mutation stream → retained backend nodes (`dioxus/architecture.md:15-19`) | **ECS-retained entities (entity = identity, no reconcile tree); `bind`(`Changed`+`set_if_neq`) = surgical update; keyed reconcile only for dynamic child lists** (draft §5) |

---

## 2. System profiles through the state lens (one paragraph each)

- **Iced / TEA** — the purest Elm: one global `State`, one central `update`, `Task<Msg>` effects-as-
  values, `view()` rebuilt every event. It is simultaneously Buiy's **strongest positive reference**
  (effects-as-values, `Task` algebra, `Subscription`) and its **sharpest cautionary tale** (the
  global-Model + central-`Message`-enum + `Msg.map` tax that the charter explicitly wants to escape —
  `iced/lessons.md:37`, `iced/elm-architecture.md:149`).

- **Xilem** — the closest published "next-gen Rust UI substrate" (`xilem/lessons.md:9`). Its two
  inventions — **id-path message routing** and **`Adapt` lensing** — are *exactly* the pair Buiy's
  proto independently re-derived (`EntityEvent` bubbling + `Callback` translation). Xilem flags the
  cost: "every view has to participate in routing" (`xilem-architecture.md:33`) — which Buiy's
  native propagation engine offloads, a genuine improvement. Coarse view-diff, not replay-oriented.

- **GPUI** — the strongest production datapoint and the most instructive *contrarian*. `App` +
  `Entity<T>` + effect-queue is "uncannily" the same shape as Bevy `World`+`Entity`+observers
  (`gpui/lessons.md:11-17`), so Buiy gets the ownership model free. But GPUI **rejects purity**,
  ships **no recorder**, makes **leaf widgets stateless**, and virtualizes high-cardinality lists
  outside the actor model — three direct rebuttals to the maximalist reading of the charter.

- **Floem** — fine-grained signals (Solid→Leptos→`floem_reactive` lineage, `floem/fine-grained-
  reactivity.md:32-39`). `O(changed)` not `O(tree)`. The crucial lesson for proto-3 is *negative*:
  signals **have no ordered funnel**, so they are structurally incompatible with a complete replay
  log. Floem is the reference for "what Buiy is NOT choosing, and why."

- **Dioxus** — signals over a VDOM. Two transferable lessons: **Stores** (per-field subscription) is
  "the difference between signals demo well and signals work at app scale" (`dioxus/signals-and-
  state.md:95`), and the **diamond-dependency / write-O(subscribers)** fan-out cost
  (`dioxus/signals-and-state.md:84-90`) — both warnings about granularity that map onto Buiy's
  `bind` fan-out.

---

## 3. Per-axis deep dives + what core-MVU should LEARN

Each axis ends with a recommendation, rationale, and the runner-up rejected.

### Axis 1 — Model representation (where app state lives)

**Spectrum.** One global value (Iced, Xilem) ↔ many typed handles (GPUI) ↔ scattered reactive cells
(Floem, Dioxus). Buiy sits with **GPUI** (per-entity, global arena = `World`), validated by
`gpui/lessons.md:11-17` ("Bevy's ECS provides the same semantics… the expensive part of GPUI's design
is free for Buiy"). The single-global-Model end is *disproven at scale*: `iced/lessons.md:37` — "adding
a field to the global Model means touching every `update` arm," friction at ~200+ variants.

**LEARN.** Per-entity decomposition is the right base. But note the one capability the typed-handle
camps (GPUI `Entity<T>`, Iced typed `State`) have that bare ECS lacks: **type-level association of
state↔behavior**. Buiy's proto recovers this with the "one `Msg` type ↔ exactly one `Model` type"
invariant (draft §3) — keep it; it is what makes routing unambiguous and is GPUI's per-entity-typed-
event shape in ECS clothing.

- **Recommendation:** per-entity `Model` components, **never** a god-Model; enforce one-Msg↔one-Model.
- **Rationale:** the only camp that scales to thousands of distinct widgets without a central enum;
  matches the substrate (ECS) Buiy already committed to.
- **Runner-up (rejected):** Iced/Xilem single-root-State with lensing — rejected because a complete
  *widget-internal* log (the whole point of core-MVU) requires leaf state to be addressable per
  entity, not hidden inside a coarse parent Model.

### Axis 2 — Update mechanism (reducer / closure / signal / diff)

**The decisive contrast is GPUI vs Elm on purity.** GPUI: "The Elm Architecture's purity is gone…
purity buys nothing further; mutation is just easier. The discipline lives in the effect queue, not in
functional purity" (`gpui/element-tree.md:124`). GPUI's run-to-completion effect queue
(`gpui/architecture.md:60-62`) prevents reentrancy without purity — and **Bevy's command-flush gives
Buiy the same guarantee for free** (`gpui/architecture.md:77`).

So the proto's pure reducer (sealed `PureEnv`, retro REDESIGN #1) cannot be justified on
safety/reentrancy grounds — GPUI proves you don't need it for that. **Its sole load-bearing payoff is
deterministic replay** (Axis 6). This is a sharpening, not a refutation: purity stays, but its
cost/benefit is now explicit — for any widget you don't record, purity is pure tax.

**LEARN.** Because purity's only payoff is the log, the **recording opt-out and the raw-ECS escape
hatch must be first-class** (charter perf + escape-hatch risks). Don't enforce purity globally on a
hot path that nobody replays.

- **Recommendation:** keep the pure reducer as the **default authoring surface**, but make
  "unrecorded / raw-ECS" a blessed, documented mode — not a hack. Frame purity to users as "the price
  of replayability," charged only where replay is wanted.
- **Rationale:** GPUI is the existence proof that a production actor UI needs no purity for safety;
  the replay thesis is the *only* thing that justifies it, so scope it to where replay is the goal.
- **Runner-up (rejected):** GPUI-style mutable closures as the core update — rejected because it
  forecloses replay (Axis 6), which is proto-3's entire reason to be.

### Axis 3 — Effects / commands model

Iced's `Task<Msg>` is the canonical "effects are **values** the framework executes, not callbacks it
calls" (`iced/elm-architecture.md:84`). Buiy's `Cmd` (draft §6) is squarely this model. Two things the
prior art shows Buiy is **missing** relative to the reference:

1. **`Subscription<Msg>` for long-lived external sources** (timers, OS events, file watchers,
   websockets), lifecycle-managed by subscribe/unsubscribe hash (`iced/architecture.md:109`). This is
   the charter's open "+ stream?" question — and it is **load-bearing for replay**: a timer- or
   IME-driven UI is *unreplayable* unless every tick/composition event enters the funnel as a logged
   Msg. Without a subscription primitive, those Msgs originate outside the funnel and the log is
   incomplete (the Axis-6 completeness trap).
2. **The full algebra** — Iced ships `none/done/perform/run/batch/chain/map`; proto has
   `none/done/task/batch` and defers `sequence`/`stream` (draft §3). `chain` (sequencing) and the
   `Subscription` stream are the gaps.

The **effects-as-values choice is what makes replay possible**, and the web search nails the
mechanism: Elm's debugger **does not replay Commands** — because (a) re-running a side effect during
replay is unsafe (it could "change a database"), and (b) "Commands are generating Messages, so if we
replay both… the same messages will arrive two times." Buiy's draft §7 ("ReplayMode re-folds but
**drops all Cmds**; every consequent Msg is itself a later log entry") is **identical** to Elm's
actual semantics. Strong validation — and only reachable because effects are values you can suppress.

- **Recommendation:** adopt Iced's full algebra as the target — `none/done/task/batch` **+ `chain`/
  `sequence` + a keyed `Subscription`-equivalent** for long-lived sources — with the hard invariant
  that *all* effects are values, suppressible in replay. Add the charter's dead-letter (loud, typed)
  + `catch_unwind` reducer supervision as core concerns layered on the drain.
- **Rationale:** effects-as-values is the precondition for "drop Cmds on replay" (proven necessary by
  both Elm and Redux); the subscription primitive is the precondition for *completeness* of the log.
- **Runner-up (rejected):** GPUI direct-call effects inside update closures — rejected: a direct call
  cannot be suppressed or re-fed in replay, breaking determinism.

### Axis 4 — Message routing & parent-child composition (avoiding `Msg.map`)

This is the axis where the charter's "no god-Model, no `Msg.map`" thesis lives, and the prior art is
unusually clear. **Iced is the negative**: nested `Element::map(SubMessage → Message)` + `Task.map` at
every composition boundary, "verbose and refactor-hostile" (`iced/lessons.md:37`,
`iced/elm-architecture.md:91-95`). **Xilem and GPUI are the two positives**, and Buiy's proto
independently converges on *both*:

- **Xilem id-path routing** (`xilem-architecture.md:29-34`): every view has an `Id`; events route up
  the root→leaf path; each ancestor's `message` can consume/transform with mutable access to *its*
  level of state — "components compose without forcing the root state shape to know about the leaf's
  data." Buiy's `EntityEvent` bubbling `ChildOf` to the nearest Model owner (retro KEEP #3) **is**
  id-path routing with the entity as the path leaf — and Bevy's propagation engine rewrites the event
  target each bubble step, so widgets **don't hand-write routing** (Xilem's stated cost,
  `xilem-architecture.md:33`, is offloaded). This is a concrete *improvement* over Xilem to claim.
- **Xilem `Adapt` lensing** (`xilem-architecture.md:36-39`): pure-functional parent↔child state
  projection — "the reason Xilem doesn't need React context, Redux selectors, or SwiftUI `@Binding`."
  In ECS, **a `Query` is a lens** (`xilem/lessons.md:71-72`). Buiy gets `Adapt` for free; name the
  equivalence so designers reach for queries instead of reinventing selectors.
- **GPUI per-entity typed events** (`gpui/element-tree.md:33-38`): no central Msg enum at all;
  subscriber translates. Buiy's one-Msg↔one-Model + Yew `Callback<T>` props (retro KEEP #4, draft §5)
  is the same — the parent maps the child's payload to *its own* Msg locally, "one closure per
  adoption edge, not per nesting depth." **This is exactly how Buiy avoids `Msg.map`.**

- **Recommendation:** entity-tree routing (native id-path via `EntityEvent` propagation) for upward
  events **+ typed `Callback<T>` props** for child→parent translation, **+ explicit-address dispatch**
  as the cross-tree escape hatch (a toolbar button targeting a distant panel is not hierarchical),
  **+ Query-as-lens** for projected reads. Document the Query≡`Adapt` equivalence.
- **Rationale:** this is the union of the *two* mechanisms the best-regarded systems use to escape the
  Iced `Msg.map` tax; the proto already has both, and Bevy makes the routing cheaper than Xilem's.
- **Runner-up (rejected):** Iced nested-enum + `Msg.map` — rejected on its own documented scaling
  failure.

### Axis 5 — Per-component vs global state

The charter's hardest open question — "does *every* widget become a Model+reducer, or do leaf widgets
stay imperative and only route?" — is answered most directly by **GPUI**: only stateful things are
`Entity<T>` views; **the bulk of UI (buttons, labels) are stateless `Div` elements composed in
`render`, holding no per-instance state** (`gpui/element-tree.md:7-13,27-38`). Editors/lists/trees get
custom stateful `Element`s; everything else is stateless. **GPUI is "actor for the stateful few,
stateless for the many."** Combined with its high-cardinality story (UniformList/List virtualize,
paying for visible rows only, `gpui/architecture.md:22-28`), this is decisive: a production actor UI
does **not** instantiate a Model+mailbox per widget.

Dioxus adds the granularity lesson: per-field **Stores** are "the application-scale unlock"
(`dioxus/signals-and-state.md:95`); coarse whole-component subscription doesn't scale. The ECS-native
answer is already idiomatic: **keep Models small/decomposed** so Bevy's `Changed<T>` is naturally
per-field-equivalent — no Store machinery needed (`dioxus/signals-and-state.md:79` names the hard case
as "subscribe to one field on one component," which decomposed components dissolve).

- **Recommendation:** **per-entity Models for stateful widgets only; leaf/presentational widgets stay
  stateless and merely route** (the GPUI model). True singletons (screen router, replay gate) use
  `bevy_state`. Keep Models decomposed so `Changed<T>` is fine-grained. High-cardinality widgets
  (tables, virtualized lists) stay imperative and route — never one Model per row.
- **Rationale:** the only prior-art datapoint at production scale (GPUI/Zed) rejects per-widget
  actors; the charter's own SCALE risk (mailbox/model overhead at thousands of widgets) points the
  same way; Bevy's `Messages<M>` is one buffer *per Msg type*, not per entity (retro KEEP #1), so
  10k buttons share one drained buffer — scale-friendly *because* not every widget is an actor.
- **Runner-up (rejected):** "every widget is a full Model+reducer+mailbox" — rejected on GPUI's
  evidence and the scale arithmetic. **This is the single biggest charter-tempering finding.**

### Axis 6 — Time-travel / replay support

This is where MVU-as-core earns its keep, and where the prior art is sharpest by **exclusion**:

- **Pure-reducer lineage (Elm/Iced/Redux) is the *only* one that supports time-travel.** Web evidence:
  Redux time-travel "relies on reducers being pure functions… re-running the reducers with the previous
  actions"; effects must be isolated in middleware, not the reducer. Elm: "because your app is a set of
  pure functions on immutable data, and the runtime handles all managed effects by returning Cmds, a
  time-traveling debugger is a natural thing to build." **Both require: pure update + effects as values
  + an ordered action/Msg log.** That is exactly Buiy's funnel.
- **The signal lineage forecloses it.** Floem/Dioxus writes are *scattered across cells* with no single
  ordered funnel (`floem/fine-grained-reactivity.md:11-14`) — there is no Msg stream to replay. This is
  the architectural reason Buiy must **not** choose signals for the core if replay is the goal.
- **The mutable-closure actor lineage forecloses it.** GPUI's direct `&mut` mutations and direct effect
  calls are unrecorded and non-suppressible — GPUI ships no replay, and *cannot* without rearchitecting.

Buiy's proto-2 already **proved** byte-identical cross-process replay via `Reflect` log + `LogicalId`
(retro KEEP #5), and its replay design (re-fold, **drop Cmds**, draft §7) matches Elm's debugger
exactly. The thesis holds.

**Two completeness risks the prior art surfaces, that the charter must resolve:**

1. **The side-channel problem (the `TextEditState` crux, concretely).** Replay is complete *only* if
   nothing mutates state outside the funnel. Floem is the evidence that scattered side-channels make a
   log incomplete. In current `buiy_core`, `TextEditState` is `#[derive(Component)]` **without
   `Reflect`** (`crates/buiy_core/src/text/edit/state.rs:92-93`) — invisible to a `Reflect` log — and
   the in-process driver lowers actions **directly** into `OnPress`/`FocusedEntity`/`EditCommand` sinks
   (`crates/buiy_core/src/a11y/inprocess.rs:363-373`), bypassing any Msg path. These are real
   side-channels today. Making the substrate core only delivers replay if these are routed through the
   funnel (their `EditCommand`s become recorded Msgs) and the state becomes `Reflect`-snapshottable.
2. **Ambient-env determinism.** Elm's `update` reads **no ambient environment** — all inputs are Msgs
   or init flags — which is *why* re-running it is deterministic. Buiy's V-B reducer reads `Res`/`Query`
   env (draft §3). The draft's answer is "env reads are reconstructed/seeded at replay-start, like Elm's
   init flags." This is a **deviation from pure Elm** and weakens the guarantee unless env is either
   proven replay-invariant or captured per-fold. This is the highest-value question for the **missing
   Elm/Redux time-travel prior-art folder** (draft §14) to settle.

- **Recommendation:** keep the pure-reducer-funnel as the core's *raison d'être*, but **scope the
  guarantee explicitly**: "deterministic replay holds for state reached through the funnel; escape-hatch
  / opt-out state is outside the replay boundary and is reconstructed at replay-init (Elm-flags model)."
  Resolve env-determinism by seeding env at init **and** asserting (test-time) that reducers are
  invariant to live env during replay; capture-per-fold only if that assertion fails.
- **Rationale:** an unscoped "complete log incl. all widget-internal state" claim is unachievable the
  moment any escape hatch exists (Floem's lesson); a *scoped* guarantee is both honest and sufficient
  for the deterministic-test / time-travel / agent-driving payoffs.
- **Runner-up (rejected):** signal-based reactivity for the core — rejected precisely because it
  forecloses replay (no funnel). This is the cleanest argument in the whole report for MVU-over-signals
  *as the core*.

### Axis 7 — Integration with retained vs immediate rendering

The spectrum runs immediate-rebuild (Iced `view()` every event, needing a parallel keyed state `Tree`,
`iced/architecture.md:57-65`) → diff-into-retained (Xilem→Masonry, Dioxus→VDOM) → **build-once +
surgical** (Floem signals). Buiy's ECS sits at the **Floem end**, but reaches it via *entity retention*
rather than signals: entities persist, the entity **is** the identity, so **no reconciliation tree is
needed** — `iced/lessons.md:57` names this as "a real ergonomic advantage worth naming explicitly." The
proto's `bind` (`Changed`-gated, `set_if_neq`, draft §5) is the surgical update.

**LEARN.** The "V" in MVU should **not** be an Elm/Iced per-frame `view(model) → tree` rebuild. That
model forces reconciliation and *double-diffs against ECS change detection*
(`floem/fine-grained-reactivity.md:60`: "Bevy ECS already runs change detection; layering React-style
coarse re-render on top would double-up"). Buiy's View is persistent entities + `Changed`-gated binds —
Floem's shape, ECS-mechanism. GPUI's idle-cost-zero ("no dirty view = no render call",
`gpui/element-tree.md:135`) maps onto `Changed`-gating and is *also* the performance mitigation: a
widget that received no Msg does no fold, trips no bind, pays no record cost.

The **one** place even ECS needs reconciliation is dynamic child collections: the proto's keyed
reconcile **by domain id** (draft §5) is exactly Iced's `keyed::column` (`iced/elm-architecture.md:137`)
and React keys — borrow it for add/remove/reorder; everything else stays retained.

- **Recommendation:** View = persistent entities + `Changed`-gated `bind`/derived systems (Floem-shape,
  ECS-mechanism); **explicitly reject** Elm/Iced per-frame view rebuild; borrow keyed-by-stable-id
  reconcile only for dynamic lists. Treat `Changed`-gating as the idle-cost-zero + 60Hz-floor mitigation.
- **Rationale:** avoids the reconciliation tax (Iced) and the double-diff (Floem's warning) while
  keeping the surgical-update asymptotics; aligns the record cost with actual Msg traffic.
- **Runner-up (rejected):** Elm `view(model)->Element` rebuild — rejected (reconciliation tax + redundant
  with ECS change detection).

---

## 4. Cross-cutting: the charter's hard risks, what the prior art says

| Risk (charter) | What the prior art evidences | Candidate mitigation |
|---|---|---|
| **PERFORMANCE — 60Hz floor; Reflect-serialize on hot path; thousands of widgets** | GPUI idle-cost-zero = only dirty views work (`gpui/element-tree.md:135`); Iced "`view()` must be cheap"+`Lazy` (`iced/elm-architecture.md:136`); Dioxus write-`O(subscribers)`+diamond fan-out (`dioxus/signals-and-state.md:84-90`). The record/Reflect cost is **Buiy-specific, unbenchmarked in the corpus**. | Record Msgs as **typed values in a `Vec`** on the hot path; **`Reflect`-serialize only at snapshot/persist boundaries**, never per fold. `Changed`-gate the funnel so only widgets that received Msgs pay. Recording opt-out/sampling per the charter. Gate with iai-callgrind (hw-independent). |
| **MIGRATION — `buiy_core` mature (~43k LOC, verified)** | **Every** prior-art system *layered* the paradigm above an imperative toolkit; **none** rewrote a mature core into actors (Masonry⊥Xilem; `floem_reactive`⊥views; `iced_core` widgets are values). | MVU as the **primary authoring surface** over the existing imperative core; keep interaction/focus/text-edit as imperative systems that **route into** the funnel. **Seam, not rewrite.** Treat charter Advantage #4 ("reshape core into MVU actors") as the highest-risk, lowest-support item — pilot on ONE subsystem before generalizing. |
| **ESCAPE HATCH — must allow raw ECS** | Universal: GPUI `Element` trait (`gpui/element-tree.md:40-61`), Xilem-via-Masonry custom widgets, Iced custom `Widget`. Every serious system ships an imperative escape. | Raw Bevy systems/queries are a **first-class, documented** escape; their state is explicitly **outside the replay boundary** (Axis 6 scoping). The need is unanimously validated; the only cost is replay-completeness, which scoping handles. |
| **WASM — zero new obstacles** | `Reflect` is already used pervasively in `buiy_core` (a11y/text components); RON/serde_json already transitive (retro: "no lockfile churn"). Dioxus/Iced/Floem all ship their state models on wasm. | The funnel is CPU + `Vec`; `Reflect` is `bevy_reflect` (wasm-clean). Keep replay **in-memory** by default; persistence is opt-in (no fs on wasm). **Low risk** — no red flag in the corpus. |
| **SCALE — per-entity model+mailbox+drain at thousands** | GPUI virtualizes (UniformList/List, visible-rows-only, `gpui/architecture.md:22-28`) and keeps leaves stateless. `Messages<M>` is one buffer **per type**, not per entity (retro KEEP #1). | **Don't** make every widget an actor (Axis 5). Virtualized/uniform widgets stay imperative and route. One Msg buffer per type means 10k buttons share one drain. The scale risk is real **only** for the maximalist reading. |

---

## 5. Where the evidence cuts AGAINST the charter (not a rubber stamp)

1. **"Every widget a Model+reducer" is unsupported.** GPUI — the one production actor UI — makes leaf
   widgets stateless and virtualizes lists (Axis 5). Resolve the charter's granularity question to
   **"actor for the stateful few; route-only for the many."**
2. **"Redesign `buiy_core` into MVU actors" (Advantage #4) is the riskiest, least-supported item.**
   No prior-art system rewrote a mature imperative core; all layered above. Recommend **seam-not-
   rewrite**, piloted on one subsystem (text-edit, since it is the `TextEditState` crux) before any
   generalization.
3. **Purity is not justified by safety.** GPUI proves reentrancy safety without it; Bevy's command-
   flush already provides it. Purity's *only* payoff is the log — so make it cheap and escapable, and
   don't sell it to users as a safety feature.
4. **A *core recordable* substrate is novel — nobody ships it.** GPUI's queue records nothing; Iced's
   `Task` has no recorder. The upside (complete log) is real, but so is the risk of being first, and
   the most load-bearing reference (Elm-debugger / Redux-DevTools) is a prior-art folder that **does
   not exist yet** (draft §14). Create it before the spec freezes the replay/env-determinism contract.
5. **"Complete log incl. all widget-internal state" is unachievable unscoped.** Floem shows scattered
   side-channels break completeness; the escape hatch *is* a side-channel by design. The honest claim
   is a **scoped** guarantee (funnel-reached state), not "complete."

The charter's core bet — pure-reducer-funnel as the road to replay — survives all of this intact and is,
in fact, *strengthened* (it is the only road). What needs tempering is the maximalist packaging around
it.

---

## 6. Open questions for the next stage (spec)

- **Env-determinism contract for replay** (Axis 6 risk 2): seed-at-init + invariance assertion, vs
  capture-per-fold? Needs the missing Elm/Redux time-travel prior-art folder to settle.
- **Subscription primitive** (Axis 3): does proto-3 ship an Iced-style keyed `Subscription<Msg>` for
  long-lived sources (required for replay completeness of timer/IME/OS-event-driven UIs)? The proto
  currently has none.
- **Which ONE subsystem pilots seam-not-rewrite?** Recommend text-edit (it is the `TextEditState`
  crux); prove the seam (route `EditCommand`s through the funnel + make state `Reflect`-snapshottable)
  before touching interaction/focus/a11y.
- **Hot-path record representation** (Axis 4 of risks): confirm typed-`Vec` record + deferred
  `Reflect`-serialize meets the 60Hz floor under iai-callgrind at ~1k active widgets.
- **`LogicalId` ↔ agent-interface test-id unification** (charter): one identity space — needs a joint
  decision with the agent-interface campaign (out of this report's scope).

---

## Sources

**Prior-art corpus (read-only, current `main`):**
- `docs/prior-art/xilem-masonry/xilem-architecture.md`, `lessons.md`
- `docs/prior-art/floem/fine-grained-reactivity.md`, `architecture.md`, `lessons.md`
- `docs/prior-art/gpui/architecture.md`, `element-tree.md`, `lessons.md`, `critiques-and-open-problems.md`
- `docs/prior-art/iced/elm-architecture.md`, `architecture.md`, `lessons.md`
- `docs/prior-art/dioxus/signals-and-state.md`, `architecture.md`

**Buiy seeds (state-mgmt-elm-prototype worktree):**
- `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md`
- `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`
- `docs/specs/2026-06-26-buiy-state-management-design.md`

**Current-`main` core spot-check (this worktree):**
- `crates/buiy_core/src/text/edit/state.rs:92-93` (`TextEditState` is `Component` **without** `Reflect`)
- `crates/buiy_core/src/a11y/inprocess.rs:363-373` (in-process driver lowers directly into
  `OnPress`/`FocusedEntity`/`EditCommand` sinks — a current side-channel)
- `crates/buiy_core` ≈ 43k LOC (maturity / migration-cost evidence)

**Web (top-up for the not-yet-created Elm/Redux time-travel folder):**
- [Why Redux Needs Reducers to Be "Pure Functions" — Bomberbot](https://www.bomberbot.com/javascript/why-redux-needs-reducers-to-be-pure-functions/)
- [Time Travel made easy — elm-lang.org](https://elm-lang.org/news/time-travel-made-easy)
- [A time-traveling full-stack test debugger — elm-pages.com](https://elm-pages.com/blog/full-stack-test-debugger/) (Elm debugger **does not replay Cmds**: unsafe + double-message)
</content>
</invoke>
