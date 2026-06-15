//! GPU-capture glue for the reftest/golden tiers — the ONE place that names
//! the concrete app builder, so Phase 3 swaps it for `DeterministicApp` in a
//! single edit. `pub` so `tests/` integration tests reach it.

use bevy::prelude::*;

/// Build the headless painting app both reftest captures share. Until the
/// determinism builder lands this delegates to the promoted
/// `buiy_core::render::golden::capture_app` (Task 1b.6).
pub fn reftest_app(logical_w: u32, logical_h: u32) -> App {
    buiy_core::render::golden::capture_app(logical_w, logical_h)
}

/// Despawn the previous scene's spawned roots between the two captures so the
/// second scene renders alone. Keeps the camera + render-target entities.
pub fn clear_reftest_scene(app: &mut App) {
    let roots: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, (With<buiy_core::components::Node>, Without<ChildOf>)>()
        .iter(app.world())
        .collect();
    for e in roots {
        app.world_mut().entity_mut(e).despawn();
    }
}
