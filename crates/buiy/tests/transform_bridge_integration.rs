//! Integration: BuiyPlugin's bridge produces a final GlobalTransform in
//! Update (before Picking) when the app supplies TransformPlugin, and
//! TransformPlugin's canonical PostUpdate pass is actually wired in.
//! HEADLESS — no DefaultPlugins, no RenderApp.

use bevy::prelude::*;
use bevy::transform::TransformSystems;
use buiy::{BuiyPlugin, Node, ResolvedLayout};
use buiy_core::layout::Style;

#[test]
fn buiy_plugin_bridge_finalizes_global_transform_in_update() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::input::InputPlugin); // BuiyPlugin requires it
    app.add_plugins(bevy::transform::TransformPlugin); // supplies the canonical PostUpdate pass
    app.add_plugins(BuiyPlugin);

    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(12.0, 34.0)))
        .id();
    app.update();

    let layout = app.world().get::<ResolvedLayout>(e).unwrap().position;
    let gt = app
        .world()
        .get::<GlobalTransform>(e)
        .expect("bridge + propagation produced GlobalTransform");
    assert!((gt.translation().x - (layout.x + 12.0)).abs() < 1e-3);
    assert!((gt.translation().y - (layout.y + 34.0)).abs() < 1e-3);
}

/// Perturb every Buiy node's `Transform.translation` in `PostUpdate`, BEFORE
/// the canonical `TransformSystems::Propagate` pass. Used by the next test to
/// isolate the behavior `TransformPlugin` uniquely provides.
fn perturb_transform_in_post_update(mut transforms: Query<&mut Transform, With<Node>>) {
    for mut transform in &mut transforms {
        transform.translation.x += 1000.0;
    }
}

#[test]
fn buiy_plugin_relies_on_transform_plugin_postupdate_pass() {
    // `CorePlugin`'s `Update` propagation chain finalizes `GlobalTransform`
    // before `BuiySet::Picking` WITHOUT `TransformPlugin` (proved headless in
    // `buiy_core/tests/render_transform_bridge.rs::
    // propagation_runs_in_update_without_transform_plugin_postupdate`). So a
    // test that only checks "GlobalTransform == composed Transform after one
    // update" passes identically whether or not `TransformPlugin` is added — it
    // cannot guard the dependency `BuiyPlugin`'s rustdoc names.
    //
    // The behavior `TransformPlugin` UNIQUELY supplies in this app is the
    // canonical `PostUpdate` propagation pass (`bevy_transform` 0.18 adds the
    // `mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms`
    // chain only in `PostUpdate` + `PostStartup`; `CorePlugin` runs its own
    // copy in `Update`). To make the dependency load-bearing, perturb each
    // node's `Transform` in `PostUpdate` AFTER the `Update` chain has already
    // run this frame, then read `GlobalTransform` immediately. Only
    // `TransformPlugin`'s `PostUpdate` `Propagate` pass — scheduled after the
    // perturbation, both in `PostUpdate` — reconciles `GlobalTransform` within
    // the same frame. Drop the `TransformPlugin` line and this assertion fails
    // (the perturbation stays unreconciled until the next frame's `Update`
    // chain), so it is a real regression guard for the dependency.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::input::InputPlugin);
    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(BuiyPlugin);
    app.add_systems(
        PostUpdate,
        perturb_transform_in_post_update.before(TransformSystems::Propagate),
    );

    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(12.0, 34.0)))
        .id();
    app.update();

    let transform = *app.world().get::<Transform>(e).unwrap();
    let global = app.world().get::<GlobalTransform>(e).unwrap();
    assert!(
        (global.translation().x - transform.translation.x).abs() < 1e-3,
        "TransformPlugin's PostUpdate Propagate pass must reconcile the \
         post-Update perturbation within the frame: GlobalTransform.x {} != \
         Transform.x {} (canonical pass missing — TransformPlugin not wired)",
        global.translation().x,
        transform.translation.x,
    );
}
