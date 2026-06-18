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
use super::translate::{ContainerSnapshot, StyleView, resolve_cq_unit_px, style_to_taffy};
use super::tree::LayoutTree;
use super::types::{
    AnchorErrorKind, AnchorName, AnchorRef, AxisDimension, BreakAfter, BreakBefore, ColumnCount,
    ColumnFill, ContainFlags, ContainerType, ContentVisibility, GridAreas, Inset, Isolation,
    LayoutWarnOnceKey, Length, PositionKind, QueryCondition, Sizing, TopLayer, TransformMatrix,
    TryCondition, WritingModeKind, ZIndex,
};
use crate::components::{Node, ResolvedLayout, ResolvedTransform};
use crate::render::components::{Filter, MixBlendMode, Opacity};
use crate::render::effect::forms_render_stacking_context;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use taffy::NodeId as TaffyNodeId;
use taffy::prelude::TraversePartialTree as _;

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

/// Dirty set for the multi-level container-query geometric cascade
/// (Phase 14). Populated by step 8 (`cq_descendant_invalidate`) with the
/// descendants of every query container whose `ResolvedLayout` changed
/// this frame; drained by step 9 (`cq_descendant_rerun`), which
/// re-translates exactly these entities so their `Length::Cq*` re-resolves
/// against the new ancestor size in the same frame. Cleared at the top of
/// step 8 each frame, so it never accumulates across frames (D1/D4).
///
/// Private cross-pass hand-off (a resource, not an author-set component):
/// follow-ups.md "Descendant invalidation on ancestor-resolved-size
/// changes" option (b). Empty in steady state.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3, § 1.5.
#[derive(Resource, Default, Debug)]
pub struct ContainerSizeDirty(pub std::collections::HashSet<Entity>);

/// Re-run request flag for the Phase-14 descendant invalidation, mirroring
/// `CqReRunRequested` (Phase 5 step 5). Set `true` by step 8 when
/// `ContainerSizeDirty` is non-empty; observed + cleared at the top of
/// step 9 (`cq_descendant_rerun`). Capped at one re-run per frame (D4):
/// deeper cascade levels settle on subsequent frames.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3.
#[derive(Resource, Default, Debug)]
pub struct CqDescendantReRunRequested(pub bool);

/// Per-frame signal that the step-5 activation-flip re-run (`cq_flip_rerun`)
/// actually re-ran Taffy this frame. Set `true` at the top of
/// `cq_flip_rerun`'s body (after it observes `CqReRunRequested`), set
/// `false` when `cq_flip_rerun` is a no-op; read by step 8
/// (`cq_descendant_invalidate`) to enforce the D4 "one re-run per frame"
/// cost ceiling.
///
/// **Why the descendant pass defers on a flip frame (D4):** an
/// activation-flip frame already spent its single re-layout in
/// `cq_flip_rerun` (the spec § 1.3 "2× Taffy on activation-flip frames"
/// ceiling). Seeding a *second* re-run from step 8 the same frame would
/// push Taffy to 3×, breaking that ceiling. The spec § 1.5 transitive
/// cascade is explicitly multi-frame ("frame N applies A's activation,
/// frame N+1 applies B's"), so on a flip frame the geometric cascade is
/// deferred: step 8 skips seeding, and the next frame's `Changed`
/// cascade (the flip re-run's recompute surfaces the new size via step 7)
/// re-seeds it. The realistic Phase-14 case (a query container resizing
/// with no rule on itself) never sets `CqReRunRequested`, so this guard
/// never suppresses the same-frame settle that the phase delivers.
#[derive(Resource, Default, Debug)]
pub struct CqFlipReRanThisFrame(pub bool);

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

/// Hysteresis margin (logical px) for the `content-visibility: auto`
/// off-screen test (spec § 5.2, D3). The viewport is expanded by this
/// margin on every side; an entity is "off-screen" only once its
/// last-frame box is fully outside the expanded rect, and snaps back
/// as soon as it re-enters — so an entity oscillating by less than
/// this margin around the edge does not thrash skip-state. Also serves
/// as the pre-roll distance (slightly-off-screen content is kept laid
/// out). Default 200px.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ContentVisibilityMargin(pub f32);

impl Default for ContentVisibilityMargin {
    fn default() -> Self {
        ContentVisibilityMargin(200.0)
    }
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
/// "this edge is not set." `Cq*` insets resolve against `cq_frame` (the
/// sticky entity's own nearest CQ ancestor) via the shared
/// `resolve_cq_unit_px`. Semantically-invalid inputs (`Fr` — grid-only,
/// `AnchorSize` — sticky has no anchor box) return `Some(0.0)` and
/// record one `warn!` per (entity, session) via `warned`.
///
/// v2 — `Length` has only `Px / Percent / Fr / Cq*`. `Vh/Vw/Vmin/Vmax/
/// Em/Rem` are not variants and never will be without a Phase 10
/// extension; the match below is closed (no wildcard arm) so the
/// compiler errors when Phase 10 adds new variants — forcing a
/// deliberate decision per future variant.
///
/// Phase 7 — sub-pass 6a (`sticky_offset`).
#[allow(clippy::too_many_arguments)]
fn resolve_sticky_inset(
    s: &Sizing,
    scroll_container_axis_size: f32,
    cq_frame: Option<ContainerSnapshot>,
    viewport: Vec2,
    wmr: &WritingModeResolved,
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
        // All Cq* variants resolve against the sticky entity's OWN
        // nearest container-query ancestor (the `cq_frame` snapshot),
        // via the shared `resolve_cq_unit_px` resolver — the same path
        // sizing, tracks, and edges use. Cqi/Cqb resolve on the
        // writing-mode inline/block axes; no-CQ-ancestor (cq_frame ==
        // None) falls back to viewport inside `resolve_cq_unit_px`,
        // identical to every other Cq* site.
        Length::Cqw(_)
        | Length::Cqh(_)
        | Length::Cqi(_)
        | Length::Cqb(_)
        | Length::Cqmin(_)
        | Length::Cqmax(_) => resolve_cq_unit_px(*length, cq_frame, viewport, wmr),
        // `anchor-size()` reads an anchor box; sticky has none. Resolve
        // to 0.0, warn once per (entity, session) — same channel + dedup
        // shape as the Fr sticky-deferred arm above.
        Length::AnchorSize(_) => {
            if warned
                .set
                .insert(LayoutWarnOnceKey::StickyAnchorSizeUnsupported(entity))
            {
                warn!(
                    "Sticky entity {:?} uses anchor-size() inset; sticky has no anchor box, so it resolves to 0.0.",
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
/// container's content-box axis size (D11). `Length::Cq*` insets
/// resolve against the sticky entity's own nearest container-query
/// ancestor (size read CURRENT-frame from Taffy) via the shared
/// `resolve_cq_unit_px`; the no-CQ-ancestor case falls back to the
/// viewport like every other Cq* site. `Length::Fr` (grid-only) and
/// `Length::AnchorSize` (sticky has no anchor box) warn-once-per-session
/// and resolve to 0.0.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.3.
#[allow(clippy::too_many_arguments)]
pub(super) fn sticky_offset(
    tree: NonSend<LayoutTree>,
    sticky_query: Query<(Entity, &Position, &Display), With<Node>>,
    overflow_q: Query<&Overflow>,
    scroll_offset_q: Query<&ScrollOffset>,
    parent_chain: Query<&ChildOf>,
    container_q: Query<(Entity, &Container)>,
    wmr_q: Query<&WritingModeResolved>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut overrides: ResMut<PostTaffyPositionOverrides>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    // Per-call memo for `world_position` — entities deeper in the
    // sticky set share `ChildOf` chain prefixes, so memoizing avoids
    // redundant walks.
    let mut memo: HashMap<(Entity, Entity), Vec2> = HashMap::new();

    // CQ-ancestor frame source for `Length::Cq*` insets. Read
    // CURRENT-frame from Taffy (NOT last-frame `&ResolvedLayout`):
    // `sticky_offset` runs in `PostTaffyOverrides` (AFTER `TaffyCompute`,
    // BEFORE `WriteResolvedLayout`), so a container's `ResolvedLayout` is
    // stale here but its Taffy size is fresh — consistent with the
    // self/parent/scroll sizes read from Taffy below. Entities Taffy
    // hasn't placed yet are skipped (`.ok()`), same skip-this-frame
    // semantics as the self/parent/scroll reads.
    let container_index: HashMap<Entity, ContainerSnapshot> = container_q
        .iter()
        .filter(|(_, c)| c.container_type != ContainerType::Normal)
        .filter_map(|(entity, c)| {
            let node = tree.by_entity.get(&entity)?;
            let layout = tree.tree.layout(*node).ok()?;
            Some((
                entity,
                ContainerSnapshot {
                    container_type: c.container_type,
                    size: Vec2::new(layout.size.width, layout.size.height),
                },
            ))
        })
        .collect();

    // Viewport fallback for the no-CQ-ancestor case (mirrors
    // `sync_styles`). `resolve_cq_unit_px` uses this when an entity has
    // no queried ancestor.
    let viewport_size = primary_window
        .single()
        .ok()
        .map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(Vec2::ZERO);

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

        // CQ frame + writing mode for any `Length::Cq*` inset — the
        // sticky entity's OWN nearest CQ ancestor (walks `ChildOf` from
        // the sticky entity, distinct from anchor's per-try anchor-box
        // frame). Resolved once per entity (not per edge) to keep the
        // `resolve_sticky_inset` signature L6-clean.
        let cq_frame = nearest_container_with_size(e, &container_index, &parent_chain);
        let wmr = wmr_q.get(e).copied().unwrap_or_default();

        // D3 / D11 — per-axis inset resolution. The caller passes the
        // correct scroll-container axis size (height for top/bottom,
        // width for left/right) for `Percent`; `resolve_sticky_inset`
        // does not need an axis-tag parameter. `Cq*` resolves against
        // the CQ frame + writing mode (not the scroll axis).
        let top = resolve_sticky_inset(
            &pos.inset.top,
            s_size.y,
            cq_frame,
            viewport_size,
            &wmr,
            e,
            &mut warned,
        );
        let bottom = resolve_sticky_inset(
            &pos.inset.bottom,
            s_size.y,
            cq_frame,
            viewport_size,
            &wmr,
            e,
            &mut warned,
        );
        let left = resolve_sticky_inset(
            &pos.inset.left,
            s_size.x,
            cq_frame,
            viewport_size,
            &wmr,
            e,
            &mut warned,
        );
        let right = resolve_sticky_inset(
            &pos.inset.right,
            s_size.x,
            cq_frame,
            viewport_size,
            &wmr,
            e,
            &mut warned,
        );

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

/// The role an entity plays in a CSS table, derived from its
/// `Display` (spec § 1, display-and-positioning.md). The four
/// structural roles (`Table` / `RowGroup` / `Row` / `Cell`) are laid
/// out by sub-pass 6b; `Caption` / `Column` / `ColumnGroup` are
/// classified but deferred-with-warn in v1 (plan D4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TablePart {
    Table,
    /// `table-row-group` / `table-header-group` / `table-footer-group`
    /// — all three collapse to `RowGroup`; header/footer reorder is a
    /// v1.x follow-up (D5).
    RowGroup,
    Row,
    Cell,
    Caption,
    Column,
    ColumnGroup,
}

/// Classify a `Display` into its `TablePart` role, or `None` if the
/// entity is not a table-family member.
pub(super) fn table_part(display: &Display) -> Option<TablePart> {
    match display {
        Display::Table => Some(TablePart::Table),
        Display::TableRowGroup | Display::TableHeaderGroup | Display::TableFooterGroup => {
            Some(TablePart::RowGroup)
        }
        Display::TableRow => Some(TablePart::Row),
        Display::TableCell => Some(TablePart::Cell),
        Display::TableCaption => Some(TablePart::Caption),
        Display::TableColumn => Some(TablePart::Column),
        Display::TableColumnGroup => Some(TablePart::ColumnGroup),
        _ => None,
    }
}

/// Resolve per-column widths for a table from each row's cell widths,
/// via a throwaway synthetic Taffy flex tree (spec § 1.2 step 2). The
/// synthetic tree has one flex-row container per table-row, each
/// holding one fixed-width leaf per cell, all rows under a synthetic
/// flex-column root; one `compute_layout` resolves the cells, and
/// column `c`'s width is the max resolved width of cell `c` across
/// rows. Column count = the widest row's cell count; rows shorter than
/// that contribute nothing to the trailing columns (ragged-row case,
/// plan D8). The synthetic `TaffyTree` is local and dropped on return
/// — the shared `LayoutTree` is never touched.
///
/// Pure (no Bevy queries / no shared state). Unit-tested in `mod tests`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
pub(super) fn resolve_column_widths(rows: &[Vec<f32>]) -> Vec<f32> {
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return Vec::new();
    }

    let mut tree: taffy::TaffyTree<()> = taffy::TaffyTree::new();
    let mut row_nodes: Vec<taffy::NodeId> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cell_nodes: Vec<taffy::NodeId> = Vec::with_capacity(row.len());
        for &w in row {
            // Fixed-size leaf: the cell's Taffy-block-computed width.
            let leaf = tree
                .new_leaf(taffy::Style {
                    size: taffy::Size {
                        width: taffy::Dimension::length(w),
                        height: taffy::Dimension::length(0.0),
                    },
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    ..Default::default()
                })
                .expect("synthetic table column leaf");
            cell_nodes.push(leaf);
        }
        let row_node = tree
            .new_with_children(
                taffy::Style {
                    display: taffy::Display::Flex,
                    flex_direction: taffy::FlexDirection::Row,
                    ..Default::default()
                },
                &cell_nodes,
            )
            .expect("synthetic table row");
        row_nodes.push(row_node);
    }
    let root = tree
        .new_with_children(
            taffy::Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                ..Default::default()
            },
            &row_nodes,
        )
        .expect("synthetic table root");
    // MaxContent so each column sizes to its widest cell, no shrink.
    tree.compute_layout(
        root,
        taffy::Size {
            width: taffy::AvailableSpace::MaxContent,
            height: taffy::AvailableSpace::MaxContent,
        },
    )
    .expect("synthetic table layout");

    let mut widths = vec![0.0f32; col_count];
    for (ri, &row_node) in row_nodes.iter().enumerate() {
        for (ci, width) in widths.iter_mut().enumerate().take(rows[ri].len()) {
            if let Ok(child) = tree.child_at_index(row_node, ci)
                && let Ok(layout) = tree.layout(child)
            {
                *width = width.max(layout.size.width);
            }
        }
    }
    widths
}

/// Resolve the CSS Multicol L1 § 7.3 *used* `(column_count, column_width)`
/// pair from the declared `column-count` / `column-width` / `column-gap`
/// and the container's available (content-box inline) width.
///
/// Four cases (plan D3):
/// - **neither** (`Auto` + `None`): 1 column, used width = `available_width`.
/// - **count only**: used count = `max(1, n)`; used width =
///   `(available - (count-1)*gap) / count`.
/// - **width only**: used count =
///   `max(1, floor((available + gap) / (width + gap)))`; used width =
///   `(available - (count-1)*gap) / count` (columns expand to fill).
/// - **both**: `column-count` is a *maximum* — used count =
///   `max(1, min(n, width_derived_count))`; used width as above.
///
/// Pure (no Bevy queries / no Taffy reads). Unit-tested in `mod tests`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
pub(super) fn resolve_column_count(
    column_count: ColumnCount,
    column_width: Option<f32>,
    gap: f32,
    available_width: f32,
) -> (usize, f32) {
    let avail = available_width.max(0.0);
    let gap = gap.max(0.0);

    // Count derivable from a usable (> 0) width: how many `width + gap`
    // slabs fit, with one fewer gap than columns (the `+ gap` numerator
    // term cancels the trailing gap). 0 → fall through to clamp.
    let width_derived = |w: f32| -> usize {
        if w <= 0.0 {
            return 0;
        }
        (((avail + gap) / (w + gap)).floor() as i64).max(0) as usize
    };

    let count = match (column_count, column_width) {
        (ColumnCount::Auto, None) => 1,
        (ColumnCount::Count(n), None) => (n as usize).max(1),
        (ColumnCount::Auto, Some(w)) => width_derived(w).max(1),
        (ColumnCount::Count(n), Some(w)) => {
            // column-count is a maximum; clamp the width-derived count.
            (n as usize).min(width_derived(w).max(1)).max(1)
        }
    };

    let used_width = if count <= 1 {
        avail
    } else {
        ((avail - (count as f32 - 1.0) * gap) / count as f32).max(0.0)
    };
    (count, used_width)
}

/// One in-flow multi-column child as seen by the packer: its entity,
/// its Taffy-computed block-size (height in horizontal writing mode),
/// and whether a forced column break is requested immediately before /
/// after it (derived from `break-before` / `break-after`). Width is not
/// stored — every column is the resolved `column_width`; the packer
/// places children at the column-x, it does not resize them (plan D1).
#[derive(Clone, Copy, Debug)]
pub(super) struct MulticolChild {
    pub entity: Entity,
    pub height: f32,
    pub force_break_before: bool,
    pub force_break_after: bool,
}

/// A packed child: its entity and its offset relative to the multicol
/// container's content-box origin (plan D7 — written straight into the
/// override map, no container-origin add).
#[derive(Clone, Copy, Debug)]
pub(super) struct PackedChild {
    pub entity: Entity,
    pub offset: Vec2,
}

/// Distribute `children` (document order) into `count` equal-width
/// columns via greedy whole-child packing (plan D2): fill a column
/// top-to-bottom until the next child would exceed `col_block_size`,
/// then move to the next column. A child is never split. Forced column
/// breaks (`force_break_before` / `force_break_after`, plan D4) start a
/// new column at the child boundary; a break before the first child of
/// column 0 is a no-op (no empty leading column). More than `count`
/// columns is never produced — the column index saturates at
/// `count - 1` so an overlong content stream stacks into the final
/// column (whole-child packing, no overflow column).
///
/// Column `c`'s x-offset is `c * (col_width + gap)`. A child's y is the
/// running cumulative height within its column.
///
/// Pure (no Bevy queries / no Taffy reads). Unit-tested in `mod tests`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
pub(super) fn pack_columns(
    children: &[MulticolChild],
    count: usize,
    col_width: f32,
    gap: f32,
    col_block_size: f32,
) -> Vec<PackedChild> {
    let count = count.max(1);
    let last_col = count - 1;
    let mut out: Vec<PackedChild> = Vec::with_capacity(children.len());
    let mut col = 0usize;
    let mut y = 0.0f32;

    for (i, child) in children.iter().enumerate() {
        let is_first_in_layout = i == 0;
        // A forced break-before, or an overflow of the current column,
        // advances to the next column — but never before placing the
        // very first child (no empty leading column).
        let force_break = child.force_break_before && !is_first_in_layout;
        let overflow = y > 0.0 && (y + child.height) > col_block_size;
        if (force_break || overflow) && col < last_col {
            col += 1;
            y = 0.0;
        }

        let x = col as f32 * (col_width + gap);
        out.push(PackedChild {
            entity: child.entity,
            offset: Vec2::new(x, y),
        });
        y += child.height;

        // A forced break-after moves the *next* child to a new column.
        if child.force_break_after && col < last_col {
            col += 1;
            y = 0.0;
        }
    }
    out
}

/// One table row: its entity and the cell entities it owns, in
/// `Children` document order (column index = position in this vec).
#[derive(Clone, Debug)]
pub(super) struct TableRowModel {
    pub entity: Entity,
    pub cells: Vec<Entity>,
}

/// One row-group (explicit `table-row-group`/`header`/`footer`, or the
/// implicit group around bare rows — plan D6): its entity and rows in
/// document order.
#[derive(Clone, Debug)]
pub(super) struct TableRowGroupModel {
    pub entity: Entity,
    pub rows: Vec<TableRowModel>,
}

/// A table's structural spine gathered from the `Children` hierarchy,
/// in document order (plan D5). Caption / column(-group) parts are not
/// stored here — they are deferred-with-warn (plan D4).
#[derive(Clone, Debug, Default)]
pub(super) struct TableModel {
    pub groups: Vec<TableRowGroupModel>,
}

/// Assign each cell / row / row-group a position **relative to the
/// table origin** (spec § 1.2 step 3). Cells sit at the cumulative
/// column-x / cumulative-row-y grid; a row and its group sit at the
/// row's y (a group at its first row's y); groups stack in document
/// order (plan D5). `row_heights` is indexed by the flat row index
/// across all groups (group order, then row order). Returns offsets
/// keyed by entity; the caller adds the table's own Taffy origin.
///
/// Pure. Unit-tested in `mod tests`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
pub(super) fn place_table_cells(
    model: &TableModel,
    col_widths: &[f32],
    row_heights: &[f32],
) -> std::collections::HashMap<Entity, Vec2> {
    // Cumulative column x-offsets: col_x[c] = sum of widths before c.
    let mut col_x: Vec<f32> = Vec::with_capacity(col_widths.len());
    let mut acc = 0.0;
    for &w in col_widths {
        col_x.push(acc);
        acc += w;
    }

    let mut placed: std::collections::HashMap<Entity, Vec2> = std::collections::HashMap::new();
    let mut y = 0.0f32;
    let mut row_index = 0usize;
    for group in &model.groups {
        let group_y = y;
        placed.insert(group.entity, Vec2::new(0.0, group_y));
        for row in &group.rows {
            placed.insert(row.entity, Vec2::new(0.0, y));
            for (ci, &cell) in row.cells.iter().enumerate() {
                let x = col_x.get(ci).copied().unwrap_or(0.0);
                placed.insert(cell, Vec2::new(x, y));
            }
            y += row_heights.get(row_index).copied().unwrap_or(0.0);
            row_index += 1;
        }
    }
    placed
}

/// Walk a table entity's `Children` hierarchy into a `TableModel`
/// (spec § 1.2 step 1). Explicit row-groups contribute their own
/// rows; bare `TableRow` children of the table form a single implicit
/// anonymous row-group in document order (plan D6). Caption / column
/// parts are skipped here (deferred-with-warn, plan D4). Returns the
/// model plus the deferred-part entities for the caller's warn pass.
///
/// `children_q` is the `Query<&Children>`; `display_q` reads each
/// child's `Display`.
///
/// The deferred-part vec is consumed by `table_layout`, which warns
/// `TableSubfeatureUnsupported` once per (entity, session) for each
/// (plan D4); the model half is laid out into the column grid.
fn gather_table(
    table: Entity,
    children_q: &Query<&Children>,
    display_q: &Query<&Display>,
) -> (TableModel, Vec<Entity>) {
    let mut model = TableModel::default();
    let mut deferred: Vec<Entity> = Vec::new();
    // The implicit group accumulates bare rows; flushed when a real
    // group is seen or at the end, preserving document order.
    let mut implicit = TableRowGroupModel {
        entity: table, // implicit group is the table box itself
        rows: Vec::new(),
    };

    let gather_row = |row: Entity| -> TableRowModel {
        let mut cells: Vec<Entity> = Vec::new();
        if let Ok(row_kids) = children_q.get(row) {
            for cell in row_kids.iter() {
                if matches!(display_q.get(cell), Ok(d) if table_part(d) == Some(TablePart::Cell)) {
                    cells.push(cell);
                }
            }
        }
        TableRowModel { entity: row, cells }
    };

    let Ok(table_kids) = children_q.get(table) else {
        return (model, deferred);
    };
    for child in table_kids.iter() {
        match display_q.get(child).ok().and_then(table_part) {
            Some(TablePart::Row) => implicit.rows.push(gather_row(child)),
            Some(TablePart::RowGroup) => {
                let mut group = TableRowGroupModel {
                    entity: child,
                    rows: Vec::new(),
                };
                if let Ok(group_kids) = children_q.get(child) {
                    for gk in group_kids.iter() {
                        if matches!(display_q.get(gk), Ok(d) if table_part(d) == Some(TablePart::Row))
                        {
                            group.rows.push(gather_row(gk));
                        }
                    }
                }
                model.groups.push(group);
            }
            Some(TablePart::Caption | TablePart::Column | TablePart::ColumnGroup) => {
                deferred.push(child);
            }
            _ => {}
        }
    }
    if !implicit.rows.is_empty() {
        // Bare rows precede explicit groups in document order only if
        // they appeared first; for v1 the common case is *either* bare
        // rows *or* explicit groups, so prepend the implicit group.
        model.groups.insert(0, implicit);
    }
    (model, deferred)
}

/// Sub-pass 6b — table layout (spec § 1.2). For each `Display::Table`
/// entity: gather its row-group / row / cell spine (step 1), resolve
/// per-column widths via a synthetic Taffy flex tree (step 2), place
/// every cell / row / row-group into the column grid relative to the
/// table origin, and write the corrected absolute positions into
/// `PostTaffyPositionOverrides` (step 3). Sizes are never touched —
/// they stay from Taffy's block layout (plan D1), matching how 6a
/// (sticky) corrects position only.
///
/// Caption / column(-group) parts and ragged (span-faking) rows warn
/// once per (entity, session) (plan D4 / D8).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
pub(super) fn table_layout(
    tree: NonSend<LayoutTree>,
    table_q: Query<(Entity, &Display), With<Node>>,
    children_q: Query<&Children>,
    display_q: Query<&Display>,
    mut overrides: ResMut<PostTaffyPositionOverrides>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    for (table, display) in table_q.iter() {
        if table_part(display) != Some(TablePart::Table) {
            continue;
        }
        // The table's own natural position (Taffy-block). Skip if Taffy
        // hasn't placed it yet (mirrors sticky_offset's continue-on-miss).
        let Some(table_node) = tree.by_entity.get(&table) else {
            continue;
        };
        let Ok(table_layout) = tree.tree.layout(*table_node) else {
            continue;
        };
        let table_origin = Vec2::new(table_layout.location.x, table_layout.location.y);

        let (model, deferred) = gather_table(table, &children_q, &display_q);
        for d in deferred {
            if warned
                .set
                .insert(LayoutWarnOnceKey::TableSubfeatureUnsupported(d))
            {
                bevy::log::warn!(
                    "Layout: table sub-feature on entity {:?} (caption / column / column-group) \
                     is deferred to v1.x (spec § 1.2); it is left at its block position.",
                    d,
                );
            }
        }
        if model.groups.is_empty() {
            continue;
        }

        // Per-row cell widths (from Taffy) + per-row heights (max cell
        // height in the row). Flat across groups, matching place order.
        let mut rows_widths: Vec<Vec<f32>> = Vec::new();
        let mut row_heights: Vec<f32> = Vec::new();
        for group in &model.groups {
            for row in &group.rows {
                let mut widths: Vec<f32> = Vec::with_capacity(row.cells.len());
                let mut max_h = 0.0f32;
                for &cell in &row.cells {
                    let (w, h) = tree
                        .by_entity
                        .get(&cell)
                        .and_then(|n| tree.tree.layout(*n).ok())
                        .map(|l| (l.size.width, l.size.height))
                        .unwrap_or((0.0, 0.0));
                    widths.push(w);
                    max_h = max_h.max(h);
                }
                rows_widths.push(widths);
                row_heights.push(max_h);
            }
        }

        // Ragged rows (differing cell counts) imply spanning, which has no
        // v1 API — lay out positionally + warn once per table (plan D8).
        let ragged = rows_widths
            .iter()
            .map(|r| r.len())
            .collect::<std::collections::HashSet<_>>()
            .len()
            > 1;
        if ragged
            && warned
                .set
                .insert(LayoutWarnOnceKey::TableSpanUnsupported(table))
        {
            bevy::log::warn!(
                "Layout: table {:?} has rows of differing cell counts; colspan/rowspan \
                 is unsupported in v1 (spec § 1.2) — cells are placed positionally.",
                table,
            );
        }

        let col_widths = resolve_column_widths(&rows_widths);
        let placed = place_table_cells(&model, &col_widths, &row_heights);
        for (entity, offset) in placed {
            overrides.by_entity.insert(entity, table_origin + offset);
        }
    }
}

/// Sub-pass 6c — multi-column packing (spec § 3.2). For each
/// `MultiColumn` container: resolve the used column count + width from
/// the container's Taffy content box (step 1), pack its in-flow
/// children into columns as whole boxes top-to-bottom (step 2,
/// respecting forced `break-before`/`after`), and write each child's
/// corrected container-content-relative position into
/// `PostTaffyPositionOverrides` (plan D7). Sizes are never touched —
/// they stay from Taffy's block layout (plan D1), matching 6a/6b.
///
/// Out-of-flow children (`Position::Absolute`/`Fixed`) and
/// `Display::None` children are excluded (plan D6). True content
/// fragmentation is deferred (plan D2); the residual warn lands in a
/// later task.
///
/// `break-before`/`break-after`/`break-inside` are container-level
/// fields on `MultiColumn` in the v1 API (the spec § 3.1 models them on
/// the multicol box), so a forced break applies *uniformly* to every
/// child — this is the literal reading of "respect break-* properties"
/// given the shipped component shape. Per-child breaks (each child
/// carrying its own `break-*`) require a per-child component that does
/// not exist yet and are a documented follow-up.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
pub(super) fn multicol_pack(
    tree: NonSend<LayoutTree>,
    multicol_q: Query<(Entity, &MultiColumn), With<Node>>,
    children_q: Query<&Children>,
    display_q: Query<&Display>,
    position_q: Query<&Position>,
    mut overrides: ResMut<PostTaffyPositionOverrides>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
) {
    for (container, mc) in multicol_q.iter() {
        // Every `Style`-spawned Node carries a `MultiColumn` component
        // (it is a non-optional bundle field), so the query matches plain
        // block containers too. A container is only a *multi-column* box
        // when `column-count` or `column-width` is explicitly set; the
        // inert default (`Auto` + `None`) is the CSS "neither" case, which
        // resolves to a single column identical to normal block flow.
        // Skip it so we never overwrite a plain block child's natural
        // position (plan D3 "neither" / D6).
        if matches!(mc.column_count, ColumnCount::Auto) && mc.column_width.is_none() {
            continue;
        }

        // The container's Taffy box (content width drives column count).
        let Some(container_node) = tree.by_entity.get(&container) else {
            continue;
        };
        let Ok(container_layout) = tree.tree.layout(*container_node) else {
            continue;
        };
        let content_width = container_layout.size.width;
        let content_height = container_layout.size.height;

        // Gather in-flow children in document order (plan D6).
        let Ok(kids) = children_q.get(container) else {
            continue;
        };
        let mut packed_input: Vec<MulticolChild> = Vec::new();
        for child in kids.iter() {
            // Skip Display::None.
            if matches!(display_q.get(child), Ok(Display::None)) {
                continue;
            }
            // Skip out-of-flow (absolute / fixed escape the columns).
            if let Ok(pos) = position_q.get(child)
                && matches!(pos.kind, PositionKind::Absolute | PositionKind::Fixed)
            {
                continue;
            }
            // Child block-size from Taffy; skip if not yet placed.
            let Some(child_node) = tree.by_entity.get(&child) else {
                continue;
            };
            let Ok(child_layout) = tree.tree.layout(*child_node) else {
                continue;
            };
            let bf = matches!(mc.break_before, BreakBefore::Column | BreakBefore::Always);
            let af = matches!(mc.break_after, BreakAfter::Column | BreakAfter::Always);
            packed_input.push(MulticolChild {
                entity: child,
                height: child_layout.size.height,
                force_break_before: bf,
                force_break_after: af,
            });
        }
        if packed_input.is_empty() {
            continue;
        }

        let gap = multicol_length_px(mc.column_gap, 0.0);
        let width = mc
            .column_width
            .map(|_| multicol_length_px(mc.column_width, 0.0));
        let (count, col_width) = resolve_column_count(mc.column_count, width, gap, content_width);

        // Residual: balanced fill cannot be honored without splitting an
        // oversized child across columns (true fragmentation — tier-E,
        // deferred, plan D2/D5). Detect a child taller than a column's
        // block-size under `column_fill: Balance` and warn once per
        // session; the layout still greedy-packs whole children.
        let col_block_size = content_height;
        if matches!(mc.column_fill, ColumnFill::Balance)
            && packed_input.iter().any(|c| c.height > col_block_size)
            && warned
                .set
                .insert(LayoutWarnOnceKey::MulticolFragmentationDeferred)
        {
            bevy::log::warn!(
                "Layout: a multi-column child is taller than its column and \
                 `column-fill: balance` needs content fragmentation, which is \
                 deferred to v1.x (flex-and-grid.md § 3.2). Falling back to \
                 greedy whole-child packing. This warn fires once per session.",
            );
        }

        let packed = pack_columns(&packed_input, count, col_width, gap, content_height);
        for p in packed {
            overrides.by_entity.insert(p.entity, p.offset);
        }
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
        // `anchor-size()` is intercepted by `try_anchored_position`'s
        // `to_px` closure before delegating here, so this arm is the
        // defensive no-anchor-box fallback (resolve to 0.0).
        Length::AnchorSize(_) => 0.0,
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
            // `anchor-size(<axis>)` resolves to the per-try anchor box's
            // size on the *named* axis (independent of which edge it sits
            // on); every other `Length` delegates to `length_inset_to_px`.
            Sizing::Length(Length::AnchorSize(AxisDimension::Width)) => anchor_size.x,
            Sizing::Length(Length::AnchorSize(AxisDimension::Height)) => anchor_size.y,
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

    // 2 + 3. Build edge map and topologically order the anchored
    // entities (Kahn sort with cycle-edge dropping).
    let (edges, order, dropped, dropped_targets, mut new_warns) =
        build_anchor_edge_map(&anchored_query, &reg, &display_query);

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

    // 6. Idempotent `LayoutAnchorBroken` marker management.
    apply_anchor_broken_markers(
        &mut commands,
        &broken_set,
        &dropped_targets,
        &anchored_query,
        &broken_query,
    );

    // 7. Emit warns (one per unique `(entity, kind)` per frame).
    emit_anchor_warns(new_warns, &mut warned);
}

/// Steps 2 + 3 of `anchor_resolution` — build the anchor edge map and
/// produce the topological order via `kahn_anchor_sort`.
///
/// The Kahn helper does its own pre-pass for external target nodes
/// (D10), so plain-Node targets are not inserted here. `dropped` is a
/// `HashSet` (mirroring `kahn_anchor_sort`'s return) — the driver
/// consumes it with set semantics. `TargetMissing` (edge build) and
/// `InCycle` (per dropped endpoint) warns are accumulated into the
/// returned `new_warns`; the driver appends the remaining kinds.
#[allow(clippy::type_complexity)]
fn build_anchor_edge_map(
    anchored_query: &Query<(Entity, &Anchor, Option<&LayoutAnchorBroken>), With<Node>>,
    reg: &AnchorNameRegistry,
    display_query: &Query<&Display>,
) -> (
    std::collections::HashMap<Entity, Option<Entity>>,
    Vec<Entity>,
    std::collections::HashSet<Entity>,
    std::collections::HashSet<Entity>,
    Vec<(Entity, AnchorErrorKind)>,
) {
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

    (edges, order, dropped, dropped_targets, new_warns)
}

/// Step 6 of `anchor_resolution` — idempotent `LayoutAnchorBroken`
/// marker management. Iterates over every entity that could currently
/// have or need the marker: anchored entities (`anchored_query`) AND
/// `dropped_targets` (which may be plain Nodes without `Anchor`), then
/// cleans up the marker on stale non-anchored entities. The loop order
/// and the `anchored_query.get(t).is_err()` cleanup guard are
/// load-bearing and must stay verbatim.
fn apply_anchor_broken_markers(
    commands: &mut Commands,
    broken_set: &std::collections::HashSet<Entity>,
    dropped_targets: &std::collections::HashSet<Entity>,
    anchored_query: &Query<(Entity, &Anchor, Option<&LayoutAnchorBroken>), With<Node>>,
    broken_query: &Query<(Entity, Option<&LayoutAnchorBroken>)>,
) {
    // Use `broken_query` to read the current marker state for the
    // non-anchored set.
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
    for &t in dropped_targets {
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
}

/// Step 7 of `anchor_resolution` — emit one `warn!` per unique
/// `(entity, kind)` pair this frame. `warned.set.insert` is the
/// per-frame dedupe gate; the match stays exhaustive over every
/// `AnchorErrorKind` arm.
fn emit_anchor_warns(
    new_warns: Vec<(Entity, AnchorErrorKind)>,
    warned: &mut LayoutAnchorWarnedThisFrame,
) {
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
    // content-visibility skip (spec § 5.2). Read-only side queries (conflict-free,
    // like `parent_grid_lookup`): the skip needs *every* entity's `Containment`,
    // last-frame `ResolvedLayout`, and optional `ContainIntrinsicSize`, not just
    // the `Changed`-filtered `nodes` set — an off-screen entity whose only change
    // is its ancestor's resize (so it is absent from `nodes`) must still be
    // classified, or its descendants would be silently re-attached every
    // steady-state frame (the skip would only hold on `Changed` frames).
    containment_lookup: Query<(Entity, &Containment), With<Node>>,
    resolved_lookup: Query<&ResolvedLayout>,
    intrinsic_lookup: Query<&ContainIntrinsicSize>,
    content_vis_margin: Res<ContentVisibilityMargin>,
    mut iter_count: ResMut<SyncStylesIterCount>,
    mut warned: ResMut<LayoutWarnedOnceSession>,
    // Text-leaf probe (text measure § 2.1, decision 2): `With<Text>` —
    // the AUTHORED component, present immediately at spawn — decides
    // whether `translate_one_entity` creates the node with its measure
    // context. Filter-only, conflict-free with every other query here.
    text_leaves: Query<(), With<crate::text::Text>>,
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

    // content-visibility skip sets (spec § 5.2). `skip_children` holds entities
    // whose Taffy child list is emptied this frame (Auto-sentinel or Hidden —
    // D4); `sentinel_size` holds the per-entity contain-intrinsic-size override
    // for the Auto-sentinel case (D2), threaded into `style_to_taffy`.
    //
    // Classified over the FULL (UNfiltered) tree via the read-only side
    // queries, NOT the `Changed`-filtered `nodes` loop below: the children
    // -detach pass (`sync_children_pass`) iterates the unfiltered tree, so the
    // skip set must hold for ALL content-visibility entities every frame. If
    // this were computed inside the `Changed` loop, an off-screen `Auto` (or
    // any `Hidden`) entity that reaches steady-state and drops out of the
    // `Changed` set would lose its `skip_children` membership, and
    // `sync_children_for_entity` would silently re-attach its descendants every
    // steady-state frame — defeating the skip (spec § 5.2 "the big perf win").
    let expanded_viewport = viewport_rect(viewport_size, content_vis_margin.0);
    let mut skip_children: HashSet<Entity> = HashSet::new();
    let mut sentinel_size: HashMap<Entity, bevy::math::Vec2> = HashMap::new();
    for (entity, containment) in containment_lookup.iter() {
        // content-visibility skip (spec § 5.2). Off-screen uses last-frame
        // ResolvedLayout vs the hysteresis-expanded viewport (D3); Auto needs
        // both off-screen AND a contain-intrinsic-size hint (D2).
        let off_screen = is_off_screen(resolved_lookup.get(entity).ok(), expanded_viewport);
        let skip =
            content_visibility_skip(containment, intrinsic_lookup.get(entity).ok(), off_screen);
        match skip {
            SkipKind::None => {
                // D6: the residual diagnostic — Auto + off-screen but no usable
                // intrinsic-size hint, so the requested skip cannot run.
                if matches!(containment.content_visibility, ContentVisibility::Auto)
                    && off_screen
                    && warned
                        .set
                        .insert(LayoutWarnOnceKey::ContentVisibilityDeferred(entity))
                {
                    bevy::log::warn!(
                        "Entity {:?} has content-visibility: auto and is off-screen, but no \
                         contain-intrinsic-size hint — the off-screen layout skip is disabled \
                         for it (spec § 5.2). Set contain-intrinsic-size to enable the skip.",
                        entity,
                    );
                }
            }
            SkipKind::AutoSentinel { intrinsic } => {
                skip_children.insert(entity);
                sentinel_size.insert(
                    entity,
                    bevy::math::Vec2::new(
                        intrinsic.width.unwrap_or(0.0),
                        intrinsic.height.unwrap_or(0.0),
                    ),
                );
            }
            SkipKind::HiddenPrune => {
                skip_children.insert(entity);
            }
        }
    }

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

        // content-visibility sentinel (spec § 5.2): the per-entity
        // `AutoSentinel` size hint, classified in the full-tree pre-pass above.
        // Threaded into this entity's `StyleView` so Taffy reserves the
        // placeholder box without measuring the (detached) descendants.
        translate_one_entity(
            item,
            &parent_areas_for,
            &container_index,
            &cq_parent_chain,
            viewport_size,
            sentinel_size.get(&entity).copied(),
            tree,
            text_leaves.contains(entity),
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
    sync_children_pass(&rows, &skip_children, tree);
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
///
/// `clippy::too_many_arguments` is silenced for the same reason as its
/// three callers: the params are the explicitly-threaded per-frame
/// context those systems share — bundling them into a struct would just
/// move the argument list one level up.
#[allow(clippy::too_many_arguments)]
pub(super) fn translate_one_entity(
    item: NodeQueryItem<'_>,
    parent_areas_for: &HashMap<Entity, GridAreas>,
    container_index: &HashMap<Entity, ContainerSnapshot>,
    cq_parent_chain: &Query<&ChildOf>,
    viewport_size: bevy::math::Vec2,
    content_visibility_intrinsic: Option<bevy::math::Vec2>,
    tree: &mut LayoutTree,
    is_text_leaf: bool,
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
        // content-visibility: auto off-screen sentinel (spec § 5.2): when
        // `Some`, `style_to_taffy` overrides the entity's resolved size with
        // this contain-intrinsic-size hint. Set by the caller from
        // `content_visibility_skip`'s `AutoSentinel` result; `None` otherwise.
        content_visibility_intrinsic,
    };
    let taffy_style = style_to_taffy(view);
    match tree.by_entity.get(&entity).copied() {
        Some(id) => {
            if let Err(err) = tree.tree.set_style(id, taffy_style) {
                warn!(?entity, ?err, "buiy: layout set_style failed");
            }
        }
        None => {
            // Text leaves are created WITH their measure context (text
            // measure § 2.1/§ 2.2): new_leaf_with_context registers the
            // entity at node birth, so a brand-new text entity is
            // measurable on its FIRST frame. Plain nodes use new_leaf —
            // no context, zero-measure dispatch never fires.
            let created = if is_text_leaf {
                tree.tree.new_leaf_with_context(taffy_style, entity)
            } else {
                tree.tree.new_leaf(taffy_style)
            };
            match created {
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
            }
        }
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
    skip_children: &HashSet<Entity>,
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
        sync_children_for_entity(entity, children, &fixed_set, skip_children, tree);
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
        if let Err(err) = set_children_if_changed(tree, root_id, &child_ids) {
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
///
/// `skip_children` carries the entities whose own child list is emptied
/// this frame (`content-visibility: auto` off-screen sentinel or
/// `content-visibility: hidden` — spec § 5.2, D4): Taffy never lays the
/// detached descendants out. Their nodes stay in `LayoutTree` for a cheap
/// re-attach on snap-back (a future `set_children` once the entity leaves
/// the skip set — no `new_leaf` churn).
fn sync_children_for_entity(
    entity: Entity,
    children: Option<&Children>,
    fixed_set: &HashSet<Entity>,
    skip_children: &HashSet<Entity>,
    tree: &mut LayoutTree,
) {
    let parent_id = match tree.by_entity.get(&entity).copied() {
        Some(id) => id,
        None => return,
    };
    let child_ids: Vec<TaffyNodeId> = if skip_children.contains(&entity) {
        // content-visibility skip (D4): detach descendants — Taffy never
        // lays the subtree out. Descendant nodes stay in LayoutTree for a
        // cheap re-attach on snap-back.
        Vec::new()
    } else {
        children
            .into_iter()
            .flatten()
            // Fixed children re-parent to the layout root (spec § 2.1) —
            // exclude them from their in-flow parent's Taffy child list.
            // The root-attach happens in `sync_children_pass` (T4).
            .filter(|c| !fixed_set.contains(c))
            .filter_map(|c| tree.by_entity.get(c).copied())
            .collect()
    };
    if let Err(err) = set_children_if_changed(tree, parent_id, &child_ids) {
        warn!(?entity, ?err, "buiy: layout set_children failed");
    }
}

/// Idempotent `set_children` — skip the call when the parent's Taffy
/// child list already matches `child_ids`.
///
/// Taffy's `set_children` ends with an unconditional `mark_dirty(parent)`
/// (taffy_tree.rs:727), and the children-sync pass runs over the FULL
/// tree every frame (deliberately — the topology is a pure per-frame
/// function of `Position.kind`, D3). Without this guard every node —
/// childless leaves included, via their empty lists — has its layout
/// cache cleared every frame. That was invisible pre-text (Taffy
/// silently recomputed identical results each frame), but it breaks the
/// measure protocol's O(0) steady state: every text leaf would re-invoke
/// the measure closure every frame (measure § 7; the steady-state
/// `TextMeasureCallCount == 0` assertions in tests/text_measure.rs).
/// Same discipline as `write_resolved_layout`'s idempotent insert.
fn set_children_if_changed(
    tree: &mut LayoutTree,
    parent_id: TaffyNodeId,
    child_ids: &[TaffyNodeId],
) -> Result<(), taffy::TaffyError> {
    let unchanged = tree.tree.child_count(parent_id) == child_ids.len()
        && tree
            .tree
            .child_ids(parent_id)
            .zip(child_ids)
            .all(|(current, &wanted)| current == wanted);
    if unchanged {
        return Ok(());
    }
    tree.tree.set_children(parent_id, child_ids)
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
///
/// Text T3: the compute rides `compute_roots_with_text_measure` — text
/// leaves measure through their registered node context (measure § 4.3);
/// the closure adds ZERO extra Taffy passes, so the counter semantics
/// above are unchanged.
pub(super) fn taffy_compute(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<(Entity, Option<&ChildOf>), With<Node>>,
    windows: Query<&bevy::window::Window>,
    mut compute_count: ResMut<LayoutTaffyComputeCount>,
    mut measure: crate::text::TextMeasureParam,
) {
    let tree = &mut *tree;

    // Frame-start resets. `cq_flip_rerun` / `cq_descendant_rerun`
    // increment both counters without resetting, so each counter ends the
    // frame at exactly the number of invocations.
    compute_count.0 = 0;
    measure.reset_call_count();

    // Layout root sizing falls back to 800x600 if no Window exists (test
    // harnesses with MinimalPlugins). Phase 0 used the same default.
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));

    let roots: Vec<(Entity, TaffyNodeId)> = nodes
        .iter()
        .filter(|(_, parent)| {
            parent
                .map(|p| !tree.by_entity.contains_key(&p.parent()))
                .unwrap_or(true)
        })
        .filter_map(|(entity, _)| tree.by_entity.get(&entity).map(|&id| (entity, id)))
        .collect();

    crate::text::measure::compute_roots_with_text_measure(
        tree,
        &mut measure,
        window_size,
        &roots,
        &mut compute_count,
        "main pass",
    );
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

/// Step 8 (`BuiyLayoutStep::CqDescendantInvalidate`) — seed the
/// multi-level container-query geometric cascade. Runs AFTER
/// `write_resolved_layout` (step 7) so it can read `Changed<ResolvedLayout>`
/// on query containers (the entities that actually changed). For every
/// query container (`Container { container_type != Normal }`) whose
/// `ResolvedLayout` changed this frame, walk its descendants and mark them
/// dirty in `ContainerSizeDirty`; if any were marked, set
/// `CqDescendantReRunRequested(true)` so step 9 re-translates them this
/// frame. Bevy ships no "ancestor changed" filter, so the cascade is found
/// by reading the changed container and walking DOWN (D2/D3).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3, § 1.5.
#[allow(clippy::type_complexity)]
pub(super) fn cq_descendant_invalidate(
    changed_containers: Query<(Entity, &Container), (With<Node>, Changed<ResolvedLayout>)>,
    children_q: Query<&Children>,
    flip_reran: Res<CqFlipReRanThisFrame>,
    mut dirty: ResMut<ContainerSizeDirty>,
    mut rerun: ResMut<CqDescendantReRunRequested>,
) {
    // Fresh set each frame — never accumulate (D4).
    dirty.0.clear();

    // D4 cost-ceiling guard: this frame's single re-layout was already spent
    // by `cq_flip_rerun` (an activation-flip frame). Seeding a second re-run
    // now would push Taffy to 3× this frame, breaking the spec § 1.3 "2×
    // Taffy on activation-flip frames" ceiling. Defer the geometric cascade:
    // the next frame's `Changed<ResolvedLayout>` (the flip re-run's recompute
    // surfaced the new size via step 7) re-seeds it (spec § 1.5's
    // explicitly multi-frame transitive-cascade contract). The realistic
    // Phase-14 case — a query container resizing with no rule on itself —
    // never triggers `cq_flip_rerun`, so this guard does not suppress the
    // same-frame settle the phase delivers.
    if flip_reran.0 {
        rerun.0 = false;
        return;
    }

    // Seeds = query containers (Size / InlineSize) whose ResolvedLayout
    // changed this frame. Plain boxes and Normal containers are skipped:
    // descendants only resolve Cq* against a query container (D3).
    let seeds: Vec<Entity> = changed_containers
        .iter()
        .filter(|(_, c)| c.container_type != ContainerType::Normal)
        .map(|(e, _)| e)
        .collect();

    if seeds.is_empty() {
        rerun.0 = false;
        return;
    }

    dirty.0 = collect_dirty_descendants(&seeds, &children_q);
    rerun.0 = !dirty.0.is_empty();
}

/// Step 9 (`BuiyLayoutStep::CqDescendantReRun`) — when
/// `cq_descendant_invalidate` (step 8) marked descendants dirty, re-run the
/// inner work of `sync_styles` + `taffy_compute` for exactly that dirty set,
/// re-write their `ResolvedLayout`, and re-evaluate container queries so a
/// rule-bearing descendant flips its marker the SAME frame (D4/D5). Capped
/// at one re-run per frame: deeper cascade levels settle on subsequent
/// frames (spec § 1.3 / § 1.5). Mirrors `cq_flip_rerun` (step 5).
///
/// Body is gated on `CqDescendantReRunRequested.0`; the flag is cleared at
/// the top so the system is a no-op on non-cascade frames.
///
/// Text T3: the re-run compute rides `compute_roots_with_text_measure`
/// (site 3 of 3, measure § 4.3) — a container resize cascading into a
/// text ancestor re-measures the leaf at its NEW width the SAME frame
/// instead of zero-collapsing it. The helper takes its own
/// `SharedFontSystem` lock, scoped per invocation, so the re-run never
/// overlaps `taffy_compute`'s lock.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3, § 1.5.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn cq_descendant_rerun(
    mut rerun: ResMut<CqDescendantReRunRequested>,
    dirty: Res<ContainerSizeDirty>,
    mut commands: Commands,
    existing: Query<&ResolvedLayout>,
    overrides: Res<PostTaffyPositionOverrides>,
    mut compute_count: ResMut<LayoutTaffyComputeCount>,
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<NodeQueryItem<'_>, With<Node>>,
    parent_grid_lookup: Query<&GridParams>,
    container_snapshot_source: Query<(Entity, &Container, &ResolvedLayout)>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    cq_parent_chain: Query<&ChildOf>,
    // Grouped into ONE param like `rule_queries` below — the grouping
    // frees the slot the measure param takes (text T3, decision 3).
    (roots, windows): (
        Query<(Entity, Option<&Children>, Option<&ChildOf>, &Position), With<Node>>,
        Query<&bevy::window::Window>,
    ),
    // Grouped into ONE param (tuples of `SystemParam`s are themselves a
    // single `SystemParam`) — this system sits at Bevy's 16-param cap, and
    // the grouping frees the slot `text_leaves` takes (T3).
    rule_queries: (
        Query<
            (
                Entity,
                &ContainerQuery,
                Option<&ContainerQueryActive>,
                Option<&ContainerQueryInactive>,
            ),
            With<Node>,
        >,
        Query<(&Container, &ResolvedLayout)>,
    ),
    // Text-leaf probe — see `sync_styles` (text measure § 2.1, decision 2).
    text_leaves: Query<(), With<crate::text::Text>>,
    mut measure: crate::text::TextMeasureParam,
) {
    if !rerun.0 {
        return;
    }
    rerun.0 = false;

    let (rules, containers) = rule_queries;
    let tree = &mut *tree;

    // Snapshots rebuilt from the just-written ResolvedLayout (step 7),
    // exactly as cq_flip_rerun rebuilds them — the dirty descendants resolve
    // Cq* against the NEW ancestor size now present in container_index.
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

    // Re-translate ONLY the dirty descendants (D4). Iterate the full node
    // set but act only on dirty members — keeps the borrow simple and the
    // work bounded by the dirty set.
    for item in nodes.iter() {
        let entity = item.0;
        if !dirty.0.contains(&entity) {
            continue;
        }
        translate_one_entity(
            item,
            &parent_areas_for,
            &container_index,
            &cq_parent_chain,
            viewport_size,
            None,
            tree,
            text_leaves.contains(entity),
        );
    }

    // Children-sync over the FULL tree (`roots`): a dirty descendant that
    // re-translated may need its parent's Taffy child list rebuilt. Mirrors
    // cq_flip_rerun's full-tree children-sync.
    let rows: Vec<(Entity, bool, Option<&Children>, Option<&ChildOf>)> = roots
        .iter()
        .map(|(entity, children, parent, position)| {
            (entity, is_fixed_root(position), children, parent)
        })
        .collect();
    sync_children_pass(&rows, &HashSet::new(), tree);

    // Re-invoke Taffy compute per root through the shared measure helper
    // (text measure § 4.3 — site 3 of 3; same shape as cq_flip_rerun).
    // NO compute_count / measure-call-count reset — those live only in
    // taffy_compute, so a cascade frame ends with count incremented,
    // observable for the 2x cap.
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));
    let root_nodes: Vec<(Entity, TaffyNodeId)> = roots
        .iter()
        .filter(|(_, _, parent, _)| {
            parent
                .map(|p| !tree.by_entity.contains_key(&p.parent()))
                .unwrap_or(true)
        })
        .filter_map(|(entity, ..)| tree.by_entity.get(&entity).map(|&id| (entity, id)))
        .collect();
    crate::text::measure::compute_roots_with_text_measure(
        tree,
        &mut measure,
        window_size,
        &root_nodes,
        &mut compute_count,
        "cq descendant re-run",
    );

    // Re-write ResolvedLayout for the dirty set from the recomputed Taffy
    // tree (mirror write_resolved_layout, scoped to dirty).
    for &entity in dirty.0.iter() {
        let Some(&id) = tree.by_entity.get(&entity) else {
            continue;
        };
        let Ok(layout) = tree.tree.layout(id) else {
            continue;
        };
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
            commands.entity(entity).insert(new);
        }
    }

    // Re-evaluate container queries against the just-recomputed sizes so a
    // rule-bearing descendant flips its marker THIS frame (D5). Same toggle
    // logic as cq_activate, reading sizes from the Taffy tree directly
    // (current this frame) — NOT the not-yet-applied Commands insert above,
    // matching cq_flip_check's explicit source pinning (architecture.md § 3.2).
    let mut memo: HashMap<Entity, Option<Entity>> = HashMap::new();
    for (entity, rule, was_active, was_inactive) in rules.iter() {
        let container_entity = resolve_nearest_container(
            entity,
            &rule.container,
            &mut memo,
            &containers,
            &cq_parent_chain,
        );
        let active = match container_entity {
            Some(cont) => match tree.by_entity.get(&cont) {
                Some(&node_id) => match tree.tree.layout(node_id) {
                    Ok(layout) => evaluate_conditions(
                        &rule.conditions,
                        Vec2::new(layout.size.width, layout.size.height),
                    ),
                    Err(_) => false,
                },
                None => false,
            },
            None => false,
        };
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
        // Fr is a grid-only unit; degrades to 0 here. anchor-size() is
        // only meaningful inside anchor inset resolution; in a container-
        // query condition value it has no anchor box, so it degrades to 0.
        Length::Fr(_)
        | Length::Cqw(_)
        | Length::Cqh(_)
        | Length::Cqi(_)
        | Length::Cqb(_)
        | Length::Cqmin(_)
        | Length::Cqmax(_)
        | Length::AnchorSize(_) => 0.0,
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

/// Flatten the descendant subtrees of `seeds` into a deduplicated set,
/// EXCLUDING the seeds themselves. Phase 14 step 8 (`cq_descendant_invalidate`)
/// calls this with the query containers whose `ResolvedLayout` changed this
/// frame; the returned set is every entity that may resolve a `Length::Cq*`
/// unit (or a `ContainerQuery`) against one of those containers and must be
/// re-translated this frame (D2).
///
/// Iterative breadth-first walk over `Children`. O(total subtree size); a
/// `HashSet` membership guard makes overlapping seed subtrees (a container
/// nested inside another changed container) cost each entity once (D2/D4).
/// No cycle guard is needed — Bevy's `Children`/`ChildOf` hierarchy is a
/// forest by construction.
pub(super) fn collect_dirty_descendants(
    seeds: &[Entity],
    children_q: &Query<&Children>,
) -> std::collections::HashSet<Entity> {
    let mut dirty: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    let mut stack: Vec<Entity> = seeds.to_vec();
    while let Some(entity) = stack.pop() {
        if let Ok(children) = children_q.get(entity) {
            for child in children.iter() {
                if dirty.insert(child) {
                    stack.push(child);
                }
            }
        }
    }
    dirty
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
/// allowed up to 16 params; this one is AT the cap (`(roots, windows)`
/// are grouped into one tuple param to make room for the measure param).
///
/// Text T3: the re-run compute rides `compute_roots_with_text_measure`
/// (site 2 of 3, measure § 4.3) — a flip that changes a text ancestor's
/// width re-measures the leaf the SAME frame instead of zero-collapsing
/// it. The helper takes its own `SharedFontSystem` lock, scoped per
/// invocation, so the re-run never overlaps `taffy_compute`'s lock.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn cq_flip_rerun(
    mut rerun: ResMut<CqReRunRequested>,
    mut flip_reran: ResMut<CqFlipReRanThisFrame>,
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
    // per-root compute below. Grouped with `windows` into ONE param (tuples
    // of `SystemParam`s are themselves a single `SystemParam`) — this system
    // sits at Bevy's 16-param cap, and the grouping frees the slot the
    // measure param takes (text T3, decision 3).
    (roots, windows): (
        Query<(Entity, Option<&Children>, Option<&ChildOf>, &Position), With<Node>>,
        Query<&bevy::window::Window>,
    ),
    // content-visibility skip inputs (spec § 5.2, D8) — read-only and disjoint
    // from the mutable `tree`/`rerun`/`compute_count`, mirroring `sync_styles`'s
    // side queries. `containment_lookup` is the FULL (unfiltered) classification
    // source (the children-detach pass iterates every parent, not just the
    // `Changed`-filtered `nodes`); `resolved_lookup`/`intrinsic_lookup` feed the
    // off-screen test + the per-axis hint. No `warned` resource here — the D6
    // diagnostic already fired in `sync_styles` this frame (do NOT re-warn).
    containment_lookup: Query<(Entity, &Containment), With<Node>>,
    resolved_lookup: Query<&ResolvedLayout>,
    intrinsic_lookup: Query<&ContainIntrinsicSize>,
    content_vis_margin: Res<ContentVisibilityMargin>,
    // Text-leaf probe — see `sync_styles` (text measure § 2.1, decision 2).
    text_leaves: Query<(), With<crate::text::Text>>,
    mut measure: crate::text::TextMeasureParam,
) {
    if !rerun.0 {
        // No activation flip this frame — the descendant pass (step 8) is
        // free to spend the single re-layout (D4).
        flip_reran.0 = false;
        return;
    }
    rerun.0 = false;
    // Record that this frame's single re-layout was spent here, so step 8
    // defers the geometric descendant cascade to the next frame (D4).
    flip_reran.0 = true;

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

    // content-visibility skip sets (spec § 5.2, D8) — reproduced IDENTICALLY to
    // `sync_styles`. Classified over the FULL tree (`containment_lookup`), not
    // the `Changed`-filtered `nodes`: the children-detach pass below iterates
    // every parent, so a skipped entity that is no longer in the re-run's
    // changed set must still be detached, or this flip frame would re-attach its
    // descendants and undo the skip (the exact thrash D8 guards against).
    let expanded_viewport = viewport_rect(viewport_size, content_vis_margin.0);
    let mut skip_children: HashSet<Entity> = HashSet::new();
    let mut sentinel_size: HashMap<Entity, bevy::math::Vec2> = HashMap::new();
    for (entity, containment) in containment_lookup.iter() {
        let off_screen = is_off_screen(resolved_lookup.get(entity).ok(), expanded_viewport);
        match content_visibility_skip(containment, intrinsic_lookup.get(entity).ok(), off_screen) {
            SkipKind::None => {}
            SkipKind::AutoSentinel { intrinsic } => {
                skip_children.insert(entity);
                sentinel_size.insert(
                    entity,
                    bevy::math::Vec2::new(
                        intrinsic.width.unwrap_or(0.0),
                        intrinsic.height.unwrap_or(0.0),
                    ),
                );
            }
            SkipKind::HiddenPrune => {
                skip_children.insert(entity);
            }
        }
    }

    for item in nodes.iter() {
        let entity = item.0;
        translate_one_entity(
            item,
            &parent_areas_for,
            &container_index,
            &cq_parent_chain,
            viewport_size,
            sentinel_size.get(&entity).copied(),
            tree,
            text_leaves.contains(entity),
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
    // Honor the content-visibility skip set (D8): the same entities
    // `sync_styles` detached this frame stay detached across the flip re-run.
    sync_children_pass(&rows, &skip_children, tree);

    // Re-invoke Taffy compute through the shared measure helper (text
    // measure § 4.3 — site 2 of 3). Same code shape as `taffy_compute`,
    // but WITHOUT the `compute_count.0 = 0` / `reset_call_count`
    // frame-resets (those live only in `taffy_compute`, so a flip frame
    // ends at `count == 2`, not `count == 1`).
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));
    let root_nodes: Vec<(Entity, TaffyNodeId)> = roots
        .iter()
        .filter(|(_, _, parent, _)| {
            parent
                .map(|p| !tree.by_entity.contains_key(&p.parent()))
                .unwrap_or(true)
        })
        .filter_map(|(entity, ..)| tree.by_entity.get(&entity).map(|&id| (entity, id)))
        .collect();
    crate::text::measure::compute_roots_with_text_measure(
        tree,
        &mut measure,
        window_size,
        &root_nodes,
        &mut compute_count,
        "cq flip re-run",
    );
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

/// Resolve a `MultiColumn` length metric (`column_width` / `column_gap`)
/// to px for the v1 packer. Only `Length::Px` is meaningful in v1
/// (percent / cq column metrics are a non-goal — plan D8); any other
/// variant, or `None`, yields `fallback`. The gap's fallback is `0.0`
/// (CSS `normal` maps to 0 pre-font-metrics); a width is only resolved
/// when `Some`, and a non-`Px` width resolving to its fallback (0.0)
/// makes `resolve_column_count` treat it as "no usable width".
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
pub(super) fn multicol_length_px(l: Option<Length>, fallback: f32) -> f32 {
    match l {
        Some(Length::Px(v)) => v,
        _ => fallback,
    }
}

/// Compose the final transform matrix per spec § 1:
/// `M = T_translate · R_rotate · S_scale · M_transform`.
/// The longhand `Translate`/`Rotate`/`Scale` (absent → identity
/// contribution) are the outer factors; `UiTransform.matrix` is the
/// innermost. A child point `p` is transformed as `M · p`, so it
/// feels the rightmost (innermost) factor first.
///
/// Pure function — no Bevy queries, no Taffy reads. Easy to unit test, and
/// consumed by the Tier-3 `transform_roundtrips` invariant (the metamorphic
/// `translate∘-translate ≈ I`, `rotate(2π) ≈ I`, `scale(k)` checks assert on
/// THIS composed matrix, never a re-implementation), hence `pub`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 1.1.
pub fn compose_transform(
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

/// The top-layer **paint rank**: a total order over [`TopLayer`] variants where
/// a SMALLER rank paints lower (earlier) and a larger rank paints higher
/// (later). Fullscreen sits at the bottom of the top layer (`0`), Modal at the
/// top (`3`); `None` (in-flow, not in the top layer) is the sentinel `u8::MAX`,
/// so any escaping variant outranks (paints below) an in-flow node.
///
/// This is the SINGLE source of truth for top-layer dominance, shared by the
/// layout escape sort (sub-pass 6f) and the verification harness's
/// `top_layer_dominates` invariant. It is deliberately NOT the `TopLayer`
/// enum's declared discriminant order (`None, Modal, Popover, Tooltip,
/// Fullscreen`), so `#[derive(Ord)]` on `TopLayer` would give the WRONG
/// dominance — callers must compare via this rank, never the discriminant.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 4.
pub fn top_layer_paint_rank(t: TopLayer) -> u8 {
    match t {
        TopLayer::Fullscreen => 0,
        TopLayer::Tooltip => 1,
        TopLayer::Popover => 2,
        TopLayer::Modal => 3,
        TopLayer::None => u8::MAX,
    }
}

/// The spec § 2 union of stacking-context-formation triggers:
/// (1) positioned with explicit `z_index`, (2) `Isolation::Isolate`,
/// (3) non-identity transform, (4) `Containment.contain ⊇ PAINT/STRICT`,
/// (5) the render-side formers (`Opacity < 1`, non-empty `Filter`,
/// `MixBlendMode != Normal` — read from the render-owned components, same
/// crate), (6) root. Trigger 5's `will-change` former is still deferred
/// with the rest of `will-change` layer promotion (spec § 7);
/// `BackdropFilter` is deliberately NOT a trigger — it forms an
/// `EffectGroup` but never a stacking context (render component-model.md
/// § 8).
///
/// Driven by the `stacking_context` sub-pass (6f).
#[allow(clippy::too_many_arguments)]
pub(super) fn forms_stacking_context(
    stacking: Option<&Stacking>,
    position_kind: PositionKind,
    has_transform: bool,
    containment: Option<&Containment>,
    opacity: Option<&Opacity>,
    filter: Option<&Filter>,
    blend: Option<&MixBlendMode>,
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
    // Trigger 5 — render-side formers. Delegates to the render effect
    // module so this trigger and the effect-group former predicate
    // (`render::effect::effect_reason_for`) share one source of truth:
    // every SC-forming `EffectGroup` is atomic in painters_z, which is the
    // compositor's contiguity invariant (render/buckets.rs).
    if forms_render_stacking_context(opacity, filter, blend) {
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
    // Trigger-5 render-side former inputs (spec § 2 trigger 5): the
    // render-owned `Opacity` / `Filter` / `MixBlendMode` components, read
    // here so an effect former paints atomically. Grouped in one tuple
    // param to stay within Bevy's 16-element SystemParam tuple.
    render_formers: (Query<&Opacity>, Query<&Filter>, Query<&MixBlendMode>),
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
    let (opacity_q, filter_q, blend_q) = &render_formers;
    let forms = |e: Entity, root: bool| {
        forms_stacking_context(
            stacking_q.get(e).ok(),
            pos_kind(e),
            transformed.get(e).is_ok(),
            containment_q.get(e).ok(),
            opacity_q.get(e).ok(),
            filter_q.get(e).ok(),
            blend_q.get(e).ok(),
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
    // The tier rank is the SINGLE source of truth shared with the verification
    // harness — see [`top_layer_paint_rank`].
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
    top_sorted.sort_by_cached_key(|&e| top_layer_paint_rank(top_layer_of(e)));
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

    use crate::layout::components::Display;

    #[test]
    fn container_size_dirty_default_is_empty() {
        assert!(ContainerSizeDirty::default().0.is_empty());
    }

    #[test]
    fn cq_descendant_rerun_requested_default_is_false() {
        assert!(!CqDescendantReRunRequested::default().0);
    }

    #[test]
    fn collect_dirty_descendants_flattens_subtree() {
        use bevy::prelude::*;
        let mut world = World::new();
        // a -> b -> c, plus a sibling leaf d under a.
        let c = world.spawn(Node).id();
        let d = world.spawn(Node).id();
        let b = world.spawn(Node).add_children(&[c]).id();
        let a = world.spawn(Node).add_children(&[b, d]).id();
        let mut q = world.query::<&Children>();
        let children_q = q.query(&world);
        let dirty = collect_dirty_descendants(&[a], &children_q);
        // a's descendants: b, c, d (a itself excluded).
        assert!(dirty.contains(&b));
        assert!(dirty.contains(&c));
        assert!(dirty.contains(&d));
        assert!(
            !dirty.contains(&a),
            "the seed container itself is not dirty"
        );
        assert_eq!(dirty.len(), 3);
    }

    #[test]
    fn collect_dirty_descendants_empty_for_leaf_seed() {
        use bevy::prelude::*;
        let mut world = World::new();
        let leaf = world.spawn(Node).id();
        let mut q = world.query::<&Children>();
        let children_q = q.query(&world);
        let dirty = collect_dirty_descendants(&[leaf], &children_q);
        assert!(
            dirty.is_empty(),
            "a seed with no children produces no dirty descendants"
        );
    }

    #[test]
    fn collect_dirty_descendants_dedups_overlapping_subtrees() {
        use bevy::prelude::*;
        let mut world = World::new();
        // a -> b -> c ; seed both a and b. c must appear once.
        let c = world.spawn(Node).id();
        let b = world.spawn(Node).add_children(&[c]).id();
        let a = world.spawn(Node).add_children(&[b]).id();
        let mut q = world.query::<&Children>();
        let children_q = q.query(&world);
        let dirty = collect_dirty_descendants(&[a, b], &children_q);
        assert!(dirty.contains(&b));
        assert!(dirty.contains(&c));
        assert_eq!(
            dirty.len(),
            2,
            "c is reached from both a and b but appears once"
        );
    }

    #[test]
    fn table_part_classifies_every_family_member() {
        assert_eq!(table_part(&Display::Table), Some(TablePart::Table));
        assert_eq!(
            table_part(&Display::TableRowGroup),
            Some(TablePart::RowGroup)
        );
        assert_eq!(
            table_part(&Display::TableHeaderGroup),
            Some(TablePart::RowGroup)
        );
        assert_eq!(
            table_part(&Display::TableFooterGroup),
            Some(TablePart::RowGroup)
        );
        assert_eq!(table_part(&Display::TableRow), Some(TablePart::Row));
        assert_eq!(table_part(&Display::TableCell), Some(TablePart::Cell));
        assert_eq!(table_part(&Display::TableCaption), Some(TablePart::Caption));
        assert_eq!(table_part(&Display::TableColumn), Some(TablePart::Column));
        assert_eq!(
            table_part(&Display::TableColumnGroup),
            Some(TablePart::ColumnGroup)
        );
    }

    #[test]
    fn table_part_is_none_for_non_table_display() {
        assert_eq!(table_part(&Display::Block), None);
        assert_eq!(table_part(&Display::None), None);
        assert_eq!(
            table_part(&Display::Flex(crate::layout::types::FlexAxis::Row)),
            None
        );
    }

    #[test]
    fn resolve_column_widths_single_row_passes_cell_widths_through() {
        // One row, three cells 30/50/20 → columns resolve to 30/50/20.
        let cols = resolve_column_widths(&[vec![30.0, 50.0, 20.0]]);
        assert_eq!(cols.len(), 3);
        assert!((cols[0] - 30.0).abs() < 0.5);
        assert!((cols[1] - 50.0).abs() < 0.5);
        assert!((cols[2] - 20.0).abs() < 0.5);
    }

    #[test]
    fn resolve_column_widths_takes_per_column_max_across_rows() {
        // Row A: 30/50  Row B: 40/20 → columns = max(30,40)=40, max(50,20)=50.
        let cols = resolve_column_widths(&[vec![30.0, 50.0], vec![40.0, 20.0]]);
        assert_eq!(cols.len(), 2);
        assert!((cols[0] - 40.0).abs() < 0.5, "col0 = max(30,40) = 40");
        assert!((cols[1] - 50.0).abs() < 0.5, "col1 = max(50,20) = 50");
    }

    #[test]
    fn resolve_column_widths_ragged_rows_use_max_row_length() {
        // Row A has 3 cells, Row B has 1 → 3 columns; the missing cells
        // contribute 0 width (D8 ragged-row handling).
        let cols = resolve_column_widths(&[vec![10.0, 20.0, 30.0], vec![15.0]]);
        assert_eq!(cols.len(), 3, "column count = widest row");
        assert!((cols[0] - 15.0).abs() < 0.5, "col0 = max(10,15) = 15");
        assert!((cols[1] - 20.0).abs() < 0.5);
        assert!((cols[2] - 30.0).abs() < 0.5);
    }

    #[test]
    fn resolve_column_widths_empty_table_is_empty() {
        assert!(resolve_column_widths(&[]).is_empty());
        // A table with rows but no cells → zero columns.
        assert!(resolve_column_widths(&[vec![], vec![]]).is_empty());
    }

    // Build entities with stable ids for assertions.
    fn ent(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn place_single_row_two_cells_in_column_grid() {
        // Two columns 40/60; one group with one row (height 20) holding
        // two cells. Cell 0 at x=0, cell 1 at x=40; both at y=0.
        let model = TableModel {
            groups: vec![TableRowGroupModel {
                entity: ent(1),
                rows: vec![TableRowModel {
                    entity: ent(2),
                    cells: vec![ent(3), ent(4)],
                }],
            }],
        };
        let placed = place_table_cells(&model, &[40.0, 60.0], &[20.0]);
        assert_eq!(placed[&ent(3)], bevy::math::Vec2::new(0.0, 0.0));
        assert_eq!(placed[&ent(4)], bevy::math::Vec2::new(40.0, 0.0));
        // Row + group sit at the table origin.
        assert_eq!(placed[&ent(2)], bevy::math::Vec2::new(0.0, 0.0));
        assert_eq!(placed[&ent(1)], bevy::math::Vec2::new(0.0, 0.0));
    }

    #[test]
    fn place_two_rows_stack_vertically_by_row_height() {
        // Row 0 height 20, row 1 height 30. Row 1 starts at y=20.
        let model = TableModel {
            groups: vec![TableRowGroupModel {
                entity: ent(1),
                rows: vec![
                    TableRowModel {
                        entity: ent(2),
                        cells: vec![ent(3)],
                    },
                    TableRowModel {
                        entity: ent(4),
                        cells: vec![ent(5)],
                    },
                ],
            }],
        };
        let placed = place_table_cells(&model, &[40.0], &[20.0, 30.0]);
        assert_eq!(placed[&ent(3)], bevy::math::Vec2::new(0.0, 0.0));
        assert_eq!(placed[&ent(2)], bevy::math::Vec2::new(0.0, 0.0));
        assert_eq!(
            placed[&ent(5)],
            bevy::math::Vec2::new(0.0, 20.0),
            "row 1 cell below row 0"
        );
        assert_eq!(
            placed[&ent(4)],
            bevy::math::Vec2::new(0.0, 20.0),
            "row 1 at y=20"
        );
    }

    #[test]
    fn place_two_groups_stack_in_document_order() {
        // Group A (1 row, height 20) then group B (1 row, height 30).
        // Group B's row starts at y=20 (D5 — document-order stacking).
        let model = TableModel {
            groups: vec![
                TableRowGroupModel {
                    entity: ent(1),
                    rows: vec![TableRowModel {
                        entity: ent(2),
                        cells: vec![ent(3)],
                    }],
                },
                TableRowGroupModel {
                    entity: ent(4),
                    rows: vec![TableRowModel {
                        entity: ent(5),
                        cells: vec![ent(6)],
                    }],
                },
            ],
        };
        let placed = place_table_cells(&model, &[40.0], &[20.0, 30.0]);
        assert_eq!(
            placed[&ent(1)],
            bevy::math::Vec2::new(0.0, 0.0),
            "group A at top"
        );
        assert_eq!(placed[&ent(3)], bevy::math::Vec2::new(0.0, 0.0));
        assert_eq!(
            placed[&ent(4)],
            bevy::math::Vec2::new(0.0, 20.0),
            "group B below A"
        );
        assert_eq!(placed[&ent(5)], bevy::math::Vec2::new(0.0, 20.0));
        assert_eq!(placed[&ent(6)], bevy::math::Vec2::new(0.0, 20.0));
    }

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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
            None,
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
            None,
            None,
            None,
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
            None,
            None,
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
            None,
            None,
            None,
            false
        ));
    }

    // ---- trigger 5 — render-side formers (spec § 2; shares term semantics
    // with `render::effect::effect_reason_for` via
    // `forms_render_stacking_context`) ----

    /// `forms_stacking_context` with no other trigger but the given
    /// render-side former inputs — DRYs the trigger-5 boundary tests below.
    fn forms_via_render(
        opacity: Option<&Opacity>,
        filter: Option<&Filter>,
        blend: Option<&MixBlendMode>,
    ) -> bool {
        forms_stacking_context(
            None,
            PositionKind::Static,
            false,
            None,
            opacity,
            filter,
            blend,
            false,
        )
    }

    #[test]
    fn opacity_below_one_forms_context() {
        assert!(forms_via_render(Some(&Opacity(0.99)), None, None));
    }

    #[test]
    fn opacity_exactly_one_does_not_form_context() {
        // The `< 1.0` boundary: 1.0 is the CSS-initial no-op — presence of
        // the component alone must not form a context.
        assert!(!forms_via_render(Some(&Opacity(1.0)), None, None));
    }

    #[test]
    fn non_empty_filter_forms_context() {
        use crate::render::components::FilterFn;
        let f = Filter(vec![FilterFn::Blur(Length::px(2.0))]);
        assert!(forms_via_render(None, Some(&f), None));
    }

    #[test]
    fn empty_filter_does_not_form_context() {
        assert!(!forms_via_render(None, Some(&Filter(vec![])), None));
    }

    #[test]
    fn non_normal_mix_blend_forms_context() {
        assert!(forms_via_render(None, None, Some(&MixBlendMode::Multiply)));
    }

    #[test]
    fn normal_mix_blend_does_not_form_context() {
        assert!(!forms_via_render(None, None, Some(&MixBlendMode::Normal)));
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

        let skip_children: HashSet<Entity> = HashSet::new();
        sync_children_for_entity(
            parent,
            Some(children),
            &fixed_set,
            &skip_children,
            &mut tree,
        );

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
    // Phase 12 — `table_layout` (sub-pass 6b) is now the real algorithm
    // (gather → resolve columns → place cells → write overrides). Its
    // behavior is covered by the integration suite `tests/layout_table.rs`
    // and the pure-helper unit tests above (`table_part`,
    // `resolve_column_widths`, `place_table_cells`). The Phase-7
    // stub-warn unit test was removed when the stub was superseded —
    // `table_layout` now requires `NonSend<LayoutTree>`, which a bare
    // `App` in a unit test does not provide.
    // -----------------------------------------------------------------

    // Phase 13 — the Phase-7 `multicol_pack_warns_once_per_session` stub
    // test was removed: 6c now packs columns (no blanket warn). Packing +
    // the residual fragmentation warn are covered by
    // `tests/layout_multicol.rs`.

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

    #[test]
    fn try_anchored_position_resolves_anchor_size_height() {
        // anchor-size(height): `inset.top` resolves to the anchor's own
        // height (40), so the anchored entity is placed at
        // anchor.y(0) + anchor.height(40) + resolved_top(40) = 80.
        let inset = Inset {
            top: Sizing::Length(Length::AnchorSize(AxisDimension::Height)),
            ..Default::default()
        };
        let pos = try_anchored_position(
            Vec2::ZERO,
            Vec2::new(80.0, 40.0),
            Vec2::new(10.0, 10.0),
            &inset,
            Vec2::new(800.0, 600.0),
        );
        assert_eq!(pos.y, 80.0);
    }

    #[test]
    fn try_anchored_position_resolves_anchor_size_width_on_left_edge() {
        // anchor-size(width) on the `left` edge resolves to the anchor's
        // width (80) regardless of axis, so x =
        // anchor.x(0) + anchor.width(80) + resolved_left(80) = 160.
        let inset = Inset {
            left: Sizing::Length(Length::AnchorSize(AxisDimension::Width)),
            ..Default::default()
        };
        let pos = try_anchored_position(
            Vec2::ZERO,
            Vec2::new(80.0, 40.0),
            Vec2::new(10.0, 10.0),
            &inset,
            Vec2::new(800.0, 600.0),
        );
        assert_eq!(pos.x, 160.0);
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

    #[test]
    fn multicol_length_px_px_passes_through() {
        assert_eq!(multicol_length_px(Some(Length::Px(120.0)), 0.0), 120.0);
    }

    #[test]
    fn multicol_length_px_none_uses_fallback() {
        assert_eq!(multicol_length_px(None, 16.0), 16.0);
    }

    #[test]
    fn multicol_length_px_non_px_uses_fallback() {
        // percent / cq column metrics are a v1 non-goal (D8) — fall back.
        assert_eq!(multicol_length_px(Some(Length::Percent(50.0)), 0.0), 0.0);
        assert_eq!(multicol_length_px(Some(Length::Cqw(10.0)), 7.0), 7.0);
    }

    #[test]
    fn resolve_column_count_neither_is_single_column() {
        // No count, no width → 1 column spanning the box.
        let (n, w) = resolve_column_count(ColumnCount::Auto, None, 0.0, 400.0);
        assert_eq!(n, 1);
        assert_eq!(w, 400.0);
    }

    #[test]
    fn resolve_column_count_count_only_divides_with_gaps() {
        // count = 3, gap = 20, width 440 → 3 cols, (440 - 2*20)/3 = 133.33.
        let (n, w) = resolve_column_count(ColumnCount::Count(3), None, 20.0, 440.0);
        assert_eq!(n, 3);
        assert!((w - 400.0 / 3.0).abs() < 1e-3, "used width = {w}");
    }

    #[test]
    fn resolve_column_count_count_zero_clamps_to_one() {
        let (n, _w) = resolve_column_count(ColumnCount::Count(0), None, 0.0, 400.0);
        assert_eq!(n, 1, "count 0 clamps to 1 column");
    }

    #[test]
    fn resolve_column_count_width_only_floors_then_fills() {
        // width 100, gap 0, available 350 → floor((350+0)/(100+0)) = 3 cols;
        // used width = (350 - 0)/3 = 116.67 (columns expand to fill).
        let (n, w) = resolve_column_count(ColumnCount::Auto, Some(100.0), 0.0, 350.0);
        assert_eq!(n, 3);
        assert!((w - 350.0 / 3.0).abs() < 1e-3, "used width = {w}");
    }

    #[test]
    fn resolve_column_count_width_only_with_gap() {
        // width 100, gap 25, available 350 → floor((350+25)/(100+25)) =
        // floor(375/125) = 3 cols; used width = (350 - 2*25)/3 = 100.
        let (n, w) = resolve_column_count(ColumnCount::Auto, Some(100.0), 25.0, 350.0);
        assert_eq!(n, 3);
        assert!((w - 100.0).abs() < 1e-3, "used width = {w}");
    }

    #[test]
    fn resolve_column_count_both_count_is_max() {
        // count = 2 (a maximum), width 100, gap 0, available 350.
        // width-derived = floor(350/100) = 3, capped at count 2 → 2 cols.
        let (n, _w) = resolve_column_count(ColumnCount::Count(2), Some(100.0), 0.0, 350.0);
        assert_eq!(n, 2, "column-count caps the width-derived count");
    }

    #[test]
    fn resolve_column_count_both_width_wins_when_smaller() {
        // count = 5, width 100, gap 0, available 350 →
        // width-derived = 3, min(5, 3) = 3 cols.
        let (n, _w) = resolve_column_count(ColumnCount::Count(5), Some(100.0), 0.0, 350.0);
        assert_eq!(n, 3);
    }

    #[test]
    fn resolve_column_count_width_wider_than_box_is_one_column() {
        // width 500 > available 400 → floor((400+0)/(500+0)) = 0 → clamp 1.
        let (n, w) = resolve_column_count(ColumnCount::Auto, Some(500.0), 0.0, 400.0);
        assert_eq!(n, 1);
        assert!((w - 400.0).abs() < 1e-3);
    }

    // Build a MulticolChild test fixture (entity, height, no forced breaks).
    fn mc_child(world: &mut World, height: f32) -> MulticolChild {
        let e = world.spawn_empty().id();
        MulticolChild {
            entity: e,
            height,
            force_break_before: false,
            force_break_after: false,
        }
    }

    #[test]
    fn pack_columns_fills_columns_top_to_bottom() {
        // 2 columns, width 100, gap 20, col block-size 100.
        // children heights [40, 40, 40]: col0 gets [40,40] (y 0,40),
        // col1 gets [40] (y 0). col x: col0 = 0, col1 = 120.
        let mut world = World::new();
        let a = mc_child(&mut world, 40.0);
        let b = mc_child(&mut world, 40.0);
        let c = mc_child(&mut world, 40.0);
        let (ea, eb, ec) = (a.entity, b.entity, c.entity);
        let packed = pack_columns(&[a, b, c], 2, 100.0, 20.0, 100.0);
        let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
        assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
        assert_eq!(pos(eb), Vec2::new(0.0, 40.0));
        assert_eq!(pos(ec), Vec2::new(120.0, 0.0));
    }

    #[test]
    fn pack_columns_overflow_starts_next_column() {
        // col block-size 50; heights [40, 40] → b doesn't fit after a
        // (40+40 > 50) → b starts col1.
        let mut world = World::new();
        let a = mc_child(&mut world, 40.0);
        let b = mc_child(&mut world, 40.0);
        let (ea, eb) = (a.entity, b.entity);
        let packed = pack_columns(&[a, b], 2, 100.0, 0.0, 50.0);
        let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
        assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
        assert_eq!(pos(eb), Vec2::new(100.0, 0.0), "overflow pushes b to col1");
    }

    #[test]
    fn pack_columns_force_break_before_starts_new_column() {
        // both fit in col0 by size, but b has force_break_before → col1.
        let mut world = World::new();
        let a = mc_child(&mut world, 10.0);
        let mut b = mc_child(&mut world, 10.0);
        b.force_break_before = true;
        let (ea, eb) = (a.entity, b.entity);
        let packed = pack_columns(&[a, b], 2, 100.0, 0.0, 500.0);
        let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
        assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
        assert_eq!(
            pos(eb),
            Vec2::new(100.0, 0.0),
            "force-break-before starts col1"
        );
    }

    #[test]
    fn pack_columns_force_break_after_pushes_next_child() {
        // a has force_break_after → b starts col1 even though it would fit.
        let mut world = World::new();
        let mut a = mc_child(&mut world, 10.0);
        a.force_break_after = true;
        let b = mc_child(&mut world, 10.0);
        let (ea, eb) = (a.entity, b.entity);
        let packed = pack_columns(&[a, b], 2, 100.0, 0.0, 500.0);
        let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
        assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
        assert_eq!(
            pos(eb),
            Vec2::new(100.0, 0.0),
            "force-break-after pushes b to col1"
        );
    }

    #[test]
    fn pack_columns_break_at_column_zero_is_no_op() {
        // force_break_before on the very first child must not create an empty
        // column 0 — it stays in col0.
        let mut world = World::new();
        let mut a = mc_child(&mut world, 10.0);
        a.force_break_before = true;
        let _ea = a.entity;
        let packed = pack_columns(&[a], 1, 100.0, 0.0, 500.0);
        assert_eq!(
            packed[0].offset,
            Vec2::new(0.0, 0.0),
            "break on first child is a no-op"
        );
    }

    #[test]
    fn pack_columns_single_column_stacks_all() {
        let mut world = World::new();
        let a = mc_child(&mut world, 30.0);
        let b = mc_child(&mut world, 30.0);
        let (ea, eb) = (a.entity, b.entity);
        let packed = pack_columns(&[a, b], 1, 400.0, 0.0, 1000.0);
        let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
        assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
        assert_eq!(pos(eb), Vec2::new(0.0, 30.0));
    }
}
