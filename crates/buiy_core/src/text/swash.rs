//! The render-world rasterization context (architecture § 1.3).

use bevy::prelude::*;

/// Render-world-only wrapper around `cosmic_text::SwashCache` (verified
/// `Send + Sync`), kept SOLELY for API access to its internal scale context.
///
/// **Uncached-only contract (architecture § 1.3, adjudicated):** Buiy
/// rasterizes exclusively via `SwashCache::get_image_uncached` — the caching
/// path (`get_image`) is never called, `image_cache` stays empty by
/// construction, and the content-addressed, LRU-bounded `BuiyAtlas` is the
/// ONE bitmap cache (gate #15: no second cache, no trim machinery).
///
/// `ResMut` only in the glyph producer's atlas-miss path (T4, lock site #3);
/// it lives outside the `SharedFontSystem` mutex so a main-world shape pass
/// never serializes against the raster cache.
#[derive(Resource)]
pub struct BuiySwashCache(pub cosmic_text::SwashCache);

impl Default for BuiySwashCache {
    fn default() -> Self {
        Self(cosmic_text::SwashCache::new())
    }
}
