//! Headless layout-snapshot gate for the S4 modal screen (Tier 1 of the
//! `buiy_verify` pyramid — no GPU, no window). Drives the **same** `spawn_modal`
//! tree the binary authors (the background button + invoker + the closed C5-d
//! `Dialog`), then pins the resolved layout of every `#Name`-tagged entity. A
//! structural regression (a dropped dialog control, a lost invoker, a wrong panel
//! box) shows as a `.snap` diff.
//!
//! The dialog starts **closed** (`CssVisibility::Hidden`), so the resting layout is
//! what is pinned. The open/trap/Esc/inert BEHAVIOR (invoker opens, Tab traps +
//! wraps, Escape closes + restores, background pruned) is the inspection-driver
//! acceptance in `crates/buiy_verify/tests/verify_headless/modal_showcase_c8c.rs`;
//! this gate pins only the resting structure.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::scene::ScenePlugin;
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::focus::FocusPlugin;
use buiy_gallery::spawn_modal;
use buiy_verify::snapshot::assert_layout_snapshot;

#[test]
fn modal_screen_lays_out_as_expected() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        // The Dialog's C5-d focus-trap rides `FocusPlugin`'s `handle_tab`, which
        // reads `Res<ButtonInput<KeyCode>>` (non-optional); seed it so the resting
        // (no-key) layout settles without the resource-missing panic.
        .add_plugins(FocusPlugin)
        .add_plugins(WidgetsPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();

    // Spawn the modal screen (background button + invoker + the closed dialog) the
    // same way the binary does via `setup_modal`. The dialog starts closed.
    spawn_modal(app.world_mut());

    assert_layout_snapshot(&mut app, "modal_screen");
}
