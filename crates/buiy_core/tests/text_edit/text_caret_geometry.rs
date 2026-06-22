//! E3 headless — caret/selection geometry, the per-entity blink phase, the
//! split caret (PRIMARY + SECONDARY indicator), and the SelectionChanged/
//! CaretMoved Messages (editing-and-ime §§ 4.1, 4.3, 5, 11). No GPU: the
//! geometry is pure CPU math over a shaped buffer; pixels are the additive
//! `_gpu` golden.

use std::time::Duration;

use bevy::ecs::message::MessageCursor;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::prelude::*;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::edit::{
    CaretBlink, CaretMoved, SelectionChanged, TextEditState, caret_rect_for,
    secondary_caret_rect_for,
};
use buiy_core::text::{
    BuiyTextPlugin, CaretVisual, FamilyEntry, FontFamily, FontStack, SelectionVisual, Text,
    TextBuffer,
};
use buiy_core::theme::UserPreferences;
use buiy_core::{CorePlugin, FocusedEntity, Node};
use cosmic_text::{Cursor, Metrics};

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

// --- secondary split-caret geometry (editing-and-ime §§ 4.1, 5) -------------
//
// These pin the PURE CPU math directly: spawn a `Text` entity, settle so the
// display `TextBuffer.buffer` is SHAPED (the `text_decoration.rs` idiom), then
// call `caret_rect_for` / `secondary_caret_rect_for` on that buffer. No editor
// or focus needed — the geometry is a function of the shaped glyph array.

/// A headless app that runs the full text pipeline so a spawned `Text` entity
/// gets a SHAPED `TextBuffer`. ThemePlugin supplies the default font-matching
/// resources the resolver needs (the `text_decoration.rs` `text_app` idiom).
fn geometry_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default());
    app
}

fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn pure_ltr_editor_caret_has_no_secondary() {
    // End-to-end through the focused-editor writer: typing ASCII yields a caret
    // with NO secondary (no direction boundary) — the follow-up's explicit
    // negative, proving the writer threads `None` through `CaretVisual.secondary`.
    let (mut app, window) = caret_app();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    type_char(&mut app, window, "h");
    type_char(&mut app, window, "i");
    app.update(); // OQ#1: geometry comes current at N+1

    let caret = app
        .world()
        .get::<CaretVisual>(editor)
        .expect("a focused editor has a caret");
    assert!(
        caret.secondary.is_none(),
        "a pure-LTR editor caret carries no secondary indicator: {:?}",
        caret.secondary
    );
}

#[test]
fn secondary_caret_rect_for_is_none_without_a_direction_boundary() {
    // A pure-LTR line: no direction boundary anywhere, so every caret position
    // has a primary but NO secondary indicator.
    let mut app = geometry_app();
    app.update();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(400.0).height_px(40.0),
            Text("hello".into()),
        ))
        .id();
    settle(&mut app);

    let buffer = &app.world().get::<TextBuffer>(e).expect("TextBuffer").buffer;
    for i in [0usize, 3, 5] {
        let caret = Cursor::new(0, i);
        assert!(
            caret_rect_for(buffer, &caret).is_some(),
            "the PRIMARY caret is unaffected at index {i}"
        );
        assert_eq!(
            secondary_caret_rect_for(buffer, &caret),
            None,
            "a pure-LTR caret at index {i} has no secondary indicator"
        );
    }
}

#[test]
fn secondary_caret_rect_for_emits_at_a_mixed_bidi_boundary() {
    // A mixed LTR/RTL line (the GPU-file corpus): the LTR↔RTL joins are genuine
    // direction boundaries where the split caret's secondary indicator lands.
    let mut app = geometry_app();
    app.update();
    crate::support::register_fixture_font(
        &mut app,
        "Noto Sans Hebrew",
        "NotoSansHebrew-hebrew.ttf",
    );
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(400.0).height_px(40.0),
            Text("hello עולם world".into()),
            FontFamily(FontStack(vec![
                FamilyEntry::Named("Fira Sans".into()),
                FamilyEntry::Named("Noto Sans Hebrew".into()),
            ])),
        ))
        .id();
    settle(&mut app);

    let buffer = &app.world().get::<TextBuffer>(e).expect("TextBuffer").buffer;
    let run = buffer.layout_runs().next().expect("one shaped run");

    // DATA-DERIVE the boundary index AND the expected before-glyph edge from the
    // REAL shaped glyph array — pinned against the shaper, never an assumption.
    // A boundary byte index `i` has a BEFORE glyph (end == i) and an AFTER glyph
    // (start == i) of OPPOSITE direction.
    let (boundary, before_x, before_w, before_rtl) = run
        .glyphs
        .iter()
        .find_map(|b| {
            run.glyphs.iter().find_map(|a| {
                (a.start == b.end && a.level.is_rtl() != b.level.is_rtl()).then_some((
                    b.end,
                    b.x,
                    b.w,
                    b.level.is_rtl(),
                ))
            })
        })
        .expect("the mixed corpus has a direction boundary");

    let caret = Cursor::new(0, boundary);
    let primary = caret_rect_for(buffer, &caret).expect("primary caret at the boundary");
    let secondary = secondary_caret_rect_for(buffer, &caret).expect("secondary at the boundary");

    // The COMMITTED rule: the secondary is the BEFORE glyph's logical-end visual
    // edge — LTR → right edge (x + w), RTL → left edge (x). Equality with the
    // primary x is ALLOWED (reordered runs can share a visual pen-x while being
    // two distinct logical insertion points); we assert the EXACT data-derived
    // edge, never `primary.x != secondary.x`.
    let expected = if before_rtl {
        before_x
    } else {
        before_x + before_w
    };
    assert_eq!(
        secondary.min.x, expected,
        "secondary sits at the before-glyph logical-end edge"
    );
    // Built from the before glyph's RUN geometry: top-anchored, a SHORTER mark.
    assert_eq!(
        secondary.min.y, run.line_top,
        "secondary is top-anchored to the line box"
    );
    assert!(
        secondary.height() <= primary.height(),
        "secondary is a shorter indicator than the full-height primary \
         (sec {sec}, primary {prim})",
        sec = secondary.height(),
        prim = primary.height()
    );
    assert_eq!(
        secondary.width(),
        primary.width(),
        "secondary shares the caret bar width"
    );
}

#[test]
fn secondary_caret_rect_for_emits_on_a_wrapped_continuation_run() {
    // A logical line that SOFT-WRAPS yields MULTIPLE LayoutRuns sharing one
    // `line_i` (cosmic 0.19 emits one run per wrapped layout_line). When the
    // direction boundary lives on a CONTINUATION segment, its glyphs are in a
    // LATER run — `secondary_caret_rect_for` must scan PAST the first matching
    // run (not return None on it). A narrow width forces several wrap segments;
    // the mixed LTR/RTL corpus puts a boundary on a non-first run.
    let mut app = geometry_app();
    app.update();
    crate::support::register_fixture_font(
        &mut app,
        "Noto Sans Hebrew",
        "NotoSansHebrew-hebrew.ttf",
    );
    let e = app
        .world_mut()
        .spawn((
            Node,
            // Narrow enough that the long mixed string wraps into multiple runs.
            Style::default().width_px(90.0).height_px(120.0),
            Text("hello world עולם foo שלום bar".into()),
            FontFamily(FontStack(vec![
                FamilyEntry::Named("Fira Sans".into()),
                FamilyEntry::Named("Noto Sans Hebrew".into()),
            ])),
        ))
        .id();
    settle(&mut app);

    let buffer = &app.world().get::<TextBuffer>(e).expect("TextBuffer").buffer;

    // Confirm the precondition this test exists to cover: the logical line
    // (line 0) actually wrapped into MORE THAN ONE LayoutRun. Without this the
    // assertion below would silently degrade to the single-run case the sibling
    // test already covers.
    let run_count = buffer.layout_runs().filter(|r| r.line_i == 0).count();
    assert!(
        run_count > 1,
        "the narrow width must soft-wrap line 0 into multiple runs (got {run_count})"
    );

    // Find a direction boundary that lives on a NON-FIRST run of line 0 — the
    // exact case the first-run-only walk dropped. Data-derive it from the real
    // shaped glyph arrays, per-run (glyph byte indices are line-relative, shared
    // across the wrap segments of one logical line).
    let mut found = None;
    for (run_idx, run) in buffer
        .layout_runs()
        .filter(|r| r.line_i == 0)
        .enumerate()
        .skip(1)
    {
        for b in run.glyphs {
            if let Some((boundary, before_x, before_w, before_rtl)) =
                run.glyphs.iter().find_map(|a| {
                    (a.start == b.end && a.level.is_rtl() != b.level.is_rtl()).then_some((
                        b.end,
                        b.x,
                        b.w,
                        b.level.is_rtl(),
                    ))
                })
            {
                found = Some((
                    run_idx,
                    run.line_top,
                    boundary,
                    before_x,
                    before_w,
                    before_rtl,
                ));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (_run_idx, run_top, boundary, before_x, before_w, before_rtl) =
        found.expect("a direction boundary on a wrapped continuation run of line 0");

    let caret = Cursor::new(0, boundary);
    let secondary = secondary_caret_rect_for(buffer, &caret)
        .expect("secondary surfaces even when the boundary is on a continuation run");

    let expected = if before_rtl {
        before_x
    } else {
        before_x + before_w
    };
    assert_eq!(
        secondary.min.x, expected,
        "secondary sits at the before-glyph logical-end edge (continuation run)"
    );
    assert_eq!(
        secondary.min.y, run_top,
        "secondary uses the OWNING continuation run's line_top, not the first run's"
    );
}
