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
//! Camera + emission (C3b): the per-pointer `PointerHits` now carries the
//! **real** camera entity (resolved by matching the pointer's target window to
//! the `Camera` whose `RenderTarget` normalizes to that window, § 3.1) and the
//! bevy_ui-convention `order = camera_order + 0.5` (§ 2.2 behavior 5), so
//! bevy_picking composites Buiy's hits correctly against any other backend. A
//! `PointerHits` is written **every** frame a pointer targets a Buiy window,
//! even with an empty pick list (§ 2.2 behavior 3) — so `InteractionPlugin`'s
//! hover diff fires `Pointer<Out>` and clears `DirectlyHovered` when the cursor
//! leaves all Buiy nodes (the Phase-0 "hover never clears" limitation is gone).
//!
//! One-frame stale read (§ 3.3, accepted + documented): `emit_picks` is in
//! `PreUpdate` while the transform bridge writes `GlobalTransform` and layout
//! writes `painters_z` in `Update`, so a frame's hit-test reads last frame's
//! absolute positions / stacking. This is the same lag the editor's pointer
//! selection already documents; a stacking or transform change takes effect for
//! picking one frame later, exactly as it already does for hover.

use crate::components::{ResolvedLayout, StackingContext};
use crate::picking::depth::{PickCandidate, global_paint_order, paint_index_lookup, resolve_picks};
use crate::picking::point_in_node;
use crate::render::components::ClipRect;
use bevy::camera::{Camera, NormalizedRenderTarget, RenderTarget};
use bevy::picking::Pickable;
use bevy::picking::PickingSystems;
use bevy::picking::backend::{HitData, PointerHits};
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

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
    // Cameras whose `RenderTarget` resolves to a window, so a pointer's target
    // window can be matched to its camera (§ 3.1). `RenderTarget` is a separate
    // `#[require]`d component on the camera entity (not a `Camera` field).
    cameras: Query<(Entity, &Camera, &RenderTarget)>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
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
    let primary = primary_window.iter().next();

    for (pointer, location) in pointers.iter() {
        let Some(loc) = location.location() else {
            continue;
        };
        // § 3.1 window filter + camera resolution: a pointer targeting a window
        // with no matching Buiy camera (a non-Buiy window, or an `Image`/
        // `TextureView` target — the deferred render-to-texture case) resolves
        // to no camera and is skipped, so it never produces a Buiy hit.
        let NormalizedRenderTarget::Window(_) = loc.target else {
            continue;
        };
        let Some((camera, camera_order)) = camera_for_target(&cameras, primary, &loc.target) else {
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
        // HitData.depth = reverse of the paint index, so bevy_picking's
        // ascending-depth hover sort puts the topmost-painted node nearest. The
        // real camera entity rides every HitData so any back-projection /
        // provenance consumer reads a live camera, not a placeholder.
        let picks: Vec<(Entity, HitData)> = resolved
            .iter()
            .map(|(e, paint_index)| {
                let depth = (paint_len.saturating_sub(1).saturating_sub(*paint_index)) as f32;
                (*e, HitData::new(camera, depth, None, None))
            })
            .collect();

        // ALWAYS emit (§ 2.2 behavior 3): an empty pick list clears hover via
        // `InteractionPlugin`'s Out diff. `order = camera_order + 0.5` is the
        // bevy_ui convention third-party backends composite against (§ 2.2
        // behavior 5).
        output.write(PointerHits::new(*pointer, picks, camera_order as f32 + 0.5));
    }
}

/// Resolve the camera for a pointer's normalized target window: the camera whose
/// `RenderTarget` normalizes to the SAME `NormalizedRenderTarget`. Returns the
/// camera entity + its `Camera::order` (for `order = camera_order + 0.5`).
///
/// Multi-window falls out for free — each pointer resolves to its own window's
/// camera; a pointer over a non-Buiy window resolves to `None` and is filtered
/// (§ 3.1). `RenderTarget::normalize` handles `WindowRef::Primary` vs
/// `WindowRef::Entity` uniformly, so a `Window(Primary)` camera matches a
/// pointer normalized to the primary-window entity.
fn camera_for_target(
    cameras: &Query<(Entity, &Camera, &RenderTarget)>,
    primary: Option<Entity>,
    target: &NormalizedRenderTarget,
) -> Option<(Entity, isize)> {
    cameras.iter().find_map(|(e, cam, rt)| {
        (cam.is_active && rt.normalize(primary).as_ref() == Some(target)).then_some((e, cam.order))
    })
}
