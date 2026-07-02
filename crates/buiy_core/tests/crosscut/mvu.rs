//! MVU substrate work-counter gates + end-to-end smoke (the L1 go/no-go, spec §11).
//!
//! These assertions ARE the L1 result. The load-bearing one is the **idempotent-fold
//! proof** (`mvu_idempotent_fold_no_mutation_no_bind`): a fold that doesn't change the value
//! leaves `models_mutated == 0` and `binds_fired == 0` — the `set_if_neq` discipline keeps a
//! no-op off the bind → re-extract cascade (spec §2). Counters mirror the render
//! work-counter gate (host-independent integers asserted EXACTLY on a settled scene).

use bevy::prelude::*;
use buiy_core::mvu::{
    Cmd, LogicalId, Model, MsgLog, MvuAppExt, MvuCorePlugin, MvuSet, MvuWorkCounters,
    RecordSession, enqueue,
};

// --- The demo model -------------------------------------------------------------------------
// `Clone + PartialEq` satisfy the `Model` trait's `set_if_neq` bounds.

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter {
    value: i64,
}

impl Model for Counter {
    type Msg = CounterMsg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum CounterMsg {
    Increment,
    /// `Add(0)` is the idempotent fold (no value change) — the `set_if_neq` test vector.
    Add(i64),
    /// Run-to-completion `Emit`: bump by 1 and re-fold until `value` reaches `target`, all in
    /// one drain pass.
    TickTo(i64),
}

fn counter_update(c: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => c.value += 1,
        CounterMsg::Add(n) => c.value += n,
        CounterMsg::TickTo(target) => {
            if c.value < target {
                c.value += 1;
                return Cmd::emit(CounterMsg::TickTo(target));
            }
        }
    }
    Cmd::none()
}

// --- Harness --------------------------------------------------------------------------------

/// Build a headless app wired for the Counter model (App::new + MvuCorePlugin only — the
/// minimal shape; no `CorePlugin`, so the MVU chain orders against empty `BuiySet` anchors).
fn counter_app() -> App {
    let mut app = App::new();
    app.add_plugins(MvuCorePlugin);
    app.register_type::<Counter>();
    app.add_model::<Counter>();
    app.add_reducer::<Counter, _>(counter_update);
    app
}

fn spawn_counter(app: &mut App, lid: u64) -> Entity {
    app.world_mut()
        .spawn((Counter::default(), LogicalId(lid)))
        .id()
}

/// Write a message straight to the inbox (the direct-inbox idiom — bypasses `enqueue` to
/// exercise the drain in isolation). The end-to-end `enqueue` path is covered by the smoke
/// test below.
fn write(app: &mut App, target: Entity, msg: CounterMsg) {
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<buiy_core::mvu::Envelope<Counter>>>()
        .write(buiy_core::mvu::Envelope::user(target, msg));
}

fn value(app: &App, e: Entity) -> i64 {
    app.world().get::<Counter>(e).unwrap().value
}

fn counters(app: &App) -> MvuWorkCounters {
    *app.world().resource::<MvuWorkCounters>()
}

/// Run frames to clear the spawn-time `Added`/`Changed` ticks so an idle frame reads all-0
/// (the counters are per-frame, reset at the head of the chain; a freshly spawned model is
/// `Changed` on its first observed frame — same settle discipline the render gate uses).
fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

// --- (a) idle ⇒ all counters 0 --------------------------------------------------------------

#[test]
fn mvu_idle_frame_all_counters_zero() {
    let mut app = counter_app();
    let _e = spawn_counter(&mut app, 1);
    settle(&mut app);

    app.update(); // one idle frame (no messages)
    let c = counters(&app);
    assert_eq!(c.drain_folds, 0, "idle: no folds");
    assert_eq!(c.messages_recorded, 0, "idle: nothing recorded");
    assert_eq!(c.models_mutated, 0, "idle: no model mutated");
    assert_eq!(
        c.binds_fired, 0,
        "idle: no Changed<Model> at the bind stage"
    );
    assert_eq!(c.emits_refolded, 0, "idle: no emits");
}

// --- (b) one message ⇒ drain_folds==1, models_mutated==1, Changed set -----------------------

#[test]
fn mvu_one_message_folds_once_and_mutates() {
    let mut app = counter_app();
    let e = spawn_counter(&mut app, 1);
    settle(&mut app);

    write(&mut app, e, CounterMsg::Increment);
    app.update();

    assert_eq!(value(&app, e), 1, "the reducer folded the message");
    let c = counters(&app);
    assert_eq!(c.drain_folds, 1, "exactly one fold");
    assert_eq!(c.models_mutated, 1, "a real change tripped set_if_neq");
    assert_eq!(c.binds_fired, 1, "Changed<Counter> reached the bind stage");
    assert_eq!(c.emits_refolded, 0, "no Emit on a plain Increment");
}

// --- (c) THE load-bearing proof: idempotent fold ⇒ models_mutated==0, binds_fired==0 --------

#[test]
fn mvu_idempotent_fold_no_mutation_no_bind() {
    let mut app = counter_app();
    let e = spawn_counter(&mut app, 1);
    settle(&mut app);

    // Add(0) folds (drain_folds == 1) but leaves the value identical → set_if_neq must NOT
    // deref_mut → Changed<Counter> untripped → no bind cascade. This is the H3 perf rule.
    write(&mut app, e, CounterMsg::Add(0));
    app.update();

    assert_eq!(value(&app, e), 0, "the value is unchanged");
    let c = counters(&app);
    assert_eq!(c.drain_folds, 1, "the fold still ran");
    assert_eq!(
        c.models_mutated, 0,
        "LOAD-BEARING: an idempotent fold does NOT mutate (set_if_neq no-op)"
    );
    assert_eq!(
        c.binds_fired, 0,
        "LOAD-BEARING: the no-op does NOT cascade to the bind stage (Changed<Model> untripped)"
    );
}

// --- (d) Emit run-to-completion folds back in ONE drain pass --------------------------------

#[test]
fn mvu_emit_runs_to_completion_in_one_drain() {
    let mut app = counter_app();
    let e = spawn_counter(&mut app, 1);
    settle(&mut app);

    // TickTo(3) from 0: folds at value 0,1,2 (each bumps + re-emits) then a terminal no-op
    // fold at value 3 → 4 folds, 3 emits, 3 real mutations, all in this one drain pass.
    write(&mut app, e, CounterMsg::TickTo(3));
    app.update();

    assert_eq!(
        value(&app, e),
        3,
        "Emit re-folded to the target in one pass"
    );
    let c = counters(&app);
    assert_eq!(c.drain_folds, 4, "3 bumps + 1 terminal no-op fold");
    assert_eq!(c.emits_refolded, 3, "3 Emits re-queued");
    assert_eq!(
        c.models_mutated, 3,
        "the terminal fold is an idempotent no-op (set_if_neq)"
    );
    assert_eq!(
        c.binds_fired, 1,
        "the entity is Changed once (mutated in-pass), bound once"
    );
}

// --- (e) RecordMode gates the record tap (default OFF pays zero) -----------------------------

#[test]
fn mvu_record_mode_off_records_nothing_full_records() {
    let mut app = counter_app();
    let e = spawn_counter(&mut app, 42);
    settle(&mut app);

    // Default mode is Off → a fold records nothing.
    write(&mut app, e, CounterMsg::Increment);
    app.update();
    assert_eq!(
        counters(&app).messages_recorded,
        0,
        "RecordMode::Off records nothing"
    );
    assert!(
        app.world().resource::<MsgLog>().entries.is_empty(),
        "RecordMode::Off leaves the log empty (production pays zero — spec §7.1)"
    );

    // Turn recording on (the shared record session) → the next fold is recorded, keyed by
    // LogicalId, Reflect-serialized. `RecordSession::start` sets RecordMode::Full + resets
    // the global seq; the log was already empty (Off recorded nothing), so it stays clean.
    app.world_mut().resource_mut::<RecordSession>().start();
    write(&mut app, e, CounterMsg::Increment);
    app.update();
    assert_eq!(
        counters(&app).messages_recorded,
        1,
        "RecordMode::Full records the fold"
    );
    let log = app.world().resource::<MsgLog>();
    assert_eq!(log.entries.len(), 1, "exactly one entry");
    assert_eq!(
        log.entries[0].lid,
        LogicalId(42),
        "keyed by the actor's LogicalId"
    );
    assert!(
        log.entries[0].ron.contains("Increment"),
        "the RON is the real message"
    );
    assert!(
        log.entries[0].type_path.contains("CounterMsg"),
        "with its type path"
    );
}

// --- (#4) end-to-end smoke: the FULL path through `enqueue` ----------------------------------

#[derive(Resource)]
struct PendingPress(Entity);

/// An `Enqueue`-set producer that fires ONE message via `enqueue` (the sanctioned mutation
/// point) when a `PendingPress` is present, then removes it. Proves the whole funnel chain in
/// one frame: Enqueue → (pinned ApplyDeferred flush) → Drain fold → Bind.
fn fire_once(pending: Option<Res<PendingPress>>, mut commands: Commands) {
    if let Some(p) = pending {
        enqueue::<Counter>(&mut commands, p.0, CounterMsg::Increment);
        commands.remove_resource::<PendingPress>();
    }
}

#[test]
fn mvu_smoke_enqueue_drives_full_path_in_one_frame() {
    let mut app = counter_app();
    app.add_systems(Update, fire_once.in_set(MvuSet::Enqueue));
    let e = spawn_counter(&mut app, 7);
    settle(&mut app); // PendingPress absent → fire_once no-ops; spawn ticks clear

    // Arm the producer; one frame must enqueue → flush → drain → bind, end to end.
    app.world_mut().insert_resource(PendingPress(e));
    app.update();

    assert_eq!(
        value(&app, e),
        1,
        "enqueue → ApplyDeferred → drain folded in ONE frame"
    );
    let c = counters(&app);
    assert_eq!(
        c.drain_folds, 1,
        "the enqueued message was drained this frame"
    );
    assert_eq!(c.models_mutated, 1, "the model changed");
    assert_eq!(c.binds_fired, 1, "a bind fired on the change");

    // The producer disarmed itself; a second frame folds nothing more.
    app.update();
    assert_eq!(value(&app, e), 1, "no spurious re-fire");
    assert_eq!(
        counters(&app).drain_folds,
        0,
        "idle again after the one-shot"
    );
}

// ============================================================================
// The stateful-leaf tier (spec §3): `A11yToggled` as a shared-reducer leaf.
//
// These prove, at the SUBSTRATE level (a bare `A11yToggled` + the shared
// `toggle_reducer`, no widget plumbing), the two load-bearing single-writer properties
// the cure (SYNTHESIS D3) rests on: the drain is the sole writer and a redundant `Set`
// is a no-op that cannot cascade (the flicker cannot occur). The end-to-end OnPress →
// drain reroute through the real Checkbox/Switch is covered in
// `buiy_widgets/tests/mvu_single_writer.rs`.
// ============================================================================

use buiy_core::a11y::{A11yToggled, Toggled};
use buiy_core::mvu::{ToggleMsg, register_toggle_leaf};

/// App::new + MvuCorePlugin + the shared toggle-leaf model/reducer (the leaf tier's
/// `A11yToggled` wiring, in isolation — no CorePlugin/a11y, so the chain orders against
/// empty `BuiySet` anchors, the same minimal shape as `counter_app`).
fn toggle_app() -> App {
    let mut app = App::new();
    app.add_plugins(MvuCorePlugin);
    register_toggle_leaf(&mut app);
    app
}

/// Write a `ToggleMsg` straight to the leaf inbox (bypasses the widget activation router to
/// exercise the shared reducer + drain in isolation).
fn toggle_write(app: &mut App, target: Entity, msg: ToggleMsg) {
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<buiy_core::mvu::Envelope<A11yToggled>>>()
        .write(buiy_core::mvu::Envelope::user(target, msg));
}

fn toggled(app: &App, e: Entity) -> Toggled {
    app.world().get::<A11yToggled>(e).unwrap().0
}

#[test]
fn leaf_toggle_flips_once_and_drain_is_sole_writer() {
    let mut app = toggle_app();
    let e = app
        .world_mut()
        .spawn((A11yToggled::default(), LogicalId(1)))
        .id();
    settle(&mut app);

    toggle_write(&mut app, e, ToggleMsg::Toggle);
    app.update();

    assert_eq!(
        toggled(&app, e),
        Toggled::True,
        "False → True via the drain"
    );
    let c = counters(&app);
    assert_eq!(c.drain_folds, 1, "exactly one fold");
    assert_eq!(
        c.models_mutated, 1,
        "the drain wrote A11yToggled EXACTLY once (no double-write)"
    );
    assert_eq!(
        c.binds_fired, 1,
        "Changed<A11yToggled> reached the bind stage once (NOT twice)"
    );

    // No other system writes A11yToggled: the next idle frame folds/mutates nothing and
    // the value is stable (no second writer flips it back).
    app.update();
    let c2 = counters(&app);
    assert_eq!(c2.drain_folds, 0, "idle: nothing re-enqueued");
    assert_eq!(
        c2.models_mutated, 0,
        "idle: the drain is the ONLY writer — no spurious mutation"
    );
    assert_eq!(toggled(&app, e), Toggled::True, "value stable");
}

#[test]
fn leaf_redundant_set_is_a_noop_no_cascade() {
    let mut app = toggle_app();
    // Default is `False`; `Set(false)` is the redundant (controlled-parent re-assert) write.
    let e = app
        .world_mut()
        .spawn((A11yToggled::default(), LogicalId(1)))
        .id();
    settle(&mut app);

    toggle_write(&mut app, e, ToggleMsg::Set(false));
    app.update();

    assert_eq!(toggled(&app, e), Toggled::False, "the value is unchanged");
    let c = counters(&app);
    assert_eq!(c.drain_folds, 1, "the fold still ran");
    assert_eq!(
        c.models_mutated, 0,
        "LOAD-BEARING: a redundant Set is an idempotent no-op (set_if_neq does NOT deref_mut)"
    );
    assert_eq!(
        c.binds_fired, 0,
        "LOAD-BEARING: the no-op does NOT cascade (Changed<A11yToggled> untripped) — the one-frame flicker cannot occur"
    );
}

// ============================================================================
// The L1 perf GO/NO-GO gate (spec §11, D11): a CAN-FAIL `BlinkLeaf` fixture.
//
// A per-frame-changing `Tick(now)` is routed through the funnel, but the reducer
// stores only the DERIVED 500ms blink phase, projected to a STRUCTURAL change
// (`ComputedPaintSkip` toggle) on a node the render extract sees. The HARD binary
// gate: on every STEADY frame `models_mutated == binds_fired == node_rebuilds ==
// node_patches == 0`; on a FLIP frame the projected counter `node_rebuilds == 1`.
//
// Run HEADLESS on the adapter-free `buiy_bench_support` harness (no wgpu adapter —
// `node_rebuilds` is set CPU-side in `record_node_counts`, BEFORE any GPU work). The
// gate CAN FAIL: a reducer that stored `now` directly would mutate every frame and
// redden the steady-frame assertions. A failed gate is a SUCCESSFUL outcome — it
// kills the over-claim with a number and narrows the framing per §11.
// ============================================================================

use buiy_bench_support::PipelineHarness;
use buiy_bench_support::mvu_scenes::{build_blink_render_scene, tick_blink};
use buiy_core::render::RenderWorkCounters;
use std::time::Duration;

fn render_counters(h: &PipelineHarness) -> RenderWorkCounters {
    *h.render.resource::<RenderWorkCounters>()
}

fn mvu_counters(h: &PipelineHarness) -> MvuWorkCounters {
    *h.app.world().resource::<MvuWorkCounters>()
}

#[test]
fn blink_funneled_node_rebuilds_zero() {
    let (mut h, node) = build_blink_render_scene();

    // SETTLE: drive steady ticks in the phase==true bucket (ms 100, bucket 0) until the
    // spawn `Added` ticks clear and the extract damage gate reaches O(0).
    for _ in 0..8 {
        tick_blink(&mut h, node, Duration::from_millis(100));
    }
    assert_eq!(
        render_counters(&h).node_rebuilds,
        0,
        "scene settled: the damage gate skips on a steady frame"
    );
    assert_eq!(
        mvu_counters(&h).models_mutated,
        0,
        "settled: the derived phase is stable in-bucket"
    );

    // The cadence is driven by the `now` we feed (deterministic — not real time):
    //   bucket = now_ms / 500 ;  phase = (bucket % 2 == 0) ;  hidden <=> phase == false.
    //   ms:    100 200 300 400 | 600  | 700  | 1000 | 1100
    //   bkt:     0   0   0   0  |  1   |  1   |   2  |   2
    //   phase:   T   T   T   T  |  F   |  F   |   T  |   T
    //   event: ---- steady ----| FLIP | stdy | FLIP | stdy

    // --- STEADY frames: every counter zero (the no-cascade property). -----------------
    for &ms in &[100u64, 200, 300, 400] {
        tick_blink(&mut h, node, Duration::from_millis(ms));
        let m = mvu_counters(&h);
        let r = render_counters(&h);
        assert_eq!(m.drain_folds, 1, "the per-frame Tick({ms}ms) folded");
        assert_eq!(
            m.models_mutated, 0,
            "STEADY {ms}ms: set_if_neq absorbed the Tick (derived phase unchanged)"
        );
        assert_eq!(
            m.binds_fired, 0,
            "STEADY {ms}ms: no Changed<BlinkLeaf> cascade to the bind"
        );
        assert_eq!(
            r.node_rebuilds, 0,
            "STEADY {ms}ms: no structural re-extract"
        );
        assert_eq!(r.node_patches, 0, "STEADY {ms}ms: no patch either");
    }

    // --- FLIP (hide): cross to bucket 1 (phase false ⇒ ComputedPaintSkip INSERTED ⇒
    //     Changed<ComputedPaintSkip> ⇒ extract Full rebuild). ----------------------------
    tick_blink(&mut h, node, Duration::from_millis(600));
    let m = mvu_counters(&h);
    let r = render_counters(&h);
    assert_eq!(
        m.models_mutated, 1,
        "FLIP: the phase changed (set_if_neq tripped exactly once)"
    );
    assert_eq!(
        m.binds_fired, 1,
        "FLIP: Changed<BlinkLeaf> reached the bind"
    );
    assert_eq!(
        r.node_rebuilds, 1,
        "FLIP: the structural projection forced exactly one Full rebuild"
    );

    // --- STEADY again within bucket 1 (phase still false ⇒ no change). -----------------
    tick_blink(&mut h, node, Duration::from_millis(700));
    assert_eq!(
        mvu_counters(&h).models_mutated,
        0,
        "STEADY again: phase stable in-bucket"
    );
    assert_eq!(
        render_counters(&h).node_rebuilds,
        0,
        "STEADY again: no re-extract"
    );

    // --- FLIP (show): cross to bucket 2 (phase true ⇒ ComputedPaintSkip REMOVED ⇒ the
    //     RemovedComponents lift stream ⇒ Full rebuild). --------------------------------
    tick_blink(&mut h, node, Duration::from_millis(1000));
    assert_eq!(
        mvu_counters(&h).models_mutated,
        1,
        "FLIP back: the phase changed once"
    );
    assert_eq!(
        render_counters(&h).node_rebuilds,
        1,
        "FLIP back: the paint-skip LIFT forced a Full rebuild"
    );

    // --- STEADY in bucket 2 (phase true ⇒ no change). ----------------------------------
    tick_blink(&mut h, node, Duration::from_millis(1100));
    assert_eq!(
        mvu_counters(&h).models_mutated,
        0,
        "STEADY: phase stable in-bucket"
    );
    assert_eq!(
        render_counters(&h).node_rebuilds,
        0,
        "STEADY: no re-extract"
    );
}

// ============================================================================
// §7.5 — the debug-only write-outside-the-funnel auditor.
//
// 4 cases prove it fires ONLY on a genuine RUNTIME raw write of Model state:
//   (1) a legit batch-drain fold,
//   (2) a legit inline AT-seam fold (`fold_one_inline`),
//   (3) a spawn-time SEED (a write BEFORE the entity's first fold — §10),
//   (4) a planted runtime raw write AFTER a fold — the ONLY case that fires.
//
// The auditor stamps in `fold_one_with` + judges inside the bind-stage `count_binds`
// (a debug-only cfg arm of the SAME system — no new system, so no entity-id drift),
// both `cfg(debug_assertions)` — invisible to release/bench builds and to every work
// counter (it writes only a separate resource, no archetype move). Gated to debug so
// it does not even compile under `--release` (where the resources are absent). The
// blink/idempotent counter gates above run unchanged alongside it — proof the stamp
// does not perturb `models_mutated`/`binds_fired`/`node_rebuilds`.
// ============================================================================
#[cfg(debug_assertions)]
mod funnel_auditor {
    use super::*;
    use buiy_core::mvu::{FunnelAuditLog, fold_one_inline};

    fn violations(app: &App) -> usize {
        app.world().resource::<FunnelAuditLog>().violations.len()
    }

    /// Raw-write a `Counter` value OUTSIDE the funnel — the escape hatch the auditor exists
    /// to catch (a direct ECS write, no `enqueue`).
    fn raw_write(app: &mut App, e: Entity, v: i64) {
        app.world_mut().get_mut::<Counter>(e).unwrap().value = v;
    }

    #[test]
    fn auditor_fires_only_on_runtime_violation() {
        // (1) LEGIT BATCH-DRAIN FOLD — enqueue + drain, then a frame. No violation: the
        //     drain is the sanctioned writer and stamps itself.
        {
            let mut app = counter_app();
            let e = spawn_counter(&mut app, 1);
            settle(&mut app);
            write(&mut app, e, CounterMsg::Add(5));
            app.update();
            assert_eq!(value(&app, e), 5, "the fold applied");
            assert_eq!(
                violations(&app),
                0,
                "(1) a funnel fold is the sanctioned write — no violation"
            );
        }

        // (2) LEGIT INLINE AT-SEAM FOLD — `fold_one_inline` bypasses the inbox but folds
        //     through the SAME `fold_one_with` body, so it stamps too. No violation.
        {
            let mut app = counter_app();
            let e = spawn_counter(&mut app, 2);
            settle(&mut app);
            let changed =
                fold_one_inline::<Counter>(app.world_mut(), e, CounterMsg::Add(3), counter_update);
            assert!(changed, "the inline fold changed the model");
            app.update();
            assert_eq!(value(&app, e), 3, "the inline fold applied");
            assert_eq!(
                violations(&app),
                0,
                "(2) an AT-seam inline fold stamps like the drain — no violation"
            );
        }

        // (3) SPAWN-TIME SEED — a write BEFORE the entity's first fold is authored initial
        //     state (§10), never a violation. Seed a non-default value and settle WITHOUT
        //     ever folding.
        {
            let mut app = counter_app();
            let e = app
                .world_mut()
                .spawn((Counter { value: 42 }, LogicalId(3)))
                .id();
            settle(&mut app);
            assert_eq!(value(&app, e), 42, "the seed stuck");
            assert_eq!(
                violations(&app),
                0,
                "(3) a pre-first-fold seed is authored initial state — no violation"
            );
        }

        // (4) PLANTED RUNTIME VIOLATION — fold once (stamping the entity), THEN raw-write
        //     `Counter` directly (outside the funnel) and run a frame. The auditor fires
        //     exactly once and names the offender.
        {
            let mut app = counter_app();
            let e = spawn_counter(&mut app, 4);
            settle(&mut app);
            // A legit fold first → the entity is now past its first funnel write (stamped).
            write(&mut app, e, CounterMsg::Add(1));
            app.update();
            assert_eq!(violations(&app), 0, "the fold itself is clean");
            // The escape hatch: a raw ECS write of Model state, no `enqueue`.
            raw_write(&mut app, e, 99);
            app.update();
            assert_eq!(value(&app, e), 99, "the raw write applied");
            assert_eq!(
                violations(&app),
                1,
                "(4) a runtime write OUTSIDE the funnel after a fold fires the auditor once"
            );
            assert_eq!(
                app.world().resource::<FunnelAuditLog>().violations[0].entity,
                e,
                "the violation names the offending entity"
            );
        }
    }
}

// ============================================================================
// `Cmd::task` — async-as-a-value (buiy_view design §3 #15).
//
// Three properties:
//   (1) a `Cmd::task` result folds back through the funnel stamped `Origin::Command`
//       (the origin-aware transport), running the effect exactly once;
//   (2) `Cmd::map` lifts a child reducer's emitted `Cmd` into the parent's `Msg` — the
//       effect-side companion to `Element::map`;
//   (3) THE determinism guarantee: replay re-folds the recorded `Origin::Command` result
//       and SUPPRESSES re-launching the effect — so a non-deterministic effect replays
//       from what actually happened, not by re-running it.
// ============================================================================
mod cmd_task {
    use super::*;
    use buiy_core::mvu::Origin;
    use buiy_core::replay::replay_into;
    use buiy_core::text::edit::EditLog;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use bevy::tasks::{AsyncComputeTaskPool, TaskPool};

    // --- A model whose `Load` launches an async effect that folds back as `Loaded`. -----

    #[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
    #[reflect(Component)]
    struct Loader {
        loading: bool,
        result: Option<i64>,
        loads: u32,
    }
    impl Model for Loader {
        type Msg = LoadMsg;
    }

    #[derive(Clone, Debug, Reflect, PartialEq)]
    enum LoadMsg {
        Load,
        Loaded(i64),
    }

    /// Build a Loader app whose async effect bumps `effect` (a shared side-effect counter) —
    /// so a test can prove the effect ran (live) or did NOT (suppressed under replay). Inits
    /// the compute pool (App::new does not) so a `Cmd::task` can actually spawn.
    fn loader_app(effect: Arc<AtomicUsize>) -> App {
        AsyncComputeTaskPool::get_or_init(TaskPool::default);
        let mut app = App::new();
        app.add_plugins(MvuCorePlugin);
        app.register_type::<Loader>();
        app.add_model::<Loader>();
        app.add_reducer::<Loader, _>(move |m: &mut Loader, msg: LoadMsg| match msg {
            LoadMsg::Load => {
                m.loading = true;
                let effect = effect.clone();
                // The effect (network/db/RNG stand-in): a side-effecting future the drain
                // spawns. `map` tags the result back into `LoadMsg::Loaded`.
                Cmd::task(
                    async move {
                        effect.fetch_add(1, Ordering::SeqCst);
                        42i64
                    },
                    LoadMsg::Loaded,
                )
            }
            LoadMsg::Loaded(v) => {
                m.loading = false;
                m.result = Some(v);
                m.loads += 1;
                Cmd::none()
            }
        });
        app
    }

    fn spawn_loader(app: &mut App, lid: u64) -> Entity {
        app.world_mut()
            .spawn((Loader::default(), LogicalId(lid)))
            .id()
    }

    fn write_load(app: &mut App, target: Entity, msg: LoadMsg) {
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<buiy_core::mvu::Envelope<Loader>>>()
            .write(buiy_core::mvu::Envelope::user(target, msg));
    }

    fn loader(app: &App, e: Entity) -> Loader {
        app.world().get::<Loader>(e).unwrap().clone()
    }

    /// Run frames (each ticks the poll) until the async result folds back, or a frame budget
    /// elapses. Returns whether it folded (a trivial task completes near-instantly on the
    /// threaded compute pool, but the exact frame is timing-dependent — so we poll a bounded loop).
    fn run_until_loaded(app: &mut App, e: Entity) -> bool {
        for _ in 0..200 {
            app.update();
            if loader(app, e).result.is_some() {
                return true;
            }
        }
        false
    }

    // --- (1) the result folds back stamped `Origin::Command` ----------------------------

    #[test]
    fn task_result_folds_back_stamped_command() {
        let effect = Arc::new(AtomicUsize::new(0));
        let mut app = loader_app(effect.clone());
        let e = spawn_loader(&mut app, 1);
        settle(&mut app);
        // Record so we can inspect the logged origins.
        app.world_mut().resource_mut::<RecordSession>().start();

        write_load(&mut app, e, LoadMsg::Load);
        assert!(
            run_until_loaded(&mut app, e),
            "the async task result folded back within the frame budget"
        );

        let m = loader(&app, e);
        assert_eq!(
            m.result,
            Some(42),
            "the mapped result folded into the model"
        );
        assert_eq!(m.loads, 1, "exactly one load completed");
        assert!(!m.loading, "the result fold cleared `loading`");
        assert_eq!(
            effect.load(Ordering::SeqCst),
            1,
            "the effect ran EXACTLY once (the drain spawned it once)"
        );

        // The log has exactly two folds: the user `Load`, then the command `Loaded`.
        let log = app.world().resource::<MsgLog>();
        assert_eq!(log.entries.len(), 2, "Load + Loaded recorded");
        assert_eq!(
            log.entries[0].origin,
            Origin::User,
            "the user-initiated Load recorded as Origin::User"
        );
        assert!(log.entries[0].ron.contains("Load"), "entry 0 is the Load");
        assert_eq!(
            log.entries[1].origin,
            Origin::Command,
            "LOAD-BEARING: the async result recorded as Origin::Command (the origin-aware transport)"
        );
        assert!(
            log.entries[1].ron.contains("Loaded"),
            "entry 1 is the async Loaded(42)"
        );
    }

    // --- (2) `Cmd::map` lifts a child reducer's emitted Cmd into the parent's Msg --------

    #[derive(Clone, Debug, PartialEq, Reflect, Default)]
    struct Child {
        n: i64,
    }
    #[derive(Clone, Debug, Reflect, PartialEq)]
    enum ChildMsg {
        /// Bumps by 1 and EMITS a follow-up (the effect the parent must re-fold via `map`).
        Bump,
        BumpAgain,
    }
    fn child_update(c: &mut Child, m: ChildMsg) -> Cmd<ChildMsg> {
        match m {
            ChildMsg::Bump => {
                c.n += 1;
                Cmd::emit(ChildMsg::BumpAgain)
            }
            ChildMsg::BumpAgain => {
                c.n += 10;
                Cmd::none()
            }
        }
    }

    #[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
    #[reflect(Component)]
    struct Parent {
        child: Child,
    }
    impl Model for Parent {
        type Msg = ParentMsg;
    }
    #[derive(Clone, Debug, Reflect, PartialEq)]
    enum ParentMsg {
        Child(ChildMsg),
    }
    // The parent delegates to the child reducer on its owned sub-state and LIFTS the returned
    // command with `.map(ParentMsg::Child)` — so the child's emitted `BumpAgain` re-folds
    // through the parent as `ParentMsg::Child(BumpAgain)`.
    fn parent_update(p: &mut Parent, m: ParentMsg) -> Cmd<ParentMsg> {
        match m {
            ParentMsg::Child(cm) => child_update(&mut p.child, cm).map(ParentMsg::Child),
        }
    }

    #[test]
    fn cmd_map_lifts_child_emit_into_parent() {
        let mut app = App::new();
        app.add_plugins(MvuCorePlugin);
        app.register_type::<Parent>();
        app.add_model::<Parent>();
        app.add_reducer::<Parent, _>(parent_update);
        let e = app
            .world_mut()
            .spawn((Parent::default(), LogicalId(1)))
            .id();
        settle(&mut app);

        // One `Child(Bump)`: child_update bumps n→1 and emits BumpAgain; `.map` re-tags it to
        // ParentMsg::Child(BumpAgain), which re-folds in the SAME drain pass (n += 10 → 11).
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<buiy_core::mvu::Envelope<Parent>>>()
            .write(buiy_core::mvu::Envelope::user(
                e,
                ParentMsg::Child(ChildMsg::Bump),
            ));
        app.update();

        assert_eq!(
            app.world().get::<Parent>(e).unwrap().child.n,
            11,
            "LOAD-BEARING: Cmd::map lifted the child's emitted BumpAgain — it re-folded through the parent"
        );
        let c = counters(&app);
        assert_eq!(c.drain_folds, 2, "Bump + the lifted BumpAgain");
        assert_eq!(c.emits_refolded, 1, "the lifted Emit was re-queued once");
    }

    // --- (3) THE determinism guarantee: replay re-folds the result, effect NOT re-run ---

    #[test]
    fn replay_replays_command_result_without_rerunning_effect() {
        // LIVE run: record a Load session. The effect runs once, the result folds back.
        let live_effect = Arc::new(AtomicUsize::new(0));
        let mut live = loader_app(live_effect.clone());
        let e = spawn_loader(&mut live, 7);
        settle(&mut live);
        live.world_mut().resource_mut::<RecordSession>().start();
        write_load(&mut live, e, LoadMsg::Load);
        assert!(run_until_loaded(&mut live, e), "live: the task completed");
        assert_eq!(loader(&live, e).result, Some(42), "live: result folded");
        assert_eq!(
            live_effect.load(Ordering::SeqCst),
            1,
            "live: the effect ran once"
        );

        // Snapshot the recorded widget log (no editor entries in this session).
        let msg_log = MsgLog {
            entries: live.world().resource::<MsgLog>().entries.clone(),
        };
        let edit_log = EditLog::default();

        // REPLAY into a FRESH app (same seed / same LogicalId, a SEPARATE effect counter).
        let replay_effect = Arc::new(AtomicUsize::new(0));
        let mut fresh = loader_app(replay_effect.clone());
        let _e2 = spawn_loader(&mut fresh, 7); // SAME LogicalId(7) as recorded
        settle(&mut fresh);

        let dead = replay_into(&mut fresh, &msg_log, &edit_log);
        assert!(dead.is_empty(), "no dead letters — the seed matches");

        // The model reached the SAME state as the live run...
        let m = fresh
            .world_mut()
            .query::<&Loader>()
            .iter(fresh.world())
            .next()
            .expect("loader present")
            .clone();
        assert_eq!(m.result, Some(42), "replay reproduced the model state");
        assert_eq!(m.loads, 1, "replay folded the recorded Loaded exactly once");
        // ...but the effect was NOT re-run: the recorded Origin::Command result was re-folded
        // directly, and the drain SUPPRESSED re-launching the `Cmd::Task` under `is_replaying`.
        assert_eq!(
            replay_effect.load(Ordering::SeqCst),
            0,
            "LOAD-BEARING: replay did NOT re-run the effect — the recorded result re-drove the model"
        );
    }
}
