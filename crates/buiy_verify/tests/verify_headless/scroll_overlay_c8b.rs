//! C8-b — the **S2 (scroll / long-list) + S3 (overlay / menu) inspection-driver
//! acceptance** (widget-gallery-exemplar §6 / co-drive grounding loops). Mirrors
//! the C8-a TodoMVC acceptance (`todomvc_c8a.rs`): every interaction branch is
//! driven through the C7 `PointerHarness` (real synthetic pointer/keyboard over
//! the production picking + scroll + dismiss path) or the in-process a11y driver
//! (`buiy_core::a11y::inprocess`: `get_by_role`/`snapshot`/`show_tooltip`/…) and
//! asserted through the live state + the a11y tree — never by reading bespoke
//! internal state.
//!
//! The screens + their app logic are
//! `buiy_gallery::{screen_scroll_list, screen_overlay_menu, OverlayMenuPlugin, …}`
//! (pure composition over the landed C5 containers + the P1d widgets). These are
//! the live gates; the static layout snapshots live in `examples/buiy_gallery/tests`.
//!
//! ## S2 — scroll / long-list (the scale-game)
//!  - **Pointer wheel** (C7 `PointerHarness.scroll()`): a wheel-down advances the
//!    1000-row `ScrollArea`'s clamped `ScrollOffset`; a large wheel clamps at the
//!    content end; a wheel-up clamps at the top.
//!  - **Keyboard scroll**: PageDown advances, End jumps to the bottom bound.
//!  - **SC-4 a11y scroll fields**: after a scroll, the driver `snapshot` of the
//!    scroll region reports the live offset + per-axis max (the C5/SC-4 source
//!    folded through `build_tree` into the consumer's scroll getters).
//!
//! ## S3 — overlay / menu
//!  - **Menu open**: activating the MenuButton (`OnPress`) opens the menu — the
//!    button's `A11yExpanded` is true + it advertises `A11yHasPopup(Menu)` in the
//!    snapshot, and the menu container holds focus.
//!  - **Arrow nav**: ArrowDown moves the menu's `active_descendant` across items.
//!  - **Enter activates**: Enter on the active item fires its `OnPress` (observed
//!    as the `MenuActivations` effect) AND closes the menu.
//!  - **Esc / outside-press close**: both light-dismiss the open menu and
//!    reconcile the button's `A11yExpanded` back to false.
//!  - **Tooltip**: a driver `show_tooltip` reveals the trigger's tooltip node
//!    (its `CssVisibility` flips), and `position_tooltip` places it (a non-origin
//!    `Anchor` chain below the trigger).
//!  - **Popover light-dismiss**: an outside press on an open anchored popover
//!    closes it; an inside press keeps it open.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseScrollUnit;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::a11y::inprocess::snapshot;
use buiy_core::a11y::{
    A11yExpanded, A11yPlugin, A11yRelations, A11yRole, A11yScroll, get_by_role, show_tooltip,
};
use buiy_core::components::Node;
use buiy_core::focus::{FocusPlugin, FocusedEntity};
use buiy_core::interaction::OnPress;
use buiy_core::layout::{Anchor, LayoutPlugin, ScrollOffset, Style, TopLayerActivation};
use buiy_core::render::components::CssVisibility;
use buiy_core::scroll::{ScrollExtent, ScrollInputPlugin};
use buiy_core::text::BuiyTextPlugin;
use buiy_gallery::{
    MENU_ITEM_LABELS, MenuActivations, OverlayMenuPlugin, ScrollList, screen_scroll_list,
    spawn_overlay_menu,
};
use buiy_verify::pointer::PointerHarness;
use buiy_widgets::menu::{Menu, MenuButton, MenuItem};
use buiy_widgets::popover::{Popover, is_open};
use buiy_widgets::tooltip::{TooltipNode, TooltipTrigger};
use buiy_widgets::{ScrollArea, WidgetsPlugin};

// ###########################################################################
// S2 — scroll / long-list
// ###########################################################################

/// The S2 row count exercised in the acceptance — the 1000× TodoMVC scale-game.
const ROWS: usize = 1000;

/// One row's fixed height (mirrors `buiy_gallery`'s `SCROLL_ROW_H`), so the test
/// can derive the expected content extent (`ROWS × ROW_H`) for the clamp bound.
const ROW_H: f32 = 28.0;

/// The S2 viewport height (mirrors `buiy_gallery`'s `SCROLL_VIEWPORT_H`).
const VIEWPORT_H: f32 = 300.0;

// ---------------------------------------------------------------------------
// S2 pointer/keyboard scroll — driven on the C7 PointerHarness.
//
// The harness has no AssetPlugin/ScenePlugin (so it cannot `spawn_scene` the
// screen-fn), so the 1000-row `ScrollArea` is spawned imperatively from the SAME
// `ScrollArea` widget + row shape the screen authors (`screen_scroll_list` /
// `scroll_row`). This exercises the real widget + the 1000-entity scale through
// the production scroll pipeline (the convergence-leg discipline `todomvc_c8a`'s
// synthetic-pointer test uses).
// ---------------------------------------------------------------------------

/// Spawn a 1000-row `ScrollArea` (the S2 scale) into the harness's 800×600 window
/// and settle layout so `update_scroll_extent` populates the extent. Returns the
/// scroll-area entity. Each row pins its `min_height` so the flex column cannot
/// shrink the overflowing content back to the viewport (the C5-a discipline).
fn spawn_scroll_screen(h: &mut PointerHarness, n: usize) -> Entity {
    use buiy_core::layout::{BoxModel, FlexAxis, FlexParams, Length, Sizing};

    let area = h
        .world_mut()
        .spawn((
            ScrollArea,
            BoxModel {
                width: Sizing::Length(Length::Px(328.0)),
                height: Sizing::Length(Length::Px(VIEWPORT_H)),
                ..Default::default()
            },
            FlexParams {
                direction: FlexAxis::Column,
                ..Default::default()
            },
            ScrollList,
            Name::new("ScrollList"),
        ))
        .id();
    let rows: Vec<Entity> = (0..n)
        .map(|i| {
            h.world_mut()
                .spawn((
                    Node,
                    Style::default()
                        .width_px(300.0)
                        .height_px(ROW_H)
                        .min_height(Sizing::Length(Length::Px(ROW_H))),
                    Name::new(format!("ScrollRow{i}")),
                ))
                .id()
        })
        .collect();
    h.world_mut().entity_mut(area).add_children(&rows);
    // Settle: layout → update_scroll_extent (after Layout, before Input).
    for _ in 0..4 {
        h.update();
    }
    area
}

fn offset_y(h: &PointerHarness, e: Entity) -> f32 {
    h.world().get::<ScrollOffset>(e).expect("ScrollOffset").y
}

#[test]
fn s2_pointer_wheel_scrolls_the_long_list_and_clamps_at_both_ends() {
    let mut h = PointerHarness::new();
    let area = spawn_scroll_screen(&mut h, ROWS);

    // The extent is current after layout: 1000 rows × 28px content, 300px viewport.
    let extent = *h.world().get::<ScrollExtent>(area).expect("ScrollExtent");
    assert!(extent.valid, "the extent is valid after the layout pass");
    let max_y = extent.max_offset().y;
    let expected_content = ROWS as f32 * ROW_H;
    assert!(
        (extent.content.y - expected_content).abs() < 1.0,
        "content extent ≈ ROWS × ROW_H ({expected_content}), got {}",
        extent.content.y
    );
    assert!(
        (max_y - (expected_content - VIEWPORT_H)).abs() < 1.0,
        "max offset = content − viewport, got {max_y}"
    );

    // Aim the synthetic pointer at the list and scroll DOWN one notch.
    let center = h.global_center(area);
    h.move_to(center);
    assert_eq!(offset_y(&h, area), 0.0, "starts at the top");

    h.scroll(MouseScrollUnit::Line, 0.0, 1.0); // one wheel-down notch
    let after_one = offset_y(&h, area);
    assert!(
        after_one > 0.0,
        "a wheel-down notch advances the long list's ScrollOffset (got {after_one})"
    );

    // Scroll far past the bottom — clamps to the max (no overshoot at 1000 rows).
    h.scroll(MouseScrollUnit::Line, 0.0, 10_000.0);
    assert_eq!(
        offset_y(&h, area),
        max_y,
        "a large wheel-down clamps at content − viewport (the end of the 1000-row list)"
    );

    // Scroll up past the top — clamps to 0.
    h.scroll(MouseScrollUnit::Line, 0.0, -10_000.0);
    assert_eq!(
        offset_y(&h, area),
        0.0,
        "a large wheel-up clamps to the top (no undershoot)"
    );
}

#[test]
fn s2_keyboard_pagedown_and_end_scroll_the_long_list() {
    let mut h = PointerHarness::new();
    let area = spawn_scroll_screen(&mut h, ROWS);
    let max_y = h.world().get::<ScrollExtent>(area).unwrap().max_offset().y;

    // Focus the container so keyboard scroll targets it (the container owns
    // keyboard scroll — the bevy-ui-widgets lesson the ScrollArea contract follows).
    h.world_mut().resource_mut::<FocusedEntity>().0 = Some(area);

    h.press_key(KeyCode::PageDown);
    let after_page = offset_y(&h, area);
    assert!(
        after_page > 0.0 && after_page <= max_y,
        "PageDown advances ~one viewport into the long list, clamped (got {after_page})"
    );

    h.press_key(KeyCode::End);
    assert_eq!(
        offset_y(&h, area),
        max_y,
        "End jumps to the bottom bound of the 1000-row list"
    );
}

// ---------------------------------------------------------------------------
// S2 SC-4 a11y scroll fields — driven on a full A11yPlugin app (build_tree runs,
// so `snapshot` reflects the live tree incl. the SC-4 scroll fields). The screen
// is spawned via the real `screen_scroll_list` scene-fn (this app HAS ScenePlugin).
// ---------------------------------------------------------------------------

/// A headless app with the a11y tree + layout + scroll + focus + widgets — the
/// build_tree path the in-process driver reads, plus the scroll pipeline (so
/// `update_a11y_scroll` keeps the SC-4 source live) and ScenePlugin (so the
/// screen-fn's `spawn_scene` resolves).
fn scroll_a11y_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.add_plugins(FocusPlugin);
    app.add_plugins(ScrollInputPlugin);
    app.add_plugins(WidgetsPlugin);
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

/// The live `ScrollList` entity (the screen's `ScrollArea`).
fn scroll_list_entity(app: &mut App) -> Entity {
    let mut q = app.world_mut().query_filtered::<Entity, With<ScrollList>>();
    q.single(app.world()).expect("one ScrollList in the screen")
}

#[test]
fn s2_a11y_snapshot_reflects_the_scroll_offset_and_extent() {
    use bevy::scene::WorldSceneExt;
    let mut app = scroll_a11y_app();
    app.world_mut()
        .spawn_scene(screen_scroll_list(ROWS))
        .expect("spawn the scroll-list screen");
    buiy_gallery::fill_scroll_list(app.world_mut(), ROWS);
    // Settle: layout → extent → a11y scroll source → build_tree.
    for _ in 0..6 {
        app.update();
    }
    let area = scroll_list_entity(&mut app);

    // The scroll region is a Group with the accessible name from the scene-fn.
    let region = get_by_role(app.world_mut(), A11yRole::Group, Some("Items"), None)
        .expect("the scroll region is in the a11y tree as a named Group");

    // Before scrolling: the snapshot's SC-4 scroll fields are present (it IS a
    // scroll container) with offset 0 and a non-zero max (the content overflows).
    let before = snapshot(app.world_mut(), Default::default());
    let s0 = before
        .node(region)
        .and_then(|n| n.state.scroll)
        .expect("the scroll region reports SC-4 scroll geometry in the snapshot");
    assert_eq!(s0.y, 0.0, "starts at the top in the a11y tree");
    assert!(
        s0.y_max > 0.0,
        "the 1000-row content overflows ⇒ a non-zero scroll max in the a11y tree (got {})",
        s0.y_max
    );

    // Scroll the focused container to the bottom (End), settle, re-snapshot: the
    // SC-4 fields now report the live offset == the max (the driver observes the
    // scroll position through the SAME tree a real AT reads).
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(area);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::End);
    }
    for _ in 0..3 {
        app.update();
    }
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
    }
    app.update();

    // The live source advanced to the bottom (cross-check the C5 source).
    let live = h_offset(&app, area);
    assert!(live > 0.0, "End scrolled the live offset down (got {live})");

    let after = snapshot(app.world_mut(), Default::default());
    let s1 = after
        .node(region)
        .and_then(|n| n.state.scroll)
        .expect("scroll geometry still present after scrolling");
    assert!(
        (s1.y - live as f64).abs() < 1.0,
        "the a11y snapshot's scroll offset tracks the live offset ({live}), got {}",
        s1.y
    );
    assert!(
        (s1.y - s1.y_max).abs() < 1.0,
        "End put the offset at the scroll max in the a11y tree ({} vs max {})",
        s1.y,
        s1.y_max
    );
}

/// The live `ScrollOffset.y` (cross-checking the a11y snapshot against the source).
fn h_offset(app: &App, e: Entity) -> f32 {
    app.world()
        .get::<ScrollOffset>(e)
        .map(|o| o.y)
        .unwrap_or(0.0)
}

#[test]
fn s2_a11y_source_mirrors_the_live_offset_on_the_pointer_path() {
    // The C5 SC-4 source on the screen's ScrollArea mirrors the live offset after a
    // pointer wheel (the same component the a11y snapshot projects). Driven on the
    // PointerHarness (which carries ScrollInputPlugin incl. `update_a11y_scroll`).
    let mut h = PointerHarness::new();
    let area = spawn_scroll_screen(&mut h, ROWS);
    let center = h.global_center(area);
    h.move_to(center);
    h.scroll(MouseScrollUnit::Line, 0.0, 3.0);
    let off = offset_y(&h, area);
    let a11y = *h
        .world()
        .get::<A11yScroll>(area)
        .expect("SC-4 source present");
    assert_eq!(
        a11y.offset.y, off,
        "A11yScroll mirrors the live ScrollOffset after a wheel"
    );
    assert!(a11y.scrollable, "1000 rows overflow ⇒ scrollable flag set");
}

// ###########################################################################
// S3 — overlay / menu
// ###########################################################################

// ---------------------------------------------------------------------------
// S3 menu open / arrow-nav / activate / Esc-close — driven on a full A11yPlugin +
// WidgetsPlugin + OverlayMenuPlugin app (so the menu's roving keyboard nav, the
// activation recorder, and the a11y snapshot all run). Keys are driven by writing
// `KeyboardInput` Pressed messages (the `menu_keyboard_nav` drain path) — the same
// synthetic-keyboard discipline `todomvc_c8a`'s `send_key` uses. The menu is
// spawned via `MenuButton::new` (the same widget the screen's `menu_button`+`menu`
// scene-fns assemble), each item carrying a `MenuAction` so the recorder logs it.
//
// The outside-press LIGHT-DISMISS (real pointer geometry) stays on the C7
// PointerHarness; the standalone-popover dismiss too.
// ---------------------------------------------------------------------------

use buiy_gallery::MenuAction;

/// A headless app with the a11y tree + focus + layout + widgets + the S3 activation
/// recorder, plus the keyboard infra the menu nav reads. `build_tree` runs so
/// `snapshot` reflects the live tree (incl. `aria-haspopup`).
fn menu_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    app.add_plugins(OverlayMenuPlugin);
    // The menu's roving nav reads `Messages<KeyboardInput>`; the dismiss handler
    // reads `Res<ButtonInput<KeyCode>>`. MinimalPlugins seeds neither.
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

/// Spawn the S3 menu (Edit → Cut/Copy/Paste, each tagged with its `MenuAction`) +
/// the tooltip trigger under a window-sized root; settle the wiring. Returns
/// `(button, menu, [items], tooltip)`.
fn spawn_menu(app: &mut App) -> (Entity, Entity, Vec<Entity>, Entity) {
    let world = app.world_mut();
    let button = world
        .spawn(MenuButton::new(
            "Edit",
            children![
                (MenuItem::new("Cut"), MenuAction(0)),
                (MenuItem::new("Copy"), MenuAction(1)),
                (MenuItem::new("Paste"), MenuAction(2)),
            ],
        ))
        .id();
    let tooltip = world.spawn(TooltipTrigger::new("?", "More info here")).id();
    let root = world
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    world.entity_mut(root).add_children(&[button, tooltip]);
    for _ in 0..4 {
        app.update();
    }
    let menu = child_menu(app.world(), button);
    let items = menu_items(app.world(), menu);
    (button, menu, items, tooltip)
}

fn child_menu(world: &World, button: Entity) -> Entity {
    world
        .get::<Children>(button)
        .expect("button has children")
        .iter()
        .find(|&c| world.get::<Menu>(c).is_some())
        .expect("button has a Menu child")
}

fn menu_items(world: &World, menu: Entity) -> Vec<Entity> {
    world
        .get::<Children>(menu)
        .map(|c| {
            c.iter()
                .filter(|&e| world.get::<MenuItem>(e).is_some())
                .collect()
        })
        .unwrap_or_default()
}

fn menu_open(world: &World, menu: Entity) -> bool {
    is_open(world.get::<CssVisibility>(menu))
}

fn expanded(world: &World, button: Entity) -> bool {
    world
        .get::<A11yExpanded>(button)
        .map(|e| e.0)
        .unwrap_or(false)
}

fn active_descendant(world: &World, menu: Entity) -> Option<Entity> {
    world
        .get::<A11yRelations>(menu)
        .and_then(|r| r.active_descendant)
}

fn activations(world: &World) -> Vec<String> {
    world.resource::<MenuActivations>().0.clone()
}

/// Open the menu via the shared `OnPress` activation sink (the route the pointer /
/// Button keymap / AT-`Click` paths converge on), then settle the open lifecycle.
fn open_menu(app: &mut App, button: Entity) {
    app.world_mut().write_message(OnPress(button));
    for _ in 0..6 {
        app.update();
    }
}

/// Write a synthetic `KeyboardInput` Pressed message + update — the `menu_keyboard_nav`
/// drain path (ArrowDown/Enter/Escape). Mirrors `todomvc_c8a::send_key`.
fn send_key(app: &mut App, key: KeyCode) {
    let window = app.world_mut().spawn(()).id();
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: bevy::input::keyboard::Key::Character("x".into()),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
}

#[test]
fn s3_activating_the_menu_button_opens_the_menu_with_haspopup_and_expanded() {
    let mut app = menu_app();
    let (button, menu, items, _tip) = spawn_menu(&mut app);
    assert!(!menu_open(app.world(), menu), "the menu starts closed");

    open_menu(&mut app, button);

    assert!(
        menu_open(app.world(), menu),
        "the button activation opened the menu"
    );
    assert!(
        expanded(app.world(), button),
        "the button's A11yExpanded is true while open (aria-expanded)"
    );
    assert!(
        app.world()
            .resource::<TopLayerActivation>()
            .order
            .contains(&menu),
        "the open menu joined the TopLayerActivation deque"
    );
    // The button advertises A11yHasPopup(Menu) in the a11y snapshot.
    let snap = snapshot(app.world_mut(), Default::default());
    let btn_node = snap
        .by_role(A11yRole::Button)
        .find(|n| n.name == "Edit")
        .expect("the MenuButton is in the a11y tree");
    assert_eq!(
        btn_node.state.has_popup,
        Some(accesskit::HasPopup::Menu),
        "the MenuButton advertises aria-haspopup=menu in the snapshot"
    );
    // The menu container holds focus + the first item is the active descendant.
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(menu),
        "the menu container holds focus on open (roving / activedescendant)"
    );
    assert_eq!(
        active_descendant(app.world(), menu),
        items.first().copied(),
        "the first item is the active descendant on open"
    );
}

#[test]
fn s3_arrowdown_moves_the_active_descendant_across_items() {
    let mut app = menu_app();
    let (button, menu, items, _tip) = spawn_menu(&mut app);
    open_menu(&mut app, button);
    assert_eq!(active_descendant(app.world(), menu), items.first().copied());

    send_key(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        active_descendant(app.world(), menu),
        items.get(1).copied(),
        "ArrowDown moves active_descendant Cut -> Copy"
    );
    send_key(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        active_descendant(app.world(), menu),
        items.get(2).copied(),
        "ArrowDown moves active_descendant Copy -> Paste"
    );
    // Wraps past the last back to the first.
    send_key(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        active_descendant(app.world(), menu),
        items.first().copied(),
        "ArrowDown wraps Paste -> Cut"
    );
}

#[test]
fn s3_enter_activates_the_active_item_and_closes_the_menu() {
    let mut app = menu_app();
    let (button, menu, _items, _tip) = spawn_menu(&mut app);
    open_menu(&mut app, button);
    assert!(
        activations(app.world()).is_empty(),
        "no activation before Enter"
    );

    // Move to the second item (Copy), then Enter: its OnPress fires (the
    // MenuActivations effect) AND the menu closes.
    send_key(&mut app, KeyCode::ArrowDown); // active = Copy (index 1)
    send_key(&mut app, KeyCode::Enter);
    for _ in 0..3 {
        app.update();
    }

    assert_eq!(
        activations(app.world()),
        vec![MENU_ITEM_LABELS[1].to_string()],
        "Enter on the active item fired its OnPress -> the observable MenuActivations effect"
    );
    assert!(
        !menu_open(app.world(), menu),
        "Enter activates AND closes the menu (APG menu behavior)"
    );
    assert!(
        !expanded(app.world(), button),
        "the close reconciled the button's A11yExpanded back to false"
    );
}

#[test]
fn s3_escape_closes_the_menu_and_collapses_the_button() {
    let mut app = menu_app();
    let (button, menu, _items, _tip) = spawn_menu(&mut app);
    open_menu(&mut app, button);
    assert!(menu_open(app.world(), menu), "the menu starts open");

    send_key(&mut app, KeyCode::Escape); // menu_keyboard_nav closes the focused open menu
    for _ in 0..3 {
        app.update();
    }

    assert!(!menu_open(app.world(), menu), "Escape closed the menu");
    assert!(
        !expanded(app.world(), button),
        "Escape reconciled the button's A11yExpanded back to false (re-open works)"
    );
}

// ---------------------------------------------------------------------------
// S3 outside-press light-dismiss — real pointer geometry on the C7 PointerHarness.
// ---------------------------------------------------------------------------

#[test]
fn s3_outside_press_light_dismisses_the_open_menu() {
    let mut h = PointerHarness::new();
    // Spawn the menu via the same widget (the harness has no recorder/ScenePlugin,
    // but the dismiss test asserts visibility + expanded, not the activation effect).
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
    for _ in 0..3 {
        h.update();
    }
    let menu = h
        .world()
        .get::<Children>(button)
        .unwrap()
        .iter()
        .find(|&c| h.world().get::<Menu>(c).is_some())
        .expect("the menu child");

    // Open it (the shared OnPress sink) + settle the top-layer registration.
    h.world_mut().write_message(OnPress(button));
    for _ in 0..6 {
        h.update();
    }
    assert!(
        is_open(h.world().get::<CssVisibility>(menu)),
        "the menu starts open"
    );
    assert!(
        h.world()
            .resource::<TopLayerActivation>()
            .order
            .contains(&menu),
        "the open menu joined the TopLayerActivation deque (the dismiss handlers find it)"
    );

    // Press far from the menu + button (inside the window => hits the root, an
    // outside target).
    h.move_to(Vec2::new(700.0, 560.0));
    h.press(PointerButton::Primary);
    h.release(PointerButton::Primary);
    for _ in 0..2 {
        h.update();
    }

    assert!(
        !is_open(h.world().get::<CssVisibility>(menu)),
        "a press outside the open menu light-dismisses it (the menu IS a Popover)"
    );
    assert!(
        h.world().get::<A11yExpanded>(button).map(|e| e.0) == Some(false),
        "the light-dismiss reconciled the button's A11yExpanded back to false"
    );
}

// ---------------------------------------------------------------------------
// S3 tooltip — show through the in-process driver + placement; popover dismiss.
// ---------------------------------------------------------------------------

/// A full A11yPlugin app with the overlay screen spawned via `spawn_overlay_menu`
/// (it has ScenePlugin), so the driver's `get_by_role`/`show_tooltip` read +
/// drive the live tree, and `position_tooltip`/`position_popover` place the
/// overlays. Returns the standalone popover entity.
fn overlay_a11y_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    app.add_plugins(OverlayMenuPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    let popover = spawn_overlay_menu(app.world_mut());
    for _ in 0..6 {
        app.update();
    }
    (app, popover)
}

fn tooltip_node_of(world: &World, trigger: Entity) -> Entity {
    world
        .get::<Children>(trigger)
        .expect("trigger has children")
        .iter()
        .find(|&c| world.get::<TooltipNode>(c).is_some())
        .expect("the trigger's TooltipNode child")
}

#[test]
fn s3_driver_show_tooltip_reveals_and_places_the_tooltip() {
    let (mut app, _popover) = overlay_a11y_app();

    // Address the tooltip trigger through the driver (the only Generic-role node
    // with the "?" name -- the TooltipTrigger keeps a neutral role).
    let trigger_id = get_by_role(app.world_mut(), A11yRole::Generic, Some("?"), None)
        .expect("the tooltip trigger is addressable by role+name");
    let trigger = buiy_core::a11y::translate::entity_for_node_id(trigger_id).unwrap();
    let tip = tooltip_node_of(app.world(), trigger);

    // Before: the tooltip node is hidden.
    assert_eq!(
        app.world().get::<CssVisibility>(tip).copied(),
        Some(CssVisibility::Hidden),
        "the tooltip starts hidden"
    );

    // Driver ShowTooltip -> the router's generic honor flips the tooltip's
    // CssVisibility to Visible (observed on the live component the AT controls).
    show_tooltip(app.world_mut(), trigger_id).expect("ShowTooltip honored");
    app.update();
    assert!(
        is_open(app.world().get::<CssVisibility>(tip)),
        "the driver ShowTooltip revealed the tooltip node (got {:?})",
        app.world().get::<CssVisibility>(tip).copied()
    );

    // Placement: `position_tooltip` wired a non-empty Anchor chain anchored to the
    // trigger (below-then-above) -- the tooltip is placed, not stranded at origin.
    let anchor = app
        .world()
        .get::<Anchor>(tip)
        .expect("the tooltip carries an Anchor (positioned)");
    assert!(
        !anchor.position_try.is_empty(),
        "position_tooltip wired the placement fallback chain (below-then-above)"
    );
    assert!(
        anchor.position_anchor.is_some(),
        "the tooltip is anchored to its trigger"
    );
}

#[test]
fn s3_outside_press_light_dismisses_the_anchored_popover() {
    // The standalone anchored popover (S3's third overlay): open it, then an
    // outside press light-dismisses it; an inside press keeps it open. Driven on
    // the PointerHarness over the production dismiss path.
    let mut h = PointerHarness::new();
    // Spawn a trigger + an OPEN anchored popover well clear of the trigger (the
    // overlay_dismiss_c5b recipe -- the same Popover the screen authors).
    let trigger = h
        .world_mut()
        .spawn((Node, Style::default().width_px(60.0).height_px(30.0)))
        .id();
    let popover = h
        .world_mut()
        .spawn((
            Style::default().width_px(160.0).height_px(80.0),
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
            Name::new("InfoPopover"),
        ))
        .id();
    let root = h
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    h.world_mut()
        .entity_mut(root)
        .add_children(&[trigger, popover]);
    for _ in 0..6 {
        h.update();
    }
    assert!(
        is_open(h.world().get::<CssVisibility>(popover)),
        "popover starts open"
    );

    // Inside press first -- must NOT dismiss.
    let center = h.global_center(popover);
    h.move_to(center);
    h.press(PointerButton::Primary);
    h.release(PointerButton::Primary);
    assert!(
        is_open(h.world().get::<CssVisibility>(popover)),
        "a press inside the popover does NOT dismiss it"
    );

    // Outside press -- light-dismisses.
    h.move_to(Vec2::new(700.0, 560.0));
    h.press(PointerButton::Primary);
    h.release(PointerButton::Primary);
    assert_eq!(
        h.world().get::<CssVisibility>(popover).copied(),
        Some(CssVisibility::Hidden),
        "a press outside the open anchored popover light-dismisses it"
    );
}
