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
}
