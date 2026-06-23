//! Buiy's `bevy_picking` backend. Reads `PointerLocation` and produces
//! `PointerHits` from `ResolvedLayout` AABBs.
//!
//! Depth (C3a / co-drive SC-3): hits are ranked by **stacking paint-order** —
//! the topmost-painted node wins — via the SAME `global_paint_order` derivation
//! the free `hit_test` uses, so the two picking paths can never disagree about
//! what is on top. `Pickable::IGNORE` decorative children pass through to the
//! nearest non-IGNORE entity; `should_block_lower` (the unmarked-node default)
//! occludes everything painted below it; a point outside a node's computed
//! `ClipRect` is not a hit.
//!
//! Still C3a scope: camera resolution, `order = camera_order + 0.5`, and no-hit
//! emission remain the Phase-0 closeout behavior (`Entity::PLACEHOLDER` camera,
//! `order = 0.0`, no emission when nothing is hit) — those are later C3
//! sub-slices, not this one. `Hovered`/`update_hovered` are unchanged: this
//! backend still feeds them via `PointerHits`.

use crate::components::{ResolvedLayout, StackingContext};
use crate::picking::depth::{PickCandidate, global_paint_order, paint_index_lookup, resolve_picks};
use crate::picking::point_in_node;
use crate::render::components::ClipRect;
use bevy::picking::Pickable;
use bevy::picking::PickingSystems;
use bevy::picking::backend::{HitData, PointerHits};
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;

/// Buiy's `bevy_picking` backend plugin. Registers `emit_picks` in
/// [`PickingSystems::Backend`] so bevy_picking can composite Buiy's
/// hit results with any other active backends.
pub struct BuiyPickingBackendPlugin;

impl Plugin for BuiyPickingBackendPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, emit_picks.in_set(PickingSystems::Backend));
    }
}

#[allow(clippy::type_complexity)]
fn emit_picks(
    pointers: Query<(&PointerId, &PointerLocation)>,
    nodes: Query<(
        Entity,
        &ResolvedLayout,
        &GlobalTransform,
        Option<&ClipRect>,
        Option<&Pickable>,
    )>,
    contexts: Query<(Entity, &StackingContext)>,
    mut output: MessageWriter<PointerHits>,
) {
    // Build the global front-to-back paint order ONCE per frame, the SAME way
    // render + the free `hit_test` derive it (co-drive SC-3 — pick-order can
    // never drift from paint-order).
    let paint_order = global_paint_order(&contexts);
    let z_of = paint_index_lookup(&paint_order);
    let paint_len = paint_order.len();

    for (pointer, location) in pointers.iter() {
        let Some(loc) = location.location() else {
            continue;
        };
        let cursor = loc.position;

        // Collect every Buiy node whose (clipped) absolute box contains the
        // cursor, paired with its global paint index. C1: absolute basis =
        // GlobalTransform.
        let mut candidates: Vec<PickCandidate> = Vec::new();
        for (entity, layout, gt, clip, pickable) in nodes.iter() {
            let abs_pos = gt.translation().truncate();
            if !point_in_node(cursor, abs_pos, layout.size, clip) {
                continue;
            }
            candidates.push(PickCandidate {
                entity,
                paint_index: z_of.get(&entity).copied().unwrap_or(0),
                pickable: pickable.copied().unwrap_or_default(),
            });
        }
        // The shared stacking + Pickable rule: topmost-painted first, IGNORE
        // dropped, truncated at the first occluder.
        let resolved = resolve_picks(candidates);
        if resolved.is_empty() {
            // C3a keeps the Phase-0 no-hit behavior (no emission when nothing is
            // hit); no-hit emission is a later C3 sub-slice.
            continue;
        }
        // HitData.depth = reverse of the paint index, so bevy_picking's
        // ascending-depth hover sort puts the topmost-painted node nearest.
        let picks: Vec<(Entity, HitData)> = resolved
            .iter()
            .map(|(e, paint_index)| {
                let depth = (paint_len.saturating_sub(1).saturating_sub(*paint_index)) as f32;
                (
                    *e,
                    HitData::new(
                        // Camera entity unknown to Buiy in C3a (still Phase-0
                        // closeout); the real-camera resolution + order =
                        // camera_order + 0.5 is a later C3 sub-slice.
                        Entity::PLACEHOLDER,
                        depth,
                        None,
                        None,
                    ),
                )
            })
            .collect();

        output.write(PointerHits::new(*pointer, picks, 0.0));
    }
}
