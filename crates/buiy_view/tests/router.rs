//! Headless **router / DX-3** verification (no GPU).
//!
//! A synthesized `OnPress` on a button routes through the library-generic
//! `route_presses` into the MVU drain and updates the model — with NO
//! app-authored routing system in sight (the app only calls `ui(..)`). An
//! `OnPress` on a non-widget entity folds nothing.

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Model};
use buiy_view::{BuiyViewAppExt, Element, button, column, find_press_target, row, text};

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
}

fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Inc => s.count += 1,
        Msg::Dec => s.count -= 1,
    }
    Cmd::none()
}

fn view(_s: &Counter) -> Element<Msg> {
    column![
        text("counter"),
        row![
            button("-").on_press(Msg::Dec),
            button("+").on_press(Msg::Inc),
        ],
    ]
}

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

#[test]
fn synthesized_onpress_routes_through_enqueue_to_the_model() {
    let mut app = counter_app();
    common::settle(&mut app);
    assert_eq!(value(&mut app), 0);

    // DX-3: a real OnPress on `+` routes through the library router → MVU drain.
    // The app installed NO routing system — `route_presses` is library-generic.
    let inc = find_press_target::<Counter>(app.world_mut(), &Msg::Inc).expect("+ handler");
    common::press(&mut app, inc);
    common::press(&mut app, inc);
    assert_eq!(value(&mut app), 2, "two + presses folded to 2 via enqueue");

    let dec = find_press_target::<Counter>(app.world_mut(), &Msg::Dec).expect("- handler");
    common::press(&mut app, dec);
    assert_eq!(value(&mut app), 1, "a - press folded back to 1");
}

#[test]
fn onpress_on_a_non_widget_is_inert() {
    let mut app = counter_app();
    common::settle(&mut app);
    let bogus = app.world_mut().spawn_empty().id();
    common::press(&mut app, bogus);
    assert_eq!(
        value(&mut app),
        0,
        "a press on a non-widget entity folds nothing"
    );
}
