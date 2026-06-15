//! GPU-capture glue for the reftest/golden tiers — the ONE place that names
//! the concrete app builder, so Phase 3 swaps it for `DeterministicApp` in a
//! single edit. `pub` so `tests/` integration tests reach it.

use bevy::prelude::*;

/// Build the headless painting app both reftest captures share. Phase 3 swapped
/// this single line from the bare `capture_app` seam to the
/// [`DeterministicApp`](crate::determinism::DeterministicApp) builder — the
/// `&mut App → RgbaImage` capture contract is identical, but every
/// nondeterminism knob (fixed virtual clock, Ahem sole-family, DPR pin,
/// MSAA/dither) is now pinned at the source. A reftest renders both halves in
/// one app run, so the staged Ahem registration drains in the first capture's
/// quiescence loop and the second half shares it.
pub fn reftest_app(logical_w: u32, logical_h: u32) -> App {
    crate::determinism::DeterministicApp::new(logical_w, logical_h).build()
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
