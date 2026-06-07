//! One backing page: the raw `guillotiere::AtlasAllocator`, its CPU `Image`
//! handle, and the live `AtlasKey -> (AllocId, URect)` map eviction reads.
//! Spec atlas-and-text-seam.md § 2.1.
//!
//! Buiy drives the **raw** allocator (not bevy_image's
//! `DynamicTextureAtlasBuilder`, which allocates internally and discards the
//! `Allocation`, hiding the `AllocId`). Owning the `AllocId` ourselves is
//! what makes LRU eviction (§ 2.4) buildable rather than clear-the-world.

use std::collections::HashMap;

use bevy::math::{URect, UVec2};
use bevy::prelude::{Handle, Image};
use guillotiere::{AllocId, AtlasAllocator, size2};

use super::AtlasKey;

/// A single atlas page of one format. Square, `size × size` texels.
pub struct AtlasPage {
    /// guillotine/shelf allocator — exposes the `allocate`/`deallocate` pair
    /// eviction needs.
    allocator: AtlasAllocator,
    /// Edge length, texels (uniform across all pages — see `AtlasConfig`).
    size: u32,
    /// CPU-side `Image`; its `GpuImage` is uploaded the frames it changes.
    /// `None` until the GPU-side wiring lands; the headless allocator logic
    /// never touches it. Read via `texture()` (pooling retains the slot).
    texture: Option<Handle<Image>>,
    /// guillotiere `AllocId` + owned pixel rect per resident key, so eviction
    /// can `deallocate(id)` and free the rect. Spec § 2.1 `live`.
    live: HashMap<AtlasKey, (AllocId, URect)>,
}

/// Convert a guillotiere `Rectangle` (`euclid Box2D<i32>`) to a Bevy `URect`.
/// guillotiere never returns negative coordinates for in-bounds allocations,
/// so the `as u32` casts are lossless.
fn rect_to_urect(r: guillotiere::Rectangle) -> URect {
    URect::new(
        r.min.x as u32,
        r.min.y as u32,
        r.max.x as u32,
        r.max.y as u32,
    )
}

impl AtlasPage {
    /// Fresh empty page of `size × size` texels.
    pub fn new(size: u32) -> Self {
        Self {
            allocator: AtlasAllocator::new(size2(size as i32, size as i32)),
            size,
            texture: None,
            live: HashMap::new(),
        }
    }

    /// Page edge length in texels.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Try to allocate a `req`-sized cell; `None` if it does not fit.
    /// Returns only the rect (no `AllocId`) — for callers that do not need
    /// to free it (tests, fit probes).
    pub fn try_alloc(&mut self, req: UVec2) -> Option<URect> {
        self.alloc(req).map(|(_, r)| r)
    }

    /// Try to allocate, returning just the `AllocId` for later `free`.
    pub fn alloc_id(&mut self, req: UVec2) -> Option<AllocId> {
        self.alloc(req).map(|(id, _)| id)
    }

    /// Core allocate: returns the `(AllocId, URect)` pair on success.
    pub fn alloc(&mut self, req: UVec2) -> Option<(AllocId, URect)> {
        let alloc = self.allocator.allocate(size2(req.x as i32, req.y as i32))?;
        Some((alloc.id, rect_to_urect(alloc.rectangle)))
    }

    /// Free a previously-allocated cell, coalescing its space.
    pub fn free(&mut self, id: AllocId) {
        self.allocator.deallocate(id);
    }

    /// No live allocations remain (eligible for the page pool, Task 7).
    pub fn is_empty(&self) -> bool {
        self.allocator.is_empty()
    }

    /// Record a resident cell so eviction can later free it.
    pub fn insert_live(&mut self, key: AtlasKey, id: AllocId, rect: URect) {
        self.live.insert(key, (id, rect));
    }

    /// The `(AllocId, URect)` of a resident key, if present.
    pub fn live_of(&self, key: &AtlasKey) -> Option<(AllocId, URect)> {
        self.live.get(key).copied()
    }

    /// Remove a resident cell from the live map (after `free`).
    pub fn remove_live(&mut self, key: &AtlasKey) {
        self.live.remove(key);
    }

    /// Number of resident cells (for tests / baseline assertions).
    pub fn live_len(&self) -> usize {
        self.live.len()
    }

    /// Reset the allocator to empty and clear the live map, **keeping the
    /// texture handle** — the expensive GPU object is reused, not realloc'd
    /// (spec § 2.5 pooling).
    pub fn reset(&mut self) {
        self.allocator = AtlasAllocator::new(size2(self.size as i32, self.size as i32));
        self.live.clear();
    }

    /// The page's texture handle (pooling reuses this slot rather than
    /// dropping it). `None` until GPU wiring lands.
    pub fn texture(&self) -> Option<&Handle<Image>> {
        self.texture.as_ref()
    }
}
