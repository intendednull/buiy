//! Track C / C3 — the typed `ValueChange<T>` notifications. Drives real widgets
//! through the shipped funnel under the GPU-free `BuiyProbePlugin` and asserts the
//! typed events: a checkbox toggle emits `ValueChange<bool>`, a slider value
//! change emits `ValueChange<f64>`, and neither fires on spawn (a `ValueChange` is
//! a change, not the initial value).

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use buiy::BuiyProbePlugin;
use buiy::prelude::*;

fn probe_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(BuiyProbePlugin);
    app
}

fn drain<M: Message + Clone>(app: &mut App) -> Vec<M> {
    app.world_mut()
        .resource_mut::<Messages<M>>()
        .drain()
        .collect()
}

#[test]
fn checkbox_toggle_emits_typed_value_change_but_not_on_spawn() {
    let mut app = probe_app();
    let cb = app.world_mut().spawn(Checkbox::new("Dark mode")).id();

    // Settle: the initial `A11yToggled` insertion must NOT emit — a `ValueChange`
    // is a *change*, not the starting value (`Ref::is_added` skip).
    for _ in 0..4 {
        app.update();
    }
    assert!(
        drain::<ValueChange<bool>>(&mut app).is_empty(),
        "spawning a checkbox must not emit a ValueChange (initial value is not a change)",
    );

    // Activate via the shared `OnPress` sink → the toggle funnel commits
    // `A11yToggled` (False → True) → the post-drain emitter writes the typed event.
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(cb));
    app.update();

    assert_eq!(
        drain::<ValueChange<bool>>(&mut app),
        vec![ValueChange {
            source: cb,
            value: true,
            is_final: true,
        }],
        "toggling the checkbox must emit exactly one ValueChange<bool>{{true}}",
    );
}

#[test]
fn slider_value_change_emits_typed_f64() {
    let mut app = probe_app();
    let slider = app
        .world_mut()
        .spawn(Slider::new("Volume", 0.0, 0.0, 1.0, 0.1))
        .id();

    for _ in 0..4 {
        app.update();
    }
    assert!(
        drain::<ValueChange<f64>>(&mut app).is_empty(),
        "spawning a slider must not emit a ValueChange",
    );

    // The slider's `A11yValue` is written directly (not funnel-routed); the
    // `Changed<A11yValue>` emitter picks it up. Compare against the *committed*
    // value so the test is robust to any step-snapping in `set_now`.
    app.world_mut()
        .get_mut::<A11yValue>(slider)
        .expect("slider carries A11yValue")
        .set_now(0.5);
    app.update();

    let committed = app.world().get::<A11yValue>(slider).unwrap().now;
    assert_eq!(
        drain::<ValueChange<f64>>(&mut app),
        vec![ValueChange {
            source: slider,
            value: committed,
            is_final: true,
        }],
        "changing the slider value must emit exactly one ValueChange<f64> of the committed value",
    );
}
