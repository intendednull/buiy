//! C3d — the consolidated focus-on-click + `:focus-visible` decay
//! (input-event-model.md § 2.7 / co-drive SC-2), proven on the C7
//! `PointerHarness`. The single shared `buiy_core::focus::focus_on_click`
//! observer owns `FocusedEntity` for ALL pointer focus (no per-widget observer):
//! a primary `Pointer<Press>` over a `Focusable` (the picked target or its
//! nearest `Focusable` ancestor) sets `FocusedEntity` to it AND `FocusVisible`
//! to `false` — pointer focus is NOT keyboard-`:focus-visible`. The keyboard
//! (Tab) path stays `FocusVisible(true)` (the decay signal C6's ring consumes).
//!
//! These drive a synthetic pointer / Tab through the PRODUCTION pipeline (layout
//! → bridge → GlobalTransform → Buiy backend → bevy_picking's hover stage →
//! `Pointer<Press>` → the shared observer) and assert the `FocusedEntity` flip +
//! the `FocusVisible` decay.

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::focus::Focusable;
use buiy_core::{Node, layout::Style};
use buiy_verify::pointer::PointerHarness;

/// A focusable widget root: a sized node carrying `Focusable`. Built from
/// `buiy_core` only (no `buiy_widgets` dependency), like `widget_root` in
/// `pointer_events_c3b.rs`.
fn focusable_root(w: f32, h: f32) -> impl Bundle {
    (
        Node,
        Style::default().width_px(w).height_px(h),
        Focusable::default(),
        Name::new("focusable-root"),
    )
}

/// Gate (focus-on-click): a primary press on a `Focusable` sets `FocusedEntity`
/// to it AND clears `FocusVisible` (pointer focus is not keyboard-visible — the
/// § 2.7 decay). One shared observer owns this for every `Focusable`.
#[test]
fn press_on_focusable_sets_focus_and_clears_focus_visible() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), focusable_root(120.0, 40.0));
    let center = h.global_center(root);

    // Nothing focused; FocusVisible defaults false. Make the decay assertion
    // meaningful by FIRST establishing focus-visible via Tab, so the pointer
    // press has a `true` to decay FROM.
    h.press_key(KeyCode::Tab);
    assert!(
        h.focus_visible(),
        "precondition: Tab established focus-visible (the value the press must clear)"
    );

    h.move_to(center);
    h.press(PointerButton::Primary);

    assert_eq!(
        h.focused(),
        Some(root),
        "a primary press on a Focusable focuses it (the shared focus-on-click observer)"
    );
    assert!(
        !h.focus_visible(),
        "pointer focus clears FocusVisible (the :focus-visible decay, § 2.7)"
    );
}

/// The ancestor walk: a press on a NON-focusable decorative child focuses its
/// nearest `Focusable` ancestor (the widget root), not the child. The picked
/// target is the child (topmost paint), so this only passes once the shared
/// observer walks `ChildOf` to the focusable root.
#[test]
fn press_on_non_focusable_child_focuses_the_focusable_ancestor() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), focusable_root(120.0, 40.0));

    // A non-focusable child filling the root — the picked (topmost) target.
    let child = h
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(120.0).height_px(40.0),
            Name::new("decorative-child"),
        ))
        .id();
    h.world_mut().entity_mut(root).add_child(child);
    // Settle layout/bridge so the child gets a GlobalTransform + ResolvedLayout.
    for _ in 0..4 {
        h.update();
    }

    let center = h.global_center(child);
    // Confirm the child is genuinely the picked (topmost) target before pressing.
    h.move_to(center);
    assert_eq!(
        h.top_hit(),
        Some(child),
        "the decorative child is the topmost-painted hit"
    );

    h.press(PointerButton::Primary);
    assert_eq!(
        h.focused(),
        Some(root),
        "the press focuses the nearest Focusable ancestor (the root), not the picked child"
    );
    assert!(
        !h.focus_visible(),
        "ancestor focus-on-click still clears FocusVisible"
    );
}

/// A press over a subtree with NO `Focusable` (a plain node) leaves focus
/// untouched — clicking inert chrome must not steal focus.
#[test]
fn press_on_non_focusable_subtree_leaves_focus_untouched() {
    let mut h = PointerHarness::new();
    // A plain (non-Focusable) node.
    let plain = h.spawn_offset_tree(
        Vec2::new(40.0, 30.0),
        (
            Node,
            Style::default().width_px(120.0).height_px(40.0),
            Name::new("plain-node"),
        ),
    );
    let center = h.global_center(plain);

    h.move_to(center);
    h.press(PointerButton::Primary);

    assert_eq!(
        h.focused(),
        None,
        "a press on a non-Focusable subtree does not move focus"
    );
}

/// Gate (keyboard focus-visible — no regression): Tab still sets
/// `FocusVisible(true)`. This is the `true` half of the decay the pointer path
/// clears; C6's ring renders only when this is `true`.
#[test]
fn tab_sets_focus_visible_true() {
    let mut h = PointerHarness::new();
    let _root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), focusable_root(120.0, 40.0));

    assert!(
        !h.focus_visible(),
        "precondition: FocusVisible starts false"
    );
    h.press_key(KeyCode::Tab);
    assert!(
        h.focus_visible(),
        "keyboard (Tab) focus IS focus-visible (FocusVisible true) — the decay's true half"
    );
}

/// The full decay cycle on ONE harness: Tab → `true`, then a pointer press →
/// `false`. Proves the two halves coexist and the pointer path genuinely decays
/// the keyboard-established signal (not just that each is independently correct).
#[test]
fn focus_visible_decays_true_on_tab_then_false_on_pointer() {
    let mut h = PointerHarness::new();
    let root = h.spawn_offset_tree(Vec2::new(40.0, 30.0), focusable_root(120.0, 40.0));

    h.press_key(KeyCode::Tab);
    assert!(h.focus_visible(), "Tab → focus-visible");

    let center = h.global_center(root);
    h.move_to(center);
    h.press(PointerButton::Primary);
    assert_eq!(h.focused(), Some(root), "pointer focuses the root");
    assert!(
        !h.focus_visible(),
        "the subsequent pointer press decays focus-visible to false"
    );
}
