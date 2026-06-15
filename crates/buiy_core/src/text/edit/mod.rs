//! `buiy_core::text::edit` — the editing facade (editing-and-ime § 2.1
//! "lock-in containment"). This module, and ONLY this module, names the
//! cosmic `Editor`/`Edit`/`Action`/`Change` types; every other Buiy system
//! talks to `TextEditState` and `TextBufferAccess`. A future substrate swap
//! stays local here. The boundary is mechanically enforced by
//! `tests/text_facade_boundary.rs`.
//!
//! E1 lands the substrate: `TextEditState`, the policy markers, and the
//! `TextBufferAccess` accessor. Input/keymap (E2), caret/selection (E3),
//! clipboard/undo (E4), IME (E5), and lifecycle/widget (E6) extend it.

mod access;
mod caret;
mod clipboard;
mod command;
mod input;
pub mod keymap;
mod pointer;
mod selection;
mod state;
mod undo;

pub use access::{
    TextBufferAccess, TextBufferAccessItem, TextBufferAccessReadOnly, TextBufferAccessReadOnlyItem,
};
pub use caret::{CaretMoved, SelectionChanged, write_caret_and_selection};
pub use clipboard::{ArboardClipboard, Clipboard, ClipboardProvider, MemClipboard};
pub use command::EditCommand;
pub use input::{EditContext, EditOutcome, TextChanged, apply_keyboard_edits};
pub use keymap::{Keymap, KeymapTable, Modifiers, default_keymap_for_platform};
pub use pointer::{ClickTracker, PointerGesture, pointer_selection, pointer_to_cursor};
pub use selection::{SelectionRange, TextSelection};
pub use state::{CaretBlink, Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
pub use undo::{DEFAULT_UNDO_DEPTH, EditRedone, EditUndone, GroupKind, UndoStack, UndoUnit};
