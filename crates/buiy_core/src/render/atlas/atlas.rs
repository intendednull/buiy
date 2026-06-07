//! The one render-world `BuiyAtlas` resource. Owns the per-format page
//! lists, the content-addressed `entries` map, the LRU ring, and the config.
//! Spec atlas-and-text-seam.md § 2–§ 3.
//!
//! Atlas mutation is single-threaded **by design**: there is exactly one
//! `BuiyAtlas`, so every insert/evict serializes through one `ResMut`. This
//! is a performance, not a correctness, coupling — entries are
//! content-addressed, so a producer that loses a frame's mutation simply
//! re-inserts on the next miss (spec § 2.1).

use std::collections::HashMap;

use bevy::math::Rect;
use bevy::prelude::Resource;

use super::{AtlasBitmap, AtlasConfig, AtlasEntry, AtlasFormat, AtlasKey, AtlasPage, LruQueue};

/// One render-world resource shared by every coverage-and-image primitive.
#[derive(Resource)]
pub struct BuiyAtlas {
    /// One backing-page list per format. Distinct formats never share a page.
    pages: HashMap<AtlasFormat, Vec<AtlasPage>>,
    /// Key -> where it lives. The seam's only handle (spec § 3).
    entries: HashMap<AtlasKey, AtlasEntry>,
    /// LRU recency ring; evicted oldest-first under pressure (spec § 2.4).
    lru: LruQueue,
    /// Emptied pages, reset and held for reuse instead of dropped (spec
    /// § 2.5). Keyed by format because formats never share a page.
    pooled: HashMap<AtlasFormat, Vec<AtlasPage>>,
    config: AtlasConfig,
    /// Monotonic frame counter advanced by `begin_frame` (Task 9). Drives
    /// LRU touch timestamps + grace expiry.
    frame: u64,
}

impl BuiyAtlas {
    /// Empty atlas with the given config.
    pub fn new(config: AtlasConfig) -> Self {
        Self {
            pages: HashMap::new(),
            entries: HashMap::new(),
            lru: LruQueue::default(),
            pooled: HashMap::new(),
            config,
            frame: 0,
        }
    }

    /// Residency probe; does **not** touch LRU (spec § 3 `get`).
    pub fn get(&self, key: &AtlasKey) -> Option<AtlasEntry> {
        self.entries.get(key).copied()
    }

    /// Idempotent insert (spec § 3 `get_or_insert`). On a hit, touch LRU and
    /// return the resident entry — the closure is **not** called, so no
    /// rasterize and no blit. On a miss, allocate (Task 7 adds eviction
    /// under pressure), record the entry + live cell, and return it.
    pub fn get_or_insert(
        &mut self,
        key: AtlasKey,
        format: AtlasFormat,
        coverage: impl FnOnce() -> AtlasBitmap,
    ) -> AtlasEntry {
        if let Some(entry) = self.entries.get(&key).copied() {
            self.lru.touch(key, self.frame);
            return entry;
        }
        let bitmap = coverage();
        debug_assert_eq!(
            bitmap.format, format,
            "bitmap format must match the key's format"
        );
        let entry = self.allocate_and_record(key.clone(), format, bitmap);
        self.lru.touch(key, self.frame);
        entry
    }

    /// Drain a warmup queue: force every requested entry resident (idempotent
    /// — a request whose key is already resident is a no-op insert). Spec
    /// § 2.3. The in-app `warmup_atlas` system calls this pre-paint.
    pub fn drain_warmup(&mut self, queue: &mut super::AtlasWarmupQueue) {
        for req in queue.take() {
            let bitmap = req.bitmap;
            self.get_or_insert(req.key, req.format, move || bitmap);
        }
    }

    /// Advance the atlas frame counter (call once per render frame before
    /// inserts). Drives LRU timestamps + grace expiry.
    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Touch an already-resident key, moving it to most-recently-used.
    /// (Each primitive that samples an entry calls this per frame — spec
    /// § 2.4 step 1. Exposed for tests as `touch_existing`.)
    pub fn touch_existing(&mut self, key: &AtlasKey) {
        if self.entries.contains_key(key) {
            self.lru.touch(key.clone(), self.frame);
        }
    }

    /// Evict every entry untouched for more than `eviction_grace` frames
    /// (spec § 2.4 step 3). The clause that makes "return to baseline" hold.
    pub fn drain_grace_expired(&mut self) {
        let grace = self.config.eviction_grace;
        for key in self.lru.grace_expired(self.frame, grace) {
            self.evict_entry(&key);
        }
    }

    /// Free one entry everywhere: page allocator, live map, entries, LRU.
    fn evict_entry(&mut self, key: &AtlasKey) {
        let Some(entry) = self.entries.remove(key) else {
            return;
        };
        if let Some(list) = self.pages.get_mut(&entry.format)
            && let Some(page) = list.get_mut(entry.page as usize)
            && let Some((id, _)) = page.live_of(key)
        {
            page.free(id);
            page.remove_live(key);
        }
        self.lru.remove(key);
    }

    /// The least-recently-used resident key of `format`, if any. Walks LRU
    /// order front-to-back and returns the first entry of the right format.
    fn next_lru_of_format(&self, format: AtlasFormat) -> Option<AtlasKey> {
        self.lru
            .iter_lru_to_mru()
            .find(|k| self.entries.get(*k).map(|e| e.format) == Some(format))
            .cloned()
    }

    /// Allocate the bitmap on the format's page set, appending a page if the
    /// existing ones are full, and record the entry + live cell. (Eviction
    /// under page-budget pressure is layered on in Task 7.)
    fn allocate_and_record(
        &mut self,
        key: AtlasKey,
        format: AtlasFormat,
        bitmap: AtlasBitmap,
    ) -> AtlasEntry {
        let page_size = self.config.page_size;
        let req = bitmap.size;

        // Try existing pages, oldest-first. Scoped so the `pages` borrow ends
        // before the eviction loop, which needs `&mut self` for eviction.
        {
            let list = self.pages.entry(format).or_default();
            for (idx, page) in list.iter_mut().enumerate() {
                if let Some((id, px)) = page.alloc(req) {
                    page.insert_live(key.clone(), id, px);
                    let entry = entry_from(idx as u16, format, px, page_size);
                    self.entries.insert(key, entry);
                    return entry;
                }
            }
        }

        // No existing page fit. If the format's page set is at budget, evict
        // LRU entries (of this format) until either a page fits the request
        // or the LRU queue is exhausted; only then append a fresh page
        // (exceeding budget rather than failing — budget bounds steady
        // state, never correctness). Spec § 2.4 step 2.
        loop {
            let list = self.pages.entry(format).or_default();
            if (list.len() as u16) < self.config.page_budget {
                break; // under budget: appending a page is allowed.
            }
            // At budget: try to free room by evicting the LRU entry of this
            // format. If none can be evicted, fall through to append.
            let Some(victim) = self.next_lru_of_format(format) else {
                break;
            };
            self.evict_entry(&victim);
            // Retry existing pages now that a cell freed.
            let list = self.pages.entry(format).or_default();
            for (idx, page) in list.iter_mut().enumerate() {
                if let Some((id, px)) = page.alloc(req) {
                    page.insert_live(key.clone(), id, px);
                    let entry = entry_from(idx as u16, format, px, page_size);
                    self.entries.insert(key, entry);
                    return entry;
                }
            }
        }

        // Append a page (under budget, or budget exceeded because the LRU was
        // exhausted and the entry still did not fit). Reuse a pooled (emptied)
        // page if one exists — it is already `reset`, so empty and ready — else
        // allocate fresh (spec § 2.5 pooling).
        let mut page = self
            .pooled
            .get_mut(&format)
            .and_then(|pool| pool.pop())
            .unwrap_or_else(|| AtlasPage::new(page_size));
        let (id, px) = page
            .alloc(req)
            .expect("a fresh page must fit a sub-page-sized request");
        page.insert_live(key.clone(), id, px);
        let list = self.pages.entry(format).or_default();
        let idx = list.len();
        list.push(page);
        let entry = entry_from(idx as u16, format, px, page_size);
        self.entries.insert(key, entry);
        entry
    }

    /// Number of resident entries (baseline assertions, gate #15 headless).
    pub fn live_entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Number of pages for a format (the lever gate #15 watches, spec § 2.2).
    pub fn page_count(&self, format: AtlasFormat) -> usize {
        self.pages.get(&format).map(|p| p.len()).unwrap_or(0)
    }

    /// After eviction, move every now-empty page into the per-format pool
    /// (reset, texture handle retained). Call once per frame after drains.
    pub fn collect_emptied_pages(&mut self) {
        for (format, list) in self.pages.iter_mut() {
            // Only pool *trailing* empty pages: popping from the end never
            // shifts a surviving entry's page index. Mid-list compaction
            // (with entry reindex) is a v1 follow-up. Spec § 2.5.
            while let Some(last) = list.last() {
                if last.live_len() == 0 && last.is_empty() {
                    let mut page = list.pop().expect("checked last");
                    page.reset();
                    self.pooled.entry(*format).or_default().push(page);
                } else {
                    break;
                }
            }
        }
    }

    /// Number of pooled (recyclable) pages for a format.
    pub fn pooled_page_count(&self, format: AtlasFormat) -> usize {
        self.pooled.get(&format).map(|p| p.len()).unwrap_or(0)
    }

    /// Test seam: evict a specific key (mirrors the per-frame eviction path).
    pub fn evict_for_test(&mut self, key: &AtlasKey) {
        self.evict_entry(key);
    }
}

/// Build an `AtlasEntry` from a placed pixel rect + page geometry.
fn entry_from(page: u16, format: AtlasFormat, px: bevy::math::URect, page_size: u32) -> AtlasEntry {
    let inv = 1.0 / page_size as f32;
    let uv = Rect {
        min: bevy::math::Vec2::new(px.min.x as f32 * inv, px.min.y as f32 * inv),
        max: bevy::math::Vec2::new(px.max.x as f32 * inv, px.max.y as f32 * inv),
    };
    AtlasEntry {
        page,
        format,
        uv,
        px,
    }
}
