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
//! Steps 2/4/5/6 are empty sub-sets in Phase 1; later phases attach
//! systems to them.

use super::components::{BoxModel, Display, FlexItem, FlexParams, Overflow, Position, Scroll};
use super::translate::{StyleView, style_to_taffy};
use super::tree::LayoutTree;
use crate::components::{Node, ResolvedLayout};
use bevy::prelude::*;
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
/// Phase 2 trigger set: `Changed<BoxModel>`, `Changed<Display>`,
/// `Changed<Position>`, `Changed<FlexParams>`, `Changed<FlexItem>`,
/// `Changed<Overflow>`, `Changed<Scroll>`, `Changed<Children>`,
/// `Changed<ChildOf>`. Phases 4–9 widen it as new components land.
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
            Option<&Children>,
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
                Changed<Children>,
                Changed<ChildOf>,
            )>,
        ),
    >,
) {
    let tree = &mut *tree;

    // Ensure every Buiy entity has a Taffy node + current style. Insert
    // happens for entities new this frame (Changed<T> triggers on insert);
    // existing entities run set_style only when something in the trigger
    // set actually changed — see foundation/architecture.md § 1.2.
    for (entity, display, bm, position, flex, flex_item, overflow, scroll, _children) in
        nodes.iter()
    {
        let view = StyleView {
            display,
            box_model: bm,
            position,
            flex_params: flex,
            flex_item,
            overflow,
            scroll,
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
    for (entity, _display, _bm, _position, _flex, _flex_item, _overflow, _scroll, children) in
        nodes.iter()
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
