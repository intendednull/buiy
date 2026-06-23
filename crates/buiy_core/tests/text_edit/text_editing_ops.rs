//! E2 — editing operations applied to the editor (editing-and-ime §§ 3,
//! 3.1, 3.3). The `apply` lowering and `value()` are tested directly against
//! a real (headless) `FontSystem` — reshaping motions / inserts need it, but
//! no adapter is involved (cosmic shaping is CPU). The system-level focus /
//! gating / Message tests follow in later steps of this task.

use buiy_core::text::SharedFontSystem;
use buiy_core::text::edit::{EditCommand, TextEditState};
use cosmic_text::{Metrics, Motion};

/// Inserting characters grows the logical value; backspace shrinks it by one
/// grapheme cluster (the `backspace_grapheme` lowering — NOT cosmic's
/// code-point-only `Action::Backspace`; see the ZWJ/combining tests below).
#[test]
fn insert_and_backspace_change_the_value() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    assert_eq!(state.value(), "");

    let mut fs = fonts.lock();
    let out = state.apply(&mut fs, EditCommand::Insert("hi".into()), false, false);
    assert!(out.value_changed);
    assert_eq!(state.value(), "hi");

    let out = state.apply(&mut fs, EditCommand::Backspace, false, false);
    assert!(out.value_changed);
    assert_eq!(state.value(), "h");
}

/// Load the committed emoji fixture font (`NotoEmoji-emoji.ttf`) into a fresh
/// `FontSystem` so an emoji-ZWJ sequence shapes against real glyphs — mirroring
/// production. The grapheme-cluster boundary logic is font-INDEPENDENT (it reads
/// the raw buffer text via `unicode-segmentation`), but registering the face
/// keeps the test exercising the same shaped-buffer path the engine always does.
fn emoji_font_system() -> SharedFontSystem {
    let fonts = SharedFontSystem::new();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fonts/NotoEmoji-emoji.ttf");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!("emoji fixture font missing ({e}); run tools/fonts/subset_fixture_fonts.sh")
    });
    fonts.lock().db_mut().load_font_data(bytes);
    fonts
}

/// Grapheme-correct Backspace over an emoji-ZWJ FAMILY sequence
/// (`👨‍👩‍👧‍👦` = man · ZWJ · woman · ZWJ · girl · ZWJ · boy — 7 scalars, 25 bytes,
/// ONE grapheme cluster). One Backspace must remove the WHOLE cluster, not the
/// trailing scalar. A naive code-point delete (cosmic-text 0.19's raw
/// `Action::Backspace`) would leave `👨‍👩‍👧‍` ending in a dangling ZWJ — this
/// test fails loudly on that regression (editing-and-ime § 3.1, normative).
#[test]
fn backspace_removes_a_whole_emoji_zwj_cluster_in_one_step() {
    let fonts = emoji_font_system();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    state.apply(&mut fs, EditCommand::Insert(family.into()), false, false);
    assert_eq!(state.value(), family, "the family emoji is in the buffer");
    assert_eq!(state.value().len(), 25, "7-scalar ZWJ sequence, 25 bytes");

    let out = state.apply(&mut fs, EditCommand::Backspace, false, false);
    assert!(out.value_changed, "the cluster was deleted");
    assert_eq!(
        state.value(),
        "",
        "ONE Backspace removed the entire grapheme cluster, not one code point"
    );
}

/// Grapheme-correct Backspace over a base+combining-mark sequence
/// (`e\u{0301}` = 'e' + COMBINING ACUTE ACCENT — 2 scalars, 3 bytes, ONE
/// grapheme cluster). One Backspace removes the whole `é`, not just the
/// combining mark (which would leave a bare `e`).
#[test]
fn backspace_removes_a_whole_combining_mark_cluster_in_one_step() {
    let fonts = SharedFontSystem::new(); // base+mark needs no special font
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    let combining = "e\u{0301}";
    state.apply(&mut fs, EditCommand::Insert(combining.into()), false, false);
    assert_eq!(
        state.value(),
        combining,
        "e + combining acute is in the buffer"
    );
    assert_eq!(
        state.value().len(),
        3,
        "'e' (1) + combining acute (2) = 3 bytes"
    );

    let out = state.apply(&mut fs, EditCommand::Backspace, false, false);
    assert!(out.value_changed, "the cluster was deleted");
    assert_eq!(
        state.value(),
        "",
        "ONE Backspace removed the whole base+mark cluster, not just the mark"
    );
}

/// The grapheme path must NOT over-delete: with TWO clusters in the buffer, one
/// Backspace removes exactly the LAST cluster and leaves the first intact.
/// Guards the boundary computation (it must find the cluster start preceding the
/// caret, not the start of the line).
#[test]
fn backspace_removes_only_the_last_cluster_when_two_are_present() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    // 'a' (single-scalar cluster) followed by a base+combining cluster.
    state.apply(
        &mut fs,
        EditCommand::Insert("ae\u{0301}".into()),
        false,
        false,
    );
    assert_eq!(state.value(), "ae\u{0301}");

    state.apply(&mut fs, EditCommand::Backspace, false, false);
    assert_eq!(
        state.value(),
        "a",
        "only the trailing é cluster went; the leading 'a' survives"
    );

    // A second Backspace removes the remaining single-scalar cluster.
    state.apply(&mut fs, EditCommand::Backspace, false, false);
    assert_eq!(state.value(), "", "the 'a' is gone too");
}

/// A non-extending motion does NOT change the value and reports
/// `value_changed = false` (so it never emits `TextChanged`).
#[test]
fn motion_does_not_change_the_value() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::Insert("abc".into()), false, false);

    let out = state.apply(
        &mut fs,
        EditCommand::Motion(Motion::Left, false),
        false,
        false,
    );
    assert!(!out.value_changed, "moving the caret is not a value change");
    assert_eq!(state.value(), "abc");
}

/// Audit #38 (T4.6): word-navigation MOTION behavior. The keymap tier
/// (`text_keymap.rs`) already pins that Ctrl/Option+arrow RESOLVES to
/// `Motion::LeftWord`/`RightWord`; this pins what those motions DO when applied —
/// the caret jumps a whole word, not a single grapheme. Starting at the buffer
/// end of `"foo bar baz"`, `LeftWord` walks back over word boundaries
/// (`baz`→`bar`→`foo`→start) and `RightWord` walks forward, so the caret lands on
/// word starts/ends, never mid-word. A regression that lowered word-nav to a
/// single-step `Left`/`Right` (or dropped the word granularity) would move by one
/// index and redden this.
#[test]
fn word_nav_motions_jump_whole_words() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    // "foo bar baz" — three words, two spaces. Byte offsets: foo[0..3] sp[3]
    // bar[4..7] sp[7] baz[8..11]. The caret starts at the end (index 11) after
    // the insert.
    state.apply(
        &mut fs,
        EditCommand::Insert("foo bar baz".into()),
        false,
        false,
    );
    assert_eq!(state.caret().index, 11, "caret starts at the buffer end");

    // A non-mutating word motion; returns the resulting caret byte index.
    fn word(state: &mut TextEditState, fs: &mut cosmic_text::FontSystem, motion: Motion) -> usize {
        let out = state.apply(fs, EditCommand::Motion(motion, false), false, false);
        assert!(!out.value_changed, "word-nav is a caret move, not an edit");
        state.caret().index
    }

    // LeftWord steps back over each word boundary, not one grapheme at a time.
    assert_eq!(
        word(&mut state, &mut fs, Motion::LeftWord),
        8,
        "back to start of 'baz'"
    );
    assert_eq!(
        word(&mut state, &mut fs, Motion::LeftWord),
        4,
        "back to start of 'bar'"
    );
    assert_eq!(
        word(&mut state, &mut fs, Motion::LeftWord),
        0,
        "back to start of 'foo'"
    );
    // Already at the start: another LeftWord cannot move past index 0.
    assert_eq!(
        word(&mut state, &mut fs, Motion::LeftWord),
        0,
        "clamped at line start"
    );

    // RightWord steps forward over each word, landing past each word's end.
    assert_eq!(
        word(&mut state, &mut fs, Motion::RightWord),
        3,
        "forward to end of 'foo'"
    );
    assert_eq!(
        word(&mut state, &mut fs, Motion::RightWord),
        7,
        "forward to end of 'bar'"
    );
    assert_eq!(
        word(&mut state, &mut fs, Motion::RightWord),
        11,
        "forward to end of 'baz'"
    );
}

/// On a `SingleLine` editor, `Enter` submits (never inserts a newline) and
/// reports `submitted`; the value is unchanged.
#[test]
fn single_line_enter_submits_without_newline() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::Insert("name".into()), false, false);

    let out = state.apply(
        &mut fs,
        EditCommand::Enter,
        /* single_line: */ true,
        false,
    );
    assert!(out.submitted, "single-line Enter submits");
    assert!(!out.value_changed, "submit does not change the value");
    assert_eq!(state.value(), "name", "no newline inserted");
}

/// On a multi-line editor, `Enter` inserts a newline (the value gains it).
#[test]
fn multi_line_enter_inserts_a_newline() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
    let out = state.apply(
        &mut fs,
        EditCommand::Enter,
        /* single_line: */ false,
        false,
    );
    assert!(out.value_changed);
    assert!(!out.submitted);
    assert_eq!(state.value(), "ab\n");
}

/// A `SingleLine` insert strips newlines from pasted/typed text (§ 3.3).
#[test]
fn single_line_insert_strips_newlines() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let out = state.apply(
        &mut fs,
        EditCommand::Insert("a\nb\nc".into()),
        /* single_line: */ true,
        false,
    );
    assert!(out.value_changed);
    assert_eq!(
        state.value(),
        "abc",
        "newlines stripped on a single-line editor"
    );
}

/// `ReadOnly` refuses mutation but allows motion: insert/backspace are
/// no-ops (value unchanged, value_changed false); a motion still moves.
#[test]
fn read_only_blocks_mutation_allows_motion() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    // Seed text WITHOUT the read-only gate, then turn the gate on.
    state.apply(&mut fs, EditCommand::Insert("locked".into()), false, false);

    let out = state.apply(
        &mut fs,
        EditCommand::Insert("X".into()),
        false,
        /* read_only: */ true,
    );
    assert!(!out.value_changed, "read-only refuses insertion");
    assert_eq!(state.value(), "locked");

    let out = state.apply(&mut fs, EditCommand::Backspace, false, true);
    assert!(!out.value_changed, "read-only refuses backspace");
    assert_eq!(state.value(), "locked");

    // Motion is allowed under read-only (caret/selection yes — § 2.2).
    let out = state.apply(
        &mut fs,
        EditCommand::Motion(Motion::Home, false),
        false,
        true,
    );
    assert!(!out.value_changed, "motion never changes the value");
}

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::edit::{ReadOnly, TextChanged};
use buiy_core::text::{BuiyTextPlugin, Text};
use buiy_core::{BuiySet, CorePlugin, FocusedEntity, Node};

/// Build a full headless editing app (Core + Focus + Layout + Text), with a
/// synthetic primary window the KeyboardInput events target.
fn editing_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        // FocusPlugin owns `FocusedEntity` (CorePlugin does NOT — M2). The
        // editing system reads it via Option<Res<FocusedEntity>>, and the
        // tests set it to focus an editor, so the resource must exist.
        .add_plugins(buiy_core::focus::FocusPlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default());
    // MinimalPlugins omits bevy's InputPlugin, which is the sole owner of
    // both `add_message::<KeyboardInput>()` and `ButtonInput<KeyCode>`. The
    // editing system reads the modifier resource and the test enqueues the
    // message, so insert both directly (the headless idiom — no winit / no
    // InputPlugin needed for the resources themselves). Without the
    // add_message, `World::write_message::<KeyboardInput>` returns `None`
    // and the event is silently dropped. (FocusPlugin's `handle_tab` also
    // reads `Res<ButtonInput<KeyCode>>`, so this insert is doubly required.)
    app.add_message::<KeyboardInput>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    // A window entity so KeyboardInput.window points somewhere valid
    // (the system reads the focused editor, not the window, but events
    // carry it).
    let window = app.world_mut().spawn(()).id();
    (app, window)
}

/// Push a logical-character key press (text-bearing) and one frame.
fn press_char(app: &mut App, window: Entity, ch: &str) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA, // physical code is irrelevant for text insert
        logical_key: Key::Character(ch.into()),
        state: ButtonState::Pressed,
        text: Some(ch.into()),
        repeat: false,
        window,
    });
    app.update();
}

/// A focused editable entity receives typed characters; an unfocused one
/// does not. TextChanged fires once per typed char.
#[test]
fn typing_routes_to_the_focused_editor_only() {
    let (mut app, window) = editing_app();
    app.add_message::<TextChanged>(); // ensure reader is valid even if plugin order varies

    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
        ))
        .id();
    let other = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.update(); // settle spawn

    // Nothing focused ⇒ typing is dropped.
    press_char(&mut app, window, "x");
    assert_eq!(
        app.world()
            .get::<buiy_core::text::edit::TextEditState>(editor)
            .unwrap()
            .value(),
        ""
    );

    // Focus the editor ⇒ typing lands there, not on `other`.
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    press_char(&mut app, window, "h");
    press_char(&mut app, window, "i");
    assert_eq!(
        app.world()
            .get::<buiy_core::text::edit::TextEditState>(editor)
            .unwrap()
            .value(),
        "hi"
    );
    assert_eq!(
        app.world()
            .get::<buiy_core::text::edit::TextEditState>(other)
            .unwrap()
            .value(),
        ""
    );
}

/// A focused `ReadOnly` editor ignores typed characters (mutation gate).
#[test]
fn read_only_focused_editor_ignores_typing() {
    let (mut app, window) = editing_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
            ReadOnly,
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    press_char(&mut app, window, "z");
    assert_eq!(
        app.world()
            .get::<buiy_core::text::edit::TextEditState>(editor)
            .unwrap()
            .value(),
        ""
    );
}

use bevy::input::keyboard::Key as LogicalKey;

/// Drive a key WITH a modifier physical key held. Presses the modifier code
/// (so `ButtonInput<KeyCode>` reports it), then the logical key, in one
/// frame.
fn press_with_command_mod(app: &mut App, window: Entity, logical: LogicalKey, text: Option<&str>) {
    // Hold the platform's command modifier for this frame — Super (Cmd) on
    // macOS, Ctrl elsewhere — to match `command_mod_active` in the system
    // (input.rs § command_mod_active). A test that hardcoded Ctrl would fail
    // on macOS, where Ctrl-A is not select-all.
    let mod_key = if cfg!(target_os = "macos") {
        KeyCode::SuperLeft
    } else {
        KeyCode::ControlLeft
    };
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(mod_key);
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: logical,
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(mod_key);
}

/// The platform select-all chord (Ctrl-A on Linux/Windows, Cmd-A on macOS) on
/// the focused editor selects the whole buffer; a subsequent non-extending
/// typed char REPLACES the selection (cosmic deletes the selection before
/// inserting). This proves the letter-command lookup AND that the selection
/// is live — on every platform's command modifier.
#[test]
fn select_all_chord_then_typing_replaces() {
    let (mut app, window) = editing_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    press_char(&mut app, window, "h");
    press_char(&mut app, window, "i");
    assert_eq!(value(&app, editor), "hi");

    // Command-A (logical 'a', text "a", platform command modifier held) ⇒
    // SelectAll.
    press_with_command_mod(
        &mut app,
        window,
        LogicalKey::Character("a".into()),
        Some("a"),
    );
    // Typing now replaces the whole selection.
    press_char(&mut app, window, "X");
    assert_eq!(
        value(&app, editor),
        "X",
        "select-all chord selected all; typing replaced it"
    );
}

/// A repeated keypress (`repeat = true`) re-applies — two delete-repeats
/// remove two graphemes.
#[test]
fn key_repeat_reapplies_the_command() {
    let (mut app, window) = editing_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    for c in ["a", "b", "c"] {
        press_char(&mut app, window, c);
    }
    assert_eq!(value(&app, editor), "abc");

    // Two Backspace events in ONE frame, the second flagged repeat — both
    // apply (the system processes every Pressed event, repeats included).
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Backspace,
        logical_key: LogicalKey::Backspace,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Backspace,
        logical_key: LogicalKey::Backspace,
        state: ButtonState::Pressed,
        text: None,
        repeat: true,
        window,
    });
    app.update();
    assert_eq!(
        value(&app, editor),
        "a",
        "two backspaces (one a repeat) removed two chars"
    );
}

/// `TextChanged` fires once per value-changing frame, and not for a pure
/// motion.
#[test]
fn text_changed_fires_on_value_change_only() {
    let (mut app, window) = editing_app();
    // A collector system that counts TextChanged per frame.
    #[derive(Resource, Default)]
    struct Seen(usize);
    app.init_resource::<Seen>();
    // Order the collector AFTER the producer (`apply_keyboard_edits` runs in
    // `BuiySet::Input`). An unordered `Update` system may be scheduled before
    // `BuiySet::Input`, in which case it reads the `TextChanged` queue a frame
    // late and the same-frame `before`/`after_type` sampling misses the write.
    app.add_systems(
        Update,
        (|mut r: MessageReader<TextChanged>, mut s: ResMut<Seen>| {
            s.0 += r.read().count();
        })
        .after(BuiySet::Input),
    );

    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    let before = app.world().resource::<Seen>().0;
    press_char(&mut app, window, "q"); // value change ⇒ one TextChanged
    let after_type = app.world().resource::<Seen>().0;
    assert_eq!(after_type, before + 1, "typing emits one TextChanged");

    // A bare ArrowLeft (motion) ⇒ no TextChanged.
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ArrowLeft,
        logical_key: LogicalKey::ArrowLeft,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    assert_eq!(
        app.world().resource::<Seen>().0,
        after_type,
        "motion emits no TextChanged"
    );
}

/// Small reader helper.
fn value(app: &App, e: Entity) -> String {
    app.world()
        .get::<buiy_core::text::edit::TextEditState>(e)
        .unwrap()
        .value()
}

use buiy_core::text::edit::SingleLine;

/// A `SingleLine` editor lays its content on ONE visual line even when the
/// content exceeds the node width — `Wrap::None` (§ 3.3). A multi-line
/// editor with the same long content wraps to >1 line.
#[test]
fn single_line_editor_buffer_does_not_wrap() {
    let mut single = editing_app().0;
    let mut multi = editing_app().0;

    let long = "wrapping word wrapping word wrapping word wrapping word";
    let make = |app: &mut App, single_line: bool| -> Entity {
        let mut e = app.world_mut().spawn((
            Node,
            Style::default(),
            Text(String::new()), // inert display carrier (editor owns its content)
            buiy_core::text::edit::TextEditState::new(cosmic_text::Metrics::new(16.0, 19.2)),
        ));
        if single_line {
            e.insert(SingleLine);
        }
        let id = e.id();
        // Seed the editor's OWNED content via the explicit verb (C2 § 2.3): the
        // display `Text`→editor seam is gone (C2 § 2.1), so the long content
        // reaches the editor through `Insert`. A SingleLine editor must still
        // lay it on ONE visual line (Wrap::None) — the property under test.
        {
            let fonts = app.world().resource::<SharedFontSystem>().clone();
            let mut fs = fonts.lock();
            let mut state = app.world_mut().get_mut::<TextEditState>(id).unwrap();
            state.apply(
                &mut fs,
                EditCommand::Insert(long.into()),
                single_line,
                false,
            );
        }
        // A narrow sized parent forces wrapping for the multi-line case.
        app.world_mut()
            .spawn((
                Node,
                Style::default()
                    .flex_column()
                    .width_px(80.0)
                    .height_px(200.0),
            ))
            .add_child(id);
        id
    };
    let s = make(&mut single, true);
    let m = make(&mut multi, false);
    single.update();
    single.update();
    multi.update();
    multi.update();

    let line_count = |app: &App, e: Entity| -> usize {
        app.world()
            .get::<buiy_core::text::edit::TextEditState>(e)
            .unwrap()
            .with_buffer(|b| b.layout_runs().count())
    };
    assert_eq!(
        line_count(&single, s),
        1,
        "single-line editor never wraps (Wrap::None)"
    );
    assert!(
        line_count(&multi, m) > 1,
        "multi-line editor wraps the long content"
    );
}
