//! Headless layout-snapshot gate for the S1 TodoMVC screen (Tier 1 of the
//! `buiy_verify` pyramid — no GPU, no window). Drives the **same**
//! `screen_todomvc` tree the binary authors + seeds the demo rows, then pins the
//! resolved layout of every `#Name`-tagged entity. A structural regression (a
//! dropped child, a lost merge, a wrong box) shows as a `.snap` diff.
//!
//! This is the "example IS the fixture" discipline applied to S1: the screen is
//! authored once (`buiy_gallery::screen_todomvc`) and both the runnable binary
//! and this gate spawn the exact same tree. Matrix enrollment of screen fixtures
//! (the reduced `Matrix::gallery_screen()`) is a later C8 slice; this dedicated
//! scene-based snapshot covers S1's layout structure without modifying the
//! coverage `build_app` (which has no `ScenePlugin`).

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::{ScenePlugin, WorldSceneExt};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_gallery::{DEMO_SEEDS, append_row, screen_todomvc};
use buiy_verify::snapshot::assert_layout_snapshot;

#[test]
fn todomvc_screen_lays_out_as_expected() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    // Author the static screen, then seed the demo rows imperatively (rows are
    // dynamic — the binary seeds them in `setup` the same way).
    app.world_mut()
        .spawn_scene(screen_todomvc(DEMO_SEEDS))
        .expect("spawn the todomvc screen");
    for &(label, completed) in DEMO_SEEDS {
        append_row(app.world_mut(), label, completed);
    }

    assert_layout_snapshot(&mut app, "todomvc_screen");
}
