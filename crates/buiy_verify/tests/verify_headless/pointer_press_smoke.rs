//! Smoke: a synthetic press at the pointer's current location does not
//! disturb the hit stream and is injected through the sanctioned path
//! (PointerInput written directly — §3.2). The DURABLE state-flip assert
//! (Checked after click) lands in Task 4 with C3/C4; this proves the
//! press/release injection seam works on the Wave-1 backend.
//!
//! This is GREEN today and NON-ignored — it must NOT be a second coordinate
//! gate. It uses a ZERO-offset tree so the target's parent-local
//! `ResolvedLayout.position` coincides with its absolute `GlobalTransform`
//! translation: the pre-C1 backend (which mis-reads local as absolute) and the
//! correct absolute read agree there, so `top_hit` is the target regardless of
//! the Bug-1 fix. What this test exercises is the `press`/`release` injection
//! seam — that writing a `PointerInput` does not panic and does not disturb the
//! hit stream — which is C1-independent. The COORDINATE divergence is proven
//! separately by `pointer_offset_regression` (the #[ignore]d C1 gate).

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_verify::pointer::PointerHarness;

#[test]
fn press_release_at_a_hit_keeps_the_entity_hit() {
    let mut h = PointerHarness::new();
    // Zero offset: local == global, so this is C1-independent (see module doc).
    let target = h.spawn_offset_tree(
        Vec2::ZERO,
        (
            Node,
            Style::default().width_px(120.0).height_px(40.0),
            Name::new("btn"),
        ),
    );
    let center = h.global_center(target);
    h.move_to(center);
    // Press then release at the same spot: the hit must remain the target
    // across the press (no spurious clearing). The CapturedEvents log is
    // empty until C3's observers exist — this asserts only the hit stream.
    h.press(PointerButton::Primary);
    assert_eq!(
        h.top_hit(),
        Some(target),
        "the target stays hit under press"
    );
    h.release(PointerButton::Primary);
    assert_eq!(
        h.top_hit(),
        Some(target),
        "the target stays hit after release"
    );
}
