//! **THE CRUX**: command-sourcing record/replay of the editor (spec §6).
//!
//! Proves: editor state replays **byte-identically** by re-folding the recorded
//! resolved-command stream into a FRESH editor from the same seed + the same
//! `FontSystem`, WITHOUT the cosmic `Editor` ever being serialized.
//!
//! Four proofs:
//! - `replay_refold_is_byte_identical` — drive an editor through the REAL apply
//!   path (`apply_tracked` + the IME primitives) with a representative command
//!   sequence (insert / motion / shift-select / backspace / delete / multi-step /
//!   cut+paste / IME preedit+commit), recording the resolved `RecordedEdit`
//!   stream; re-fold that stream into a fresh editor from the same seed; assert
//!   value + caret + selection are identical. (Tests the lossless mirror + the
//!   `apply_recorded` fold; would FAIL if the `Motion` mirror dropped a variant
//!   or `Paste`'s resolved text were not captured.)
//! - `live_tap_records_resolved_stream` — the non-circular integration proof:
//!   drive an App editor with RAW synthetic `KeyboardInput`/`Ime` events
//!   (recording ON); the live record tap captures the resolved stream; replaying
//!   the captured `EditLog` into a fresh editor reproduces the live editor's
//!   final value byte-for-byte.
//! - `recorded_edit_reflect_ron_roundtrips` — a `RecordedEdit` (incl. a `Motion`
//!   mirror + an IME variant) round-trips through `Reflect` → RON → `Reflect`
//!   (the cross-process-persistence claim the log rests on).
//! - `record_mode_off_pays_zero` — the default `RecordMode::Off` records nothing
//!   on a live editing frame (production pays zero — H7).

use std::time::Duration;

use bevy::input::ButtonInput;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use bevy::window::Ime;
use serde::de::DeserializeSeed;

use buiy_core::Node;
use buiy_core::mvu::{LogicalId, RecordSession};
use buiy_core::text::edit::{
    ClipboardProvider, EditCommand, EditLog, MemClipboard, MotionMirror, RecordedEdit,
    RecordedPreeditCursor, TextEditState,
};
use buiy_core::text::{SharedFontSystem, edit::Clipboard};

// ---------------------------------------------------------------------------
// A logical snapshot of editor state — the byte-identity comparison surface.
// (NOT the cosmic Editor: just the value + caret + selection projections that
// the editor exposes through the facade.)
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct Snap {
    value: String,
    caret: String,     // Debug of the cosmic Cursor (line, index, affinity)
    selection: String, // Debug of the Buiy TextSelection
}

fn snap(state: &TextEditState) -> Snap {
    Snap {
        value: state.value(),
        caret: format!("{:?}", state.caret()),
        selection: format!("{:?}", state.mirror_selection()),
    }
}

// ===========================================================================
// (A) THE CRUX — re-folding the recorded stream is byte-identical.
// ===========================================================================

/// A single op applied to the ORIGINAL editor through the real apply path; each
/// is mirrored into the recorded stream EXACTLY as the live tap does.
enum Op {
    /// A resolved keyboard/clipboard command.
    Cmd(EditCommand),
    /// A paste: set the (shared, OS-like) clipboard, then run `EditCommand::Paste`.
    SetClipboardThenPaste(&'static str),
    ImePreedit(&'static str, Option<(usize, usize)>),
    ImeCommit(&'static str),
    ImeCancel,
}

#[test]
fn replay_refold_is_byte_identical() {
    let fonts = SharedFontSystem::new();

    // The representative recorded sequence (after a non-empty seed). Horizontal
    // motions only (width-independent — cosmic shapes them on demand via the
    // FontSystem, so a fresh widthless replay editor folds them identically).
    let ops = vec![
        Op::Cmd(EditCommand::Insert(" World".into())),
        Op::Cmd(EditCommand::Motion(cosmic_text::Motion::Left, false)),
        Op::Cmd(EditCommand::Motion(cosmic_text::Motion::Left, false)),
        // shift-extend select 3 left
        Op::Cmd(EditCommand::Motion(cosmic_text::Motion::Left, true)),
        Op::Cmd(EditCommand::Motion(cosmic_text::Motion::Left, true)),
        Op::Cmd(EditCommand::Motion(cosmic_text::Motion::Left, true)),
        Op::Cmd(EditCommand::Backspace), // delete the selection
        Op::Cmd(EditCommand::Insert("X".into())),
        Op::Cmd(EditCommand::Delete), // forward delete
        // multi-step: select all then cut to the clipboard
        Op::Cmd(EditCommand::SelectAll),
        Op::Cmd(EditCommand::Cut),
        Op::Cmd(EditCommand::Insert("again".into())),
        Op::Cmd(EditCommand::Motion(cosmic_text::Motion::Left, false)),
        // paste re-reads the clipboard the Cut wrote (resolved-text capture)
        Op::Cmd(EditCommand::Paste),
        // an explicit paste of fresh clipboard content
        Op::SetClipboardThenPaste("[pasted]"),
        // IME compose then CANCEL (exercises the remove-preedit path)
        Op::ImePreedit("ab", Some((0, 2))),
        Op::ImeCancel,
        // IME compose + commit
        Op::ImePreedit("ni", Some((0, 2))),
        Op::ImePreedit("nih", Some((0, 3))),
        Op::ImeCommit("nihao"),
    ];

    // --- Drive the ORIGINAL through the real apply path, recording the stream.
    let mut original = seeded_editor(&fonts, "Hello");
    let mut recorded: Vec<RecordedEdit> = Vec::new();
    // A single shared clipboard for the original, simulating the OS clipboard
    // (Cut writes it; the later bare Paste reads it).
    let mut shared_clip = MemClipboard::default();
    {
        let mut fs = fonts.lock();
        for op in &ops {
            match op {
                Op::Cmd(cmd) => {
                    // Record exactly as the live keyboard tap does.
                    recorded.push(RecordedEdit::for_command(cmd, || {
                        shared_clip.get_text().unwrap_or_default()
                    }));
                    apply_cmd_with_clip(&mut original, &mut fs, cmd.clone(), &mut shared_clip);
                }
                Op::SetClipboardThenPaste(text) => {
                    shared_clip.set_text((*text).to_string());
                    recorded.push(RecordedEdit::for_command(&EditCommand::Paste, || {
                        shared_clip.get_text().unwrap_or_default()
                    }));
                    apply_cmd_with_clip(
                        &mut original,
                        &mut fs,
                        EditCommand::Paste,
                        &mut shared_clip,
                    );
                }
                Op::ImePreedit(value, cursor) => {
                    recorded.push(RecordedEdit::ImePreedit {
                        value: (*value).to_string(),
                        cursor: cursor.map(|(begin, end)| RecordedPreeditCursor { begin, end }),
                    });
                    original.splice_preedit(&mut fs, value, *cursor);
                }
                Op::ImeCommit(value) => {
                    recorded.push(RecordedEdit::ImeCommit((*value).to_string()));
                    original.commit_preedit(&mut fs, value, Duration::ZERO);
                }
                Op::ImeCancel => {
                    recorded.push(RecordedEdit::ImeCancel);
                    original.remove_preedit(&mut fs);
                }
            }
        }
    }
    let original_snap = snap(&original);

    // --- Re-fold the recorded stream into a FRESH editor from the same seed.
    // NB: the cosmic Editor was never serialized — `recorded` is the whole input.
    let mut replay = seeded_editor(&fonts, "Hello");
    {
        let mut fs = fonts.lock();
        for edit in &recorded {
            replay.apply_recorded(&mut fs, edit, false, false, Duration::ZERO);
        }
    }
    let replay_snap = snap(&replay);

    assert_eq!(
        original_snap, replay_snap,
        "CRUX: re-folding the recorded command stream into a fresh editor from \
         the same seed reconstructs value + caret + selection BYTE-IDENTICALLY \
         — the cosmic Editor never serialized"
    );
    // Sanity: the sequence actually produced non-trivial content (not an empty
    // editor where any two snapshots would trivially match).
    assert!(
        !original_snap.value.is_empty(),
        "the driven sequence left real content"
    );
}

/// Seed a fresh editor with `text` (the replay "seed buffer"). Uses the same
/// `EditCommand::Insert` seam the tests use elsewhere; recording is irrelevant
/// (this is the initial condition, not part of the recorded stream).
fn seeded_editor(fonts: &SharedFontSystem, text: &str) -> TextEditState {
    let mut state = TextEditState::for_font_size(16.0);
    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::Insert(text.into()), false, false);
    state
}

/// Apply a resolved command through the real `apply_tracked` fold with a given
/// clipboard (the original's shared, OS-like clipboard).
fn apply_cmd_with_clip(
    state: &mut TextEditState,
    fs: &mut cosmic_text::FontSystem,
    cmd: EditCommand,
    clip: &mut MemClipboard,
) {
    use buiy_core::text::edit::EditContext;
    let mut ctx = EditContext {
        single_line: false,
        read_only: false,
        now: Duration::ZERO,
        clipboard: clip,
    };
    state.apply_tracked(fs, cmd, &mut ctx);
}

// ===========================================================================
// (B) The live record tap captures the resolved stream from RAW input, and
//     replaying it reproduces the live editor's state (non-circular proof).
// ===========================================================================

fn live_app(lid: u64) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(buiy_core::focus::FocusPlugin);
    // MinimalPlugins omits InputPlugin → register the input message + resources.
    app.add_message::<KeyboardInput>();
    app.add_message::<Ime>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
    let window = app.world_mut().spawn(()).id();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            buiy_core::layout::Style::default()
                .width_px(600.0)
                .height_px(60.0),
            buiy_core::text::Text(String::new()),
            TextEditState::for_font_size(16.0),
            LogicalId(lid),
        ))
        .id();
    app.world_mut().resource_mut::<buiy_core::FocusedEntity>().0 = Some(editor);
    // Deterministic virtual clock (undo coalescing / blink are time-driven).
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    (app, editor, window)
}

fn press_char(app: &mut App, window: Entity, ch: &str) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA, // physical code irrelevant for text
        logical_key: Key::Character(ch.into()),
        state: ButtonState::Pressed,
        text: Some(ch.into()),
        repeat: false,
        window,
    });
    app.update();
}

fn press_key(app: &mut App, window: Entity, key_code: KeyCode, logical_key: Key) {
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
}

fn editor_state(app: &App, e: Entity) -> &TextEditState {
    app.world().get::<TextEditState>(e).unwrap()
}

#[test]
fn live_tap_records_resolved_stream() {
    let lid = 7u64;
    let (mut app, editor, window) = live_app(lid);

    // Seed "Hi" with recording OFF (the initial condition).
    press_char(&mut app, window, "H");
    press_char(&mut app, window, "i");
    let seed = editor_state(&app, editor).value();
    assert_eq!(seed, "Hi", "seed typed via the live path");

    // Turn recording ON (the W4 shared switch) — everything from here is the recorded
    // stream. `live_app` adds `BuiyTextPlugin`, which inits the `RecordSession`.
    app.world_mut().resource_mut::<RecordSession>().start();

    // Type more, move, shift-select, backspace.
    press_char(&mut app, window, "!");
    press_key(&mut app, window, KeyCode::ArrowLeft, Key::ArrowLeft);
    // shift-extend one left
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    press_key(&mut app, window, KeyCode::ArrowLeft, Key::ArrowLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ShiftLeft);
    press_key(&mut app, window, KeyCode::Backspace, Key::Backspace);

    // Paste from the (OS-like) clipboard via the command modifier.
    app.world_mut()
        .resource_mut::<Clipboard>()
        .0
        .set_text("PASTE".to_string());
    let cmd_mod = if cfg!(target_os = "macos") {
        KeyCode::SuperLeft
    } else {
        KeyCode::ControlLeft
    };
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(cmd_mod);
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyV,
        logical_key: Key::Character("v".into()),
        state: ButtonState::Pressed,
        text: Some("v".into()),
        repeat: false,
        window,
    });
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(cmd_mod);

    // IME compose + commit (raw winit-style Ime events).
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "ni".to_string(),
        cursor: Some((0, 2)),
    });
    app.update();
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "world".to_string(),
    });
    app.update();

    // The live editor's final state.
    let live_snap = snap(editor_state(&app, editor));

    // The captured stream — the resolved RecordedEdits (NOT raw key events).
    let stream: Vec<RecordedEdit> = {
        let log = app.world().resource::<EditLog>();
        log.stream_for(LogicalId(lid))
            .map(|e| e.edit.clone())
            .collect()
    };
    assert!(
        !stream.is_empty(),
        "the live tap recorded the resolved command stream"
    );
    // It captured resolved verbs, not raw keys: the Paste carries its text and a
    // commit carries its string.
    assert!(
        stream
            .iter()
            .any(|e| matches!(e, RecordedEdit::Paste(t) if t == "PASTE")),
        "Paste recorded with its RESOLVED clipboard text (impure read hoisted): {stream:?}"
    );
    assert!(
        stream
            .iter()
            .any(|e| matches!(e, RecordedEdit::ImeCommit(t) if t == "world")),
        "IME commit recorded with its committed string: {stream:?}"
    );
    assert!(
        stream
            .iter()
            .any(|e| matches!(e, RecordedEdit::Motion(MotionMirror::Left, true))),
        "shift-extend motion recorded with extend=true: {stream:?}"
    );

    // --- Replay the captured stream into a fresh editor from the same seed,
    // using the SAME FontSystem the live app folded against (the determinism
    // boundary, literally — an Arc clone of the one engine).
    let fonts = app.world().resource::<SharedFontSystem>().clone();
    let mut replay = seeded_editor(&fonts, &seed);
    {
        let mut fs = fonts.lock();
        for edit in &stream {
            replay.apply_recorded(&mut fs, edit, false, false, Duration::ZERO);
        }
    }
    let replay_snap = snap(&replay);

    assert_eq!(
        live_snap.value, replay_snap.value,
        "CRUX (live): replaying the recorded stream reproduces the live editor's \
         VALUE byte-for-byte — raw input → resolved record → re-fold"
    );
    assert_eq!(
        live_snap, replay_snap,
        "CRUX (live): value + caret + selection all reproduced"
    );
}

// ===========================================================================
// (C) The recorded vocabulary round-trips through Reflect → RON → Reflect
//     (the cross-process-persistence claim the log rests on).
// ===========================================================================

#[test]
fn recorded_edit_reflect_ron_roundtrips() {
    // Build a registry with the W3 record types (the `BuiyTextPlugin` registers
    // exactly these; here we register them standalone to keep the test focused).
    let mut registry = bevy::reflect::TypeRegistry::new();
    registry.register::<RecordedEdit>();
    registry.register::<MotionMirror>();
    registry.register::<buiy_core::text::edit::LayoutCursorMirror>();
    registry.register::<RecordedPreeditCursor>();

    let samples = vec![
        RecordedEdit::Insert("héllo".into()),
        RecordedEdit::Motion(MotionMirror::LeftWord, true),
        RecordedEdit::Motion(MotionMirror::GotoLine(3), false),
        RecordedEdit::Paste("[clip]".into()),
        RecordedEdit::ImePreedit {
            value: "ni".into(),
            cursor: Some(RecordedPreeditCursor { begin: 0, end: 2 }),
        },
        RecordedEdit::ImeCommit("世界".into()),
        RecordedEdit::ImeCancel,
    ];

    let registration = registry
        .get(std::any::TypeId::of::<RecordedEdit>())
        .expect("RecordedEdit registered");
    for original in samples {
        // Reflect → RON (the typed serializer the record log uses, mirroring
        // `mvu::MsgLog::record`).
        let serializer = TypedReflectSerializer::new(&original, &registry);
        let ron = ron::ser::to_string(&serializer).expect("reflect-serialize RecordedEdit");
        // RON → Reflect (the matched typed deserializer).
        let mut de = ron::Deserializer::from_str(&ron).expect("ron deserializer");
        let dynamic = TypedReflectDeserializer::new(registration, &registry)
            .deserialize(&mut de)
            .expect("reflect-deserialize RecordedEdit");
        let round_tripped =
            RecordedEdit::from_reflect(dynamic.as_ref()).expect("from_reflect RecordedEdit");
        assert_eq!(
            original, round_tripped,
            "RecordedEdit round-trips through Reflect→RON→Reflect (cross-process \
             log persistence): {ron}"
        );
    }
}

// ===========================================================================
// (D) Default RecordMode::Off pays zero — a live editing frame records nothing.
// ===========================================================================

#[test]
fn record_mode_off_pays_zero() {
    let (mut app, _editor, window) = live_app(11);

    // Recording is OFF by default. Drive a real edit.
    press_char(&mut app, window, "a");
    press_char(&mut app, window, "b");
    press_key(&mut app, window, KeyCode::ArrowLeft, Key::ArrowLeft);
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "x".to_string(),
        cursor: Some((0, 1)),
    });
    app.update();

    let log = app.world().resource::<EditLog>();
    assert!(
        log.entries.is_empty(),
        "RecordMode::Off records nothing on a live editing frame (production pays \
         zero — H7); got {} entries",
        log.entries.len()
    );
}
