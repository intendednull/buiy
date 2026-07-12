//! Top-layer composite consumption (paint-order-and-top-layer.md § 3).
//! Top-layer members are ALREADY at the tail of the root context's
//! `painters_z`, appended by layout sub-pass 6f in tier order. Render
//! partitions the root list into (in-flow, top-layer-tail) by reading each
//! entry's `TopLayer` membership and paints the tail at the root — it NEVER
//! re-sorts (§ 3.1: "Render's only ordering input is the `painters_z` tail").
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 3, § 3.1.

use crate::layout::TopLayer;
use bevy::prelude::Entity;

/// The top-layer ancestor CLIMB (paint-order-and-top-layer.md § 3.1) — factored so
/// the node, glyph, and icon producers classify an entity IDENTICALLY (one source
/// of truth for the ordering; a divergence would trip the per-tier tail-contiguity
/// tripwire). An entity is top-layer iff itself OR any `ChildOf` ancestor is a
/// top-layer FORMER (`is_former`). The climb is MANDATORY: a plain descendant of an
/// overlay carries `None` on its OWN `Stacking`, so a per-node read would
/// misclassify it as base and split the contiguous top-layer tail every tier packer
/// partitions at (buckets::`TopLayerBoundaryTracker` / `partition_glyph_ranges`).
///
/// `is_former` decides whether an entity forms a top-layer stacking context; the
/// node producer keys it on the paint-fan `Stacking.top_layer`, the glyph/icon
/// producers on the SC's `cross_root_rank > 0` (layout 6f stamps
/// [`cross_root_rank`](crate::render::extract::cross_root_rank) `> 0` iff
/// `top_layer != None`, so the two agree on every EMITTED entity — a paint-skipped
/// former's subtree emits nothing, the only place they could differ). `parent_of`
/// yields an entity's `ChildOf` parent (`None` at a root).
pub fn in_top_layer(
    start: Entity,
    is_former: impl Fn(Entity) -> bool,
    parent_of: impl Fn(Entity) -> Option<Entity>,
) -> bool {
    let mut cur = start;
    loop {
        if is_former(cur) {
            return true;
        }
        match parent_of(cur) {
            Some(parent) => cur = parent,
            None => return false,
        }
    }
}

/// Stably reorder `items` so every top-layer element (`is_top`) forms the trailing
/// SUFFIX, with the relative order of BOTH the base prefix and the top-layer suffix
/// preserved verbatim (a stable partition, NOT a sort). This MATERIALIZES the
/// single-boundary invariant every tier packer relies on (top-layer content is one
/// global contiguous suffix of the paint order).
///
/// It is needed because the cross-root paint walk (`context_roots` +
/// `context_tree_paint_order`) does NOT produce that suffix on its own: a PARENTED
/// top-layer node escapes to the tail of its own root's `painters_z`, but a
/// SEPARATE base root (another rank-0 stacking context with a HIGHER entity id —
/// e.g. the dooduel podium confetti, ~110 independent `Translate` roots spawned
/// after the view) sorts AFTER that root, so its base content follows the earlier
/// root's escaped top-layer tail. Without this partition the per-tier
/// tail-contiguity `debug_assert` (a base node/run after a top-layer one) trips.
///
/// A scene already in suffix order (the single-root common case) is reordered to
/// the IDENTICAL order (stable), so the per-tier boundary and every golden are
/// unchanged — the byte-stable path. Callers skip the call entirely when the scene
/// has no top-layer former (nothing to move).
pub fn stable_top_layer_suffix<T>(items: &mut [T], is_top: impl Fn(&T) -> bool) {
    // `sort_by_cached_key` is STABLE (preserves the relative order of equal keys)
    // and evaluates `is_top` ONCE per element (the climb is a `ChildOf` walk, not
    // free). `bool: Ord` puts `false` (base) before `true` (top-layer) — i.e. a
    // stable partition with the top-layer elements as the suffix.
    items.sort_by_cached_key(|it| is_top(it));
}

/// Split a root context's `painters_z` into `(in_flow, top_layer_tail)` by
/// reading each entry's top-layer membership via `top_layer_of`. The relative
/// order of BOTH partitions is preserved verbatim from the input — this is a
/// stable partition, NOT a sort. The tail is whatever layout already ordered.
pub fn partition_top_layer<F>(
    root_painters: &[Entity],
    top_layer_of: F,
) -> (Vec<Entity>, Vec<Entity>)
where
    F: Fn(Entity) -> TopLayer,
{
    let mut in_flow = Vec::new();
    let mut tail = Vec::new();
    for &p in root_painters {
        if top_layer_of(p) == TopLayer::None {
            in_flow.push(p);
        } else {
            tail.push(p);
        }
    }
    (in_flow, tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::TopLayer;
    use bevy::prelude::Entity;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    #[test]
    fn partitions_tail_preserving_order() {
        // Layout produced: [inflow0, inflow1, FULLSCREEN, POPOVER, MODAL].
        // partition keeps in-flow first (in order) and the tail in the exact
        // order layout emitted (tier order) — render does NOT re-sort it.
        let (i0, i1, fs, pop, modal) = (e(1), e(2), e(3), e(4), e(5));
        let root = vec![i0, i1, fs, pop, modal];
        let tl = move |q: Entity| {
            if q == fs {
                TopLayer::Fullscreen
            } else if q == pop {
                TopLayer::Popover
            } else if q == modal {
                TopLayer::Modal
            } else {
                TopLayer::None
            }
        };
        let (in_flow, tail) = partition_top_layer(&root, tl);
        assert_eq!(in_flow, vec![i0, i1]);
        // Verbatim tail — Fullscreen < Popover < Modal as layout ordered it.
        assert_eq!(tail, vec![fs, pop, modal]);
    }

    #[test]
    fn no_top_layer_means_empty_tail() {
        let root = vec![e(1), e(2)];
        let (in_flow, tail) = partition_top_layer(&root, |_| TopLayer::None);
        assert_eq!(in_flow, vec![e(1), e(2)]);
        assert!(tail.is_empty());
    }

    #[test]
    fn render_does_not_reorder_an_out_of_tier_tail() {
        // Hostile fixture: a tail layout (hypothetically) emitted as
        // [MODAL, POPOVER] — render MUST keep that exact order, proving it
        // does not impose its own tier sort (§ 3.1 hard constraint). If render
        // sorted, this would come back [POPOVER, MODAL].
        let (modal, pop) = (e(1), e(2));
        let root = vec![modal, pop];
        let tl = move |q: Entity| {
            if q == modal {
                TopLayer::Modal
            } else {
                TopLayer::Popover
            }
        };
        let (_in_flow, tail) = partition_top_layer(&root, tl);
        assert_eq!(tail, vec![modal, pop], "render must not re-sort the tail");
    }

    // === in_top_layer (the shared ancestor climb) ============================

    #[test]
    fn in_top_layer_climbs_to_a_former_ancestor() {
        // chain: leaf(4) -> mid(3) -> former(2) -> root(1). `former` is the only
        // top-layer former; the climb must tag `leaf` and `mid` (inheritance), the
        // former itself, and NOT `root`.
        let (root, former, mid, leaf) = (e(1), e(2), e(3), e(4));
        let parent_of = move |q: Entity| match q {
            x if x == leaf => Some(mid),
            x if x == mid => Some(former),
            x if x == former => Some(root),
            _ => None,
        };
        let is_former = move |q: Entity| q == former;
        assert!(
            in_top_layer(leaf, is_former, parent_of),
            "deep descendant inherits"
        );
        assert!(in_top_layer(mid, is_former, parent_of));
        assert!(
            in_top_layer(former, is_former, parent_of),
            "the former itself"
        );
        assert!(
            !in_top_layer(root, is_former, parent_of),
            "an ancestor of the former is not top-layer"
        );
    }

    #[test]
    fn in_top_layer_is_false_with_no_former_on_the_chain() {
        let parent_of = move |q: Entity| (q == e(2)).then(|| e(1));
        assert!(!in_top_layer(e(2), |_| false, parent_of));
        assert!(!in_top_layer(e(1), |_| false, parent_of));
    }

    // === stable_top_layer_suffix (the global stable partition) ===============

    #[test]
    fn stable_suffix_moves_top_layer_to_the_tail_preserving_both_orders() {
        // Interleaved base/top: [b0, t0, b1, t1, b2] -> the three base elements keep
        // their relative order as the prefix, the two top elements keep theirs as the
        // suffix (a STABLE partition, not a sort).
        let mut v = vec![(0, false), (1, true), (2, false), (3, true), (4, false)];
        stable_top_layer_suffix(&mut v, |&(_, top)| top);
        assert_eq!(
            v,
            vec![(0, false), (2, false), (4, false), (1, true), (3, true)],
            "base prefix (0,2,4) then top suffix (1,3), both in original order"
        );
    }

    #[test]
    fn stable_suffix_is_a_noop_when_already_suffix_ordered() {
        // The single-root common case (top-layer already the escaped tail): the
        // partition yields the IDENTICAL order, so no boundary / golden shifts.
        let original = vec![(0, false), (1, false), (2, true), (3, true)];
        let mut v = original.clone();
        stable_top_layer_suffix(&mut v, |&(_, top)| top);
        assert_eq!(
            v, original,
            "already-suffix input is unchanged (byte-stable path)"
        );
    }

    #[test]
    fn stable_suffix_handles_all_base_and_all_top() {
        let mut all_base = vec![(0, false), (1, false)];
        let snapshot = all_base.clone();
        stable_top_layer_suffix(&mut all_base, |&(_, top)| top);
        assert_eq!(all_base, snapshot, "no top-layer element ⇒ unchanged");

        let mut all_top = vec![(0, true), (1, true)];
        let snapshot = all_top.clone();
        stable_top_layer_suffix(&mut all_top, |&(_, top)| top);
        assert_eq!(all_top, snapshot, "all top-layer ⇒ unchanged");
    }
}
