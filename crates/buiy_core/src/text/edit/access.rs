//! `TextBufferAccess` — the one accessor every system uses to reach "the
//! entity's buffer" (measure-and-layout § 2.3; editing-and-ime § 2.2a). It
//! binds the display `TextBuffer` and the optional `TextEditState`, and
//! dispatches buffer reads/writes and the intrinsics cache **editor-first**:
//! when `TextEditState` is present its owned `Buffer` is authoritative
//! (`BufferRef::Owned`), else the display `TextBuffer.buffer`. Display-only
//! and editable entities take the same code path; compatibility with
//! `BufferRef::Owned` holds by construction.
//!
//! This file is INSIDE the `text::edit` facade — one of the two allowed to
//! name `Edit`. The `with_buffer*` methods hand callers a bare
//! `&Buffer`/`&mut Buffer`, so sync/measure/commit/extract stay
//! buffer-shaped and never name a cosmic editor type
//! (`tests/text_facade_boundary.rs` is the tripwire).
//!
//! **Change-detection (measure § 7):** a width probe is not a damage signal.
//! Mutable buffer access bypasses change detection on BOTH the `TextBuffer`
//! and `TextEditState` members, so the measure/commit/sync writes never tick
//! `Changed<TextBuffer>` / `Changed<TextEditState>` — damage keys on the
//! commit OUTPUT components (`ComputedTextLayout`), the existing contract.
//! The editor-arm bypass is guarded DIRECTLY by
//! `tests/text_edit_substrate.rs`'s
//! `with_buffer_mut_bypasses_change_detection_on_the_editor_arm` (a
//! `Ref::is_changed()` probe across a `clear_trackers()` baseline). The
//! steady-frame parity test cannot reach it: nothing in the crate reads
//! `Changed<TextEditState>` in E1, and on a no-change frame Taffy's layout
//! cache keeps the measure closure (the only accessor caller) from running —
//! so the editor-arm bypass is also defense-in-depth for the first
//! `Changed<TextEditState>` reader a later phase adds.

use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::query::QueryData;
use cosmic_text::{Buffer, Edit};

use super::state::TextEditState;
use crate::text::{IntrinsicWidths, TextBuffer};

/// The shared buffer accessor (measure-and-layout § 2.3). `#[query_data(
/// mutable)]` generates the read-only companion (`TextBufferAccessReadOnly`)
/// automatically — extract binds that form.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct TextBufferAccess {
    /// The display-only buffer — authoritative iff `edit` is `None`.
    display: &'static mut TextBuffer,
    /// The editor — authoritative when present (§ 2.2a).
    edit: Option<&'static mut TextEditState>,
}

// NOTE: in Bevy 0.18.1 `#[derive(QueryData)]` generates the item struct
// with TWO lifetimes — `Item<'__w, '__s>` (world + state)
// (`bevy_ecs_macros-0.18.1/src/query_data.rs:72-81,309`). So the item type
// is `TextBufferAccessItem<'_, '_>` everywhere it is NAMED (the `impl` line
// here, the `SyncedTextItem` member in Step 3.1). Method params like
// `&mut TextBufferAccessItem` elide fine.
impl TextBufferAccessItem<'_, '_> {
    /// Read the authoritative buffer (editor-owned if present, else the
    /// display buffer). `&self`: read-only, no tick.
    pub fn with_buffer<T>(&self, f: impl FnOnce(&Buffer) -> T) -> T {
        match self.edit.as_ref() {
            Some(state) => state.editor.with_buffer(f),
            None => f(&self.display.buffer),
        }
    }

    /// Mutate the authoritative buffer. Bypasses change detection on
    /// whichever side is authoritative (measure § 7).
    pub fn with_buffer_mut<T>(&mut self, f: impl FnOnce(&mut Buffer) -> T) -> T {
        match self.edit.as_mut() {
            Some(state) => {
                let state = state.bypass_change_detection();
                state.editor.with_buffer_mut(f)
            }
            None => {
                let display = self.display.bypass_change_detection();
                f(&mut display.buffer)
            }
        }
    }

    /// The cached intrinsics for the authoritative buffer (decision 3 — the
    /// cache lives with the buffer it describes).
    pub fn intrinsics(&self) -> Option<IntrinsicWidths> {
        match self.edit.as_ref() {
            Some(state) => state.intrinsics,
            None => self.display.intrinsics(),
        }
    }

    /// Fill the authoritative cache (the measure closure is the only
    /// writer). Bypasses change detection (a probe is not damage).
    pub fn cache_intrinsics(&mut self, widths: IntrinsicWidths) {
        match self.edit.as_mut() {
            Some(state) => state.bypass_change_detection().intrinsics = Some(widths),
            None => self
                .display
                .bypass_change_detection()
                .cache_intrinsics(widths),
        }
    }

    /// Invalidate the authoritative cache (every content change — `TextSync`).
    pub fn invalidate_intrinsics(&mut self) {
        match self.edit.as_mut() {
            Some(state) => state.bypass_change_detection().intrinsics = None,
            None => self
                .display
                .bypass_change_detection()
                .invalidate_intrinsics(),
        }
    }
}

impl TextBufferAccessReadOnlyItem<'_, '_> {
    /// Read the authoritative buffer (the extract producer's form). The
    /// editor's `Edit::with_buffer` is `&self`, so this stays read-only —
    /// the `Extract` main-world read-only contract (architecture § 4.4).
    pub fn with_buffer<T>(&self, f: impl FnOnce(&Buffer) -> T) -> T {
        match self.edit.as_ref() {
            Some(state) => state.editor.with_buffer(f),
            None => f(&self.display.buffer),
        }
    }
}
