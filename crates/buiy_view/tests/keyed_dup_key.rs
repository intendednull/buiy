//! Dup-key guard regression: `keyed_column` panics in dev/test on a duplicate key.
//!
//! A `key_fn` that returns the SAME key for two rows silently corrupts reconciliation (both rows
//! resolve to one entity + it lands in the child order twice). The debug-only guard (in the
//! reconcile-keyed PATCH path) makes that author bug LOUD in dev/test (free in release). This
//! proves the guard is NOT vacuous — it fires on a collision (the verification "prove RED" rule).
//!
//! The collision is caught on a re-reconcile (a patch), not the initial build, so the test folds
//! one state change to trigger the keyed patch.

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Envelope, Model};
use buiy_view::{BuiyViewAppExt, Element, column, keyed_column, text};

#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct DupApp {
    n: u32,
}
impl Model for DupApp {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Bump,
}

fn update(s: &mut DupApp, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Bump => s.n += 1,
    }
    Cmd::none()
}

// `key_fn` returns the constant `7` for EVERY row — a duplicate-key collision. The `n`-dependent
// label makes a `Bump` re-reconcile the tree (→ the keyed patch path → the guard).
fn view(s: &DupApp) -> Element<Msg> {
    column![
        text!("n={}", s.n),
        keyed_column([1u32, 2, 3], |_item| 7u64, |item| text!("{item}")),
    ]
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "duplicate key")]
fn keyed_column_duplicate_key_panics_in_debug() {
    let mut app = common::logic_app();
    app.ui(DupApp::default(), update, view);
    common::settle(&mut app); // initial build (spawn) — no patch yet

    // Fold a Bump → Changed<DupApp> → the reconcile PATCH walks the tree → reconcile_keyed_children
    // hits the duplicate key `7` and the guard panics.
    let model = app
        .world_mut()
        .query_filtered::<Entity, With<DupApp>>()
        .single(app.world())
        .expect("model entity");
    app.world_mut()
        .resource_mut::<Messages<Envelope<DupApp>>>()
        .write(Envelope::user(model, Msg::Bump));
    app.update(); // drain: n changes
    app.update(); // reconcile (before layout): patches → keyed guard fires
}
