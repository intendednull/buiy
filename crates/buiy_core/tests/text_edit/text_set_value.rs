//! C2 — the editor's seed / programmatic-set channel is the EXISTING
//! `EditCommand` verbs: `Insert` seeds an empty editor; `SelectAll` + `Insert`
//! does a whole-value programmatic set. There is NO `EditCommand::SetValue`
//! (the agent-interface campaign owns the `EditCommand` surface and lowers
//! `Action::SetValue`-text via this same `SelectAll` + `Insert` — umbrella § 2.7,
//! action-router.md § 4, phasing.md P1c).
//!
//! Headless: builds a `TextEditState` directly and applies via the public
//! facade, locking the shared FontSystem the way `text_mouse_selection.rs`
//! does (no LayoutPlugin needed — apply is a pure edit-path call).

use buiy_core::text::SharedFontSystem;
use buiy_core::text::edit::{EditCommand, EditOutcome, TextEditState};

fn editor(fonts: &SharedFontSystem, seed: &str) -> TextEditState {
    // for_font_size(16.0) == Metrics::new(16.0, 19.2) (the 1.2 line-height
    // scale, state.rs:170) — the ONE constructor form used across all plans.
    let mut state = TextEditState::for_font_size(16.0);
    let mut fs = fonts.lock();
    // SEED: a fresh editor is empty (one empty line, value()==""); a single
    // Insert into the empty buffer makes the whole value. This is the seed path.
    state.apply(&mut fs, EditCommand::Insert(seed.into()), false, false);
    drop(fs);
    state
}

/// Programmatic whole-value set via the EXISTING verbs: select-all, then insert
/// (type-over). This is the exact lowering the agent-interface router applies
/// for `Action::SetValue` on a text field (action-router.md § 4).
fn set_value(state: &mut TextEditState, fonts: &SharedFontSystem, new: &str) -> EditOutcome {
    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::SelectAll, false, false);
    let outcome = state.apply(&mut fs, EditCommand::Insert(new.into()), false, false);
    drop(fs);
    outcome
}

#[test]
fn select_all_plus_insert_replaces_the_whole_value() {
    let fonts = SharedFontSystem::new();
    let mut state = editor(&fonts, "old content");
    assert_eq!(state.value(), "old content");

    let outcome = set_value(&mut state, &fonts, "new value");

    assert_eq!(
        state.value(),
        "new value",
        "SelectAll + Insert replaces the entire logical value"
    );
    assert!(
        outcome.value_changed,
        "a real value change flags value_changed → TextChanged"
    );
}

#[test]
fn insert_seeds_an_empty_editor() {
    let fonts = SharedFontSystem::new();
    // A bare editor is "" before any verb; seeding is a single Insert.
    let mut state = TextEditState::for_font_size(16.0);
    assert_eq!(state.value(), "", "a fresh editor is empty");
    let mut fs = fonts.lock();
    state.apply(
        &mut fs,
        EditCommand::Insert("seed text".into()),
        false,
        false,
    );
    drop(fs);
    assert_eq!(
        state.value(),
        "seed text",
        "Insert into the empty editor seeds the whole value"
    );
}

#[test]
fn select_all_plus_delete_clears_the_value() {
    // The clear-to-empty channel via the EXISTING verbs is `SelectAll` + `Delete`
    // (delete the whole selection). NOTE: `SelectAll` + `Insert("")` is NOT a
    // clear — the `Insert` arm iterates `text.chars()`, so an empty string fires
    // no `Action::Insert` and never deletes the selection (verified: the value
    // is unchanged). The genuine clear uses `Delete` on the selection.
    let fonts = SharedFontSystem::new();
    let mut state = editor(&fonts, "content");

    let mut fs = fonts.lock();
    state.apply(&mut fs, EditCommand::SelectAll, false, false);
    // Sanity: Insert("") leaves the selection intact (no chars to insert).
    state.apply(&mut fs, EditCommand::Insert("".into()), false, false);
    assert_eq!(
        state.value(),
        "content",
        "Insert(\"\") fires no Action::Insert, so it does NOT clear the selection"
    );
    // Delete the selection — the real clear-to-empty channel.
    state.apply(&mut fs, EditCommand::Delete, false, false);
    drop(fs);
    assert_eq!(
        state.value(),
        "",
        "SelectAll + Delete empties the editor (the clear-to-empty channel)"
    );
}

#[test]
fn set_value_via_existing_verbs_is_undoable() {
    // The programmatic set inherits the existing verbs' undo behavior (no new
    // grouping is added by C2). One Undo after SelectAll+Insert returns toward
    // the prior value via the existing recorded edits — assert the value is
    // restorable, not a specific undo-unit count (the count is the existing
    // verbs' behavior, agent-interface-owned, not a C2 contract).
    let fonts = SharedFontSystem::new();
    let mut state = editor(&fonts, "first");
    set_value(&mut state, &fonts, "second");
    assert_eq!(state.value(), "second");

    let mut fs = fonts.lock();
    // Undo enough to walk back past the type-over (the existing verbs each
    // record; loop until the value differs from "second" or the stack drains).
    for _ in 0..4 {
        if state.value() != "second" {
            break;
        }
        state.apply(&mut fs, EditCommand::Undo, false, false);
    }
    drop(fs);
    assert_ne!(
        state.value(),
        "second",
        "the programmatic set is undoable via the existing verbs"
    );
}
