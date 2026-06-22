//! Headless gate for `hello_bsn` (Tier 1 of the `buiy_verify` pyramid — a
//! layout snapshot, no GPU). Drives the **same** `bsn!` tree the example
//! authors (`hello_bsn::hello_bsn_scene`) and pins the resolved layout of every
//! entity, proving the BSN-authored tree produces the expected entity/layout
//! structure headlessly.
//!
//! This is the example's gate: a structural regression in the authored tree (a
//! dropped child, a lost merge, a wrong box) shows as a `.snap` diff. It runs
//! without a window or adapter, so it stays green on the headless CI lane.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::{ScenePlugin, WorldSceneExt};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_verify::snapshot::assert_layout_snapshot;
use hello_bsn::hello_bsn_scene;

#[test]
fn hello_bsn_tree_lays_out_as_expected() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // `spawn_scene` resolves through the asset registry + `Assets<ScenePatch>`.
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        // Layout + the widget/text plugins that register the scene-fn
        // required-components before the spawn.
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    // Author the example's tree synchronously (the `world.spawn_scene` twin of
    // the binary's `commands.spawn_scene`), then snapshot the resolved layout.
    app.world_mut()
        .spawn_scene(hello_bsn_scene())
        .expect("spawn the hello_bsn scene");

    // `assert_layout_snapshot` runs one update (drives layout) and dumps every
    // `ResolvedLayout` box, keyed by the `#Name`s the tree assigns.
    assert_layout_snapshot(&mut app, "hello_bsn_tree");
}
