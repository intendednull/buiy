//! Headless **conditional / `when`** verification (spec §5, no GPU).
//!
//! A `when(cond, panel)` slot alternates content↔[`Kind::Empty`] at a **stable
//! index**, so a pinned title, a toggle button, and a *later sibling* all keep
//! their exact entity ids while the conditional shows/hides — the positional
//! churn a bare absent child would cause is designed out (spec §2 #5).

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Envelope, Model};
use buiy_core::text::Text;
use buiy_view::{
    BuiyViewAppExt, Element, Kind, button, column, entities_of_kind, find_press_target, text, when,
};
use buiy_widgets::Button;

// --- App: one bool driving a `when` panel -----------------------------------

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Cond {
    show: bool,
}
impl Model for Cond {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Toggle,
}

fn update(s: &mut Cond, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Toggle => s.show = !s.show,
    }
    Cmd::none()
}

/// Title (pinned) · toggle button (pinned) · the `when` slot (alternates) ·
/// footer (a **later sibling**, the churn canary). The panel is a `Column` — a
/// distinct kind from `Empty`, so the slot is a genuine kind-swap.
fn view(s: &Cond) -> Element<Msg> {
    column![
        text("Title"),
        button("toggle").on_press(Msg::Toggle),
        when(s.show, column![text("Panel")]),
        text("Footer"),
    ]
}

fn cond_app() -> App {
    let mut app = common::logic_app();
    app.ui(Cond::default(), update, view);
    app
}

// --- Helpers ----------------------------------------------------------------

fn text_id(app: &mut App, content: &str) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Text)>();
    q.iter(world)
        .find(|(_, t)| t.0 == content)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("text node {content:?} exists"))
}

fn toggle_button_id(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<Button>>();
    let mut v: Vec<Entity> = q.iter(world).collect();
    assert_eq!(v.len(), 1, "exactly one toggle button");
    v.pop().unwrap()
}

/// How many `Empty` placeholders exist (`1` when hidden, `0` when shown).
fn empty_slots(app: &mut App) -> usize {
    entities_of_kind(app.world_mut(), Kind::Empty).len()
}

/// Is the `Panel` content realized right now?
fn panel_shown(app: &mut App) -> bool {
    let world = app.world_mut();
    let mut q = world.query::<&Text>();
    q.iter(world).any(|t| t.0 == "Panel")
}

/// Fold a `Toggle` and let the reconcile (before Layout, #10) settle the swap.
fn toggle(app: &mut App) {
    let world = app.world_mut();
    let model = world
        .query_filtered::<Entity, With<Cond>>()
        .iter(world)
        .next()
        .expect("model entity");
    app.world_mut()
        .resource_mut::<Messages<Envelope<Cond>>>()
        .write(Envelope::user(model, Msg::Toggle));
    app.update(); // frame N: drain folds — model changes
    app.update(); // frame N+1: reconcile (before Layout) swaps the slot
}

#[test]
fn when_slot_swaps_empty_and_content_keeping_siblings_stable() {
    let mut app = cond_app();
    common::settle(&mut app);

    // Seed: hidden ⇒ the slot is an `Empty` placeholder, not an absent child.
    assert!(!panel_shown(&mut app), "seed: panel hidden");
    assert_eq!(
        empty_slots(&mut app),
        1,
        "seed: the `when` slot holds one Empty"
    );

    // Pin the three siblings' identities up front.
    let title0 = text_id(&mut app, "Title");
    let button0 = toggle_button_id(&mut app);
    let footer0 = text_id(&mut app, "Footer");

    // The toggle button routes `Toggle` (proves it is live across the swaps).
    assert!(
        find_press_target::<Cond>(app.world_mut(), &Msg::Toggle).is_some(),
        "toggle button routes Toggle"
    );

    // Toggle ON/OFF twice — the slot alternates Empty↔content each time, and the
    // three siblings NEVER change identity (no positional churn).
    for round in 0..2 {
        // Show.
        toggle(&mut app);
        assert!(panel_shown(&mut app), "round {round}: panel shown");
        assert_eq!(
            empty_slots(&mut app),
            0,
            "round {round}: no Empty when shown"
        );
        assert_eq!(
            text_id(&mut app, "Title"),
            title0,
            "round {round}: Title id stable (show)"
        );
        assert_eq!(
            toggle_button_id(&mut app),
            button0,
            "round {round}: button id stable (show)"
        );
        assert_eq!(
            text_id(&mut app, "Footer"),
            footer0,
            "round {round}: Footer (later sibling) id stable across the SHOW swap"
        );

        // Hide.
        toggle(&mut app);
        assert!(!panel_shown(&mut app), "round {round}: panel hidden");
        assert_eq!(
            empty_slots(&mut app),
            1,
            "round {round}: Empty restored when hidden"
        );
        assert_eq!(
            text_id(&mut app, "Title"),
            title0,
            "round {round}: Title id stable (hide)"
        );
        assert_eq!(
            toggle_button_id(&mut app),
            button0,
            "round {round}: button id stable (hide)"
        );
        assert_eq!(
            text_id(&mut app, "Footer"),
            footer0,
            "round {round}: Footer (later sibling) id stable across the HIDE swap"
        );
    }
}
