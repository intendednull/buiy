//! Buiy's `bevy_picking` backend. Reads `PointerLocation` and produces
//! `PointerHits` from `ResolvedLayout` AABBs.
//!
//! Phase 0 closeout scope: per-pointer hits, top-most-by-area resolution
//! (matches the Phase 0 `hit_test` semantics in `mod.rs`). Multi-pointer
//! arbitration, pointer-target window filtering, and full backend priority
//! land in `buiy-input-events-design`.

use crate::components::ResolvedLayout;
use crate::picking::point_in_aabb;
use bevy::picking::PickingSystems;
use bevy::picking::backend::{HitData, PointerHits};
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;

/// Buiy's `bevy_picking` backend plugin. Registers `emit_picks` in
/// [`PickingSystems::Backend`] so bevy_picking can composite Buiy's
/// AABB hit results with any other active backends.
pub struct BuiyPickingBackendPlugin;

impl Plugin for BuiyPickingBackendPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, emit_picks.in_set(PickingSystems::Backend));
    }
}

fn emit_picks(
    pointers: Query<(&PointerId, &PointerLocation)>,
    nodes: Query<(Entity, &ResolvedLayout)>,
    mut output: MessageWriter<PointerHits>,
) {
    for (pointer, location) in pointers.iter() {
        let Some(loc) = location.location() else {
            continue;
        };
        let cursor = loc.position;

        // Collect every Buiy node under the cursor, with its area as the
        // tie-break for "top-most".
        let mut hits: Vec<(Entity, f32)> = Vec::new();
        for (entity, layout) in nodes.iter() {
            if point_in_aabb(cursor, layout) {
                let area = layout.size.x * layout.size.y;
                hits.push((entity, area));
            }
        }
        if hits.is_empty() {
            continue;
        }
        // Smallest area = closest to the user (top of the stack). bevy_picking
        // expects depth-sorted; emit one HitData per entity with depth derived
        // from area rank.
        hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let picks: Vec<(Entity, HitData)> = hits
            .iter()
            .enumerate()
            .map(|(i, (e, _))| {
                (
                    *e,
                    HitData::new(
                        // Camera entity unknown to Buiy in Phase 0 closeout; the
                        // render-graph node draws into the active 2D camera's
                        // ViewTarget, but bevy_picking expects a camera ref for
                        // its own back-projection. v0.x sub-spec wires this.
                        Entity::PLACEHOLDER,
                        i as f32,
                        None,
                        None,
                    ),
                )
            })
            .collect();

        output.write(PointerHits::new(*pointer, picks, 0.0));
    }
}
