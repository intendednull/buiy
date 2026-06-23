//! Buiy picking: AABB hit-test utilities and the `bevy_picking` backend wiring.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/cross-cutting.md § 3.18.

use crate::components::ResolvedLayout;
use bevy::ecs::query::QueryState;
use bevy::picking::backend::PointerHits;
use bevy::prelude::*;

pub mod backend;

pub use backend::BuiyPickingBackendPlugin;

/// Phase 0 picking exposes a simple AABB hit-test fn for tests + a
/// `Hovered` resource updated by consuming `PointerHits` from the Buiy
/// `bevy_picking` backend. The full `bevy_picking::backend::PickingBackend`
/// registration lives in v0.x.
pub struct PickingPlugin;

#[derive(Resource, Reflect, Default, Clone, Debug)]
#[reflect(Resource)]
pub struct Hovered(pub Option<Entity>);

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Hovered>()
            .init_resource::<Hovered>()
            .add_systems(Update, update_hovered.in_set(crate::BuiySet::Picking));
    }
}

/// Free-function AABB hit-test for tests and library consumers. Operates
/// on `&World` via `QueryState::try_new`, which returns `None` if the
/// queried components are not yet registered (e.g., test set-up missed
/// `CorePlugin`). In that case there can be no matching entities, so we
/// return `None`.
///
/// Hit-tests against the entity's **absolute** position via its non-optional
/// `GlobalTransform` (the transform bridge is the sole accumulator of
/// `position − ancestor_scroll`), never the parent-local `ResolvedLayout.position`
/// (C1). A node without a `GlobalTransform` (never bridged) is absent from the
/// query — the same drop render accepts; no fallback.
pub fn hit_test(world: &World, point: Vec2) -> Option<Entity> {
    let mut state = QueryState::<(Entity, &ResolvedLayout, &GlobalTransform)>::try_new(world)?;
    let mut best: Option<(Entity, f32)> = None; // entity, area (smallest wins for top-most)
    for (entity, layout, gt) in state.iter(world) {
        // C1: absolute basis = GlobalTransform, NOT the parent-local
        // ResolvedLayout.position. A node without a GlobalTransform (never
        // bridged) is absent from the query — the same drop render accepts;
        // no fallback (D2).
        let abs_pos = gt.translation().truncate();
        if point_in_aabb(point, abs_pos, layout.size) {
            let area = layout.size.x * layout.size.y;
            if best.map(|(_, a)| area < a).unwrap_or(true) {
                best = Some((entity, area));
            }
        }
    }
    best.map(|(e, _)| e)
}

pub(crate) fn point_in_aabb(point: Vec2, abs_pos: Vec2, size: Vec2) -> bool {
    let max = abs_pos + size;
    point.x >= abs_pos.x && point.x <= max.x && point.y >= abs_pos.y && point.y <= max.y
}

fn update_hovered(mut hovered: ResMut<Hovered>, mut events: MessageReader<PointerHits>) {
    // The top-most hit is at index 0 of `picks` (sorted ascending by depth in
    // `BuiyPickingBackendPlugin::emit_picks`). Multiple pointers: we honor the
    // most-recently-emitted hit and fall through to clearing if no events
    // arrive this frame.
    let mut latest: Option<Entity> = None;
    let mut saw_event = false;
    for ev in events.read() {
        saw_event = true;
        if let Some((entity, _)) = ev.picks.first() {
            latest = Some(*entity);
        } else {
            latest = None;
        }
    }
    if saw_event {
        hovered.0 = latest;
    }
    // Phase 0 closeout limitation: `emit_picks` skips emission when no Buiy
    // node is under the cursor (see `backend::emit_picks`). When the cursor
    // leaves all Buiy nodes (or the window), no event arrives and `Hovered`
    // retains its last value. v0.x `buiy-input-events-design` widens the
    // backend to emit "no hit" events so `Hovered` can clear correctly.
}

#[cfg(test)]
mod aabb_tests {
    use super::*;

    #[test]
    fn point_in_aabb_uses_absolute_top_left() {
        // A widget whose absolute top-left is (70,90), size 100x100. A point at
        // (120,140) is inside the ABSOLUTE box but OUTSIDE a box anchored at the
        // origin — so this only passes once point_in_aabb takes an absolute pos.
        let abs_pos = Vec2::new(70.0, 90.0);
        let size = Vec2::new(100.0, 100.0);
        assert!(point_in_aabb(Vec2::new(120.0, 140.0), abs_pos, size));
        assert!(!point_in_aabb(Vec2::new(20.0, 20.0), abs_pos, size));
        // boundary inclusive on both edges
        assert!(point_in_aabb(abs_pos, abs_pos, size));
        assert!(point_in_aabb(abs_pos + size, abs_pos, size));
    }
}
