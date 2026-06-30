//! **Command-sourcing** the editor (spec §6): the
//! Buiy-owned `Reflect` mirror of the editor's *resolved* input vocabulary, the
//! recordable [`EditLog`], and the [`TextEditState::apply_recorded`] replay
//! fold.
//!
//! Design + rationale: `docs/specs/2026-06-29-mvu-as-core-design.md` (§6 editor
//! command-sourcing — the editor is the PureEnv-exempt routing leaf).
//!
//! ## The crux this file proves
//! `TextEditState` (`state.rs`) wraps `cosmic_text::Editor<'static>` and is
//! `#[derive(Component)]`-only — deliberately **not** `Reflect` (it wraps a
//! foreign engine + foreign `Change`s). The "MVU-as-core ⇒ whole-UI replay"
//! thesis dies here UNLESS editor state can be captured + replayed **without**
//! reflecting the cosmic `Editor`. The answer — *command-sourcing*: the editor
//! is already a de-facto reducer (one verb vocabulary [`EditCommand`], lowered
//! at one site `apply_tracked`). **Record the resolved command/IME stream;
//! replay = re-fold the stream into a FRESH editor from the same seed +
//! the same `FontSystem`. The cosmic `Editor` never serializes.**
//!
//! The editor is the documented **`PureEnv` exemption** (its fold needs
//! `&mut FontSystem` and reads the OS clipboard — it is NOT a pure `Model`):
//! determinism is guaranteed at the *boundary* (same `FontSystem` + seed ⇒ same
//! fold), not by purity. This file adds a **record tap** to the existing
//! imperative editor; it does NOT convert the editor into a reducer.
//!
//! ## Why a Buiy-owned mirror (not record `EditCommand` directly)
//! [`EditCommand`] names `cosmic_text::Motion` (foreign, not `Reflect`). The
//! recorded message must round-trip through `Reflect` so the log persists
//! cross-process (replay in a fresh process). So we mirror the *resolved*
//! vocabulary in Buiy-owned `Reflect` types ([`MotionMirror`], [`RecordedEdit`])
//! with lossless `From`/`to` conversions, and record those.

use std::time::Duration;

use bevy::prelude::*;
use cosmic_text::{FontSystem, Motion};

use crate::mvu::LogicalId;

use super::clipboard::{ClipboardProvider, MemClipboard};
use super::command::EditCommand;
use super::input::{EditContext, EditOutcome};
use super::state::TextEditState;

// ---------------------------------------------------------------------------
// The `Motion` mirror — a Buiy-owned, `Reflect` copy of cosmic's pure-data
// cursor-movement enum (the one foreign type `EditCommand` names).
// ---------------------------------------------------------------------------

/// A `Reflect` mirror of `cosmic_text::LayoutCursor` (the one data-carrying
/// `Motion` variant's payload). Pure data — three indices.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub struct LayoutCursorMirror {
    pub line: usize,
    pub layout: usize,
    pub glyph: usize,
}

/// A Buiy-owned, `Reflect + Clone + PartialEq` mirror of `cosmic_text::Motion`
/// — the cursor-movement verb [`EditCommand::Motion`] carries. **Lossless** over
/// the WHOLE cosmic vocabulary (all 22 variants), not only the ~12 the keymap
/// emits today: a complete mirror is the same cost as a partial one and keeps
/// the record log honest if the keymap grows. The `From` pair below is the
/// round-trip seam.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MotionMirror {
    LayoutCursor(LayoutCursorMirror),
    Previous,
    Next,
    Left,
    Right,
    Up,
    Down,
    Home,
    SoftHome,
    End,
    ParagraphStart,
    ParagraphEnd,
    PageUp,
    PageDown,
    Vertical(i32),
    PreviousWord,
    NextWord,
    LeftWord,
    RightWord,
    BufferStart,
    BufferEnd,
    GotoLine(usize),
}

impl From<Motion> for MotionMirror {
    fn from(m: Motion) -> Self {
        match m {
            Motion::LayoutCursor(c) => MotionMirror::LayoutCursor(LayoutCursorMirror {
                line: c.line,
                layout: c.layout,
                glyph: c.glyph,
            }),
            Motion::Previous => MotionMirror::Previous,
            Motion::Next => MotionMirror::Next,
            Motion::Left => MotionMirror::Left,
            Motion::Right => MotionMirror::Right,
            Motion::Up => MotionMirror::Up,
            Motion::Down => MotionMirror::Down,
            Motion::Home => MotionMirror::Home,
            Motion::SoftHome => MotionMirror::SoftHome,
            Motion::End => MotionMirror::End,
            Motion::ParagraphStart => MotionMirror::ParagraphStart,
            Motion::ParagraphEnd => MotionMirror::ParagraphEnd,
            Motion::PageUp => MotionMirror::PageUp,
            Motion::PageDown => MotionMirror::PageDown,
            Motion::Vertical(px) => MotionMirror::Vertical(px),
            Motion::PreviousWord => MotionMirror::PreviousWord,
            Motion::NextWord => MotionMirror::NextWord,
            Motion::LeftWord => MotionMirror::LeftWord,
            Motion::RightWord => MotionMirror::RightWord,
            Motion::BufferStart => MotionMirror::BufferStart,
            Motion::BufferEnd => MotionMirror::BufferEnd,
            Motion::GotoLine(n) => MotionMirror::GotoLine(n),
        }
    }
}

impl From<MotionMirror> for Motion {
    fn from(m: MotionMirror) -> Self {
        match m {
            MotionMirror::LayoutCursor(c) => Motion::LayoutCursor(cosmic_text::LayoutCursor {
                line: c.line,
                layout: c.layout,
                glyph: c.glyph,
            }),
            MotionMirror::Previous => Motion::Previous,
            MotionMirror::Next => Motion::Next,
            MotionMirror::Left => Motion::Left,
            MotionMirror::Right => Motion::Right,
            MotionMirror::Up => Motion::Up,
            MotionMirror::Down => Motion::Down,
            MotionMirror::Home => Motion::Home,
            MotionMirror::SoftHome => Motion::SoftHome,
            MotionMirror::End => Motion::End,
            MotionMirror::ParagraphStart => Motion::ParagraphStart,
            MotionMirror::ParagraphEnd => Motion::ParagraphEnd,
            MotionMirror::PageUp => Motion::PageUp,
            MotionMirror::PageDown => Motion::PageDown,
            MotionMirror::Vertical(px) => Motion::Vertical(px),
            MotionMirror::PreviousWord => Motion::PreviousWord,
            MotionMirror::NextWord => Motion::NextWord,
            MotionMirror::LeftWord => Motion::LeftWord,
            MotionMirror::RightWord => Motion::RightWord,
            MotionMirror::BufferStart => Motion::BufferStart,
            MotionMirror::BufferEnd => Motion::BufferEnd,
            MotionMirror::GotoLine(n) => Motion::GotoLine(n),
        }
    }
}

// ---------------------------------------------------------------------------
// The recorded edit vocabulary — the resolved keyboard/clipboard verbs PLUS the
// resolved IME sub-events, in ONE ordered stream (so replay re-folds them in
// the exact order the editor applied them).
// ---------------------------------------------------------------------------

/// The in-preedit cursor an `Ime::Preedit` carries — a `(begin, end)` byte range
/// INTO the preedit string. A named `Reflect` struct (not a bare `(usize, usize)`
/// tuple) so the record log round-trips through a `TypeRegistry` without needing
/// the tuple type registered.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecordedPreeditCursor {
    pub begin: usize,
    pub end: usize,
}

/// One **resolved** edit the editor applied — the unit of the record log. Mirrors
/// the keyboard/clipboard [`EditCommand`] vocabulary AND the IME sub-events
/// (`Ime::Preedit`/`Commit`/cancel), interleaved in application order.
///
/// **Self-contained replay (the impure-read hoist, spec §6).** Two command
/// classes read OS state *inside* the editor's fold, so the recorded form
/// carries the *resolved effect* instead, making replay independent of any OS:
/// - [`RecordedEdit::Paste`] carries the resolved clipboard **text** (the
///   editor's `Paste` reads the OS clipboard; replay re-feeds the recorded text
///   through a throwaway in-memory clipboard).
/// - The IME sub-events carry their resolved `value`/`cursor` (the OS IME
///   already hands these to us in the `Ime` event, so they are self-contained
///   for free).
///
/// `Cut`/`Copy` *write* the clipboard — an external sink that does not affect
/// the editor's own text/cursor/selection — so they need no resolved payload;
/// `Cut`'s deletion is deterministic from editor state and replays exactly.
#[derive(Reflect, Clone, PartialEq, Eq, Debug)]
pub enum RecordedEdit {
    /// Literal text insertion (the layout-resolved key `text`).
    Insert(String),
    Backspace,
    Delete,
    Enter,
    /// Cursor movement + whether it extends the selection (Shift held).
    Motion(MotionMirror, bool),
    SelectAll,
    Escape,
    /// Cut — deletes the selection (deterministic); the clipboard WRITE is a
    /// side effect outside the replay boundary.
    Cut,
    /// Copy — no editor-state change; the clipboard WRITE is outside the boundary.
    Copy,
    /// Paste, carrying the **resolved** clipboard text (the hoisted impure read).
    Paste(String),
    Undo,
    Redo,
    Submit,
    /// `Ime::Preedit` with a non-empty value — splice/replace the composition span.
    ImePreedit {
        value: String,
        cursor: Option<RecordedPreeditCursor>,
    },
    /// `Ime::Commit` — finalize the composition with the committed string.
    ImeCommit(String),
    /// An empty `Ime::Preedit` or `Ime::Disabled` mid-composition — cancel/remove
    /// the live preedit (reverse-applying any compose-over-selection delete).
    ImeCancel,
}

impl RecordedEdit {
    /// Build the recorded mirror of a resolved keyboard/clipboard [`EditCommand`].
    /// `resolve_paste` is invoked **only** for `Paste` (to capture the resolved
    /// clipboard text exactly once), so non-paste recording never reads the
    /// clipboard. This is the keyboard/clipboard half of the record tap.
    pub fn for_command(cmd: &EditCommand, resolve_paste: impl FnOnce() -> String) -> RecordedEdit {
        match cmd {
            EditCommand::Insert(s) => RecordedEdit::Insert(s.clone()),
            EditCommand::Backspace => RecordedEdit::Backspace,
            EditCommand::Delete => RecordedEdit::Delete,
            EditCommand::Enter => RecordedEdit::Enter,
            EditCommand::Motion(m, extend) => RecordedEdit::Motion((*m).into(), *extend),
            EditCommand::SelectAll => RecordedEdit::SelectAll,
            EditCommand::Escape => RecordedEdit::Escape,
            EditCommand::Cut => RecordedEdit::Cut,
            EditCommand::Copy => RecordedEdit::Copy,
            EditCommand::Paste => RecordedEdit::Paste(resolve_paste()),
            EditCommand::Undo => RecordedEdit::Undo,
            EditCommand::Redo => RecordedEdit::Redo,
            EditCommand::Submit => RecordedEdit::Submit,
        }
    }

    /// The keyboard/clipboard [`EditCommand`] this mirror lowers back to, or
    /// `None` for the IME sub-events (which the replay fold drives via the
    /// `splice`/`commit`/`remove` primitives, not `apply_tracked`).
    pub fn to_command(&self) -> Option<EditCommand> {
        Some(match self {
            RecordedEdit::Insert(s) => EditCommand::Insert(s.clone()),
            RecordedEdit::Backspace => EditCommand::Backspace,
            RecordedEdit::Delete => EditCommand::Delete,
            RecordedEdit::Enter => EditCommand::Enter,
            RecordedEdit::Motion(m, extend) => EditCommand::Motion((*m).into(), *extend),
            RecordedEdit::SelectAll => EditCommand::SelectAll,
            RecordedEdit::Escape => EditCommand::Escape,
            RecordedEdit::Cut => EditCommand::Cut,
            RecordedEdit::Copy => EditCommand::Copy,
            RecordedEdit::Paste(_) => EditCommand::Paste,
            RecordedEdit::Undo => EditCommand::Undo,
            RecordedEdit::Redo => EditCommand::Redo,
            RecordedEdit::Submit => EditCommand::Submit,
            RecordedEdit::ImePreedit { .. }
            | RecordedEdit::ImeCommit(_)
            | RecordedEdit::ImeCancel => {
                return None;
            }
        })
    }
}

// ---------------------------------------------------------------------------
// The recordable log — a parallel `EditLog` keyed by `LogicalId`.
// ---------------------------------------------------------------------------

/// One recorded fold: which editor (by stable [`LogicalId`]), in what order, the
/// resolved [`RecordedEdit`], and the virtual-clock instant it was applied at (so
/// replay reproduces undo coalescing too).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditLogEntry {
    pub lid: LogicalId,
    pub seq: u64,
    pub edit: RecordedEdit,
    pub now: Duration,
}

/// The append-only **editor-command** record log (the editor's parallel to
/// [`MsgLog`](crate::mvu::MsgLog)).
///
/// **A SEPARATE storage from [`MsgLog`](crate::mvu::MsgLog), but unified under one
/// switch + one sequence (spec §6/§7.2).** The editor is the documented
/// `PureEnv` exemption and is NOT a `Model`, so it has no `Envelope<M>`/`Msg` to put
/// in `MsgLog`; its stream is a single ordered [`RecordedEdit`] vocabulary interleaving
/// keyboard + IME. Storing the entries **typed** (not eagerly RON-serialized like
/// `MsgLog`) pays no per-keystroke serialize cost; the [`RecordedEdit`] `Reflect` derive
/// still lets the log persist cross-process when exported.
///
/// The unified record session ties the two logs together not by merging their storage
/// (their natural forms differ — typed `RecordedEdit` vs generic RON) but by sharing ONE
/// [`RecordSession`](crate::mvu::RecordSession): every entry's `seq` is the global one,
/// so [`crate::replay`] merges both logs into one totally-ordered whole-UI stream. The
/// gate is also shared — when the session is [`RecordMode::Off`](crate::mvu::RecordMode)
/// the tap's `tick_seq` returns `None` and this log stays untouched (production pays zero).
#[derive(Resource, Default)]
pub struct EditLog {
    pub entries: Vec<EditLogEntry>,
}

impl EditLog {
    /// Drop all recorded entries (test/checkpoint convenience).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Record one resolved edit for `lid`, stamped with the **global** `seq` (from
    /// [`RecordSession::tick_seq`](crate::mvu::RecordSession::tick_seq)). The tap only
    /// reaches here when recording is on.
    pub fn record(&mut self, seq: u64, lid: LogicalId, edit: RecordedEdit, now: Duration) {
        self.entries.push(EditLogEntry {
            lid,
            seq,
            edit,
            now,
        });
    }

    /// The recorded stream for one actor, in application order (the replay input).
    pub fn stream_for(&self, lid: LogicalId) -> impl Iterator<Item = &EditLogEntry> {
        self.entries.iter().filter(move |e| e.lid == lid)
    }
}

// ---------------------------------------------------------------------------
// The replay fold — re-apply one recorded edit to (a fresh seed of) the editor.
// ---------------------------------------------------------------------------

impl TextEditState {
    /// **The replay fold** (spec §6): re-apply ONE [`RecordedEdit`] to this editor,
    /// reconstructing the same state the original apply produced — *without* the
    /// cosmic `Editor` ever having been serialized. Folding the whole recorded
    /// stream into a FRESH editor seeded identically reconstructs the original
    /// editor's text + cursor + selection byte-for-byte (proven by the
    /// command-sourcing crux test).
    ///
    /// Keyboard/clipboard verbs lower back to [`EditCommand`] and route through
    /// the SAME `apply_tracked` fold the live input uses; the impure reads are
    /// re-fed from the recorded form (a throwaway [`MemClipboard`] seeded with
    /// the recorded `Paste` text — there is no OS clipboard on replay). IME
    /// sub-events drive the same `splice`/`commit`/`remove` primitives
    /// `apply_ime` does. Determinism holds at the boundary: same `font_system`
    /// + same seed ⇒ same fold.
    pub fn apply_recorded(
        &mut self,
        font_system: &mut FontSystem,
        rec: &RecordedEdit,
        single_line: bool,
        read_only: bool,
        now: Duration,
    ) -> EditOutcome {
        match rec {
            // IME sub-events: drive the same primitives `apply_ime` calls.
            RecordedEdit::ImePreedit { value, cursor } => {
                let cursor = cursor.map(|c| (c.begin, c.end));
                let value_changed = self.splice_preedit(font_system, value, cursor);
                EditOutcome {
                    value_changed,
                    submitted: false,
                    reshaped: true,
                }
            }
            RecordedEdit::ImeCommit(value) => {
                self.commit_preedit(font_system, value, now);
                EditOutcome {
                    value_changed: true,
                    submitted: false,
                    reshaped: true,
                }
            }
            RecordedEdit::ImeCancel => {
                let value_changed = self.remove_preedit(font_system);
                EditOutcome {
                    value_changed,
                    submitted: false,
                    reshaped: true,
                }
            }
            // Paste: re-feed the recorded text through a throwaway clipboard
            // (no OS clipboard on replay), then run the real `Paste` fold.
            RecordedEdit::Paste(text) => {
                let mut clip = MemClipboard::default();
                clip.set_text(text.clone());
                let mut ctx = EditContext {
                    single_line,
                    read_only,
                    now,
                    clipboard: &mut clip,
                };
                self.apply_tracked(font_system, EditCommand::Paste, &mut ctx)
            }
            // All other resolved verbs: lower back to `EditCommand`, run the
            // real fold (a throwaway clipboard absorbs any Cut/Copy WRITE).
            keyboard => {
                let command = keyboard
                    .to_command()
                    .expect("non-IME RecordedEdit lowers to an EditCommand");
                let mut clip = MemClipboard::default();
                let mut ctx = EditContext {
                    single_line,
                    read_only,
                    now,
                    clipboard: &mut clip,
                };
                self.apply_tracked(font_system, command, &mut ctx)
            }
        }
    }
}
