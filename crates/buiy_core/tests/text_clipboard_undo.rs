//! E4 — clipboard facade + undo/redo engine (editing-and-ime §§ 7, 8, 11).
//! Clipboard is driven through the FAKE `MemClipboard` provider (no OS
//! clipboard touched) — platform-independent, avoiding the macOS/Windows CI
//! issues E2/E3 hit. The undo engine is tested both as a unit (the property
//! test, grouping fixtures) and through the real `apply_keyboard_edits`
//! system (Task 7). Headless throughout — no adapter.

use buiy_core::text::edit::{ClipboardProvider, MemClipboard};

#[test]
fn mem_clipboard_round_trips_text() {
    let mut c = MemClipboard::default();
    assert_eq!(c.get_text(), None, "empty clipboard reads None");
    c.set_text("hello".to_string());
    assert_eq!(c.get_text(), Some("hello".to_string()));
    c.set_text("world".to_string());
    assert_eq!(c.get_text(), Some("world".to_string()), "set overwrites");
}

#[test]
fn mem_clipboard_is_usable_as_a_trait_object() {
    let mut boxed: Box<dyn ClipboardProvider> = Box::new(MemClipboard::default());
    boxed.set_text("via dyn".to_string());
    assert_eq!(boxed.get_text(), Some("via dyn".to_string()));
}

use buiy_core::text::edit::{GroupKind, UndoStack, UndoUnit};
use cosmic_text::{Change, ChangeItem, Cursor};

/// A one-item insert `Change` at `(0, idx)` of `text` — a test helper that
/// mirrors what `finish_change` produces for an `Action::Insert`.
fn insert_change(idx: usize, text: &str) -> Change {
    Change {
        items: vec![ChangeItem {
            start: Cursor::new(0, idx),
            end: Cursor::new(0, idx + text.len()),
            text: text.to_string(),
            insert: true,
        }],
    }
}

fn unit(change: Change, group: GroupKind, before: usize, after: usize) -> UndoUnit {
    use buiy_core::text::edit::TextSelection;
    UndoUnit {
        change,
        caret_before: Cursor::new(0, before),
        caret_after: Cursor::new(0, after),
        selection_before: TextSelection::collapsed(Cursor::new(0, before)),
        selection_after: TextSelection::collapsed(Cursor::new(0, after)),
        group,
    }
}

#[test]
fn record_pushes_a_nonempty_unit_and_clears_redo() {
    let mut stack = UndoStack::default();
    // Seed a redo entry to prove a new record clears it (§ 8).
    stack.push_redo_for_test(unit(insert_change(0, "x"), GroupKind::Discrete, 0, 1));
    assert_eq!(stack.redo_len(), 1);

    stack.record(unit(insert_change(0, "a"), GroupKind::Discrete, 0, 1));
    assert_eq!(stack.undo_len(), 1);
    assert_eq!(stack.redo_len(), 0, "a new edit clears the redo stack");
}

#[test]
fn record_drops_an_empty_change() {
    // finish_change returns Some(Change{items: []}) when nothing was recorded
    // (Backspace at offset 0). The stack must NOT push it.
    let mut stack = UndoStack::default();
    stack.record(unit(Change::default(), GroupKind::DeleteRun, 0, 0));
    assert_eq!(stack.undo_len(), 0, "empty change is never a unit");
}

#[test]
fn pop_undo_moves_the_unit_to_redo_and_back() {
    let mut stack = UndoStack::default();
    stack.record(unit(insert_change(0, "a"), GroupKind::Discrete, 0, 1));

    let popped = stack.pop_undo().expect("one unit to undo");
    assert_eq!(popped.caret_after, Cursor::new(0, 1));
    assert_eq!(stack.undo_len(), 0);
    assert_eq!(stack.redo_len(), 1, "undone unit is now redoable");

    let redone = stack.pop_redo().expect("one unit to redo");
    assert_eq!(redone.caret_before, Cursor::new(0, 0));
    assert_eq!(stack.redo_len(), 0);
    assert_eq!(stack.undo_len(), 1, "redone unit is undoable again");
}

#[test]
fn depth_bound_drops_the_oldest_unit() {
    let mut stack = UndoStack::with_depth(3);
    for i in 0..5u32 {
        // Distinct groups so they never coalesce (Task 3) — here Discrete.
        stack.record(unit(
            insert_change(i as usize, "x"),
            GroupKind::Discrete,
            i as usize,
            i as usize + 1,
        ));
    }
    assert_eq!(stack.undo_len(), 3, "bounded to the 3 most recent units");
    // The oldest survivor is the 3rd recorded (caret_before index 2).
    let oldest = stack.undo.first().expect("non-empty");
    assert_eq!(oldest.caret_before, Cursor::new(0, 2));
}

use std::time::Duration;

/// A delete `Change`: one delete item covering `[idx, idx+len)` of `text`.
fn delete_change(idx: usize, text: &str) -> Change {
    Change {
        items: vec![ChangeItem {
            start: Cursor::new(0, idx),
            end: Cursor::new(0, idx + text.len()),
            text: text.to_string(),
            insert: false,
        }],
    }
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn adjacent_typing_within_the_window_coalesces_into_one_unit() {
    let mut stack = UndoStack::default();
    // Type "a" at 0→1, then "b" at 1→2, 100ms later, caret adjacent.
    stack.record_grouped(
        unit(insert_change(0, "a"), GroupKind::TypingRun, 0, 1),
        ms(0),
    );
    stack.record_grouped(
        unit(insert_change(1, "b"), GroupKind::TypingRun, 1, 2),
        ms(100),
    );
    assert_eq!(stack.undo_len(), 1, "adjacent in-window typing is ONE unit");
    let merged = &stack.undo[0];
    assert_eq!(merged.change.items.len(), 2, "both items kept for replay");
    assert_eq!(merged.caret_before, Cursor::new(0, 0), "before = first");
    assert_eq!(merged.caret_after, Cursor::new(0, 2), "after = last");
}

#[test]
fn typing_past_the_time_window_starts_a_new_unit() {
    let mut stack = UndoStack::default();
    stack.record_grouped(
        unit(insert_change(0, "a"), GroupKind::TypingRun, 0, 1),
        ms(0),
    );
    // 2 seconds later — well past the 1s window.
    stack.record_grouped(
        unit(insert_change(1, "b"), GroupKind::TypingRun, 1, 2),
        ms(2000),
    );
    assert_eq!(stack.undo_len(), 2, "a long pause seals the run");
}

#[test]
fn typing_with_a_caret_jump_starts_a_new_unit() {
    let mut stack = UndoStack::default();
    stack.record_grouped(
        unit(insert_change(5, "a"), GroupKind::TypingRun, 5, 6),
        ms(0),
    );
    // In-window, but the caret is NOT adjacent (clicked elsewhere then typed).
    stack.record_grouped(
        unit(insert_change(0, "b"), GroupKind::TypingRun, 0, 1),
        ms(100),
    );
    assert_eq!(stack.undo_len(), 2, "non-adjacent caret seals the run");
}

#[test]
fn a_seal_breaks_the_run_even_within_the_window() {
    let mut stack = UndoStack::default();
    stack.record_grouped(
        unit(insert_change(0, "a"), GroupKind::TypingRun, 0, 1),
        ms(0),
    );
    stack.seal(); // an arrow key / click happened between the two types
    stack.record_grouped(
        unit(insert_change(1, "b"), GroupKind::TypingRun, 1, 2),
        ms(50),
    );
    assert_eq!(stack.undo_len(), 2, "a sealed run never re-opens");
}

#[test]
fn same_direction_deletes_coalesce_typing_and_delete_do_not_mix() {
    let mut stack = UndoStack::default();
    // Two backspaces: delete "b" at 1, then "a" at 0 — same direction, adjacent.
    stack.record_grouped(
        unit(delete_change(1, "b"), GroupKind::DeleteRun, 2, 1),
        ms(0),
    );
    stack.record_grouped(
        unit(delete_change(0, "a"), GroupKind::DeleteRun, 1, 0),
        ms(50),
    );
    assert_eq!(stack.undo_len(), 1, "adjacent deletes coalesce");

    // A typing unit must NOT join a delete run (different GroupKind).
    stack.record_grouped(
        unit(insert_change(0, "x"), GroupKind::TypingRun, 0, 1),
        ms(60),
    );
    assert_eq!(stack.undo_len(), 2, "typing never joins a delete run");
}

#[test]
fn discrete_and_composition_never_coalesce() {
    let mut stack = UndoStack::default();
    stack.record_grouped(
        unit(insert_change(0, "pasted"), GroupKind::Discrete, 0, 6),
        ms(0),
    );
    stack.record_grouped(
        unit(insert_change(6, "more"), GroupKind::Discrete, 6, 10),
        ms(10),
    );
    assert_eq!(stack.undo_len(), 2, "Discrete units stand alone");
    stack.record_grouped(
        unit(insert_change(10, "字"), GroupKind::Composition, 10, 11),
        ms(20),
    );
    stack.record_grouped(
        unit(insert_change(11, "体"), GroupKind::Composition, 11, 12),
        ms(25),
    );
    assert_eq!(stack.undo_len(), 4, "each composition is its own unit");
}
