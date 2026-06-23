//! The shared `painters_z` → pick-depth primitive (input-event-model.md § 2.3,
//! co-drive SC-3). Picking derives its global front-to-back order from the
//! **same** stacking-context flatten render uses, so pick-order can never drift
//! from paint-order.
//!
//! `global_paint_order` reuses [`crate::render::extract::context_roots`] +
//! [`crate::render::extract::context_tree_paint_order`] over a `HashMap<Entity,
//! &[Entity]>` built from the `StackingContext` query — the identical derivation
//! `extract_buiy_nodes` runs (`render/extract.rs` ~:609). The depth a node
//! receives is its **reverse** index in this list (§ 2.2 behavior 2): the
//! last-painted (visually topmost) entity sorts nearest, matching bevy_picking's
//! ascending-depth hover sort (`hover.rs`, `sort_by_key(FloatOrd(depth))`).

use crate::components::StackingContext;
use crate::render::extract::{context_roots, context_tree_paint_order};
use bevy::ecs::system::Query;
use bevy::picking::Pickable;
use bevy::prelude::Entity;
use std::collections::HashMap;

/// The global **front-to-back** paint order of every entity that paints, derived
/// from every forming `StackingContext` exactly the way render derives it
/// (`render::extract::{context_roots, context_tree_paint_order}`).
///
/// Convention (topmost-painted-last): `out[0]` is the **bottom-most** painted
/// entity and `out[out.len() - 1]` is the **topmost**. A node's pick depth is its
/// reverse index — `paint_len - 1 - paint_index` — so the topmost-painted entity
/// gets the smallest depth (nearest the user), which is what `emit_picks` and the
/// free `hit_test` both sort against (§ 2.2 / § 2.3).
///
/// Shared derivation: index within the nearest ancestor `StackingContext`,
/// composed across nested contexts (a nested SC root appears as one atomic entry
/// in its parent's list and its descendants live only in its own `painters_z`),
/// with an ECS-entity-order tiebreak across degenerate multi-roots
/// (`context_roots` sorts by entity). Top-layer members are already at the tail
/// of the root context's `painters_z` (layout sub-pass 6f), so they correctly
/// sort topmost for free.
///
/// Stays `pub` so the agent-interface campaign's `a11y/inprocess.rs` can consume
/// it for its `HitTargetable` actionability gate (co-drive SC-3; that campaign's
/// deferred follow-up #3, "a stacking-aware `hit_test`").
pub fn global_paint_order(contexts: &Query<(Entity, &StackingContext)>) -> Vec<Entity> {
    // The identical index render builds at `extract_buiy_nodes` (`extract.rs`
    // ~:609): every forming context keyed by its root entity, slice-borrowed.
    let sc_by_entity: HashMap<Entity, &[Entity]> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.painters_z.as_slice()))
        .collect();
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { sc_by_entity.get(&e).copied() };

    // Root contexts (entity-sorted by the shared helper), then the recursive
    // tree walk per root — the SAME order `extract_buiy_nodes` emits for render.
    let roots = context_roots(&sc_by_entity);
    let mut order = Vec::new();
    for root in roots {
        context_tree_paint_order(root, &painters_z_of, &mut order);
    }
    order
}

/// Map a `global_paint_order` result to a per-entity paint **index** lookup
/// (`entity → paint_index`, 0 = bottom-most). The depth a node receives is the
/// reverse of this index; callers compute `paint_len - 1 - idx` (or sort by the
/// index descending for "topmost first"). Hoisted so both `hit_test` and
/// `emit_picks` build the lookup the one way.
pub(crate) fn paint_index_lookup(paint_order: &[Entity]) -> HashMap<Entity, usize> {
    paint_order
        .iter()
        .enumerate()
        .map(|(i, e)| (*e, i))
        .collect()
}

/// One geometric hit candidate: the entity whose (clipped) absolute box contains
/// the cursor, its global paint index (`global_paint_order`), and its resolved
/// `Pickable` (default when the entity carries none). The single input shape both
/// `hit_test` and `emit_picks` reduce to, so the stacking + `Pickable` arbitration
/// has exactly one implementation (co-drive SC-3 "two paths must agree").
#[derive(Clone, Copy, Debug)]
pub(crate) struct PickCandidate {
    pub entity: Entity,
    /// Index into `global_paint_order` (0 = bottom-most, higher = nearer).
    pub paint_index: usize,
    pub pickable: Pickable,
}

/// Resolve the geometric hit set into the final top-most-first pick list,
/// applying the shared stacking + `Pickable` rule (input-event-model.md § 2.2
/// `build_picks`):
///
/// 1. **Skip `Pickable::IGNORE`** (`!should_block_lower && !is_hoverable`): the
///    decorative-internal convention — it neither receives a hit nor occludes, so
///    the hit passes through to the next entity (typically its widget-root).
/// 2. **Sort top-most first** by descending paint index (last-painted wins). The
///    paint index is total (no ties), so the order is deterministic.
/// 3. **Truncate at the first occluder (inclusive).** Walking top-down, the first
///    entity with `should_block_lower == true` (the bevy_picking default — a node
///    with no `Pickable` blocks) terminates the walk: it is kept, everything below
///    it is dropped. A `should_block_lower == false` surface is kept and the walk
///    continues, so lower entities still receive the hit.
///
/// Returns the surviving entities paired with their paint index, top-most first.
/// Callers map the paint index to `HitData.depth` via `paint_len - 1 - index`.
pub(crate) fn resolve_picks(mut candidates: Vec<PickCandidate>) -> Vec<(Entity, usize)> {
    // 1. Drop IGNORE entities — invisible to picking (neither hit nor occlude).
    candidates.retain(|c| !is_ignore(&c.pickable));
    // 2. Top-most first: higher paint index = nearer the user.
    candidates.sort_unstable_by_key(|c| std::cmp::Reverse(c.paint_index));
    // 3. Truncate at the first should_block_lower occluder (inclusive).
    let mut out = Vec::with_capacity(candidates.len());
    for c in candidates {
        out.push((c.entity, c.paint_index));
        if c.pickable.should_block_lower {
            break;
        }
    }
    out
}

/// `Pickable::IGNORE` predicate: blocks nothing AND emits no events. A bare
/// `should_block_lower == false` surface (still hoverable) is NOT ignored.
fn is_ignore(p: &Pickable) -> bool {
    !p.should_block_lower && !p.is_hoverable
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(raw: u32) -> Entity {
        Entity::from_raw_u32(raw).unwrap()
    }

    fn cand(entity: Entity, paint_index: usize, pickable: Pickable) -> PickCandidate {
        PickCandidate {
            entity,
            paint_index,
            pickable,
        }
    }

    /// Topmost-painted (highest paint index) is first; a default occluder
    /// truncates everything beneath it (inclusive).
    #[test]
    fn resolve_picks_topmost_first_truncated_at_occluder() {
        let (a, b, c) = (e(1), e(2), e(3));
        // a bottom (0), b middle (1), c top (2); all default-pickable (block).
        let got = resolve_picks(vec![
            cand(a, 0, Pickable::default()),
            cand(c, 2, Pickable::default()),
            cand(b, 1, Pickable::default()),
        ]);
        // Top-painted `c` is the sole survivor (it blocks lower).
        assert_eq!(got, vec![(c, 2)]);
    }

    /// A `should_block_lower == false` surface is kept AND the walk continues, so
    /// the next entity beneath still survives until a real occluder.
    #[test]
    fn resolve_picks_non_blocking_surface_lets_lower_through() {
        let (root, passthrough) = (e(1), e(2));
        let got = resolve_picks(vec![
            cand(root, 0, Pickable::default()), // occluder beneath
            cand(
                passthrough,
                1,
                Pickable {
                    should_block_lower: false,
                    is_hoverable: true,
                },
            ),
        ]);
        // passthrough (top) is kept, then `root` (the occluder) terminates.
        assert_eq!(got, vec![(passthrough, 1), (root, 0)]);
    }

    /// `Pickable::IGNORE` is dropped entirely — neither hit nor occluder — so the
    /// hit falls through to the entity beneath it.
    #[test]
    fn resolve_picks_ignore_is_invisible() {
        let (root, ignored) = (e(1), e(2));
        let got = resolve_picks(vec![
            cand(root, 0, Pickable::default()),
            cand(ignored, 1, Pickable::IGNORE), // painted on top but invisible
        ]);
        assert_eq!(
            got,
            vec![(root, 0)],
            "IGNORE passes the hit through to root"
        );
    }

    #[test]
    fn resolve_picks_empty_is_empty() {
        assert!(resolve_picks(vec![]).is_empty());
    }
}
