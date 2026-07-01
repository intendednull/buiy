//! C3b — the `bevy_picking` `Pointer<E>` event layer + activation, proven on
//! the C7 `PointerHarness` (input-event-model.md §6 gates). Each test drives a
//! synthetic pointer through the PRODUCTION picking pipeline (layout → bridge →
//! GlobalTransform → Buiy backend → bevy_picking's hover stage → `Pointer<E>` +
//! observers) and asserts on the emitted events captured in `CapturedEvents`,
//! the `OnPress` activation sink, and the Buiy-native `MultiClick` gesture.
//!
//! These exercise the C3b deliverables:
//!  - `Pointer<Click>` → `OnPress` on a widget root (§2.5) + observer capture.
//!  - `Pointer<Over>`/`Pointer<Out>` fire on enter/leave (§2.2 no-hit emission).
//!  - `MultiClick { count: 2 }` on a double-click within the `ClickTracker`
//!    window+radius; a slow second click does NOT (§2.11).
//!  - `Pointer<Scroll>` fires on a wheel input (§2.6 wheel entry).

use std::time::Duration;

use bevy::ecs::message::Messages;
use bevy::input::mouse::MouseScrollUnit;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::a11y::A11yRole;
use buiy_core::interaction::OnPress;
use buiy_core::{Node, layout::Style};
use buiy_verify::pointer::PointerHarness;

/// A widget-root bundle the C3 activation producer recognizes: a node carrying
/// the button activation role (`A11yRole::Button`) — what `Button`'s
/// `#[require]` contract attaches. Built from `buiy_core` only, so the harness
/// needs no `buiy_widgets` dependency.
fn widget_root(w: f32, h: f32) -> impl Bundle {
    (
        Node,
        Style::default().width_px(w).height_px(h),
        A11yRole::Button,
        Name::new("widget-root"),
    )
}

/// Read whether `Messages<OnPress>` carries an `OnPress(entity)`.
fn on_press_fired(h: &mut PointerHarness, entity: Entity) -> bool {
    let messages = h.world_mut().resource::<Messages<OnPress>>();
    let mut cursor = messages.get_cursor();
    cursor.read(messages).any(|ev| ev.0 == entity)
}

/// Gate #5 (pointer activation → `OnPress`) + observer capture: a synthetic
/// press-then-release on a widget root emits `OnPress` AND the harness records
/// the `Pointer<Click>`.
#[test]
fn click_on_widget_root_emits_on_press_and_records_click() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let center = h.global_center(root);
    h.move_to(center);
    h.click(PointerButton::Primary);

    assert!(
        on_press_fired(&mut h, root),
        "a pointer Click on the widget root must emit OnPress (the SC-1 sink)"
    );
    assert!(
        h.captured().saw(root, "click"),
        "the harness CapturedEvents must record the Pointer<Click> on the root"
    );
    // Press + Release also flow through (the press-arm plumbing).
    assert!(h.captured().saw(root, "press"), "Pointer<Press> recorded");
    assert!(
        h.captured().saw(root, "release"),
        "Pointer<Release> recorded"
    );
}

/// Gate #8 (press-arm / release-off-target = drag-cancel): a press on the root
/// then a release OFF the root produces NO `Pointer<Click>` and NO `OnPress`.
/// bevy_picking's `Pointer<Click>` only fires when press + release share a
/// target, so the drag-cancel falls out for free.
#[test]
fn release_off_target_does_not_activate() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let center = h.global_center(root);

    h.move_to(center);
    h.press(PointerButton::Primary);
    // Move OFF the root (far corner, outside any node) THEN release.
    h.move_to(Vec2::new(700.0, 560.0));
    h.release(PointerButton::Primary);

    assert!(
        !on_press_fired(&mut h, root),
        "release off-target must NOT activate (drag-cancel)"
    );
    assert!(
        !h.captured().saw(root, "click"),
        "no Pointer<Click> when press + release do not share a target"
    );
}

/// Touch-input fix (cold tap): a `PointerId::Touch` tap on a widget root with NO
/// prior hover/move emits `OnPress`. This is the case bevy_picking's `Pointer<Click>`
/// and `Pointer<Release>` CANNOT activate — both target the PREVIOUS frame's hover
/// map (events.rs:656), which a first-touch pointer never populates (spawned on
/// press, despawned after release). The fix records the press via `Pointer<Press>`
/// (current hover map) + activates on the raw `Release` `PointerInput` gated on the
/// current hover map. Verified end-to-end in a real WebGL2 browser; this is the
/// headless CI guard. (RED before the fix — the tap produced no `OnPress`.)
#[test]
fn cold_touch_tap_activates_widget_root() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let center = h.global_center(root);

    // NO move_to — a cold tap (the finger's first contact IS the press).
    h.touch_tap_cold(center);

    assert!(
        on_press_fired(&mut h, root),
        "a cold touch tap on the widget root must emit OnPress (the touch-input fix)"
    );
}

/// Touch drag-cancel: a touch press on the root then a release OFF the root does
/// NOT activate — the same drag-cancel the mouse `Click` gives for free, here via
/// the current-hover-map check in `touch_tap_activates` (the release's current
/// hover does not contain the pressed target).
#[test]
fn touch_release_off_target_does_not_activate() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let center = h.global_center(root);

    // Press on the root, release at empty space (finger dragged off).
    h.touch_tap(center, Vec2::new(700.0, 560.0));

    assert!(
        !on_press_fired(&mut h, root),
        "a touch release dragged off the pressed target must NOT activate"
    );
}

/// Gate #3 (no-hit clears hover): moving onto a node fires `Pointer<Over>`;
/// moving off all Buiy nodes fires `Pointer<Out>`. The Out depends on the
/// backend's no-hit emission (§2.2) — without it the hover never clears.
#[test]
fn over_and_out_fire_on_enter_and_leave() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let center = h.global_center(root);

    // Enter: a Pointer<Over> over the root.
    h.move_to(center);
    assert!(
        h.captured().saw(root, "over"),
        "Pointer<Over> fires when the cursor enters the node"
    );

    // Leave: move to empty space; the no-hit emission drives Pointer<Out>.
    h.move_to(Vec2::new(700.0, 560.0));
    assert!(
        h.captured().saw(root, "out"),
        "Pointer<Out> fires when the cursor leaves all Buiy nodes (no-hit emission)"
    );
}

/// Gate #9 (`MultiClick` on a non-editor entity): two clicks within the
/// `ClickTracker` window+radius over a plain widget root fire `MultiClick`
/// with `count == 2`; a slow second click (advance the clock past the 450ms
/// window) does NOT. Derived from the SAME `ClickTracker` heuristic the editor
/// uses, not bevy's untuned `Click.count`.
#[test]
fn double_click_fires_multiclick_but_slow_second_click_does_not() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let center = h.global_center(root);

    h.move_to(center);
    h.double_click(PointerButton::Primary);
    assert!(
        h.captured().saw(root, "multiclick"),
        "a fast double-click within the ClickTracker window fires MultiClick"
    );

    // A second harness for the slow case so the capture log starts clean.
    let mut slow = PointerHarness::new();
    let slow_root = slow.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let slow_center = slow.global_center(slow_root);
    slow.move_to(slow_center);
    slow.click(PointerButton::Primary);
    // Advance the clock well past the 450ms multi-click window before the
    // second click — the ClickTracker resets the streak to a single click.
    slow.advance_time(Duration::from_millis(600));
    slow.click(PointerButton::Primary);
    assert!(
        !slow.captured().saw(slow_root, "multiclick"),
        "a slow second click (outside the window) does NOT fire MultiClick"
    );
}

/// Gate #6 (`Pointer<Scroll>` wheel entry): a synthetic wheel input over a node
/// fires `Pointer<Scroll>` with the expected `unit`/`y` (§2.6). The wheel entry
/// is free once `InteractionPlugin` is wired; this proves a `Pointer<Scroll>`
/// reaches the hovered entity.
#[test]
fn wheel_input_fires_pointer_scroll() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), widget_root(120.0, 40.0));
    let center = h.global_center(root);

    h.move_to(center);
    h.scroll(MouseScrollUnit::Line, 0.0, -3.0);
    assert!(
        h.captured().saw(root, "scroll"),
        "a wheel input over the node fires Pointer<Scroll> on the hovered entity"
    );
    let (unit, y) = h.last_scroll(root).expect("a recorded Pointer<Scroll>");
    assert_eq!(
        unit,
        MouseScrollUnit::Line,
        "the wheel unit carries deltaMode"
    );
    assert_eq!(y, -3.0, "the wheel y-delta carries through");
}
