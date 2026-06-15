//! `TextSelection` — the Buiy-owned, multi-range-SHAPED selection type
//! (editing-and-ime § 4.2). v1 ships single-range *behavior* (`secondary`
//! always empty), but the public type, the `SelectionChanged` payload, and the
//! geometry pipeline are multi-range-shaped so the multi-cursor next slice
//! (§ 13) is additive, not a reshape. This type is a PROJECTION of the editor's
//! single `cosmic_text::Selection` (the editor owns BiDi-correct motion); the
//! input systems drive the editor, and `write_caret_and_selection` mirrors the
//! editor OUT into this type each frame the editor changed (architecture note,
//! E3 plan § Architecture "mirror direction"). It names ONE cosmic type —
//! `Cursor`, pure-data — so the facade-boundary tripwire (`Editor`/`Edit`/
//! `Action`/`Change`) does not flag it.

use cosmic_text::Cursor;
use smallvec::SmallVec;

/// One contiguous selection range: a held `anchor` and a moving `active`
/// endpoint (the caret end). A collapsed range (`anchor == active`) IS a caret.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SelectionRange {
    /// The fixed end (where the drag/extend started).
    pub anchor: Cursor,
    /// The moving end (the live caret).
    pub active: Cursor,
}

impl SelectionRange {
    /// The endpoints in document order (`lo ≤ hi`, `(line, index)`
    /// lexicographic — the `selection_bounds()` ordering). Direction-agnostic;
    /// geometry sweeps use this, the caret uses `active`.
    pub fn ordered(&self) -> (Cursor, Cursor) {
        if (self.active.line, self.active.index) < (self.anchor.line, self.anchor.index) {
            (self.active, self.anchor)
        } else {
            (self.anchor, self.active)
        }
    }

    /// `anchor == active` (position-wise): paints nothing; the caret is `active`.
    pub fn is_collapsed(&self) -> bool {
        (self.anchor.line, self.anchor.index) == (self.active.line, self.active.index)
    }
}

/// The full selection: a `primary` range plus `secondary` ranges for the
/// multi-cursor next slice. **v1: `secondary` is always empty.**
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextSelection {
    pub primary: SelectionRange,
    pub secondary: SmallVec<[SelectionRange; 2]>,
}

impl TextSelection {
    /// A collapsed selection (a bare caret) at `caret`.
    pub fn collapsed(caret: Cursor) -> Self {
        Self {
            primary: SelectionRange {
                anchor: caret,
                active: caret,
            },
            secondary: SmallVec::new(),
        }
    }

    /// Build from the editor's `selection_bounds()` ordered pair plus the live
    /// `active` cursor (so the anchor — the OTHER bound — and the moving end are
    /// distinguished for direction-aware undo/extend later).
    ///
    /// For a `Selection::Normal` selection `active` IS one of `lo`/`hi` (cosmic's
    /// `selection_bounds()` returns `(cursor, select)` / `(select, cursor)`), and
    /// the anchor is the other bound — direction is preserved. But a
    /// `Selection::Word`/`Line` selection sets `cursor` to the interior click
    /// position and `selection_bounds()` EXPANDS `lo`/`hi` out to the word/line
    /// boundaries (cosmic-text 0.19 `edit/mod.rs:235-280`,
    /// `edit/editor.rs:784-811`), so the live `cursor` is interior to `(lo, hi)`
    /// and equal to NEITHER bound. In that case there is no meaningful per-bound
    /// direction to recover, so we anchor at `lo` and put `active` at `hi`,
    /// preserving the FULL ordered span (`ordered() == (lo, hi)`) — dropping no
    /// part of the selected range. The geometry writer paints from
    /// `ordered()`, so this is what keeps a double-click word highlight whole.
    pub fn from_bounds(lo: Cursor, hi: Cursor, active: Cursor) -> Self {
        let at = |c: &Cursor| (c.line, c.index);
        let (anchor, active) = if at(&active) == at(&hi) {
            (lo, active)
        } else if at(&active) == at(&lo) {
            (hi, active)
        } else {
            // `active` matches neither bound (Word/Line selection): the cursor
            // is interior to the expanded span. Preserve the whole range.
            (lo, hi)
        };
        Self {
            primary: SelectionRange { anchor, active },
            secondary: SmallVec::new(),
        }
    }

    /// The whole selection collapses to a caret (no painted range anywhere).
    pub fn is_collapsed(&self) -> bool {
        self.primary.is_collapsed() && self.secondary.is_empty()
    }
}
