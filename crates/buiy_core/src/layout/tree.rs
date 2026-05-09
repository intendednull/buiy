//! `LayoutTree` — the bridge state between Buiy entities and Taffy.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.1.
//!
//! Stored as a `NonSendResource` because Taffy's `Dimension` packs a
//! `*const ()` regardless of the `calc` feature, so `TaffyTree` is
//! `!Send + !Sync`. Reused frame-to-frame so Taffy's internal cache stays
//! warm. GC handled by `systems::gc_removed_nodes`.

// Entire module is unreachable until Task 7's LayoutPlugin consumes it.
#![allow(dead_code)]

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

    pub(crate) fn taffy_node_for(&self, entity: Entity) -> Option<TaffyNodeId> {
        self.by_entity.get(&entity).copied()
    }
}
