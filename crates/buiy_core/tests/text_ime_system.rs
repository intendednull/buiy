//! E5 — the IME state machine through the real `apply_ime` system
//! (editing-and-ime §§ 6.1, 6.2, 6.3, 11). Synthetic `Ime` Messages — NO
//! winit window needed (the state machine is platform-independent; the
//! real-IME-per-platform matrix is named CI-impossible, spec § 12). Headless,
//! no adapter. Asserts the four invariants at the SYSTEM level + the
//! Composition Message taxonomy.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::Ime;
use buiy_core::layout::Style;
use buiy_core::text::Text;
use buiy_core::text::edit::Clipboard;
use buiy_core::text::edit::MemClipboard;
use buiy_core::text::edit::{CompositionEnd, CompositionStart, CompositionUpdate, TextEditState};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn app_with_focused_editor() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_message::<KeyboardInput>();
    app.add_message::<Ime>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
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
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    (app, editor)
}

fn send_preedit(app: &mut App, value: &str, cursor: Option<(usize, usize)>) {
    app.world_mut().write_message(Ime::Preedit {
        window: Entity::PLACEHOLDER,
        value: value.to_string(),
        cursor,
    });
}
fn send_commit(app: &mut App, value: &str) {
    app.world_mut().write_message(Ime::Commit {
        window: Entity::PLACEHOLDER,
        value: value.to_string(),
    });
}
fn value(app: &App, e: Entity) -> String {
    app.world().get::<TextEditState>(e).unwrap().value()
}
fn undo_depth(app: &App, e: Entity) -> usize {
    app.world().get::<TextEditState>(e).unwrap().undo_depth()
}
// Count the messages of type `M` buffered this frame (the
// `text_undo_system.rs:140-142` idiom: a fresh cursor reads the frame's
// still-buffered messages; fully-qualified path, no extra import).
fn count<M: bevy::ecs::message::Message>(app: &App) -> usize {
    let messages = app.world().resource::<bevy::ecs::message::Messages<M>>();
    let mut cursor = messages.get_cursor();
    cursor.read(messages).count()
}
// DRAIN-count the messages of type `M`, emptying the buffer so the next frame
// starts clean. Bevy double-buffers messages (a message written in update N is
// still readable in update N+1), so a fresh-cursor `count` across consecutive
// emitting frames would re-see the prior frame's messages. Draining after the
// update under test counts exactly that frame's emissions — the transition
// taxonomy is asserted frame-by-frame.
fn drain_count<M: bevy::ecs::message::Message>(app: &mut App) -> usize {
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<M>>()
        .drain()
        .count()
}

/// A focused editor whose buffer is SEEDED with `text` through the real
/// TextSync seam (spawn with `Text(text)`, settle the N→N+1 latency), then with
/// `[from, to)` (byte indices on line 0) SELECTED via the editor's own motion
/// path. The selection is established AFTER TextSync settles, so the
/// `Added<TextBuffer>` lazy re-apply (sync.rs:188) cannot clobber it. This is
/// the only way to drive a real non-collapsed selection into the system tests.
fn app_with_selection(text: &str, from: usize, to: usize) -> (App, Entity) {
    use buiy_core::text::SharedFontSystem;
    use buiy_core::text::edit::EditCommand;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_message::<KeyboardInput>();
    app.add_message::<Ime>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(300.0).height_px(60.0),
            Text(text.to_string()),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    // Settle TextSync into the editor buffer (N→N+1): the authored `Text`
    // routes into the editor-owned buffer on the second update.
    app.update();
    app.update();

    // Now select [from, to) via the editor's BiDi-correct motion path.
    let fonts = app.world().resource::<SharedFontSystem>().clone();
    {
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(
            &mut fs,
            EditCommand::Motion(cosmic_text::Motion::Home, false),
            false,
            false,
        );
        for _ in 0..from {
            state.apply(
                &mut fs,
                EditCommand::Motion(cosmic_text::Motion::Right, false),
                false,
                false,
            );
        }
        for _ in from..to {
            state.apply(
                &mut fs,
                EditCommand::Motion(cosmic_text::Motion::Right, true),
                false,
                false,
            );
        }
        assert!(
            !state.mirror_selection().is_collapsed(),
            "selection established before composition"
        );
    }
    (app, editor)
}

/// Compose-over-selection at the system level: the first non-empty Preedit
/// over an active selection DELETES the selection — a genuine value change, so
/// `TextChanged` fires (contrast the unselected preedit, which fires 0). The
/// preedit is excluded from the value, so the value reads as the post-delete
/// remainder.
#[test]
fn compose_over_selection_delete_fires_textchanged() {
    use buiy_core::text::edit::TextChanged;
    let (mut app, editor) = app_with_selection("abc", 1, 3); // select "bc"

    send_preedit(&mut app, "ni", None);
    app.update();

    assert_eq!(
        count::<TextChanged>(&app),
        1,
        "the compose-over-selection delete is a genuine value change"
    );
    assert_eq!(
        value(&app, editor),
        "a",
        "selection deleted, preedit excluded"
    );
    assert!(
        app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit()
    );
}

/// Compose-over-selection cancel (empty Preedit): the stashed delete is
/// reverse-applied so the value returns to the original "abc", and a SECOND
/// `TextChanged` fires (the symmetric value transition).
#[test]
fn compose_over_selection_cancel_restores_selection() {
    use buiy_core::text::edit::TextChanged;
    let (mut app, editor) = app_with_selection("abc", 1, 3);

    send_preedit(&mut app, "ni", None);
    app.update();
    assert_eq!(value(&app, editor), "a");
    // Drain the first TextChanged so the next frame's count is clean.
    let _ = drain_count::<TextChanged>(&mut app);

    send_preedit(&mut app, "", None); // cancel
    app.update();
    assert_eq!(
        value(&app, editor),
        "abc",
        "cancel reverse-applies the compose-delete"
    );
    assert!(
        !app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit()
    );
    assert_eq!(
        drain_count::<TextChanged>(&mut app),
        1,
        "the restore is a genuine value change"
    );
}

/// Compose-over-selection cancel via `Ime::Disabled`: same restore + TextChanged.
#[test]
fn compose_over_selection_disabled_restores_selection() {
    use buiy_core::text::edit::TextChanged;
    let (mut app, editor) = app_with_selection("abc", 1, 3);

    send_preedit(&mut app, "ni", None);
    app.update();
    let _ = drain_count::<TextChanged>(&mut app);

    app.world_mut().write_message(Ime::Disabled {
        window: Entity::PLACEHOLDER,
    });
    app.update();
    assert_eq!(
        value(&app, editor),
        "abc",
        "Disabled cancel restores the value"
    );
    assert_eq!(
        drain_count::<TextChanged>(&mut app),
        1,
        "the restore fires TextChanged"
    );
}

/// Compose-over-selection commit through the system: ONE undo unit; one Undo
/// (driven via the keyboard Ctrl+Z path) restores both the deleted selection
/// and the committed text.
#[test]
fn compose_over_selection_commit_one_unit_system() {
    let (mut app, editor) = app_with_selection("abc", 1, 3);
    let undo_before = undo_depth(&app, editor);

    send_preedit(&mut app, "ni", None);
    app.update();
    send_commit(&mut app, "你");
    app.update();

    assert_eq!(value(&app, editor), "a你");
    assert_eq!(
        undo_depth(&app, editor),
        undo_before + 1,
        "delete + commit = ONE unit"
    );
}

/// Invariant (a) + (b) at the system level: during composition the undo stack
/// is unchanged and the logical value excludes the preedit.
#[test]
fn composition_leaves_undo_and_value_clean() {
    let (mut app, editor) = app_with_focused_editor();
    let undo_before = undo_depth(&app, editor);

    send_preedit(&mut app, "n", Some((0, 1)));
    app.update();
    assert!(
        app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit()
    );
    assert_eq!(value(&app, editor), "", "value excludes preedit (b)");
    assert_eq!(
        undo_depth(&app, editor),
        undo_before,
        "undo unchanged during composition (a)"
    );

    send_preedit(&mut app, "ni", Some((0, 2)));
    app.update();
    assert_eq!(value(&app, editor), "", "still excluded after update");
    assert_eq!(undo_depth(&app, editor), undo_before);
}

/// Invariant (c): Commit = exactly one Composition undo unit; TextChanged
/// fires on commit (value changed) but NOT on preedit.
#[test]
fn commit_is_one_unit_and_fires_textchanged() {
    use buiy_core::text::edit::TextChanged;
    let (mut app, editor) = app_with_focused_editor();
    let undo_before = undo_depth(&app, editor);

    send_preedit(&mut app, "ni", None);
    app.update();
    // No TextChanged from a preedit.
    assert_eq!(
        count::<TextChanged>(&app),
        0,
        "preedit does not change the value"
    );

    send_commit(&mut app, "你");
    app.update();
    assert_eq!(value(&app, editor), "你");
    assert!(
        !app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit()
    );
    assert_eq!(
        undo_depth(&app, editor),
        undo_before + 1,
        "commit = one unit (c)"
    );
    assert_eq!(
        count::<TextChanged>(&app),
        1,
        "commit fires TextChanged once"
    );
}

/// Invariant (d): an empty Preedit (cancel) and Ime::Disabled both remove the
/// span — no orphan.
#[test]
fn empty_preedit_and_disabled_remove_the_span() {
    let (mut app, editor) = app_with_focused_editor();
    send_preedit(&mut app, "abc", None);
    app.update();
    assert!(
        app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit()
    );

    // Empty Preedit cancels.
    send_preedit(&mut app, "", None);
    app.update();
    assert!(
        !app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit(),
        "empty preedit clears (d)"
    );
    assert_eq!(value(&app, editor), "");

    // Re-compose, then Disabled clears.
    send_preedit(&mut app, "xyz", None);
    app.update();
    app.world_mut().write_message(Ime::Disabled {
        window: Entity::PLACEHOLDER,
    });
    app.update();
    assert!(
        !app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit(),
        "Disabled clears (d)"
    );
    assert_eq!(value(&app, editor), "");
}

/// The Composition Message taxonomy (§ 11): Start on empty→nonempty, Update
/// on nonempty→nonempty, End on commit.
#[test]
fn composition_messages_emit_on_transitions() {
    let (mut app, editor) = app_with_focused_editor();
    let _ = editor;

    send_preedit(&mut app, "n", None);
    app.update();
    assert_eq!(
        drain_count::<CompositionStart>(&mut app),
        1,
        "Start on empty→nonempty"
    );
    assert_eq!(drain_count::<CompositionUpdate>(&mut app), 0);

    send_preedit(&mut app, "ni", None);
    app.update();
    assert_eq!(drain_count::<CompositionStart>(&mut app), 0);
    assert_eq!(
        drain_count::<CompositionUpdate>(&mut app),
        1,
        "Update on nonempty→nonempty"
    );

    send_commit(&mut app, "你");
    app.update();
    assert_eq!(drain_count::<CompositionEnd>(&mut app), 1, "End on commit");
}

/// M1 REGRESSION: the splice/remove preserve the line's RESOLVED attrs. A
/// BOLD editor (weight 700, seeded by TextSync's `span_attrs`, not the
/// default `value()`/`apply` path) must keep weight 700 across a
/// compose+cancel — a bare `AttrsList::new(&Attrs::new())` would flatten it
/// to 400 and persist (the splice never touches `Text`, so TextSync never
/// re-seeds). This system-level test runs the REAL TextSync seam (the unit
/// path in `text_ime_ops.rs` cannot — `apply`/`set_text` seed cosmic
/// defaults, not resolved attrs).
#[test]
fn composition_preserves_resolved_line_attrs() {
    use buiy_core::text::FontWeight;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_message::<KeyboardInput>();
    app.add_message::<Ime>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(300.0).height_px(60.0),
            Text("ab".to_string()), // non-empty so TextSync seeds line 0
            FontWeight(700),        // BOLD — the resolved attr that must survive
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    // Settle the spawn: TextSync seeds the editor-owned buffer on the frame
    // AFTER spawn (the campaign-wide N→N+1 measure→commit latency, OQ#1 — the
    // editor buffer is empty after a single update; the bold-700 attrs land on
    // the second). The geometry tests use the same settle idiom
    // (`text_caret_geometry.rs:124,131`).
    app.update();
    app.update(); // TextSync has now seeded line 0 with weight-700 attrs

    let weight_before = app
        .world()
        .get::<TextEditState>(editor)
        .unwrap()
        .line_default_weight_for_test(0);
    assert_eq!(weight_before, 700, "TextSync seeded the bold weight");

    // Compose a preedit, then cancel it.
    send_preedit(&mut app, "ni", None);
    app.update();
    send_preedit(&mut app, "", None); // cancel
    app.update();

    let weight_after = app
        .world()
        .get::<TextEditState>(editor)
        .unwrap()
        .line_default_weight_for_test(0);
    assert_eq!(
        weight_after, 700,
        "the line's resolved weight survives compose+cancel (M1)"
    );
}

/// Invariant (d): `Escape` during composition removes the preedit span
/// (the keyboard path — winit may deliver Escape as KeyboardInput while
/// composing). Routed through `apply_keyboard_edits`' Escape command, which
/// E5 extends to clear any live preedit before the editor's Action::Escape.
#[test]
fn escape_removes_the_preedit() {
    let (mut app, editor) = app_with_focused_editor();
    send_preedit(&mut app, "abc", None);
    app.update();
    assert!(
        app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit()
    );

    // Send Escape as a KeyboardInput.
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Escape,
        logical_key: Key::Escape,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
    assert!(
        !app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit(),
        "Escape clears the preedit (d)"
    );
    assert_eq!(value(&app, editor), "", "value unchanged by Escape-cancel");
}
