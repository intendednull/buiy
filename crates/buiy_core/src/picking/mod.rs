//! Buiy picking: AABB hit-test utilities and the `bevy_picking` backend wiring.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/cross-cutting.md § 3.18.

use crate::components::{ResolvedLayout, StackingContext};
use crate::picking::depth::{PickCandidate, paint_index_lookup, resolve_picks};
use crate::render::components::ClipRect;
use bevy::ecs::query::QueryState;
use bevy::picking::Pickable;
use bevy::picking::backend::PointerHits;
use bevy::prelude::*;

pub mod backend;
pub mod depth;

pub use backend::BuiyPickingBackendPlugin;
/// Re-exported at the `picking` module root so consumers reach it as
/// `buiy_core::picking::global_paint_order` (the SC-3 surface the agent-interface
/// campaign's `inprocess.rs` consumes), alongside `picking::hit_test`.
pub use depth::global_paint_order;

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
        // Register every component the free `hit_test` and the backend query so
        // `QueryState::try_new` over the optional terms (`ClipRect`/`Pickable`)
        // and the paint-order term (`StackingContext`) succeeds even on a world
        // that has not yet spawned any of them (a bare-fixture test). `try_new`
        // fails if a queried component type — even an `Option<&T>` term — is not
        // in `world.components()`; pre-registering them makes the helper robust
        // on any app that adds `PickingPlugin`. `register_component` is idempotent.
        let world = app.world_mut();
        world.register_component::<StackingContext>();
        world.register_component::<ClipRect>();
        world.register_component::<Pickable>();
    }
}

/// Free-function hit-test for tests and library consumers — the topmost-painted
/// Buiy node whose (clipped) absolute box contains `point`. Operates on `&World`
/// via `QueryState::try_new`, which returns `None` if the queried components are
/// not yet registered (e.g., test set-up missed `CorePlugin`); in that case there
/// can be no matching entities, so we return `None`.
///
/// **Absolute basis (C1):** hit-tests the entity's window-space box via its
/// non-optional `GlobalTransform` (the transform bridge is the sole accumulator
/// of `position − ancestor_scroll`), never the parent-local
/// `ResolvedLayout.position`. A node without a `GlobalTransform` (never bridged)
/// is absent from the query — the same drop render accepts; no fallback (D2).
///
/// **Stacking depth (C3):** among the geometric hits, the **topmost-painted**
/// node wins (NOT the smallest-area one — the former Phase-0 stopgap that
/// mis-picked under overlays). The paint order is `global_paint_order`, the SAME
/// derivation `emit_picks` uses (co-drive SC-3 — the two paths must agree), so a
/// pointer activation and an AT actionability query can never disagree about what
/// is on top.
///
/// **`Pickable` filter (C3, net-new):** a `Pickable::IGNORE` decorative child is
/// invisible — the hit passes through to the next entity (typically its
/// widget-root parent); `should_block_lower` (the default for an unmarked node)
/// occludes everything painted below it. A point inside the node's box but
/// outside its computed `ClipRect` (own-box ∩ ancestor clips) is **not** a hit.
///
/// Stays `pub` (SC-3): the agent-interface campaign's `a11y/inprocess.rs`
/// consumes it for its `HitTargetable` actionability gate ("not obscured").
pub fn hit_test(world: &World, point: Vec2) -> Option<Entity> {
    let mut nodes = QueryState::<(
        Entity,
        &ResolvedLayout,
        &GlobalTransform,
        Option<&ClipRect>,
        Option<&Pickable>,
    )>::try_new(world)?;
    // Build the global paint order once, the same way render + `emit_picks` do.
    // A world with no `StackingContext` registered (a bare-fixture test that never
    // stood up the layout pass) yields an empty paint order — every candidate then
    // shares paint index 0 and the Pickable rule alone arbitrates. This is NOT a
    // reason to fail the hit-test: `try_new` returning `None` means "no contexts",
    // not "no nodes".
    let paint_order = QueryState::<(Entity, &StackingContext)>::try_new(world)
        .map(|mut contexts| global_paint_order(&contexts.query(world)))
        .unwrap_or_default();
    let z_of = paint_index_lookup(&paint_order);

    let mut candidates: Vec<PickCandidate> = Vec::new();
    for (entity, layout, gt, clip, pickable) in nodes.iter(world) {
        // C1: absolute basis = GlobalTransform, NOT the parent-local
        // ResolvedLayout.position. A node with no GlobalTransform is absent
        // from the query — no fallback (D2).
        let abs_pos = gt.translation().truncate();
        if !point_in_node(point, abs_pos, layout.size, clip) {
            continue;
        }
        candidates.push(PickCandidate {
            entity,
            paint_index: z_of.get(&entity).copied().unwrap_or(0),
            pickable: pickable.copied().unwrap_or_default(),
        });
    }
    // The shared stacking + Pickable rule (resolve_picks): topmost-painted first,
    // IGNORE dropped, truncated at the first occluder. The topmost survivor is
    // the hit.
    resolve_picks(candidates).first().map(|(e, _)| *e)
}

/// `point` is inside the entity's pickable region: inside its absolute AABB AND
/// (if it carries a computed `ClipRect`) inside that clip. The `ClipRect` is the
/// own-box ∩ ancestor-clip intersection render scissors to; a point clipped away
/// visually must not be a pick either (input-event-model.md § 2.4 clip honoring).
pub(crate) fn point_in_node(
    point: Vec2,
    abs_pos: Vec2,
    size: Vec2,
    clip: Option<&ClipRect>,
) -> bool {
    if !point_in_aabb(point, abs_pos, size) {
        return false;
    }
    match clip {
        Some(c) => {
            point.x >= c.min.x && point.x <= c.max.x && point.y >= c.min.y && point.y <= c.max.y
        }
        None => true,
    }
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
