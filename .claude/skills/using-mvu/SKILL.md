---
name: using-mvu
description: How to AUTHOR and TEST application state with Buiy's MVU (Model-View-Update) funnel — define a Model + pure reducer, register with mvu_model, enqueue (never fold) messages, read live widget state through the domain accessors, clock-drive with ClockPlugin/advance_clock, and unit-test a model headlessly with the MvuTestApp builder. Use whenever adding or changing app/widget state, wiring a reducer, driving a time-based model, or writing an MVU test. Mirrors docs/specs/2026-06-29-mvu-as-core-design.md.
---

# Using Buiy's MVU state funnel

MVU is Buiy's **primary state interface**: widget and app state flows through one
ordered Model-View-Update funnel in `buiy_core`, which makes it recordable and
replayable. A model is a `Component`; a pure reducer folds messages; handlers only
`enqueue`; one ordered **drain** is the sole writer; a bind projects the folded
model into the view.

**Source of truth:** the design spec
[`docs/specs/2026-06-29-mvu-as-core-design.md`](../../../docs/specs/2026-06-29-mvu-as-core-design.md)
(§2 the substrate, §3 the tiers, §7 record/replay). The code-proximate twin is the
crate root doc [`crates/buiy_core/src/mvu/mod.rs`](../../../crates/buiy_core/src/mvu/mod.rs).
If this skill drifts from those, they win — update this skill in the same commit.
Every code recipe below is compiled (and mostly run) as a test in
[`crates/buiy/tests/using_mvu_examples.rs`](../../../crates/buiy/tests/using_mvu_examples.rs)
— that file is the anti-rot net; keep it and this skill in lockstep.

The whole app-author surface is reached through one import — `use buiy::prelude::*;`
— no second `use bevy::prelude::*;` and no direct `buiy_core` dependency.

## When to use this skill

Before: adding or changing any app/widget state; writing a reducer; registering a
model; driving a time-based model (countdown, animation, game loop); reading widget
state in a system; or writing a headless MVU test. If you are only *running* the
gates, jump to [Running the tests](#running-the-tests).

## The core loop (the mental model)

```
handler ──enqueue(msg)──▶ inbox ──drain (the ONLY writer)──▶ Model changed? ──bind──▶ view
                                     folds via a pure reducer,     (set_if_neq:
                                     commits with set_if_neq       no change ⇒ no cascade)
```

Three rules the whole substrate rests on, each a silent-wrong footgun if broken —
see [Gotchas](#gotchas-the-silent-wrong-ones):

1. **Enqueue, never fold.** Handlers/observers/callbacks may *only* `enqueue`. The
   drain is the sole place a model changes.
2. **`set_if_neq` discipline.** The drain folds onto a clone and commits with
   `set_if_neq`, so an idempotent fold does **not** trip `Changed<M>` — store
   *derived* values, never raw per-frame inputs.
3. **A stable `LogicalId` per model entity** — the replay key. Missing/duplicate is
   now **loud in debug builds** (Track A).

## Recipe: define a Model + reducer, and register it

A model is a `Clone + PartialEq + Reflect` component; the reducer is a **free
function** `fn(&mut Model, Msg) -> Cmd<Msg>` (not a method — the env must be a real,
purity-checked param). Register model + reducer in **one call** with `mvu_model`
(the model type is inferred from the reducer's `&mut M` — no turbofish):

```rust
use buiy::prelude::*;

#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter { value: i64 }

#[derive(Clone, Debug, PartialEq, Reflect)]
enum CounterMsg { Increment, Add(i64), Reset }

impl Model for Counter { type Msg = CounterMsg; }

fn update(c: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => c.value += 1,
        CounterMsg::Add(n)    => c.value += n,
        CounterMsg::Reset     => c.value = 0,
    }
    Cmd::none() // or Cmd::emit(msg) to re-fold, Cmd::task(fut, map) for async
}

// In build(): `mvu_model` returns a `ModelWiring` handle — call `.app()` to escape
// back to `&mut App` and keep chaining. (BuiyPlugin already installs the MVU
// substrate via the widgets plugin, so you only register the model.)
fn build(app: &mut App) {
    app.mvu_model(update).app();
}
```

The reducer returns a `Cmd<Msg>`: `Cmd::none()` (no effect), `Cmd::emit(msg)`
(re-fold in the same drain pass, run-to-completion), or `Cmd::task(future, map)`
(spawn an async effect whose result folds back stamped `Origin::Command`, and is
suppressed on replay).

## Recipe: enqueue (never fold) a message

Send a message with `enqueue::<M>(&mut commands, entity, msg)` from any system or
observer that holds `Commands`. Place the producing system in `MvuSet::Enqueue` so
the pinned `ApplyDeferred` flushes it into the **same-frame** drain:

```rust
use buiy::prelude::*;
# #[derive(Component, Default, Clone, PartialEq, Reflect)] #[reflect(Component)] struct Counter { value: i64 }
# #[derive(Clone, Debug, PartialEq, Reflect)] enum CounterMsg { Increment }
# impl Model for Counter { type Msg = CounterMsg; }

fn on_click(mut commands: Commands, model: Query<Entity, With<Counter>>) {
    for e in &model {
        enqueue::<Counter>(&mut commands, e, CounterMsg::Increment);
    }
}

fn wire(app: &mut App) {
    app.add_systems(Update, on_click.in_set(MvuSet::Enqueue));
}
```

Do **not** call `update(...)` yourself, and do **not** grab `Query<&mut Counter>`
to mutate it — that skips the funnel, the record tap, and `set_if_neq`. There is no
public "fold" entry point by design.

## Recipe: read live widget state (the domain accessors)

Never touch the foreign `accesskit::Toggled`/`Value` enums. Query the state
component alongside the widget marker and read through the widget's **domain
accessor**, which returns a domain type:

```rust
use buiy::prelude::*;

fn read(
    boxes:   Query<&A11yToggled,  With<Checkbox>>,
    sliders: Query<&A11yValue,    With<Slider>>,
    disc:    Query<&A11yExpanded, With<Disclosure>>,
) {
    let _checked = boxes.iter().filter(|t| Checkbox::checked(t)).count(); // -> bool
    for v in &sliders { let _ = Slider::value(v); }                       // -> f64
    for e in &disc    { let _ = Disclosure::expanded(e); }                // -> bool
}
// also: Switch::on(&A11yToggled), Slider::min/max/fraction(&A11yValue),
//       TextInput::value(&A11yTextValue). Overlay open-state (Track F), each
//       Option<&CssVisibility> (absent = open): Popover::is_open, Menu::is_open,
//       Dialog::is_open, TooltipNode::is_open.
```

React to a **value change** with the typed `MessageReader<ValueChange<T>>`
(`<bool>` for checkbox/switch, `<f64>` for slider) — not the untyped `OnPress` sink.

## Recipe: clock-drive a time-based model

For a countdown / animation / game loop, add `ClockPlugin::<M>::new(Msg::Tick)`. It
enqueues `Msg::Tick(now)` onto every actor of `M` **every frame** (`now` is the
elapsed `Res<Time>`). It is a **poll clock, not edge-triggered** — the reducer
receives `now` each frame and must store only the **derived** value so `set_if_neq`
absorbs a steady frame:

```rust
use buiy::prelude::*;
use std::time::Duration;

#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Countdown { remaining: u64 }

#[derive(Clone, Debug, PartialEq, Reflect)]
enum ClockMsg { Tick(Duration) }

impl Model for Countdown { type Msg = ClockMsg; }

fn countdown(m: &mut Countdown, msg: ClockMsg) -> Cmd<ClockMsg> {
    let ClockMsg::Tick(now) = msg; // irrefutable: ClockMsg has one variant
    m.remaining = 10u64.saturating_sub(now.as_secs()); // DERIVED — never store `now`
    Cmd::none()
}

fn build(app: &mut App) {
    app.mvu_model(countdown)
        .app()
        .add_plugins(ClockPlugin::<Countdown>::new(ClockMsg::Tick));
}
```

`map` is a bare `fn(Duration) -> M::Msg` (an enum tuple-variant ctor like
`ClockMsg::Tick` is exactly that), so it is `Copy` and cannot capture —
determinism-safe by type. In a **headless test** drive it deterministically with
`advance_clock(&mut app, delta)` (virtual clock, no real sleeps) — never a real
`sleep`.

## Recipe: headless-test a model with `MvuTestApp`

`buiy::test::MvuTestApp` (behind the `test-support` feature, so it is compiled out
of release) is the shared harness — stand up the GPU-free preset, register the
reducer, spawn, step, read — so you never hand-roll one per suite. **A whole test
in ~5 lines:**

```rust,ignore
use buiy::prelude::*;
use buiy::test::MvuTestApp;

let mut t = MvuTestApp::new(update); // preset + reducer; model type inferred
let e = t.spawn(Counter::default()); // spawned WITH a fresh, unique LogicalId
t.enqueue(e, CounterMsg::Increment); // enqueue — never fold
t.step();                            // one frame: inbox → drain → bind
assert_eq!(t.read(e).value, 1);      // read the live model
```

The surface: `new(reducer)` (infers `M`), `with_clock(map)` (a poll clock),
`spawn(model)` / `spawn_with_id(model, id)`, `enqueue(e, msg)`, `step()` /
`settle()` / `advance_clock(delta)` (all chainable), `read(e)` / `try_read(e)`, and
the escape hatches `app_mut()` / `world()` / `world_mut()` / `snapshot_report()`.

Because it stands up the **full GPU-free probe preset**, the same harness reads and
drives **widget** state through [`buiy::probe`](../../../crates/buiy/src/lib.rs)
(mirror the `using-buiy-verification` a11y/probe loop):

```rust,ignore
use buiy::probe::*;
let mut t = MvuTestApp::new(update);
t.world_mut().spawn(Checkbox::new("Dark mode"));
t.settle();
let cb = get_by_role(t.world_mut(), A11yRole::Checkbox, Some("Dark mode"), None).unwrap();
click(t.world_mut(), cb).unwrap();
t.step();
assert!(t.snapshot_report().contains("[checked]"));
```

For a multi-model test, reach `t.app_mut()` / `t.world_mut()` and register/drive the
extra models directly.

## The three tiers (pick the smallest that fits)

State granularity is tiered (spec §3). Pick the **smallest** tier that models the
state honestly:

| Tier | What it is | How to reach it |
|---|---|---|
| **Leaf** | one scalar/toggle owned by a shared reducer (the built-in `Checkbox`/`Switch` use the `A11yToggled` toggle leaf). Folds in an **early** caller-chosen window (`.after(Picking).before(A11yUpdate)`) so an AT-driver click reflects in the a11y tree the same frame. | built-in for the toggle widgets; add `ControlledLeaf` (`use buiy_core::mvu::ControlledLeaf;` — not re-exported through `buiy::prelude`) to a leaf to make its value **controlled by a parent model** (the view reconciler drives it; the built-in press-to-toggle is suppressed). |
| **Machine** | a **whole-state** `Model` + reducer — the default for a screen/app/list (a counter, a menu machine, a form, the TodoMVC list). Folds in the late `MvuSet::Drain`. | `mvu_model(reducer)`. |
| **Raw-ECS** | mutate components directly, **outside** MVU — no record/replay, no `set_if_neq`. | only for transient/derived view state that must NOT be recorded; anything that should replay belongs in a model. |

A tier that must fold at a specific point in the frame installs its drain in a
caller-chosen `SystemSet` via `add_reducer_in_set` (the early-window model). See
spec §3–§4 for the full rationale.

## Gotchas (the silent-wrong ones)

Each of these is a bug that produces a *plausible-looking* wrong result rather than
a crash — the class the app-author-ergonomics campaign exists to kill.

- **`set_if_neq` drain discipline — store DERIVED state, not raw inputs.** The drain
  commits with `Mut::set_if_neq`: a fold that leaves the value byte-identical does
  **not** trip `Changed<M>` and does **not** cascade bind → layout → re-extract.
  This is the perf floor. It also means a clock reducer that stored raw `now` would
  mutate every frame (a 60Hz re-extract cliff) — store the *derived* value.
  Observable in a test: after an idempotent fold,
  `world.resource::<MvuWorkCounters>().models_mutated == 0` and `binds_fired == 0`.

- **Enqueue, never fold.** Calling the reducer directly (or grabbing
  `Query<&mut Model>` in a handler) bypasses the funnel — no record tap, no
  `set_if_neq`, no single-writer guarantee, and in debug builds the §7.5 auditor
  flags the out-of-funnel write. Handlers `enqueue`; the drain folds.

- **Every Model entity needs a stable, unique `LogicalId` — now LOUD in debug
  (Track A).** `LogicalId` is the replay key. Two fail-silent footguns for
  raw-MVU authors are now surfaced in **debug builds only** (`#[cfg(debug_assertions)]`
  — compiled out of release); both route through one shared helper
  (`buiy_core::mvu::MvuDiagnostics::report`): a per-operation **`warn!`** *plus* a
  typed entry pushed onto the debug-only `MvuDiagnostics` resource (`.violations:
  Vec<MvuDiagnostic>`) that a test can assert on:
  - **Missing id (`MvuDiagnostic::UnresolvedRecordedFold { entity, .. }`).** A
    **recorded** fold on a `Model` entity with **no** `LogicalId` silently stamps
    the log entry `UNRESOLVED` and dead-letters on replay — corrupting the log
    before the (already-loud) replay-time dead-letter fires. Surfaced at the record
    site, so it fires **only while a `RecordSession` is recording** (an id-less fold
    corrupts nothing when not recording — e.g. a legitimate id-less `A11yToggled`
    checkbox), and it is **`ControlledLeaf`-exempt** (those leaves are intentionally
    model-reconstructed).
  - **Duplicate id (`MvuDiagnostic::DuplicateLogicalId { id, entities }`).** Two
    entities of the **same** model type sharing one `LogicalId` — replay's
    `resolve_lid` silently picks the first, mis-routing folds. Detected in the
    bind-stage auditor (change-gated on `Changed<LogicalId>`), **within-type**
    (cross-type collisions surface as replay dead-letters, the already-loud path).

  The fix is always: give every model entity a stable, unique id. `MvuTestApp::spawn`
  does this for you (auto-incrementing, skipping past any `spawn_with_id`); the view
  reconciler seeds `MODEL_LID` — so this bites **hand-rolled** raw-MVU models.

- **`BoxSizing` defaults to `ContentBox` — an authored `.width(72)` is the *content*
  width.** A padded `button()` (8px each side) with `.width(72)` renders an **88px**
  border box; pick (`ResolvedLayout.size`) and paint agree on 88 — this is a *sizing
  surprise*, not a picking bug. To size the **outer** box, account for the padding or
  set `BoxSizing::BorderBox` (the `Style::border_box()` builder). App UIs usually
  prefer `border_box()`.

## Consolidates

This skill is the task-oriented home for MVU authoring. The scattered coverage it
consolidates — `AGENTS.md` §State (the agent front-door snippet), `docs/guide/
getting-started.md` §8 (the linear tutorial intro), and the `mvu` module doctest —
stays as short intros that point here; this skill is where the recipes + gotchas live.

## Running the tests

```sh
# The guide's compiled+run examples + the MvuTestApp dogfood (test-support is auto-on
# for buiy's own dev builds via a self dev-dependency):
cargo test -p buiy --test using_mvu_examples
cargo test -p buiy --doc            # the module + method doctests

# The MVU substrate gates (work counters, drain, record/replay, the §7.5 + Track-A
# debug auditors) live in buiy_core:
cargo test -p buiy_core --test mvu  # (via the crosscut harness)
```

The full headless workspace gate (must stay green **without** a GPU):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets --locked -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked && \
  xvfb-run -a cargo test --workspace --locked   # drop xvfb-run on macOS/Windows
```

## Verify before claiming an MVU change "works"

Run the actual test and read the output — the campaign's dominant lesson is
*headless-green ≠ works*. For a state change, drive it through `MvuTestApp` (or the
running app / the probe) and assert the **live model** and, where relevant, the
**work counters** (`models_mutated`/`binds_fired`) — not a "should fold" assertion.
For a new detection/diagnostic, prove it goes RED on the footgun it targets (spawn
an id-less model while recording → assert the `MvuDiagnostic` fires), then confirm
the correct case stays clean. See `superpowers:verification-before-completion`.
