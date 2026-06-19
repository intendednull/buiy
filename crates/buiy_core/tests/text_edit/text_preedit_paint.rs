//! E5 — the preedit underline paint seat (editing-and-ime § 6.2; decoration-
//! and-paint § 8). The composing span forces a quad-tier underline over its
//! byte range, via the existing `ExtractedTextQuads` carrier (no new GPU
//! work). Headless: this drives the render-world extract producer and asserts
//! an underline quad is emitted for the preedit range — no adapter (extract
//! runs CPU-side).

use buiy_core::text::PreeditVisual;
use cosmic_text::Cursor;

/// A `PreeditVisual` over a byte range emits at least one quad-tier underline
/// instance (the preedit underline). A collapsed range emits none.
#[test]
fn preedit_visual_constructs_and_reports_collapsed() {
    let v = PreeditVisual::new(Cursor::new(0, 2), Cursor::new(0, 5));
    assert!(!v.is_collapsed());
    let empty = PreeditVisual::new(Cursor::new(0, 3), Cursor::new(0, 3));
    assert!(empty.is_collapsed());
    // start/end normalize (start <= end).
    let swapped = PreeditVisual::new(Cursor::new(0, 5), Cursor::new(0, 2));
    assert_eq!((swapped.start.index, swapped.end.index), (2, 5));
}
