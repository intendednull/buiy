//! The Counter, authored in the `buiy_view` surface.
//!
//! This is the WHOLE app-author surface: a `Model`, an `enum Msg`, a pure
//! `update`, and a `view(&Model) -> Element<Msg>`. There is NO hand-written
//! routing system and NO hand-written `Changed<Model>` bind — the library's
//! router + reconciler do both. Compare to the `hello_button` MVU demo, whose
//! `route_counter_press` (DX-3) and `bind_counter_label` (DX-2) are exactly the
//! two systems this surface deletes.
//!
//! Shared by the windowed `counter_view` bin and the headless
//! `capture_counter_view` bin, so both drive the same authored code.

use bevy::prelude::*;
use buiy::view::{BuiyViewAppExt, Element, Space, button, column, row, text};
use buiy_core::mvu::{Cmd, Model};

/// MODEL — the single source of truth.
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Counter {
    pub count: i32,
}

impl Model for Counter {
    type Msg = Msg;
}

/// The app's messages.
#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum Msg {
    Inc,
    Dec,
    Reset,
}

/// UPDATE — the pure reducer.
pub fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Inc => s.count += 1,
        Msg::Dec => s.count -= 1,
        Msg::Reset => s.count = 0,
    }
    Cmd::none()
}

/// VIEW — a declarative description. `on_press_maybe` disables Reset at 0.
pub fn view(s: &Counter) -> Element<Msg> {
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

/// Install the Counter onto an app already carrying the Buiy plugins.
pub fn install(app: &mut App) -> &mut App {
    app.ui(Counter::default(), update, view)
}
