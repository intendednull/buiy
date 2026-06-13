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
mod state;

pub use access::{
    TextBufferAccess, TextBufferAccessItem, TextBufferAccessReadOnly, TextBufferAccessReadOnlyItem,
};
pub use state::{Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
