//! `LayoutTree` — the bridge state between Buiy entities and Taffy.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.1.
//!
//! Stored as a `NonSendResource` because Taffy's `Dimension` packs a
//! `*const ()` regardless of the `calc` feature, so `TaffyTree` is
//! `!Send + !Sync`. Reused frame-to-frame so Taffy's internal cache stays
//! warm. GC handled by `systems::gc_removed_nodes`.

use bevy::prelude::Entity;
use std::collections::HashMap;
use taffy::{NodeId as TaffyNodeId, TaffyTree};

pub struct LayoutTree {
    /// `TaffyTree<Entity>` (text measure § 2.1): text leaves register
    /// their entity as the node context; the measure closure receives
    /// `Option<&mut Entity>` straight from Taffy's leaf dispatch and
    /// resolves it against ECS queries. Non-text nodes carry no context.
    pub(crate) tree: TaffyTree<Entity>,
    pub(crate) by_entity: HashMap<Entity, TaffyNodeId>,
}

// Manual impl: taffy implements `Default` only for `TaffyTree<()>`, but
// `TaffyTree::<NodeContext>::new()` is generic (taffy_tree.rs:541).
impl Default for LayoutTree {
    fn default() -> Self {
        Self {
            tree: TaffyTree::new(),
            by_entity: HashMap::new(),
        }
    }
}

impl LayoutTree {
    pub fn len(&self) -> usize {
        self.by_entity.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_entity.is_empty()
    }

    /// Mark the Taffy node for `entity` dirty (recursive to ancestors —
    /// taffy_tree.rs:873). Taffy caches measure results; an un-dirtied node
    /// serves a stale measurement (text architecture § 4.1). No-op when the
    /// entity has no node yet — a brand-new text leaf's node is created
    /// fresh by `sync_styles` later the same frame, dirty by construction
    /// (text measure-and-layout § 2.2).
    pub(crate) fn mark_dirty_for_entity(&mut self, entity: Entity) {
        if let Some(&node) = self.by_entity.get(&entity) {
            self.tree
                .mark_dirty(node)
                .expect("LayoutTree: by_entity points at a live Taffy node");
        }
    }

    /// Register `entity` as its own node's measure context (text measure
    /// § 2.2). EDGE-TRIGGERED ONLY — `set_node_context` calls `mark_dirty`
    /// internally (taffy_tree.rs:656), so a per-frame call would silently
    /// kill the O(0) steady state (the text_sync dirty-probe test is the
    /// tripwire). No-op when the entity has no node yet: a brand-new text
    /// leaf's node is created WITH its context by `translate_one_entity`
    /// later the same frame.
    pub(crate) fn set_text_context(&mut self, entity: Entity) {
        if let Some(&node) = self.by_entity.get(&entity) {
            self.tree
                .set_node_context(node, Some(entity))
                .expect("LayoutTree: by_entity points at a live Taffy node");
        }
    }

    /// Unregister on the Text-removal edge. The internal `mark_dirty` is
    /// load-bearing here: the now-plain leaf must re-measure as zero.
    pub(crate) fn clear_text_context(&mut self, entity: Entity) {
        if let Some(&node) = self.by_entity.get(&entity) {
            self.tree
                .set_node_context(node, None)
                .expect("LayoutTree: by_entity points at a live Taffy node");
        }
    }

    /// Test-only access to the entity-to-Taffy mapping. Read-only.
    #[doc(hidden)]
    pub fn by_entity(&self) -> &std::collections::HashMap<bevy::prelude::Entity, taffy::NodeId> {
        &self.by_entity
    }

    /// Test-only access to the inner Taffy tree. Read-only.
    #[doc(hidden)]
    pub fn tree_ref(&self) -> &taffy::TaffyTree<bevy::prelude::Entity> {
        &self.tree
    }
}
