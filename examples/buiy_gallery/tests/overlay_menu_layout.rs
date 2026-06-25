//! Headless layout-snapshot gate for the S3 overlay/menu screen (Tier 1 of the
//! `buiy_verify` pyramid — no GPU, no window). Drives the **same**
//! `screen_overlay_menu` tree the binary authors (+ the standalone popover), then
//! pins the resolved layout of every `#Name`-tagged entity. A structural
//! regression (a dropped trigger, a lost menu child, a wrong card box) shows as a
//! `.snap` diff.
//!
//! The open/positioned/dismiss BEHAVIOR (menu open, arrow-nav, activate, Esc /
//! outside-press close, tooltip show + placement, popover light-dismiss) is the
//! inspection-driver acceptance in
//! `crates/buiy_verify/tests/verify_headless/scroll_overlay_c8b.rs`; this gate pins
//! only the resting (closed) layout structure.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_gallery::spawn_overlay_menu;
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
