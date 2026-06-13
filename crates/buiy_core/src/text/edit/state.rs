//! `TextEditState` — the editor state machine over `cosmic_text::Editor`
//! (editing-and-ime § 2.1: wrap `Editor`, do not rebuild it), and the four
//! decomposed policy markers (§ 2.2). This module is INSIDE the
//! `text::edit` facade: it is one of the two files allowed to name a cosmic
//! `Editor`/`Edit` type (the other is `access.rs`); every other Buiy module
//! reaches the editor's buffer only through `TextBufferAccess`
//! (`tests/text_facade_boundary.rs` is the tripwire).
//!
//! **E1 field set (E1 plan decision 1):** only `editor` and `intrinsics`.
//! The spec § 2.2 sketch lists `selection`/`preedit`/`undo`/`blink` too, but
//! each is dead state until its phase reads it — E3 adds `selection` +
//! `blink`, E4 `undo`, E5 `preedit`, together with the system that consumes
//! it. No orphan placeholder fields.

use bevy::prelude::*;
use cosmic_text::{Buffer, Editor, Metrics};

use crate::text::IntrinsicWidths;

/// The editor state machine for an editable text entity (editing-and-ime
/// § 2.2). Optional on a text entity: entities with only a display
/// `TextBuffer` never pay for it (editor-optional / buffer-required — the
/// `TextBufferAccess` dispatch reaches whichever exists).
///
/// **Buffer ownership (§ 2.2a):** the editor wraps `BufferRef::Owned(Buffer)`
/// — the only `BufferRef` shape that allows mutation (`Borrowed`
/// self-borrows, which a component cannot do; `Arc` forbids mutation). When
/// `TextEditState` is present its owned buffer is **authoritative**: the
/// measure seam, `TextCommit`, the glyph producer, and `TextSync` all reach
/// it through `TextBufferAccess` (this campaign's `access.rs`), preferring it
/// over the display-only `TextBuffer.buffer`.
///
/// `Editor` is `Send + Sync` in 0.19 (verified — docs.rs auto-traits), so
/// this is a plain `Component`, no `NonSend` contortion. Machinery state —
/// NOT reflect-registered (it carries a `cosmic_text::Editor`, and this
/// module is the cosmic boundary; the `TextBuffer` precedent,
/// `components.rs`).
#[derive(Component)]
pub struct TextEditState {
    /// The wrapped editor over `BufferRef::Owned`. Private: the only way to
    /// reach its buffer from outside `text::edit` is `TextBufferAccess`.
    pub(crate) editor: Editor<'static>,
    /// Cached intrinsic widths for the AUTHORITATIVE (editor-owned) buffer
    /// (E1 plan decision 3 — moved off `TextBuffer` so the cache keys to the
    /// buffer it describes). `None` until measure computes them, and after
    /// every `TextSync` invalidation. Read/written only through
    /// `TextBufferAccess`'s cache methods.
    pub(crate) intrinsics: Option<IntrinsicWidths>,
}

impl TextEditState {
    /// A new editor over an empty, unshaped owned buffer at `metrics`.
    /// FontSystem-free: `Buffer::new_empty` takes no `FontSystem`, and
    /// `Editor::new` is pure struct construction (verified,
    /// `cosmic-text-0.19.0/src/edit/editor.rs:37`) — so construction is NOT
    /// a lock site (architecture § 1.2), mirroring `TextBuffer::new`.
    pub fn new(metrics: Metrics) -> Self {
        Self {
            editor: Editor::new(Buffer::new_empty(metrics)),
            intrinsics: None,
        }
    }

    /// Read the editor's owned buffer. Test/inspection convenience that
    /// stays INSIDE the facade (it lives in `text::edit`); production
    /// readers go through `TextBufferAccess`. Mirrors `Edit::with_buffer`.
    pub fn with_buffer<T>(&self, f: impl FnOnce(&Buffer) -> T) -> T {
        use cosmic_text::Edit;
        self.editor.with_buffer(f)
    }

    /// The cached intrinsics, if valid for the current content version.
    pub fn intrinsics(&self) -> Option<IntrinsicWidths> {
        self.intrinsics
    }
}

/// Marker: editable but not mutable — caret + selection + copy yes, mutation
/// no (editing-and-ime § 2.2). IME stays disabled on a `ReadOnly` editor.
/// Behavior is E2/E5/E6; E1 only lands the marker.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct ReadOnly;

/// Marker: no focus, no caret, no IME (editing-and-ime § 2.2). The strongest
/// suppression: editing systems gate on `not Disabled` (E2+). E1 lands the
/// marker.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct Disabled;

/// Marker: Enter ⇒ Submit, `Wrap::None`, newline-stripped paste
/// (editing-and-ime §§ 2.2, 3.3). Behavior is E2; E1 lands the marker.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct SingleLine;

/// The placeholder string, shown when the logical value is empty
/// (editing-and-ime § 10). Rendering is E6; E1 lands the carrier. The string
/// never enters the editor buffer — it is a display-only Buffer at paint.
#[derive(Component, Reflect, Default, Clone, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct Placeholder(pub String);
