//! The undo/redo engine (editing-and-ime § 8). A Buiy-owned two-stack model
//! over the verified cosmic `Change` substrate — `Change::reverse()` +
//! `Edit::apply_change()` are the exact replay pair (the `vi` reference,
//! `cosmic-text-0.19.0/src/edit/vi.rs:13-28`). This file NAMES
//! `cosmic_text::Change`, so it MUST stay inside the `text::edit` facade
//! (the boundary tripwire `tests/text_facade_boundary.rs`).
//!
//! Grouping (§ 8): an IME composition is ONE unit (Composition — shaped here,
//! emitted by E5); consecutive typing coalesces by time window + caret
//! adjacency into a TypingRun; consecutive same-direction deletes into a
//! DeleteRun; any motion/click/discrete command seals the open group. The
//! seam between "this is one unit" and "this extends the open run" is
//! `record` + `seal`; the application of changes to the editor is the
//! caller's job (input.rs), so this file names no `Editor`/`Edit`/`Action`.

use bevy::prelude::{Entity, Message};
use cosmic_text::{Change, Cursor};

use super::selection::TextSelection;

/// The default undo depth (spec § 8: "v1 default 1000 units").
pub const DEFAULT_UNDO_DEPTH: usize = 1000;

/// The typing/delete coalescing window (spec § 8 "by time window"). Edits this
/// close in (virtual) time, with an adjacent caret and the same group kind,
/// fold into one undo unit. 1s matches the common editor convention (the user
/// perceives a continuous typing burst as one undoable action).
pub const COALESCE_WINDOW: std::time::Duration = std::time::Duration::from_millis(1000);

/// How a unit groups with its neighbors when coalescing (§ 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    /// An IME composition — exactly one unit, never coalesces (§ 6.2c).
    /// Shaped in E4; emitted by E5.
    Composition,
    /// A run of inserted characters that coalesces by time + adjacency.
    TypingRun,
    /// A run of same-direction deletions that coalesces likewise.
    DeleteRun,
    /// A standalone unit (paste, cut, a single deliberate edit) — never
    /// coalesces with a neighbor.
    Discrete,
}

/// One undoable edit: the cosmic `Change` plus the caret + selection on
/// either side, so undo/redo restore the full cursor state, not just the text
/// (spec § 8). `selection_*` are captured via `mirror_selection()` at the
/// edit boundaries (the E3 mirror-direction invariant — never the stale
/// `state.selection` field).
#[derive(Debug, Clone)]
pub struct UndoUnit {
    pub change: Change,
    pub caret_before: Cursor,
    pub caret_after: Cursor,
    pub selection_before: TextSelection,
    pub selection_after: TextSelection,
    pub group: GroupKind,
}

/// The two-stack undo history, one per editor (a `TextEditState` field).
/// Depth-bounded: when `undo` exceeds `depth`, the OLDEST unit is dropped
/// (`Vec::remove(0)` — the history is short and bounded, so the O(n) shift is
/// irrelevant; correctness over micro-optimization).
#[derive(Debug)]
pub struct UndoStack {
    pub undo: Vec<UndoUnit>,
    pub redo: Vec<UndoUnit>,
    depth: usize,
    /// When the open coalescing run was last extended (virtual time). Only
    /// meaningful while `has_open_group()`. The grouping window compares the
    /// NEW edit's `now` against this.
    last_edit_at: std::time::Duration,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::with_depth(DEFAULT_UNDO_DEPTH)
    }
}

impl UndoStack {
    pub fn with_depth(depth: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            depth: depth.max(1),
            last_edit_at: std::time::Duration::ZERO,
        }
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// `true` while a coalescing run is open (the last unit is a TypingRun or
    /// DeleteRun). Used by the grouping logic (Task 3) and by `seal`.
    pub fn has_open_group(&self) -> bool {
        matches!(
            self.undo.last().map(|u| u.group),
            Some(GroupKind::TypingRun | GroupKind::DeleteRun)
        )
    }

    /// Record a new edit. Drops an empty `Change` (a no-op edit — Backspace at
    /// offset 0; `finish_change` returns `Some(Change{items: []})`,
    /// `editor.rs:512`). A new edit ALWAYS clears the redo stack (§ 8).
    /// Grouping/coalescing is applied by `record_grouped` (Task 3); the bare
    /// `record` pushes a standalone unit (and is what the grouping path calls
    /// when it decides NOT to coalesce).
    pub fn record(&mut self, unit: UndoUnit) {
        if unit.change.items.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(unit);
        self.enforce_depth();
    }

    /// Record an edit, coalescing it into the open run when eligible (§ 8).
    /// `now` is the current virtual-clock instant (deterministic in tests via
    /// `Time<Virtual>::advance_by`). Coalesces iff ALL hold:
    ///   - the open last unit has the SAME `group` (TypingRun or DeleteRun),
    ///   - `now - last_edit_at <= COALESCE_WINDOW`,
    ///   - the new edit's `caret_before` is adjacent to the run's `caret_after`
    ///     (continuing the same caret position — typed/deleted contiguously).
    ///
    /// Otherwise it starts a fresh unit (`record`). `Discrete`/`Composition`
    /// never coalesce. Empty changes are dropped by `record` (Backspace at 0).
    pub fn record_grouped(&mut self, unit: UndoUnit, now: std::time::Duration) {
        if unit.change.items.is_empty() {
            return;
        }
        let coalesces = matches!(unit.group, GroupKind::TypingRun | GroupKind::DeleteRun)
            && self.undo.last().is_some_and(|open| {
                open.group == unit.group
                    && now.saturating_sub(self.last_edit_at) <= COALESCE_WINDOW
                    && at(open.caret_after) == at(unit.caret_before)
            });

        if coalesces {
            let open = self.undo.last_mut().expect("checked by `coalesces`");
            // Extend: append the change items, carry the caret/selection AFTER
            // forward (BEFORE stays the run's original — undo restores to the
            // start of the whole burst).
            open.change.items.extend(unit.change.items);
            open.caret_after = unit.caret_after;
            open.selection_after = unit.selection_after;
            self.last_edit_at = now;
            // A coalesced edit is still a new edit ⇒ redo is stale.
            self.redo.clear();
        } else {
            self.last_edit_at = now;
            self.record(unit);
        }
    }

    /// Pop the most recent undo unit onto the redo stack and return it (so the
    /// caller can `apply_change(reverse)` + restore `_before`). `None` if
    /// there is nothing to undo.
    pub fn pop_undo(&mut self) -> Option<UndoUnit> {
        let unit = self.undo.pop()?;
        self.redo.push(unit.clone());
        Some(unit)
    }

    /// Pop the most recent redo unit back onto the undo stack and return it
    /// (so the caller can `apply_change(change)` + restore `_after`). `None`
    /// if there is nothing to redo.
    pub fn pop_redo(&mut self) -> Option<UndoUnit> {
        let unit = self.redo.pop()?;
        self.undo.push(unit.clone());
        Some(unit)
    }

    /// Seal any open coalescing run: the next `record_grouped` starts a fresh
    /// unit even if it would otherwise coalesce. Called on any motion / click
    /// / discrete command and on focus loss (E6). Re-tags the open run as
    /// `Discrete` so `has_open_group` goes false (the text is unchanged — only
    /// the coalescing eligibility ends).
    pub fn seal(&mut self) {
        if let Some(last) = self.undo.last_mut()
            && matches!(last.group, GroupKind::TypingRun | GroupKind::DeleteRun)
        {
            last.group = GroupKind::Discrete;
        }
    }

    fn enforce_depth(&mut self) {
        while self.undo.len() > self.depth {
            self.undo.remove(0);
        }
    }

    /// Test-only seam: seed a redo entry (so `record_clears_redo` can prove the
    /// clear). Not used in production — production redo entries only ever
    /// arrive via `pop_undo`.
    pub fn push_redo_for_test(&mut self, unit: UndoUnit) {
        self.redo.push(unit);
    }
}

/// Position key for caret comparison (line, byte index) — `Cursor` is not
/// `Ord`, so adjacency compares the positional pair.
fn at(c: Cursor) -> (usize, usize) {
    (c.line, c.index)
}

/// Emitted when an edit is undone (editing-and-ime § 11 row `EditUndone`).
/// Payload: the entity + the undone unit's `GroupKind`.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditUndone(pub Entity, pub GroupKind);

/// Emitted when an edit is redone (editing-and-ime § 11 row `EditRedone`).
/// Payload: the entity + the redone unit's `GroupKind`.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditRedone(pub Entity, pub GroupKind);
