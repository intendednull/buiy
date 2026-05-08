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
pub fn hit_test(world: &World, point: Vec2) -> Option<Entity> {
    let mut state = QueryState::<(Entity, &ResolvedLayout)>::try_new(world)?;
    let mut best: Option<(Entity, f32)> = None; // entity, area (smallest wins for top-most)
    for (entity, layout) in state.iter(world) {
        if point_in_aabb(point, layout) {
            let area = layout.size.x * layout.size.y;
            if best.map(|(_, a)| area < a).unwrap_or(true) {
                best = Some((entity, area));
            }
        }
    }
    best.map(|(e, _)| e)
}

pub(crate) fn point_in_aabb(point: Vec2, layout: &ResolvedLayout) -> bool {
    let max = layout.position + layout.size;
    point.x >= layout.position.x
        && point.x <= max.x
        && point.y >= layout.position.y
        && point.y <= max.y
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
