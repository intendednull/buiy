//! Headless layout-snapshot gate for the S2 long-list screen (Tier 1 of the
//! `buiy_verify` pyramid — no GPU, no window). Drives the **same**
//! `screen_scroll_list` tree the binary authors + seeds a small row set, then pins
//! the resolved layout of every `#Name`-tagged entity. A structural regression (a
//! dropped child, a wrong viewport box, a lost `ScrollArea` require) shows as a
//! `.snap` diff.
//!
//! The snapshot uses a SMALL row count (the structure is what is pinned — the card,
//! the heading, the `ScrollArea` viewport, and a few rows). The 1000-row
//! scale-game is exercised in the inspection-driver acceptance
//! (`crates/buiy_verify/tests/verify_headless/scroll_overlay_c8b.rs`), where the
//! clamp + a11y scroll fields are asserted, not pinned as a giant dump.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::{ScenePlugin, WorldSceneExt};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_gallery::{fill_scroll_list, screen_scroll_list};
use buiy_verify::snapshot::assert_layout_snapshot;

/// The row count the snapshot pins (small + reviewable; the scale-game is in the
/// driver-acceptance test, not here).
const SNAPSHOT_ROWS: usize = 6;

#[test]
fn scroll_list_screen_lays_out_as_expected() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    // Author the static screen, then seed a small row set (rows are dynamic — the
    // binary seeds 1000 the same way via `fill_scroll_list`).
    app.world_mut()
        .spawn_scene(screen_scroll_list(SNAPSHOT_ROWS))
        .expect("spawn the scroll-list screen");
    fill_scroll_list(app.world_mut(), SNAPSHOT_ROWS);

    assert_layout_snapshot(&mut app, "scroll_list_screen");
}
