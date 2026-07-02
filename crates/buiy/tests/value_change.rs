//! Track C / C3 — the typed `ValueChange<T>` notifications. Drives real widgets
//! through the shipped funnel under the GPU-free `BuiyProbePlugin` and asserts the
//! typed events: a checkbox toggle emits `ValueChange<bool>`, a slider value
//! change emits `ValueChange<f64>`, neither fires on spawn, and a *saturated
//! no-op* write emits nothing (the `set_if_neq` change-honesty guard).

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use buiy::BuiyProbePlugin;
use buiy::prelude::*;
use buiy::probe::{get_by_role, increment};

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

    // Load-bearing spawn check: drain after a SINGLE update — the frame the initial
    // `A11yToggled` is inserted (and where `Ref::is_added` is consumed). If that
    // skip regressed, the spurious spawn emission would still be in `Messages` here.
    app.update();
    assert!(
        drain::<ValueChange<bool>>(&mut app).is_empty(),
        "the spawn frame must not emit a ValueChange (initial value is not a change)",
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

    app.update();
    assert!(
        drain::<ValueChange<f64>>(&mut app).is_empty(),
        "the spawn frame must not emit a ValueChange",
    );

    // A real change (0.0 → 0.5) trips `Changed<A11yValue>`; the emitter reports the
    // committed value (compared against the stored `now` to be robust to snapping).
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

/// Regression (C3 gate finding #1): a value verb that *saturates* — an AT
/// `Increment` on a slider already at its maximum — commits no change, so it must
/// emit NOTHING. The `A11yValue` honor now commits via `set_if_neq`, so the no-op
/// does not trip `Changed<A11yValue>` (before the fix, the `&mut` mutator tripped
/// `Changed` on the clamped no-op and fired a phantom `ValueChange<f64>`).
#[test]
fn saturated_increment_emits_no_value_change() {
    let mut app = probe_app();
    // Spawn already at the maximum (now == max == 1.0).
    app.world_mut()
        .spawn(Slider::new("Volume", 1.0, 0.0, 1.0, 0.1));
    for _ in 0..6 {
        app.update();
    }
    let _ = drain::<ValueChange<f64>>(&mut app); // discard any settle-phase noise

    let node = get_by_role(app.world_mut(), A11yRole::Slider, Some("Volume"), None)
        .expect("slider is in the a11y tree");
    increment(app.world_mut(), node).expect("Increment is honored");
    app.update();

    assert!(
        drain::<ValueChange<f64>>(&mut app).is_empty(),
        "an Increment at the maximum is a no-op and must NOT emit a phantom ValueChange",
    );
}
