//! Headless **child-lift / `Element::map`** verification (spec §5, no GPU).
//!
//! A reusable child `Counter` component (its own `view` + `update`) is embedded
//! TWICE — left and right — into ONE parent model via `.map(Msg::Left)` /
//! `.map(Msg::Right)`. Each child is **parent-owned sub-state** (a field of the
//! single model); the child's `view`/`update` are reused **verbatim** and its
//! messages are lifted into the parent's `Msg`. Pressing left `+`×3 / right `+`×1
//! yields `(3, 1)` in isolation, and a left `-` never touches the right — the Elm
//! `Html.map` composition default (spec §2 #6).

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Model};
use buiy_view::{BuiyViewAppExt, Element, button, find_press_target, row, text};

// --- The reusable child component: a self-contained Counter -----------------
//
// This module is written EXACTLY as a standalone component would be (own Model
// shape, own Msg, own `view`/`update`); the parent reuses it verbatim.
mod child {
    use super::*;

    #[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
    #[reflect(Component)]
    pub struct Counter {
        pub count: i32,
    }

    #[derive(Clone, Debug, PartialEq, Reflect)]
    pub enum Msg {
        Inc,
        Dec,
    }

    /// Reused verbatim by the parent (`child::update(&mut s.left, cm)`).
    pub fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
        match m {
            Msg::Inc => s.count += 1,
            Msg::Dec => s.count -= 1,
        }
        Cmd::none()
    }

    /// Reused verbatim by the parent (`child::view(&s.left).map(Msg::Left)`).
    pub fn view(s: &Counter) -> Element<Msg> {
        row![
            button("-").on_press(Msg::Dec),
            text!("{}", s.count),
            button("+").on_press(Msg::Inc),
        ]
    }
}

// --- The parent: two child Counters as owned sub-state ----------------------

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct TwoCounters {
    left: child::Counter,
    right: child::Counter,
}
impl Model for TwoCounters {
    type Msg = Msg;
}

/// The two child-carrying variants ARE the lift targets: `Msg::Left` /
/// `Msg::Right` each have type `fn(child::Msg) -> Msg`.
#[derive(Clone, Debug, PartialEq, Reflect)]
enum Msg {
    Left(child::Msg),
    Right(child::Msg),
}

/// The parent reducer delegates ONE line to the reused child reducer on the
/// owned sub-state (message-lifting's parent half).
fn update(s: &mut TwoCounters, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Left(cm) => {
            let _child_cmd = child::update(&mut s.left, cm);
        }
        Msg::Right(cm) => {
            let _child_cmd = child::update(&mut s.right, cm);
        }
    }
    Cmd::none()
}

/// The parent view lifts each reused child view into the parent's `Msg`.
fn view(s: &TwoCounters) -> Element<Msg> {
    row![
        child::view(&s.left).map(Msg::Left),
        child::view(&s.right).map(Msg::Right),
    ]
}

fn app() -> App {
    let mut app = common::logic_app();
    app.ui(TwoCounters::default(), update, view);
    app
}

fn counts(app: &mut App) -> (i32, i32) {
    let world = app.world_mut();
    let m = world
        .query::<&TwoCounters>()
        .iter(world)
        .next()
        .expect("model");
    (m.left.count, m.right.count)
}

/// Press whichever button routes `want` (drives the REAL lifted press path).
fn press_msg(app: &mut App, want: &Msg) {
    let target = find_press_target::<TwoCounters>(app.world_mut(), want)
        .unwrap_or_else(|| panic!("a button routes {want:?}"));
    common::press(app, target);
}

#[test]
fn map_lifts_two_children_isolated() {
    let mut app = app();
    common::settle(&mut app);
    assert_eq!(counts(&mut app), (0, 0), "seed");

    // The lift produced DISTINCT parent messages for the two identical children:
    // the left `+` routes `Left(Inc)`, the right `+` routes `Right(Inc)`.
    assert!(
        find_press_target::<TwoCounters>(app.world_mut(), &Msg::Left(child::Msg::Inc)).is_some(),
        "left + lifted to Left(Inc)"
    );
    assert!(
        find_press_target::<TwoCounters>(app.world_mut(), &Msg::Right(child::Msg::Inc)).is_some(),
        "right + lifted to Right(Inc)"
    );

    // Left +×3, right +×1 → (3, 1), each folded onto its OWN sub-state.
    press_msg(&mut app, &Msg::Left(child::Msg::Inc));
    press_msg(&mut app, &Msg::Left(child::Msg::Inc));
    press_msg(&mut app, &Msg::Left(child::Msg::Inc));
    press_msg(&mut app, &Msg::Right(child::Msg::Inc));
    assert_eq!(counts(&mut app), (3, 1), "left +×3 / right +×1, isolated");

    // A left `-` decrements ONLY the left — the right is never touched.
    press_msg(&mut app, &Msg::Left(child::Msg::Dec));
    assert_eq!(
        counts(&mut app),
        (2, 1),
        "left - touches only left; right unchanged (sub-state isolation)"
    );
}
