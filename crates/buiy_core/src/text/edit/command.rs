//! `EditCommand` — the Buiy-owned editing verb vocabulary (editing-and-ime
//! § 3). It borrows the cosmic `Action` *shape* but is Buiy-owned because
//! clipboard / undo / submit verbs do not exist in `Action`. It names ONE
//! cosmic type — `Motion` — which is a pure-data cursor-movement enum (no
//! `Editor`/`Edit`/`Change`), so the facade-boundary tripwire (`Editor`,
//! `Edit`, `Action`, `Change`) does not flag it. The lowering to `Action`
//! (input.rs) is what must stay in the facade.
//!
//! **`Insert(String)` not `SmolStr`** (E2 erratum 1): `smol_str` is not a
//! Buiy dependency; a `String` copied from `KeyboardInput.text` lowers
//! identically (`Action::Insert` takes a `char`, so we iterate `chars()`).

use cosmic_text::Motion;

/// A single editing command, the unit the keymap produces and the editor
/// applies (editing-and-ime § 3). Clipboard verbs (`Cut`/`Copy`/`Paste`)
/// and undo verbs (`Undo`/`Redo`) are recognized here so the keymap rows
/// exist from E2, but their behavior lands in E4 (this phase routes them to
/// a no-op with a documented TODO — they must not silently insert text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditCommand {
    /// Cursor movement (arrows, Home/End, word-nav, PgUp/PgDn, doc
    /// start/end). `extend = true` grows the selection (Shift held); the
    /// editor moves in **visual** order per UAX #9 — the keymap never
    /// computes BiDi (§ 4.1).
    Motion(Motion, /* extend: */ bool),
    /// Insert literal text (the layout-resolved, dead-key-composed event
    /// `text` field), iterated as chars into `Action::Insert` (§ 3).
    Insert(String),
    /// Grapheme-correct deletion before / at the caret (inherited from
    /// `Action::Backspace` / `Action::Delete`).
    Backspace,
    Delete,
    /// Newline (multi-line) — on a `SingleLine` editor this is intercepted
    /// to `Submit` before reaching the editor (§ 3.3).
    Enter,
    /// § 7 — behavior is E4. Recognized here so the keymap rows exist.
    Cut,
    Copy,
    Paste,
    /// § 8 — behavior is E4.
    Undo,
    Redo,
    /// Select the whole buffer (Ctrl/Cmd-A).
    SelectAll,
    /// Clear the selection / cancel composition (§ 6.2d).
    Escape,
    /// Single-line Enter (§ 3.3): the host-facing `EditSubmitted` Message is
    /// finalized in E6; E2 emits it internally as an `EditOutcome` flag.
    Submit,
}
