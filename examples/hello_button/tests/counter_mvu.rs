//! Headless **logic** verification for the MVU counter (no GPU).
//!
//! Drives the real MVU schedule (CorePlugin → WidgetsPlugin → MvuCorePlugin →
//! CounterPlugin) with synthesized `OnPress` and asserts both the model fold AND
//! the projected label `Text`. This is the portable correctness gate; the GPU
//! `capture_counter` bin is the visual one.
//!
//! DX probe: an app author who wants to unit-test an MVU feature must hand-compose
//! the logic-plugin subset (everything BuiyHeadlessPlugin has *except*
//! BuiyRenderPlugin, which needs a render backend). There is no ready-made
//! "headless logic app" builder for app authors.

use bevy::prelude::*;
use buiy::{Button, CorePlugin, OnPress, Text, WidgetsPlugin};
use hello_button::{Counter, CounterPlugin, DecButton, IncButton, ResetButton};

fn logic_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        // The logic-plugin subset (no BuiyRenderPlugin — no GPU needed).
        .add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            WidgetsPlugin,
        ))
        .add_plugins(CounterPlugin);
    app
}

fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

fn value(app: &mut App) -> i64 {
    app.world_mut()
        .query::<&Counter>()
        .iter(app.world())
        .next()
        .expect("counter exists")
        .value
}

/// The label `Text` projected by the bind (proves the View half, not just the model).
fn label(app: &mut App) -> String {
    let mut q = app.world_mut().query::<(&Text, Option<&IncButton>)>();
    // The label is the only Text that is NOT on a button; find it by elimination:
    // collect all Text, return the one whose value parses as the count. Simplest:
    // the label text equals the current counter value rendered.
    let want = value(app).to_string();
    let world = app.world();
    q.iter(world)
        .map(|(t, _)| t.0.clone())
        .find(|s| *s == want)
        .unwrap_or_default()
}

fn press(app: &mut App, target: Entity) {
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(target));
    app.update(); // route(Enqueue) → ApplyDeferred → drain(Drain) → bind(Bind)
}

fn button<M: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, (With<Button>, With<M>)>()
        .iter(app.world())
        .next()
        .expect("button exists")
}

#[test]
fn counter_starts_at_zero_and_labels_zero() {
    let mut app = logic_app();
    settle(&mut app);
    assert_eq!(value(&mut app), 0);
    assert_eq!(label(&mut app), "0", "the bind projected the initial 0");
}

#[test]
fn increment_decrement_reset_fold_and_project() {
    let mut app = logic_app();
    settle(&mut app);
    let inc = button::<IncButton>(&mut app);
    let dec = button::<DecButton>(&mut app);
    let reset = button::<ResetButton>(&mut app);

    press(&mut app, inc);
    assert_eq!(value(&mut app), 1, "+ folded to 1");
    assert_eq!(label(&mut app), "1", "+ projected to label");

    press(&mut app, inc);
    press(&mut app, inc);
    assert_eq!(value(&mut app), 3);
    assert_eq!(label(&mut app), "3");

    press(&mut app, dec);
    assert_eq!(value(&mut app), 2, "- folded");
    assert_eq!(label(&mut app), "2");

    press(&mut app, reset);
    assert_eq!(value(&mut app), 0, "reset folded");
    assert_eq!(label(&mut app), "0");
}

#[test]
fn unrelated_press_does_not_fold() {
    let mut app = logic_app();
    settle(&mut app);
    let bogus = app.world_mut().spawn_empty().id();
    press(&mut app, bogus);
    assert_eq!(
        value(&mut app),
        0,
        "a press on a non-counter button is inert"
    );
}
