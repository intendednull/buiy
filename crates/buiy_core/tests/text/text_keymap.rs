//! E2 — the keymap table and `EditCommand` vocabulary (editing-and-ime
//! §§ 3, 3.1, 3.2). Pure / headless: no World, no adapter, no FontSystem —
//! the table is data and `resolve` is a lookup.

use buiy_core::text::edit::{EditCommand, Modifiers};
use cosmic_text::Motion;

/// `EditCommand` carries the § 3 verbs. This pins the public shape the
/// keymap produces and the lowering consumes.
#[test]
fn edit_command_carries_its_variants() {
    // Motion with the extend flag (Shift+arrow ⇒ extend = true).
    let m = EditCommand::Motion(Motion::Left, true);
    assert_eq!(m, EditCommand::Motion(Motion::Left, true));
    assert_ne!(m, EditCommand::Motion(Motion::Left, false));

    // The discrete verbs construct and compare.
    assert_eq!(EditCommand::Backspace, EditCommand::Backspace);
    assert_eq!(
        EditCommand::Insert(String::from("a")),
        EditCommand::Insert("a".into())
    );
    assert_ne!(EditCommand::Cut, EditCommand::Copy);
    assert_eq!(EditCommand::Submit, EditCommand::Submit);
}

use buiy_core::text::edit::KeymapTable;

/// `Modifiers` is a plain bitset-ish struct; equality and the `ctrl()`/
/// `shift()` constructors behave.
#[test]
fn modifiers_construct_and_compare() {
    assert_eq!(Modifiers::NONE, Modifiers::default());
    assert_ne!(Modifiers::NONE, Modifiers::ctrl());
    assert_eq!(
        Modifiers::ctrl(),
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        }
    );
    let cs = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    assert!(cs.ctrl && cs.shift && !cs.alt && !cs.cmd);
}

/// An empty table resolves nothing; an inserted row resolves exactly.
#[test]
fn keymap_table_resolves_inserted_rows() {
    use buiy_core::text::edit::keymap::KeyKind;
    let mut table = KeymapTable::default();
    assert_eq!(table.resolve(Modifiers::NONE, KeyKind::ArrowLeft), None);
    table.insert(
        Modifiers::NONE,
        KeyKind::ArrowLeft,
        EditCommand::Motion(Motion::Left, false),
    );
    assert_eq!(
        table.resolve(Modifiers::NONE, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::Left, false)),
    );
    // A different modifier set does not collide.
    assert_eq!(table.resolve(Modifiers::shift(), KeyKind::ArrowLeft), None);
}

use buiy_core::text::edit::keymap::{KeyKind, linux_windows_keymap, macos_keymap};

/// § 3.1 (NORMATIVE), Linux/Windows table. Arrows, word-nav (Ctrl+arrow),
/// Home/End, doc start/end (Ctrl+Home/End), PgUp/PgDn, Shift⇒extend,
/// Ctrl-A/X/C/V/Z, redo (Ctrl-Y and Ctrl-Shift-Z), Backspace/Delete, Enter,
/// Escape.
#[test]
fn linux_windows_table_encodes_the_normative_rows() {
    let t = linux_windows_keymap();
    let none = Modifiers::NONE;
    let ctrl = Modifiers::ctrl();
    let shift = Modifiers::shift();
    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };

    // Plain arrows ⇒ Motion, no extend.
    assert_eq!(
        t.resolve(none, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::Left, false))
    );
    assert_eq!(
        t.resolve(none, KeyKind::ArrowRight),
        Some(EditCommand::Motion(Motion::Right, false))
    );
    assert_eq!(
        t.resolve(none, KeyKind::ArrowUp),
        Some(EditCommand::Motion(Motion::Up, false))
    );
    assert_eq!(
        t.resolve(none, KeyKind::ArrowDown),
        Some(EditCommand::Motion(Motion::Down, false))
    );

    // Shift+arrow ⇒ extend = true.
    assert_eq!(
        t.resolve(shift, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::Left, true))
    );

    // Ctrl+arrow ⇒ word-nav (visual LeftWord/RightWord, § 4.1).
    assert_eq!(
        t.resolve(ctrl, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::LeftWord, false))
    );
    assert_eq!(
        t.resolve(ctrl, KeyKind::ArrowRight),
        Some(EditCommand::Motion(Motion::RightWord, false))
    );
    // Ctrl+Shift+arrow ⇒ word-nav extend.
    assert_eq!(
        t.resolve(ctrl_shift, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::LeftWord, true))
    );

    // Home/End and doc start/end.
    assert_eq!(
        t.resolve(none, KeyKind::Home),
        Some(EditCommand::Motion(Motion::Home, false))
    );
    assert_eq!(
        t.resolve(none, KeyKind::End),
        Some(EditCommand::Motion(Motion::End, false))
    );
    assert_eq!(
        t.resolve(ctrl, KeyKind::Home),
        Some(EditCommand::Motion(Motion::BufferStart, false))
    );
    assert_eq!(
        t.resolve(ctrl, KeyKind::End),
        Some(EditCommand::Motion(Motion::BufferEnd, false))
    );
    assert_eq!(
        t.resolve(shift, KeyKind::Home),
        Some(EditCommand::Motion(Motion::Home, true))
    );

    // PgUp/PgDn.
    assert_eq!(
        t.resolve(none, KeyKind::PageUp),
        Some(EditCommand::Motion(Motion::PageUp, false))
    );
    assert_eq!(
        t.resolve(none, KeyKind::PageDown),
        Some(EditCommand::Motion(Motion::PageDown, false))
    );

    // Deletion + Enter + Escape.
    assert_eq!(
        t.resolve(none, KeyKind::Backspace),
        Some(EditCommand::Backspace)
    );
    assert_eq!(t.resolve(none, KeyKind::Delete), Some(EditCommand::Delete));
    assert_eq!(t.resolve(none, KeyKind::Enter), Some(EditCommand::Enter));
    assert_eq!(t.resolve(none, KeyKind::Escape), Some(EditCommand::Escape));

    // Clipboard + undo + select-all on Ctrl (§§ 7, 8, 3.1).
    assert_eq!(
        t.resolve(ctrl, KeyKind::Char),
        None,
        "Char is never a table row"
    );
    // Char-keyed verbs (A/X/C/V/Z/Y) are keyed on KeyKind::Char + a letter,
    // which the table does NOT model (Char carries no letter). They are
    // resolved by the SYSTEM from the logical key's character + modifiers
    // (Step 5). So the table holds only the non-character rows; the
    // letter-command rows are a separate lookup the system owns. This test
    // asserts the table's contract; the letter rows are tested in
    // text_editing_ops via the system.
}

/// § 3.2, macOS table: Cmd replaces Ctrl for doc-ends / clipboard /
/// select-all / undo; Option replaces Ctrl for word-nav; Cmd+arrow is
/// line-ends (Home/End).
#[test]
fn macos_table_uses_cmd_and_option() {
    let t = macos_keymap();
    let none = Modifiers::NONE;
    let cmd = Modifiers::cmd();
    let alt = Modifiers::alt();

    // Plain arrows unchanged.
    assert_eq!(
        t.resolve(none, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::Left, false))
    );
    // Option+arrow ⇒ word-nav (macOS convention).
    assert_eq!(
        t.resolve(alt, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::LeftWord, false))
    );
    assert_eq!(
        t.resolve(alt, KeyKind::ArrowRight),
        Some(EditCommand::Motion(Motion::RightWord, false))
    );
    // Cmd+arrow ⇒ line-ends (Home/End).
    assert_eq!(
        t.resolve(cmd, KeyKind::ArrowLeft),
        Some(EditCommand::Motion(Motion::Home, false))
    );
    assert_eq!(
        t.resolve(cmd, KeyKind::ArrowRight),
        Some(EditCommand::Motion(Motion::End, false))
    );
    // Cmd+Up/Down ⇒ buffer start/end (macOS doc nav).
    assert_eq!(
        t.resolve(cmd, KeyKind::ArrowUp),
        Some(EditCommand::Motion(Motion::BufferStart, false))
    );
    assert_eq!(
        t.resolve(cmd, KeyKind::ArrowDown),
        Some(EditCommand::Motion(Motion::BufferEnd, false))
    );
    // Linux Ctrl rows are NOT present on macOS (the data swap, not a runtime
    // branch — proves the tables genuinely differ).
    assert_eq!(t.resolve(Modifiers::ctrl(), KeyKind::ArrowLeft), None);
}
