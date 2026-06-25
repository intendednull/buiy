//! C5-b — light-dismiss, proven headless on the C7 `PointerHarness`
//! (scroll-overlay-modal.md §B.5, §6 Slice-B gate "Light-dismiss").
//!
//! Each test drives a synthetic primary `Pointer<Press>` / `Escape` through the
//! PRODUCTION dismiss path (the harness now adds `WidgetsPlugin`, so the
//! `light_dismiss_on_press` observer + the `escape_dismiss` keyboard handler are
//! the real systems) over a real anchored top-layer overlay, and asserts on the
//! overlay's `CssVisibility` (the open/close channel).
//!
//! Gates exercised (§6 Slice B):
//!  - **Outside press dismisses** — a press OUTSIDE an open popover closes it.
//!  - **Inside press keeps it open** — a press INSIDE the popover does NOT close
//!    it.
//!  - **Escape dismisses** — Escape closes the top-most open overlay.

use bevy::input::keyboard::KeyCode;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Stacking, Style, TopLayer, TopLayerActivation};
use buiy_core::render::components::CssVisibility;
use buiy_verify::pointer::PointerHarness;
use buiy_widgets::LightDismiss;
use buiy_widgets::dismiss::press_is_outside;
use buiy_widgets::popover::Popover;

/// Spawn a root tree with a trigger button (top-left) and an OPEN popover
/// positioned at a known, non-overlapping spot. Returns `(popover, trigger)`.
/// The popover is anchored to the trigger (so `position_popover` lowers the
/// anchor + enforces the top layer + syncs the `LightDismiss.trigger`), but a
/// large gap keeps the popover box clear of the trigger so the inside/outside
/// presses are unambiguous.
fn spawn_open_popover(h: &mut PointerHarness) -> (Entity, Entity) {
    // Trigger: a 60×30 box at the root origin (window-space (0,0)).
    let trigger = h
        .world_mut()
        .spawn((Node, Style::default().width_px(60.0).height_px(30.0)))
        .id();
    // Popover: a 120×80 box anchored below the trigger with a big gap so it sits
    // well clear of the trigger box. Open by default (CssVisibility::Visible).
    let popover = h
        .world_mut()
        .spawn((
            Style::default().width_px(120.0).height_px(80.0),
            Popover {
                anchor: Some(trigger),
                positions: vec![buiy_widgets::PopoverPlacement {
                    side: buiy_widgets::PopoverSide::Bottom,
                    gap: 200.0,
                    ..Default::default()
                }],
                ..Default::default()
            },
            CssVisibility::Visible,
        ))
        .id();
    let root = h
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    h.world_mut()
        .entity_mut(root)
        .add_children(&[trigger, popover]);
    // Settle: position_popover (before Layout) → anchor_resolution (in Layout)
    // → bridge → GlobalTransform → stacking_context (joins TopLayerActivation).
    for _ in 0..6 {
        h.update();
    }
    (popover, trigger)
}

fn visibility(h: &PointerHarness, e: Entity) -> Option<CssVisibility> {
    h.world().get::<CssVisibility>(e).copied()
}

fn is_open(h: &PointerHarness, e: Entity) -> bool {
    buiy_widgets::popover::is_open(h.world().get::<CssVisibility>(e))
}

#[test]
fn open_popover_is_registered_as_a_top_layer_overlay() {
    // Pre-condition for the dismiss gates: the open popover is in the top layer
    // and the activation deque (so the dismiss handlers can find it on top).
    let mut h = PointerHarness::new();
    let (popover, trigger) = spawn_open_popover(&mut h);

    assert_eq!(
        h.world().get::<Stacking>(popover).unwrap().top_layer,
        TopLayer::Popover
    );
    assert!(
        h.world()
            .resource::<TopLayerActivation>()
            .order
            .contains(&popover),
        "open popover joined the activation deque"
    );
    // The trigger exemption is synced from the anchor.
    assert_eq!(
        h.world().get::<LightDismiss>(popover).unwrap().trigger,
        Some(trigger)
    );
    assert!(is_open(&h, popover), "popover starts open");
}

#[test]
fn press_outside_an_open_popover_dismisses_it() {
    let mut h = PointerHarness::new();
    let (popover, trigger) = spawn_open_popover(&mut h);
    assert!(is_open(&h, popover), "popover starts open");

    // The stacking-aware `hit_test`-based outside check agrees: (700,500) is
    // outside the overlay (independent confirmation of the pick-layer split the
    // observer rides).
    assert!(
        press_is_outside(h.world(), Vec2::new(700.0, 500.0), popover, Some(trigger)),
        "the hit_test outside-detection agrees the far point is outside"
    );

    // Press at a far point that is neither the popover box nor the trigger.
    // The popover sits ~(0, 230)..(120, 310); the trigger ~(0,0)..(60,30).
    // (700, 500) is clear of both (and inside the 800×600 window so it hits the
    // root, an outside target).
    h.move_to(Vec2::new(700.0, 500.0));
    h.press(PointerButton::Primary);
    h.release(PointerButton::Primary);

    assert_eq!(
        visibility(&h, popover),
        Some(CssVisibility::Hidden),
        "a press outside the open popover light-dismisses it"
    );
    assert!(!is_open(&h, popover));
}

#[test]
fn press_inside_an_open_popover_keeps_it_open() {
    let mut h = PointerHarness::new();
    let (popover, _trigger) = spawn_open_popover(&mut h);
    assert!(is_open(&h, popover), "popover starts open");

    // Press at the popover's own center — an INSIDE press must NOT dismiss it.
    let center = h.global_center(popover);
    // The hit_test-based check agrees the center is INSIDE the overlay.
    assert!(
        !press_is_outside(h.world(), center, popover, None),
        "the hit_test outside-detection agrees the popover center is inside"
    );
    h.move_to(center);
    h.press(PointerButton::Primary);
    h.release(PointerButton::Primary);

    assert!(
        is_open(&h, popover),
        "a press inside the popover does NOT dismiss it (visibility = {:?})",
        visibility(&h, popover)
    );
    assert_ne!(visibility(&h, popover), Some(CssVisibility::Hidden));
}

#[test]
fn escape_dismisses_the_topmost_open_overlay() {
    let mut h = PointerHarness::new();
    let (popover, _trigger) = spawn_open_popover(&mut h);
    assert!(is_open(&h, popover), "popover starts open");

    // Escape closes the top-most open light-dismiss overlay.
    h.press_key(KeyCode::Escape);

    assert_eq!(
        visibility(&h, popover),
        Some(CssVisibility::Hidden),
        "Escape closes the top-most open overlay"
    );
    assert!(!is_open(&h, popover));
}
