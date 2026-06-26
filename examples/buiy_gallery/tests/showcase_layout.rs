//! Headless layout-snapshot gate for the S5 Controls showcase screen (Tier 1 of
//! the `buiy_verify` pyramid — no GPU, no window). Drives the **same**
//! `spawn_showcase` tree the binary authors (the design's 2-column controls grid —
//! the Switch / Slider+preview / Segmented+Stepper / Meter+Run-build cards + the
//! full-width Disclosure accordion), then pins the resolved layout of every
//! `#Name`-tagged entity. A structural regression (a dropped widget, a wrong card
//! box, a lost grid span) shows as a `.snap` diff.
//!
//! This gate pins the resting LAYOUT. The PAINT — the slider preview's gradient +
//! glow, the cards' border bands, a keyboard-focused widget's focus-ring `Outline`
//! — is observed at the display-list / extract tier in the inspection-driver
//! acceptance (`crates/buiy_verify/tests/verify_headless/modal_showcase_c8c.rs`),
//! where each widget's function (toggle / value / expand) is also driven; layout
//! cannot observe paint membership, so it is asserted there, not pinned here.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_gallery::spawn_showcase;
use buiy_verify::snapshot::assert_layout_snapshot;

#[test]
fn showcase_screen_lays_out_as_expected() {
    // No `FocusPlugin` (this gate pins the resting layout, not the focus ring — the
    // ring is observed at the extract tier in `modal_showcase_c8c.rs`), mirroring
    // the C8-b layout gates' plugin set. `ScenePlugin` stays — the C2 composites
    // (segmented/stepper/meter/search) the showcase reuses build their text-input
    // fields via `spawn_scene`.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    spawn_showcase(app.world_mut());

    assert_layout_snapshot(&mut app, "showcase_screen");
}
