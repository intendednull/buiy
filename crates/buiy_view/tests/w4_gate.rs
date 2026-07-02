//! **The W4 go/no-go gate** (spec §5 — the load-bearing can-fail test).
//!
//! Question: does rebuild-then-diff defeat `set_if_neq` at steady state? The
//! [`ViewWorkCounters`] gate answers it on a settled Counter, with `nodes_patched`
//! counting only REAL value-changing writes (an untripped `set_if_neq` does not
//! count — so a walk-and-write-everything reconciler cannot pass by touching all
//! nodes). The four assertions (spec §5):
//!
//! 1. **Idle frame** ⇒ `ViewWorkCounters.reconciles == 0` (the reconciler is
//!    `Changed<M>`-gated; an idempotent fold never trips it).
//! 2. **Idempotent fold** ⇒ `MvuWorkCounters.models_mutated == 0` AND
//!    `binds_fired == 0` AND `ViewWorkCounters.reconciles == 0` (no cascade).
//! 3. **Localized value change** (`Inc`) ⇒ reconcile runs once,
//!    `nodes_spawned == 0 && nodes_despawned == 0`, and `nodes_patched == 1`
//!    (EXACTLY the one `Count` label — not the loose `>= 1`).
//! 4. **Downstream bound** ⇒ the localized change's layout dirty-set stays a
//!    small constant (patching one label does NOT re-dirty the whole tree's
//!    layout). Measured via `SyncStylesIterCount` (the per-frame count of nodes
//!    layout re-translated); a whole-tree re-dirty would spike it to ~N.

mod common;

use bevy::prelude::*;
use buiy_core::layout::SyncStylesIterCount;
use buiy_core::mvu::{Cmd, Envelope, Model, MvuWorkCounters};
use buiy_view::{
    BuiyViewAppExt, Element, Kind, Space, ViewWorkCounters, button, column, entities_of_kind, row,
    text,
};

// --- The settled Counter (the same surface the reconcile test authors) ------

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter {
    count: i32,
}
impl Model for Counter {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Inc,
    Reset,
}

fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Inc => s.count += 1,
        Msg::Reset => s.count = 0,
    }
    Cmd::none()
}

fn view(s: &Counter) -> Element<Msg> {
    column![
        text!("Count: {}", s.count).size(48.0),
        row![
            button("+").on_press(Msg::Inc),
            button("Reset").on_press_maybe((s.count != 0).then_some(Msg::Reset)),
        ]
        .gap(Space::Sm),
    ]
    .gap(Space::Md)
    .padding(Space::Xl)
    .align_center()
}

fn counter_app() -> App {
    let mut app = common::logic_app();
    app.ui(Counter::default(), update, view);
    app
}

// --- Helpers ----------------------------------------------------------------

fn model_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<Counter>>()
        .iter(world)
        .next()
        .expect("model entity")
}

/// Enqueue one message onto the model inbox (does NOT advance any frame).
fn enqueue(app: &mut App, msg: Msg) {
    let model = model_entity(app);
    app.world_mut()
        .resource_mut::<Messages<Envelope<Counter>>>()
        .write(Envelope::user(model, msg));
}

fn view_counters(app: &App) -> ViewWorkCounters {
    *app.world().resource::<ViewWorkCounters>()
}

fn mvu_counters(app: &App) -> MvuWorkCounters {
    *app.world().resource::<MvuWorkCounters>()
}

fn sync_iter(app: &App) -> usize {
    app.world().resource::<SyncStylesIterCount>().0
}

/// Settle to a truly quiescent state (the seed reconcile + text-reshape echoes
/// have quiesced), then confirm the app is idle before the assertions run.
fn settled_counter() -> App {
    let mut app = counter_app();
    for _ in 0..8 {
        app.update();
    }
    app
}

// ---------------------------------------------------------------------------
// (1) Idle frame ⇒ reconciles == 0.
// ---------------------------------------------------------------------------

#[test]
fn w4_1_idle_frame_no_reconcile() {
    let mut app = settled_counter();
    app.update(); // one idle frame — no model change
    assert_eq!(
        view_counters(&app).reconciles,
        0,
        "GATE #1 RED: an idle frame reconciled — the reconciler is not `Changed<M>`-gated"
    );
}

// ---------------------------------------------------------------------------
// (2) Idempotent fold ⇒ no model mutation, no bind, no reconcile cascade.
// ---------------------------------------------------------------------------

#[test]
fn w4_2_idempotent_fold_no_cascade() {
    let mut app = settled_counter();

    // `Reset` at count 0 is a no-op fold: `set_if_neq` leaves the model untripped.
    enqueue(&mut app, Msg::Reset);
    app.update(); // frame N: the drain folds the no-op late

    let mvu = mvu_counters(&app);
    assert_eq!(
        mvu.models_mutated, 0,
        "GATE #2 RED: an idempotent fold mutated the model (set_if_neq defeated)"
    );
    assert_eq!(
        mvu.binds_fired, 0,
        "GATE #2 RED: an idempotent fold fired a bind (the cascade propagated)"
    );
    assert_eq!(
        view_counters(&app).reconciles,
        0,
        "GATE #2 RED: an idempotent fold's frame reconciled"
    );

    // The no-op must not cascade into the FOLLOWING frame's reconcile either.
    app.update();
    assert_eq!(
        view_counters(&app).reconciles,
        0,
        "GATE #2 RED: the idempotent fold cascaded to a reconcile one frame later"
    );
}

// ---------------------------------------------------------------------------
// (3) Localized value change (Inc) ⇒ one reconcile, no spawn/despawn,
//     nodes_patched == 1 (EXACTLY the one Count label).
// ---------------------------------------------------------------------------

#[test]
fn w4_3_localized_change_patches_exactly_one_node() {
    let mut app = settled_counter();

    // Move OFF zero first so the measured Inc does NOT cross the `Reset`
    // enable/disable threshold (that would also patch the Reset button). At
    // count 1 the Reset handler is already attached; 1→2 changes only the label.
    enqueue(&mut app, Msg::Inc);
    for _ in 0..6 {
        app.update(); // fold + reconcile + quiesce at count 1
    }
    assert_eq!(
        view_counters(&app).reconciles,
        0,
        "quiesced before the measured Inc"
    );

    // The measured localized change: count 1 → 2.
    enqueue(&mut app, Msg::Inc);
    app.update(); // frame N: the drain folds Inc late → model Changed
    let after_fold = view_counters(&app);
    assert_eq!(
        after_fold.reconciles, 0,
        "the fold's own frame does not reconcile (reconcile reads the PRIOR frame)"
    );

    app.update(); // frame N+1: the front-of-frame reconcile patches the label
    let c = view_counters(&app);

    assert_eq!(
        c.reconciles, 1,
        "GATE #3 RED: the localized change did not reconcile once"
    );
    assert_eq!(
        c.nodes_spawned, 0,
        "GATE #3 RED: a value change SPAWNED nodes (rebuild, not patch)"
    );
    assert_eq!(
        c.nodes_despawned, 0,
        "GATE #3 RED: a value change DESPAWNED nodes (rebuild, not patch)"
    );
    assert_eq!(
        c.nodes_patched, 1,
        "GATE #3 RED: expected EXACTLY the one Count label patched, got {} (a \
         walk-and-write-everything reconciler over-patches)",
        c.nodes_patched
    );
}

// ---------------------------------------------------------------------------
// (4) Downstream bound: the localized change's layout dirty-set stays a small
//     constant — patching one label does NOT re-dirty the whole tree's layout.
// ---------------------------------------------------------------------------

#[test]
fn w4_4_downstream_layout_dirty_bounded() {
    let mut app = settled_counter();

    // Total realized layout nodes in the tree (the ceiling a whole-tree re-dirty
    // would hit). Counting a few representative kinds is enough for the bound.
    let node_count = entities_of_kind(app.world_mut(), Kind::Column).len()
        + entities_of_kind(app.world_mut(), Kind::Row).len()
        + entities_of_kind(app.world_mut(), Kind::Text).len()
        + entities_of_kind(app.world_mut(), Kind::Button).len();
    assert!(
        node_count >= 5,
        "sanity: the Counter tree has several nodes ({node_count})"
    );

    // Get off zero so the measured Inc is label-only (as in (3)).
    enqueue(&mut app, Msg::Inc);
    for _ in 0..6 {
        app.update();
    }

    // Idle-frame layout baseline (steady state re-translates zero nodes).
    let idle_sync = sync_iter(&app);

    // The localized change, sampling the layout dirty-set across the whole event
    // window (reconcile + layout + the ResolvedLayout self-heal, which lags a
    // frame). The MAX over the window catches a whole-tree re-dirty whenever it
    // would occur.
    enqueue(&mut app, Msg::Inc);
    let mut max_sync = idle_sync;
    for _ in 0..4 {
        app.update();
        max_sync = max_sync.max(sync_iter(&app));
    }

    assert_eq!(
        idle_sync, 0,
        "GATE #4 note: steady state re-translates zero layout nodes"
    );
    assert!(
        max_sync < node_count,
        "GATE #4 RED: patching one label re-dirtied the whole tree's layout \
         (SyncStylesIterCount peaked at {max_sync}, node_count {node_count}) — a \
         steady-state rebuild storm defeats set_if_neq downstream"
    );
    // The bound is the changed subtree, not the tree: at most the label + its
    // direct layout ancestors, a small constant independent of tree size.
    assert!(
        max_sync <= 3,
        "GATE #4 RED: the layout dirty-set is not a small constant (peaked at \
         {max_sync}); a localized label change should dirty only the changed subtree"
    );
}
