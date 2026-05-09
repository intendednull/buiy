//! Layout via Taffy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/visuals.md § 3.2 and
//! architecture.md § 2.3. Phase 0 supports flex row/column with fixed
//! width/height; the full layout surface lives in `buiy-layout-design`.
//!
//! ## Why `NonSend`?
//!
//! Taffy 0.10 packs every `Dimension` into a tagged pointer
//! (`*const ()`), which makes `TaffyTree` `!Send + !Sync` regardless of
//! whether the `calc` feature is in use. Layout is inherently a
//! single-threaded pass over the tree, so storing it as a non-send
//! resource is both correct and free of `unsafe`.

// Phase 1 layout-foundation submodules (dead code until Task 2 wires them in).
// Spec: docs/specs/2026-05-08-buiy-layout-design/.
mod types;

use crate::{
    BuiySet,
    components::{FlexDirection as BuiyFlexDirection, Node, ResolvedLayout, Style},
};
use bevy::prelude::*;
use std::collections::HashMap;
use taffy::{
    AvailableSpace, Dimension, FlexDirection, NodeId as TaffyNodeId, Size, Style as TaffyStyle,
    TaffyTree,
};

/// Maps Bevy `Entity` to Taffy node IDs. Reused across frames to keep
/// Taffy's internal cache warm. Stored as a `NonSend` resource because
/// Taffy's compact-length representation contains a `*const ()`.
#[derive(Default)]
pub struct LayoutTree {
    tree: TaffyTree<()>,
    by_entity: HashMap<Entity, TaffyNodeId>,
}

impl LayoutTree {
    /// Number of entity-to-Taffy-node mappings currently held. Exposed for
    /// tests that need to assert GC actually freed orphan entries.
    pub fn len(&self) -> usize {
        self.by_entity.len()
    }

    /// Whether the tracker holds no entity-to-Taffy-node mappings.
    pub fn is_empty(&self) -> bool {
        self.by_entity.is_empty()
    }
}

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<LayoutTree>().add_systems(
            Update,
            // GC must run before sync so the same tick's despawns don't
            // leave dangling parent/child refs visible to Taffy's
            // set_children call.
            (gc_removed_nodes, sync_and_compute_layout)
                .chain()
                .in_set(BuiySet::Layout),
        );
    }
}

/// Drop Taffy nodes for entities whose `Node` component was removed
/// (despawn or component-remove). Without this, `by_entity` and the
/// underlying `TaffyTree` grow monotonically across despawns.
fn gc_removed_nodes(mut tree: NonSendMut<LayoutTree>, mut removed: RemovedComponents<Node>) {
    let tree = &mut *tree;
    for entity in removed.read() {
        if let Some(id) = tree.by_entity.remove(&entity)
            && let Err(err) = tree.tree.remove(id)
        {
            warn!(?entity, ?err, "buiy: layout gc remove failed");
        }
    }
}

fn style_to_taffy(style: &Style) -> TaffyStyle {
    TaffyStyle {
        size: Size {
            width: if style.width > 0.0 {
                Dimension::length(style.width)
            } else {
                Dimension::auto()
            },
            height: if style.height > 0.0 {
                Dimension::length(style.height)
            } else {
                Dimension::auto()
            },
        },
        flex_direction: match style.flex_direction {
            BuiyFlexDirection::Row => FlexDirection::Row,
            BuiyFlexDirection::Column => FlexDirection::Column,
        },
        ..Default::default()
    }
}

/// One pass: ensure every Buiy entity has a Taffy node, sync style, compute
/// layout starting from roots (entities with `Node` and no Buiy parent),
/// write `ResolvedLayout` back.
#[allow(clippy::type_complexity)]
fn sync_and_compute_layout(
    mut commands: Commands,
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<(Entity, &Style, Option<&ChildOf>, Option<&Children>), With<Node>>,
    windows: Query<&bevy::window::Window>,
) {
    let tree = &mut *tree;

    // Ensure every Buiy entity has a Taffy node + current style.
    for (entity, style, _parent, _children) in nodes.iter() {
        let taffy_style = style_to_taffy(style);
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
    for (entity, _style, _parent, children) in nodes.iter() {
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

    // Compute layout for roots (entities with Node and no Buiy parent).
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));

    for (entity, _style, parent, _children) in nodes.iter() {
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

    // Walk the tree and write ResolvedLayout for every entity.
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
