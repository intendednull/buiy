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

/// The pure Buiy→Bevy transform composition (the seam the walk writes).
///
/// Folds an entity's logical-px `position`, the accumulated ancestor
/// `scroll` sum, and its optional composed `ResolvedTransform.matrix` into one
/// `Transform`:
///   `base = from_translation(position − scroll)`,
///   then `base * matrix` (identity fast-path when `matrix` is `None`).
/// `Transform::from_matrix` decomposes to TRS, dropping any projective row
/// (clip-and-transform.md § B.2 / § B.5). Stays in logical-px, y-down,
/// window-relative space — NO y-flip / scale here (§ B.4); those live in the
/// GPU view uniform.
///
/// Distinct from `crate::layout`'s 6e `compose_transform` (which builds the
/// `ResolvedTransform.matrix` from the layout `UiTransform`); this is the
/// render-prep bridge step that consumes that matrix.
pub fn compose_buiy_transform(position: Vec2, scroll: Vec2, matrix: Option<Mat4>) -> Transform {
    let base = Mat4::from_translation((position - scroll).extend(0.0));
    let composed = match matrix {
        Some(m) => base * m,
        None => base,
    };
    Transform::from_matrix(composed)
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
            let new_t = compose_buiy_transform(
                resolved.position,
                acc,
                resolved_transform.map(|rt| rt.matrix),
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_no_matrix_is_position_minus_scroll_y_down_no_flip() {
        // No ResolvedTransform: translation is exactly (position − scroll), in
        // logical-px y-down space (NO flip — § B.4), z = 0.
        let t = compose_buiy_transform(Vec2::new(30.0, 80.0), Vec2::new(0.0, 30.0), None);
        assert_eq!(t.translation, Vec3::new(30.0, 50.0, 0.0));
        assert_eq!(t.rotation, Quat::IDENTITY);
        assert_eq!(t.scale, Vec3::ONE);
    }

    #[test]
    fn compose_zero_scroll_no_matrix_equals_position() {
        let t = compose_buiy_transform(Vec2::new(12.0, 34.0), Vec2::ZERO, None);
        assert_eq!(t.translation, Vec3::new(12.0, 34.0, 0.0));
    }

    #[test]
    fn compose_folds_translation_matrix_after_base() {
        // base = from_translation(position − scroll); composed = base * matrix.
        // A pure-translation matrix adds onto the base translation.
        let position = Vec2::new(10.0, 20.0);
        let scroll = Vec2::new(4.0, 0.0);
        let matrix = Mat4::from_translation(Vec3::new(15.0, 25.0, 0.0));
        let t = compose_buiy_transform(position, scroll, Some(matrix));
        // (10−4 + 15, 20−0 + 25) = (21, 45).
        assert!((t.translation.x - 21.0).abs() < 1e-5, "{}", t.translation.x);
        assert!((t.translation.y - 45.0).abs() < 1e-5, "{}", t.translation.y);
        assert_eq!(t.translation.z, 0.0);
    }

    #[test]
    fn compose_matrix_order_is_base_times_matrix_not_matrix_times_base() {
        // Composition is base * matrix (the matrix applies in the node's local
        // pre-translation frame). For a scale matrix this differs from the
        // reversed order: base*scale leaves the base translation untouched,
        // whereas scale*base would scale the translation. Pin the production
        // order so a swap reddens here.
        let position = Vec2::new(100.0, 0.0);
        let scroll = Vec2::ZERO;
        let scale = Mat4::from_scale(Vec3::new(2.0, 2.0, 1.0));
        let t = compose_buiy_transform(position, scroll, Some(scale));
        // base translation (100,0) is preserved (base * scale), NOT doubled.
        assert!(
            (t.translation.x - 100.0).abs() < 1e-5,
            "{}",
            t.translation.x
        );
        assert!((t.scale.x - 2.0).abs() < 1e-5, "{}", t.scale.x);
    }

    #[test]
    fn compose_drops_projective_perspective_row() {
        // Transform::from_matrix decomposes to TRS, dropping any projective
        // row (§ B.2 / § B.5) — perspective cannot survive the bridge.
        let mut m = Mat4::from_translation(Vec3::new(7.0, 0.0, 0.0));
        m.z_axis.w = -0.01; // perspective on z
        let t = compose_buiy_transform(Vec2::ZERO, Vec2::ZERO, Some(m));
        assert_eq!(
            t.to_matrix().z_axis.w,
            0.0,
            "projective z-perspective dropped"
        );
        assert!((t.translation.x - 7.0).abs() < 1e-5);
    }
}
