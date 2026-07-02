//! #17 — a **capturing** `on_input_with` handler (the inline per-row edit case).
//!
//! The bare `on_input(fn(String)->Msg)` can't close over a row id; `on_input_with` takes a
//! capturing `Fn` (stored boxed), so `move |v| Msg::Edit(id, v)` works. This drives the REAL
//! editor path (keystrokes → editor → `TextChanged` → `route_text_input` → the boxed handler →
//! the model), proving the captured value + the boxed handler route end-to-end.

mod common;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;

use buiy_core::focus::FocusedEntity;
use buiy_core::mvu::{Cmd, Model};
use buiy_view::{BuiyViewAppExt, Element, Kind, column, find_kind, text_input};

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct EditApp {
    /// The controlled editor value (fed back to the input, like any controlled draft).
    draft: String,
    /// The last (row id, value) the captured handler folded — proves BOTH the captured id and
    /// the typed value arrived.
    last: Option<(u64, String)>,
}
impl Model for EditApp {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Edit(u64, String),
}

fn update(s: &mut EditApp, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Edit(id, v) => {
            s.draft = v.clone(); // controlled: feed the value back to the input
            s.last = Some((id, v));
        }
    }
    Cmd::none()
}

/// The row id the handler CAPTURES — the whole point of `on_input_with` (a bare fn can't).
const ROW_ID: u64 = 42;

fn view(s: &EditApp) -> Element<Msg> {
    let id = ROW_ID; // captured by value → pure (satisfies the #17 purity contract)
    column![text_input(s.draft.clone()).on_input_with(move |v| Msg::Edit(id, v))]
}

fn type_into_field(app: &mut App, s: &str) {
    let field = find_kind(app.world_mut(), Kind::TextInput).expect("input realized");
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(field);
    app.update();
    for ch in s.chars() {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character(ch.to_string().into()),
            state: ButtonState::Pressed,
            text: Some(ch.to_string().into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

fn model(app: &mut App) -> EditApp {
    app.world_mut()
        .query::<&EditApp>()
        .iter(app.world())
        .next()
        .expect("model exists")
        .clone()
}

#[test]
fn capturing_on_input_with_routes_the_captured_id_and_value() {
    let mut app = common::logic_app();
    app.ui(EditApp::default(), update, view);
    common::settle(&mut app);

    assert_eq!(model(&mut app).last, None, "seed: nothing typed");

    // Type "hi" through the real editor → each keystroke folds `Edit(42, <value-so-far>)`.
    type_into_field(&mut app, "hi");

    assert_eq!(
        model(&mut app).last,
        Some((ROW_ID, "hi".to_string())),
        "LOAD-BEARING (#17): the CAPTURING on_input_with routed the captured id (42) + the typed value"
    );
}
