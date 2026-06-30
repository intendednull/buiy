//! C5-c — the open menu honors the C5-b light-dismiss (an outside press closes
//! it), proven headless on the C7 `PointerHarness` (scroll-overlay-modal.md §B.3 +
//! §B.5). The menu IS a `Popover`, so it inherits the `auto` `LightDismiss` policy;
//! this confirms the dismiss channel still works for a *menu* and keeps the
//! controlling button's `A11yExpanded` (`aria-expanded`) in lock-step (the dismiss
//! enqueues `MenuMsg::Close`; `menu_reducer` folds it and `bind_menu_model` projects
//! the collapse — replacing the old `sync_menu_dismissed` reconciliation).
//!
//! Gates exercised:
//!  - **Open menu is a top-layer overlay** — opening the menu (clicking the
//!    button) registers it in the `TopLayerActivation` deque.
//!  - **Outside press dismisses the menu** — a press OUTSIDE the open menu closes
//!    it, AND the button's `A11yExpanded` flips back to `false`.
//!  - **Inside press keeps it open** — a press INSIDE the menu does NOT close it.

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::a11y::{A11yExpanded, A11yRelations};
use buiy_core::components::Node;
use buiy_core::focus::FocusedEntity;
use buiy_core::layout::{Style, TopLayerActivation};
use buiy_core::render::components::CssVisibility;
use buiy_verify::pointer::PointerHarness;
use buiy_widgets::menu::{Menu, MenuButton, MenuItem};

/// Spawn a menu button (3 items) inside a root, settle the button↔menu wiring,
/// then **open** the menu by writing `OnPress` for the button (the activation sink
/// the pointer/keyboard/AT paths converge on). Returns `(button, menu, [items])`
/// with the menu open + registered in the activation deque.
fn spawn_open_menu(h: &mut PointerHarness) -> (Entity, Entity, Vec<Entity>) {
    let button = h
        .world_mut()
        .spawn(MenuButton::new(
            "Edit",
            children![
                MenuItem::new("Cut"),
                MenuItem::new("Copy"),
                MenuItem::new("Paste"),
            ],
        ))
        .id();
    let root = h
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    h.world_mut().entity_mut(root).add_children(&[button]);
    // Settle the children!/wiring before opening.
    for _ in 0..3 {
        h.update();
    }
    let menu = menu_of(h, button);

    // Open the menu (OnPress → route_menu_press enqueues Toggle → menu_reducer folds
    // Toggle→Open → bind_menu_model projects visibility + A11yExpanded).
    h.world_mut()
        .write_message(buiy_core::interaction::OnPress(button));
    // Settle: open (visible) → position_popover → anchor_resolution → bridge →
    // stacking_context (joins TopLayerActivation).
    for _ in 0..6 {
        h.update();
    }
    let items = items_of(h, menu);
    (button, menu, items)
}

fn menu_of(h: &PointerHarness, button: Entity) -> Entity {
    let world = h.world();
    world
        .get::<Children>(button)
        .expect("button has children")
        .iter()
        .find(|&c| world.get::<Menu>(c).is_some())
        .expect("button has a Menu child")
}

fn items_of(h: &PointerHarness, menu: Entity) -> Vec<Entity> {
    let world = h.world();
    world
        .get::<Children>(menu)
        .map(|c| {
            c.iter()
                .filter(|&e| world.get::<MenuItem>(e).is_some())
                .collect()
        })
        .unwrap_or_default()
}

fn is_open(h: &PointerHarness, e: Entity) -> bool {
    buiy_widgets::popover::is_open(h.world().get::<CssVisibility>(e))
}

fn expanded(h: &PointerHarness, button: Entity) -> bool {
    h.world()
        .get::<A11yExpanded>(button)
        .map(|e| e.0)
        .unwrap_or(false)
}

#[test]
fn open_menu_is_registered_as_a_top_layer_overlay() {
    let mut h = PointerHarness::new();
    let (button, menu, _items) = spawn_open_menu(&mut h);

    assert!(
        is_open(&h, menu),
        "the menu is open after the button activation"
    );
    assert!(
        expanded(&h, button),
        "the button's A11yExpanded is true while open"
    );
    assert_eq!(
        h.world()
            .get::<A11yRelations>(button)
            .map(|r| r.controls.clone()),
        Some(vec![menu]),
        "the button controls the menu"
    );
    assert!(
        h.world()
            .resource::<TopLayerActivation>()
            .order
            .contains(&menu),
        "the open menu joined the TopLayerActivation deque (the dismiss handlers find it)"
    );
}

#[test]
fn press_outside_an_open_menu_dismisses_it_and_collapses_the_button() {
    let mut h = PointerHarness::new();
    let (button, menu, _items) = spawn_open_menu(&mut h);
    assert!(is_open(&h, menu), "the menu starts open");

    // Press far from the menu + button (inside the 800×600 window so it hits the
    // root, an outside target).
    h.move_to(Vec2::new(700.0, 500.0));
    h.press(PointerButton::Primary);
    h.release(PointerButton::Primary);
    // Settle the dismiss → menu_dismiss_hook enqueues Close → menu_reducer folds it →
    // bind_menu_model projects the collapse.
    for _ in 0..2 {
        h.update();
    }

    assert!(
        !is_open(&h, menu),
        "a press outside the open menu light-dismisses it (the menu IS a Popover)"
    );
    assert!(
        !expanded(&h, button),
        "the dismiss reconciled the button's A11yExpanded back to false (re-open works)"
    );
}

#[test]
fn press_inside_an_open_menu_keeps_it_open() {
    let mut h = PointerHarness::new();
    let (_button, menu, _items) = spawn_open_menu(&mut h);
    assert!(is_open(&h, menu), "the menu starts open");

    // Press-DOWN at the menu's own center — an INSIDE press must NOT light-dismiss
    // it. Light-dismiss fires on `Pointer<Press>` (dismiss.rs), so a press-down is
    // the precise inside/outside containment test. We deliberately do NOT release:
    // a full click here lands on the centre `MenuItem` and SELECTS it (close-on-
    // select — the pointer mirror of keyboard Enter, see
    // `menu_item_click_emits_on_press` + its activation tests), which is a separate
    // behaviour from light-dismiss. The press-down alone must keep the menu open.
    let center = h.global_center(menu);
    h.move_to(center);
    h.press(PointerButton::Primary);
    for _ in 0..2 {
        h.update();
    }

    assert!(
        is_open(&h, menu),
        "a press inside the menu does NOT dismiss it (visibility = {:?})",
        h.world().get::<CssVisibility>(menu).copied()
    );
}

#[test]
fn open_menu_focuses_the_container_for_roving() {
    // The container holds focus (the aria-activedescendant model) — confirm the
    // menu container, not an item, is focused while open.
    let mut h = PointerHarness::new();
    let (_button, menu, items) = spawn_open_menu(&mut h);
    assert_eq!(
        h.world().resource::<FocusedEntity>().0,
        Some(menu),
        "the menu container holds focus while open (roving / activedescendant)"
    );
    // The first item is the active descendant.
    assert_eq!(
        h.world()
            .get::<A11yRelations>(menu)
            .and_then(|r| r.active_descendant),
        items.first().copied(),
        "the first item is the active descendant on open"
    );
}
