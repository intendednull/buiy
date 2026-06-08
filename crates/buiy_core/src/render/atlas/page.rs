//! One backing page: the raw `guillotiere::AtlasAllocator`, its CPU pixel
//! buffer, and the live `AtlasKey -> (AllocId, URect)` map eviction reads.
//! Spec atlas-and-text-seam.md § 2.1.
//!
//! Buiy drives the **raw** allocator (not bevy_image's
//! `DynamicTextureAtlasBuilder`, which allocates internally and discards the
//! `Allocation`, hiding the `AllocId`). Owning the `AllocId` ourselves is
//! what makes LRU eviction (§ 2.4) buildable rather than clear-the-world.
//!
//! ## CPU pixel buffer + the upload seam (design fork #1)
//!
//! The page owns its texels as a plain `Vec<u8>` (`width*height*bytes_per_texel`
//! row-major, the same layout `RenderQueue::write_texture` expects), NOT a Bevy
//! `Handle<Image>`. The GPU-verify design's fork #1 uploads dirty pages with
//! `RenderQueue::write_texture` from a render-world-side `BuiyAtlas`, deliberately
//! avoiding the `Assets<Image>` → `RenderAssets<GpuImage>` auto-extract path (a
//! cross-world handle round-trip + a frame of latency that fights warmup
//! determinism). A plain byte buffer is exactly the "CPU `Image`" the blit + the
//! § 7/§ 4.1 byte-identity test read, with no asset machinery. The GPU `Texture`
//! itself lives in the separate device-owning [`AtlasGpu`] render resource
//! (`atlas/mod.rs`), so `BuiyAtlas` and this page stay device-free and the
//! headless allocator tests need no wgpu adapter.
//!
//! [`AtlasGpu`]: super::AtlasGpu

use std::collections::HashMap;

use bevy::math::{URect, UVec2};
use guillotiere::{AllocId, AtlasAllocator, size2};

use super::{AtlasFormat, AtlasKey};

/// A single atlas page of one format. Square, `size × size` texels.
pub struct AtlasPage {
    /// guillotine/shelf allocator — exposes the `allocate`/`deallocate` pair
    /// eviction needs.
    allocator: AtlasAllocator,
    /// Edge length, texels (uniform across all pages — see `AtlasConfig`).
    size: u32,
    /// The page's format — fixes `bytes_per_texel` for the blit/upload math
    /// (a guillotiere page is one format; the two never share, spec § 2.2).
    format: AtlasFormat,
    /// CPU-side texels, row-major `size*size*bytes_per_texel` bytes. The blit
    /// target and the byte-identity source (spec § 7 / § 4.1). Uploaded to the
    /// GPU `Texture` (in [`AtlasGpu`]) the frames `dirty` is set.
    ///
    /// [`AtlasGpu`]: super::AtlasGpu
    pixels: Vec<u8>,
    /// Set by [`blit`](Self::blit); cleared by [`clear_dirty`](Self::clear_dirty)
    /// after the upload. A page that gained texels this frame re-uploads; an
    /// unchanged page does not (spec § 2.2 dirty-gated upload).
    dirty: bool,
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
    /// Fresh empty page of `size × size` texels in `format`. The CPU pixel
    /// buffer is zeroed (transparent / zero-coverage); it is marked dirty so a
    /// brand-new page's cleared texels upload at least once.
    pub fn new(size: u32, format: AtlasFormat) -> Self {
        let bytes = (size as usize) * (size as usize) * format.bytes_per_texel() as usize;
        Self {
            allocator: AtlasAllocator::new(size2(size as i32, size as i32)),
            size,
            format,
            pixels: vec![0; bytes],
            dirty: true,
            live: HashMap::new(),
        }
    }

    /// Page edge length in texels.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// The page's format.
    pub fn format(&self) -> AtlasFormat {
        self.format
    }

    /// Blit `data` (a `rect.width()*rect.height()*bytes_per_texel` row-major
    /// bitmap, tightly packed) into the page's CPU buffer at `rect`, marking the
    /// page dirty so the next prepare uploads it. Panics if `data` is the wrong
    /// length or `rect` is out of bounds — a producer-side contract violation,
    /// not a recoverable runtime condition. Spec § 2.2.
    pub fn blit(&mut self, rect: URect, data: &[u8]) {
        let bpt = self.format.bytes_per_texel() as usize;
        let (w, h) = (rect.width() as usize, rect.height() as usize);
        assert_eq!(
            data.len(),
            w * h * bpt,
            "blit source length must equal rect area * bytes_per_texel"
        );
        assert!(
            rect.max.x <= self.size && rect.max.y <= self.size,
            "blit rect must stay inside the page"
        );
        let page_row = self.size as usize * bpt;
        let src_row = w * bpt;
        for row in 0..h {
            let dst_y = rect.min.y as usize + row;
            let dst_start = dst_y * page_row + rect.min.x as usize * bpt;
            let src_start = row * src_row;
            self.pixels[dst_start..dst_start + src_row]
                .copy_from_slice(&data[src_start..src_start + src_row]);
        }
        self.dirty = true;
    }

    /// The page's CPU texels (row-major `size*size*bytes_per_texel`). The blit
    /// target and the §7/§4.1 byte-identity source; the upload reads it.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// `true` if the page gained texels since the last [`clear_dirty`](Self::clear_dirty)
    /// — the dirty-gated upload signal (spec § 2.2).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag after the page's texels have been uploaded.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
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

    /// Reset the allocator to empty and clear the live map, **keeping the page
    /// buffer slot** (zeroed and re-marked dirty so the recycled page uploads a
    /// clean state) — the expensive GPU `Texture` (in [`AtlasGpu`]) is reused at
    /// the same page index, not realloc'd (spec § 2.5 pooling). The pixel `Vec`
    /// is zeroed in place, never freed.
    ///
    /// [`AtlasGpu`]: super::AtlasGpu
    pub fn reset(&mut self) {
        self.allocator = AtlasAllocator::new(size2(self.size as i32, self.size as i32));
        self.live.clear();
        self.pixels.fill(0);
        self.dirty = true;
    }
}
