//! Track A — the agent probe / "eyes." Proves the `BuiyProbePlugin` preset stands
//! up a Buiy scene **headless + GPU-free** (no wgpu adapter, no window, no
//! `RenderApp`) and that the agent-facing feedback loop — `snapshot` +
//! `snapshot_report` — reads it back. These tests RUN in the ordinary headless
//! gate; there is nothing to `#[ignore]` because the probe never rasterizes.

use bevy::prelude::*;
use buiy::{BuiyProbePlugin, Button};
use buiy_core::a11y::A11yRole;
use buiy_core::a11y::inprocess::{TreeView, snapshot};

/// The minimal GPU-free substrate `BuiyProbePlugin` documents: `MinimalPlugins`
/// (no render/winit), `AssetPlugin` (the text stack's fallback font), and
/// `InputPlugin` (focus/keymap read `Res<ButtonInput<KeyCode>>`).
fn probe_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(BuiyProbePlugin);
    app
}

/// Wave 1 — the preset composes a working GPU-free stack: a spawned `Button` lays
/// out and projects an `A11yRole::Button` node named "Save" through the shipped
/// in-process `snapshot`, with **no camera, no window, no adapter**.
#[test]
fn probe_snapshot_sees_button_headless_gpu_free() {
    let mut app = probe_app();
    app.world_mut().spawn(Button::new("Save"));

    // Layout + a11y build + text shaping settle within a few frames.
    for _ in 0..8 {
        app.update();
    }

    let tree = snapshot(app.world_mut(), TreeView::Unmerged);
    assert!(
        tree.by_role(A11yRole::Button).any(|n| n.name == "Save"),
        "probe snapshot must expose the Button node named \"Save\"; tree = {tree:#?}",
    );
}
