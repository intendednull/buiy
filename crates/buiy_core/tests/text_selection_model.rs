//! E3 — the Buiy-owned `TextSelection` type (editing-and-ime § 4.2): the
//! multi-range-SHAPED public selection (v1 single-range behavior — `secondary`
//! always empty), built from the editor's `selection_bounds()` shape, and a
//! collapsed-selection caret-position. Pure-data headless tests.

use buiy_core::text::edit::{SelectionRange, TextSelection};
use cosmic_text::Cursor;

#[test]
fn selection_range_orders_anchor_and_active() {
    // active < anchor (a backward drag): ordered() yields (active, anchor).
    let r = SelectionRange {
        anchor: Cursor::new(0, 8),
        active: Cursor::new(0, 3),
    };
    let (lo, hi) = r.ordered();
    assert_eq!((lo.line, lo.index), (0, 3));
    assert_eq!((hi.line, hi.index), (0, 8));
    assert!(!r.is_collapsed());

    // A forward drag orders the other way.
    let f = SelectionRange {
        anchor: Cursor::new(0, 3),
        active: Cursor::new(1, 1),
    };
    let (lo, hi) = f.ordered();
    assert_eq!((lo.line, lo.index, hi.line, hi.index), (0, 3, 1, 1));
}

#[test]
fn collapsed_range_is_a_caret() {
    let c = SelectionRange {
        anchor: Cursor::new(2, 5),
        active: Cursor::new(2, 5),
    };
    assert!(c.is_collapsed());
    let (lo, hi) = c.ordered();
    assert_eq!((lo.line, lo.index), (hi.line, hi.index));
}

#[test]
fn text_selection_v1_is_single_range() {
    let sel = TextSelection::collapsed(Cursor::new(0, 4));
    assert!(sel.primary.is_collapsed());
    assert!(
        sel.secondary.is_empty(),
        "v1 behavior: secondary always empty"
    );
    assert!(sel.is_collapsed());

    let ranged =
        TextSelection::from_bounds(Cursor::new(0, 1), Cursor::new(0, 6), Cursor::new(0, 6));
    assert!(!ranged.is_collapsed());
    assert!(ranged.secondary.is_empty());
    // active is the moving endpoint (here the end); anchor the held one.
    assert_eq!(
        (ranged.primary.active.line, ranged.primary.active.index),
        (0, 6)
    );
    assert_eq!(
        (ranged.primary.anchor.line, ranged.primary.anchor.index),
        (0, 1)
    );
}

/// A `Selection::Word`/`Line` selection puts the live cursor INTERIOR to the
/// expanded `(lo, hi)` bounds (cosmic sets `cursor = raw_hit` then expands the
/// bounds out to the word/line boundary — `edit/editor.rs:784-811` +
/// `edit/mod.rs:235-280`), so `from_bounds`'s `active` equals neither bound.
/// The constructed range must still round-trip to the FULL ordered span — if it
/// anchored on the interior cursor it would drop the `[lo, cursor)` portion and
/// the geometry writer (which paints from `ordered()`) would truncate the
/// highlight.
#[test]
fn word_selection_interior_cursor_round_trips_to_full_span() {
    // Double-click in the middle of "hello" (index 2): bounds expand to
    // (0,0)..(0,5), but the live cursor is the interior hit (0,2).
    let lo = Cursor::new(0, 0);
    let hi = Cursor::new(0, 5);
    let interior_active = Cursor::new(0, 2);

    let sel = TextSelection::from_bounds(lo, hi, interior_active);
    let (got_lo, got_hi) = sel.primary.ordered();
    assert_eq!(
        (got_lo.line, got_lo.index, got_hi.line, got_hi.index),
        (0, 0, 0, 5),
        "an interior cursor must not truncate the painted span"
    );

    // A multi-line Line-selection with an interior cursor likewise spans whole.
    let sel = TextSelection::from_bounds(Cursor::new(1, 0), Cursor::new(3, 7), Cursor::new(2, 4));
    let (got_lo, got_hi) = sel.primary.ordered();
    assert_eq!(
        (got_lo.line, got_lo.index, got_hi.line, got_hi.index),
        (1, 0, 3, 7)
    );
}
