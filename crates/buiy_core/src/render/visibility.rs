//! The `write_paint_skip` render-prep pass: subtree-scoped paint suppression.
//!
//! A top-down `Children` walk — the same shape as `write_clip_rects` /
//! `write_buiy_transform` — writes the computed [`ComputedPaintSkip`] marker
//! onto every entity in a `CssVisibility::Hidden` / `OffscreenAuto` subtree
//! (the root AND its descendants) and removes it from entities no longer in a
//! suppressed subtree. Extract reads the marker as the SINGLE per-entity skip
//! source, so a `Visible`/default child of a `Hidden` parent stops painting —
//! the subtree-scoped skip paint-order-and-top-layer.md § 5.3 / § 5.4 mandate,
//! which R5's per-entity leaf skip did not cover.
//!
//! v1 semantics are a **blanket** subtree drop: there is no
//! `visibility: visible` override inside a hidden subtree until a visibility
//! cascade exists (the pass stops nothing at an explicit `Visible`). An
//! entity's OWN skip input takes precedence over the inherited one for the
//! marker's `reason`, so flipping an ancestor back to visible leaves a
//! still-hidden descendant marked with its own reason.
//!
//! Runs in `Update`, `.after(BuiySet::Animate).before(BuiySet::Picking)`
//! (architecture.md § 5.2), alongside the clip / effect-group prep passes.
//! The walk is seed-gated: it runs ONLY when a visibility input or the
//! hierarchy changed this frame, so a steady-state frame does O(0) work
//! (early return, no tree walk, no structural ops).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/
//! paint-order-and-top-layer.md § 5.3 / § 5.4; design note (Option A,
//! ratified): 2026-06-06-render-subtree-visibility-suppression-design.md.

use crate::components::Node;
use crate::render::clip::reconcile_one;
use bevy::prelude::*;

// The skip types are owned by render/components.rs (beside the other computed
// markers) — re-exported here so the predicate output is nameable through
// `render::visibility` (where the system and predicate that produce it live),
// mirroring how `render::effect` re-exports `EffectGroup`/`EffectReason`.
pub use crate::render::components::{ComputedPaintSkip, SkipReason};
use crate::render::components::{CssVisibility, OffscreenAuto};

/// Decide whether a `Node` entity ROOTS a suppressed subtree, and why.
/// `None` => the entity introduces no suppression of its own (it may still be
/// inside an ancestor's suppressed subtree — that is the walk's job, not this
/// predicate's). Inputs are bound as `Option<&T>` / `bool` exactly as the
/// walk's input fan binds them. Precedence (first match wins): render-owned
/// `CssVisibility::Hidden`, then `OffscreenAuto`. `content-visibility: hidden`
/// is deliberately NOT consulted here — the Hidden entity's own box paints
/// (§ 5.2) and its descendants are pruned layout-side.
pub fn node_skip_reason(
    css_visibility: Option<&CssVisibility>,
    offscreen_auto: bool,
) -> Option<SkipReason> {
    if matches!(css_visibility, Some(CssVisibility::Hidden)) {
        return Some(SkipReason::CssHidden);
    }
    if offscreen_auto {
        return Some(SkipReason::OffscreenAuto);
    }
    None
}

/// The per-entity suppression inputs the walk reads (the two § 5.3/§ 5.4
/// skip carriers). A `type` alias keeps the `Query` signature readable,
/// matching the sibling `ClipNodeData` in `render::clip`.
type SkipInputs<'w> = (Option<&'w CssVisibility>, Option<&'w OffscreenAuto>);

/// Render-prep — computes the subtree-scoped [`ComputedPaintSkip`] marker by
/// a top-down `Children` walk (§ 5.3 / § 5.4): every entity inside a
/// `CssVisibility::Hidden` / `OffscreenAuto` subtree carries the marker;
/// every entity outside one carries none. Insert/remove ops are
/// change-gated (`reconcile_one`, shared with the clip pass) so extract's
/// `Changed<ComputedPaintSkip>` probe fires only on a real flip.
///
/// The WALK itself is also gated: it runs only when a seed fired this frame —
/// a `CssVisibility` / `OffscreenAuto` change (add counts), a removal of
/// either, or a hierarchy edit (`Children` / `ChildOf` changed or removed —
/// reparenting into/out of a hidden subtree must re-place the marker). Any
/// seed triggers a FULL all-roots walk (cheap: the reconcile makes re-visits
/// op-free) rather than a minimal-subtree computation — visibility flips are
/// rare, and the full walk is what keeps a node reparented OUT of a hidden
/// subtree from keeping a stale marker. A seedless frame returns before
/// touching the tree (the O(0) steady state).
///
/// Runs in `Update`, `.after(BuiySet::Animate).before(BuiySet::Picking)`
/// (architecture.md § 5.2), so extract sees a settled marker the same frame.
// A seed-gated tree walk reads many independently-tracked inputs (the root
// set, the hierarchy links, the input fan, the stored markers, the seed probe,
// four removal streams); bundling them would obscure, not clarify — the same
// shape (and the same allow) as `write_buiy_transform`.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn write_paint_skip(
    mut commands: Commands,
    all_nodes: Query<Entity, With<Node>>,
    child_of: Query<&ChildOf>,
    node_marker: Query<(), With<Node>>,
    children: Query<&Children>,
    inputs: Query<SkipInputs, With<Node>>,
    existing: Query<Option<&ComputedPaintSkip>>,
    // Seed probe: did any suppression input or hierarchy link change this
    // frame? `Changed<T>` includes `Added<T>`, so a freshly-spawned hidden
    // root or a child newly parented under one seeds the walk.
    seeds: Query<
        (),
        (
            With<Node>,
            Or<(
                Changed<CssVisibility>,
                Changed<OffscreenAuto>,
                Changed<Children>,
                Changed<ChildOf>,
            )>,
        ),
    >,
    // Removals — the seeds `Changed` cannot see: dropping the CssVisibility /
    // OffscreenAuto component un-hides a subtree, and a hierarchy detach
    // (child pulled out to become a root, last child leaving a parent) must
    // clear stale markers. Drained every frame so the cursors never lag.
    mut removed_vis: RemovedComponents<CssVisibility>,
    mut removed_offscreen: RemovedComponents<OffscreenAuto>,
    mut removed_child_of: RemovedComponents<ChildOf>,
    mut removed_children: RemovedComponents<Children>,
) {
    // Drain ALL removal cursors before the early-out (the cursor-advance
    // idiom extract's despawn stream uses) — a removal this frame must seed
    // the walk, and an un-drained cursor would replay stale events later.
    let removal_seeded = removed_vis.read().count() > 0
        || removed_offscreen.read().count() > 0
        || removed_child_of.read().count() > 0
        || removed_children.read().count() > 0;
    if seeds.is_empty() && !removal_seeded {
        return; // steady state: no walk, no ops.
    }

    // A walk root is a Node with no `ChildOf`, OR whose `ChildOf` parent is
    // not a Node — the same two-disjunct root definition `write_clip_rects` /
    // `write_buiy_transform` and layout use. Seeding only detached Nodes
    // would silently skip a Buiy subtree parented under a non-Node Bevy
    // entity, leaving its markers stale.
    for entity in all_nodes.iter() {
        let is_root = match child_of.get(entity) {
            Ok(parent) => node_marker.get(parent.parent()).is_err(),
            Err(_) => true,
        };
        if is_root {
            walk(entity, None, &mut commands, &children, &inputs, &existing);
        }
    }
}

/// One node of the top-down walk. `inherited` is the nearest suppressing
/// ancestor's reason (`None` outside any suppressed subtree); an entity's OWN
/// skip input takes precedence for its marker, then flows to its children.
fn walk(
    entity: Entity,
    inherited: Option<SkipReason>,
    commands: &mut Commands,
    children: &Query<&Children>,
    inputs: &Query<SkipInputs, With<Node>>,
    existing: &Query<Option<&ComputedPaintSkip>>,
) {
    // A non-Node entity in the Children tree is not a Buiy node — skip it and
    // its subtree (paint suppression applies to the Buiy node tree), matching
    // the clip walk's boundary rule.
    let Ok((css_vis, offscreen)) = inputs.get(entity) else {
        return;
    };

    let own = node_skip_reason(css_vis, offscreen.is_some());
    let effective = own.or(inherited);

    let prev = existing.get(entity).unwrap_or(None);
    reconcile_one(
        commands,
        entity,
        effective.map(|reason| ComputedPaintSkip { reason }),
        prev,
    );

    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            walk(child, effective, commands, children, inputs, existing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper mirroring what the walk binds per entity: Option of each skip
    // input. The same shape the old extract-side leaf predicate had — the
    // predicate moved producer-side unchanged when the computed marker became
    // extract's single skip source.
    fn skip(css_vis: Option<CssVisibility>, offscreen: bool) -> Option<SkipReason> {
        node_skip_reason(css_vis.as_ref(), offscreen)
    }

    #[test]
    fn visible_entity_roots_no_suppression() {
        assert_eq!(skip(None, false), None);
        assert_eq!(skip(Some(CssVisibility::Visible), false), None);
    }

    #[test]
    fn css_visibility_hidden_roots_a_suppressed_subtree() {
        assert_eq!(
            skip(Some(CssVisibility::Hidden), false),
            Some(SkipReason::CssHidden)
        );
    }

    #[test]
    fn css_visibility_collapse_is_not_a_paint_skip_in_v1() {
        // Collapse is a deferred table/flex marker (component-model.md § 12.1)
        // — v1 ships only the Hidden paint-skip, so Collapse paints normally.
        assert_eq!(skip(Some(CssVisibility::Collapse), false), None);
    }

    #[test]
    fn offscreen_auto_roots_a_suppressed_subtree() {
        assert_eq!(skip(None, true), Some(SkipReason::OffscreenAuto));
    }

    #[test]
    fn content_visibility_hidden_entity_still_paints_its_own_box() {
        // paint-order-and-top-layer.md § 5.2: a `content-visibility: hidden`
        // entity's OWN box paints; only its descendants are pruned, and that
        // prune happens layout-side (they never reach painters_z). The
        // predicate therefore does NOT consult Containment — it is not even
        // an input.
        assert_eq!(skip(None, false), None);
    }

    #[test]
    fn css_hidden_takes_precedence_over_offscreen() {
        // Precedence is observable in the marker's reason; CssHidden first.
        assert_eq!(
            skip(Some(CssVisibility::Hidden), true),
            Some(SkipReason::CssHidden)
        );
    }
}
