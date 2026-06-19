//! E6 Task 1 — the host-facing `EditSubmitted` Message: a single-line editor
//! whose focused Enter resolves to `EditCommand::Submit` emits exactly one
//! `EditSubmitted(entity)` (editing-and-ime § 11). A multi-line editor's Enter
//! inserts a newline and emits NO `EditSubmitted`.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::text::BuiyTextPlugin;
use buiy_core::text::edit::{EditSubmitted, SingleLine, TextEditState};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(BuiyTextPlugin::default());
    app.init_resource::<ButtonInput<KeyCode>>();
    // MinimalPlugins has no InputPlugin, so `KeyboardInput` is unregistered —
    // `write_message` would silently drop and `apply_keyboard_edits` would see
    // `events: None` and no-op (the `text_input_latency` harness precedent at
    // its `add_message::<KeyboardInput>()`). Register it so the synthetic press
    // is actually read.
    app.add_message::<KeyboardInput>();
    app
}

fn press_enter(app: &mut App, window: Entity) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Enter,
        logical_key: Key::Enter,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
}

fn submitted_count(app: &mut App) -> usize {
    let messages = app.world().resource::<Messages<EditSubmitted>>();
    let mut cursor = messages.get_cursor();
    cursor.read(messages).count()
}

#[test]
fn single_line_enter_emits_one_edit_submitted() {
    let mut app = app();
    let window = app.world_mut().spawn(()).id();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            TextEditState::new(Metrics::new(16.0, 19.2)),
            SingleLine,
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    press_enter(&mut app, window);
    app.update();

    assert_eq!(
        submitted_count(&mut app),
        1,
        "single-line Enter ⇒ one EditSubmitted"
    );
}

#[test]
fn multi_line_enter_emits_no_edit_submitted() {
    let mut app = app();
    let window = app.world_mut().spawn(()).id();
    let editor = app
        .world_mut()
        .spawn((Node, TextEditState::new(Metrics::new(16.0, 19.2)))) // NOT SingleLine
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    press_enter(&mut app, window);
    app.update();

    assert_eq!(
        submitted_count(&mut app),
        0,
        "multi-line Enter inserts a newline, no submit"
    );
}
