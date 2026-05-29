//! Per-step systems for the layout pipeline.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3-4.
//!
//! Phase 1 implements:
//!   - Step 0 `gc_removed_nodes` — `LayoutTree` GC from `RemovedComponents<Node>`.
//!   - Step 1 `sync_styles` — translate changed components and sync hierarchy.
//!   - Step 3 `taffy_compute` — `tree.compute_layout` from each root.
//!   - Step 7 `write_resolved_layout` — write `ResolvedLayout` back to entities.
//!
//! Phase 4 adds:
//!   - Pre-step-1 `inherit_writing_mode` — walk ancestors to populate
//!     `WritingModeResolved` on every Node.
//!
//! Steps 2/4/5/6 are empty sub-sets in Phase 1; later phases attach
//! systems to them.

use super::components::{
    Anchor, BoxModel, ContainIntrinsicSize, Container, ContainerQuery, ContainerQueryActive,
    ContainerQueryInactive, Containment, Display, FlexItem, FlexParams, GridItem, GridParams,
    LayoutAnchorBroken, MultiColumn, Overflow, Position, Rotate, Scale, Scroll, ScrollOffset,
    Stacking, Translate, UiTransform, WritingMode, WritingModeResolved,
};
use super::translate::{ContainerSnapshot, StyleView, style_to_taffy};
use super::tree::LayoutTree;
use super::types::{
    AnchorErrorKind, AnchorName, AnchorRef, ContainFlags, ContainerType, ContentVisibility,
    GridAreas, Inset, Isolation, LayoutWarnOnceKey, Length, PositionKind, QueryCondition, Sizing,
    TopLayer, TransformMatrix, TryCondition, WritingModeKind, ZIndex,
};
use crate::components::{Node, ResolvedLayout, ResolvedTransform};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use taffy::{AvailableSpace, NodeId as TaffyNodeId, Size};

/// Resource — set by `cq_flip_check` (step 4) when one or more
/// `ContainerQuery` activation states differ from this frame's
/// step-2 result. Consumed by `cq_flip_rerun` (step 5), which gates
/// its body on the flag and clears it after re-running. Cleared
/// implicitly at the next frame's step 4 (which overwrites the
/// flag with its own decision).
///
/// Architecture: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.2.
#[derive(Resource, Default, Debug)]
pub struct CqReRunRequested(pub bool);

/// Resource — per-frame counter of how many times Taffy's
/// `compute_layout` was invoked. Reset to zero at the start of each
/// `taffy_compute` invocation (i.e. once per frame), then bumped
/// after each compute (once per layout root). `cq_flip_rerun` also
/// bumps the counter when it re-runs Taffy, so a flip frame ends
/// with `count == 2 * roots` and a non-flip frame ends with
/// `count == roots`.
///
/// Used by `tests/layout_container_queries.rs` to assert the
/// architecture.md § 3.2 "cap at 2× Taffy per frame" contract.
#[derive(Resource, Default, Debug)]
pub struct LayoutTaffyComputeCount(pub u32);

/// Resource — per-frame count of entities the `sync_styles` Or-filter
/// matched for re-translation. Set at the top of every `sync_styles`
/// invocation (overwritten, not accumulated). Used by
/// `tests/layout_container_queries.rs` to assert the Phase 2 O(0)
/// steady-state invariant: in a steady-state frame the iter count is
/// zero, and mutating components excluded from the filter (notably
/// `ScrollOffset` / `ScrollSnapItem`) keeps it zero.
#[derive(Resource, Default, Debug)]
pub struct SyncStylesIterCount(pub usize);

/// Phase 6 — anchor-name lookup table maintained by observers on
/// `On<Insert, Anchor>` / `On<Replace, Anchor>` / `On<Remove, Anchor>`.
///
/// Storage:
/// - `by_name`: anchor name → ordered `Vec<(Entity, u64)>`. Last entry
///   is the current winner (spec: "most-recently-inserted wins").
/// - `entity_epochs`: every `Anchor`-bearing entity's monotonic insertion
///   epoch. Used by `anchor_resolution`'s Kahn-cycle-edge-drop algorithm
///   to identify the most-recently-inserted entity in a cycle.
/// - `next_epoch`: monotonic counter bumped on every observer-driven
///   insert. Never decrements.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Resource, Debug)]
pub struct AnchorNameRegistry {
    by_name: std::collections::HashMap<String, Vec<(Entity, u64)>>,
    entity_epochs: std::collections::HashMap<Entity, u64>,
    next_epoch: u64,
}

impl Default for AnchorNameRegistry {
    fn default() -> Self {
        Self {
            by_name: std::collections::HashMap::new(),
            entity_epochs: std::collections::HashMap::new(),
            // Start at 1 so `entity_epoch(e) > 0` is a faithful
            // "tracked" predicate (epoch 0 is reserved for the
            // `unwrap_or(0)` fallback in `entity_epoch` — i.e.
            // "no entry for this entity").
            next_epoch: 1,
        }
    }
}

impl AnchorNameRegistry {
    /// Insert an entity under a name, bumping the epoch. If the same
    /// `(name, entity)` pair already exists, this is a *re*-insert
    /// (e.g. component replaced) — the epoch bumps so the cycle tiebreaker
    /// considers this entry the most recent.
    ///
    /// Use [`Self::track_epoch`] for unnamed anchors — `insert` is for
    /// the named case only.
    pub fn insert(&mut self, name: String, entity: Entity) {
        let epoch = self.bump_epoch_for(entity);
        let bucket = self.by_name.entry(name).or_default();
        bucket.retain(|(e, _)| *e != entity);
        bucket.push((entity, epoch));
    }

    /// Track the entity's insertion epoch without inserting into any
    /// name bucket. Used by the `On<Insert, Anchor>` observer for
    /// `Anchor.anchor_name == None` cases — the entity still needs an
    /// epoch entry (for the Kahn cycle-edge-drop tiebreaker) but should
    /// NOT pollute `by_name` with sentinel buckets.
    pub fn track_epoch(&mut self, entity: Entity) {
        let _ = self.bump_epoch_for(entity);
    }

    fn bump_epoch_for(&mut self, entity: Entity) -> u64 {
        let epoch = self.next_epoch;
        self.next_epoch += 1;
        self.entity_epochs.insert(entity, epoch);
        epoch
    }

    /// Remove every entry for this entity from every name bucket and
    /// from `entity_epochs`. Called on `On<Remove, Anchor>` and
    /// `On<Replace, Anchor>` (the replace path removes then re-inserts
    /// using the new anchor_name).
    pub fn remove(&mut self, entity: Entity) {
        for bucket in self.by_name.values_mut() {
            bucket.retain(|(e, _)| *e != entity);
        }
        // Drop emptied buckets to avoid unbounded growth.
        self.by_name.retain(|_, bucket| !bucket.is_empty());
        self.entity_epochs.remove(&entity);
    }

    /// Most-recently-inserted entity claiming this name (spec § 3.1
    /// last-wins semantics), or `None` if no entity claims it.
    pub fn find_entity_by_name(&self, name: &str) -> Option<Entity> {
        self.by_name.get(name)?.last().map(|(e, _)| *e)
    }

    /// Entity's most-recent insertion epoch. Used by the Kahn
    /// cycle-edge-drop algorithm.
    pub fn entity_epoch(&self, entity: Entity) -> u64 {
        self.entity_epochs.get(&entity).copied().unwrap_or(0)
    }

    /// Iterate `(name, bucket)` pairs for `DuplicateName` detection
    /// (D11). `bucket.len() > 1` means duplicate; the last entry is
    /// the late-inserter / warn target.
    pub(super) fn iter_buckets(&self) -> impl Iterator<Item = (&str, &[(Entity, u64)])> {
        self.by_name.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }
}

/// Phase 6/7 — transient override map populated by every sub-pass of
/// `BuiyLayoutStep::PostTaffyOverrides` (`sticky_offset` 6a,
/// `table_layout` 6b no-op, `multicol_pack` 6c no-op, and
/// `anchor_resolution` 6d) and consumed by `write_resolved_layout`
/// (step 7). Cleared by `clear_post_taffy_overrides` which runs first
/// in the sub-pass chain.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
#[derive(Resource, Default, Debug)]
pub struct PostTaffyPositionOverrides {
    pub by_entity: std::collections::HashMap<Entity, Vec2>,
}

/// Activation order for the single global top layer (spec § 4.2). A
/// `VecDeque` where the most-recently-activated top-layer entity is at
/// the back (paints last / on top within its tier). Maintained by
/// sub-pass 6f via a per-frame current-membership rebuild (D3): entries
/// no longer top-layer (deactivated or despawned) are dropped, newly
/// top-layer entities are appended in tree order.
///
/// Single global (not per-window): `buiy_core` has no per-window layout
/// yet (D2). Per-window top layers are a follow-up.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 4.2.
///
/// The membership rebuild (D3) is owned by `stacking_context` (6f): it
/// drops entries that are no longer top-layer (deactivated / despawned)
/// and appends newly-activated entities at the back, so the deque stays in
/// activation order with the most-recently-activated entity last.
#[derive(Resource, Default, Debug)]
pub struct TopLayerActivation {
    pub order: std::collections::VecDeque<Entity>,
}

/// Phase 6 — per-frame warn-dedup set. Cleared at the top of
/// `anchor_resolution` and populated solely by `anchor_resolution`
/// itself (all kinds, including `DuplicateName` which is re-detected
/// each frame by scanning `AnchorNameRegistry::iter_buckets` — see
/// Decision D11). Observers do NOT touch this set.
/// Spec § 3.2 step 4: "warn fires once per (entity, frame)".
#[derive(Resource, Default, Debug)]
pub struct LayoutAnchorWarnedThisFrame {
    pub set: std::collections::HashSet<(Entity, AnchorErrorKind)>,
}

/// Phase 7 — session-scoped warn-dedup set. Cleared only on
/// `BuiyExit` (see `clear_warned_once_on_exit` below). Used by the
/// Phase-7 sticky / table / multicol sub-passes (Tasks 5-7) to
/// emit each `LayoutWarnOnceKey` at most once per `App` lifetime.
///
/// Phase 6's `LayoutAnchorWarnedThisFrame` per-frame resource is
/// preserved unchanged — that anchor-specific divergence from
/// spec § 6 stays in place (see Phase 6 CHANGELOG).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 6
/// ("deduplicated via a `HashSet` resource cleared on `BuiyExit`").
#[derive(Resource, Default, Debug)]
pub struct LayoutWarnedOnceSession {
    pub set: std::collections::HashSet<LayoutWarnOnceKey>,
}

/// Phase 7 — the sole site that clears `PostTaffyPositionOverrides`
/// each frame. Runs first in `BuiyLayoutStep::PostTaffyOverrides`.
/// Decouples per-frame clear from any one sub-pass so future
/// sub-passes can be inserted without ordering surprises.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
pub(super) fn clear_post_taffy_overrides(mut overrides: ResMut<PostTaffyPositionOverrides>) {
    overrides.by_entity.clear();
}

/// Phase 7 — clears the session-scoped warn-dedup set on app
/// shutdown. Spec § 6: "deduplicated via a `HashSet` resource
/// cleared on `BuiyExit`."
///
/// Carries `#[allow(dead_code)]` because `buiy_core` does not yet
/// expose a `BuiyState` / `BuiyExit` lifecycle enum, so there is
/// no `OnExit(...)` hook to register against (plan decision D7 —
/// the wire-up is deferred until the foundation lifecycle states
/// are settled). The contract — "warn-once persists for the
/// lifetime of one `App` instance; recreating `App` resets the
/// warns" — is currently satisfied by `init_resource` constructing
/// a fresh empty `LayoutWarnedOnceSession` on every `App::new()`;
/// tests that need to reset mid-session call this function
/// directly.
///
/// Pattern mirrors the deferred `clear_post_taffy_overrides` that
/// was added unwired in 89d8fe8 and later wired in 286bb6c once
/// `BuiyLayoutStep::PostTaffyOverrides` had downstream consumers.
#[allow(dead_code)]
pub(super) fn clear_warned_once_on_exit(mut warned: ResMut<LayoutWarnedOnceSession>) {
    warned.set.clear();
}

// ---------------------------------------------------------------------
// Phase 7 — sub-pass 6a: sticky positioning.
//
// The four helpers below (`nearest_scroll_container`, `world_position`,
// `resolve_sticky_inset`, `compute_sticky_displacement`) plus the
// `sticky_offset` system implement the CSS § 6.3 sticky-positioning
// algorithm. `sticky_offset` is wired into
// `BuiyLayoutStep::PostTaffyOverrides` (Task 8); the helpers are
// reachable transitively. The pure helper `compute_sticky_displacement`
// is covered by unit tests in `mod tests`; integration coverage of the
// full pipeline lands in Task 10.
//
// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.3.

/// Walk up `ChildOf` from `entity`, returning the first ancestor whose
/// `Overflow.is_scroll_container()` is true. Returns `None` if no
/// scroll-container ancestor exists.
///
/// Phase 7 — sub-pass 6a (`sticky_offset`) uses this to find the
/// reference frame for sticky displacement (innermost wins, per D9).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.1.
fn nearest_scroll_container(
    entity: Entity,
    parent_chain: &Query<&ChildOf>,
    overflow_q: &Query<&Overflow>,
) -> Option<Entity> {
    let mut current = entity;
    loop {
        // ChildOf is not a tuple struct in Bevy 0.18; use `.parent()`.
        let parent = parent_chain.get(current).ok()?.parent();
        if let Ok(overflow) = overflow_q.get(parent)
            && overflow.is_scroll_container()
        {
            return Some(parent);
        }
        current = parent;
    }
}

/// Compute `entity`'s position in `ancestor`'s content-box coordinate
/// system by walking `ChildOf` from `entity` up to (but not including)
/// `ancestor`, summing the Taffy `.location` of each step.
///
/// Uses the provided `memo` cache to avoid re-walking shared subpaths
/// (mirrors `resolve_writing_mode`'s memoization pattern). Memoization
/// key is `(entity, ancestor)` to handle multiple scroll-container
/// frames in the same call.
///
/// Returns `None` if (a) `entity` has no `LayoutTree` mapping, (b) the
/// walk leaves `ancestor`'s subtree without finding `ancestor`, or
/// (c) a `tree.tree.layout()` read fails.
///
/// Phase 7 — sub-pass 6a (`sticky_offset`).
fn world_position(
    entity: Entity,
    ancestor: Entity,
    tree: &LayoutTree,
    parent_chain: &Query<&ChildOf>,
    memo: &mut HashMap<(Entity, Entity), Vec2>,
) -> Option<Vec2> {
    if entity == ancestor {
        return Some(Vec2::ZERO);
    }
    if let Some(cached) = memo.get(&(entity, ancestor)) {
        return Some(*cached);
    }
    // ChildOf accessor is `.parent()` in Bevy 0.18.
    let parent = parent_chain.get(entity).ok()?.parent();
    let parent_position = world_position(parent, ancestor, tree, parent_chain, memo)?;
    let node_id = tree.by_entity.get(&entity)?;
    let layout = tree.tree.layout(*node_id).ok()?;
    let position = parent_position + Vec2::new(layout.location.x, layout.location.y);
    memo.insert((entity, ancestor), position);
    Some(position)
}

/// Resolve a `Sizing` inset to pixels in the scroll container's
/// reference frame, per D3 / D11.
///
/// Returns `Some(px)` for "this edge is sticky-active" or `None` for
/// "this edge is not set." Inputs that are deferred (`Cq*`) or
/// semantically invalid (`Fr` — grid-only) return `Some(0.0)` and
/// record one `warn!` per (entity, session) via `warned`.
///
/// v2 — `Length` has only `Px / Percent / Fr / Cq*`. `Vh/Vw/Vmin/Vmax/
/// Em/Rem` are not variants and never will be without a Phase 10
/// extension; the match below is closed (no wildcard arm) so the
/// compiler errors when Phase 10 adds new variants — forcing a
/// deliberate decision per future variant.
///
/// Phase 7 — sub-pass 6a (`sticky_offset`).
fn resolve_sticky_inset(
    s: &Sizing,
    scroll_container_axis_size: f32,
    entity: Entity,
    warned: &mut LayoutWarnedOnceSession,
) -> Option<f32> {
    let length = match s {
        Sizing::Length(l) => l,
        // Auto, None, FitContent, MinContent, MaxContent, Stretch —
        // edge not set. Intrinsic-size keywords are never meaningful
        // as positional insets in any CSS.
        Sizing::Auto
        | Sizing::None
        | Sizing::FitContent(_)
        | Sizing::MinContent
        | Sizing::MaxContent
        | Sizing::Stretch => return None,
    };
    Some(match length {
        Length::Px(p) => *p,
        Length::Percent(p) => scroll_container_axis_size * (p / 100.0),
        Length::Fr(_) => {
            if warned
                .set
                .insert(LayoutWarnOnceKey::StickyFrUnsupported(entity))
            {
                warn!(
                    "Sticky entity {:?} uses fr inset; fr is grid-only and resolves to 0.0 on sticky inset.",
                    entity,
                );
            }
            0.0
        }
        // All Cq* variants — full resolution is deferred to a Phase
        // 7.x follow-up (would port Phase 6's `length_inset_to_px`,
        // which takes an anchor-box second argument; sticky's
        // reference frame is the sticky entity's own cq-ancestor, a
        // different shape). v1: warn once per entity, resolve to 0.0.
        Length::Cqw(_)
        | Length::Cqh(_)
        | Length::Cqi(_)
        | Length::Cqb(_)
        | Length::Cqmin(_)
        | Length::Cqmax(_) => {
            if warned
                .set
                .insert(LayoutWarnOnceKey::StickyCqDeferred(entity))
            {
                warn!(
                    "Sticky entity {:?} uses Cq* inset; sticky-cq resolution is deferred to a Phase 7.x follow-up. Inset resolves to 0.0.",
                    entity,
                );
            }
            0.0
        }
    })
}

/// Compute the per-axis sticky displacement, given the natural Taffy
/// position and size of the sticky element, its parent, the scroll
/// container's size, the current scroll offset, and the resolved inset
/// values.
///
/// All positions are in the scroll container's content-box coordinate
/// frame. Output is a displacement to add to the sticky element's
/// natural-relative-to-parent position to get the final
/// position-in-parent-frame.
///
/// **v1 deviation: when both `inset_top` and `inset_bottom` are set,
/// top wins.** A future correct dual-clamp implementation will replace
/// this if-else with an "upper-stuck vs lower-stuck, smallest
/// perturbation from natural wins" rule (CSS spec § 6.3). Documented in
/// CHANGELOG; the `sticky_both_top_and_bottom_active_top_wins` test
/// pins the current behavior.
///
/// Pure function — no Bevy queries, no Taffy reads. Easy to unit test.
///
/// Phase 7 — sub-pass 6a.
#[allow(clippy::too_many_arguments)]
fn compute_sticky_displacement(
    e_natural_in_s: Vec2,        // sticky element position in S's content-box frame
    e_size: Vec2,                // sticky element size
    parent_in_s: Vec2,           // parent position in S's content-box frame
    parent_size: Vec2,           // parent size
    scroll_container_size: Vec2, // S's content-box size
    scroll_offset: Vec2,         // current ScrollOffset on S
    inset_top: Option<f32>,
    inset_bottom: Option<f32>,
    inset_left: Option<f32>,
    inset_right: Option<f32>,
) -> Vec2 {
    let visible_top = scroll_offset.y;
    let visible_bottom = scroll_offset.y + scroll_container_size.y;
    let visible_left = scroll_offset.x;
    let visible_right = scroll_offset.x + scroll_container_size.x;

    let parent_bottom = parent_in_s.y + parent_size.y;
    let parent_right = parent_in_s.x + parent_size.x;

    let desired_y = if let Some(top_px) = inset_top {
        let threshold = visible_top + top_px;
        e_natural_in_s
            .y
            .max(threshold)
            .min(parent_bottom - e_size.y)
            .max(parent_in_s.y)
    } else if let Some(bottom_px) = inset_bottom {
        let threshold = visible_bottom - bottom_px;
        (threshold - e_size.y)
            .min(e_natural_in_s.y)
            .max(parent_in_s.y)
            .min(parent_bottom - e_size.y)
    } else {
        e_natural_in_s.y
    };
    let desired_x = if let Some(left_px) = inset_left {
        let threshold = visible_left + left_px;
        e_natural_in_s
            .x
            .max(threshold)
            .min(parent_right - e_size.x)
            .max(parent_in_s.x)
    } else if let Some(right_px) = inset_right {
        let threshold = visible_right - right_px;
        (threshold - e_size.x)
            .min(e_natural_in_s.x)
            .max(parent_in_s.x)
            .min(parent_right - e_size.x)
    } else {
        e_natural_in_s.x
    };

    Vec2::new(desired_x - e_natural_in_s.x, desired_y - e_natural_in_s.y)
}

/// Sub-pass 6a — sticky offset.
///
/// For each entity with `Position::Sticky`:
/// 1. Find nearest scroll-container ancestor via `nearest_scroll_container`.
/// 2. If none, skip (no warn — silent no-op per D5).
/// 3. Compute world positions in the scroll-container frame.
/// 4. Resolve insets per `resolve_sticky_inset`.
/// 5. Compute displacement per `compute_sticky_displacement`.
/// 6. Write `entity_natural_relative_to_parent + displacement` to
///    `PostTaffyPositionOverrides.by_entity`. Skip the write when the
///    displacement is zero (avoid spurious override entries).
///
/// `Display::None` entities are skipped (D10). When the sticky element
/// has no `ChildOf` (it's a layout root), we skip — Bevy's parent
/// query will simply return `Err`.
///
/// Sticky behaves as `Relative` when no scroll-container ancestor is
/// in scope (D5, silent no-op) — useful for sticky-in-static-context
/// placeholder patterns. Percent insets resolve against the scroll
/// container's content-box axis size (D11). `Length::Fr` and
/// `Length::Cq*` insets warn-once-per-session and resolve to 0.0.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.3.
#[allow(clippy::too_many_arguments)]
pub(super) fn sticky_offset(
    tree: NonSend<LayoutTree>,
    sticky_query: Query<(Entity, &Position, &Display), With<Node>>,
    overflow_q: Query<&Overflow>,
    scroll_offset_q: Query<&ScrollOffset>,
    parent_chain: Query<&ChildOf>,
    mut overrides: ResMut<PostTaffyPositionOverrides>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    // Per-call memo for `world_position` — entities deeper in the
    // sticky set share `ChildOf` chain prefixes, so memoizing avoids
    // redundant walks.
    let mut memo: HashMap<(Entity, Entity), Vec2> = HashMap::new();

    for (e, pos, display) in sticky_query.iter() {
        // D14 — filter in Rust (no Bevy `Without<Display::None>` exists,
        // `Or<>` slots are scarce). D10 — skip `Display::None`.
        if !matches!(pos.kind, PositionKind::Sticky) || matches!(display, Display::None) {
            continue;
        }
        // D5 — no scroll container, silent no-op.
        let Some(scroll_container) = nearest_scroll_container(e, &parent_chain, &overflow_q) else {
            continue;
        };

        // Read sizes / natural-position from Taffy. Each Taffy read
        // failure is a "skip this frame" — Taffy may not have placed
        // the entity yet (e.g. mid-spawn). No warn here — Taffy's own
        // error log covers actual misuse.
        let Some(e_node) = tree.by_entity.get(&e) else {
            continue;
        };
        let Ok(e_layout) = tree.tree.layout(*e_node) else {
            continue;
        };
        let e_size = Vec2::new(e_layout.size.width, e_layout.size.height);
        let e_natural_rel = Vec2::new(e_layout.location.x, e_layout.location.y);

        let Ok(parent_co) = parent_chain.get(e) else {
            continue;
        };
        let parent = parent_co.parent();
        let Some(parent_node) = tree.by_entity.get(&parent) else {
            continue;
        };
        let Ok(parent_layout) = tree.tree.layout(*parent_node) else {
            continue;
        };
        let parent_size = Vec2::new(parent_layout.size.width, parent_layout.size.height);

        let Some(s_node) = tree.by_entity.get(&scroll_container) else {
            continue;
        };
        let Ok(s_layout) = tree.tree.layout(*s_node) else {
            continue;
        };
        let s_size = Vec2::new(s_layout.size.width, s_layout.size.height);

        let Some(e_in_s) = world_position(e, scroll_container, &tree, &parent_chain, &mut memo)
        else {
            continue;
        };
        let Some(parent_in_s) =
            world_position(parent, scroll_container, &tree, &parent_chain, &mut memo)
        else {
            continue;
        };

        // `ScrollOffset` is opt-in per Phase 2 — many scroll
        // containers don't carry one. Default to zero in that case.
        let scroll_offset = scroll_offset_q
            .get(scroll_container)
            .copied()
            .unwrap_or_default();

        // D3 / D11 — per-axis inset resolution. The caller passes the
        // correct scroll-container axis size (height for top/bottom,
        // width for left/right); `resolve_sticky_inset` does not need
        // an axis-tag parameter.
        let top = resolve_sticky_inset(&pos.inset.top, s_size.y, e, &mut warned);
        let bottom = resolve_sticky_inset(&pos.inset.bottom, s_size.y, e, &mut warned);
        let left = resolve_sticky_inset(&pos.inset.left, s_size.x, e, &mut warned);
        let right = resolve_sticky_inset(&pos.inset.right, s_size.x, e, &mut warned);

        let displacement = compute_sticky_displacement(
            e_in_s,
            e_size,
            parent_in_s,
            parent_size,
            s_size,
            Vec2::new(scroll_offset.x, scroll_offset.y),
            top,
            bottom,
            left,
            right,
        );

        if displacement == Vec2::ZERO {
            // No displacement — leave the override map untouched.
            // Avoids polluting the map with no-op entries (which
            // `write_resolved_layout` would otherwise apply
            // redundantly).
            continue;
        }

        overrides.by_entity.insert(e, e_natural_rel + displacement);
    }
}

/// Sub-pass 6b — table layout stub.
///
/// Spec § 1.2: "v1 ships only the API surface and the fallback path;
/// the full algorithm is deferred to a v1.x point release." The
/// fallback path (Table → Block) is handled by `translate.rs`. This
/// sub-pass exists solely to emit a `warn!` once per (entity,
/// session) the first time each `Display::Table*` value is
/// encountered.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
pub(super) fn table_layout(
    table_q: Query<(Entity, &Display), With<Node>>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    for (e, d) in table_q.iter() {
        if !is_table_display(d) {
            continue;
        }
        if warned.set.insert(LayoutWarnOnceKey::TableUnsupported(e)) {
            bevy::log::warn!(
                "Layout: Display::Table* on entity {:?} — table layout algorithm is deferred to v1.x (spec § 1.2). Falling back to Display::Block. Use Display::Grid for v1 table-like layouts.",
                e,
            );
        }
    }
}

fn is_table_display(d: &Display) -> bool {
    matches!(
        d,
        Display::Table
            | Display::TableRowGroup
            | Display::TableHeaderGroup
            | Display::TableFooterGroup
            | Display::TableRow
            | Display::TableCell
            | Display::TableCaption
            | Display::TableColumnGroup
            | Display::TableColumn
    )
}

/// Sub-pass 6c — multi-column packing stub.
///
/// Spec § 3.2 (`flex-and-grid.md`): "Multi-column is tier-E; v1 ships
/// the API but the algorithm is a stub that produces single-column
/// layout with `warn!` once per session." This sub-pass emits the
/// single warn — no per-entity tracking.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
pub(super) fn multicol_pack(
    multicol_q: Query<&MultiColumn>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    if multicol_q.iter().next().is_none() {
        return; // No multicol entities; no warn.
    }
    if warned.set.insert(LayoutWarnOnceKey::MulticolUnsupported) {
        bevy::log::warn!(
            "Layout: MultiColumn detected — multi-column packing algorithm is deferred to v1.x (flex-and-grid.md § 3.2). Falling back to single-column layout. This warn fires once per session."
        );
    }
}

/// Private helper invoked by the `On<Insert, Anchor>` observer closure
/// registered in `LayoutPlugin::build` (D12). Adds the entity to the
/// registry under its `anchor_name` if any; otherwise tracks just the
/// epoch (D11/B2 — no empty-string sentinel bucket).
///
/// Duplicate-name detection is NOT done here; it happens in
/// `anchor_resolution` via `reg.iter_buckets()` (D11). Observers run
/// between frames; clearing `LayoutAnchorWarnedThisFrame` at the top
/// of `anchor_resolution` would otherwise lose any observer-recorded
/// warns.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
pub(super) fn handle_anchor_insert(
    entity: Entity,
    q: &Query<&Anchor>,
    reg: &mut AnchorNameRegistry,
) {
    let Ok(anchor) = q.get(entity) else {
        return; // entity may have been despawned mid-flush
    };
    match &anchor.anchor_name {
        Some(AnchorName::Named(name)) => {
            reg.insert(name.clone(), entity);
        }
        Some(AnchorName::Implicit) | None => {
            // Track the epoch only — D11/B2 — never put unnamed
            // anchors into `by_name` (would pollute the registry
            // and corrupt `find_entity_by_name("")` semantics).
            reg.track_epoch(entity);
        }
    }
}

/// Kahn topological sort over the (anchored → anchor) DAG. Returns the
/// resolved topological order (anchor targets first, anchored last) and
/// the set of entities whose outgoing edge was dropped to break a cycle.
///
/// On cycle: identifies the remaining cycle-bound nodes (post-Kahn nodes
/// with in_degree > 0), finds the one with the highest insertion epoch
/// via `epochs(entity)`, drops its outgoing edge, and re-runs Kahn from
/// scratch. Repeats until all nodes are placed.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.4.
///
/// `edges`: anchored entity → `Some(anchor_target)` or `None` (entity is
/// only an anchor target, no outgoing edge). Returns: (order, dropped).
fn kahn_anchor_sort(
    edges: &std::collections::HashMap<Entity, Option<Entity>>,
    epochs: &dyn Fn(Entity) -> u64,
) -> (Vec<Entity>, std::collections::HashSet<Entity>) {
    let mut current_edges: std::collections::HashMap<Entity, Option<Entity>> = edges.clone();
    let mut dropped: std::collections::HashSet<Entity> = std::collections::HashSet::new();

    // D10 — pre-pass: ensure every target of a `Some(t)` edge is also a
    // key in `current_edges` (with `None` outgoing). Without this, a
    // target Entity that has no Anchor component (e.g. a plain Node
    // pointed at via AnchorRef::Entity(e)) ends up with in_degree > 0
    // but is never dequeued — Kahn flags it as a cycle node and the
    // edge-drop is a no-op, looping forever. Pre-populating these
    // "external target" nodes gives the algorithm a well-defined
    // termination check.
    let external_targets: Vec<Entity> = current_edges
        .values()
        .filter_map(|t| t.as_ref().copied())
        .filter(|t| !current_edges.contains_key(t))
        .collect();
    for t in external_targets {
        current_edges.insert(t, None);
    }

    loop {
        // Build in_degree map: number of edges ending at each node.
        let mut in_degree: std::collections::HashMap<Entity, usize> =
            std::collections::HashMap::new();
        for &e in current_edges.keys() {
            in_degree.entry(e).or_insert(0);
        }
        for t in current_edges.values().flatten() {
            *in_degree.entry(*t).or_insert(0) += 1;
        }

        // Queue of zero-in-degree nodes.
        let mut queue: std::collections::VecDeque<Entity> = in_degree
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(e, _)| *e)
            .collect();

        let mut order: Vec<Entity> = Vec::with_capacity(current_edges.len());
        while let Some(n) = queue.pop_front() {
            order.push(n);
            // Decrement in_degree of the node n points at (if any).
            if let Some(Some(target)) = current_edges.get(&n).copied() {
                let d = in_degree.entry(target).or_insert(1);
                *d = d.saturating_sub(1);
                if *d == 0 {
                    queue.push_back(target);
                }
            }
        }

        if order.len() == current_edges.len() {
            // Kahn over (anchored → anchor_target) yields sources first
            // (anchored before target). Anchor resolution requires
            // targets first (the anchor's box must be resolved before
            // the anchored entity reads it), so reverse the order.
            order.reverse();
            return (order, dropped);
        }

        // Cycle detected. Find the remaining cycle-bound nodes
        // (in_degree > 0 at termination), pick the one with the highest
        // epoch, drop its outgoing edge, re-run.
        let cycle_nodes: Vec<Entity> = in_degree
            .iter()
            .filter(|&(_, &d)| d > 0)
            .map(|(e, _)| *e)
            .collect();

        if cycle_nodes.is_empty() {
            // Defensive: should not happen if order.len() != edges.len()
            return (order, dropped);
        }

        let &drop_from = cycle_nodes
            .iter()
            .max_by_key(|&&e| epochs(e))
            .expect("cycle_nodes non-empty");

        // Drop the outgoing edge from this node.
        if let Some(entry) = current_edges.get_mut(&drop_from) {
            *entry = None;
        }
        dropped.insert(drop_from);
    }
}

/// Resolve a `Length` for inset use. `Px` → its value; `Percent` →
/// percent of the relevant axis. `Fr` → 0 (grid-only unit). `Cq*` → 0
/// (container units in `PositionTry::inset` are tier-C deferred and
/// tracked in follow-ups).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.4.
fn length_inset_to_px(l: Length, axis: f32, _viewport: Vec2) -> f32 {
    match l {
        Length::Px(v) => v,
        Length::Percent(p) => axis * (p / 100.0),
        Length::Fr(_) => 0.0,
        Length::Cqw(_)
        | Length::Cqh(_)
        | Length::Cqi(_)
        | Length::Cqb(_)
        | Length::Cqmin(_)
        | Length::Cqmax(_) => 0.0,
    }
}

/// Compute the anchored entity's would-be top-left from the anchor's
/// resolved box and the try's inset.
///
/// Convention: `inset` is interpreted relative to the anchor's box edges.
/// - `inset.top != 0`: place anchored entity BELOW anchor
///   (`anchored.top = anchor.bottom + top`).
/// - `inset.bottom != 0`: place anchored entity ABOVE anchor
///   (`anchored.bottom = anchor.top - bottom`).
/// - `inset.left != 0`: place anchored entity to the RIGHT of anchor
///   (`anchored.left = anchor.right + left`).
/// - `inset.right != 0`: place anchored entity to the LEFT of anchor
///   (`anchored.right = anchor.left - right`).
///
/// When `top == bottom == 0`, anchored.top = anchor.top (vertically aligned).
/// When `left == right == 0`, anchored.left = anchor.left (horizontally
/// aligned).
///
/// `Sizing::Auto`, intrinsic keywords, and `Stretch` → 0.0 (no offset).
/// `Sizing::Length(_)` → resolve via `length_inset_to_px`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.4.
fn try_anchored_position(
    anchor_pos: Vec2,
    anchor_size: Vec2,
    anchored_size: Vec2,
    inset: &Inset,
    viewport: Vec2,
) -> Vec2 {
    let to_px = |s: &Sizing, axis: f32| -> f32 {
        match s {
            Sizing::Auto => 0.0,
            Sizing::None => 0.0,
            Sizing::Length(l) => length_inset_to_px(*l, axis, viewport),
            // B4: `FitContent` is a tuple variant `FitContent(Length)`;
            // the wildcard `(_)` discards the inner Length (no
            // fit-content semantics in inset position resolution).
            Sizing::MinContent | Sizing::MaxContent | Sizing::FitContent(_) => 0.0,
            Sizing::Stretch => 0.0,
        }
    };
    let top = to_px(&inset.top, anchor_size.y);
    let bottom = to_px(&inset.bottom, anchor_size.y);
    let left = to_px(&inset.left, anchor_size.x);
    let right = to_px(&inset.right, anchor_size.x);

    let x = if right > 0.0 {
        anchor_pos.x - right - anchored_size.x
    } else if left > 0.0 {
        anchor_pos.x + anchor_size.x + left
    } else {
        anchor_pos.x
    };
    let y = if bottom > 0.0 {
        anchor_pos.y - bottom - anchored_size.y
    } else if top > 0.0 {
        anchor_pos.y + anchor_size.y + top
    } else {
        anchor_pos.y
    };
    Vec2::new(x, y)
}

/// Evaluate `[TryCondition]` against this frame's Taffy output.
///
/// `FitsInViewport`: anchored box rect must lie entirely within
/// `(0,0,viewport.x,viewport.y)`.
/// `FitsInContainer(ref)`: resolve the referenced container's box from
/// Taffy and check containment (`Display::None` containers always fail
/// per D9).
/// `AnchorVisible`: the anchor's resolved box must intersect the
/// viewport rect.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.4.
fn try_conditions_pass(
    conditions: &[TryCondition],
    anchored_rect: (Vec2, Vec2), // (pos, size)
    anchor_rect: (Vec2, Vec2),
    viewport: Vec2,
    tree: &LayoutTree,
    reg: &AnchorNameRegistry,
    display_query: &Query<&Display>,
) -> bool {
    conditions.iter().all(|c| match c {
        TryCondition::FitsInViewport => {
            let (pos, size) = anchored_rect;
            pos.x >= 0.0
                && pos.y >= 0.0
                && pos.x + size.x <= viewport.x
                && pos.y + size.y <= viewport.y
        }
        TryCondition::FitsInContainer(r) => {
            let container = match r {
                AnchorRef::Entity(e) => Some(*e),
                AnchorRef::Name(n) => reg.find_entity_by_name(n),
            };
            let Some(c) = container else { return false };
            // D9 — Display::None containers fail the condition.
            if let Ok(Display::None) = display_query.get(c) {
                return false;
            }
            let Some(taffy) = tree.by_entity.get(&c).copied() else {
                return false;
            };
            let Ok(layout) = tree.tree.layout(taffy) else {
                return false;
            };
            let cpos = Vec2::new(layout.location.x, layout.location.y);
            let csize = Vec2::new(layout.size.width, layout.size.height);
            let (apos, asize) = anchored_rect;
            apos.x >= cpos.x
                && apos.y >= cpos.y
                && apos.x + asize.x <= cpos.x + csize.x
                && apos.y + asize.y <= cpos.y + csize.y
        }
        TryCondition::AnchorVisible => {
            let (pos, size) = anchor_rect;
            // Intersection of anchor rect with viewport rect
            // (0,0,viewport.x,viewport.y).
            pos.x + size.x > 0.0 && pos.y + size.y > 0.0 && pos.x < viewport.x && pos.y < viewport.y
        }
    })
}

/// Step 6 sub-pass 6d — anchor resolution.
///
/// Algorithm:
/// 1. Clear `warned.set` (anchor-specific per-frame warn-dedup state;
///    `overrides.by_entity` is cleared by `clear_post_taffy_overrides`,
///    which runs first in the `PostTaffyOverrides` chain).
/// 2. Build the (anchored → anchor_target) edge map from
///    `anchored_query`. Targets are resolved by `AnchorRef::Entity(e)`
///    or `AnchorRef::Name(n) → AnchorNameRegistry::find_entity_by_name`
///    honoring `Display::None` per D9.
/// 3. `kahn_anchor_sort` topo-sorts the DAG, dropping a (cycle-source,
///    target) edge from the most-recently-inserted node in each cycle
///    (D4). Per D8 both endpoints of every dropped edge get
///    `LayoutAnchorBroken`.
/// 4. Detect `DuplicateName` (D11) by scanning
///    `AnchorNameRegistry::iter_buckets` for `bucket.len() > 1`.
/// 5. Walk the topological order. For each anchored entity, evaluate
///    `position_try` against this frame's Taffy output + viewport. The
///    first try whose conditions all pass wins. If every try fails,
///    write `Vec2::ZERO` and mark `LayoutAnchorBroken`.
/// 6. Idempotent `LayoutAnchorBroken` marker management — insert only
///    when missing, remove only when present (Phase 5 precedent).
///    Covers anchored entities AND non-anchor cycle targets (D8).
/// 7. Emit one `warn!` per unique `(entity, kind)` per frame (D5).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2 + § 3.4.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn anchor_resolution(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
    anchored_query: Query<(Entity, &Anchor, Option<&LayoutAnchorBroken>), With<Node>>,
    display_query: Query<&Display>,
    broken_query: Query<(Entity, Option<&LayoutAnchorBroken>)>,
    reg: Res<AnchorNameRegistry>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut overrides: ResMut<PostTaffyPositionOverrides>,
    mut warned: ResMut<LayoutAnchorWarnedThisFrame>,
) {
    // 1. Clear frame-local state. Observers do NOT contribute to
    // `warned.set` (D11) — they only update the registry. Duplicates
    // are re-detected from the registry below.
    //
    // Phase 7 — `PostTaffyPositionOverrides` is cleared by
    // `clear_post_taffy_overrides` (the first link in the
    // `BuiyLayoutStep::PostTaffyOverrides` chain), NOT here. Only the
    // anchor-specific per-frame warn set stays under this system's
    // ownership.
    warned.set.clear();

    // When no primary window is present (headless tests, multi-window
    // before the primary attaches), treat the viewport as effectively
    // unbounded. `FitsInViewport` passes trivially (no upper bound),
    // `AnchorVisible` reads the anchor box against (0, 0, MAX, MAX)
    // which is always an intersection for non-degenerate boxes.
    let viewport = primary_window
        .single()
        .ok()
        .map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(Vec2::splat(f32::MAX));

    // 2. Build edge map. The Kahn helper does its own pre-pass for
    // external target nodes (D10), so we don't insert plain-Node
    // targets here.
    let mut edges: std::collections::HashMap<Entity, Option<Entity>> =
        std::collections::HashMap::new();
    let mut new_warns: Vec<(Entity, AnchorErrorKind)> = Vec::new();

    // Helper: target resolution honoring Display::None (D9). Returns
    // Some(entity) only when the target is name-resolvable AND not
    // Display::None.
    let resolve_target = |r: &AnchorRef| -> Option<Entity> {
        let candidate = match r {
            AnchorRef::Entity(t) => Some(*t),
            AnchorRef::Name(n) => reg.find_entity_by_name(n),
        }?;
        if let Ok(Display::None) = display_query.get(candidate) {
            return None;
        }
        Some(candidate)
    };

    for (e, anchor, _) in anchored_query.iter() {
        let target = anchor.position_anchor.as_ref().and_then(&resolve_target);
        edges.insert(e, target);
        if anchor.position_anchor.is_some() && target.is_none() {
            new_warns.push((e, AnchorErrorKind::TargetMissing));
        }
    }

    // 3. Kahn sort. The helper handles external-target pre-pass and
    // cycle-edge dropping.
    let entity_epochs_fn = |e: Entity| reg.entity_epoch(e);
    let (order, dropped) = kahn_anchor_sort(&edges, &entity_epochs_fn);

    // D8 — both endpoints of a dropped cycle edge get
    // `LayoutAnchorBroken`. `dropped_targets`: the target Entity at the
    // other end of each dropped edge (read from the pre-drop edges
    // map).
    let mut dropped_targets: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for d in &dropped {
        new_warns.push((*d, AnchorErrorKind::InCycle));
        if let Some(Some(target)) = edges.get(d).copied() {
            dropped_targets.insert(target);
        }
    }

    // 4. DuplicateName detection (D11). Scan registry buckets;
    // `bucket.len() > 1` means duplicate; the last entry is the
    // late-inserter / warn target.
    for (_name, bucket) in reg.iter_buckets() {
        if bucket.len() > 1
            && let Some(&(late_entity, _)) = bucket.last()
        {
            new_warns.push((late_entity, AnchorErrorKind::DuplicateName));
        }
    }

    // 5. Walk topological order. Resolve position-try chain per entity.
    let mut broken_set: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    // Cycle endpoints are broken regardless of try-chain outcome.
    for d in &dropped {
        broken_set.insert(*d);
    }
    for t in &dropped_targets {
        broken_set.insert(*t);
    }

    for &e in &order {
        let Some((_, anchor, _existing_broken)) = anchored_query.get(e).ok() else {
            continue;
        };
        if anchor.position_anchor.as_ref().is_none() {
            continue;
        }

        if dropped.contains(&e) {
            overrides.by_entity.insert(e, Vec2::ZERO);
            // `broken_set` already contains `e`.
            continue;
        }

        let target = edges.get(&e).copied().flatten();
        let Some(target_entity) = target else {
            overrides.by_entity.insert(e, Vec2::ZERO);
            broken_set.insert(e);
            continue;
        };

        // Read anchor target's box from Taffy.
        let Some(target_taffy) = tree.by_entity.get(&target_entity).copied() else {
            overrides.by_entity.insert(e, Vec2::ZERO);
            broken_set.insert(e);
            new_warns.push((e, AnchorErrorKind::TargetMissing));
            continue;
        };
        let Ok(target_layout) = tree.tree.layout(target_taffy) else {
            overrides.by_entity.insert(e, Vec2::ZERO);
            broken_set.insert(e);
            new_warns.push((e, AnchorErrorKind::TargetMissing));
            continue;
        };
        // D1 fix — closes Phase 6 follow-up "Anchor positioning —
        // anchor target IS sticky/table/multicol." When the anchor
        // target itself was displaced by sub-pass 6a (sticky), 6b
        // (table), or 6c (multicol), its corrected position lives in
        // `overrides.by_entity`. Reading from the override map first
        // (fallback to Taffy) lets `position_try` evaluate against the
        // *displaced* target box, which is what an author expects when
        // they anchor a tooltip to a sticky header.
        //
        // Only *position* is overridden per D1; size always comes from
        // Taffy (sub-passes 6a-6c do not modify size).
        let anchor_pos = overrides
            .by_entity
            .get(&target_entity)
            .copied()
            .unwrap_or_else(|| Vec2::new(target_layout.location.x, target_layout.location.y));
        let anchor_size = Vec2::new(target_layout.size.width, target_layout.size.height);

        // Anchored entity's own size (from Taffy).
        let anchored_size = tree
            .by_entity
            .get(&e)
            .copied()
            .and_then(|id| tree.tree.layout(id).ok())
            .map(|l| Vec2::new(l.size.width, l.size.height))
            .unwrap_or(Vec2::ZERO);

        // Iterate `position_try`; first passing wins.
        let mut resolved_position: Option<Vec2> = None;
        for try_ in &anchor.position_try {
            let candidate = try_anchored_position(
                anchor_pos,
                anchor_size,
                anchored_size,
                &try_.inset,
                viewport,
            );
            let candidate_rect = (candidate, anchored_size);
            let anchor_rect = (anchor_pos, anchor_size);
            if try_conditions_pass(
                &try_.conditions,
                candidate_rect,
                anchor_rect,
                viewport,
                &tree,
                &reg,
                &display_query,
            ) {
                resolved_position = Some(candidate);
                break;
            }
        }

        match resolved_position {
            Some(pos) => {
                overrides.by_entity.insert(e, pos);
                // `broken_set` does NOT contain `e` — idempotent
                // remove fires below.
            }
            None => {
                overrides.by_entity.insert(e, Vec2::ZERO);
                broken_set.insert(e);
                new_warns.push((e, AnchorErrorKind::AllFallbacksFailed));
            }
        }
    }

    // 6. Idempotent `LayoutAnchorBroken` marker management. Iterate
    // over every entity that could currently have or need the marker —
    // anchored entities (anchored_query) AND dropped_targets (which
    // may be plain Nodes without Anchor). Use `broken_query` to read
    // the current marker state for the non-anchored set.
    for (e, _, existing_broken) in anchored_query.iter() {
        let is_broken = broken_set.contains(&e);
        if is_broken && existing_broken.is_none() {
            commands.entity(e).insert(LayoutAnchorBroken);
        } else if !is_broken && existing_broken.is_some() {
            commands.entity(e).remove::<LayoutAnchorBroken>();
        }
    }
    // Also handle plain-Node targets in `dropped_targets` (they may not
    // be in anchored_query but still need the marker per D8).
    for &t in &dropped_targets {
        if let Ok((_, existing_broken)) = broken_query.get(t)
            && existing_broken.is_none()
        {
            commands.entity(t).insert(LayoutAnchorBroken);
        }
    }
    // Cleanup: remove `LayoutAnchorBroken` from entities NOT in
    // `broken_set` but currently carrying the marker AND not in
    // `anchored_query` (anchored case handled above). Covers the case
    // where a previously cycle-broken plain-Node target becomes
    // un-broken.
    for (t, existing_broken) in broken_query.iter() {
        if existing_broken.is_some() && !broken_set.contains(&t) && anchored_query.get(t).is_err() {
            commands.entity(t).remove::<LayoutAnchorBroken>();
        }
    }

    // 7. Emit warns (one per unique `(entity, kind)` per frame).
    for (entity, kind) in new_warns {
        if warned.set.insert((entity, kind)) {
            match kind {
                AnchorErrorKind::TargetMissing => {
                    warn!(?entity, "buiy: anchor target missing or has Display::None");
                }
                AnchorErrorKind::AllFallbacksFailed => {
                    warn!(?entity, "buiy: every position_try fallback failed");
                }
                AnchorErrorKind::InCycle => {
                    warn!(
                        ?entity,
                        "buiy: anchor cycle detected; dropped this entity's outgoing edge (both cycle endpoints marked LayoutAnchorBroken)"
                    );
                }
                AnchorErrorKind::DuplicateName => {
                    warn!(
                        ?entity,
                        "buiy: duplicate anchor_name — late inserter wins, shadowed entries lose name lookup"
                    );
                }
                AnchorErrorKind::AnchorSizeUsed => {
                    warn!(
                        ?entity,
                        "buiy: anchor-size() in PositionTry::inset is deferred to v1.x; resolving to 0"
                    );
                }
            }
        }
    }
}

/// Step 0 — drop Taffy nodes for entities whose `Node` component was
/// removed (despawn or component-remove). `RemovedComponents<Node>`
/// ordering across a parent/child despawn pair is not guaranteed by
/// Bevy, so the GC tolerates either order: parent-first leaves children
/// orphaned in Taffy (cleaned up by entity), child-first leaves the
/// parent's `set_children` reference dangling (Taffy's `remove(parent)`
/// cleans that up).
///
/// Phase 1 keeps Phase 0's blanket-warn behavior. The spec's
/// architecture.md § 4.3 calls for silently swallowing `NotFound`; the
/// Taffy 0.10 error variant for that case is uncertain enough that the
/// pinning is deferred to a follow-up task that audits Taffy's error
/// enum and refines the match.
pub(super) fn gc_removed_nodes(
    mut tree: NonSendMut<LayoutTree>,
    mut removed: RemovedComponents<Node>,
) {
    let tree = &mut *tree;
    for entity in removed.read() {
        if let Some(id) = tree.by_entity.remove(&entity)
            && let Err(err) = tree.tree.remove(id)
        {
            warn!(?entity, ?err, "buiy: layout gc remove failed");
        }
    }
}

/// Step 1 — for every entity with `Node`, translate its decomposed
/// components into a `taffy::Style` and ensure the entity has a Taffy
/// node + correct child list. The query carries an `Or<(Changed<...>)>`
/// filter so steady-state frames (no layout component or hierarchy
/// changes anywhere in the world) iterate **zero** entities, matching
/// spec architecture.md § 9's O(0) steady-state contract.
///
/// `Changed<T>` triggers on insertion as well as modification, so newly
/// spawned entities are picked up on their first frame.
///
/// Phase 4 trigger set: `Changed<BoxModel>`, `Changed<Display>`,
/// `Changed<Position>`, `Changed<FlexParams>`, `Changed<FlexItem>`,
/// `Changed<Overflow>`, `Changed<Scroll>`, `Changed<GridParams>`,
/// `Changed<GridItem>`, `Changed<WritingMode>`, `Changed<WritingModeResolved>`,
/// `Changed<Children>`, `Changed<ChildOf>`. Phase 5 widens with
/// `Changed<Container>`, `Changed<ContainerQuery>`,
/// `Changed<ContainerQueryActive>`, `Changed<ContainerQueryInactive>`,
/// and `Changed<ResolvedLayout>` (Task 7 — see the inline comment on
/// the filter for the container-unit cascade rationale). Phases 6–9
/// widen further as new components land. `Changed<ChildOf>` is
/// included so that re-parenting a grid item under a different grid
/// container picks up the new container's `template_areas`.
/// `Changed<WritingMode>` triggers when an author edits the entity's
/// own writing mode; `Changed<WritingModeResolved>` triggers after
/// `inherit_writing_mode` (pre-step-1) re-derives the resolved cache
/// for an entity whose effective writing mode actually changed (the
/// inherit system is careful to skip writes when the value is
/// unchanged, preserving the O(0) steady-state contract). The four
/// new Phase 5 container/CQ entries are nested under a single inner
/// `Or<(..)>` to stay under Bevy 0.18's 15-element tuple cap on the
/// outer `Or`; a nested `Or` counts as a single outer entry.
///
/// **`Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` are
/// intentionally excluded.** `ScrollOffset` is runtime state (mutated
/// every scroll-input frame) and `ScrollSnapItem` is consumed by the
/// snap-point math in `buiy-input-events-design`, not by layout. Their
/// exclusion is asserted by `tests/layout_scroll_offset_no_invalidate.rs`.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn sync_styles(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<
        (
            Entity,
            &Display,
            &BoxModel,
            &Containment,
            &Position,
            &FlexParams,
            Option<&FlexItem>,
            &Overflow,
            &Scroll,
            &GridParams,
            Option<&GridItem>,
            &WritingModeResolved,
            Option<&Children>,
            Option<&ChildOf>,
        ),
        (
            With<Node>,
            Or<(
                Changed<Display>,
                Changed<BoxModel>,
                Changed<Position>,
                Changed<FlexParams>,
                Changed<FlexItem>,
                Changed<Overflow>,
                Changed<Scroll>,
                Changed<GridParams>,
                Changed<GridItem>,
                Changed<WritingMode>,
                Changed<WritingModeResolved>,
                Changed<Children>,
                Changed<ChildOf>,
                // Phase 5 Task 7: container units (`Length::Cq*`) resolve
                // against the entity's nearest queried ancestor's
                // *previous-frame* `ResolvedLayout`. When an ancestor's
                // resolved size changes — or when the entity is freshly
                // added with a not-yet-resolved ancestor — the cascade
                // surfaces as `Changed<ResolvedLayout>` on this entity
                // (its own size is what just shifted to track the
                // ancestor's previous-frame value). Including it here
                // re-translates the entity on the next frame so the new
                // ancestor size flows through. Phase 2 invariant intact:
                // ScrollOffset / ScrollSnapItem stay excluded, and
                // ResolvedLayout in steady-state does not refresh
                // (Bevy 0.18 `Commands::insert` increments the change
                // tick on every write, but the per-frame work this
                // produces is bounded by the actual size cascade —
                // entities whose computed Taffy size is genuinely
                // stable converge after at most one extra frame and
                // then stop firing).
                Changed<ResolvedLayout>,
                // Phase 5 Task 9 / Phase 6 Task 9: container/CQ + Anchor
                // change set. Nested under a single inner `Or` so the
                // outer tuple stays at 15 entries (Bevy 0.18 caps `Or`
                // tuples at 15). The semantics are identical to spelling
                // the entries at the top level — `Or<(A, Or<(B, C)>)>`
                // matches exactly when `A || B || C`.
                //
                // Phase 6: `Changed<Anchor>` joins the inner Or so
                // `sync_styles` re-translates an entity when its Anchor
                // component is inserted/modified — the entity may need a
                // Taffy node sync if it was just spawned, and the
                // anchor-resolution sub-pass 6d only consults Taffy
                // layouts for entities whose nodes are up to date.
                // `LayoutAnchorBroken` is intentionally OMITTED: it's a
                // devtools marker that doesn't affect Taffy translation.
                //
                // Phase 7: `Changed<MultiColumn>` joins the inner Or per
                // spec architecture.md § 1.2 line 42. The trigger is
                // currently a no-op — multicol doesn't feed Taffy in v1
                // (sub-pass 6c is a warn-once-per-session stub) — but the
                // hook is wired now so the v1.x packing algorithm flows
                // through `sync_styles` without a filter widening.
                // Inner Or<> grows 5 → 6 entries (cap 15).
                Or<(
                    Changed<Container>,
                    Changed<ContainerQuery>,
                    Changed<ContainerQueryActive>,
                    Changed<ContainerQueryInactive>,
                    Changed<Anchor>,
                    Changed<MultiColumn>,
                    // Phase 8: re-translate when `Containment` changes so
                    // the SIZE / INLINE_SIZE auto-size zeroing (spec § 5.1)
                    // flows through `style_to_taffy`. Inner Or<> grows
                    // 6 → 7 entries (cap 15).
                    Changed<Containment>,
                )>,
            )>,
        ),
    >,
    parent_grid_lookup: Query<&GridParams>,
    container_snapshot_source: Query<(Entity, &Container, &ResolvedLayout)>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    cq_parent_chain: Query<&ChildOf>,
    // Full (UNfiltered) node set for the children-sync pass. The main
    // `nodes` query above is `Changed`-filtered, so a deep descendant that
    // flips `Position::Fixed -> Absolute` enters `nodes` but its REAL parent
    // (whose `Children` did not change) does not — leaving the parent's Taffy
    // child list never rebuilt and the now-Absolute node orphaned-attached to
    // the root from the prior frame. The children-sync pass must therefore see
    // the whole tree (mirroring `stacking_context`'s unfiltered full-tree
    // query) so every parent's child list is rebuilt from current Fixed-status.
    fixed_sync_nodes: Query<(Entity, Option<&Children>, Option<&ChildOf>, &Position), With<Node>>,
    mut iter_count: ResMut<SyncStylesIterCount>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    let tree = &mut *tree;

    // Phase 5 Task 9 — publish the per-frame iter count for the Phase 2
    // O(0) steady-state invariant assertion in
    // `tests/layout_container_queries.rs`. Cheap: iter is just over the
    // matched archetypes, and in steady state the filter matches zero
    // entities (which is the entire point of the assertion).
    iter_count.0 = nodes.iter().count();

    // Precompute parent-areas: for every entity in the changed set, look
    // up its parent's `GridParams.template_areas` (if any). This map is
    // small — one entry per entity in the changed set with a parent that
    // declares template_areas — and avoids a per-entity query inside the
    // iteration. ChildOf is followed once. The second `Query<&GridParams>`
    // parameter is read-only and therefore conflict-free with the main
    // (filtered) query under Bevy 0.18.
    let parent_areas_for: HashMap<Entity, GridAreas> = nodes
        .iter()
        .filter_map(|(entity, .., parent)| {
            let p = parent?;
            let grid = parent_grid_lookup.get(p.parent()).ok()?;
            grid.template_areas.clone().map(|a| (entity, a))
        })
        .collect();

    // Build the per-entity container-size snapshot once per frame.
    // One pass over all `Container` carriers (the count is small —
    // query containers are sparse compared to leaf nodes), keyed by
    // entity. The per-entity ancestor walk below resolves the nearest
    // queried ancestor by walking `ChildOf` and looking up this index.
    // `Normal` containers are skipped — only `Size` / `InlineSize` are
    // query targets. Spec § 1.4.
    let container_index: HashMap<Entity, ContainerSnapshot> = container_snapshot_source
        .iter()
        .filter_map(|(entity, container, layout)| {
            if container.container_type == ContainerType::Normal {
                None
            } else {
                Some((
                    entity,
                    ContainerSnapshot {
                        container_type: container.container_type,
                        size: layout.size,
                    },
                ))
            }
        })
        .collect();

    // Viewport fallback. Phase 5 reads the primary window inline;
    // Phase 10's `Length::Vw/Vh` infrastructure will replace this read
    // without behavior change.
    let viewport_size = primary_window
        .single()
        .ok()
        .map(|w| bevy::math::Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(bevy::math::Vec2::ZERO);

    // Ensure every Buiy entity has a Taffy node + current style. Insert
    // happens for entities new this frame (Changed<T> triggers on insert);
    // existing entities run set_style only when something in the trigger
    // set actually changed — see foundation/architecture.md § 1.2.
    for item in nodes.iter() {
        // SIZE / INLINE_SIZE containment with an auto size on a contained
        // axis → treated as 0px (spec § 5.1). Warn once per (entity,
        // session). The substitution itself happens in `style_to_taffy`
        // (pure); this is just the log. Lives in `sync_styles` (which
        // holds the warn resource), not `translate_one_entity` (shared
        // with `cq_flip_rerun`, which has no warn resource).
        let (entity, _, box_model, containment, .., writing_mode_resolved, _, _) = item;
        let contain = containment.contain;
        let size_all = contain.contains(ContainFlags::SIZE);
        // Inline = width under horizontal modes, height under vertical /
        // sideways modes (D5 mapping; sideways normalize to vertical).
        let inline_is_horizontal =
            matches!(writing_mode_resolved.mode, WritingModeKind::HorizontalTb);
        let zeroed_width = (size_all
            || (contain.contains(ContainFlags::INLINE_SIZE) && inline_is_horizontal))
            && matches!(box_model.width, Sizing::Auto);
        let zeroed_height = (size_all
            || (contain.contains(ContainFlags::INLINE_SIZE) && !inline_is_horizontal))
            && matches!(box_model.height, Sizing::Auto);
        if (zeroed_width || zeroed_height)
            && warned
                .set
                .insert(LayoutWarnOnceKey::SizeContainmentZeroed(entity))
        {
            bevy::log::warn!(
                "Entity {:?} has size containment (contain: size/inline-size) with an \
                 auto size on a contained axis; treating the auto size as 0px (spec § 5.1). \
                 Declare an explicit width/height.",
                entity,
            );
        }

        // content-visibility != Visible is recognized but deferred in Phase 8
        // (Auto needs last-frame ResolvedLayout + viewport + contain-intrinsic-size;
        // Hidden needs a tree-skip path). Store the value; warn once per entity.
        if !matches!(containment.content_visibility, ContentVisibility::Visible)
            && warned
                .set
                .insert(LayoutWarnOnceKey::ContentVisibilityDeferred(entity))
        {
            bevy::log::warn!(
                "Entity {:?} sets content-visibility != visible; Phase 8 stores the value \
                 but does not yet skip off-screen layout/paint (deferred). The value is \
                 recognized and will be honored in a follow-up.",
                entity,
            );
        }

        translate_one_entity(
            item,
            &parent_areas_for,
            &container_index,
            &cq_parent_chain,
            viewport_size,
            tree,
        );
    }

    // Sync child relationships for each Buiy entity (Fixed children are
    // excluded from their in-flow parent's Taffy list and attached to the
    // root instead — spec § 2.1, D1/D2/D4). Built from the UNfiltered
    // `fixed_sync_nodes` (NOT the `Changed`-filtered `nodes`): a parent's
    // Taffy child list must be rebuilt whenever ANY of its children changes
    // Fixed-status, even though the parent itself is unchanged. Iterating the
    // full tree mirrors `stacking_context` and keeps the topology a pure
    // per-frame function of `Position.kind` (D3 — no stale flag).
    let rows: Vec<(Entity, bool, Option<&Children>, Option<&ChildOf>)> = fixed_sync_nodes
        .iter()
        .map(|(entity, children, parent, position)| {
            (entity, is_fixed_root(position), children, parent)
        })
        .collect();
    sync_children_pass(&rows, tree);
}

/// Per-entity tuple emitted by `sync_styles`'s (and `cq_flip_rerun`'s)
/// main filtered query. Aliased so both systems share one tuple shape
/// — change the alias if the filter widens.
#[allow(clippy::type_complexity)]
type NodeQueryItem<'w> = (
    Entity,
    &'w Display,
    &'w BoxModel,
    &'w Containment,
    &'w Position,
    &'w FlexParams,
    Option<&'w FlexItem>,
    &'w Overflow,
    &'w Scroll,
    &'w GridParams,
    Option<&'w GridItem>,
    &'w WritingModeResolved,
    Option<&'w Children>,
    Option<&'w ChildOf>,
);

/// Per-entity translation work — build a `StyleView`, call
/// `style_to_taffy`, and either `set_style` (existing node) or
/// `new_leaf` + register (new node) on the `LayoutTree`.
///
/// Factored out of `sync_styles` so `cq_flip_rerun` (Phase 5 step 5)
/// reuses the exact same per-entity translation when re-running after
/// a same-frame activation flip. Approach B from the plan: a normal
/// system with the union of `sync_styles` + `taffy_compute` params,
/// delegating per-entity work to this helper instead of `&mut World`
/// + `SystemState`.
///
/// The children-sync pass is intentionally NOT here — Taffy's
/// `set_children` requires all child nodes to exist first, so it
/// must run in a second pass after every entity has been translated.
/// `sync_children_for_entity` is the matching helper.
pub(super) fn translate_one_entity(
    item: NodeQueryItem<'_>,
    parent_areas_for: &HashMap<Entity, GridAreas>,
    container_index: &HashMap<Entity, ContainerSnapshot>,
    cq_parent_chain: &Query<&ChildOf>,
    viewport_size: bevy::math::Vec2,
    tree: &mut LayoutTree,
) {
    let (
        entity,
        display,
        bm,
        containment,
        position,
        flex,
        flex_item,
        overflow,
        scroll,
        grid_params,
        grid_item,
        writing_mode_resolved,
        _children,
        _parent,
    ) = item;

    let nearest_container = nearest_container_with_size(entity, container_index, cq_parent_chain);
    let view = StyleView {
        display,
        box_model: bm,
        containment,
        position,
        flex_params: flex,
        flex_item,
        overflow,
        scroll,
        grid_params,
        grid_item,
        parent_areas: parent_areas_for.get(&entity),
        writing_mode_resolved,
        nearest_container,
        viewport_size,
        // Populated by sync_styles in T5 from content_visibility_skip's
        // AutoSentinel result; defaults to None here.
        content_visibility_intrinsic: None,
    };
    let taffy_style = style_to_taffy(view);
    match tree.by_entity.get(&entity).copied() {
        Some(id) => {
            if let Err(err) = tree.tree.set_style(id, taffy_style) {
                warn!(?entity, ?err, "buiy: layout set_style failed");
            }
        }
        None => match tree.tree.new_leaf(taffy_style) {
            Ok(id) => {
                tree.by_entity.insert(entity, id);
            }
            Err(err) => {
                warn!(
                    ?entity,
                    ?err,
                    "buiy: layout new_leaf failed; entity will be skipped this frame"
                );
            }
        },
    }
}

/// Whether this entity's box re-parents to the layout root in the Taffy
/// tree so its containing block is the root (spec § 2.1 `Fixed` row).
/// Pure function of `Position.kind` (D3): `Fixed` re-parents, everything
/// else keeps its in-flow Taffy parent. `Absolute` does NOT re-parent —
/// it resolves against its nearest positioned ancestor (= its real
/// Taffy parent), which is the only behavioral difference from `Fixed`.
pub(super) fn is_fixed_root(position: &Position) -> bool {
    matches!(position.kind, PositionKind::Fixed)
}

/// How step 1 should treat an entity's subtree for `content-visibility`
/// (spec § 5.2). Pure classification produced by
/// [`content_visibility_skip`].
// Wired into `sync_styles` in Phase 11 T5; `allow(dead_code)` keeps the
// gate green until that caller lands.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum SkipKind {
    /// No skip — lay the entity and its descendants out normally.
    None,
    /// `content-visibility: auto`, off-screen, with a `contain-intrinsic-size`
    /// hint (D2): give the entity the intrinsic-size as its Taffy size and
    /// detach its descendants from the Taffy tree.
    AutoSentinel { intrinsic: ContainIntrinsicSize },
    /// `content-visibility: hidden` (D7): detach the entity's descendants
    /// from the Taffy tree (the entity itself still lays out).
    HiddenPrune,
}

/// Classify an entity's `content-visibility` skip for step 1 (spec § 5.2).
///
/// - `Visible` → never skip.
/// - `Hidden` → always `HiddenPrune` (descendants detached; entity box
///   still resolves — D7).
/// - `Auto` → `AutoSentinel` only when BOTH off-screen AND a
///   `ContainIntrinsicSize` with at least one axis hint is present (D2);
///   otherwise `None` (lay out normally — the off-screen *paint* skip is
///   a render concern Phase 11 does not own).
///
/// `off_screen` is computed by the caller from last-frame `ResolvedLayout`
/// vs the hysteresis-expanded viewport ([`is_off_screen`], D3).
// Wired into `sync_styles` in Phase 11 T5; `allow(dead_code)` keeps the
// gate green until that caller lands.
#[allow(dead_code)]
pub(super) fn content_visibility_skip(
    containment: &Containment,
    intrinsic: Option<&ContainIntrinsicSize>,
    off_screen: bool,
) -> SkipKind {
    match containment.content_visibility {
        ContentVisibility::Visible => SkipKind::None,
        ContentVisibility::Hidden => SkipKind::HiddenPrune,
        ContentVisibility::Auto => match intrinsic {
            Some(h) if off_screen && h.has_hint() => SkipKind::AutoSentinel { intrinsic: *h },
            _ => SkipKind::None,
        },
    }
}

/// The viewport rectangle for the content-visibility off-screen test,
/// expanded outward by `margin` on every side (the hysteresis dead-band,
/// D3). Origin is the layout root's top-left `(0, 0)`; `viewport_size`
/// is the primary window size (or `Vec2::ZERO` when window-less).
// Wired into `sync_styles` in Phase 11 T5; `allow(dead_code)` keeps the
// gate green until that caller lands.
#[allow(dead_code)]
pub(super) fn viewport_rect(viewport_size: Vec2, margin: f32) -> Rect {
    Rect {
        min: Vec2::new(-margin, -margin),
        max: viewport_size + Vec2::splat(margin),
    }
}

/// Whether an entity is "off-screen" for `content-visibility: auto`
/// (spec § 5.2, D3): its *last-frame* `ResolvedLayout` border box does
/// NOT intersect the hysteresis-expanded viewport. An entity with no
/// resolved layout yet (first frame) is treated as on-screen — we have
/// no geometry to skip against.
// Wired into `sync_styles` in Phase 11 T5; `allow(dead_code)` keeps the
// gate green until that caller lands.
#[allow(dead_code)]
pub(super) fn is_off_screen(resolved: Option<&ResolvedLayout>, expanded_viewport: Rect) -> bool {
    let Some(rl) = resolved else {
        return false;
    };
    let box_rect = Rect::from_corners(rl.position, rl.position + rl.size);
    // Off-screen iff the boxes do not overlap. `Rect::intersect` returns
    // an empty rect (zero positive area) when there is no overlap.
    expanded_viewport.intersect(box_rect).is_empty()
}

/// The whole second pass: build the set of entities that re-parent to the
/// layout root (`Position::Fixed` — spec § 2.1, D3) once, then call
/// `sync_children_for_entity` for every entity so each parent's Taffy
/// child list excludes its `Fixed` children (D1/D4), and finally attach
/// the excluded `Fixed` nodes to the layout root's Taffy child list
/// (D1/D2). Both callers (`sync_styles`, `cq_flip_rerun`) run this whole
/// pass so the Taffy topology stays in lock-step.
///
/// Takes a `(entity, is_fixed, children, parent)` row per entity (collected
/// by the caller from its `NodeQueryItem` query) rather than the `Query`
/// itself: `Query<D, F>` is invariant over `D`, so the two callers' distinct
/// `Changed<...>` filters cannot share one generic signature. Collecting
/// the rows up front is the single point that cannot drift (D4).
fn sync_children_pass(
    rows: &[(Entity, bool, Option<&Children>, Option<&ChildOf>)],
    tree: &mut LayoutTree,
) {
    // Entities whose box re-parents to the layout root in the Taffy tree
    // (Position::Fixed — spec § 2.1). Built once; consumed below both to
    // exclude Fixed from their in-flow parent's child list and to attach
    // them to the root's child list.
    let fixed_set: HashSet<Entity> = rows
        .iter()
        .filter(|(_, is_fixed, _, _)| *is_fixed)
        .map(|(entity, _, _, _)| *entity)
        .collect();

    for &(entity, _, children, _) in rows {
        sync_children_for_entity(entity, children, &fixed_set, tree);
    }

    attach_fixed_to_root(rows, &fixed_set, tree);
}

/// Attach every `Fixed` node to the layout ROOT's Taffy child list so
/// Taffy's native absolute algorithm resolves them (including percentage
/// insets) against the root's content box — the sole behavioral difference
/// from `Absolute` (spec § 2.1 `Fixed` row; D1/D2).
///
/// Root = the existing root-detection rule (no `ChildOf`, or a `ChildOf`
/// whose parent is not in `LayoutTree`) shared with `taffy_compute` /
/// `stacking_context`. Single global tree: the first matching root wins
/// (D2). The root's own in-flow children were already set by
/// `sync_children_for_entity` (Fixed excluded); we re-set with the union of
/// (in-flow non-Fixed) ∪ (all Fixed) so we do not clobber them and Fixed
/// appears exactly once.
fn attach_fixed_to_root(
    rows: &[(Entity, bool, Option<&Children>, Option<&ChildOf>)],
    fixed_set: &HashSet<Entity>,
    tree: &mut LayoutTree,
) {
    if fixed_set.is_empty() {
        return;
    }

    // The Fixed node ids, in `rows` iteration order (D2).
    let fixed_node_ids: Vec<TaffyNodeId> = rows
        .iter()
        .filter(|(_, is_fixed, _, _)| *is_fixed)
        .filter_map(|(entity, _, _, _)| tree.by_entity.get(entity).copied())
        .collect();

    for &(entity, _, children, parent) in rows {
        let is_root = parent
            .map(|p| !tree.by_entity.contains_key(&p.parent()))
            .unwrap_or(true);
        if !is_root {
            continue;
        }
        let Some(root_id) = tree.by_entity.get(&entity).copied() else {
            continue;
        };
        // In-flow children of the root, Fixed excluded — mirrors the filter
        // in `sync_children_for_entity` so we reproduce exactly what it set.
        let mut child_ids: Vec<TaffyNodeId> = children
            .into_iter()
            .flatten()
            .filter(|c| !fixed_set.contains(c))
            .filter_map(|c| tree.by_entity.get(c).copied())
            .collect();
        // Append every Fixed node, skipping the root itself in the
        // degenerate "root is Fixed" case (a node cannot parent itself).
        child_ids.extend(fixed_node_ids.iter().copied().filter(|&fid| fid != root_id));
        if let Err(err) = tree.tree.set_children(root_id, &child_ids) {
            warn!(
                ?entity,
                ?err,
                "buiy: layout set_children (fixed root attach) failed"
            );
        }
        break; // single global tree — attach to the first root only (D2).
    }
}

/// Per-entity child-sync — second-pass companion to
/// `translate_one_entity`. Taffy's `set_children` requires all child
/// nodes to exist first, so this must run after every entity has been
/// translated. Factored out so `cq_flip_rerun` can re-use it.
///
/// `fixed_set` carries the entities that re-parent to the layout root
/// (`Position::Fixed` — spec § 2.1, D4): they are excluded from their
/// in-flow parent's Taffy child list here, and attached to the root's
/// list in `sync_children_pass` (Phase 10 T4).
fn sync_children_for_entity(
    entity: Entity,
    children: Option<&Children>,
    fixed_set: &HashSet<Entity>,
    tree: &mut LayoutTree,
) {
    let parent_id = match tree.by_entity.get(&entity).copied() {
        Some(id) => id,
        None => return,
    };
    let child_ids: Vec<TaffyNodeId> = children
        .into_iter()
        .flatten()
        // Fixed children re-parent to the layout root (spec § 2.1) —
        // exclude them from their in-flow parent's Taffy child list.
        // The root-attach happens in `sync_children_pass` (T4).
        .filter(|c| !fixed_set.contains(c))
        .filter_map(|c| tree.by_entity.get(c).copied())
        .collect();
    if let Err(err) = tree.tree.set_children(parent_id, &child_ids) {
        warn!(?entity, ?err, "buiy: layout set_children failed");
    }
}

/// Step 3 — call `tree.compute_layout` from each root. A root is an
/// entity with `Node` and either no `ChildOf`, or a `ChildOf` whose
/// target is not in `LayoutTree` (i.e., a non-Buiy parent).
///
/// Resets `LayoutTaffyComputeCount` to zero at the start of each
/// invocation (i.e. once per frame) and bumps it after every
/// successful `compute_layout`. `cq_flip_rerun` (step 5) bumps the
/// same counter when it re-runs, so a flip frame ends with
/// `count == 2 * roots` and a non-flip frame with `count == roots`.
/// The Phase 5 "cap at 2× Taffy per frame" architecture invariant
/// is asserted by `tests/layout_container_queries.rs`.
pub(super) fn taffy_compute(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<(Entity, Option<&ChildOf>), With<Node>>,
    windows: Query<&bevy::window::Window>,
    mut compute_count: ResMut<LayoutTaffyComputeCount>,
) {
    let tree = &mut *tree;

    // Frame-start reset. `cq_flip_rerun` increments without resetting,
    // so the counter ends each frame at exactly the number of Taffy
    // invocations (1 for non-flip, 2 for flip).
    compute_count.0 = 0;

    // Layout root sizing falls back to 800x600 if no Window exists (test
    // harnesses with MinimalPlugins). Phase 0 used the same default.
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));

    for (entity, parent) in nodes.iter() {
        let is_root = parent
            .map(|p| !tree.by_entity.contains_key(&p.parent()))
            .unwrap_or(true);
        if !is_root {
            continue;
        }
        if let Some(id) = tree.by_entity.get(&entity).copied() {
            match tree.tree.compute_layout(
                id,
                Size {
                    width: AvailableSpace::Definite(window_size.x),
                    height: AvailableSpace::Definite(window_size.y),
                },
            ) {
                Ok(_) => {
                    compute_count.0 += 1;
                }
                Err(err) => {
                    warn!(?entity, ?err, "buiy: layout compute_layout failed");
                }
            }
        }
    }
}

/// Step 7 — read `tree.layout(id)` for every tracked entity and write
/// the resulting position+size into `ResolvedLayout`. On Taffy `Err`,
/// retain the previous frame's value.
///
/// **Idempotent insert** — only writes when the new value differs from
/// the entity's current `ResolvedLayout` (Phase 5 Task 7). Without this
/// guard, `Commands::insert` would refresh the change tick every frame,
/// keeping `Changed<ResolvedLayout>` perpetually true — which would
/// cascade `sync_styles` into iterating every node every frame (Task 7
/// added `Changed<ResolvedLayout>` to the Or-filter so the
/// container-unit size cascade can re-translate descendants). With the
/// guard, `Changed<ResolvedLayout>` fires only on actual size /
/// position changes, preserving the O(0) steady-state contract
/// (Phase 2 invariant).
pub(super) fn write_resolved_layout(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
    existing: Query<&ResolvedLayout>,
    overrides: Res<PostTaffyPositionOverrides>,
) {
    let mut to_write: Vec<(Entity, ResolvedLayout)> = Vec::new();
    for (&entity, &id) in tree.by_entity.iter() {
        if let Ok(layout) = tree.tree.layout(id) {
            // Phase 6/7 — any `PostTaffyOverrides` sub-pass (sticky,
            // table, multicol, anchor) may have written a position
            // override for this entity. Size is always from Taffy; only
            // position is overridden.
            let position = overrides
                .by_entity
                .get(&entity)
                .copied()
                .unwrap_or_else(|| Vec2::new(layout.location.x, layout.location.y));
            let new = ResolvedLayout {
                position,
                size: Vec2::new(layout.size.width, layout.size.height),
            };
            let unchanged = existing
                .get(entity)
                .map(|cur| cur.position == new.position && cur.size == new.size)
                .unwrap_or(false);
            if !unchanged {
                to_write.push((entity, new));
            }
        }
    }
    for (e, rl) in to_write {
        commands.entity(e).insert(rl);
    }
}

/// Pre-step-1 — populate `WritingModeResolved` for every `Node` entity
/// from the nearest ancestor with `WritingMode`, falling back to default
/// when no ancestor sets it.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.2.
///
/// Implementation:
/// 1. Resolve each entity's effective `WritingMode` by walking up the
///    `ChildOf` chain until a `WritingMode` is found (or the root is
///    reached, falling back to `default`).
/// 2. Memoize the resolution: each entity's effective value is computed
///    at most once per frame, even when many descendants share an
///    ancestor — total cost O(N), not O(N × depth).
/// 3. Compare against the entity's current `WritingModeResolved`. Only
///    `commands.insert(...)` when the value actually changes — avoids
///    cascading `Changed<WritingModeResolved>` to `sync_styles` every
///    frame, which would void the O(0) steady-state contract.
pub(super) fn inherit_writing_mode(
    mut commands: Commands,
    nodes: Query<(Entity, Option<&WritingModeResolved>), With<Node>>,
    wm_lookup: Query<&WritingMode>,
    parent_chain: Query<&ChildOf>,
) {
    let mut memo: HashMap<Entity, WritingMode> = HashMap::new();

    for (entity, current) in nodes.iter() {
        let effective = resolve_writing_mode(entity, &mut memo, &wm_lookup, &parent_chain);
        let new_resolved = WritingModeResolved::from_writing_mode(&effective);
        if current.copied() != Some(new_resolved) {
            commands.entity(entity).insert(new_resolved);
        }
    }
}

/// Walk up the `ChildOf` chain memoizing each ancestor's effective
/// `WritingMode`. Recursive on the parent path; depth bounded by the
/// hierarchy depth.
///
/// CSS-faithful "default = inherit" semantic: a `WritingMode` whose
/// fields are all default (`HorizontalTb + Ltr + Mixed + Normal`) is
/// treated as **unset** for inheritance purposes. This matters because
/// `Style` (Task 6) inserts `WritingMode::default()` into every
/// Style-spawned entity's Bundle — without the default-equals-unset
/// rule, no descendant would ever inherit (every entity would
/// short-circuit on its own default-valued component). Spec § 2.2:
/// "its own `WritingMode` if set, else the nearest ancestor's".
///
/// Trade-off: an author cannot explicitly *override* an ancestor with
/// the all-defaults value — the override is observationally identical
/// to inheriting whatever defaults bubble up from the root. Since the
/// root's fallback is also `WritingMode::default()`, this is a
/// no-observable-difference distinction.
fn resolve_writing_mode(
    entity: Entity,
    memo: &mut HashMap<Entity, WritingMode>,
    wm_lookup: &Query<&WritingMode>,
    parent_chain: &Query<&ChildOf>,
) -> WritingMode {
    if let Some(cached) = memo.get(&entity) {
        return *cached;
    }
    let own = wm_lookup.get(entity).ok().copied();
    let effective = match own {
        Some(wm) if wm != WritingMode::default() => wm,
        _ => match parent_chain.get(entity) {
            Ok(p) => resolve_writing_mode(p.parent(), memo, wm_lookup, parent_chain),
            Err(_) => WritingMode::default(),
        },
    };
    memo.insert(entity, effective);
    effective
}

/// Pure evaluation of a `ContainerQuery`'s condition list against a
/// resolved container size. Returns `true` iff *every* condition holds
/// (CSS `@container` is AND-combined). Empty `conditions` = always
/// active (matches `@container (width)` which holds iff a container
/// exists at all).
///
/// Length units inside `MinWidth`/`MaxWidth`/`MinHeight`/`MaxHeight`
/// are resolved to absolute pixels:
/// - `Px(v)` → `v`.
/// - `Percent(p)` → `p%` of the container's own resolved size on the
///   relevant axis (CSS-faithful — percentage in a `@container` query
///   resolves against the container).
/// - `Fr` / `Cq*` → 0 (warn-once at translate time, not here; this
///   helper is pure and the `Length::Px` case is the common path).
pub(super) fn evaluate_conditions(conds: &[QueryCondition], container: Vec2) -> bool {
    use QueryCondition::*;
    conds.iter().all(|c| match *c {
        MinWidth(len) => container.x >= length_to_px(len, container.x),
        MaxWidth(len) => container.x <= length_to_px(len, container.x),
        MinHeight(len) => container.y >= length_to_px(len, container.y),
        MaxHeight(len) => container.y <= length_to_px(len, container.y),
        MinAspectRatio(r) => {
            if container.y == 0.0 {
                0.0 >= r
            } else {
                (container.x / container.y) >= r
            }
        }
        MaxAspectRatio(r) => {
            if container.y == 0.0 {
                // h == 0 → undefined; do not match.
                false
            } else {
                (container.x / container.y) <= r
            }
        }
        Orientation(o) => match o {
            crate::layout::types::Orientation::Portrait => container.x <= container.y,
            crate::layout::types::Orientation::Landscape => container.x > container.y,
        },
    })
}

fn length_to_px(len: Length, axis_basis: f32) -> f32 {
    match len {
        Length::Px(v) => v,
        Length::Percent(p) => p * 0.01 * axis_basis,
        // Phase 5 container queries don't recurse — Cq* inside a
        // condition value would be a degenerate case (a rule about
        // a container, sized in units of that same container). Warn
        // is unnecessary because authors compose with Length::Px.
        // Fr is a grid-only unit; degrades to 0 here.
        Length::Fr(_)
        | Length::Cqw(_)
        | Length::Cqh(_)
        | Length::Cqi(_)
        | Length::Cqb(_)
        | Length::Cqmin(_)
        | Length::Cqmax(_) => 0.0,
    }
}

/// Step 2 (`BuiyLayoutStep::CqActivate`) — for each entity with
/// `ContainerQuery`, find the matching container ancestor and toggle
/// `ContainerQueryActive` / `ContainerQueryInactive` based on whether
/// every condition holds against the ancestor's *previous frame*
/// resolved size.
///
/// Memoization mirrors `inherit_writing_mode`'s ancestor walk
/// (systems.rs:308-362): one `HashMap<Entity, Option<Entity>>` per
/// system call; entries cached as the walk descends and reused by
/// siblings sharing an ancestor. Per spec § 1.3 step 2, the read is
/// of *previous frame's* `ResolvedLayout` — at `CqActivate` time
/// (between `SyncStyles` and `TaffyCompute`) the `ResolvedLayout`
/// component still holds what step 7 wrote last frame.
///
/// Idempotent flip — only `commands.insert(...)` when the marker would
/// change. Avoids `Changed<ContainerQueryActive>` cascading into
/// `sync_styles` every frame, which would void the O(0) steady-state
/// contract (Phase 2 invariant; mirror of Phase 4 systems.rs:319-321).
#[allow(clippy::type_complexity)]
pub(super) fn cq_activate(
    mut commands: Commands,
    rules: Query<
        (
            Entity,
            &ContainerQuery,
            Option<&ContainerQueryActive>,
            Option<&ContainerQueryInactive>,
        ),
        With<Node>,
    >,
    containers: Query<(&Container, &ResolvedLayout)>,
    parent_chain: Query<&ChildOf>,
) {
    let mut memo: HashMap<Entity, Option<Entity>> = HashMap::new();

    for (entity, rule, was_active, was_inactive) in rules.iter() {
        let container_entity = resolve_nearest_container(
            entity,
            &rule.container,
            &mut memo,
            &containers,
            &parent_chain,
        );

        let active = match container_entity {
            Some(c) => match containers.get(c) {
                Ok((_container, layout)) => evaluate_conditions(&rule.conditions, layout.size),
                Err(_) => false,
            },
            None => {
                // No container ancestor → rule cannot activate.
                false
            }
        };

        // Idempotent flip.
        if active && was_active.is_none() {
            commands
                .entity(entity)
                .insert(ContainerQueryActive)
                .remove::<ContainerQueryInactive>();
        } else if !active && was_inactive.is_none() {
            commands
                .entity(entity)
                .insert(ContainerQueryInactive)
                .remove::<ContainerQueryActive>();
        }
    }
}

/// Walk up `ChildOf` from `entity`, returning the snapshot for the
/// first ancestor present in `lookup`. Not memoized across entities —
/// depth is bounded by hierarchy depth and the changed-set size
/// (Phase 2 invariant: most frames the set is empty). A memo across
/// entities is a future optimization; v1 keeps the helper stateless.
///
/// Used by `sync_styles` to resolve the nearest queried ancestor's
/// snapshot for `Length::Cq*` resolution at the `style_to_taffy`
/// boundary. Spec § 1.4.
fn nearest_container_with_size(
    entity: Entity,
    lookup: &HashMap<Entity, ContainerSnapshot>,
    parent_chain: &Query<&ChildOf>,
) -> Option<ContainerSnapshot> {
    let mut cur = entity;
    loop {
        let parent = parent_chain.get(cur).ok()?.parent();
        if let Some(snap) = lookup.get(&parent) {
            return Some(*snap);
        }
        cur = parent;
    }
}

/// Walk up `ChildOf` from `entity`, returning the first ancestor that
/// is a query container (`Container::container_type != Normal`) and,
/// if `name` is `Some(n)`, has matching `container_name`. Memoized.
///
/// `cq_activate` (Task 6) reads previous-frame `ResolvedLayout` from
/// the container ancestor, which is why this version takes the wider
/// `Query<(&Container, &ResolvedLayout)>`. `cq_flip_check` reads
/// instead from `tree.layout(node_id)` (architecture.md § 3.2
/// explicit pinning) and uses the narrower `Query<&Container>` via
/// `resolve_nearest_container_by_name`. Both helpers are kept
/// separate because Bevy 0.18 query parameters are structural — the
/// wider query cannot be passed where the narrower one is expected
/// without an adapter, and adding the adapter just to share one walk
/// would obscure the per-site read-set.
pub(super) fn resolve_nearest_container(
    entity: Entity,
    name: &Option<String>,
    memo: &mut HashMap<Entity, Option<Entity>>,
    containers: &Query<(&Container, &ResolvedLayout)>,
    parent_chain: &Query<&ChildOf>,
) -> Option<Entity> {
    if let Some(cached) = memo.get(&entity) {
        return *cached;
    }
    let result = match parent_chain.get(entity) {
        Ok(p) => {
            let parent = p.parent();
            let matches = containers.get(parent).ok().and_then(|(c, _)| {
                if c.container_type == ContainerType::Normal {
                    return None;
                }
                match (name, &c.container_name) {
                    (None, _) => Some(parent), // any queried ancestor
                    (Some(want), Some(have)) if want == have => Some(parent),
                    _ => None,
                }
            });
            match matches {
                Some(e) => Some(e),
                None => resolve_nearest_container(parent, name, memo, containers, parent_chain),
            }
        }
        Err(_) => None, // no parent → no container ancestor
    };
    memo.insert(entity, result);
    result
}

/// Name-aware ancestor walk used by `cq_flip_check`. Same shape as
/// `resolve_nearest_container` (Task 6) minus the `&ResolvedLayout`
/// read — `cq_flip_check` reads sizes from `tree.layout(node_id)`
/// (architecture.md § 3.2 explicit pinning), so the broader
/// `Query<(&Container, &ResolvedLayout)>` would over-claim what this
/// helper actually needs.
pub(super) fn resolve_nearest_container_by_name(
    entity: Entity,
    name: &Option<String>,
    memo: &mut HashMap<Entity, Option<Entity>>,
    containers: &Query<&Container>,
    parent_chain: &Query<&ChildOf>,
) -> Option<Entity> {
    if let Some(cached) = memo.get(&entity) {
        return *cached;
    }
    let result = match parent_chain.get(entity) {
        Ok(p) => {
            let parent = p.parent();
            let matches = containers.get(parent).ok().and_then(|c| {
                if c.container_type == ContainerType::Normal {
                    return None;
                }
                match (name, &c.container_name) {
                    (None, _) => Some(parent),
                    (Some(want), Some(have)) if want == have => Some(parent),
                    _ => None,
                }
            });
            match matches {
                Some(e) => Some(e),
                None => {
                    resolve_nearest_container_by_name(parent, name, memo, containers, parent_chain)
                }
            }
        }
        Err(_) => None,
    };
    memo.insert(entity, result);
    result
}

/// Step 4 (`BuiyLayoutStep::CqFlipCheck`) — re-evaluate every
/// `ContainerQuery` against this frame's fresh Taffy output. The size
/// source per architecture.md § 3.2 is **`tree.layout(node_id)`**,
/// NOT entity-side `ResolvedLayout` (which is still last-frame's
/// value because step 7 hasn't written yet this frame).
///
/// If any rule's activation differs from what `cq_activate` (step 2)
/// settled on this frame, toggle markers and set
/// `CqReRunRequested(true)`. Entities with no resolvable container
/// ancestor are treated as `active_now = false`, mirroring
/// `cq_activate`'s handling (a previously-active rule whose
/// ancestor became unavailable must be allowed to flip back).
///
/// **No `Without<ContainerQuery>` filter** on the `containers` query
/// — an entity can legitimately be both a query container AND carry a
/// `ContainerQuery` (mid-tree container reacting to its own
/// ancestor). Excluding such entities silently breaks descendant
/// resolution. Read-side concern only; `&Container` and
/// `&ContainerQuery` are disjoint components, so Bevy 0.18's borrow
/// checker doesn't require the filter.
#[allow(clippy::type_complexity)]
pub(super) fn cq_flip_check(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
    rules: Query<(Entity, &ContainerQuery, Option<&ContainerQueryActive>), With<Node>>,
    containers: Query<&Container>,
    parent_chain: Query<&ChildOf>,
    mut rerun: ResMut<CqReRunRequested>,
) {
    let mut memo: HashMap<Entity, Option<Entity>> = HashMap::new();
    let mut any_flipped = false;

    for (entity, rule, was_active) in rules.iter() {
        let container_entity = resolve_nearest_container_by_name(
            entity,
            &rule.container,
            &mut memo,
            &containers,
            &parent_chain,
        );

        let active_now = match container_entity {
            Some(c) => match tree.by_entity.get(&c) {
                Some(node_id) => match tree.tree.layout(*node_id) {
                    Ok(layout) => evaluate_conditions(
                        &rule.conditions,
                        Vec2::new(layout.size.width, layout.size.height),
                    ),
                    // Taffy doesn't know this node yet (entity hasn't
                    // been translated this frame). Treat as inactive
                    // — mirrors no-ancestor handling so a
                    // previously-active rule whose container was
                    // never translated this frame can flip back.
                    Err(_) => false,
                },
                None => false,
            },
            None => false,
        };

        let was_active_b = was_active.is_some();
        if active_now != was_active_b {
            any_flipped = true;
            if active_now {
                commands
                    .entity(entity)
                    .insert(ContainerQueryActive)
                    .remove::<ContainerQueryInactive>();
            } else {
                commands
                    .entity(entity)
                    .insert(ContainerQueryInactive)
                    .remove::<ContainerQueryActive>();
            }
        }
    }

    rerun.0 = any_flipped;
}

/// Step 5 (`BuiyLayoutStep::CqFlipReRun`) — when `cq_flip_check`
/// signaled a flip in step 4, re-run the inner work of `sync_styles`
/// and `taffy_compute` once. Cap at one re-run per frame
/// (architecture.md § 3.2: "step 4 does not re-run; transitive flips
/// wait until next frame"). At most 2× Taffy per frame.
///
/// Approach B (committed in the plan): a normal Bevy system with the
/// union of `sync_styles` + `taffy_compute` params. The
/// `SystemState`-on-`&mut World` approach is rejected because the
/// existing `sync_styles` declares `NonSendMut<LayoutTree>` and many
/// `Query<...>` params — leaving it as a "trivial wrapper" while
/// moving the body into an `&mut World` inner doesn't compose.
///
/// The work is INTENTIONALLY duplicative with `sync_styles` +
/// `taffy_compute`. The plan accepted this trade-off so the body
/// stays an ordinary system the compiler can borrow-check.
/// `translate_one_entity` is the per-entity sharing point; the
/// container-snapshot + viewport + children passes are inlined here
/// because their input shape is straightforward.
///
/// Body is gated on `CqReRunRequested.0`; when false, the system
/// returns immediately (the common, no-flip case). Bumps
/// `LayoutTaffyComputeCount` for each Taffy re-invocation so the
/// "cap at 2× Taffy" architecture invariant is observable in tests.
///
/// `clippy::too_many_arguments` is silenced because the param set is
/// the (intentional) union of `sync_styles` + `taffy_compute` — not
/// a function that could meaningfully be split. Bevy systems are
/// allowed up to 16 params; this one uses 10.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn cq_flip_rerun(
    mut rerun: ResMut<CqReRunRequested>,
    mut compute_count: ResMut<LayoutTaffyComputeCount>,
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<
        NodeQueryItem<'_>,
        (
            With<Node>,
            Or<(
                Changed<Display>,
                Changed<BoxModel>,
                Changed<Position>,
                Changed<FlexParams>,
                Changed<FlexItem>,
                Changed<Overflow>,
                Changed<Scroll>,
                Changed<GridParams>,
                Changed<GridItem>,
                Changed<WritingMode>,
                Changed<WritingModeResolved>,
                Changed<Children>,
                Changed<ChildOf>,
                Changed<ResolvedLayout>,
                // CQ-only entries — Changed<Anchor> (Phase 6) and Changed<MultiColumn>
                // (Phase 7) are intentionally excluded from the CQ flip pass because
                // neither feeds Taffy in v1; adding them here would be misleading
                // forward-compat (sync_styles has them for correctness/forward-compat).
                Or<(
                    Changed<Container>,
                    Changed<ContainerQuery>,
                    Changed<ContainerQueryActive>,
                    Changed<ContainerQueryInactive>,
                )>,
            )>,
        ),
    >,
    parent_grid_lookup: Query<&GridParams>,
    container_snapshot_source: Query<(Entity, &Container, &ResolvedLayout)>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    cq_parent_chain: Query<&ChildOf>,
    // Full (UNfiltered) node set — serves both the children-sync pass (which
    // must rebuild every parent's Taffy child list from current Fixed-status,
    // not just the `Changed`-filtered `nodes`; see `sync_styles`) and the
    // per-root Taffy `compute_layout` loop below.
    roots: Query<(Entity, Option<&Children>, Option<&ChildOf>, &Position), With<Node>>,
    windows: Query<&bevy::window::Window>,
) {
    if !rerun.0 {
        return;
    }
    rerun.0 = false;

    let tree = &mut *tree;

    // Rebuild the parent-areas + container-index + viewport snapshots
    // for the re-run. Same shape as `sync_styles`'s setup; we cannot
    // hand those off from the first pass because they're stack-local.
    // The re-run cost is bounded by the changed-set, which is small.
    let parent_areas_for: HashMap<Entity, GridAreas> = nodes
        .iter()
        .filter_map(|(entity, .., parent)| {
            let p = parent?;
            let grid = parent_grid_lookup.get(p.parent()).ok()?;
            grid.template_areas.clone().map(|a| (entity, a))
        })
        .collect();

    let container_index: HashMap<Entity, ContainerSnapshot> = container_snapshot_source
        .iter()
        .filter_map(|(entity, container, layout)| {
            if container.container_type == ContainerType::Normal {
                None
            } else {
                Some((
                    entity,
                    ContainerSnapshot {
                        container_type: container.container_type,
                        size: layout.size,
                    },
                ))
            }
        })
        .collect();

    let viewport_size = primary_window
        .single()
        .ok()
        .map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(Vec2::ZERO);

    for item in nodes.iter() {
        translate_one_entity(
            item,
            &parent_areas_for,
            &container_index,
            &cq_parent_chain,
            viewport_size,
            tree,
        );
    }
    // Children-sync over the FULL tree (`roots`), not the `Changed`-filtered
    // `nodes`: a child that flipped Fixed-status re-homes only if its (possibly
    // unchanged) parent's child list is rebuilt. See `sync_styles` for the
    // detailed rationale.
    let rows: Vec<(Entity, bool, Option<&Children>, Option<&ChildOf>)> = roots
        .iter()
        .map(|(entity, children, parent, position)| {
            (entity, is_fixed_root(position), children, parent)
        })
        .collect();
    sync_children_pass(&rows, tree);

    // Re-invoke Taffy compute. Same code shape as `taffy_compute`,
    // but WITHOUT the `compute_count.0 = 0` frame-reset (that lives
    // only in `taffy_compute`, so a flip frame ends at `count == 2`,
    // not `count == 1`).
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));
    for (entity, _children, parent, _position) in roots.iter() {
        let is_root = parent
            .map(|p| !tree.by_entity.contains_key(&p.parent()))
            .unwrap_or(true);
        if !is_root {
            continue;
        }
        if let Some(id) = tree.by_entity.get(&entity).copied() {
            match tree.tree.compute_layout(
                id,
                Size {
                    width: AvailableSpace::Definite(window_size.x),
                    height: AvailableSpace::Definite(window_size.y),
                },
            ) {
                Ok(_) => {
                    compute_count.0 += 1;
                }
                Err(err) => {
                    warn!(?entity, ?err, "buiy: layout compute_layout (re-run) failed");
                }
            }
        }
    }
}

/// Convert a `TransformMatrix` to a `Mat4`. `None` → identity.
/// `Translate`/`Rotate`/`Scale`/`Skew`/`Matrix` map directly;
/// `Compose([A, B, …])` folds to the matrix product `A · B · …`
/// (outermost first; rightmost transforms a child point first).
///
/// `Length`s in `Translate` resolve as px today (percent/cq transform
/// translates resolve against the entity's own box — deferred to the
/// render/animation phase; px is the only meaningful unit at compose
/// time for Phase 8). Non-px `Length` variants resolve to their px
/// magnitude via `Length::Px` only; other variants contribute 0.0.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
fn transform_matrix_to_mat4(m: &TransformMatrix) -> Mat4 {
    match m {
        TransformMatrix::None => Mat4::IDENTITY,
        TransformMatrix::Translate(x, y, z) => {
            Mat4::from_translation(Vec3::new(length_px(x), length_px(y), length_px(z)))
        }
        TransformMatrix::Rotate(q) => Mat4::from_quat(*q),
        TransformMatrix::Scale(x, y, z) => Mat4::from_scale(Vec3::new(*x, *y, *z)),
        TransformMatrix::Skew(ax, ay) => {
            // 2D skew: shear matrix with tan(angle) off-diagonals.
            let mut mat = Mat4::IDENTITY;
            mat.y_axis.x = ax.tan();
            mat.x_axis.y = ay.tan();
            mat
        }
        TransformMatrix::Matrix(mat) => *mat,
        TransformMatrix::Compose(list) => list.iter().fold(Mat4::IDENTITY, |acc, item| {
            acc * transform_matrix_to_mat4(item)
        }),
    }
}

/// Resolve a `Length` to px for transform translation. Only `Px` is
/// meaningful at compose time in Phase 8; other units (percent /
/// cq) resolve against the entity's own box and are deferred to the
/// render/animation phase — they contribute 0.0 here.
fn length_px(l: &Length) -> f32 {
    match l {
        Length::Px(p) => *p,
        _ => 0.0,
    }
}

/// Compose the final transform matrix per spec § 1:
/// `M = T_translate · R_rotate · S_scale · M_transform`.
/// The longhand `Translate`/`Rotate`/`Scale` (absent → identity
/// contribution) are the outer factors; `UiTransform.matrix` is the
/// innermost. A child point `p` is transformed as `M · p`, so it
/// feels the rightmost (innermost) factor first.
///
/// Pure function — no Bevy queries, no Taffy reads. Easy to unit test.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 1.1.
pub(super) fn compose_transform(
    ui: &UiTransform,
    t: Option<&Translate>,
    r: Option<&Rotate>,
    s: Option<&Scale>,
) -> Mat4 {
    let t_mat = match t {
        Some(Translate(x, y, z)) => {
            Mat4::from_translation(Vec3::new(length_px(x), length_px(y), length_px(z)))
        }
        None => Mat4::IDENTITY,
    };
    let r_mat = match r {
        Some(Rotate(q)) => Mat4::from_quat(*q),
        None => Mat4::IDENTITY,
    };
    let s_mat = match s {
        Some(Scale(x, y, z)) => Mat4::from_scale(Vec3::new(*x, *y, *z)),
        None => Mat4::IDENTITY,
    };
    let m_transform = transform_matrix_to_mat4(&ui.matrix);
    t_mat * r_mat * s_mat * m_transform
}

/// The spec § 2 union of stacking-context-formation triggers that are
/// implementable in `buiy_core` today (D1): (1) positioned + explicit
/// `z_index`, (2) `Isolation::Isolate`, (3) non-identity transform,
/// (4) `Containment.contain ⊇ PAINT/STRICT`, (6) root. Trigger (5)'s
/// render-side formers (opacity/filter/blend) and the will-change SC
/// former are deferred — their components don't exist yet (spec § 7);
/// add an `|| render_side_former` clause here when they land.
///
/// Driven by the `stacking_context` sub-pass (6f).
pub(super) fn forms_stacking_context(
    stacking: Option<&Stacking>,
    position_kind: PositionKind,
    has_transform: bool,
    containment: Option<&Containment>,
    is_root: bool,
) -> bool {
    // Trigger 6 — root.
    if is_root {
        return true;
    }
    // Trigger 3 — non-identity transform (ResolvedTransform present).
    if has_transform {
        return true;
    }
    if let Some(s) = stacking {
        // Trigger 2 — isolation.
        if matches!(s.isolation, Isolation::Isolate) {
            return true;
        }
        // Trigger 1 — positioned (non-static) with an explicit z-index.
        if !matches!(position_kind, PositionKind::Static) && matches!(s.z_index, ZIndex::Layer(_)) {
            return true;
        }
    }
    // Trigger 4 — paint / strict containment.
    if let Some(c) = containment
        && c.contain
            .intersects(ContainFlags::PAINT | ContainFlags::STRICT)
    {
        return true;
    }
    false
}

/// The spec § 2.1 paint tiers, as the primary sort rank. Document order
/// (the `Children`-iteration order of the input `Vec`) breaks ties
/// within a tier via a STABLE sort.
///
/// Returns `(tier, z)`:
/// - tier 0, z = the negative z   → negative `z_index` (positioned), lowest first
/// - tier 1, z = 0                → in-flow non-positioned (document order)
/// - tier 2, z = 0                → in-flow positioned, `z_index: Auto` (document order)
/// - tier 3, z = the positive z   → positive `z_index` (positioned), lowest first
///
/// (Floats — spec tier between non-positioned and auto-positioned — are
/// always empty in Buiy, so they are omitted; the four live tiers keep
/// the spec's relative order.) `z_index` on a `PositionKind::Static`
/// entity is IGNORED (CSS quirk, spec § 3): a static element stays in
/// tier 1 regardless of its `z_index`.
///
/// Driven by the `stacking_context` sub-pass (6f).
pub(super) fn paint_key(stacking: Option<&Stacking>, position_kind: PositionKind) -> (u8, i32) {
    let positioned = !matches!(position_kind, PositionKind::Static);
    let z = match stacking.map(|s| s.z_index) {
        Some(ZIndex::Layer(n)) if positioned => Some(n),
        _ => None, // Auto, or static (z ignored)
    };
    match z {
        Some(n) if n < 0 => (0, n),
        None if !positioned => (1, 0),
        None => (2, 0), // positioned + auto z
        Some(n) /* n >= 0 */ => {
            if n == 0 {
                // Positioned with explicit z-index 0 sits with the
                // positive tier per CSS (0 is "explicit", paints above
                // auto-positioned). Spec § 3: "0 is default for explicit".
                (3, 0)
            } else {
                (3, n)
            }
        }
    }
}

/// Phase 8 — sub-pass 6e of `BuiyLayoutStep::PostTaffyOverrides`.
/// Composes each entity's `UiTransform` + optional `Translate` /
/// `Rotate` / `Scale` longhands into the private `ResolvedTransform`
/// render handoff per spec § 1 (`M = T·R·S·M_transform`).
///
/// Runs AFTER `anchor_resolution` (6d). Unlike 6a–6d, writes NOTHING
/// to `PostTaffyPositionOverrides` — a transform does not move the
/// layout box (spec § 1.2). For identity transforms it inserts no
/// `ResolvedTransform` and removes a stale one (spec § 7). Skips
/// `Display::None` entities.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.1.
#[allow(clippy::type_complexity)]
pub(super) fn transform_composition(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &UiTransform,
            Option<&Translate>,
            Option<&Rotate>,
            Option<&Scale>,
            &Display,
            Option<&ResolvedTransform>,
        ),
        With<Node>,
    >,
) {
    for (e, ui, t, r, s, display, existing) in query.iter() {
        if matches!(display, Display::None) {
            continue;
        }
        let m = compose_transform(ui, t, r, s);
        if m == Mat4::IDENTITY {
            // Identity → no ResolvedTransform; remove a stale one.
            if existing.is_some() {
                commands.entity(e).remove::<ResolvedTransform>();
            }
            continue;
        }
        // Idempotent insert (mirror write_resolved_layout's gate).
        if existing.map(|rt| rt.matrix) != Some(m) {
            commands.entity(e).insert(ResolvedTransform { matrix: m });
        }
    }
}

/// Sub-pass 6f — stacking-context detection + paint-order resolution.
/// Runs after `transform_composition` (6e) so it can read the composed
/// `ResolvedTransform` (trigger 3). Writes the private `StackingContext`
/// render handoff; writes NOTHING to `PostTaffyPositionOverrides`
/// (stacking does not move the layout box). Spec § 2 / § 2.1 / § 4.
///
/// Top-layer entities escape their parent stacking context and attach to
/// their root-ancestor context (the window proxy); the global activation
/// order is tracked in `TopLayerActivation` (spec § 4).
#[allow(clippy::too_many_arguments)]
pub(super) fn stacking_context(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
    nodes: Query<(Entity, Option<&ChildOf>), With<Node>>,
    parent_chain: Query<&ChildOf>,
    children_q: Query<&Children>,
    stacking_q: Query<&Stacking>,
    position_q: Query<&Position>,
    transformed: Query<(), With<crate::components::ResolvedTransform>>,
    containment_q: Query<&Containment>,
    display_q: Query<&Display>,
    existing_sc: Query<Option<&crate::components::StackingContext>>,
    have_sc: Query<Entity, With<crate::components::StackingContext>>,
    mut activation: ResMut<TopLayerActivation>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    use crate::components::StackingContext;

    // --- closures reading the per-entity queries ---
    let display_none = |e: Entity| matches!(display_q.get(e), Ok(Display::None));
    let pos_kind = |e: Entity| {
        position_q
            .get(e)
            .map(|p| p.kind)
            .unwrap_or(PositionKind::Static)
    };
    let top_layer_of = |e: Entity| {
        stacking_q
            .get(e)
            .map(|s| s.top_layer)
            .unwrap_or(TopLayer::None)
    };
    let is_root = |parent: Option<&ChildOf>| {
        parent
            .map(|p| !tree.by_entity.contains_key(&p.parent()))
            .unwrap_or(true)
    };
    let forms = |e: Entity, root: bool| {
        forms_stacking_context(
            stacking_q.get(e).ok(),
            pos_kind(e),
            transformed.get(e).is_ok(),
            containment_q.get(e).ok(),
            root,
        )
    };

    // --- 1. top-layer activation rebuild (D3) ---
    // Single global top layer (D2). Recompute the current top-layer
    // membership from the live `Stacking.top_layer` values; drop deque
    // entries that are no longer top-layer (deactivated / despawned),
    // keeping the existing activation order, then append newly-present
    // entities in tree-iteration order (most-recent at the back).
    let mut fullscreen_count = 0usize;
    let mut current_top: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for (e, _) in nodes.iter() {
        if display_none(e) {
            continue;
        }
        match top_layer_of(e) {
            TopLayer::None => {}
            TopLayer::Fullscreen => {
                fullscreen_count += 1;
                current_top.insert(e);
            }
            _ => {
                current_top.insert(e);
            }
        }
    }
    activation.order.retain(|e| current_top.contains(e));
    for (e, _) in nodes.iter() {
        if current_top.contains(&e) && !activation.order.contains(&e) {
            activation.order.push_back(e);
        }
    }
    if fullscreen_count > 1
        && warned
            .set
            .insert(LayoutWarnOnceKey::MultipleFullscreenTopLayer)
    {
        bevy::log::warn!(
            "Layout: {fullscreen_count} entities request TopLayer::Fullscreen; CSS designates a single fullscreen element. Buiy v1 keeps them all in the top layer ordered by activation — single-winner enforcement (extras falling back to normal stacking) is a follow-up (spec § 4.2)."
        );
    }

    // --- 2. find the root + classify which entities form contexts ---
    // (Single global tree → expect exactly one root in the MinimalPlugins
    // harness; multiple roots are each their own context.)
    let mut forming: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for (e, parent) in nodes.iter() {
        if display_none(e) {
            continue;
        }
        if forms(e, is_root(parent)) {
            forming.insert(e);
        }
    }

    // --- 3. build each forming context's painters_z ---
    // For an SC root R: walk R's subtree in document order; collect every
    // descendant that belongs to R's context. A child C of the current
    // node belongs to R's context UNLESS C itself forms a context — in
    // which case C is an atomic entry (added) but we do NOT descend into
    // C (it owns its own painters_z). Non-forming children are added and
    // descended through. Skip Display::None and (in T9) top-layer entities.
    let painters_of = |sc_root: Entity| -> Vec<Entity> {
        let mut painters: Vec<Entity> = Vec::new();
        let mut stack: Vec<Entity> = Vec::new();
        if let Ok(kids) = children_q.get(sc_root) {
            // push in reverse so we pop in document order
            stack.extend(kids.iter().rev());
        }
        while let Some(node) = stack.pop() {
            if display_none(node) {
                continue;
            }
            // Top-layer entities escape their parent context — they are
            // attached to the root context in step 5 (spec § 4.1).
            if top_layer_of(node) != TopLayer::None {
                continue;
            }
            painters.push(node);
            if !forming.contains(&node)
                && let Ok(kids) = children_q.get(node)
            {
                stack.extend(kids.iter().rev());
            }
        }
        // Stable sort by paint tier; the Vec is already in document order,
        // so equal-tier entries keep document order (spec § 2.1).
        painters.sort_by_cached_key(|&e| paint_key(stacking_q.get(e).ok(), pos_kind(e)));
        painters
    };

    // --- 4. compute the escaped top-layer paint order (spec § 4.2) ---
    // Top-layer entities escape their parent and attach to their
    // root-ancestor context, ordered by tier (Fullscreen bottom < Tooltip <
    // Popover < Modal top) then, within a tier, activation order. The deque
    // is already in activation order, so a STABLE sort by tier preserves
    // activation order inside each tier.
    //
    // Attaching to the entity's own root ancestor (rather than a single
    // global `roots.first()`) keeps one global top layer when there is a
    // single root (D2) while staying correct + deterministic if multiple
    // root trees exist; true per-window scoping is the deferred follow-up.
    // An entity that is itself a root does NOT escape (it has no parent
    // context to escape from) — it forms its own root context, so it is
    // excluded here to avoid a self-reference in its own `painters_z`.
    fn tier_rank(t: TopLayer) -> u8 {
        match t {
            TopLayer::Fullscreen => 0,
            TopLayer::Tooltip => 1,
            TopLayer::Popover => 2,
            TopLayer::Modal => 3,
            TopLayer::None => u8::MAX,
        }
    }
    let root_ancestor = |start: Entity| -> Entity {
        let mut cur = start;
        while let Ok(parent) = parent_chain.get(cur) {
            let p = parent.parent();
            if tree.by_entity.contains_key(&p) {
                cur = p;
            } else {
                break; // parent is not a Buiy node → `cur` is the root
            }
        }
        cur
    };
    let mut top_sorted: Vec<Entity> = activation.order.iter().copied().collect();
    top_sorted.sort_by_cached_key(|&e| tier_rank(top_layer_of(e)));
    let mut escaped_by_root: std::collections::HashMap<Entity, Vec<Entity>> =
        std::collections::HashMap::new();
    for &e in &top_sorted {
        let r = root_ancestor(e);
        if r != e {
            escaped_by_root.entry(r).or_default().push(e);
        }
    }

    // --- 5. write StackingContext on forming entities; remove stale ---
    for &e in &forming {
        let mut painters_z = painters_of(e);
        // Escaped top-layer members of this root context paint after all
        // in-flow painters (spec § 4.1 / D8).
        if let Some(escaped) = escaped_by_root.get(&e) {
            painters_z.extend(escaped.iter().copied());
        }
        let new = StackingContext { painters_z };
        // Idempotent insert (mirror transform_composition's gate): only
        // write when the value differs from the existing one.
        let differs = existing_sc
            .get(e)
            .ok()
            .flatten()
            .map(|sc| sc.painters_z != new.painters_z)
            .unwrap_or(true);
        if differs {
            commands.entity(e).insert(new);
        }
    }
    // Remove StackingContext from entities that no longer form one.
    for e in have_sc.iter() {
        if !forming.contains(&e) || display_none(e) {
            commands.entity(e).remove::<StackingContext>();
        }
    }
}

#[cfg(test)]
mod cq_tests {
    use super::*;
    use crate::layout::types::Orientation;

    /// `evaluate_conditions` is a pure helper — tested without spawning
    /// an App. Phase 5 keeps the helper `pub(super) fn` so this test
    /// can reach it.
    #[test]
    fn evaluate_conditions_min_width_threshold() {
        let conds = [QueryCondition::MinWidth(Length::Px(600.0))];
        // Container is 700 px wide → MinWidth(600) holds.
        assert!(evaluate_conditions(&conds, Vec2::new(700.0, 400.0)));
        // Container is 500 px wide → MinWidth(600) fails.
        assert!(!evaluate_conditions(&conds, Vec2::new(500.0, 400.0)));
    }

    #[test]
    fn evaluate_conditions_aspect_ratio() {
        let landscape_min = [QueryCondition::MinAspectRatio(1.5)];
        assert!(evaluate_conditions(&landscape_min, Vec2::new(800.0, 400.0))); // 2.0
        assert!(!evaluate_conditions(
            &landscape_min,
            Vec2::new(400.0, 800.0)
        )); // 0.5
    }

    #[test]
    fn evaluate_conditions_orientation() {
        let portrait = [QueryCondition::Orientation(Orientation::Portrait)];
        assert!(evaluate_conditions(&portrait, Vec2::new(300.0, 600.0)));
        assert!(!evaluate_conditions(&portrait, Vec2::new(600.0, 300.0)));
    }

    #[test]
    fn evaluate_conditions_zero_height_does_not_panic_on_aspect() {
        // Defensive — h == 0 produces inf or nan. Specify: treat as 0.0
        // aspect (never landscape, never satisfies MinAspectRatio>0).
        let conds = [QueryCondition::MinAspectRatio(1.0)];
        assert!(!evaluate_conditions(&conds, Vec2::new(300.0, 0.0)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{Containment, Position, Stacking};
    use crate::layout::types::{ContainFlags, Isolation, PositionKind, TopLayer, ZIndex};

    fn stk(z: ZIndex, iso: Isolation) -> Stacking {
        Stacking {
            z_index: z,
            isolation: iso,
            top_layer: TopLayer::None,
        }
    }

    #[test]
    fn positioned_with_explicit_z_forms_context() {
        let s = stk(ZIndex::Layer(0), Isolation::Auto);
        assert!(forms_stacking_context(
            Some(&s),
            PositionKind::Relative,
            false,
            None,
            false
        ));
    }

    #[test]
    fn static_with_explicit_z_does_not_form_context() {
        // CSS quirk: z-index on a static element does NOT form a context.
        let s = stk(ZIndex::Layer(5), Isolation::Auto);
        assert!(!forms_stacking_context(
            Some(&s),
            PositionKind::Static,
            false,
            None,
            false
        ));
    }

    #[test]
    fn positioned_with_auto_z_does_not_form_context() {
        let s = stk(ZIndex::Auto, Isolation::Auto);
        assert!(!forms_stacking_context(
            Some(&s),
            PositionKind::Absolute,
            false,
            None,
            false
        ));
    }

    #[test]
    fn isolate_forms_context_regardless_of_position() {
        let s = stk(ZIndex::Auto, Isolation::Isolate);
        assert!(forms_stacking_context(
            Some(&s),
            PositionKind::Static,
            false,
            None,
            false
        ));
    }

    #[test]
    fn non_identity_transform_forms_context() {
        assert!(forms_stacking_context(
            None,
            PositionKind::Static,
            true,
            None,
            false
        ));
    }

    #[test]
    fn paint_containment_forms_context() {
        let c = Containment {
            contain: ContainFlags::PAINT,
            ..Default::default()
        };
        assert!(forms_stacking_context(
            None,
            PositionKind::Static,
            false,
            Some(&c),
            false
        ));
    }

    #[test]
    fn strict_containment_forms_context() {
        let c = Containment {
            contain: ContainFlags::STRICT,
            ..Default::default()
        };
        assert!(forms_stacking_context(
            None,
            PositionKind::Static,
            false,
            Some(&c),
            false
        ));
    }

    #[test]
    fn root_always_forms_context() {
        assert!(forms_stacking_context(
            None,
            PositionKind::Static,
            false,
            None,
            true
        ));
    }

    #[test]
    fn plain_in_flow_element_does_not_form_context() {
        assert!(!forms_stacking_context(
            None,
            PositionKind::Static,
            false,
            None,
            false
        ));
    }

    #[test]
    fn top_layer_activation_default_is_empty() {
        assert!(TopLayerActivation::default().order.is_empty());
    }

    #[test]
    fn paint_key_negative_z_sorts_first() {
        // Negative z-index → tier 0; in-flow non-positioned → tier 1;
        // auto-positioned → tier 3; positive z → tier 4.
        let neg = stk(ZIndex::Layer(-1), Isolation::Auto);
        let pos = stk(ZIndex::Layer(2), Isolation::Auto);
        let auto = stk(ZIndex::Auto, Isolation::Auto);
        // positioned entities
        let kn = paint_key(Some(&neg), PositionKind::Relative);
        let kp = paint_key(Some(&pos), PositionKind::Relative);
        let ka = paint_key(Some(&auto), PositionKind::Relative);
        let kf = paint_key(None, PositionKind::Static); // in-flow non-positioned
        assert!(kn < kf, "negative z paints behind in-flow");
        assert!(kf < ka, "in-flow paints behind auto-positioned");
        assert!(ka < kp, "auto-positioned paints behind positive z");
    }

    #[test]
    fn paint_key_orders_positive_z_ascending() {
        let z1 = stk(ZIndex::Layer(1), Isolation::Auto);
        let z2 = stk(ZIndex::Layer(2), Isolation::Auto);
        assert!(
            paint_key(Some(&z1), PositionKind::Relative)
                < paint_key(Some(&z2), PositionKind::Relative)
        );
    }

    #[test]
    fn paint_key_static_z_index_is_ignored() {
        // z-index on a static element does not lift it out of in-flow tier.
        let z5 = stk(ZIndex::Layer(5), Isolation::Auto);
        let kf = paint_key(None, PositionKind::Static);
        assert_eq!(
            paint_key(Some(&z5), PositionKind::Static).0,
            kf.0,
            "static z-index stays in the in-flow tier"
        );
    }

    #[test]
    fn is_fixed_root_true_for_fixed() {
        let p = Position {
            kind: PositionKind::Fixed,
            ..Default::default()
        };
        assert!(is_fixed_root(&p));
    }

    #[test]
    fn is_fixed_root_false_for_absolute() {
        let p = Position {
            kind: PositionKind::Absolute,
            ..Default::default()
        };
        assert!(!is_fixed_root(&p));
    }

    #[test]
    fn is_fixed_root_false_for_static_relative_sticky() {
        for k in [
            PositionKind::Static,
            PositionKind::Relative,
            PositionKind::Sticky,
        ] {
            let p = Position {
                kind: k,
                ..Default::default()
            };
            assert!(!is_fixed_root(&p), "{k:?} must not re-parent to root");
        }
    }

    // The exclusion path is the load-bearing T3 behavior: a `Fixed` child
    // must be dropped from its in-flow parent's Taffy child list (it is
    // re-parented onto the root in T4). This drives `sync_children_for_entity`
    // directly with a populated `fixed_set` and a real `LayoutTree`, then
    // inspects the resulting Taffy topology — so removing the
    // `.filter(|c| !fixed_set.contains(c))` line makes it FAIL (true RED).
    #[test]
    fn sync_children_excludes_fixed_from_parent_taffy_children() {
        // Real Bevy hierarchy so we get a genuine `Children` component.
        let mut world = World::new();
        let in_flow = world.spawn_empty().id();
        let fixed = world.spawn_empty().id();
        let parent = world.spawn_empty().add_children(&[in_flow, fixed]).id();

        // Real LayoutTree with a Taffy leaf node per entity.
        let mut tree = LayoutTree::default();
        let parent_id = tree.tree.new_leaf(taffy::Style::default()).unwrap();
        let in_flow_id = tree.tree.new_leaf(taffy::Style::default()).unwrap();
        let fixed_id = tree.tree.new_leaf(taffy::Style::default()).unwrap();
        tree.by_entity.insert(parent, parent_id);
        tree.by_entity.insert(in_flow, in_flow_id);
        tree.by_entity.insert(fixed, fixed_id);

        let fixed_set: HashSet<Entity> = [fixed].into_iter().collect();

        let children = world.get::<Children>(parent).unwrap();
        assert_eq!(
            children.iter().collect::<Vec<_>>(),
            vec![in_flow, fixed],
            "precondition: parent has both children in the Bevy hierarchy",
        );

        sync_children_for_entity(parent, Some(children), &fixed_set, &mut tree);

        let taffy_children = tree.tree.children(parent_id).unwrap();
        assert!(
            taffy_children.contains(&in_flow_id),
            "in-flow sibling stays in the parent's Taffy child list",
        );
        assert!(
            !taffy_children.contains(&fixed_id),
            "Fixed child is excluded from the parent's Taffy child list \
             (it re-parents to the root in T4)",
        );
        assert_eq!(
            taffy_children.len(),
            1,
            "only the in-flow sibling remains; the Fixed child is gone",
        );
    }

    #[test]
    fn compose_identity_is_identity() {
        let ui = UiTransform::default();
        let m = compose_transform(&ui, None, None, None);
        assert_eq!(m, Mat4::IDENTITY);
    }

    #[test]
    fn compose_matrix_translate_only() {
        let ui = UiTransform {
            matrix: TransformMatrix::Translate(Length::px(10.0), Length::px(20.0), Length::ZERO),
            ..Default::default()
        };
        let m = compose_transform(&ui, None, None, None);
        assert_eq!(m, Mat4::from_translation(Vec3::new(10.0, 20.0, 0.0)));
    }

    #[test]
    fn compose_matrix_scale_only() {
        let ui = UiTransform {
            matrix: TransformMatrix::Scale(2.0, 3.0, 1.0),
            ..Default::default()
        };
        let m = compose_transform(&ui, None, None, None);
        assert_eq!(m, Mat4::from_scale(Vec3::new(2.0, 3.0, 1.0)));
    }

    #[test]
    fn compose_longhands_with_matrix_order() {
        // Longhand translate (10,0,0), longhand scale (2,2,1), matrix Rotate(z 90°).
        // M = T_translate · R_rotate · S_scale · M_transform
        //   = T(10) · R_longhand_identity? — NOTE: Rotate longhand absent, so R = IDENTITY.
        // With t = Translate(10,0,0), r = None, s = Scale(2,2,1), matrix = Rotate(z, FRAC_PI_2):
        //   M = T(10,0,0) · IDENTITY · S(2,2,1) · Rz(90°)
        let ui = UiTransform {
            matrix: TransformMatrix::Rotate(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ..Default::default()
        };
        let t = Translate(Length::px(10.0), Length::ZERO, Length::ZERO);
        let s = Scale(2.0, 2.0, 1.0);
        let m = compose_transform(&ui, Some(&t), None, Some(&s));
        let expected = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0))
            * Mat4::from_scale(Vec3::new(2.0, 2.0, 1.0))
            * Mat4::from_quat(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
        assert_eq!(m, expected);
    }

    #[test]
    fn compose_matrix_compose_product_order() {
        // Compose([A, B]) = A · B (A outermost, B transforms a child point first).
        let a = TransformMatrix::Translate(Length::px(5.0), Length::ZERO, Length::ZERO);
        let b = TransformMatrix::Scale(2.0, 1.0, 1.0);
        let ui = UiTransform {
            matrix: TransformMatrix::Compose(vec![a, b]),
            ..Default::default()
        };
        let m = compose_transform(&ui, None, None, None);
        let expected = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0))
            * Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
        assert_eq!(m, expected);
    }

    #[test]
    fn anchor_name_registry_lookup_returns_most_recent() {
        let mut r = AnchorNameRegistry::default();
        let e1 = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let e2 = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        r.insert("foo".into(), e1);
        r.insert("foo".into(), e2);
        assert_eq!(r.find_entity_by_name("foo"), Some(e2));
    }

    #[test]
    fn anchor_name_registry_remove_falls_back_to_prior() {
        let mut r = AnchorNameRegistry::default();
        let e1 = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let e2 = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        r.insert("foo".into(), e1);
        r.insert("foo".into(), e2);
        r.remove(e2);
        assert_eq!(r.find_entity_by_name("foo"), Some(e1));
    }

    #[test]
    fn anchor_name_registry_remove_unknown_is_noop() {
        let mut r = AnchorNameRegistry::default();
        r.remove(bevy::prelude::Entity::from_raw_u32(99).unwrap()); // does not panic
    }

    #[test]
    fn anchor_name_registry_epoch_monotonic() {
        let mut r = AnchorNameRegistry::default();
        let e1 = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let e2 = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        r.insert("a".into(), e1);
        r.insert("b".into(), e2);
        assert!(r.entity_epoch(e2) > r.entity_epoch(e1));
    }

    #[test]
    fn post_taffy_position_overrides_default_empty() {
        let o = PostTaffyPositionOverrides::default();
        assert!(o.by_entity.is_empty());
    }

    #[test]
    fn clear_post_taffy_overrides_clears_by_entity() {
        let mut app = App::new();
        app.init_resource::<PostTaffyPositionOverrides>();
        app.add_systems(Update, clear_post_taffy_overrides);
        app.world_mut()
            .resource_mut::<PostTaffyPositionOverrides>()
            .by_entity
            .insert(Entity::from_raw_u32(42).unwrap(), Vec2::new(10.0, 20.0));
        app.update();
        let overrides = app.world().resource::<PostTaffyPositionOverrides>();
        assert!(
            overrides.by_entity.is_empty(),
            "clear system did not empty the map"
        );
    }

    #[test]
    fn layout_anchor_warned_default_empty() {
        let w = LayoutAnchorWarnedThisFrame::default();
        assert!(w.set.is_empty());
    }

    #[test]
    fn warned_once_session_default_empty() {
        let r = LayoutWarnedOnceSession::default();
        assert!(r.set.is_empty());
    }

    #[test]
    fn warned_once_session_dedup() {
        let mut r = LayoutWarnedOnceSession::default();
        let key = LayoutWarnOnceKey::TableUnsupported(Entity::from_raw_u32(1).unwrap());
        let first = r.set.insert(key);
        let second = r.set.insert(key);
        assert!(first, "first insert should report true (newly added)");
        assert!(
            !second,
            "second insert should report false (already present)"
        );
    }

    #[test]
    fn kahn_sort_orders_simple_chain() {
        // a → b → c
        let mut edges = std::collections::HashMap::new();
        let a = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let b = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        let c = bevy::prelude::Entity::from_raw_u32(3).unwrap();
        edges.insert(a, Some(b));
        edges.insert(b, Some(c));
        edges.insert(c, None);
        let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
        // anchor targets come BEFORE anchored entities: c, b, a
        let ci = order.iter().position(|&e| e == c).unwrap();
        let bi = order.iter().position(|&e| e == b).unwrap();
        let ai = order.iter().position(|&e| e == a).unwrap();
        assert!(ci < bi);
        assert!(bi < ai);
        assert!(dropped.is_empty());
    }

    #[test]
    fn kahn_sort_breaks_2_node_cycle_at_higher_epoch() {
        // a → b, b → a; epoch(b) > epoch(a)
        let mut edges = std::collections::HashMap::new();
        let a = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let b = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        edges.insert(a, Some(b));
        edges.insert(b, Some(a));
        let epochs = move |e: Entity| if e == b { 10 } else { 5 };
        let (order, dropped) = kahn_anchor_sort(&edges, &epochs);
        assert_eq!(dropped.len(), 1);
        assert!(dropped.contains(&b)); // b's edge (b → a) was dropped
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn kahn_sort_breaks_3_node_cycle_at_highest_epoch() {
        // a → b → c → a (cycle); epoch(c) > epoch(b) > epoch(a)
        let mut edges = std::collections::HashMap::new();
        let a = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let b = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        let c = bevy::prelude::Entity::from_raw_u32(3).unwrap();
        edges.insert(a, Some(b));
        edges.insert(b, Some(c));
        edges.insert(c, Some(a));
        let epochs = move |e: Entity| match e {
            x if x == c => 30,
            x if x == b => 20,
            _ => 10,
        };
        let (order, dropped) = kahn_anchor_sort(&edges, &epochs);
        assert_eq!(dropped.len(), 1);
        assert!(dropped.contains(&c));
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn kahn_sort_handles_two_independent_cycles() {
        // (a → b → a) + (c → d → c); each cycle drops its higher-epoch node
        let mut edges = std::collections::HashMap::new();
        let a = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let b = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        let c = bevy::prelude::Entity::from_raw_u32(3).unwrap();
        let d = bevy::prelude::Entity::from_raw_u32(4).unwrap();
        edges.insert(a, Some(b));
        edges.insert(b, Some(a));
        edges.insert(c, Some(d));
        edges.insert(d, Some(c));
        let epochs = move |e: Entity| match e {
            x if x == b => 20,
            x if x == d => 40,
            _ => 10,
        };
        let (order, dropped) = kahn_anchor_sort(&edges, &epochs);
        assert_eq!(dropped.len(), 2);
        assert!(dropped.contains(&b));
        assert!(dropped.contains(&d));
        assert_eq!(order.len(), 4);
    }

    #[test]
    fn kahn_sort_empty_input_is_empty_output() {
        let edges = std::collections::HashMap::new();
        let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
        assert!(order.is_empty());
        assert!(dropped.is_empty());
    }

    #[test]
    fn kahn_sort_only_targets_no_anchored() {
        // a (no outgoing), b (no outgoing) — both should appear, no edges
        let mut edges = std::collections::HashMap::new();
        let a = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let b = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        edges.insert(a, None);
        edges.insert(b, None);
        let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
        assert_eq!(order.len(), 2);
        assert!(dropped.is_empty());
    }

    #[test]
    fn kahn_sort_external_target_no_anchor_doesnt_loop() {
        // a → b, but b is NOT in edges (it's a plain Node target).
        // D10 pre-pass should add b as `b → None`, Kahn terminates cleanly.
        let mut edges = std::collections::HashMap::new();
        let a = bevy::prelude::Entity::from_raw_u32(1).unwrap();
        let b = bevy::prelude::Entity::from_raw_u32(2).unwrap();
        edges.insert(a, Some(b));
        // NOT inserting b.
        let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
        assert_eq!(order.len(), 2);
        let ai = order.iter().position(|&e| e == a).unwrap();
        let bi = order.iter().position(|&e| e == b).unwrap();
        assert!(bi < ai); // b is the target — comes first
        assert!(dropped.is_empty());
    }

    // -----------------------------------------------------------------
    // Phase 7 Task 5 — `compute_sticky_displacement` pure-helper tests.
    //
    // The full `sticky_offset` system is covered by integration tests
    // in Task 10; here we exercise every branch of the per-axis sticky
    // algorithm in isolation. Assertion values from plan v2 Task 5
    // Step 6 (post-test-reviewer corrections).
    // -----------------------------------------------------------------

    #[test]
    fn sticky_no_inset_no_displacement() {
        let d = compute_sticky_displacement(
            Vec2::new(10.0, 20.0),    // natural in S
            Vec2::new(100.0, 50.0),   // size
            Vec2::new(0.0, 0.0),      // parent in S
            Vec2::new(300.0, 1000.0), // parent size
            Vec2::new(300.0, 500.0),  // S size
            Vec2::ZERO,               // scroll offset
            None,
            None,
            None,
            None, // no insets
        );
        assert_eq!(d, Vec2::ZERO);
    }

    #[test]
    fn sticky_top_pins_when_scrolled_past() {
        let d = compute_sticky_displacement(
            Vec2::new(0.0, 50.0), // natural at y=50
            Vec2::new(100.0, 30.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(300.0, 1000.0),
            Vec2::new(300.0, 500.0),
            Vec2::new(0.0, 100.0), // scrolled down by 100
            Some(10.0),
            None,
            None,
            None, // top: 10px
        );
        // visible_top = 100, threshold = 110. natural_y = 50.
        // desired_y = max(50, 110) = 110, clamped by parent_bottom - size
        // = 1000 - 30 = 970, by parent_in_s.y = 0.
        // displacement.y = 110 - 50 = 60.
        assert_eq!(d, Vec2::new(0.0, 60.0));
    }

    #[test]
    fn sticky_top_does_not_pull_up() {
        let d = compute_sticky_displacement(
            Vec2::new(0.0, 50.0), // natural at y=50
            Vec2::new(100.0, 30.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(300.0, 1000.0),
            Vec2::new(300.0, 500.0),
            Vec2::ZERO, // not scrolled
            Some(10.0),
            None,
            None,
            None,
        );
        // visible_top = 0, threshold = 10. natural_y = 50.
        // desired_y = max(50, 10) = 50, clamped by parent. = 50.
        // displacement = 50 - 50 = 0.
        assert_eq!(d, Vec2::ZERO);
    }

    #[test]
    fn sticky_top_clamped_by_parent_bottom() {
        let d = compute_sticky_displacement(
            Vec2::new(0.0, 10.0),
            Vec2::new(100.0, 30.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(300.0, 50.0), // small parent — height 50
            Vec2::new(300.0, 1000.0),
            Vec2::new(0.0, 100.0),
            Some(5.0),
            None,
            None,
            None,
        );
        // visible_top = 100, threshold = 105. natural_y = 10.
        // desired_y = max(10, 105) = 105, clamped by parent_bottom -
        // size = 50 - 30 = 20, by parent_in_s.y = 0 → 20.
        // displacement = 20 - 10 = 10.
        assert_eq!(d, Vec2::new(0.0, 10.0));
    }

    // ---- v2 — bottom-pin branch coverage (test-reviewer BLOCKER B1) ----

    #[test]
    fn sticky_bottom_pins_when_scroll_near_bottom() {
        // visible_bottom = scroll_offset.y + S.y = 150 + 500 = 650.
        // threshold = 650 - 10 = 640. e_h = 30.
        // (threshold - e_h) = 610. min(610, natural=700) = 610.
        // .max(parent_top=0) = 610. .min(parent_bottom - e_h = 970)
        // = 610. displacement = 610 - 700 = -90.
        let d = compute_sticky_displacement(
            Vec2::new(0.0, 700.0), // natural y=700
            Vec2::new(100.0, 30.0),
            Vec2::new(0.0, 0.0),      // parent in S
            Vec2::new(300.0, 1000.0), // parent height
            Vec2::new(300.0, 500.0),  // S size
            Vec2::new(0.0, 150.0),    // scroll
            None,
            Some(10.0),
            None,
            None, // bottom: 10px
        );
        assert_eq!(d, Vec2::new(0.0, -90.0));
    }

    #[test]
    fn sticky_bottom_does_not_push_down_before_scroll() {
        // visible_bottom = 0 + 500 = 500, threshold = 490,
        // threshold - e_h = 460. min(460, natural=300) = 300.
        // displacement = 0.
        let d = compute_sticky_displacement(
            Vec2::new(0.0, 300.0),
            Vec2::new(100.0, 30.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(300.0, 1000.0),
            Vec2::new(300.0, 500.0),
            Vec2::ZERO,
            None,
            Some(10.0),
            None,
            None,
        );
        assert_eq!(d, Vec2::ZERO);
    }

    #[test]
    fn sticky_bottom_clamped_by_parent_top() {
        // parent_in_s.y = 100, parent_height = 200. natural_y = 280,
        // e_h = 30. visible_bottom = 0 + 100 = 100, threshold = 90,
        // threshold - e_h = 60. .min(natural=280) = 60.
        // .max(parent_top=100) = 100. .min(parent_bottom - e_h = 270)
        // = 100. displacement = 100 - 280 = -180.
        let d = compute_sticky_displacement(
            Vec2::new(0.0, 280.0),
            Vec2::new(100.0, 30.0),
            Vec2::new(0.0, 100.0), // parent has nonzero top
            Vec2::new(300.0, 200.0),
            Vec2::new(300.0, 100.0), // tiny scroll container
            Vec2::ZERO,
            None,
            Some(10.0),
            None,
            None,
        );
        assert_eq!(d, Vec2::new(0.0, -180.0));
    }

    // ---- v2 — both-top-and-bottom-active behavior (test-reviewer BLOCKER B2) ----

    #[test]
    fn sticky_both_top_and_bottom_active_top_wins() {
        // v1 deviation: when both insets are set, top wins. This test
        // documents the behavior — a future correct dual-clamp impl
        // will fail this test and that's the signal to flip it.
        let d = compute_sticky_displacement(
            Vec2::new(0.0, 50.0),
            Vec2::new(100.0, 30.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(300.0, 1000.0),
            Vec2::new(300.0, 500.0),
            Vec2::new(0.0, 100.0),
            Some(10.0),
            Some(10.0),
            None,
            None, // both insets set
        );
        // Top-pin branch fires: visible_top=100, threshold=110,
        // max(50, 110)=110. Clamped by parent_bottom - e_h = 970 → 110.
        // Displacement = 60. Bottom inset is ignored.
        assert_eq!(d, Vec2::new(0.0, 60.0));
    }

    // -----------------------------------------------------------------
    // Phase 7 Task 6 — `table_layout` system tests.
    // -----------------------------------------------------------------

    #[test]
    fn table_layout_warns_once_per_entity() {
        let mut app = App::new();
        app.init_resource::<LayoutWarnedOnceSession>();
        app.add_systems(Update, table_layout);
        let e = app.world_mut().spawn((Node, Display::Table)).id();
        app.update();
        let warned = app.world().resource::<LayoutWarnedOnceSession>();
        assert!(warned.set.contains(&LayoutWarnOnceKey::TableUnsupported(e)));

        app.update();
        let warned = app.world().resource::<LayoutWarnedOnceSession>();
        assert_eq!(
            warned
                .set
                .iter()
                .filter(|k| matches!(k, LayoutWarnOnceKey::TableUnsupported(_)))
                .count(),
            1,
        );
    }

    // -----------------------------------------------------------------
    // Phase 7 Task 7 — `multicol_pack` system tests.
    // -----------------------------------------------------------------

    #[test]
    fn multicol_pack_warns_once_per_session() {
        let mut app = App::new();
        app.init_resource::<LayoutWarnedOnceSession>();
        app.add_systems(Update, multicol_pack);
        let _e1 = app.world_mut().spawn((Node, MultiColumn::default())).id();
        let _e2 = app.world_mut().spawn((Node, MultiColumn::default())).id();
        app.update();
        let warned = app.world().resource::<LayoutWarnedOnceSession>();
        assert_eq!(
            warned
                .set
                .iter()
                .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolUnsupported))
                .count(),
            1,
        );

        app.update();
        let warned = app.world().resource::<LayoutWarnedOnceSession>();
        assert_eq!(
            warned
                .set
                .iter()
                .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolUnsupported))
                .count(),
            1,
        );
    }

    // --- content_visibility_skip (Phase 11, spec § 5.2, D2/D7) ---

    fn cvis(cv: ContentVisibility) -> Containment {
        Containment {
            content_visibility: cv,
            ..Default::default()
        }
    }

    #[test]
    fn skip_none_when_visible() {
        let c = cvis(ContentVisibility::Visible);
        // off-screen + hint present, but Visible never skips.
        let hint = ContainIntrinsicSize {
            width: Some(100.0),
            height: Some(50.0),
        };
        assert_eq!(
            content_visibility_skip(&c, Some(&hint), /*off_screen=*/ true),
            SkipKind::None
        );
    }

    #[test]
    fn skip_hidden_always_prunes() {
        let c = cvis(ContentVisibility::Hidden);
        // Hidden prunes descendants regardless of geometry / hint.
        assert_eq!(
            content_visibility_skip(&c, None, /*off_screen=*/ false),
            SkipKind::HiddenPrune
        );
        assert_eq!(
            content_visibility_skip(&c, None, /*off_screen=*/ true),
            SkipKind::HiddenPrune
        );
    }

    #[test]
    fn skip_auto_on_screen_is_none() {
        let c = cvis(ContentVisibility::Auto);
        let hint = ContainIntrinsicSize {
            width: Some(100.0),
            height: Some(50.0),
        };
        assert_eq!(
            content_visibility_skip(&c, Some(&hint), /*off_screen=*/ false),
            SkipKind::None
        );
    }

    #[test]
    fn skip_auto_off_screen_without_hint_is_none() {
        // D2: Auto + off-screen but NO intrinsic-size hint → lay out normally.
        let c = cvis(ContentVisibility::Auto);
        assert_eq!(
            content_visibility_skip(&c, None, /*off_screen=*/ true),
            SkipKind::None
        );
        // a present-but-empty hint (both None) also does not qualify.
        let empty = ContainIntrinsicSize::default();
        assert_eq!(
            content_visibility_skip(&c, Some(&empty), /*off_screen=*/ true),
            SkipKind::None
        );
    }

    #[test]
    fn skip_auto_off_screen_with_hint_is_sentinel() {
        let c = cvis(ContentVisibility::Auto);
        let hint = ContainIntrinsicSize {
            width: Some(120.0),
            height: Some(40.0),
        };
        assert_eq!(
            content_visibility_skip(&c, Some(&hint), /*off_screen=*/ true),
            SkipKind::AutoSentinel {
                intrinsic: ContainIntrinsicSize {
                    width: Some(120.0),
                    height: Some(40.0)
                }
            }
        );
    }

    // --- viewport_rect + is_off_screen (Phase 11, spec § 5.2, D3) ---

    #[test]
    fn viewport_rect_expands_by_margin() {
        let r = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
        assert_eq!(r.min, Vec2::new(-200.0, -200.0));
        assert_eq!(r.max, Vec2::new(1000.0, 800.0));
    }

    #[test]
    fn on_screen_box_is_not_off_screen() {
        let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
        let rl = ResolvedLayout {
            position: Vec2::new(100.0, 100.0),
            size: Vec2::new(50.0, 50.0),
        };
        assert!(!is_off_screen(Some(&rl), vp));
    }

    #[test]
    fn box_beyond_expanded_viewport_is_off_screen() {
        let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
        // x starts at 1100 > max.x (1000) → fully outside the expanded rect.
        let rl = ResolvedLayout {
            position: Vec2::new(1100.0, 100.0),
            size: Vec2::new(50.0, 50.0),
        };
        assert!(is_off_screen(Some(&rl), vp));
    }

    #[test]
    fn box_within_margin_is_still_on_screen_hysteresis() {
        let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
        // x = 900: past the 800 viewport edge but inside the +200 margin → on-screen.
        let rl = ResolvedLayout {
            position: Vec2::new(900.0, 100.0),
            size: Vec2::new(50.0, 50.0),
        };
        assert!(
            !is_off_screen(Some(&rl), vp),
            "within the hysteresis margin counts as on-screen"
        );
    }

    #[test]
    fn no_last_frame_layout_is_on_screen() {
        let vp = viewport_rect(Vec2::new(800.0, 600.0), 200.0);
        assert!(
            !is_off_screen(None, vp),
            "never skip without last-frame geometry (D3)"
        );
    }
}

#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::layout::components::Anchor;
    use crate::layout::types::AnchorName;

    fn app_with_observers() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AnchorNameRegistry>();
        app.init_resource::<LayoutAnchorWarnedThisFrame>();
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Insert, Anchor>,
             q: Query<&Anchor>,
             mut reg: ResMut<AnchorNameRegistry>| {
                super::handle_anchor_insert(trigger.event().entity, &q, &mut reg);
            },
        );
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Replace, Anchor>,
             mut reg: ResMut<AnchorNameRegistry>| {
                reg.remove(trigger.event().entity);
            },
        );
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Remove, Anchor>,
             mut reg: ResMut<AnchorNameRegistry>| {
                reg.remove(trigger.event().entity);
            },
        );
        app
    }

    #[test]
    fn observer_insert_registers_named_anchor() {
        let mut app = app_with_observers();
        let e = app
            .world_mut()
            .spawn(Anchor {
                anchor_name: Some(AnchorName::Named("foo".into())),
                ..default()
            })
            .id();
        // Observers fire synchronously on `spawn`, so the registry
        // reflects the new entry immediately.
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("foo"), Some(e));
    }

    #[test]
    fn observer_remove_cleans_registry() {
        let mut app = app_with_observers();
        let e = app
            .world_mut()
            .spawn(Anchor {
                anchor_name: Some(AnchorName::Named("foo".into())),
                ..default()
            })
            .id();
        app.world_mut().entity_mut(e).remove::<Anchor>();
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("foo"), None);
    }

    #[test]
    fn observer_replace_removes_then_reinserts() {
        let mut app = app_with_observers();
        let e = app
            .world_mut()
            .spawn(Anchor {
                anchor_name: Some(AnchorName::Named("old".into())),
                ..default()
            })
            .id();
        app.world_mut().entity_mut(e).insert(Anchor {
            anchor_name: Some(AnchorName::Named("new".into())),
            ..default()
        });
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("old"), None);
        assert_eq!(reg.find_entity_by_name("new"), Some(e));
    }

    #[test]
    fn observer_anchor_without_name_is_tracked_by_epoch_only() {
        let mut app = app_with_observers();
        let e = app.world_mut().spawn(Anchor::default()).id();
        let reg = app.world().resource::<AnchorNameRegistry>();
        // No named entry — but the entity is in entity_epochs (for
        // cycle-resolution lookups that don't go through `by_name`).
        assert!(reg.entity_epoch(e) > 0);
        // The empty-string bucket should NOT contain the entity.
        // (regression test for the v1 plan's empty-string side-channel).
        assert_eq!(reg.find_entity_by_name(""), None);
    }

    // DuplicateName detection moved to anchor_resolution (D11) — the
    // observer no longer touches LayoutAnchorWarnedThisFrame. Test
    // coverage for duplicate-name warns lives in the integration tests
    // (tests/layout_anchor_positioning.rs).
}
