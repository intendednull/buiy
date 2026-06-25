//! The load-bearing RED proof (C7 §6): a synthetic pointer over the
//! VISUALLY-CORRECT absolute position of an offset widget must hit that
//! widget. On current main (pre-C1) picking reads parent-local
//! `ResolvedLayout.position` as absolute, so the hit lands on the wrong
//! entity or no entity — this test is RED until C1 routes picking through
//! `GlobalTransform`. It is what makes Tier A a real gate, not a
//! green-by-construction rubber stamp (the existing picking_backend.rs
//! hand-writes ResolvedLayout and is structurally blind to this bug).

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_verify::pointer::PointerHarness;

// C1 LANDED (2026-06-22): picking now reads the absolute position via the
// non-optional GlobalTransform, so the synthetic pointer over the offset
// widget's global center hits the target. The committed RED #[ignore] (above)
// → this un-ignore is C1's coordinate-fix verification.
#[test]
fn synthetic_pointer_hits_offset_widget_at_its_global_position() {
    let mut h = PointerHarness::new();
    // The harness wraps `scene` under a root positioned at the EXPLICIT
    // `offset`, so the target sits at a NON-ORIGIN absolute position: its
    // ResolvedLayout.position is parent-local (small), its
    // GlobalTransform.translation is the accumulated absolute. Bug 1 only
    // diverges when these differ — `offset` forces that divergence.
    let target = h.spawn_offset_tree(
        Vec2::new(80.0, 60.0),
        (
            Node,
            Style::default().width_px(100.0).height_px(50.0),
            Name::new("target"),
        ),
    );

    // Read the target's absolute (global) center the layout chain produced,
    // and aim the synthetic pointer at it in window space.
    let center = h.global_center(target);
    h.move_to(center);

    let hit = h.top_hit();
    assert_eq!(
        hit,
        Some(target),
        "the synthetic pointer over the target's GLOBAL center must hit the \
         target; on pre-C1 main the backend mis-reads parent-local position \
         as absolute and this FAILS (the C1 regression gate)"
    );
}
