//! E3 headless — the mouse-selection LOGIC (editing-and-ime § 4, mouse
//! Click/DoubleClick/TripleClick/Drag): the window→buffer-local mapping, the
//! click-count classifier, and the gesture→Action application. The full
//! `Pointer<Press>`/`Pointer<Drag>` observer wiring (C3c) is GPU/windowed; here
//! we pin the platform-independent geometry + state machine the observers drive.

use std::time::Duration;

use bevy::math::Vec2;
use bevy::prelude::*;
use buiy_core::text::edit::{
    ClickTracker, EditCommand, PointerGesture, TextBufferAccess, TextEditState, pointer_to_cursor,
};
use buiy_core::text::{SharedFontSystem, TextBuffer};
use cosmic_text::Metrics;

/// `Buffer::hit` / `layout_runs` read laid-out geometry, which cosmic produces
/// lazily — only after a `set_size`-triggered reshape (it no-ops on an unsized
/// buffer whose dirty flag was never set). In the running engine the layout
/// commit (`text::commit`) sizes + shapes the editor buffer every frame before
/// any geometry reader runs; this helper reproduces that exact production seam
/// (`set_size` + `shape_until_scroll` through `TextBufferAccess::with_buffer_mut`)
/// so the pure-logic pointer tests see the same shaped buffer the windowed
/// `pointer_selection` system always does. No test-only production API: it uses
/// the public facade the commit uses.
fn shaped_editor(fonts: &SharedFontSystem, text: &str) -> (World, Entity) {
    let metrics = Metrics::new(16.0, 19.2);
    let mut state = TextEditState::new(metrics);
    {
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert(text.into()), false, false);
    }
    let mut world = World::new();
    let entity = world.spawn((TextBuffer::new(metrics), state)).id();
    let mut query = world.query::<TextBufferAccess>();
    let mut item = query.get_mut(&mut world, entity).unwrap();
    let mut fs = fonts.lock();
    item.with_buffer_mut(|buffer| {
        buffer.set_size(Some(400.0), None);
        buffer.shape_until_scroll(&mut fs, false);
    });
    drop(fs);
    (world, entity)
}

#[test]
fn pointer_maps_to_a_cursor_via_buffer_local_coords() {
    let fonts = SharedFontSystem::new();
    let (mut world, entity) = shaped_editor(&fonts, "hello");
    let mut query = world.query::<TextBufferAccess>();
    let item = query.get(&world, entity).unwrap();
    // A hit at far-left (x≈0) lands at index 0; a hit far-right lands at the end
    // (index 5). content_offset/origin are zero here (no node).
    let origin = Vec2::ZERO;
    let pointer = Vec2::new(0.0, 8.0); // left edge, on the single line
    let cursor = item
        .with_buffer(|b| pointer_to_cursor(b, pointer, origin))
        .unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 0));

    let pointer = Vec2::new(1000.0, 8.0); // far right ⇒ clamps to line end
    let cursor = item
        .with_buffer(|b| pointer_to_cursor(b, pointer, origin))
        .unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 5));
}

#[test]
fn click_tracker_classifies_single_double_triple_by_time_and_adjacency() {
    let mut t = ClickTracker::default();
    let near = Vec2::new(10.0, 10.0);
    // First click ⇒ single.
    assert_eq!(
        t.classify(near, Duration::from_millis(0)),
        PointerGesture::Click
    );
    // Within the double window + near ⇒ double.
    assert_eq!(
        t.classify(near, Duration::from_millis(200)),
        PointerGesture::DoubleClick
    );
    // Again within the window + near ⇒ triple.
    assert_eq!(
        t.classify(near, Duration::from_millis(380)),
        PointerGesture::TripleClick
    );
    // A 4th in-window adjacent click cycles back to single (triple is the max,
    // then the streak restarts — the standard OS click-counter cadence
    // 1,2,3,1,2,3…).
    assert_eq!(
        t.classify(near, Duration::from_millis(520)),
        PointerGesture::Click
    );
    // A far click breaks the streak and resets to single even within the time
    // window (adjacency, not just timing, gates a multi-click).
    let far = Vec2::new(200.0, 10.0);
    assert_eq!(
        t.classify(far, Duration::from_millis(600)),
        PointerGesture::Click
    );
    // A click after the window lapses ⇒ single (the streak times out).
    assert_eq!(
        t.classify(near, Duration::from_millis(2000)),
        PointerGesture::Click
    );
}

#[test]
fn gesture_applies_the_matching_cosmic_action_and_moves_the_caret() {
    let fonts = SharedFontSystem::new();
    let (mut world, entity) = shaped_editor(&fonts, "hello world");
    let origin = Vec2::ZERO;

    {
        let mut state = world.get_mut::<TextEditState>(entity).unwrap();
        let mut fs = fonts.lock();
        // A single click near the start collapses the selection there.
        state.apply_pointer_gesture(&mut fs, PointerGesture::Click, Vec2::new(0.0, 8.0), origin);
    }
    {
        let state = world.get::<TextEditState>(entity).unwrap();
        assert!(state.editor_selection_is_none());
        assert_eq!(state.caret().index, 0);
    }

    {
        let mut state = world.get_mut::<TextEditState>(entity).unwrap();
        let mut fs = fonts.lock();
        // A double click selects the word under the pointer (cosmic DoubleClick).
        state.apply_pointer_gesture(
            &mut fs,
            PointerGesture::DoubleClick,
            Vec2::new(2.0, 8.0),
            origin,
        );
    }
    let state = world.get::<TextEditState>(entity).unwrap();
    let (lo, hi) = state
        .editor_selection_bounds()
        .expect("double-click selects a word");
    assert_eq!((lo.index, hi.index), (0, 5), "the word 'hello'");
}

/// Audit #38 (T4.6): drag-selection — a press sets the anchor (a collapsed
/// caret), then a `Drag` to a later point EXTENDS a selection from that anchor to
/// the drag position (cosmic `Action::Drag` keeps the selection anchor fixed and
/// moves only the active endpoint). Pins the anchor+extend gesture
/// (`apply_pointer_gesture(Drag)`, `pointer.rs:98`) that no prior test exercised
/// — the existing mouse tests cover Click/Double/Triple only. A regression that
/// collapsed the selection on drag (or reset the anchor each move) reddens this.
#[test]
fn drag_extends_a_selection_from_the_press_anchor() {
    let fonts = SharedFontSystem::new();
    let (mut world, entity) = shaped_editor(&fonts, "hello world");
    let origin = Vec2::ZERO;

    {
        let mut state = world.get_mut::<TextEditState>(entity).unwrap();
        let mut fs = fonts.lock();
        // Press at the far left ⇒ anchor at index 0, a bare caret (no selection).
        state.apply_pointer_gesture(&mut fs, PointerGesture::Click, Vec2::new(0.0, 8.0), origin);
        assert!(
            state.editor_selection_is_none(),
            "a fresh press is a collapsed caret, not a selection"
        );
        assert_eq!(state.caret().index, 0, "anchor caret at the line start");
    }

    {
        let mut state = world.get_mut::<TextEditState>(entity).unwrap();
        let mut fs = fonts.lock();
        // Drag rightward to a point past several glyphs ⇒ a selection growing
        // from the index-0 anchor to the drag hit. The active endpoint follows
        // the pointer; the anchor stays at 0.
        state.apply_pointer_gesture(
            &mut fs,
            PointerGesture::Drag,
            Vec2::new(1000.0, 8.0),
            origin,
        );
    }

    let state = world.get::<TextEditState>(entity).unwrap();
    let (lo, hi) = state
        .editor_selection_bounds()
        .expect("a drag from the anchor produces a non-collapsed selection");
    // The drag reached the far right ⇒ the active endpoint clamped to the line
    // end (index 11). The ordered bounds span the whole anchored range.
    assert_eq!(
        (lo.index, hi.index),
        (0, 11),
        "drag selected from the press anchor (0) to the drag point (line end)"
    );
    // The moving endpoint (the caret) is the drag target, the anchor held at 0.
    assert_eq!(
        state.caret().index,
        11,
        "the active endpoint follows the drag, not the held anchor"
    );
}

/// Audit #38 (T4.6): a BACKWARD drag (anchor right, drag left) keeps the same
/// anchor and moves the active endpoint left, so the ORDERED bounds put the drag
/// target first and the anchor last — proving the anchor is genuinely fixed (not
/// re-seeded to the moving point). Complements the forward-drag case above.
#[test]
fn backward_drag_holds_the_anchor_and_moves_the_active_endpoint_left() {
    let fonts = SharedFontSystem::new();
    let (mut world, entity) = shaped_editor(&fonts, "hello world");
    let origin = Vec2::ZERO;

    {
        let mut state = world.get_mut::<TextEditState>(entity).unwrap();
        let mut fs = fonts.lock();
        // Press at the far right ⇒ anchor at the line end (index 11).
        state.apply_pointer_gesture(
            &mut fs,
            PointerGesture::Click,
            Vec2::new(1000.0, 8.0),
            origin,
        );
        assert_eq!(state.caret().index, 11, "anchor caret at the line end");
    }
    {
        let mut state = world.get_mut::<TextEditState>(entity).unwrap();
        let mut fs = fonts.lock();
        // Drag back to the far left ⇒ active endpoint moves to 0, anchor stays 11.
        state.apply_pointer_gesture(&mut fs, PointerGesture::Drag, Vec2::new(0.0, 8.0), origin);
    }

    let state = world.get::<TextEditState>(entity).unwrap();
    let (lo, hi) = state
        .editor_selection_bounds()
        .expect("a backward drag still produces a non-collapsed selection");
    assert_eq!(
        (lo.index, hi.index),
        (0, 11),
        "ordered bounds still span the full range regardless of drag direction"
    );
    assert_eq!(
        state.caret().index,
        0,
        "the active endpoint moved to the drag target; the anchor held at 11"
    );
}
