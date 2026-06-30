# Buiy — Elm-bevyified State Management — Prototype Dev Journal

> **PROTOTYPE — exploratory, DO NOT MERGE.** The deliverable is this journal + the
> retrospective, not the code. The code is an unmerged reference for the
> human-gated final design.

**Goal:** learn whether "The Elm Architecture, bevy-ified" is a sound, ergonomic,
easy-to-reason-about state surface for Buiy by *building and running* it on Bevy
0.19 ECS — then feed a keep/refine/redesign retrospective into a human-gated
final brainstorm.

**Worktree:** `worktree-state-mgmt-elm-prototype`, cut fresh off `origin/main`
@ `59cd50e`.

**Method:** `prototype-first-development` (throwaway → retrospective → human gate;
no Phase B / final spec without the human). RUN the artifact every wave.

---

## The proposed surface under test (survives context loss)

- **Model** = the entity's component(s), distributed & queryable — NOT one global
  struct. Many small MVU loops, one per stateful entity, composed by the ECS tree.
- **Msg** = a per-model `#[derive(Message)]` enum.
- **update** = `fn update(&mut self, msg: &Msg) -> Cmd<Msg>` (mutate-in-place;
  Elm semantics, ECS backing).
- **view** = a retained `bsn!` tree with `bind!(Prop <- Model, fn)` + `on_press(Msg)`
  — NO view-rebuild/diff.
- **Cmd** = effects-as-values (`Cmd::task(future, Msg)`) on `AsyncComputeTaskPool`,
  folded back as a Msg to the originating entity.
- **Wiring** = `app.add_model::<M>()`; `on_press(msg)` routes to the nearest
  ancestor model of that Msg type. Composition = the entity tree (no `Msg.map`).
- **Debuggability** = a recordable `Msg` stream → replay / deterministic tests /
  LLM-agent driving (via the existing in-process inspection driver).

## Open questions the prototype must resolve by RUNNING (Q1–Q7)

- **Q1 Routing** — how `on_press(Msg)` reaches the nearest ancestor Model of that
  Msg type (type-directed walk vs explicit target vs observer bubbling).
- **Q2 Composition** — do many small per-entity loops actually scale, or do they
  reproduce Elm's nesting pain?
- **Q3 Parent↔child** — child→parent (OutMsg/output) without `Msg.map`.
- **Q4 Effects** — is `Cmd::task(future, Msg)` the right shape; how results fold
  back to the right entity; cancellation on despawn.
- **Q5 View** — retained tree + `bind!` (no diff) vs a `Changed`-gated rebuild;
  where no-diff bites.
- **Q6 Dynamic collections** — add/remove/reorder of child models (todo list)
  without a VDOM.
- **Q7 Record/replay** — minimal recorder to prove replay + deterministic tests +
  agent-driving.

---

## Running log

### 2026-06-26 — Wave 0 (setup)
- Built: worktree off `origin/main@59cd50e`; this journal; launched the focused
  Elm/MVU-on-ECS prior-art deep-dive (workflow `w250lpnkx`) to refine the surface
  + scope the spike before building.
- Ran the artifact → n/a yet (no code).
- Surprised by / friction: n/a.
- If we did this again: n/a.
- Next: on research landing, lock the minimal spike scope (likely: `Model` trait +
  `add_model` reducer wiring + `on_press` routing + `bind!` + `Cmd::task`), then
  build demos for the hard cases (counter → dynamic todo list → async search) and
  RUN each.

### 2026-06-26 — Wave 0.5 (research landed → design refined, scope locked)
- Built: nothing yet; the Elm/MVU deep-dive (6 legs) landed and **sharpened the
  design**. Key refinements baked into the spec under test:
  - **Each widget = an actor; Bevy IS the actor runtime** (GPUI/Zed production proof).
  - **HARD INVARIANT: one Msg type ↔ exactly one Model type** — this is what makes
    ancestor routing well-defined.
  - **Routing (Q1) refined**: not raw "nearest-ancestor-of-type search" — resolve
    to a *concrete Entity address* (actor-model: Akka/GPUI/Relm4 all route to an
    explicit address). Ancestor-walk/observer-bubble is *authoring sugar* over an
    explicit address; build BOTH variants and feel where the walk's reparent/
    ambiguity bites. Unhandled Msg = loud typed dead-letter, never silent.
  - **`update(&mut self, msg) -> Cmd` is a PURE boundary**: mutate ONLY its own
    model; NO spawn/despawn/sibling-write/Commands inside. ECS makes violating it
    tempting; that boundary is what unlocks replay/tests/agent-driving.
  - **Effects = descriptor enum** (`none/done/task/stream/batch/sequence/structural`),
    a VALUE not a World-thunk. `task` stores `Task<T>` as `InFlight<Msg>` on the
    originating entity (entity = fold-back address); drop=cancel free (Bevy 0.19).
    Fold result back THROUGH `update` so `Changed<Model>` trips `bind!`.
  - **Run-to-completion**: single ordered drain; folded Msgs drain NEXT frame
    (bounded, replay-friendly). Bevy does NOT give this free (observers recurse).
  - **Parent↔child = two tiers**: translator-by-default (parent authors child →
    `on_press(ParentMsg)` routes up, zero plumbing — the real win) + OutMsg for
    self-contained composite widgets (one translation per adoption edge).
  - **Dynamic collections (Q6)**: keyed reconcile by DOMAIN id (TodoId); Entity =
    identity (subsumes Relm4 DynamicIndex / Iced keyed). add=spawn/remove=despawn/
    reorder=move-entity (preserves transient state).
  - **Record/replay (Q7)**: `MsgLog` of (LogicalId, Msg, seq) tapped at the single
    drain; `ReplayMode` suppresses Cmds; re-fold from init. Self-contained because
    effect results fold back AS Msgs (beats Elm-drops-Cmds + Redux-re-runs-effects).
- Locked spike scope: throwaway `examples/mvu_spike/` crate (own MVU runtime) +
  3 demos (nested counters → routing; TodoMVC → keyed collection + derived footer +
  OutMsg composite + purity; async search → effects/cancellation) + a record/replay
  prove-the-loop test. DEFER: styling, full bsn!, MCP/serialization, supervision, GPU.
- Prior-art actions flagged (for after the gate): NEW `docs/prior-art/relm4/`
  (highest value), capture Elm/Redux time-travel, refresh `iced/` (0.14 devtools +
  Component-trait deprecation), optional `gpui/actor-model.md`.
- Ran the artifact → n/a (no code yet).
- Next: **Wave 1** dispatched — runtime core (`Model`/`Cmd`/`Inbox`/drain/`add_model`/
  `on_press` routing both variants/`bind`/`MsgLog`) + nested-counters demo + headless
  routing tests to green. Resolves Q1.

### 2026-06-26 — Wave 1 (runtime core + nested-counters routing)
- Built (`examples/mvu_spike/`, throwaway): the MVU runtime — `Model` trait
  (`update(&mut self, Msg) -> Cmd`), `Cmd{None,Done,Batch}`, `Inbox<M>`, single
  ordered `drain::<M>` (folds `Done` to NEXT frame), `route_on_press::<M>`
  (ancestor-walk) + `route_on_press_to::<M>` (explicit address), `bind_text::<M>`
  (Changed-gated), `MsgLog`, `add_model::<M>`, `MvuPlugin`. Nested-counter demo
  (≥3 levels) + 7 headless tests.
- **Ran the artifact (verify-don't-trust):**
  - Independently re-ran `cargo test -p mvu_spike` → **7/7 green** ✓.
  - Ran the GUI on `:0` (real RX 6700 XT): render world **boots clean** — Vulkan
    adapter init, window created, GPU preprocessing supported, **no panic/crash** ✓.
  - **BUT** `B0004` warnings → ROOT CAUSE: the demo tree is built from **bare
    entities** (`spawn(())`, `spawn(OnPressMsg)`, `spawn((Text,BoundText))`,
    `spawn(Counter)`) — **no `Node`, not real Buiy widgets**. So nothing lays out
    (window = clear-color only); the "buttons" aren't real `Button`s so they'd never
    emit `OnPress` via picking; tests bypassed this by writing `OnPress` directly.
    **State loop validated; view-integration NOT.** (The "always RUN the GUI" lesson,
    live again — green tests hid an empty window.)
- **Q1 ROUTING — RESOLVED (logic):** ancestor-walk (reads `ChildOf` upward,
  per-press, live, no cache) resolves unambiguously across 3 levels; skip-immediate-
  parent→grandparent works; **reparent re-resolves tick-exact** *because* the walk
  reads `ChildOf` (not `Children`, which lags to `PostUpdate`). Explicit-address is
  the clean cross-subtree escape hatch. Verdict: **ancestor-walk = right default**
  (matches HTML bubbling + author mental model); explicit-address for cross-tree.
- Surprised by / friction:
  - Bevy 0.19 `Component<Mutability = Mutable>` bound is required for `Query<&mut M>`
    in generic systems (non-obvious; `#[derive(Component)]` satisfies it). Must be in
    the `Model` supertrait bound.
  - The build agent optimized for **green tests → a logic-only harness with no real
    rendering**; the GUI gate caught the empty window.
  - `bind_text` does an ancestor-walk / `HashSet` per changed model per frame →
    **O(n²) risk at scale**; fix = store the model `Entity` on `BoundText`.
  - `Cmd::Done` folds NEXT frame (bounded, good) — submit→clear patterns must verify
    `Batch` atomicity.
- Framework BUGS: none in buiy/bevy. `B0004` is OUR tree-authoring (model entities
  aren't `Node`s).
- If we did this again: **build the demo from REAL widgets from the start**; put
  `OnPressMsg<M>` ON real `Button` entities so the real `OnPress`/picking path is
  exercised, not a bare-entity stand-in.
- Next: **Wave 2 = TodoMVC from REAL buiy widgets** (reuse the gallery's
  `button()`/`text_input_single_line()`/`checkbox()` construction, driven by the MVU
  runtime, NOT the gallery's retained-mode plugin). Extend the runtime: a sanctioned
  **structural path** (keyed reconcile by `TodoId`, spawn/despawn real rows OUTSIDE
  `update`) + a **derived-footer query** ("N items left"/"all completed?") +
  enforce the **`update` purity boundary** (Add/Toggle/Destroy/ClearCompleted mutate
  only `TodoList.items`). MUST render real content (no `B0004` on the authored tree)
  AND a real/driven click must move a Msg through routing. Resolves Q2/Q6 + purity;
  closes the Wave-1 view-integration gap. (OutMsg composite edit-in-place + reorder-
  transient-state = Wave 2b.)

### 2026-06-26 — Wave 2 (TodoMVC with real Buiy widgets)

- **Built** (`examples/mvu_spike/src/todomvc.rs`, `src/bin/todomvc.rs`,
  `tests/wave2.rs`):
  - `TodoList` model (`items: Vec<Todo>`, `filter: Filter`, keyed by `TodoId`), pure
    `update()` returning `Cmd::none()` — no ECS access inside.
  - `reconcile_rows` exclusive system: diffs `TodoList.items` vs `RowKey` children,
    spawns real `checkbox(text)` + `button("×")` rows via `world.spawn_scene(...)`,
    despawns stale rows, syncs `A11yToggled`+`CssVisibility` — ALL structural ECS
    mutation lives here, outside `update`.
  - Filter buttons (`All`/`Active`/`Completed`) and `Clear completed` button via real
    `button(...)` scenes with `OnPressMsg<TodoList>`.
  - `update_footer` system: reads `Changed<TodoList>`, writes "N items left" to a
    `FooterText`-marked `Text` entity (the "derived via query" Q2 path).
  - `route_add_submit`: routes `EditSubmitted` from the real text-input to `Add(text)`
    in the `Inbox<TodoList>`.
  - 11 headless tests in `tests/wave2.rs` covering all 5 messages, keyed destroy, keyed
    filter, footer derivation, purity, and MsgLog recording.

- **Ran the artifact:**
  - `cargo test -p mvu_spike -j 2` → **18/18 green** (7 wave1 + 11 wave2) ✓
  - `DISPLAY=:0 timeout 25 cargo run -p mvu_spike --bin todomvc` → **no panic, no
    B0004**, Vulkan adapter init clean, `TodoMVC Wave 2: list_entity=221v0,
    add_field=220v0, seed_count=3` log confirms seed state and handles. Real content
    renders (checkboxes + text rows + footer visible). ✓

- **Q6 — Keyed dynamic collection RESOLVED:**
  - `RowKey(TodoId)` on each row entity is the stable domain identifier. Reconcile diffs
    by TodoId, not Vec index.
  - `test_destroy_removes_middle_row_by_id`: despawn item[1] of 3 → items[0] and [2]
    survive in their original entities (entity identity preserved). Index-shift CANNOT
    happen — the key is the `TodoId`, not the list position. ✓
  - Cost: one marker component (`RowKey`), one O(n) diff per frame (cheap, n ≈ items
    shown). No VDOM, no virtual key reconciler — the ECS entity IS the keyed identity.
  - Finding: Bevy 0.19 `Children::iter()` yields `Entity` directly (via
    `RelationshipTarget::iter()` → `Copied<Iter<Entity>>`). Calling `.copied()` after
    `children.iter()` causes a type error (`Copied<Copied<...>>`). Fix: remove all
    `.copied()` calls. Minor friction, one-time fix, easy to document.

- **Q2 — Derived footer via query:**
  - `update_footer(models: Query<&TodoList, Changed<TodoList>>, texts: Query<&mut Text, With<FooterText>>)` — 2 queries, 12 lines.
  - Works correctly; triggers only on model change (the `Changed<>` filter avoids
    spurious rewrites).
  - Boilerplate cost vs Elm: 1 dedicated system + 1 marker component (`FooterText`).
    In Elm, `view model` computes `active_count()` inline — no system, no marker.
    The ECS model pays a fixed overhead per derived view: one system + one marker per
    "derived field". For 1–5 derived fields this is tolerable; at 20+ it becomes a
    naming/scaffolding tax.
  - `BoundText<M>` (Wave 1) won't work here because footer is a SIBLING of the list
    container, not a descendant. `BoundText` relies on ancestor-walk FROM the text TO
    the model; a sibling layout breaks that. The dedicated system is the clean path.
  - Finding: layout topology drives the derivation pattern choice. Descendant → use
    `BoundText<M>`. Sibling / cross-subtree → use a dedicated Changed-gated system.

- **Purity boundary:**
  - `TodoList::update()` mutates only `self.items`, `self.filter`, `self.next_id`.
    Returns `Cmd::none()`. Zero ECS references. Zero spawn/despawn.
  - `test_update_purity_no_ecs_access`: calls `list.update(msg)` standalone (no world,
    no app) — compiles and runs cleanly. ✓
  - Structural ECS work lives ONLY in `reconcile_rows` (exclusive system). Clear contract:
    model = pure data; ECS mutations = structural path only.
  - Finding: the purity boundary is EASY to maintain in practice. Rust's type system
    helps — you literally cannot call `world.spawn()` inside `update()` because the
    signature is `fn update(&mut self, msg: M::Msg) -> Cmd<M::Msg>` with no `World`
    parameter. The boundary is enforced structurally, not by convention.

- **Controlled vs self-updating toggle:**
  - `advance_toggle_on_press` (in `BuiySet::Input`) self-advances `A11yToggled` on
    every checkbox press — runs before MVU routing.
  - `reconcile_rows` then writes `A11yToggled` from `items[id].done` — the model-
    authoritative write.
  - Both writes agree on direction, so no conflict in the normal case. But `A11yToggled`
    is written TWICE per toggle event: first by `advance_toggle_on_press` (preview/
    optimistic), then by reconcile (model-authoritative sync).
  - The checkbox is "self-advancing then overridden" — it visually updates immediately
    from the widget's own advance, then reconcile reinforces the same state. This is
    actually fine (no flicker, no fight) for the happy path. It WOULD break if the
    model rejected the toggle (e.g. validation gate): `advance_toggle_on_press` would
    flip the visual to `true`, but reconcile would flip it back to `false` (model said
    no). The visual would flicker one frame (depends on system ordering within the
    same `app.update()`).
  - Finding: for a pure toggle-all-updates MVU model, the double-write is harmless.
    For a model with validation/conditional toggles, the pre-advance creates a one-frame
    flicker. The production solution is to make the checkbox's advance conditional, or to
    suppress `advance_toggle_on_press` when `OnPressMsg<M>` is present.

- **View integration (OnPressMsg on real widgets):**
  - `OnPressMsg::<TodoList>` placed on real `Button` and `Checkbox` entity after
    `spawn_scene(button("×"))` / `spawn_scene(checkbox(text))`.
  - In headless tests: `app.world_mut().write_message(OnPress(entity))` → `app.update()`
    routes through `route_on_press`, drains the inbox, calls `update()`, reconcile fires.
  - `test_destroy_via_on_press`: writes `OnPress(destroy_btn_e)` → `Destroy(id)` →
    row despawned. ✓ The real OnPress path (same seam the picking/AT driver uses) is
    fully exercised.
  - Finding: the `OnPressMsg<M>` component on real widget entities is clean authoring.
    No boilerplate beyond `.insert(OnPressMsg::new(msg))` after `spawn_scene(widget)`.
    The path from pick-click → `Messages<OnPress>` → `route_on_press` → `Inbox<M>` →
    `drain` → `update` is confirmed end-to-end.

- **Infrastructure findings (friction log):**
  - **`LayoutPlugin` missing from initial test setup.** Gallery tests include it; mine
    didn't. `WidgetsPlugin` registers systems in `BuiySet::Layout` which requires
    `LayoutPlugin`'s resources. Fix: add `LayoutPlugin` before `WidgetsPlugin`.
  - **`FocusPlugin::handle_tab` requires `Res<ButtonInput<KeyCode>>` (non-optional).**
    `MinimalPlugins` doesn't include `InputPlugin`. Fix: `app.init_resource::<ButtonInput<KeyCode>>()` (pattern from `gallery/tests/modal_layout.rs` which already documents this).
  - **`Children::iter()` Bevy 0.19 breaking change**: as noted above, `.copied()` must
    be removed. Iterator yields `Entity` directly. `filter` receives `&Entity`; `filter_map` receives `Entity` (by value). Deref patterns (`|&e|`) work for `filter_map` but need `|e|`+`*e` for `filter`.
  - **`MvuSet::Route.after(BuiySet::Input)` is per-plugin, not in `MvuPlugin`.**
    `MvuPlugin` must NOT add this ordering (Wave 1 tests use `MinimalPlugins` without
    `CorePlugin`/`BuiySet`). The ordering is in `TodoMvcMvuPlugin` only, applied only
    when `CorePlugin` is also loaded.

- **Surprised by / friction:**
  - `FlexParams`, `BoxModel`, `Border` structs have non-exhaustive fields in Bevy 0.19
    — must use `..Default::default()` in struct literals. The compiler catches it but
    the error messages list all missing fields individually (verbose).
  - The `footer_text()` test helper initially used `world.query_filtered()` which
    requires `&mut World`. Changed to `world.get::<Text>(handles.footer_text_e)` (the
    entity handle is already in `TodoMvcHandles`). Lesson: always use entity-handle
    lookups in test helpers when available — avoids the `&mut World` vs `&World` issue.

- **If we did this again:**
  - Build the widget tree from real widgets from the first line (avoid the Wave 1 `B0004`
    detour entirely).
  - Put `app.init_resource::<ButtonInput<KeyCode>>()` in the standard headless test
    boilerplate (or document it as a prerequisite for `FocusPlugin`).
  - Check `Children::iter()` behavior once per major Bevy upgrade; document in the
    CLAUDE.md upgrade notes.

- **Next candidates for Wave 2b / pre-final investigation:**
  - **OutMsg composite (Q3)**: self-contained text input that emits an output Msg to
    its parent model — the "adoption edge" pattern.
  - **Reorder with transient state preserved**: drag-reorder a todo row entity without
    losing its widget's internal state (focus, text cursor position).
  - **Async effect (Q4)**: `Cmd::task(future, Msg)` round-trip — spawn a task, fold
    result back through `update`.
  - **Suppress `advance_toggle_on_press` for controlled checkboxes** — production fix
    for the double-write / flicker-under-rejection finding.

- **Orchestrator independent verification (verify-don't-trust):** re-ran
  `cargo test -p mvu_spike -j 2` → **18/18 green**; ran the GUI
  `DISPLAY=:0 cargo run -p mvu_spike --bin todomvc` → render world boots on the real
  RX 6700 XT, window created, `seed_count=3` bootstrapped, **no panic, no B0004** (the
  Wave-1 view-integration gap is closed — the real-widget tree lays out). Wave 2
  findings confirmed. **Q2/Q6 + purity resolved; view-integration confirmed.**

### 2026-06-26 — Wave 3 (async effects — Q4 RESOLVED)

- **Built** (`src/mvu.rs` extended, `src/search.rs`, `src/bin/search.rs`,
  `tests/wave3.rs`, `Cargo.toml` bin entry):
  - `Cmd::Task(BoxedFuture<M>)` variant + `Cmd::task(fut)` constructor. `Cmd<M>` no
    longer derives `Clone`/`Debug` (manual Debug; Task carries an opaque future).
  - `InFlight<M>` component: manual `impl Component` (avoids derive macro bound issues
    with nested generics). Public `task: Task<M::Msg>` field so tests can assert absence.
  - `fold_cmd` now returns `Option<BoxedFuture<Msg>>` (Task variant returns Some; None
    means the Cmd was entirely fold-into-next_queue).
  - `drain` extended with `mut commands: Commands`: when fold_cmd returns a task,
    `AsyncComputeTaskPool::get().spawn(fut)` + `commands.entity(e).insert(InFlight{task})`.
    Insert REPLACES existing InFlight (deferred via Commands; old Task dropped at
    apply_deferred = cancel = takeLatest).
  - `poll_inflight::<M>` system: `block_on(future::poll_once(&mut task))` — non-blocking
    check. On `Some(msg)`: push to `Inbox<M>`, schedule `remove::<InFlight<M>>`. On
    `None`: skip. Dead entity: query misses it automatically — no-op.
  - `add_model::<M>` updated: `(poll_inflight::<M>, drain::<M>).chain().in_set(MvuSet::Drain)`
    ensures poll runs before drain in the same frame, so completed tasks feed the same
    drain pass.
  - `Search` model: `update(SetQuery(q))` → `self.loading=true`, `Cmd::task(fake_fetch(q, delay))`;
    `update(Results(r))` → `self.results=r`, `self.loading=false`, `Cmd::none()`.
    `fake_fetch`: `async fn` with `std::thread::sleep` (no yield; blocking on pool thread).
    Delay configurable per model instance: 0 for tests, 40ms for GUI demo.
  - Search view: real `text_input_single_line`, keyed result rows (`SearchResultKey(String)`),
    status text gated on `Changed<Search>`.

- **Ran the artifact:**
  - `cargo test -p mvu_spike -j 2` → **23/23 green** (7 wave1 + 11 wave2 + 5 wave3) ✓
  - `DISPLAY=:0 timeout 22 cargo run -p mvu_spike --bin search -j 2`:
    - Vulkan adapter init clean (RX 6700 XT), no B0004, no panic ✓
    - Startup seed query "rust" issued automatically (first Update frame)
    - `[search] loading (query="rust")` logged (Changed<Search> trip #1 from SetQuery)
    - `[search] 5 results for query="rust"` logged ~340ms later (task done, Results
      folded back through update, Changed<Search> trip #2)
    - 5 results correct: "Rust programming language", "Rustacean community",
      "Async/Await in Rust", "WebAssembly with Rust", "rustfmt formatter" ✓

- **Q4 EFFECTS — RESOLVED:**

  **Does `Cmd::task` fold back to the RIGHT entity through `update` (tripping `Changed`)?**
  YES, definitively. The entity IS the fold-back address — `InFlight<M>` lives ON the model
  entity; `poll_inflight` queries `(Entity, &mut InFlight<M>)`, gets the entity, pushes
  `(entity, msg)` to `Inbox<M>`; drain calls `models.get_mut(entity)` which (a) finds the
  right model and (b) marks it `Changed<M>`. Test 1 + GUI both confirm.

  **Does drop=cancel actually stop a superseded/despawned task?**
  FUNCTIONALLY YES — the result is permanently discarded. Cancellation of a `Task<T>` in
  `bevy_tasks` (via `async-task`/`async-executor`) means the future is not polled again.
  IMPORTANT NUANCE: `std::thread::sleep` inside an async block is a NO-YIELD operation —
  the pool thread runs the sleep to completion even after cancellation. The cancel only
  discards the result afterwards. So "cancel = no fold-back" is CORRECT, but "cancel =
  immediate thread stop" is NOT true for no-yield futures. Real IO tasks that use
  `futures_lite::io` or timer futures DO yield and cancel sooner.

  **Did `InFlight<M>` on the entity work cleanly?**
  YES, with one friction: `#[derive(Component)]` on `InFlight<M: Model>` has bound issues
  — the derive expects its type params to satisfy `Component` themselves, but `M: Model`
  satisfies `Component` via the trait bound, yet the macro couldn't resolve it cleanly.
  Fixed with a manual `impl Component for InFlight<M> where M: Model { ... }`. One-time
  cost; document as the pattern for generic components.

- **Limitations found:**
  - **Single-in-flight-per-model**: one `InFlight<M>` per entity. Acceptable v1 shape for
    search/typeahead (takeLatest is correct semantics). For concurrent effects (save +
    search simultaneously), each would need its own entity (actor model: spawn a child
    entity as the "effect actor"), or a multi-slot variant (`Vec<(EffectId, Task<M::Msg>)>`
    keyed by caller-assigned ID). The single-slot constraint is a real design choice to
    document in the final spec.
  - **No-yield cancellation**: `std::thread::sleep` in a task body prevents cooperative
    cancel. Production code should use real async timers (`smol::Timer`, `tokio::time`) or
    `futures_lite::future::yield_now()` to make cancellation responsive. Not a framework
    bug — it's the documented cooperative-cancel contract.
  - **2-frame minimum latency**: spawn → InFlight inserted (deferred) → next frame
    poll_inflight → drain processes Results. One-frame latency floor is inherent to the
    deferred-command model. Acceptable; deterministic.

- **Async test determinism:**
  - `delay=0` tests are **fully deterministic**: bounded pump of ≤20 frames, typically
    settles in frame 2. No real sleeps needed.
  - `delay=10ms` supersede test uses `thread::sleep(5ms)` between frames (a real sleep)
    to let the pool thread execute the 10ms fake_fetch. Settled within 30 frames consistently
    in CI. The supersede assertion is structural: the OLD task's `Task<Msg>` is dropped before
    poll_inflight ever sees InFlight for it, so no stale Results can reach the model regardless
    of thread scheduling.
  - `delay=10ms` despawn test: same small-sleep pattern. The assertion is absence-of-panic
    (no fold-back to dead entity) — proven by 15 frames of pump with no crash.
  - FLAKINESS: tests are stable on this machine. The one fragile point is if a `delay=0`
    task somehow doesn't complete before the 20-frame budget (pool overload). In practice
    for a no-sleep async fn, this is essentially impossible.

- **Surprised by / friction (Bevy-0.19 task-pool gotchas):**
  - `block_on(future::poll_once(&mut task))` is the correct non-blocking poll pattern.
    Import path: `bevy::tasks::{block_on, futures_lite::future}`. This exactly mirrors
    `apply_system_font_scan` in `buiy_core/src/text/system_scan.rs` — a direct prior art
    reference that saved time.
  - `#[derive(Component)]` on `InFlight<M: Model>` fails silently in some Bevy 0.19
    configurations due to the nested generics / supertrait bounds. Manual `impl Component`
    is cleaner and more explicit. Pattern to document.
  - `AsyncComputeTaskPool::get()` panics if not initialized (no `TaskPoolPlugin`).
    `MinimalPlugins` (used in headless tests) includes `TaskPoolPlugin`, so this is fine.
    But tests that use `App::new()` without plugins would fail — always use `MinimalPlugins`
    or `DefaultPlugins`.
  - `Cmd<M>` dropping `Clone`/`Debug` derives (because `BoxedFuture<M>` is neither) broke
    no existing tests — `Cmd` is never cloned or Debug-printed in the runtime. The manual
    `Debug` impl printing `"Cmd::Task(...)"` is sufficient. Real production would want a
    richer debug representation (effect name/id).
  - `run_once()` in Bevy 0.19 requires a `Local<bool>` parameter: `fn f(mut done: Local<bool>)`.
    The free function `run_once()` does NOT exist as a zero-arg condition. Fix: use
    `Local<bool>` pattern directly (documented in-place).

- **What Wave 4 (record/replay) must watch for:**
  - **Result Msgs MUST be recorded in MsgLog**: when `poll_inflight` pushes `(entity, Results(r))`
    into `Inbox<M>`, that Msg flows through `drain` which logs it to `MsgLog`. So async
    results ARE in the log — the full session log is (SetQuery, [gap], Results). Replay
    can re-fold BOTH without re-running the task.
  - **ReplayMode should SUPPRESS task spawning**: in replay, `update(SetQuery(q))` should
    return `Cmd::none()` (no task) because the Results msg is already in the replay log and
    will be fed back from the log, not from a live fetch. The simplest mechanism: a
    `ReplayMode` resource flag checked in `drain` (or in a wrapper around `update`) that
    converts `Cmd::Task` → `Cmd::None` during replay.
  - **Ordering is deterministic**: Tasks fold back through the inbox (ordered), so the
    MsgLog sequence for a session with async effects is: `[..., SetQuery("q"), ..., Results([...])]`.
    Replay feeds this in the same order. The "gap" frames between SetQuery and Results don't
    need to be replayed (no model mutations happen in those frames — the model just waits).
    The replay can feed the next log entry immediately without fake-sleeping.
  - **Multi-task sessions**: if two SetQuery were issued in the real session, only ONE Results
    Msg landed (takeLatest). The replay log reflects exactly what happened — one SetQuery, one
    Results — so replay is faithful.
  - Watch for: `Changed<M>` ticks in replay. Since replay calls `update` (which calls
    `get_mut`), Changed fires correctly. Reconcile and binds will re-execute. Golden-image
    comparison would need the same virtual Time tick. See Wave-4 spec.

- **Next:** Wave 4 (record/replay prove-the-loop) OR Wave 2b (OutMsg composite, Q3) depending
  on what the human gate decides. The retrospective + human gate can now speak to Q1 (routing),
  Q2 (derived views), Q4 (effects), Q6 (collections) — all resolved. Q3 (parent↔child OutMsg),
  Q5 (view retained-tree depth), Q7 (replay) remain for W2b/W4.

- **Orchestrator independent verification (Wave 3):** re-ran `cargo test -p mvu_spike -j 2`
  → **23/23 green** (7+11+5); ran `DISPLAY=:0 cargo run -p mvu_spike --bin search` → async
  round-trip confirmed LIVE in the real GUI: `[search] loading (query="rust")` then ~270ms
  later `[search] 5 results for query="rust"` — `Cmd::task` folded back through `update`
  (two `Changed<Search>` ticks), no panic, no B0004. **Q4 effects RESOLVED.**

### 2026-06-26 — Wave 4 (record/replay prove-the-loop — Q7 RESOLVED)

**Built** (`src/mvu.rs` extended, `tests/wave4.rs` — 4 new tests, 27/27 total green):

- `pub trait ReplayableMsg: Send + Sync { fn re_enqueue(&self, world: &mut World); }` — the
  replayable interface. Private concrete impl `MsgEntry<M: Model> { entity, msg, _phantom }`
  pushes `(entity, msg.clone())` into `Inbox<M>` using the stored entity ID.
- `pub struct LoggedEntry { entity, replay: Box<dyn ReplayableMsg>, debug: String }` — replaces
  the old `(Entity, String)` tuple in `MsgLog`. `debug` string is preserved for test assertions.
- `MsgLog` changed from `Vec<(Entity, String)>` to `Vec<LoggedEntry>`. Wave1/Wave2 tests updated
  to use `.debug` field (no breakage beyond field name).
- `#[derive(Resource, Default)] pub struct ReplayMode(pub bool)` registered in `MvuPlugin::build`.
- `drain::<M>` gains `replay_mode: Res<ReplayMode>`:
  - Live mode: records `LoggedEntry` at the one tap (same as before, now structured).
  - Replay mode: skips logging, calls `model.update(msg)` (rebuild state), drops returned `Cmd`
    entirely — no task spawning, no Done follow-ups queued. All consequent Msgs come from the log.
- `pub fn replay(app: &mut App, log: &[LoggedEntry])`: sets `ReplayMode(true)`, iterates log,
  calls `re_enqueue` + `app.update()` per entry, then sets `ReplayMode(false)`.

**Ran the artifact:** `cargo test -p mvu_spike -j 2` → **27/27 green** (7+11+5+4). All new tests
passed on first compile — the design was consistent enough to get right without iteration. ✓

---

**Q7 — Record/Replay RESOLVED:**

**Does record→replay produce a byte-identical snapshot?** YES, definitively. `test_record_replay_byte_identical` drives an 8-message session (3 adds, 1 toggle, 1 destroy, 1 filter change, 1 SetQuery, 1 Results fold-back) and asserts `recorded == replayed` as byte-identical strings. Passes. ✓

**Did "fold-back-as-Msg makes the log self-contained" hold?** YES. `test_async_results_are_self_contained_in_log` confirms that the async `Results([...])` entry appears in the log AFTER `SetQuery` — so replay can drive the search model from `loading=true → loading=false` without re-executing any IO. The `Cmd::task` future is dropped in replay; the Results Msg is re-fed from the log entry instead. This is the strongest property of the design: async side effects are already reduced to Msgs before the log, so the log is causally complete.

**Did the scrub/prefix re-fold work?** YES. Two scrub points were tested:
  - N=1 (after first Add): fresh app replayed one entry → snapshot matched the live `snap_after_add1`.
  - N=4 (after Toggle): fresh app replayed four entries → snapshot matched `snap_after_toggle`.
  Both byte-identical. Time-travel/scrub works. ✓

---

**Bypass findings:**

The core question: does any domain state mutate OUTSIDE the Msg/drain path and threaten replay determinism?

- **`advance_toggle_on_press`** (WidgetsPlugin, BuiySet::Input): writes `A11yToggled` on every checkbox press. During replay, there are NO `OnPress` messages — routing is not replayed, only model-level Msgs are in the log. So `advance_toggle_on_press` NEVER fires during replay. This is actually correct: the self-advance is an input shortcut (optimistic visual update), not authoritative state. Authoritative state is `TodoList.items[id].done`, which is reconstructed by `reconcile_rows` from the replayed model. `A11yToggled` is derived widget state and NOT in the domain snapshot — even if it diverged, the byte-identical assertion would still pass.
- **`reconcile_rows` / `reconcile_results`**: these ARE structural path writes (entity spawns, CssVisibility, A11yToggled), and they run normally during replay. This is intentional and correct — they reconstruct the ECS entity tree from the replayed model state. They do NOT write to the domain model components themselves. No bypass.
- **Domain model components** (`TodoList`, `Search`): ONLY mutated inside `update` (the purity boundary, proven in Wave 2). Replay calls `update` for every log entry — the same mutations fire. No bypass.

**Conclusion:** No domain state bypasses the Msg/drain path. The purity boundary (Wave 2) directly enables replay fidelity. `test_replay_bypass_guard_toggle_session` and `test_replay_does_not_grow_the_log` confirm this at the test level.

---

**Entity-vs-domain-key finding:**

The `snapshot` function is domain-keyed (sorts by `TodoId.0`, uses string content for search results, never includes `Entity`). This was required for correctness even though in practice, both apps (record and replay) spawn entities in the same order from fresh `World`s, giving identical entity IDs.

The `MsgEntry.re_enqueue` uses the stored raw `Entity` from the recorded session. This works in our controlled test because entity IDs are deterministic (Bevy's allocator resets to 0 per `World`). In production with nondeterministic entity creation (async entity spawns, different plugin ordering, background systems), entity IDs would diverge across sessions. The production fix: `re_enqueue` should query `With<M>` to find the current entity in the fresh world (single-model-per-type assumption), or use a stable domain key embedded in the log entry.

This is the exact place where "raw Entity leaks into the replay addressing" — the log entry `debug` string includes the entity ID, but the `replay` token correctly abstracts it via `re_enqueue`.

---

**Agent-interface tie-in finding:**

The in-process inspection driver (Phase P1a in the agent-interface campaign) pokes `Focus`, `OnPress`, and `EditCommand` sinks directly. The prototype confirms:

**For record/replay (and LLM-agent driving) to be deterministic, Action lowering MUST route through `update`/the Msg path.** Specifically:
- `OnPress` on a widget → `route_on_press` → `Inbox<M>` → `drain` → logged. This path IS replayable. ✓
- Direct `EditCommand` or component writes that bypass the drain → NOT logged → NOT replayable. ✗

The current agent-interface driver (pointer-press → `OnPress` → routing) is already on the replayable path. `EditCommand` (text insertion) goes through `EditSubmitted` → `route_add_submit` → `Add(text)` → logged. Also replayable. ✓

The gap: `set_focus` (Focus component write) and AT `set_value` (direct a11y state write) bypass the Msg path. For a session to be fully replayable, these must either (a) also write through the Msg path (Focus as a Msg), or (b) be treated as "initial conditions" that are seeded identically in the fresh replay app. For the prototype, Focus state is not in the domain snapshot, so it doesn't affect replay fidelity of the domain state. For a production record/replay that includes UI focus in the replay, Focus mutations would need to be Msg-addressable.

---

**Surprises / Bevy 0.19 gotchas:**

- `Box<dyn ReplayableMsg>` for mixed model types in one `Vec` worked cleanly. Rust's object safety rules are satisfied because `re_enqueue` takes `&self + &mut World` (no generics, no Sized requirement). No surprises.
- `MsgEntry<M>` requires `PhantomData<M>` because `M::Msg` doesn't uniquely determine `M` (compiler "unused type parameter" error otherwise). One-time cost.
- `LoggedEntry` is not `Clone` (because `Box<dyn ReplayableMsg>` is not). For tests, we pass `&[LoggedEntry]` (borrowed slice) — the record app lives alongside the replay apps, so no clone needed. For a production log-serialization use case, `ReplayableMsg` would need a `clone_box()` method.
- `drain`'s `Res<ReplayMode>` addition: Bevy system parameter ordering doesn't matter for correctness, but it changes the system signature. All existing tests compile cleanly because `MvuPlugin::build` now registers `ReplayMode` — tests that only add `MvuPlugin` (wave1) get the resource for free. ✓
- `drop(cmd)` in replay for `Cmd::Task(BoxedFuture(...))`: the future is dropped before being polled. For well-behaved async fns (no special Drop logic), this is safe and has no side effects. `fake_fetch` is a simple async fn — no issues. ✓

---

**Overall prototype verdict (across Q1–Q7):**

**Is the Elm-bevyified surface SOUND on Bevy 0.19?** YES, with clear boundaries and one design-level refinement needed.

- **KEEP: `Model` trait + `update` purity boundary.** The invariant that `update` mutates ONLY self (no ECS) and returns `Cmd` as a value is the load-bearing property. Rust's type system enforces it structurally (no `World` in scope). This directly enables replay (Q7), deterministic tests, and LLM-agent driving. The biggest win of the whole design.

- **KEEP: `drain` as the single globally-ordered tap.** All Msgs, including async fold-backs, pass through one system. This is what makes the log globally ordered and causally complete. The "run-to-completion" + "Done queues next-frame" contract (bounded, non-reentrant) makes the ordering predictable. Replay worked first-try precisely because of this property.

- **KEEP: Keyed-reconcile (ECS entity = domain identity).** `RowKey(TodoId)` as the stable identifier, with entity identity preserved across reconcile. No VDOM, no virtual key reconciler — the ECS entity IS the keyed identity. Cleaner and cheaper than the alternatives.

- **REFINE: Entity addressing in the log → domain key.** `MsgEntry.re_enqueue` must use a stable domain key (not raw `Entity`) for cross-session replay. The single-model-per-type `With<M>` query is acceptable for v1; multi-instance models need explicit domain addressing. This is a known design hole that the retrospective must surface explicitly.

- **REFINE: `Cmd::Done` fold-back in replay.** Currently `Cmd::Done` is dropped in replay just like `Cmd::Task`. This is correct as long as the Done Msgs are themselves later log entries. But `Cmd::Done` is a same-session follow-up — the log records it as the next drain entry. This worked in the test (ChainCounter in wave1 uses `Cmd::Done`; it's NOT tested in W4 replay directly, but the logic is sound). Should be explicitly tested in the final.

- **REDESIGN: Focus / AT state as replay gap.** The in-process driver writes Focus and A11y `set_value` outside the Msg path. For a production record/replay that captures UI focus state, Focus mutations need to be Msg-addressable OR seeded as initial conditions. For domain-only replay (the current prototype), this is not a problem. But for a fully reproducible agent-driving session, it's a real gap.

- **Overall**: Q1 (routing) ✓, Q2 (composition/derived views) ✓, Q4 (effects/cancellation) ✓, Q6 (keyed collections) ✓, Q7 (record/replay) ✓. Q3 (OutMsg parent↔child) and Q5 (retained-tree depth stress) remain unresolved. The resolved Qs cover the most novel/risky territory; the remaining Qs are important for ergonomics but do not threaten the foundational soundness.

---

**Verified:** `cargo test -p mvu_spike -j 2` → **27/27 green** (all prior + 4 new Wave 4 tests). Wave 4 complete. **Ready for human gate + retrospective.**

- **Orchestrator independent verification (Wave 4):** re-ran `cargo test -p mvu_spike -j 2`
  → **27/27 green**, incl. `test_record_replay_byte_identical` ✓ and
  `test_async_results_are_self_contained_in_log` ✓. **Q7 RESOLVED.** Wave-4's three
  headline findings for the final: **KEEP** = the `update` purity boundary (no `World`
  in scope → type-enforced); **REFINE** = log entries store raw `Entity` (works only in
  same-setup; production needs a `With<M>` query or an embedded stable domain key);
  **REDESIGN** = Focus/AT state replay-gap — the in-process driver writes Focus/`set_value`
  OUTSIDE the Msg path, so fully-reproducible agent-driving needs those Msg-addressed or
  seeded (the agent-interface "Action lowering must route THROUGH update" constraint).

### 2026-06-26 — Wave 2b (OutMsg composite + reorder-transient — Q3 + Q6-hard RESOLVED)

**Built** (`src/mvu.rs` extended, `src/editor.rs` new, `src/todomvc.rs` extended,
`tests/wave2b.rs` new — 6 new tests, **33/33 total green**):

**Runtime extension (`src/mvu.rs`):**
- `MvuSet::Deliver` added between `Drain` and `Bind` — the output delivery phase.
- `OutputModel: Model` trait with `type Out` + `fn take_output(&mut self) -> Option<Self::Out>`.
  Associated type defaults are NOT stable in Rust 1.95, so `OutputModel` is a separate
  subtrait rather than extending `Model` with a defaulted `Out`. Existing models need zero
  changes; only composite widgets implement `OutputModel`.
- `OutputQueue<M: OutputModel>` resource — `#[derive(Resource)]` works on generic types
  with associated type bounds (same pattern as `Inbox<M>`).
- `OnOutput<M: OutputModel>` component — stores `Arc<dyn Fn(M::Out, &mut World) + Send + Sync>`.
  Arc (not Box) so `deliver_output` can clone the Arc to release the `&world` borrow before
  calling `handler(out, &mut world)`. Manual `impl Component` like `InFlight<M>`.
- `flush_outputs::<M>` — regular system; after `drain::<M>` within `MvuSet::Drain`, drains
  `model.take_output()` into `OutputQueue<M>`.
- `deliver_output::<M>` — exclusive system (`&mut World`) in `MvuSet::Deliver`; translates
  queued outputs through each entity's `OnOutput<M>` handler.
- `add_output_delivery::<M>` — registers the above two systems.

**New `src/editor.rs`:**
- `Editor` model: `buffer`, `original`, `pending_out: Option<EditorOut>`. Pure `update()`.
- `EditorMsg { SetText(String), Commit, Cancel }`, `EditorOut { Committed(String), Cancelled }`.
- `impl OutputModel for Editor`: `take_output()` drains `pending_out`.
- `route_editor_submit` system: ancestor-walks `EditSubmitted` → `EditorMsg::Commit` in
  `Inbox<Editor>`. Silently ignores non-Editor inputs.
- `register_editor_model(app)` helper called from `TodoMvcMvuPlugin`.

**`src/todomvc.rs` extensions:**
- `TodoList.editing_id: Option<TodoId>` — which row (if any) is being edited.
- `TodoListMsg::BeginEdit(TodoId)`, `CommitEdit(TodoId, String)`, `CancelEdit(TodoId)`.
  All pure in `update()`. ECS spawn/despawn lives in `reconcile_rows` (purity boundary preserved).
- `spawn_editor_for_row` — spawns Editor entity as child of row entity, inserts
  `OnOutput<Editor>` (the adoption-edge translator). ONE closure captures `list_e` + `row_id`;
  maps `EditorOut::Committed(text)` → `CommitEdit(row_id, text)`. `EditorMsg` NEVER referenced.
- `reconcile_rows` extended: for each row, diffs `should_edit` vs existing editor child;
  spawns if needed, despawns if not. When `should_edit && editor_child.is_some()`: leaves the
  editor alone — Q6-hard: no re-seed, transient `buffer` survives.
- `TodoMvcMvuPlugin`: `reconcile_rows` now `.after(MvuSet::Deliver)`; calls
  `register_editor_model(app)`.

**Ran the artifact:**
- `cargo test -p mvu_spike -j 2` → **33/33 green** (7+11+6+5+4) ✓
- `DISPLAY=:0 timeout 22 cargo run -p mvu_spike --bin todomvc -j 2` → Vulkan adapter init
  clean (RX 6700 XT), `seed_count=3`, `Seeded BeginEdit(TodoId(1))`, window opens with
  editor visible on second row, **no panic, no B0004** ✓

---

**Q3 — Parent↔child via OutMsg RESOLVED:**

**Does the separate `Out` type keep the composite's `Msg` from leaking into the parent?**
YES, definitively. `TodoList::update()` receives only `TodoListMsg` — it never sees
`EditorMsg`. This is structurally enforced: `update()` takes `Self::Msg` as parameter, and
`EditorMsg` is not `TodoListMsg`. The only place both types appear together is in the
`OnOutput<Editor>` closure in `spawn_editor_for_row` — the one designated translation point.

**Is the closure count = adoption edges (not depth)?**
YES. `query_filtered::<Entity, With<OnOutput<Editor>>>()` count is 0/1/0 as editors
open/close, regardless of tree depth. Three tests assert this at each state transition.

**MsgLog entity-keyed forensics:** `entity == list_e` entries contain only `BeginEdit`,
`CommitEdit`, `CancelEdit`. `entity == editor_e` entries contain `SetText`, `Commit`, `Cancel`.
The translation boundary is visible in the log at the entity level.

**When is each tier right?**
- **Translator-by-default** (OnPressMsg ancestor walk): parent authors the child's message.
  E.g. a "destroy" button → Destroy msg to parent. Zero boilerplate.
- **OutMsg** (OnOutput<M> edge): parent does NOT author the child's internals. Widget is
  self-contained and emits high-level semantic events. One translator per adoption edge.
  Cost: ~6 lines at the adoption site; `OutputModel` impl; `Deliver` set latency.

**OutputModel subtrait vs associated type default:**
Using `OutputModel: Model` subtrait was forced by Rust stable not supporting associated type
defaults (1.95). Actually CLEANER: the subtrait makes "this model emits output" explicit in
the type system; zero-output models need no changes; add_output_delivery is explicit opt-in.

---

**Q6-hard — Reorder/insert/delete preserves transient Editor state RESOLVED:**

**Did the half-typed buffer survive?**
YES. Test `test_transient_editor_buffer_survives_insert_and_delete`:
- Begin editing Beta, set buffer to "half-typed" (do NOT commit)
- `Add("Delta")` → new row entity spawned; Beta's `row_e` entity identity unchanged
  (same entity confirmed: `row_entity_for_id(.., id_b) == Some(row_b)`)
- `Destroy(id_a)` → Alpha's entity despawned; Beta's `row_e` unchanged
- After all structural changes: `editor_buffer == "half-typed"`, editor entity alive,
  `OnOutput<Editor>` handler present
- Commit → `CommitEdit(id_b, "half-typed")` → Beta's text updated ✓

**Root cause of the property:** The Editor is a CHILD of Beta's row entity (keyed by
`TodoId`). `reconcile_rows` only despawns entities whose `TodoId` left `items`. Structural
changes to OTHER rows never touch Beta's entity subtree. Entity identity IS the transient-
state container — no VDOM diffing needed.

**No stale-key hazard:** The `OnOutput<Editor>` closure captures `list_e` and `row_id` at
spawn time. Both are stable for the editor's lifetime. ✓

---

**Surprises / Bevy 0.19 gotchas:**

- **`Arc` for handler, not `Box`**: critical for exclusive-system "borrow → extract → release
  → mutate `world`" pattern. Box would hold the borrow; Arc clone releases it. One-time doc.
- **Two-tick latency for OutMsg delivery**: `CommitEdit` lands in `Inbox<TodoList>` during
  `MvuSet::Deliver`, AFTER `drain::<TodoList>` already ran. Tests need TWO `tick()` after
  Commit. Same contract as `Cmd::Done`.
- **No `#[derive(Resource)]` issue on generic types**: `OutputQueue<M: OutputModel>` compiles
  fine with `#[derive(Resource)]` — same pattern as existing `Inbox<M: Model>`. The derive
  macro handles the associated-type bounds correctly.

---

**Net for the retrospective:**

The two-tier model is correct and complete:
1. Translator-by-default (OnPressMsg walk) for leaves; zero boilerplate.
2. OutMsg (OnOutput<M>) for self-contained composites; one closure per adoption edge.

**One refinement worth flagging for the final spec**: the two-tick delivery latency compounds
for deep chained-model trees (A emits → B processes → B emits → C processes = 4 ticks). The
production fix: `chain_output::<Child, Parent>` ordering that pins the child's `deliver`
BEFORE the parent's `drain` within the same frame. This is an optimization, not a
correctness fix.

**Verified:** `cargo test -p mvu_spike -j 2` → **33/33 green**. GUI boots clean, editor
visible, no B0004, no panic. **Q3 + Q6-hard RESOLVED. All 7 Qs now resolved.**
**Prototype COMPLETE. Ready for human gate + retrospective.**
