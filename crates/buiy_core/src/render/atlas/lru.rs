//! LRU recency map keyed by `AtlasKey`, with per-key last-touched frame.
//! Drives eviction order (spec § 2.4 step 1–2) and the `eviction_grace`
//! idle-drain clause (step 3).
//!
//! ## Recency is recorded, order is derived (perf audit #5)
//!
//! Every primitive that samples a glyph calls `touch` once **per frame**, so on
//! a static text screen `touch` runs once per *visible glyph* — the dominant
//! cost of an idle text frame (perf-profiled at ~50–60% of the steady frame).
//! It must be O(1). So this stores only a `HashMap<AtlasKey, (frame, seq)>`:
//! `frame` is the last-touched frame (drives grace expiry); `seq` is a global
//! monotonic touch counter that gives a **total, deterministic recency order**.
//! `touch` is a single hash lookup + write (no scan, no clone on the resident
//! path). LRU order — needed only by eviction-victim selection — is derived
//! lazily by sorting on `seq`, off the hot path. The previous `VecDeque`
//! ring made `touch` O(entries) (a linear `position` scan + `remove` memmove +
//! key clone) — O(visible glyphs × entries) per frame.

use std::collections::HashMap;

use super::AtlasKey;

/// Recency map. `seq` orders touches; front of derived order is least-recently-used.
#[derive(Default)]
pub struct LruQueue {
    /// Per-key `(last-touched frame, monotonic touch sequence)`. Frame drives
    /// grace expiry; seq gives a deterministic LRU order without a ring.
    touched: HashMap<AtlasKey, (u64, u64)>,
    /// Monotonic touch counter. Incremented on every `touch`; the largest seq
    /// is most-recently-used.
    seq: u64,
}

impl LruQueue {
    /// Mark `key` most-recently-used at `frame`. **O(1)** — a single hash
    /// lookup + write on the resident path (no scan, no clone). Idempotent on
    /// membership: re-touching updates the stamp, never duplicates.
    pub fn touch(&mut self, key: &AtlasKey, frame: u64) {
        let s = self.seq;
        self.seq = self.seq.wrapping_add(1);
        if let Some(v) = self.touched.get_mut(key) {
            *v = (frame, s);
        } else {
            self.touched.insert(key.clone(), (frame, s));
        }
    }

    /// Remove and return the least-recently-used key, if any. O(n) — eviction
    /// path only, not the per-frame hot path.
    pub fn pop_lru(&mut self) -> Option<AtlasKey> {
        let key = self
            .touched
            .iter()
            .min_by_key(|entry| entry.1.1)
            .map(|(k, _)| k.clone())?;
        self.touched.remove(&key);
        Some(key)
    }

    /// Drop a specific key (e.g. when evicted under grace).
    pub fn remove(&mut self, key: &AtlasKey) {
        self.touched.remove(key);
    }

    /// Keys untouched for more than `grace` frames as of `now` (spec § 2.4
    /// step 3). Order is unspecified; callers evict all of them.
    pub fn grace_expired(&self, now: u64, grace: u32) -> Vec<AtlasKey> {
        self.touched
            .iter()
            .filter(|&(_, &(t, _))| now.saturating_sub(t) > grace as u64)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Iterate keys least-recently-used first. Derived lazily by sorting on the
    /// monotonic touch sequence; only eviction-victim selection calls this, so
    /// the O(n log n) is off the per-frame hot path.
    pub fn iter_lru_to_mru(&self) -> impl Iterator<Item = &AtlasKey> {
        let mut v: Vec<(&AtlasKey, u64)> = self.touched.iter().map(|(k, &(_, s))| (k, s)).collect();
        v.sort_by_key(|&(_, s)| s);
        v.into_iter().map(|(k, _)| k)
    }

    /// Number of tracked entries.
    pub fn len(&self) -> usize {
        self.touched.len()
    }

    /// No tracked entries.
    pub fn is_empty(&self) -> bool {
        self.touched.is_empty()
    }
}
