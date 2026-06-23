//! Headless layout-snapshot gate for the S5 F-tier showcase screen (Tier 1 of the
//! `buiy_verify` pyramid — no GPU, no window). Drives the **same**
//! `screen_showcase` tree the binary authors (the styled `#ShowcaseCard` holding a
//! `Switch` + `Slider` + `Disclosure`), then pins the resolved layout of every
//! `#Name`-tagged entity. A structural regression (a dropped widget, a wrong card
//! box, a lost border-width) shows as a `.snap` diff.
//!
//! This gate pins the resting LAYOUT. The F-tier PAINT — the card's shadow + border
//! bands + a keyboard-focused widget's focus-ring `Outline` — is observed at the
//! display-list / extract tier in the inspection-driver acceptance
//! (`crates/buiy_verify/tests/verify_headless/modal_showcase_c8c.rs`), where each
//! widget's function (toggle / increment / expand) is also driven; layout cannot
//! observe paint membership, so it is asserted there, not pinned here.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::{ScenePlugin, WorldSceneExt};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_gallery::screen_showcase;
use buiy_verify::snapshot::assert_layout_snapshot;

#[test]
fn showcase_screen_lays_out_as_expected() {
    // No `FocusPlugin` (this gate pins the resting layout, not the focus ring — the
    // ring is observed at the extract tier in `modal_showcase_c8c.rs`), mirroring
    // the C8-b layout gates' plugin set.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    app.world_mut()
        .spawn_scene(screen_showcase())
        .expect("spawn the showcase screen");

    assert_layout_snapshot(&mut app, "showcase_screen");
}
