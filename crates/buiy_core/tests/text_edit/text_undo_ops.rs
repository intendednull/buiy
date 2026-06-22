//! E4 — `apply_tracked` wraps each editing op as an undo unit, and Undo/Redo
//! replay through `apply_change` (editing-and-ime § 8). Driven directly
//! against a headless `FontSystem` (cosmic shaping is CPU — no adapter), the
//! E2 `text_editing_ops.rs` pattern. Clipboard ops use the FAKE provider.

use buiy_core::text::SharedFontSystem;
use buiy_core::text::edit::{EditCommand, EditContext, MemClipboard, TextEditState};
use cosmic_text::{Metrics, Motion};
use std::time::Duration;

/// A context for a multi-line, mutable editor at virtual time `now`, with a
/// fresh fake clipboard the caller can inspect.
fn ctx(now_ms: u64, clipboard: &mut MemClipboard) -> EditContext<'_> {
    EditContext {
        single_line: false,
        read_only: false,
        now: Duration::from_millis(now_ms),
        clipboard,
    }
}

#[test]
fn typing_then_undo_restores_the_previous_value_and_caret() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();

    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("hi".into()),
        &mut ctx(0, &mut clip),
    );
    assert_eq!(state.value(), "hi");
    assert_eq!(state.undo_depth(), 1, "one typing run recorded");

    let out = state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(10, &mut clip));
    assert_eq!(state.value(), "", "undo removes the typed run");
    assert!(out.value_changed, "undo is a value change (republish)");
    assert_eq!(state.undo_depth(), 0);
    assert_eq!(state.redo_depth(), 1, "the undone run is redoable");
}

#[test]
fn redo_reapplies_the_change_and_restores_the_after_caret() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();

    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("ab".into()),
        &mut ctx(0, &mut clip),
    );
    state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(10, &mut clip));
    assert_eq!(state.value(), "");

    let out = state.apply_tracked(&mut fs, EditCommand::Redo, &mut ctx(20, &mut clip));
    assert_eq!(state.value(), "ab", "redo reapplies the change");
    assert!(out.value_changed);
    assert_eq!(state.redo_depth(), 0);
    assert_eq!(state.undo_depth(), 1);
}

#[test]
fn a_motion_between_two_types_seals_the_run_so_undo_is_two_steps() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();

    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("a".into()),
        &mut ctx(0, &mut clip),
    );
    // An arrow key seals the open typing run.
    state.apply_tracked(
        &mut fs,
        EditCommand::Motion(Motion::Left, false),
        &mut ctx(5, &mut clip),
    );
    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("b".into()),
        &mut ctx(10, &mut clip),
    );
    assert_eq!(state.value(), "ba");
    assert_eq!(state.undo_depth(), 2, "the motion split the run");

    state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(20, &mut clip));
    assert_eq!(state.value(), "a", "first undo removes the second insert");
}

#[test]
fn backspace_at_offset_zero_records_no_unit() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();
    // Empty buffer: Backspace changes nothing ⇒ empty Change ⇒ no unit.
    let out = state.apply_tracked(&mut fs, EditCommand::Backspace, &mut ctx(0, &mut clip));
    assert!(!out.value_changed);
    assert_eq!(state.undo_depth(), 0, "a no-op edit is never an undo unit");
}

use buiy_core::text::edit::ClipboardProvider;

/// Select the whole buffer, then run the command. Helper to set up a
/// non-empty selection for Cut/Copy.
fn select_all(
    state: &mut TextEditState,
    fs: &mut cosmic_text::FontSystem,
    clip: &mut MemClipboard,
) {
    state.apply_tracked(fs, EditCommand::SelectAll, &mut ctx(0, clip));
}

#[test]
fn copy_puts_the_selection_on_the_clipboard_without_changing_the_value() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();

    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("hello".into()),
        &mut ctx(0, &mut clip),
    );
    select_all(&mut state, &mut fs, &mut clip);
    let out = state.apply_tracked(&mut fs, EditCommand::Copy, &mut ctx(10, &mut clip));

    assert!(!out.value_changed, "copy never changes the value");
    assert_eq!(clip.get_text(), Some("hello".to_string()));
    assert_eq!(state.value(), "hello", "buffer intact");
}

#[test]
fn cut_copies_then_deletes_the_selection_as_one_undoable_unit() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();

    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("hello".into()),
        &mut ctx(0, &mut clip),
    );
    select_all(&mut state, &mut fs, &mut clip);
    let out = state.apply_tracked(&mut fs, EditCommand::Cut, &mut ctx(10, &mut clip));

    assert!(out.value_changed, "cut removes the selection");
    assert_eq!(clip.get_text(), Some("hello".to_string()));
    assert_eq!(state.value(), "", "selection deleted");
    assert_eq!(state.undo_depth(), 2, "the insert run + the cut");

    // Undo the cut restores the text.
    state.apply_tracked(&mut fs, EditCommand::Undo, &mut ctx(20, &mut clip));
    assert_eq!(state.value(), "hello", "undo brings the cut text back");
}

#[test]
fn paste_inserts_the_clipboard_text() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();
    clip.set_text("pasted".to_string());

    let out = state.apply_tracked(&mut fs, EditCommand::Paste, &mut ctx(0, &mut clip));
    assert!(out.value_changed);
    assert_eq!(state.value(), "pasted");
    assert_eq!(state.undo_depth(), 1, "paste is one undoable unit");
}

#[test]
fn single_line_paste_strips_newlines() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();
    clip.set_text("a\nb\r\nc".to_string());

    // single_line: true in the context.
    let mut single_ctx = EditContext {
        single_line: true,
        read_only: false,
        now: Duration::from_millis(0),
        clipboard: &mut clip,
    };
    let out = state.apply_tracked(&mut fs, EditCommand::Paste, &mut single_ctx);
    assert!(out.value_changed);
    assert_eq!(
        state.value(),
        "abc",
        "newlines stripped on a single-line editor (§ 3.3)"
    );
}

#[test]
fn paste_with_an_empty_clipboard_is_a_no_op() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();
    let out = state.apply_tracked(&mut fs, EditCommand::Paste, &mut ctx(0, &mut clip));
    assert!(!out.value_changed);
    assert_eq!(state.undo_depth(), 0, "nothing to paste, nothing recorded");
}

// ── HTML flavor (the follow-up slice) ────────────────────────────────────
// Copy/Cut now set BOTH the plain-text flavor (unchanged § 7 path) AND an
// HTML flavor (the escaped plain text — a plain-text editor has no rich runs).
// Paste still PREFERS the § 3.3 text path and never consults the HTML getter.

#[test]
fn copy_also_sets_an_escaped_html_flavor() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();

    // Characters that MUST be escaped in html: & < > " '
    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("a<b>&\"'".into()),
        &mut ctx(0, &mut clip),
    );
    select_all(&mut state, &mut fs, &mut clip);
    state.apply_tracked(&mut fs, EditCommand::Copy, &mut ctx(10, &mut clip));

    // The plain-text flavor is the raw selection (§ 7 path, unchanged).
    assert_eq!(clip.get_text(), Some("a<b>&\"'".to_string()));
    // The html flavor is the escaped form.
    assert_eq!(
        clip.get_html(),
        Some("a&lt;b&gt;&amp;&quot;&#39;".to_string()),
        "copy sets an escaped-html flavor alongside the text"
    );
}

#[test]
fn cut_sets_both_text_and_html_flavors() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();

    state.apply_tracked(
        &mut fs,
        EditCommand::Insert("x<y".into()),
        &mut ctx(0, &mut clip),
    );
    select_all(&mut state, &mut fs, &mut clip);
    let out = state.apply_tracked(&mut fs, EditCommand::Cut, &mut ctx(10, &mut clip));

    assert!(out.value_changed, "cut removes the selection");
    assert_eq!(clip.get_text(), Some("x<y".to_string()));
    assert_eq!(clip.get_html(), Some("x&lt;y".to_string()));
    assert_eq!(state.value(), "", "selection deleted");
    assert_eq!(state.undo_depth(), 2, "the insert run + the cut");
}

#[test]
fn paste_prefers_text_and_ignores_html() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    let mut clip = MemClipboard::default();
    // Seed DISTINCT text and html flavors: Paste must take the text one.
    clip.set_text("abc".to_string());
    clip.set_html("<b>zzz</b>".to_string());

    let out = state.apply_tracked(&mut fs, EditCommand::Paste, &mut ctx(0, &mut clip));
    assert!(out.value_changed);
    assert_eq!(
        state.value(),
        "abc",
        "paste takes the § 3.3 text path, never the html flavor"
    );
    assert_eq!(state.undo_depth(), 1, "paste is one undoable unit");
}
