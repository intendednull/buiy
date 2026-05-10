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
    BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
    WritingMode, WritingModeResolved,
};
use super::translate::{StyleView, style_to_taffy};
use super::tree::LayoutTree;
use super::types::GridAreas;
use crate::components::{Node, ResolvedLayout};
use bevy::prelude::*;
use std::collections::HashMap;
use taffy::{AvailableSpace, NodeId as TaffyNodeId, Size};

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
/// `Changed<Children>`, `Changed<ChildOf>`. Phases 5–9 widen it as new
/// components land. `Changed<ChildOf>` is included so that re-parenting a
/// grid item under a different grid container picks up the new container's
/// `template_areas`. `Changed<WritingMode>` triggers when an author edits
/// the entity's own writing mode; `Changed<WritingModeResolved>` triggers
/// after `inherit_writing_mode` (pre-step-1) re-derives the resolved cache
/// for an entity whose effective writing mode actually changed (the
/// inherit system is careful to skip writes when the value is unchanged,
/// preserving the O(0) steady-state contract).
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
            )>,
        ),
    >,
    parent_grid_lookup: Query<&GridParams>,
) {
    let tree = &mut *tree;

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

    // Ensure every Buiy entity has a Taffy node + current style. Insert
    // happens for entities new this frame (Changed<T> triggers on insert);
    // existing entities run set_style only when something in the trigger
    // set actually changed — see foundation/architecture.md § 1.2.
    for (
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
    ) in nodes.iter()
    {
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

    // Sync child relationships for each Buiy entity.
    for (
        entity,
        _display,
        _bm,
        _position,
        _flex,
        _flex_item,
        _overflow,
        _scroll,
        _grid_params,
        _grid_item,
        _writing_mode_resolved,
        children,
        _parent,
    ) in nodes.iter()
    {
        let parent_id = match tree.by_entity.get(&entity).copied() {
            Some(id) => id,
            None => continue,
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
}

/// Step 3 — call `tree.compute_layout` from each root. A root is an
/// entity with `Node` and either no `ChildOf`, or a `ChildOf` whose
/// target is not in `LayoutTree` (i.e., a non-Buiy parent).
pub(super) fn taffy_compute(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<(Entity, Option<&ChildOf>), With<Node>>,
    windows: Query<&bevy::window::Window>,
) {
    let tree = &mut *tree;

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
        if let Some(id) = tree.by_entity.get(&entity).copied()
            && let Err(err) = tree.tree.compute_layout(
                id,
                Size {
                    width: AvailableSpace::Definite(window_size.x),
                    height: AvailableSpace::Definite(window_size.y),
                },
            )
        {
            warn!(?entity, ?err, "buiy: layout compute_layout failed");
        }
    }
}

/// Step 7 — read `tree.layout(id)` for every tracked entity and write
/// the resulting position+size into `ResolvedLayout`. On Taffy `Err`,
/// retain the previous frame's value.
pub(super) fn write_resolved_layout(mut commands: Commands, tree: NonSend<LayoutTree>) {
    let mut to_write: Vec<(Entity, ResolvedLayout)> = Vec::new();
    for (&entity, &id) in tree.by_entity.iter() {
        if let Ok(layout) = tree.tree.layout(id) {
            to_write.push((
                entity,
                ResolvedLayout {
                    position: Vec2::new(layout.location.x, layout.location.y),
                    size: Vec2::new(layout.size.width, layout.size.height),
                },
            ));
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
fn resolve_writing_mode(
    entity: Entity,
    memo: &mut HashMap<Entity, WritingMode>,
    wm_lookup: &Query<&WritingMode>,
    parent_chain: &Query<&ChildOf>,
) -> WritingMode {
    if let Some(cached) = memo.get(&entity) {
        return *cached;
    }
    let effective = if let Ok(wm) = wm_lookup.get(entity) {
        *wm
    } else if let Ok(p) = parent_chain.get(entity) {
        resolve_writing_mode(p.parent(), memo, wm_lookup, parent_chain)
    } else {
        WritingMode::default()
    };
    memo.insert(entity, effective);
    effective
}
