//! Buiy `bevy_picking` backend. Per-window registration; full backend
//! priority + window filter live in `buiy-input-events-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/cross-cutting.md § 3.18.

use crate::components::ResolvedLayout;
use bevy::ecs::query::QueryState;
use bevy::prelude::*;

/// Phase 0 picking exposes a simple AABB hit-test fn for tests + a
/// minimal Bevy system that updates a `Hovered` resource. The full
/// `bevy_picking::backend::PickingBackend` registration lives in v0.x.
pub struct PickingPlugin;

#[derive(Resource, Default, Clone, Debug)]
pub struct Hovered(pub Option<Entity>);

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hovered>()
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

fn point_in_aabb(point: Vec2, layout: &ResolvedLayout) -> bool {
    let max = layout.position + layout.size;
    point.x >= layout.position.x
        && point.x <= max.x
        && point.y >= layout.position.y
        && point.y <= max.y
}

fn update_hovered(
    mut hovered: ResMut<Hovered>,
    windows: Query<&Window>,
    layouts: Query<(Entity, &ResolvedLayout)>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        hovered.0 = None;
        return;
    };
    // Inline hit_test against the live query to avoid needing &World.
    let mut best: Option<(Entity, f32)> = None;
    for (entity, layout) in layouts.iter() {
        if point_in_aabb(cursor, layout) {
            let area = layout.size.x * layout.size.y;
            if best.map(|(_, a)| area < a).unwrap_or(true) {
                best = Some((entity, area));
            }
        }
    }
    hovered.0 = best.map(|(e, _)| e);
}
