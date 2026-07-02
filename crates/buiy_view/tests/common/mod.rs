//! Shared headless harness for the `buiy_view` logic tests (no GPU).
#![allow(dead_code)]

use bevy::prelude::*;
use buiy_core::interaction::OnPress;

/// The logic-plugin subset (everything the surface needs except the GPU render
/// plugin) — mirrors the `hello_button` MVU logic test. MVU scaffolding
/// (`MvuCorePlugin`) rides in with `WidgetsPlugin`.
pub fn logic_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            buiy_core::CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            buiy_widgets::WidgetsPlugin,
        ));
    app
}

/// Enough frames for the seed reconcile (frame 1, before Layout) to build the
/// tree from the `Startup`-spawned model.
pub fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

/// Synthesize a real `OnPress` on `target` and drive it through the funnel.
///
/// Two updates because the reconciler runs **`.before(BuiySet::Layout)`** (#10):
/// frame N routes + drains (the model changes), frame N+1's front-of-frame
/// reconcile reads that `Changed<M>` and patches the derived tree.
pub fn press(app: &mut App, target: Entity) {
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(target));
    app.update(); // frame N: route(Enqueue) → drain(Drain) — model changes
    app.update(); // frame N+1: reconcile(before Layout) patches the derived tree
}
