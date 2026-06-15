//! E4 — the undo engine through the real `apply_keyboard_edits` system
//! (editing-and-ime §§ 8, 11). A focused editor, synthetic `KeyboardInput`,
//! and a `Time<Virtual>` clock advanced deterministically (the E3 blink-test
//! pattern, `text_caret_selection.rs:178`) — so the time-window coalescing is
//! reproducible, never wall-clock. Headless: no adapter, the FAKE clipboard.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use buiy_core::layout::Style;
use buiy_core::text::Text;
use buiy_core::text::edit::{Clipboard, EditUndone, GroupKind, MemClipboard, TextEditState};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;
use std::time::Duration;

/// Build a minimal app (BuiyTextPlugin keymap + system + Clipboard resource,
/// FocusPlugin, the KeyboardInput / ButtonInput infra) with a focused editable
/// entity. The clipboard is overridden to the fake.
fn app_with_focused_editor() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_message::<KeyboardInput>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    // Override the OS clipboard with the in-memory fake (no display needed).
    app.insert_resource(Clipboard(Box::new(MemClipboard::default())));

    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(300.0).height_px(60.0),
            Text(String::new()),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    // Pause the virtual clock so the ONLY time progression is our explicit
    // `advance_by` — `app.update()` no longer adds a real per-frame delta, so
    // the coalescing-window timing is fully deterministic, not "wide enough"
    // (n1). `advance_by` still advances a paused clock.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update(); // settle layout
    (app, editor)
}

/// Send a character keypress for the next `app.update()`.
fn type_char(app: &mut App, c: char) {
    let window = Entity::PLACEHOLDER;
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA, // physical code is irrelevant to char insertion
        logical_key: Key::Character(c.to_string().into()),
        state: ButtonState::Pressed,
        text: Some(c.to_string().into()),
        repeat: false,
        window,
    });
}

fn advance(app: &mut App, ms: u64) {
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(ms));
}

fn undo_depth(app: &App, e: Entity) -> usize {
    app.world().get::<TextEditState>(e).unwrap().undo_depth()
}

#[test]
fn typing_within_the_window_coalesces_into_one_undo_unit() {
    let (mut app, editor) = app_with_focused_editor();

    type_char(&mut app, 'a');
    app.update();
    advance(&mut app, 100); // well within the 1s window
    type_char(&mut app, 'b');
    app.update();
    advance(&mut app, 100);
    type_char(&mut app, 'c');
    app.update();

    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().value(),
        "abc"
    );
    assert_eq!(undo_depth(&app, editor), 1, "in-window typing is ONE unit");
}

#[test]
fn typing_across_the_window_splits_into_separate_units() {
    let (mut app, editor) = app_with_focused_editor();

    type_char(&mut app, 'a');
    app.update();
    advance(&mut app, 2000); // past the 1s window — seals the run
    type_char(&mut app, 'b');
    app.update();

    assert_eq!(undo_depth(&app, editor), 2, "a long pause splits the run");
}

#[test]
fn undo_emits_edit_undone_with_the_group_kind() {
    let (mut app, editor) = app_with_focused_editor();
    type_char(&mut app, 'x');
    app.update();

    // Send Ctrl/Cmd-Z. On Linux/Windows that's Ctrl-Z; press the modifier.
    #[cfg(not(target_os = "macos"))]
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    #[cfg(target_os = "macos")]
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::SuperLeft);

    let window = Entity::PLACEHOLDER;
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyZ,
        logical_key: Key::Character("z".into()),
        state: ButtonState::Pressed,
        text: Some("z".into()),
        repeat: false,
        window,
    });
    app.update();

    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().value(),
        "",
        "Ctrl/Cmd-Z undid the typed char"
    );
    // The EditUndone Message fired this frame with the TypingRun group.
    let messages = app.world().resource::<Messages<EditUndone>>();
    let mut reader = messages.get_cursor();
    let got: Vec<_> = reader.read(messages).copied().collect();
    assert_eq!(got.len(), 1, "exactly one EditUndone");
    assert_eq!(got[0].0, editor);
    assert_eq!(
        got[0].1,
        GroupKind::TypingRun,
        "the undone unit was a typing run"
    );
}
