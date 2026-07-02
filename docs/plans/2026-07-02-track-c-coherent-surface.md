# Track C — One Coherent Surface — Coordination Plan

> **For agentic workers:** this is a **coordination plan** for the campaign's largest track. It decomposes Track C into ordered, independently-landable **slices** (each its own review-gated PR under `subagent-driven-development`), with a **just-in-time per-slice execution plan** (the agent-interface convention). Realizes spec `docs/specs/2026-07-01-first-class-llm-dev-support-design.md` §3.1 + §"Track C". Base: `origin/main` @ `667cb8a` (Tracks B + A landed). Branch family: `feat/track-c-*`.

**Goal:** make Buiy's authoring surface *coherent* for an LLM author — one obvious constructor per widget, state readable without a foreign enum, typed events over the untyped sink, and a prelude that expresses a whole app without recalling module paths.

**Design is already resolved** (spec §3.1 Builder→Bundle proven by prototype R3; §8 resolves the child hook as an `On<Add, Marker>` observer). This track is **execution**, not a new prototype.

---

## Current-surface map (research 2026-07-02, agent `aa19fd2`)

- **11 widgets** in `crates/buiy_widgets/src/`, each `::new(...) -> impl Bundle` returning an **anonymous tuple** with `children![…]` hand-wired; scene-fns (`button()` …) + `_scene` dual-names are the `bsn!` styling path.
- **Events:** one untyped **`OnPress(Entity)`** (a buffered `Message`, `crates/buiy_core/src/interaction.rs:30`) is the activation sink for everything; **no `ValueChange<T>`**. Menu alone has a full MVU vocabulary (`MenuModel`/`MenuMsg`/`menu_reducer`, `menu.rs:591/605/627`). Toggle leaves route `OnPress → enqueue::<A11yToggled>(ToggleMsg)` (`widgets/lib.rs:93,228`).
- **Hooks:** Buiy uses `On<Insert/Discard/Remove, Anchor>` **closure observers** (`crates/buiy_core/src/layout/mod.rs:109-127`, Decision D12 — closures because `On<'w,'t,E,B>` lifetimes don't elide in named fns). **No widget builds children via a hook today** — the R3 spike proved `On<Add, Marker>` + `with_child(...)` works and is the target.
- **State read paths:** `A11yToggled(pub accesskit::Toggled)` (checkbox/switch), `A11yValue{now,…}` (slider), `A11yExpanded(pub bool)` (disclosure), `TextEditState`/`A11yTextValue` (text input). **No domain accessors** — callers compare `.0 == Toggled::True` (foreign enum; the F1 silent-wrong).
- **Prelude** (`crates/buiy/src/lib.rs`, `pub mod prelude { pub use crate::* }`): the full Buiy surface incl. MVU, but **zero bevy ECS essentials** (`Component`/`Commands`/`Query`/`MessageReader`/`With`/`Camera2d`/`App`/… are a *private* `use bevy::prelude::*` at L61). `Text`/`Node` glob-collision with `bevy::prelude` is **dormant in-repo** (workspace bevy omits `bevy_ui`/`bevy_text`) but **real for any downstream `DefaultPlugins` app** (the environment the prototype ran in). `Style` does not collide (merged into `Node` upstream).

---

## Slice decomposition (ordered; each a landable, gated PR)

Ordered to **front-load low-risk / high-value** (the prototype's #1 wall was the prelude) and to land the additive surface *before* the big constructor refactor consumes it.

### C1 — Curated prelude (N1) — **first, lowest risk, highest LLM value**
Make `use buiy::prelude::*;` express a whole Buiy app *and its systems* without a second `use bevy::prelude::*;`.
- Re-export a **curated** set of Bevy ECS authoring essentials through `buiy` (NOT a blanket `pub use bevy::prelude::*`): the `Component`/`Resource`/`Message`/`Event` derives, `Commands`/`Query`/`Res`/`ResMut`/`Single`/`With`/`Without`/`MessageReader`/`MessageWriter`/`App`/`Startup`/`Update`/`Entity`/`Camera2d`/`default()` — the exact set the N=4 probes needed to write a system.
- **Resolve the `Text`/`Node` collision** by making `buiy::prelude` **self-sufficient**: an author uses ONLY `buiy::prelude::*` (buiy's `Text`/`Node` win, no `bevy::prelude::*` needed), so the glob-collision never arises. Document this as the one-import contract.
- **Risk:** low (additive re-exports). **Ripple:** none to widgets. **Verify:** a new `examples/` (or doctest) authoring a full system with ONLY `buiy::prelude::*`; confirm the in-repo examples still compile.

### C2 — Domain accessors (F1)
Read widget state without the foreign `accesskit::Toggled`.
- `Checkbox::checked(&…) -> bool`, `Switch::on(&…) -> bool`, `Slider::value(&…) -> f64`, `Disclosure::expanded(&…) -> bool` (+ `TextInput::value`). **Design decision (record in the C2 slice plan):** accessor shape — extension trait over the state component vs. associated fn taking `&A11yToggled` — and the `Toggled::Mixed → bool` mapping (checkbox is tri-state).
- **Risk:** medium (shape choice). **Ripple:** migrates the `.0 == Toggled::True` call sites (gallery tests, `interaction.rs`). **Additive**; lands before C4 uses it.

### C3 — Typed events (F2 / D2)
`ValueChange<T>{is_final}` over the untyped `OnPress`.
- Likely `buiy_core::interaction` (the `OnPress` co-drive precedent). Emit from the toggle commit (`ToggleLeafSet::Drain`, discrete → `is_final=true`) and the slider `A11yValue` path (continuous → `is_final` load-bearing). **Harmonize with**, not fork, the existing `MenuMsg`/MVU funnel (spec §8 N4 "one message substrate").
- **Risk:** medium. **Additive** (keep `OnPress` working).

### C4 — Builder→Bundle constructors + children-via-observer (the big refactor)
The §3.1 core. Per widget: a named `#[derive(Bundle)]` builder with **component-typed fields** + chained setters storing real components (`.checked(true) → A11yToggled(Toggled::True)`); an `On<Add, Marker>` observer (D12 closure pattern) builds the visible children so the bundle stays flat and the author never hand-wires `children!`.
- **Sequence within C4 (sub-waves, each committable):** (a) the flat single-child widgets (Button, Checkbox) — establish the observer pattern; (b) nested-subtree widgets (Switch, Slider) — observer builds the 2-level track→thumb subtree, visual systems ordered after; (c) sibling-relation widgets (Disclosure, Tooltip, Dialog, Menu) — preserve the `Added<Children>` edge-wiring (the observer's `with_child` still triggers `Added<Children>`; validate ordering); (d) the misfits — TextInput (Text-on-root, two ctors → keep `single_line`/`multi_line`), Popover (data-struct `anchored_to`), ScrollArea (add a builder or keep container-only — decide in the C4 plan).
- **Risk:** high; touches every widget. **Gate hard** (per-sub-wave review + run the gallery).

### C5 — bsn-reachable decomposed style + deprecate the old spellings (F4 / §3.1)
- Make the decomposed style components reachable in `bsn!` on the steered path (the F4 fix), so the canonical path and the nice API coincide.
- **Deprecate** the bare-marker single-field patch (the §4.1c trap), the scene-fns, and the `_scene` dual-names (spec §3.1). **Ripple (hard):** the scene-fns collide name-for-name with `buiy::view` builders (why `view` is a separate module) and back the `counter_view`/`todomvc_view` examples — coordinate the deprecation with `buiy::view` and migrate examples.
- **Risk:** medium-high. **Last**, after the builder path is the canonical one.

---

## Cross-cutting notes / decisions to make in the per-slice plans
- **Menu already has MVU** — C3/C4 harmonize with `MenuModel`/`MenuMsg`, don't fork.
- **§4.1c trap** actively bites at `popover.rs:227-239` — C4/C5 should let the builder path make that defensive re-assert unnecessary, or document why it stays.
- **`buiy::view` interaction** — C5's scene-fn deprecation is the coupling point; sequence so `view` (the safer-V surface) and the deprecation don't fight.
- Each slice: full gate (fmt/clippy `-D warnings`/doc `-D warnings`/`nextest --workspace`) + the GPU `--ignored` lane only if it touches render (C1–C4 are non-render; C5 may touch style). **RUN the gallery** for any constructor/visual change (widget-catalog lesson: headless-green ≠ works).

## Self-review
- Realizes spec §3.1 (Builder→Bundle, children-via-hook) + the four Track C bullets (constructor, accessors, typed events, prelude, bsn-style).
- Decomposed so the additive, low-risk surface (C1 prelude, C2 accessors, C3 events) lands and de-risks *before* the big C4 constructor refactor consumes it; C5 (deprecation) is last.
- Each slice is independently landable + verifiable; the hard cases (nested/sibling/misfit widgets) are named and sequenced within C4, not hand-waved.
