//! E5 — IME composition operations applied directly to the editor
//! (editing-and-ime §§ 6.1, 6.2). The splice/remove/commit primitives are
//! tested against a real (headless) `FontSystem` — reshape needs none at
//! splice time, but the commit `insert_at` does. No adapter (cosmic shaping
//! is CPU); no winit window (synthetic operations). The four invariants are
//! each a named test here + in `text_ime_system.rs` (system level).

use buiy_core::text::SharedFontSystem;
use buiy_core::text::edit::{EditCommand, TextEditState};
use cosmic_text::Metrics;

/// A Preedit splice inserts the composing text into the buffer (so it
/// reflows + shapes), records the live span, and — invariant (a) — adds
/// NOTHING to the undo stack.
#[test]
fn preedit_splice_inserts_into_buffer_without_touching_undo() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    // Seed a logical value and move the caret to the end.
    state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
    assert_eq!(state.value(), "ab");
    let undo_before = state.undo_depth();

    // Splice a preedit "X" at the caret (after "ab").
    state.splice_preedit(&mut fs, "X", Some((0, 1)));

    // The buffer CONTENT now contains the preedit (it shapes + reflows)...
    assert_eq!(state.buffer_text_for_test(), "abX");
    // ...but the LOGICAL value excludes it (invariant b, proven fully in Task 2).
    assert_eq!(state.value(), "ab");
    // ...and undo is UNCHANGED (invariant a).
    assert_eq!(
        state.undo_depth(),
        undo_before,
        "splice must not record a Change"
    );
    assert!(state.has_preedit());
}

/// A second Preedit REPLACES the first span (no accumulation).
#[test]
fn preedit_replace_swaps_the_span() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);

    state.splice_preedit(&mut fs, "X", None);
    assert_eq!(state.buffer_text_for_test(), "abX");
    state.splice_preedit(&mut fs, "YZ", None);
    assert_eq!(
        state.buffer_text_for_test(),
        "abYZ",
        "second preedit replaces the first"
    );
    assert_eq!(state.value(), "ab");
}

/// Removing the preedit restores the buffer to its pre-composition content
/// and clears the span (invariant d — no orphan).
#[test]
fn remove_preedit_restores_buffer_and_clears_span() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);

    state.splice_preedit(&mut fs, "XY", None);
    assert_eq!(state.buffer_text_for_test(), "abXY");
    state.remove_preedit(&mut fs);
    assert_eq!(state.buffer_text_for_test(), "ab", "buffer restored");
    assert!(!state.has_preedit(), "no orphan span");
    assert_eq!(state.value(), "ab");
}

/// Invariant (b): `value()` excludes the live preedit byte range even when
/// the preedit is mid-line (the reflow case the splice exists for).
#[test]
fn value_excludes_preedit_midline() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    // "hello world", caret moved to byte 5 (between "hello" and " world").
    state.apply(
        &mut fs,
        EditCommand::Insert("hello world".into()),
        false,
        false,
    );
    for _ in 0..6 {
        state.apply(&mut fs, cosmic_motion_left(), false, false);
    }
    // Caret now at index 5. Splice a preedit there.
    state.splice_preedit(&mut fs, "XYZ", None);
    assert_eq!(
        state.buffer_text_for_test(),
        "helloXYZ world",
        "preedit shapes mid-line"
    );
    assert_eq!(
        state.value(),
        "hello world",
        "logical value excludes the preedit"
    );

    state.remove_preedit(&mut fs);
    assert_eq!(state.value(), "hello world");
}

fn cosmic_motion_left() -> EditCommand {
    EditCommand::Motion(cosmic_text::Motion::Left, false)
}

/// Compose-over-selection: when a composition STARTS over a non-collapsed
/// selection, the first splice DELETES the selection first (replace-selection
/// convention) and splices the preedit at the now-collapsed caret. The stashed
/// delete is NOT yet on the undo stack — invariant (a) still holds for the
/// splice itself.
#[test]
fn compose_over_selection_splice_deletes_and_stashes() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    // "abc", then select "bc" (SelectAll selects all; shrink by moving the
    // anchor — simplest is: caret to index 1, then extend Right ×2).
    state.apply(&mut fs, EditCommand::Insert("abc".into()), false, false);
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Home, false),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, false),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, true),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, true),
        false,
        false,
    );
    assert!(
        !state.mirror_selection().is_collapsed(),
        "bc is selected before composition starts"
    );
    let undo_before = state.undo_depth();

    // Start a composition over the selection.
    state.splice_preedit(&mut fs, "X", Some((0, 1)));

    // The selection is gone, the preedit sits at the collapsed caret.
    assert_eq!(state.buffer_text_for_test(), "aX");
    // Logical value excludes the preedit AND the selection is already deleted.
    assert_eq!(state.value(), "a");
    assert!(state.has_preedit());
    // The stashed delete has NOT reached the undo stack yet (invariant a holds
    // for the splice; the delete is folded in only at commit).
    assert_eq!(
        state.undo_depth(),
        undo_before,
        "the stashed compose-delete is not yet an undo unit"
    );
}

/// Compose-over-selection: commit folds the stashed delete + the commit-insert
/// into ONE `GroupKind::Composition` unit. A single Undo restores BOTH the
/// deleted selection text and the committed text; redo replays both.
#[test]
fn compose_over_selection_commit_is_one_unit_and_one_undo_restores_both() {
    use buiy_core::text::edit::GroupKind;
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    // "abc", select "bc".
    state.apply(&mut fs, EditCommand::Insert("abc".into()), false, false);
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Home, false),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, false),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, true),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, true),
        false,
        false,
    );
    let undo_before = state.undo_depth();

    state.splice_preedit(&mut fs, "ni", None);
    state.commit_preedit(&mut fs, "你", Duration::ZERO);

    assert_eq!(state.value(), "a你", "selection replaced by the commit");
    assert!(!state.has_preedit());
    assert_eq!(
        state.undo_depth(),
        undo_before + 1,
        "delete + commit fold into ONE unit"
    );
    assert_eq!(
        state.undo_top_group_for_test(),
        Some(GroupKind::Composition)
    );

    // ONE undo restores BOTH the deleted "bc" and the committed "你".
    state.apply(&mut fs, EditCommand::Undo, false, false);
    assert_eq!(
        state.value(),
        "abc",
        "one undo reverses both the delete and the commit"
    );
    assert!(
        !state.mirror_selection().is_collapsed(),
        "the bc selection is restored on undo"
    );

    // Redo replays both.
    state.apply(&mut fs, EditCommand::Redo, false, false);
    assert_eq!(state.value(), "a你", "redo replays delete + commit");
}

/// Compose-over-selection cancel (Escape / empty preedit at the unit level):
/// reverse-applies the stashed delete so the value returns to the original and
/// the selection is restored. The unselected path (no stash) is a no-op.
#[test]
fn compose_over_selection_cancel_restores_via_remove() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    state.apply(&mut fs, EditCommand::Insert("abc".into()), false, false);
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Home, false),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, false),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, true),
        false,
        false,
    );
    state.apply(
        &mut fs,
        EditCommand::Motion(cosmic_text::Motion::Right, true),
        false,
        false,
    );

    state.splice_preedit(&mut fs, "ni", None);
    assert_eq!(state.value(), "a", "selection deleted at compose start");

    // Cancel via remove_preedit: the deleted "bc" comes back, selection restored.
    state.remove_preedit(&mut fs);
    assert_eq!(
        state.value(),
        "abc",
        "cancel reverse-applies the compose-delete"
    );
    assert!(!state.has_preedit());
    assert!(
        !state.mirror_selection().is_collapsed(),
        "the bc selection is restored on cancel"
    );
}

use std::time::Duration;

/// Invariant (c): a full composition (one or more Preedit splices then a
/// Commit) records EXACTLY ONE undo unit, grouped `Composition`; undoing it
/// restores the pre-composition value in ONE step.
#[test]
fn commit_is_exactly_one_composition_undo_unit() {
    use buiy_core::text::edit::GroupKind;
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
    let undo_before = state.undo_depth();

    // Compose: two Preedit updates, then Commit "好".
    state.splice_preedit(&mut fs, "h", None);
    state.splice_preedit(&mut fs, "ha", None);
    assert_eq!(
        state.undo_depth(),
        undo_before,
        "no preedit splice reached undo"
    );

    state.commit_preedit(&mut fs, "好", Duration::ZERO);
    assert_eq!(state.value(), "ab好");
    assert!(!state.has_preedit(), "commit clears the span");
    assert_eq!(
        state.undo_depth(),
        undo_before + 1,
        "commit = ONE undo unit"
    );
    assert_eq!(
        state.undo_top_group_for_test(),
        Some(GroupKind::Composition)
    );

    // One Undo restores the pre-composition value.
    state.apply(&mut fs, EditCommand::Undo, false, false);
    assert_eq!(
        state.value(),
        "ab",
        "undo removes the whole commit in one step"
    );
}

/// A composition does NOT coalesce into a preceding typing run (Composition
/// never coalesces; the commit seals the open group first).
#[test]
fn commit_does_not_coalesce_with_prior_typing() {
    let fonts = SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();

    state.apply(&mut fs, EditCommand::Insert("x".into()), false, false); // a TypingRun unit
    let depth = state.undo_depth();
    state.splice_preedit(&mut fs, "a", None);
    state.commit_preedit(&mut fs, "亜", Duration::from_millis(10));
    assert_eq!(
        state.undo_depth(),
        depth + 1,
        "commit is its own unit, not coalesced"
    );
}
