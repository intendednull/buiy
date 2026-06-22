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
use buiy_core::text::edit::{
    Clipboard, EditRedone, EditUndone, GroupKind, MemClipboard, TextEditState,
};
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

/// The platform command modifier (Ctrl on Linux/Windows, Cmd/Super on macOS) —
/// the one the undo/redo letter-commands key on, mirroring
/// `command_modifier_held` in `input.rs`. A test that hardcoded Ctrl would
/// resolve nothing on macOS (where the chord is Cmd-Shift-Z).
fn command_mod_key() -> KeyCode {
    if cfg!(target_os = "macos") {
        KeyCode::SuperLeft
    } else {
        KeyCode::ControlLeft
    }
}

/// Send a single Ctrl/Cmd-(+Shift)-letter chord for the next `app.update()`:
/// press the command modifier (and Shift if asked), enqueue the logical letter,
/// then release the held keys after the frame. The physical `key_code` is
/// irrelevant to letter-command resolution (that keys on `logical_key` + the
/// `ButtonInput<KeyCode>` modifier state, exactly as the OS delivers it).
fn press_letter_chord(app: &mut App, letter: char, shift: bool) {
    let cmd = command_mod_key();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(cmd);
    if shift {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
    }
    let window = Entity::PLACEHOLDER;
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA, // physical code is irrelevant to letter commands
        logical_key: Key::Character(letter.to_string().into()),
        state: ButtonState::Pressed,
        text: Some(letter.to_string().into()),
        repeat: false,
        window,
    });
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(cmd);
    if shift {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
    }
}

/// Drain every `EditRedone` queued this frame (the system's reader semantics).
fn drain_redone(app: &App) -> Vec<EditRedone> {
    let messages = app.world().resource::<Messages<EditRedone>>();
    let mut reader = messages.get_cursor();
    reader.read(messages).copied().collect()
}

/// Redo through the keyboard system via the Ctrl/Cmd-Shift-Z chord
/// (`input.rs:434` `'z' if shift => Redo`): type → undo → redo-chord restores
/// the value AND emits exactly ONE `EditRedone` carrying the typed run's group
/// (the `input.rs:586` emit). A regression in the chord mapping or the emit
/// fails this — mirrors `undo_emits_edit_undone_with_the_group_kind`.
#[test]
fn ctrl_shift_z_redoes_and_emits_exactly_one_edit_redone() {
    let (mut app, editor) = app_with_focused_editor();
    type_char(&mut app, 'x');
    app.update();

    // Undo first, so there is a redoable unit on the redo stack.
    press_letter_chord(&mut app, 'z', /* shift: */ false);
    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().value(),
        "",
        "Ctrl/Cmd-Z undid the typed char (precondition for redo)"
    );

    // Now Ctrl/Cmd-Shift-Z redoes.
    press_letter_chord(&mut app, 'z', /* shift: */ true);
    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().value(),
        "x",
        "Ctrl/Cmd-Shift-Z redid the typed char"
    );

    let got = drain_redone(&app);
    assert_eq!(got.len(), 1, "exactly one EditRedone");
    assert_eq!(got[0].0, editor);
    assert_eq!(
        got[0].1,
        GroupKind::TypingRun,
        "the redone unit was a typing run"
    );
}

/// Redo through the keyboard system via the Ctrl-Y chord on non-macOS
/// (`input.rs:436` `'y' if !macos => Redo`). On macOS Ctrl/Cmd-Y is NOT a redo,
/// so this case is cfg-gated to the platforms where the mapping is live; a
/// regression in the `'y'` arm fails this on Linux/Windows.
#[cfg(not(target_os = "macos"))]
#[test]
fn ctrl_y_redoes_on_non_macos() {
    let (mut app, editor) = app_with_focused_editor();
    type_char(&mut app, 'q');
    app.update();

    press_letter_chord(&mut app, 'z', /* shift: */ false); // undo
    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().value(),
        "",
        "Ctrl-Z undid (precondition)"
    );

    press_letter_chord(&mut app, 'y', /* shift: */ false); // Ctrl-Y redo
    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().value(),
        "q",
        "Ctrl-Y redid the typed char on non-macOS"
    );

    let got = drain_redone(&app);
    assert_eq!(got.len(), 1, "exactly one EditRedone for the Ctrl-Y redo");
    assert_eq!(got[0].1, GroupKind::TypingRun);
}
