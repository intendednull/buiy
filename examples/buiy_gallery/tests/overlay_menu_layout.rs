//! Headless layout-snapshot gate for the S3 overlay/menu screen (Tier 1 of the
//! `buiy_verify` pyramid — no GPU, no window). Drives the **same**
//! `spawn_overlay_menu` tree the binary authors (the file card + ⋮ menu + footer,
//! plus the tooltip + standalone popover primitives), then pins the resolved
//! layout of every `#Name`-tagged entity. A structural regression (a dropped
//! trigger, a lost menu child, a wrong card box) shows as a `.snap` diff.
//!
//! The open/positioned/dismiss BEHAVIOR (menu open, arrow-nav, activate, Esc /
//! outside-press close, tooltip show + placement, popover light-dismiss) is the
//! inspection-driver acceptance in
//! `crates/buiy_verify/tests/verify_headless/scroll_overlay_c8b.rs`; this gate pins
//! only the resting (closed) layout structure.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::prelude::Entity;
use bevy::scene::ScenePlugin;
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::interaction::OnPress;
use buiy_core::text::Text;
use buiy_gallery::{
    MENU_ITEM_LABELS, MENU_NO_ACTION, MenuAction, MenuActivations, MenuLastActionField,
    OverlayMenuPlugin, spawn_overlay_menu,
};
use buiy_verify::snapshot::assert_layout_snapshot;

#[test]
fn overlay_menu_screen_lays_out_as_expected() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    // Spawn the screen + the standalone anchored popover (the binary spawns the
    // same tree via `setup_overlay_menu`). The menu + tooltip + popover all start
    // closed/hidden, so the resting layout is what is pinned.
    spawn_overlay_menu(app.world_mut());

    assert_layout_snapshot(&mut app, "overlay_menu_screen");
}

/// Activating a menu item (its shared `OnPress` sink — the route the menu's
/// keyboard Enter / a pointer click converge on) records the observable
/// `MenuActivations` effect **and** rewrites the footer "last action" value
/// (`MenuLastActionField`). The C8b acceptance drives the full open→roving→Enter
/// path through the a11y driver; this gate pins the app-logic grounding loop
/// (`OverlayMenuPlugin`'s `record_menu_activation` + `update_last_action`) in
/// isolation, so the footer-update contract has a fast headless guard.
#[test]
fn activating_a_menu_item_records_it_and_updates_the_last_action_footer() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin)
        .add_plugins(OverlayMenuPlugin);

    spawn_overlay_menu(app.world_mut());
    app.update();

    // At rest the footer reads "—" (the design's `lastAction: '—'`).
    assert_eq!(
        last_action_text(&mut app),
        MENU_NO_ACTION,
        "the footer starts at the no-action placeholder"
    );
    assert!(
        app.world().resource::<MenuActivations>().0.is_empty(),
        "no activation recorded before any item fires"
    );

    // Fire the "Duplicate" item (index 2) through the shared OnPress sink.
    let item = menu_item_with_action(&mut app, 2);
    app.world_mut().write_message(OnPress(item));
    app.update();

    assert_eq!(
        app.world().resource::<MenuActivations>().0,
        vec![MENU_ITEM_LABELS[2].to_string()],
        "the activation appended the item's label to the MenuActivations effect log"
    );
    assert_eq!(
        last_action_text(&mut app),
        MENU_ITEM_LABELS[2],
        "the footer 'last action' value updated to the activated item's label"
    );
}

/// The current text of the footer `MenuLastActionField` value leaf.
fn last_action_text(app: &mut App) -> String {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&Text, bevy::prelude::With<MenuLastActionField>>();
    q.single(world)
        .expect("the footer last-action value leaf exists")
        .0
        .clone()
}

/// The menu-item entity carrying `MenuAction(idx)`.
fn menu_item_with_action(app: &mut App, idx: usize) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &MenuAction)>();
    q.iter(world)
        .find(|(_, a)| a.0 == idx)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("a menu item carries MenuAction({idx})"))
}
