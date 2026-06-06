//! The `Transform` / `GlobalTransform` bridge (render-prep, main world).
//!
//! `write_buiy_transform` is the SOLE writer of a laid-out entity's Bevy
//! `Transform.translation`. It folds `ResolvedLayout.position`, the
//! accumulated ancestor `ScrollOffset`, and the optional composed
//! `ResolvedTransform.matrix` into one `Transform`; Bevy's propagation
//! chain (scheduled by `CorePlugin` in `Update`) then owns the resulting
//! `GlobalTransform`. Render reads `GlobalTransform`, never `ResolvedLayout`.
//!
//! The bridge stays in logical-px, y-down, window-relative space: the
//! y-down → y-up flip and the logical → physical scale live in the GPU
//! view uniform, never here (clip-and-transform.md § B.4). A flip in the
//! bridge would double-apply against the view uniform.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § B.

use crate::components::{ResolvedLayout, ResolvedTransform};
use crate::layout::{Overflow, ScrollOffset};
use bevy::prelude::*;
use std::collections::HashSet;

/// Render-prep re-run seed for `write_buiy_transform` — the single trigger
/// feeding the single writer (§ B.2/§ B.3). Seeded each frame by
/// `seed_scroll_dirty` from the union of `Changed<ResolvedLayout>`,
/// `Changed<ResolvedTransform>`, and `Changed<ScrollOffset>` on a
/// scroll-container, then drained top-down by the walk. Empty in steady
/// state — the render-prep analogue of layout's `ContainerSizeDirty`.
#[derive(Resource, Default, Debug)]
pub struct ScrollDirty(pub HashSet<Entity>);

/// Render-prep — repopulate `ScrollDirty` for this frame. Cleared, then
/// seeded with every entity whose `ResolvedLayout` or `ResolvedTransform`
/// changed, every entity whose `ResolvedTransform` was *removed* this frame
/// (sub-pass 6e drops it when the composed matrix returns to identity — a
/// removal does not match `Changed`, so without this the walk would never
/// recompose the node back to its position-only translation and
/// `Transform.translation` would stay stale at the old transformed value),
/// plus every scroll-container whose `ScrollOffset` changed (the walk expands
/// that container's subtree). Steady-state frames leave it empty.
pub fn seed_scroll_dirty(
    mut dirty: ResMut<ScrollDirty>,
    changed_layout: Query<Entity, Changed<ResolvedLayout>>,
    changed_transform: Query<Entity, Changed<ResolvedTransform>>,
    mut removed_transform: RemovedComponents<ResolvedTransform>,
    changed_scroll: Query<(Entity, &Overflow), Changed<ScrollOffset>>,
) {
    dirty.0.clear();
    dirty.0.extend(changed_layout.iter());
    dirty.0.extend(changed_transform.iter());
    dirty.0.extend(removed_transform.read());
    for (e, overflow) in changed_scroll.iter() {
        if overflow.is_scroll_container() {
            dirty.0.insert(e);
        }
    }
}

/// Render-prep — the SOLE writer of a laid-out entity's Bevy `Transform`.
/// Top-down `Children` walk: per entity compose
///   `base = from_translation(position − accumulated_ancestor_scroll)`
/// then `base * ResolvedTransform.matrix` (identity fast-path when the
/// optional `ResolvedTransform` is absent), into ONE `Transform`. The walk
/// descends only into `ScrollDirty`-seeded subtrees (§ B.2/§ B.3), so a
/// steady-state frame visits no entities. No y-flip / scale here (§ B.4).
///
/// Inserting `Transform` (via `Commands`) pulls in its required
/// `GlobalTransform` + `TransformTreeChanged` companions, which Bevy's
/// propagation chain (scheduled by `CorePlugin`) then consumes.
///
/// Spec: clip-and-transform.md § B.2 / § B.3.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn write_buiy_transform(
    mut commands: Commands,
    all_nodes: Query<Entity, With<crate::components::Node>>,
    child_of: Query<&ChildOf>,
    node_marker: Query<(), With<crate::components::Node>>,
    layout: Query<(
        &ResolvedLayout,
        Option<&ResolvedTransform>,
        Option<&ScrollOffset>,
        Option<&Overflow>,
    )>,
    children: Query<&Children>,
    existing: Query<Option<&Transform>>,
    dirty: Res<ScrollDirty>,
) {
    // A walk root is a Node with no `ChildOf`, OR whose `ChildOf` parent is
    // not a Node — the same two-disjunct root definition `write_clip_rects`
    // and layout use (spec § A.3). Seeding only detached Nodes would silently
    // drop the walk for a Buiy subtree parented under a non-Node Bevy entity,
    // leaving it with no `Transform` (and thus painted/picked at the origin).
    for entity in all_nodes.iter() {
        let is_root = match child_of.get(entity) {
            Ok(parent) => node_marker.get(parent.parent()).is_err(),
            Err(_) => true,
        };
        if is_root {
            walk(
                entity,
                Vec2::ZERO,
                false,
                &mut commands,
                &layout,
                &children,
                &existing,
                &dirty,
            );
        }
    }
}

/// One node of the top-down walk. `acc` is the running ancestor scroll sum;
/// `ancestor_seeded` is true once any ancestor (or this node) is in
/// `ScrollDirty`, forcing the whole subtree to recompose (a parent-box or
/// ancestor-scroll change shifts every descendant translation).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn walk(
    entity: Entity,
    acc: Vec2,
    ancestor_seeded: bool,
    commands: &mut Commands,
    layout: &Query<(
        &ResolvedLayout,
        Option<&ResolvedTransform>,
        Option<&ScrollOffset>,
        Option<&Overflow>,
    )>,
    children: &Query<&Children>,
    existing: &Query<Option<&Transform>>,
    dirty: &ScrollDirty,
) {
    let seeded = ancestor_seeded || dirty.0.contains(&entity);

    // Compose this node's translation and push the child-facing scroll acc.
    let mut child_acc = acc;
    if let Ok((resolved, resolved_transform, scroll, overflow)) = layout.get(entity) {
        if seeded {
            let base = Mat4::from_translation((resolved.position - acc).extend(0.0));
            let composed = match resolved_transform {
                Some(rt) => base * rt.matrix,
                None => base,
            };
            let new_t = Transform::from_matrix(composed);
            // Change-gate: only write when the translation actually moved
            // (steady-state frames recompose nothing because the walk does
            // not descend unseeded subtrees, but a seeded subtree whose
            // node didn't move still skips the structural op here).
            let unchanged = existing.get(entity).ok().flatten().is_some_and(|prev| {
                prev.translation == new_t.translation
                    && prev.rotation == new_t.rotation
                    && prev.scale == new_t.scale
            });
            if !unchanged {
                commands.entity(entity).insert(new_t);
            }
        }
        // A scroll container adds its own offset to the child-facing acc.
        if overflow.is_some_and(|o| o.is_scroll_container())
            && let Some(off) = scroll
        {
            child_acc = acc + Vec2::new(off.x, off.y);
        }
    }

    if let Ok(kids) = children.get(entity) {
        for &child in kids {
            walk(
                child, child_acc, seeded, commands, layout, children, existing, dirty,
            );
        }
    }
}
