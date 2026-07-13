//! Track C 5a/5b-2 — the compile-and-RUN gate for the **`using-mvu` guide**
//! (`.claude/skills/using-mvu/SKILL.md`) and the dogfood for the `MvuTestApp`
//! harness (`buiy::test`, 5b-2).
//!
//! Every recipe the guide shows is authored here as a compiled example — the
//! system-level snippets as functions (compile-checked), and the ones with an
//! observable result as `#[test]`s that RUN through [`MvuTestApp`] and assert.
//! A guide whose examples silently rot is exactly the fail-silent shape the
//! app-author-ergonomics campaign fights, so this file is the guide's anti-rot
//! net: change the API and this file (hence the guide) must be updated too.
//!
//! Gated on `test-support` (the harness's feature); buiy enables it for its own
//! dev builds via a self `dev-dependency`, so this runs under `cargo test -p buiy`.

#![cfg(feature = "test-support")]
#![allow(dead_code)]

use std::time::Duration;

use buiy::prelude::*;
use buiy::test::MvuTestApp;

// ── Recipe: define a Model + its messages + a pure reducer ────────────────────
// A model is a `Clone + PartialEq + Reflect` component; the reducer is a free
// `fn(&mut Model, Msg) -> Cmd<Msg>`.
#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter {
    value: i64,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
enum CounterMsg {
    Increment,
    Add(i64),
    Reset,
}

impl Model for Counter {
    type Msg = CounterMsg;
}

fn update(c: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => c.value += 1,
        CounterMsg::Add(n) => c.value += n,
        CounterMsg::Reset => c.value = 0,
    }
    Cmd::none()
}

// ── Recipe: enqueue from a handler (NEVER fold) ───────────────────────────────
// Handlers/observers/callbacks may only `enqueue`; the drain is the sole writer.
// Placed in `MvuSet::Enqueue`, the message folds the SAME frame.
fn enqueue_from_a_handler(mut commands: Commands, model: Query<Entity, With<Counter>>) {
    for e in &model {
        enqueue::<Counter>(&mut commands, e, CounterMsg::Increment);
    }
}

fn wire_the_handler(app: &mut App) {
    app.mvu_model(update)
        .app()
        .add_systems(Update, enqueue_from_a_handler.in_set(MvuSet::Enqueue));
}

// ── Recipe: read live WIDGET state through the domain accessors ───────────────
// Query the state component alongside the widget marker; the accessor returns a
// domain type (`bool` / `f64`), never the foreign `accesskit` enum.
fn read_live_widget_state(
    boxes: Query<&A11yToggled, With<Checkbox>>,
    switches: Query<&A11yToggled, With<Switch>>,
    sliders: Query<&A11yValue, With<Slider>>,
    disclosures: Query<&A11yExpanded, With<Disclosure>>,
) {
    let _checked = boxes.iter().filter(|t| Checkbox::checked(t)).count();
    let _on = switches.iter().filter(|t| Switch::on(t)).count();
    for v in &sliders {
        let _ = (Slider::value(v), Slider::fraction(v));
    }
    for e in &disclosures {
        let _ = Disclosure::expanded(e);
    }
}

// ── Recipe: react to a typed value change (not the untyped OnPress sink) ───────
fn on_toggle(mut changes: MessageReader<ValueChange<bool>>) {
    for c in changes.read() {
        let _ = (c.source, c.value, c.is_final);
    }
}

// ── Recipe: a time-driven (clock) model — store the DERIVED value, never `now` ─
#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Countdown {
    remaining: u64,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
enum ClockMsg {
    Tick(Duration),
}

impl Model for Countdown {
    type Msg = ClockMsg;
}

const TOTAL_SECS: u64 = 10;

fn countdown_update(m: &mut Countdown, msg: ClockMsg) -> Cmd<ClockMsg> {
    let ClockMsg::Tick(now) = msg;
    // DERIVE from `now`; storing `now` raw would mutate every frame and defeat
    // `set_if_neq` (the steady-frame no-cascade rule).
    m.remaining = TOTAL_SECS.saturating_sub(now.as_secs());
    Cmd::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// The DOGFOOD: drive a model in ~5 lines and prove it works.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn counter_dogfood_five_lines() {
    let mut t = MvuTestApp::new(update); // preset + reducer; model type inferred
    let e = t.spawn(Counter::default()); // spawned WITH a fresh LogicalId
    t.enqueue(e, CounterMsg::Increment); // enqueue — never fold
    t.step(); // one frame: inbox → drain → bind
    assert_eq!(t.read(e).value, 1); // read the live model
}

/// The `set_if_neq` gotcha, made observable: an idempotent fold (`Add(0)`) runs
/// the reducer but leaves the value identical, so it does NOT trip `Changed<M>`
/// and does NOT cascade a re-extract. Read the work counters off the world.
#[test]
fn idempotent_fold_is_absorbed_no_cascade() {
    let mut t = MvuTestApp::new(update);
    let e = t.spawn(Counter::default());
    t.settle(); // clear the spawn's Added/Changed ticks

    t.enqueue(e, CounterMsg::Add(0)); // folds, but the value is unchanged
    t.step();

    let counters = *t.world().resource::<MvuWorkCounters>();
    assert_eq!(counters.drain_folds, 1, "the fold still ran");
    assert_eq!(
        counters.models_mutated, 0,
        "set_if_neq absorbed the no-op — no mutation"
    );
    assert_eq!(
        counters.binds_fired, 0,
        "and therefore no cascade to the bind stage"
    );
    assert_eq!(t.read(e).value, 0);
}

/// A clock-driven model, driven deterministically by the virtual clock — no real
/// sleeps. `advance_clock` accumulates, so elapsed is 3s then 7s.
#[test]
fn clock_drives_derived_state() {
    let mut t = MvuTestApp::new(countdown_update).with_clock(ClockMsg::Tick);
    let e = t.spawn(Countdown::default());

    t.advance_clock(Duration::from_secs(3));
    assert_eq!(t.read(e).remaining, 7);

    t.advance_clock(Duration::from_secs(4)); // elapsed now 7s
    assert_eq!(t.read(e).remaining, 3);
}

/// Read (and drive) live WIDGET state through the probe on the same harness: the
/// full GPU-free preset means `get_by_role` / `click` / `snapshot_report` work.
#[test]
fn widget_state_via_probe() {
    use buiy::probe::*;

    let mut t = MvuTestApp::new(update); // any model registers the substrate
    t.world_mut().spawn(Checkbox::new("Dark mode"));
    t.settle();

    assert!(t.snapshot_report().contains("Checkbox \"Dark mode\""));

    let cb = get_by_role(t.world_mut(), A11yRole::Checkbox, Some("Dark mode"), None).unwrap();
    click(t.world_mut(), cb).unwrap();
    t.step(); // the toggle commits through the leaf funnel next step

    assert!(
        t.snapshot_report().contains("[checked]"),
        "the probe click flipped the checkbox state"
    );
}

/// The LogicalId gotcha, made LOUD (Track A): a *recorded* fold on a model with no
/// `LogicalId` corrupts the replay log — in debug builds it now emits a per-op
/// `warn!` + a typed `MvuDiagnostic::UnresolvedRecordedFold` entry a test asserts
/// on. `MvuTestApp::spawn` avoids this by always attaching a fresh id; this test
/// reproduces the footgun by spawning id-less directly on the world.
#[cfg(debug_assertions)]
#[test]
fn missing_logical_id_is_loud_in_debug() {
    use buiy_core::mvu::{MvuDiagnostic, MvuDiagnostics, RecordSession};

    let mut t = MvuTestApp::new(update);
    // Spawn WITHOUT a LogicalId — the raw-MVU-author footgun `spawn()` protects you from.
    let e = t.world_mut().spawn(Counter::default()).id();
    t.settle();
    t.world_mut().resource_mut::<RecordSession>().start(); // recording gates the check

    t.enqueue(e, CounterMsg::Increment);
    t.step();

    let flagged = t
        .world()
        .resource::<MvuDiagnostics>()
        .violations
        .iter()
        .any(|d| matches!(d, MvuDiagnostic::UnresolvedRecordedFold { entity, .. } if *entity == e));
    assert!(
        flagged,
        "a recorded fold on an id-less model fires the loud diagnostic"
    );
}

/// Registering compile-only recipe fns so they are type-checked (not DCE'd before
/// the check that gates the guide).
#[test]
fn guide_snippets_typecheck() {
    let _ = (
        wire_the_handler as fn(&mut App),
        read_live_widget_state
            as fn(
                Query<&A11yToggled, With<Checkbox>>,
                Query<&A11yToggled, With<Switch>>,
                Query<&A11yValue, With<Slider>>,
                Query<&A11yExpanded, With<Disclosure>>,
            ),
        on_toggle as fn(MessageReader<ValueChange<bool>>),
    );
}
