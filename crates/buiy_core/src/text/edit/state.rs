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

use std::time::Duration;

use bevy::prelude::*;
use cosmic_text::{Buffer, Editor, Metrics};

use super::selection::TextSelection;
use crate::text::IntrinsicWidths;

/// The per-entity caret-blink phase origin (editing-and-ime §§ 5, 10). The T7
/// `write_caret_blink` writer was deliberately GLOBAL + stateless — a square
/// wave of the raw app clock (visual.rs module doc: "per-entity phase reset on
/// edit/caret-move is the editing campaign's `CaretBlink` state"). E3 lands that
/// state: the blink is phase-relative to `origin`, which is RESET to the current
/// app-clock instant on every edit and caret move, so the caret is always
/// solid-on for one half-period immediately after the user acts (web parity).
/// Reduced-motion steadiness is the writer's concern, not this type's.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CaretBlink {
    /// App-clock instant (`Time::elapsed()`) of the last reset. The blink phase
    /// is `now − origin`. Default `ZERO` = "blink from the start of time" (a
    /// caret that has never moved blinks on the global phase, harmless).
    pub origin: Duration,
}

impl CaretBlink {
    /// Stamp `now` as the new phase origin (call on edit / caret move).
    pub fn reset(&mut self, now: Duration) {
        self.origin = now;
    }

    /// Elapsed time since the origin, saturating (a paused/rewound test clock
    /// where `now < origin` yields `ZERO`, never an underflow panic).
    pub fn phase_elapsed(&self, now: Duration) -> Duration {
        now.saturating_sub(self.origin)
    }
}

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
    /// The Buiy-owned mirror of the editor's primary selection (§ 4.2). A
    /// PROJECTION: recomputed from the editor by `write_caret_and_selection`
    /// each frame the editor changed; never a second source of truth. Read by
    /// the geometry writer + the `SelectionChanged` emitter.
    pub(crate) selection: TextSelection,
    /// The per-entity caret-blink phase origin (§§ 5, 10). Reset on edit /
    /// caret move by `write_caret_and_selection`; read by `write_caret_blink`.
    pub(crate) blink: CaretBlink,
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
            selection: TextSelection::collapsed(cosmic_text::Cursor::new(0, 0)),
            blink: CaretBlink::default(),
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

    /// The logical value: the editor buffer's full text. Pre-IME this is the
    /// complete buffer content (editing-and-ime § 3); E5 refines this to
    /// subtract the live preedit byte range (invariant § 6.2b). Lines are
    /// joined with `\n` (the LF normalization the value contract uses; the
    /// editor stores per-line endings separately, `BufferLine::ending`).
    pub fn value(&self) -> String {
        use cosmic_text::Edit;
        self.editor.with_buffer(|buffer| {
            let mut out = String::new();
            for (i, line) in buffer.lines.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                out.push_str(line.text());
            }
            out
        })
    }

    /// Project the editor's current `Selection` + cursor into a Buiy
    /// `TextSelection` (§ 4.2). The editor owns BiDi-correct motion; this is the
    /// READ-OUT mirror. A `selection_bounds()` of `None` (no selection) is a
    /// collapsed caret at the live cursor.
    pub fn mirror_selection(&self) -> TextSelection {
        use cosmic_text::Edit;
        let active = self.editor.cursor();
        match self.editor.selection_bounds() {
            Some((lo, hi)) if (lo.line, lo.index) != (hi.line, hi.index) => {
                TextSelection::from_bounds(lo, hi, active)
            }
            _ => TextSelection::collapsed(active),
        }
    }

    /// The live caret (the editor's cursor) — the `active` endpoint.
    pub fn caret(&self) -> cosmic_text::Cursor {
        use cosmic_text::Edit;
        self.editor.cursor()
    }

    /// The per-entity caret-blink phase origin (the `write_caret_blink` reader).
    pub fn blink_origin(&self) -> std::time::Duration {
        self.blink.origin
    }

    /// Drop the cached intrinsic widths — the "buffer content changed,
    /// re-measure me" half of the dirty-mark seam (M1). The input system
    /// calls this on a reshaping edit, exactly as `sync_one` calls the
    /// accessor's `invalidate_intrinsics` after a `Text` change
    /// (`sync.rs:330`). Mutating `self.intrinsics` directly (not through the
    /// accessor) is correct here: the system already holds `&mut
    /// TextEditState` to apply the edit, and the `Changed<TextEditState>`
    /// tick the apply incurs is harmless (nothing keys off it; the edit IS a
    /// change). The other half — Taffy node dirtiness — is the system's
    /// `mark_dirty_for_entity` call (Task 5), because the node lives in
    /// `LayoutTree`, not on the component.
    pub fn invalidate_intrinsics(&mut self) {
        self.intrinsics = None;
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
