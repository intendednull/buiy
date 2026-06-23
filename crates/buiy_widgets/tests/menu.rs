//! C5-c — Menu / MenuButton / MenuItem (scroll-overlay-modal.md §B.3), proven at
//! the widget layer.
//!
//! Gates exercised:
//!  - **gate #3 (a11y bundle)** — a `MenuButton` advertises `A11yHasPopup(Menu)` +
//!    `A11yExpanded` and `controls` its menu; the `Menu` is `A11yRole::Menu` with
//!    `A11yRole::MenuItem` children; when navigated, the menu's `active_descendant`
//!    points at the active item — asserted both at the component level and through
//!    the production `build_tree` → in-process consumer snapshot.
//!  - **Keyboard nav (roving via active_descendant)** — open (A11yExpanded true) →
//!    ArrowDown/Up move active_descendant across items (wrap) → Home/End → Enter
//!    activates the active item (`OnPress` fires for it) → Esc closes
//!    (A11yExpanded false), focus restored to the button.
//!  - **Open/close lifecycle** — a click (OnPress) on the button opens the menu
//!    (visible + focused + first item active); a second click closes it.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use buiy_core::a11y::inprocess::TreeView;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{
    A11yExpanded, A11yHasPopup, A11yPlugin, A11yRelations, A11yRole, HasPopup, snapshot,
};
use buiy_core::focus::{FocusPlugin, Focusable, FocusedEntity};
use buiy_core::interaction::OnPress;
use buiy_core::render::components::CssVisibility;
use buiy_core::{CorePlugin, components::Node};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::menu::{Menu, MenuButton, MenuItem};

/// A headless app with the a11y + focus + widget surface. `A11yPlugin` runs
/// `build_tree` (so the in-process snapshot reflects the menu), `FocusPlugin`
/// owns `FocusedEntity`, and `WidgetsPlugin` carries the menu systems
/// (`sync_menu_open` / `menu_keyboard_nav` / `wire_menu_button`). `KeyboardInput`
/// is registered manually (no `InputPlugin` under `MinimalPlugins`).
fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_message::<KeyboardInput>();
    app
}

/// Spawn a 3-item menu button ("Cut"/"Copy"/"Paste"), settle the wiring, and
/// return `(button, menu, [items])`.
fn spawn_menu_button(app: &mut App) -> (Entity, Entity, Vec<Entity>) {
    let button = app
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
    // Settle: `children!` spawn → `wire_menu_button` (Added<Children>) wires the
    // button↔menu edges.
    for _ in 0..3 {
        app.update();
    }
    let menu = menu_of(app, button);
    let items = items_of(app, menu);
    (button, menu, items)
}

/// The `Menu` child of `button`.
fn menu_of(app: &App, button: Entity) -> Entity {
    let world = app.world();
    world
        .get::<Children>(button)
        .expect("button has children")
        .iter()
        .find(|&c| world.get::<Menu>(c).is_some())
        .expect("button has a Menu child")
}

/// The `MenuItem` children of `menu`, in document order.
fn items_of(app: &App, menu: Entity) -> Vec<Entity> {
    let world = app.world();
    world
        .get::<Children>(menu)
        .expect("menu has children")
        .iter()
        .filter(|&c| world.get::<MenuItem>(c).is_some())
        .collect()
}

/// Send one KeyDown of `key` and run the schedule.
fn key_down(app: &mut App, key: KeyCode) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Character("x".into()),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

/// Write `OnPress(entity)` (the shared activation sink — the pointer/keyboard/AT
/// path all converge here) and run the schedule.
fn press(app: &mut App, entity: Entity) {
    app.world_mut().write_message(OnPress(entity));
    app.update();
}

fn active_descendant(app: &App, menu: Entity) -> Option<Entity> {
    app.world()
        .get::<A11yRelations>(menu)
        .and_then(|r| r.active_descendant)
}

fn is_visible(app: &App, e: Entity) -> bool {
    buiy_widgets::popover::is_open(app.world().get::<CssVisibility>(e))
}

fn expanded(app: &App, button: Entity) -> bool {
    app.world()
        .get::<A11yExpanded>(button)
        .map(|e| e.0)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// gate #3 — the a11y bundle: MenuButton haspopup+expanded+controls; Menu role +
// MenuItem children.
// ---------------------------------------------------------------------------

#[test]
fn bare_menu_button_marker_materializes_the_full_required_contract() {
    let mut app = app();
    let b = app.world_mut().spawn(MenuButton).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(b).is_some(), "Node");
    assert!(world.get::<Focusable>(b).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(b).copied(),
        Some(A11yRole::Button),
        "the menu button is A11yRole::Button (the popup is state-keyed, not a new role)"
    );
    assert_eq!(
        world.get::<A11yHasPopup>(b).map(|h| h.0),
        Some(HasPopup::Menu),
        "A11yHasPopup(Menu) advertises aria-haspopup=menu"
    );
    assert_eq!(
        world.get::<A11yExpanded>(b).map(|e| e.0),
        Some(false),
        "A11yExpanded present, defaulting to false (closed) — the disclosure pattern"
    );
}

#[test]
fn menu_button_new_wires_controls_anchor_and_menu_shape() {
    let mut app = app();
    let (button, menu, items) = spawn_menu_button(&mut app);

    assert_eq!(items.len(), 3, "Cut/Copy/Paste items");

    let world = app.world();
    // The button controls the menu (wired by `wire_menu_button`).
    assert_eq!(
        world
            .get::<A11yRelations>(button)
            .map(|r| r.controls.clone()),
        Some(vec![menu]),
        "the MenuButton's A11yRelations.controls references the menu"
    );
    // The menu is an A11yRole::Menu popover anchored to the button.
    assert_eq!(
        world.get::<A11yRole>(menu).copied(),
        Some(A11yRole::Menu),
        "the menu is A11yRole::Menu"
    );
    assert_eq!(
        world
            .get::<buiy_widgets::Popover>(menu)
            .and_then(|p| p.anchor),
        Some(button),
        "the menu's Popover is anchored to the button (positioned below it)"
    );
    // Each item is an A11yRole::MenuItem.
    for &item in &items {
        assert_eq!(
            world.get::<A11yRole>(item).copied(),
            Some(A11yRole::MenuItem),
            "each entry is an A11yRole::MenuItem"
        );
    }
    // The menu starts closed.
    assert!(!is_visible(&app, menu), "the menu starts closed (hidden)");
}

#[test]
fn gate3_a11y_tree_menu_role_items_and_active_descendant() {
    // The gate-#3 assertion through the PRODUCTION build_tree → in-process
    // consumer: open the menu, navigate, and read the menu's active_descendant +
    // its MenuItem children back through the same path a real AT consumes.
    let mut app = app();
    let (button, menu, items) = spawn_menu_button(&mut app);

    // Open the menu (click the button → OnPress → advance_expanded → sync_menu_open).
    press(&mut app, button);
    assert!(
        expanded(&app, button),
        "the button is expanded after opening"
    );
    assert!(is_visible(&app, menu), "the menu is visible after opening");
    // On open, active_descendant points at the first item.
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[0]),
        "on open, the menu's active_descendant is the first item"
    );

    // Move to the second item, then read the a11y tree.
    key_down(&mut app, KeyCode::ArrowDown);
    app.update(); // settle build_tree

    let tree = snapshot(app.world_mut(), TreeView::default());
    let menu_node = tree
        .node(node_id_for(menu))
        .expect("the menu emits an a11y node");
    assert_eq!(
        menu_node.role,
        A11yRole::Menu,
        "the a11y node's role is Menu"
    );
    // The menu's children are the MenuItems (in document order).
    let item_ids: Vec<_> = items.iter().map(|&e| node_id_for(e)).collect();
    assert_eq!(
        menu_node.children, item_ids,
        "the menu's a11y children are the MenuItems in order"
    );
    // The active_descendant resolved to the active item's NodeId.
    assert_eq!(
        menu_node.active_descendant,
        Some(node_id_for(items[1])),
        "the menu's a11y active_descendant points at the active (second) item"
    );
    // Each MenuItem emits a MenuItem-role node.
    for &item in &items {
        let n = tree
            .node(node_id_for(item))
            .expect("each item emits an a11y node");
        assert_eq!(n.role, A11yRole::MenuItem);
    }
}

// ---------------------------------------------------------------------------
// Open/close lifecycle — click toggles via A11yExpanded.
// ---------------------------------------------------------------------------

#[test]
fn click_opens_then_closes_the_menu() {
    let mut app = app();
    let (button, menu, items) = spawn_menu_button(&mut app);

    // Click 1: open.
    press(&mut app, button);
    assert!(expanded(&app, button), "first click opens (expanded)");
    assert!(is_visible(&app, menu), "menu visible");
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[0]),
        "first item active on open"
    );
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(menu),
        "focus moves into the menu container on open"
    );

    // Click 2: close (the button's A11yExpanded flips back via advance_expanded).
    press(&mut app, button);
    assert!(!expanded(&app, button), "second click closes (collapsed)");
    assert!(!is_visible(&app, menu), "menu hidden");
    assert_eq!(
        active_descendant(&app, menu),
        None,
        "active_descendant cleared on close"
    );
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(button),
        "focus restored to the button on close"
    );
}

// ---------------------------------------------------------------------------
// Roving keyboard nav — Arrow/Home/End move active_descendant; Enter activates;
// Esc closes.
// ---------------------------------------------------------------------------

#[test]
fn arrow_keys_move_active_descendant_with_wrap() {
    let mut app = app();
    let (button, menu, items) = spawn_menu_button(&mut app);
    press(&mut app, button); // open: active = items[0], menu focused
    assert_eq!(active_descendant(&app, menu), Some(items[0]));

    // ArrowDown: 0 → 1 → 2 → wrap to 0.
    key_down(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[1]),
        "down → item 1"
    );
    key_down(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[2]),
        "down → item 2"
    );
    key_down(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[0]),
        "down past the last item wraps to the first"
    );

    // ArrowUp: 0 → wrap to last (2) → 1.
    key_down(&mut app, KeyCode::ArrowUp);
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[2]),
        "up past the first item wraps to the last"
    );
    key_down(&mut app, KeyCode::ArrowUp);
    assert_eq!(active_descendant(&app, menu), Some(items[1]), "up → item 1");
}

#[test]
fn home_and_end_jump_to_first_and_last_item() {
    let mut app = app();
    let (button, menu, items) = spawn_menu_button(&mut app);
    press(&mut app, button);

    key_down(&mut app, KeyCode::End);
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[2]),
        "End jumps to the last item"
    );
    key_down(&mut app, KeyCode::Home);
    assert_eq!(
        active_descendant(&app, menu),
        Some(items[0]),
        "Home jumps to the first item"
    );
}

#[test]
fn enter_activates_the_active_item_and_closes() {
    let mut app = app();
    let (button, menu, items) = spawn_menu_button(&mut app);
    press(&mut app, button);
    key_down(&mut app, KeyCode::ArrowDown); // active = items[1]
    assert_eq!(active_descendant(&app, menu), Some(items[1]));

    // Register a system to drain OnPress so we can observe the activation target.
    let drained = {
        // Enter activates the active item (writes OnPress for items[1]) and closes.
        // Capture the OnPress written this frame by reading it after the keydown.
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Character("x".into()),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        // Run menu_keyboard_nav (writes OnPress(items[1])) and read it before the
        // next frame consumes it.
        app.update();
        let sys = app
            .world_mut()
            .register_system(|mut reader: MessageReader<OnPress>| {
                reader.read().map(|p| p.0).collect::<Vec<_>>()
            });
        let out = app.world_mut().run_system(sys).unwrap();
        app.world_mut().unregister_system(sys).ok();
        out
    };
    assert!(
        drained.contains(&items[1]),
        "Enter writes OnPress for the active item (items[1]); drained = {drained:?}"
    );

    // The menu closed (the button's A11yExpanded flipped to false → sync_menu_open).
    assert!(!expanded(&app, button), "Enter closes the menu (collapsed)");
    assert!(!is_visible(&app, menu), "menu hidden after activate");
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(button),
        "focus restored to the button after activate"
    );
}

#[test]
fn escape_closes_the_menu_and_restores_focus() {
    let mut app = app();
    let (button, menu, _items) = spawn_menu_button(&mut app);
    press(&mut app, button);
    assert!(is_visible(&app, menu), "menu open before Escape");

    key_down(&mut app, KeyCode::Escape);

    assert!(!expanded(&app, button), "Escape closes (collapsed)");
    assert!(!is_visible(&app, menu), "menu hidden after Escape");
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(button),
        "focus restored to the button after Escape"
    );
}
