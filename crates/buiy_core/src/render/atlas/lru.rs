//! LRU recency ring keyed by `AtlasKey`, with per-key last-touched frame.
//! Drives eviction order (spec § 2.4 step 1–2) and the `eviction_grace`
//! idle-drain clause (step 3). A `VecDeque` ordered LRU→MRU plus a frame map;
//! atlas entry counts are small, so the O(n) `touch`-reorder is fine and
//! keeps the structure trivially correct.

use std::collections::VecDeque;

use super::AtlasKey;

/// LRU→MRU recency ring. Front is least-recently-used.
#[derive(Default)]
pub struct LruQueue {
    order: VecDeque<AtlasKey>,
    /// Per-key frame index of the most recent touch.
    last_touched: std::collections::HashMap<AtlasKey, u64>,
}

impl LruQueue {
    /// Mark `key` most-recently-used at `frame`. Idempotent on membership:
    /// re-touching moves the existing entry, never duplicates it.
    pub fn touch(&mut self, key: AtlasKey, frame: u64) {
        if let Some(pos) = self.order.iter().position(|k| *k == key) {
            self.order.remove(pos);
        }
        self.last_touched.insert(key.clone(), frame);
        self.order.push_back(key);
    }

    /// Remove and return the least-recently-used key, if any.
    pub fn pop_lru(&mut self) -> Option<AtlasKey> {
        let key = self.order.pop_front()?;
        self.last_touched.remove(&key);
        Some(key)
    }

    /// Drop a specific key (e.g. when evicted under grace).
    pub fn remove(&mut self, key: &AtlasKey) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.last_touched.remove(key);
    }

    /// Keys untouched for more than `grace` frames as of `now` (spec § 2.4
    /// step 3). Order is unspecified; callers evict all of them.
    pub fn grace_expired(&self, now: u64, grace: u32) -> Vec<AtlasKey> {
        self.last_touched
            .iter()
            .filter(|&(_, &t)| now.saturating_sub(t) > grace as u64)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Iterate keys least-recently-used first.
    pub fn iter_lru_to_mru(&self) -> impl Iterator<Item = &AtlasKey> {
        self.order.iter()
    }

    /// Number of tracked entries.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// No tracked entries.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}
