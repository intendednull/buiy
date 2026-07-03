//! `on_submit_with` (F7, spec §2.8) — the capturing submit that folds the editor's
//! **submitted text** directly into a `Msg`, deleting the `on_input → SetDraft →
//! on_submit → Submit` two-message dance.
//!
//! Drives the REAL editor→router→drain path headlessly (no GPU): focus the input, type
//! through the recorded keyboard system, press Enter (`EditSubmitted`), and assert the
//! model folded a `Submit(<typed text>)` — with NO `on_input` handler and NO draft field
//! on the model. Contrast `todomvc.rs`'s `editor_bridge_on_input_and_submit`, which needs
//! both an `on_input` sync AND a draft field to carry the same value.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::window::Ime;

use buiy_core::focus::FocusedEntity;
use buiy_core::mvu::{Cmd, Model};
use buiy_core::text::edit::{Clipboard, EditSubmitted, MemClipboard};
use buiy_view::{BuiyViewAppExt, Element, Kind, column, editor_value, find_kind, text_input};

// The app-author surface: a model with NO draft field. The submitted text is captured
// straight off the editor by `on_submit_with` — there is nothing to sync per keystroke.
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct GuessApp {
    last_guess: String,
    submits: u32,
}

impl Model for GuessApp {
    type Msg = GuessMsg;
}

#[derive(Clone, Debug, PartialEq, Reflect)]
enum GuessMsg {
    // A tuple-variant ctor IS `fn(String) -> GuessMsg` — exactly what `on_submit_with` takes.
    Submit(String),
}

fn update(s: &mut GuessApp, m: GuessMsg) -> Cmd<GuessMsg> {
    match m {
        GuessMsg::Submit(text) => {
            s.last_guess = text;
            s.submits += 1;
        }
    }
    Cmd::none()
}

fn view(_s: &GuessApp) -> Element<GuessMsg> {
    // The controlled value is empty (there is no draft field). It is drift-only, so it
    // does NOT clobber in-progress typing (the model does not change while typing — no
    // `on_input`); it re-asserts empty only after a submit changes the model, clearing
    // the input for the next guess.
    column![
        text_input(String::new())
            .placeholder("Guess the word…")
            .on_submit_with(GuessMsg::Submit),
    ]
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            buiy_core::CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            buiy_widgets::WidgetsPlugin,
        ));
    app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
    app.add_message::<Ime>();
    app.ui(GuessApp::default(), update, view);
    app
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

fn model(app: &mut App) -> GuessApp {
    app.world_mut()
        .query::<&GuessApp>()
        .iter(app.world())
        .next()
        .expect("guess model exists")
        .clone()
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

fn submit_field(app: &mut App) {
    let field = find_kind(app.world_mut(), Kind::TextInput).expect("input realized");
    app.world_mut()
        .resource_mut::<Messages<EditSubmitted>>()
        .write(EditSubmitted(field));
    for _ in 0..4 {
        app.update();
    }
}

/// The whole point: with NO `on_input` and NO draft field, `on_submit_with` captures the
/// editor's submitted text and folds `Submit(text)`.
#[test]
fn capturing_submit_folds_the_typed_text_without_a_draft() {
    let mut app = app();
    settle(&mut app);

    type_into_field(&mut app, "cat");
    // The model has NOT changed while typing — no `on_input` sync exists.
    assert_eq!(model(&mut app).submits, 0, "no message folds until submit");

    submit_field(&mut app);
    settle(&mut app);

    let m = model(&mut app);
    assert_eq!(m.submits, 1, "Enter folded exactly one Submit");
    assert_eq!(
        m.last_guess, "cat",
        "the submitted text was captured directly"
    );

    // The controlled (empty) value cleared the editor after the submit changed the model.
    let field = find_kind(app.world_mut(), Kind::TextInput).unwrap();
    assert_eq!(
        editor_value(app.world_mut(), field),
        "",
        "the input cleared for the next guess"
    );
}

/// A second guess round-trips too (the input is reusable after clearing).
#[test]
fn capturing_submit_is_reusable_across_guesses() {
    let mut app = app();
    settle(&mut app);

    type_into_field(&mut app, "dog");
    submit_field(&mut app);
    settle(&mut app);
    assert_eq!(model(&mut app).last_guess, "dog");

    type_into_field(&mut app, "bird");
    submit_field(&mut app);
    settle(&mut app);
    let m = model(&mut app);
    assert_eq!(m.last_guess, "bird");
    assert_eq!(m.submits, 2, "two independent guesses folded");
}
