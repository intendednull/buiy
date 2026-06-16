//! E3 headless — caret/selection geometry, the per-entity blink phase, the
//! split caret, and the SelectionChanged/CaretMoved Messages (editing-and-ime
//! §§ 4.1, 4.3, 5, 11). No GPU: the geometry is pure CPU math over a shaped
//! buffer; pixels are the additive `_gpu` golden.

use std::time::Duration;

use bevy::ecs::message::MessageCursor;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::prelude::*;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::edit::{CaretBlink, CaretMoved, SelectionChanged, TextEditState};
use buiy_core::text::{BuiyTextPlugin, CaretVisual, SelectionVisual, Text};
use buiy_core::theme::UserPreferences;
use buiy_core::{CorePlugin, FocusedEntity, Node};
use cosmic_text::Metrics;

#[test]
fn caret_blink_origin_defaults_to_zero_and_resets() {
    let mut blink = CaretBlink::default();
    assert_eq!(blink.origin, Duration::ZERO, "fresh caret blinks from t=0");
    // Reset stamps the current app-clock instant as the new phase origin.
    blink.reset(Duration::from_millis(1234));
    assert_eq!(blink.origin, Duration::from_millis(1234));
    // The phase is measured RELATIVE to the origin.
    assert_eq!(
        blink.phase_elapsed(Duration::from_millis(1734)),
        Duration::from_millis(500)
    );
    // A `now` before the origin (clock paused/rewound in tests) saturates to 0
    // rather than underflowing.
    assert_eq!(
        blink.phase_elapsed(Duration::from_millis(1000)),
        Duration::ZERO
    );
}

#[test]
fn text_edit_state_mirrors_editor_selection_into_text_selection() {
    let fonts = buiy_core::text::SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    // Type "hello", then select-all so the editor holds a Normal selection.
    state.apply(
        &mut fs,
        buiy_core::text::edit::EditCommand::Insert("hello".into()),
        false,
        false,
    );
    state.apply(
        &mut fs,
        buiy_core::text::edit::EditCommand::SelectAll,
        false,
        false,
    );
    drop(fs);

    let sel = state.mirror_selection();
    assert!(!sel.is_collapsed(), "select-all is a non-empty selection");
    let (lo, hi) = sel.primary.ordered();
    assert_eq!((lo.line, lo.index), (0, 0));
    assert_eq!((hi.line, hi.index), (0, 5));

    // With no selection (collapse), the mirror is a caret at the cursor.
    let mut fs = fonts.lock();
    state.apply(
        &mut fs,
        buiy_core::text::edit::EditCommand::Motion(cosmic_text::Motion::Home, false),
        false,
        false,
    );
    drop(fs);
    let sel = state.mirror_selection();
    assert!(sel.is_collapsed(), "collapsed selection is a caret");
    assert_eq!((sel.primary.active.line, sel.primary.active.index), (0, 0));
}

/// A headless app that runs the full text pipeline (TextSync → measure →
/// TextCommit) plus E3's render-prep window, with focus + input wired so the
/// editor can be driven by synthetic KeyboardInput. Returns (app, window).
fn caret_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(buiy_core::focus::FocusPlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default());
    app.add_message::<KeyboardInput>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    let window = app.world_mut().spawn(()).id();
    (app, window)
}

fn spawn_editor(app: &mut App, text: &str) -> Entity {
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(text)),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    // A sized parent so layout produces a real ComputedTextLayout.
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_row().width_px(400.0).height_px(40.0),
        ))
        .add_child(editor);
    editor
}

fn type_char(app: &mut App, window: Entity, ch: &str) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character(ch.into()),
        state: ButtonState::Pressed,
        text: Some(ch.into()),
        repeat: false,
        window,
    });
    app.update();
}

#[test]
fn focused_editor_gets_a_caret_visual_with_a_real_rect() {
    let (mut app, window) = caret_app();
    let editor = spawn_editor(&mut app, "");
    app.update(); // settle spawn + first layout
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    type_char(&mut app, window, "h");
    type_char(&mut app, window, "i");
    app.update(); // OQ#1: the edit reshaped at N; geometry comes current here

    let caret = app
        .world()
        .get::<CaretVisual>(editor)
        .expect("a focused editor has a caret");
    // Non-degenerate rect: the caret bar has the line-box height and sits to
    // the RIGHT of x=0 (after "hi"), with a positive width.
    assert!(
        caret.rect.height() > 1.0,
        "caret spans the line box: {:?}",
        caret.rect
    );
    assert!(
        caret.rect.min.x > 0.0,
        "caret after 'hi' is right of origin: {:?}",
        caret.rect
    );
    assert!(
        caret.rect.width() >= 1.0,
        "caret has a >=1px bar width: {:?}",
        caret.rect
    );
}

#[test]
fn selection_writes_selection_visual_and_collapse_removes_it() {
    let (mut app, window) = caret_app();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    type_char(&mut app, window, "h");
    type_char(&mut app, window, "i");

    // Select-all (Ctrl/Cmd-A) — platform-correct modifier.
    let cmd = if cfg!(target_os = "macos") {
        KeyCode::SuperLeft
    } else {
        KeyCode::ControlLeft
    };
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(cmd);
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character("a".into()),
        state: ButtonState::Pressed,
        text: Some("a".into()),
        repeat: false,
        window,
    });
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(cmd);
    app.update();

    let sel = app
        .world()
        .get::<SelectionVisual>(editor)
        .expect("a non-empty selection paints");
    assert!(!sel.is_collapsed());
    assert_eq!((sel.start.line, sel.start.index), (0, 0));
    assert_eq!((sel.end.line, sel.end.index), (0, 2)); // "hi" = 2 bytes

    // Collapse via Home (a non-extend motion) — SelectionVisual is REMOVED.
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Home,
        logical_key: Key::Home,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    app.update();
    assert!(
        app.world().get::<SelectionVisual>(editor).is_none(),
        "collapsing the selection removes the paint seat"
    );
}

#[test]
fn caret_move_and_selection_change_emit_messages_on_transition_only() {
    let (mut app, window) = caret_app();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    // PERSISTENT cursors: Bevy messages live for TWO frames (double-buffered),
    // so a fresh `get_cursor()` each frame re-reads the prior frame's still-
    // buffered message. We keep one cursor per Message and advance it, so each
    // drain reports only what was emitted SINCE the last drain — the
    // "on transition only" contract under test.
    let mut caret_cursor = app
        .world()
        .resource::<Messages<CaretMoved>>()
        .get_cursor_current();
    let mut sel_cursor = app
        .world()
        .resource::<Messages<SelectionChanged>>()
        .get_cursor_current();

    let drain_caret = |app: &mut App, cursor: &mut MessageCursor<CaretMoved>| -> Vec<Entity> {
        let msgs = app.world().resource::<Messages<CaretMoved>>();
        cursor.read(msgs).map(|m| m.0).collect()
    };
    let drain_sel = |app: &mut App, cursor: &mut MessageCursor<SelectionChanged>| -> usize {
        let msgs = app.world().resource::<Messages<SelectionChanged>>();
        cursor.read(msgs).count()
    };

    type_char(&mut app, window, "a");
    type_char(&mut app, window, "b");
    // Typing moves the caret. An Input-path edit lazily UN-shapes the editor
    // buffer that frame (M1 / OQ#1): `write_caret_and_selection` cannot read a
    // caret rect from the unshaped buffer, so the caret geometry — and with it
    // the CaretMoved transition — comes current at N+1, the same frame
    // measure/commit reshapes and the edited glyph publishes (it does NOT fire
    // on a degenerate same-frame fallback — that would trip the extract § 3.2
    // tripwire). One trailing settle frame lands that N+1 geometry; the move is
    // then observable. (Sibling `focused_editor_gets_a_caret_visual_with_a_real_
    // rect` settles the same way before reading the rect.)
    app.update();
    assert!(
        !drain_caret(&mut app, &mut caret_cursor).is_empty(),
        "typing moves the caret (CaretMoved comes current at N+1, when the \
         reshaped caret geometry publishes)"
    );

    // A pure left motion: CaretMoved, no SelectionChanged (selection stays empty).
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ArrowLeft,
        logical_key: Key::ArrowLeft,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    assert!(
        !drain_caret(&mut app, &mut caret_cursor).is_empty(),
        "ArrowLeft moves the caret"
    );
    assert_eq!(
        drain_sel(&mut app, &mut sel_cursor),
        0,
        "no selection change on a bare motion"
    );

    // An IDLE frame (no input): neither Message fires (transition-only).
    app.update();
    assert!(
        drain_caret(&mut app, &mut caret_cursor).is_empty(),
        "idle frame: no CaretMoved"
    );
    assert_eq!(
        drain_sel(&mut app, &mut sel_cursor),
        0,
        "idle frame: no SelectionChanged"
    );
}

#[test]
fn blink_is_phase_relative_to_the_per_entity_caret_origin() {
    let (mut app, window) = caret_app();
    // Pause the virtual clock so we control elapsed precisely.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    // Advance well past one half-period, THEN type: the edit resets the origin,
    // so the caret must be VISIBLE immediately after (phase 0 from the reset),
    // not hidden by the absolute clock.
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(700));
    app.update();
    type_char(&mut app, window, "x"); // resets blink origin to now (~700ms)
    app.update();
    assert!(
        app.world().get::<CaretVisual>(editor).unwrap().visible,
        "caret is solid-on immediately after an edit (phase reset)"
    );

    // Advance one half-period (500ms) past the reset ⇒ hidden phase.
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(500));
    app.update();
    assert!(
        !app.world().get::<CaretVisual>(editor).unwrap().visible,
        "caret hides one half-period after the reset"
    );
}

#[test]
fn reduced_motion_keeps_the_caret_steady() {
    let (mut app, window) = caret_app();
    // `UserPreferences` is `#[non_exhaustive]` (theme.rs) — a struct literal is
    // forbidden from this external test crate. Build the default and mutate the
    // field (the T7 `text_caret_selection.rs:222-223` precedent). `caret_app`
    // adds no ThemePlugin, so insert the resource ourselves.
    let mut prefs = UserPreferences::default();
    prefs.prefers_reduced_motion = true;
    app.insert_resource(prefs);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    type_char(&mut app, window, "x");
    // Advance many half-periods: a steady caret never hides.
    for _ in 0..5 {
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(Duration::from_millis(500));
        app.update();
        assert!(
            app.world().get::<CaretVisual>(editor).unwrap().visible,
            "reduced motion ⇒ steady caret"
        );
    }
}
