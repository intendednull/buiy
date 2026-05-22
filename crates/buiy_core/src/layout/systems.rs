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
    Anchor, BoxModel, Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive,
    Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll, WritingMode,
    WritingModeResolved,
};
use super::translate::{ContainerSnapshot, StyleView, style_to_taffy};
use super::tree::LayoutTree;
use super::types::{AnchorErrorKind, AnchorName, ContainerType, GridAreas, Length, QueryCondition};
use crate::components::{Node, ResolvedLayout};
use bevy::prelude::*;
use std::collections::HashMap;
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

/// Phase 6 — frame-local map of anchor-resolution position overrides.
/// `anchor_resolution` clears this at the top of each call and populates
/// it for every entity with `Anchor.position_anchor.is_some()`. Step 7
/// (`write_resolved_layout`) consults the map per entity and uses the
/// override position (with size still from `tree.tree.layout()`) when
/// present.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2.
#[derive(Resource, Default, Debug)]
pub struct AnchorOverrides {
    pub by_entity: std::collections::HashMap<Entity, Vec2>,
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
        let mut in_degree: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
        for &e in current_edges.keys() {
            in_degree.entry(e).or_insert(0);
        }
        for (_, target) in &current_edges {
            if let Some(t) = target {
                *in_degree.entry(*t).or_insert(0) += 1;
            }
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
#[allow(clippy::type_complexity)]
pub(super) fn sync_styles(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<
        (
            Entity,
            &Display,
            &BoxModel,
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
                // Phase 5 Task 9: container/CQ change set. Nested under
                // a single inner `Or` so the outer tuple stays at 15
                // entries (Bevy 0.18 caps `Or` tuples at 15). The
                // semantics are identical to spelling the four entries
                // at the top level — `Or<(A, Or<(B, C)>)>` matches
                // exactly when `A || B || C`.
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
    mut iter_count: ResMut<SyncStylesIterCount>,
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
        .filter_map(|(entity, _, _, _, _, _, _, _, _, _, _, _, parent)| {
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
        translate_one_entity(
            item,
            &parent_areas_for,
            &container_index,
            &cq_parent_chain,
            viewport_size,
            tree,
        );
    }

    // Sync child relationships for each Buiy entity.
    for (entity, .., children, _parent) in nodes.iter() {
        sync_children_for_entity(entity, children, tree);
    }
}

/// Per-entity tuple emitted by `sync_styles`'s (and `cq_flip_rerun`'s)
/// main filtered query. Aliased so both systems share one tuple shape
/// — change the alias if the filter widens.
#[allow(clippy::type_complexity)]
type NodeQueryItem<'w> = (
    Entity,
    &'w Display,
    &'w BoxModel,
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

/// Per-entity child-sync — second-pass companion to
/// `translate_one_entity`. Taffy's `set_children` requires all child
/// nodes to exist first, so this must run after every entity has been
/// translated. Factored out so `cq_flip_rerun` can re-use it.
fn sync_children_for_entity(entity: Entity, children: Option<&Children>, tree: &mut LayoutTree) {
    let parent_id = match tree.by_entity.get(&entity).copied() {
        Some(id) => id,
        None => return,
    };
    let child_ids: Vec<TaffyNodeId> = children
        .into_iter()
        .flatten()
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
) {
    let mut to_write: Vec<(Entity, ResolvedLayout)> = Vec::new();
    for (&entity, &id) in tree.by_entity.iter() {
        if let Ok(layout) = tree.tree.layout(id) {
            let new = ResolvedLayout {
                position: Vec2::new(layout.location.x, layout.location.y),
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
                // Phase 5 Task 9: same widening as `sync_styles` — kept
                // in sync via the shared `NodeQueryItem` shape. See the
                // sync_styles inline comment for the nested-Or rationale.
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
    roots: Query<(Entity, Option<&ChildOf>), With<Node>>,
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
    for (entity, .., children, _parent) in nodes.iter() {
        sync_children_for_entity(entity, children, tree);
    }

    // Re-invoke Taffy compute. Same code shape as `taffy_compute`,
    // but WITHOUT the `compute_count.0 = 0` frame-reset (that lives
    // only in `taffy_compute`, so a flip frame ends at `count == 2`,
    // not `count == 1`).
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));
    for (entity, parent) in roots.iter() {
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
    fn anchor_overrides_default_empty() {
        let o = AnchorOverrides::default();
        assert!(o.by_entity.is_empty());
    }

    #[test]
    fn layout_anchor_warned_default_empty() {
        let w = LayoutAnchorWarnedThisFrame::default();
        assert!(w.set.is_empty());
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
}

#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::layout::components::Anchor;
    use crate::layout::types::AnchorName;
    use bevy::prelude::*;

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
