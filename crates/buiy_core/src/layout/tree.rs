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

#[derive(Default)]
pub struct LayoutTree {
    pub(crate) tree: TaffyTree<()>,
    pub(crate) by_entity: HashMap<Entity, TaffyNodeId>,
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

    /// Test-only access to the entity-to-Taffy mapping. Read-only.
    #[doc(hidden)]
    pub fn by_entity(&self) -> &std::collections::HashMap<bevy::prelude::Entity, taffy::NodeId> {
        &self.by_entity
    }

    /// Test-only access to the inner Taffy tree. Read-only.
    #[doc(hidden)]
    pub fn tree_ref(&self) -> &taffy::TaffyTree<()> {
        &self.tree
    }
}
