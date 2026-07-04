//! The headless **stroke** driver on the [`PointerHarness`]. These are the first
//! tests to drive `bevy_picking`'s DRAG machine headlessly: every other harness
//! method writes `PointerLocation` directly and emits NO `Move` action, so it
//! never trips drag derivation. [`PointerHarness::stroke`] presses primary, then
//! writes a `PointerAction::Move` per path point, and `pointer_events` derives
//! `DragStart` → `Drag` → `DragEnd` on the PRESS target, which these assert on.
//!
//! The property this locks in: the Press → N×`Move{delta}` → Release recipe drives
//! real `Pointer<Drag>` events with correct per-move deltas and start→end distance,
//! entirely headless (no window, no GPU) — so a headless test or multi-agent
//! playtest can drive a freehand drawing surface through the production pointer
//! path.

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::{Node, layout::Style};
use buiy_verify::pointer::PointerHarness;

/// A plain pickable Buiy node (nodes are pickable by default — no `Pickable`
/// needed), standing in for a drawing-canvas rect.
fn draggable(w: f32, h: f32) -> impl Bundle {
    (
        Node,
        Style::default().width_px(w).height_px(h),
        Name::new("canvas"),
    )
}

fn approx(a: Vec2, b: Vec2, eps: f32) -> bool {
    (a - b).length() <= eps
}

/// The headline proof: a straight `stroke` across a node emits exactly one
/// `DragStart`, one `Pointer<Drag>` per `Move` step (each with the correct delta),
/// and one `DragEnd` whose distance is the full start→end vector — all on the PRESS
/// target, entirely headless.
#[test]
fn stroke_emits_pointer_drag_with_correct_deltas_and_distance() {
    let mut h = PointerHarness::new();
    let node = h.spawn_offset_tree(Vec2::new(40.0, 30.0), draggable(200.0, 120.0));
    let center = h.global_center(node);
    let from = center - Vec2::new(80.0, 0.0);
    let to = center + Vec2::new(80.0, 0.0);

    // 4 equal steps → 4 Moves of (40, 0) each.
    h.drag(from, to, 4);

    // Exactly one DragStart, on the press target, at the press location.
    let starts: Vec<_> = h.captured_drag().of(node, "dragstart").collect();
    assert_eq!(
        starts.len(),
        1,
        "one DragStart on the pressed node (subsequent Moves don't re-start)"
    );
    assert!(
        approx(starts[0].position, from, 0.5),
        "DragStart fires at the press location {from}, got {}",
        starts[0].position
    );

    // One Drag per Move, each with delta ≈ (40, 0), the last landing at `to`.
    let drags: Vec<_> = h.captured_drag().of(node, "drag").collect();
    assert_eq!(
        drags.len(),
        4,
        "one Pointer<Drag> per Move step (got {drags:?})"
    );
    for s in &drags {
        assert!(
            approx(s.delta, Vec2::new(40.0, 0.0), 0.01),
            "each Drag delta ≈ (40, 0), got {}",
            s.delta
        );
    }
    assert!(
        approx(drags.last().unwrap().position, to, 0.5),
        "the final Drag position is the stroke end {to}, got {}",
        drags.last().unwrap().position
    );

    // Exactly one DragEnd, distance = full start→end vector.
    let ends: Vec<_> = h.captured_drag().of(node, "dragend").collect();
    assert_eq!(ends.len(), 1, "one DragEnd on release");
    assert!(
        approx(ends[0].delta, Vec2::new(160.0, 0.0), 0.5),
        "DragEnd distance is start→end (160, 0), got {}",
        ends[0].delta
    );

    // The phase log records the drag too (parity with the other Pointer<E>).
    assert!(
        h.captured().saw(node, "drag"),
        "the phase log records the Drag"
    );
}

/// The `stroke` vs `move_to` distinction, made explicit (the recipe's load-bearing
/// property): a press, then a `move_to` to a new point (a DIRECT location write, no
/// `Move` action), then a release produces NO `Pointer<Drag>` — only the
/// `Move`-action path drives the drag machine. This is why a real stroke helper was
/// needed and a sequence of `move_to`s would silently draw nothing.
#[test]
fn move_to_between_press_and_release_does_not_drag() {
    let mut h = PointerHarness::new();
    let node = h.spawn_offset_tree(Vec2::new(40.0, 30.0), draggable(200.0, 120.0));
    let center = h.global_center(node);

    h.move_to(center - Vec2::new(60.0, 0.0));
    h.press(PointerButton::Primary);
    // A direct-location move (emits no PointerAction::Move) — the drag machine
    // never sees a move, so no Drag is derived.
    h.move_to(center + Vec2::new(60.0, 0.0));
    h.release(PointerButton::Primary);

    assert!(
        h.captured_drag().of(node, "drag").next().is_none(),
        "move_to writes PointerLocation directly and emits no Move, so it never drags"
    );
    assert!(
        !h.captured().saw(node, "dragstart"),
        "no DragStart either — the drag machine requires a Move action"
    );
}

/// A multi-segment `stroke` (an arbitrary polyline, not just a straight drag)
/// derives one `Drag` per non-degenerate segment on the press target — the shape a
/// freehand brush produces. A repeated point (zero delta) is skipped.
#[test]
fn polyline_stroke_drags_once_per_nonzero_segment() {
    let mut h = PointerHarness::new();
    let node = h.spawn_offset_tree(Vec2::new(20.0, 20.0), draggable(300.0, 300.0));
    let c = h.global_center(node);
    // 3 distinct moves + 1 repeated point (the repeat is a zero-delta no-op).
    let path = [
        c + Vec2::new(-60.0, -60.0),
        c + Vec2::new(0.0, -60.0),
        c + Vec2::new(0.0, -60.0), // repeat — skipped
        c + Vec2::new(60.0, 0.0),
        c + Vec2::new(60.0, 60.0),
    ];
    h.stroke(&path);

    let drags = h.captured_drag().of(node, "drag").count();
    assert_eq!(
        drags, 3,
        "3 non-degenerate segments → 3 Drags; the zero-delta repeat is skipped"
    );
    assert_eq!(h.captured_drag().of(node, "dragstart").count(), 1);
    assert_eq!(h.captured_drag().of(node, "dragend").count(), 1);
}

/// A `path` shorter than two points is a no-op — nothing to drag, no press, no
/// events. Guards the boundary the [`PointerHarness::stroke`] doc promises.
#[test]
fn degenerate_stroke_is_a_noop() {
    let mut h = PointerHarness::new();
    let node = h.spawn_offset_tree(Vec2::new(10.0, 10.0), draggable(120.0, 120.0));
    let c = h.global_center(node);

    h.stroke(&[]);
    h.stroke(&[c]); // single point — no segment

    assert_eq!(h.captured_drag().of(node, "dragstart").count(), 0);
    assert_eq!(h.captured_drag().of(node, "drag").count(), 0);
    assert_eq!(h.captured_drag().of(node, "dragend").count(), 0);
}
