//! Headless **reconcile / DX-2** verification (no GPU).
//!
//! The app writes only `view(&Model)`; the library materializes + patches the
//! entity tree. We assert the reconciler builds the right nodes, **patches the
//! label IN PLACE** (identical entity ids across a 0→3→Reset fold), and
//! attaches/detaches the `Reset` handler as the model crosses 0 — with NO
//! app-authored `Changed<Model>` bind system.

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Model};
use buiy_core::text::Text;
use buiy_view::{
    BuiyViewAppExt, Element, Space, button, column, find_press_target, has_press_handler, row, text,
};
use buiy_widgets::Button;

// --- The app author's WHOLE surface: Model + Msg + update + view -----------

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
    Dec,
    Reset,
}

fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Inc => s.count += 1,
        Msg::Dec => s.count -= 1,
        Msg::Reset => s.count = 0,
    }
    Cmd::none()
}

fn view(s: &Counter) -> Element<Msg> {
    column![
        text!("Count: {}", s.count).size(48.0),
        row![
            button("-").on_press(Msg::Dec),
            button("+").on_press(Msg::Inc),
            button("Reset").on_press_maybe((s.count != 0).then_some(Msg::Reset)),
        ]
        .gap(Space::Sm),
    ]
    .gap(Space::Md)
    .padding(Space::Xl)
    .align_center()
}

// --- Harness ---------------------------------------------------------------

fn counter_app() -> App {
    let mut app = common::logic_app();
    app.ui(Counter::default(), update, view);
    app
}

fn value(app: &mut App) -> i32 {
    app.world_mut()
        .query::<&Counter>()
        .iter(app.world())
        .next()
        .expect("counter exists")
        .count
}

/// The reconciled label node (the `Text` whose content starts with "Count:").
fn label_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Text)>();
    q.iter(world)
        .find(|(_, t)| t.0.starts_with("Count:"))
        .map(|(e, _)| e)
        .expect("the label node exists")
}

fn label_text(app: &mut App) -> String {
    let e = label_entity(app);
    app.world_mut()
        .get::<Text>(e)
        .expect("label text")
        .0
        .clone()
}

/// All button root entities, sorted (stable-identity check).
fn button_ids(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<Button>>();
    let mut v: Vec<Entity> = q.iter(world).collect();
    v.sort();
    v
}

#[test]
fn reconciler_builds_the_seed_tree() {
    let mut app = counter_app();
    common::settle(&mut app);

    assert_eq!(value(&mut app), 0);
    assert_eq!(label_text(&mut app), "Count: 0", "view projected the seed");
    assert_eq!(button_ids(&mut app).len(), 3, "- + Reset");

    // Reset is disabled at 0 (on_press_maybe(None)) ⇒ no handler; -/+ enabled.
    assert!(
        find_press_target::<Counter>(app.world_mut(), &Msg::Dec).is_some(),
        "- routes Dec"
    );
    assert!(
        find_press_target::<Counter>(app.world_mut(), &Msg::Inc).is_some(),
        "+ routes Inc"
    );
    assert!(
        find_press_target::<Counter>(app.world_mut(), &Msg::Reset).is_none(),
        "Reset is disabled at 0 — no handler attached"
    );
}

#[test]
fn press_patches_in_place_and_toggles_the_reset_handler() {
    let mut app = counter_app();
    common::settle(&mut app);

    // Snapshot identities BEFORE any fold — to prove patch-in-place (reuse).
    let label_before = label_entity(&mut app);
    let buttons_before = button_ids(&mut app);

    // Three `+` presses through the real router → MVU drain → reconciler patch.
    let inc = find_press_target::<Counter>(app.world_mut(), &Msg::Inc).expect("+ handler");
    common::press(&mut app, inc);
    common::press(&mut app, inc);
    common::press(&mut app, inc);
    assert_eq!(value(&mut app), 3, "three + presses folded to 3");
    assert_eq!(
        label_text(&mut app),
        "Count: 3",
        "reconciler re-patched label"
    );

    // Patch-in-place: the label + button entities are the SAME (no rebuild).
    assert_eq!(
        label_entity(&mut app),
        label_before,
        "label patched in place — same entity id after the fold"
    );
    assert_eq!(
        button_ids(&mut app),
        buttons_before,
        "buttons reused — no despawn/respawn churn on a value change"
    );

    // Reset is now enabled (count != 0) — the reconciler ATTACHED its handler.
    let reset =
        find_press_target::<Counter>(app.world_mut(), &Msg::Reset).expect("Reset enabled at 3");
    assert!(has_press_handler::<Counter>(app.world_mut(), reset));

    // Press Reset → 0; the reconciler DETACHES the handler again.
    common::press(&mut app, reset);
    assert_eq!(value(&mut app), 0, "Reset folded to 0");
    assert_eq!(label_text(&mut app), "Count: 0");
    assert!(
        find_press_target::<Counter>(app.world_mut(), &Msg::Reset).is_none(),
        "Reset disabled again at 0 — handler detached"
    );
    // Identities STILL stable across the structural handler changes.
    assert_eq!(
        button_ids(&mut app),
        buttons_before,
        "buttons never respawned"
    );
}
