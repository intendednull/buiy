//! The 1×1 solid-white `CoverageR8` stamp (decoration-and-paint § 4.3):
//! one reserved Buiy-internal sentinel `AtlasKey` whose cell, stretched by
//! `GlyphAlphaInstance.rect`, is an over-glyph tinted rectangle with ZERO
//! new pipeline — line-through (T6) and the caret (T7) ride it, emitted
//! after the run's glyphs so they paint over the text (the CSS Text
//! Decoration L3 painting order; quads can never paint over glyphs —
//! § 4.1's fixed rank).
//!
//! Residency: warmup-pinned at plugin startup (`register_render_world`
//! pushes [`solid_stamp_warmup_request`] — the render architecture § 1.1
//! finish-ordering seam delivers a live `RenderApp`), re-inserted on miss
//! like any content-addressed entry (the producer's `get_or_insert`), and
//! TOUCH-MAINTAINED while any stamp instance is live (the key joins
//! `ResidentTextKeys` per instance — glyph-pipeline § 6.3: an idle-but-
//! painted stamp past `eviction_grace` would otherwise lose its cell to
//! reuse and sample someone else's bitmap).

use bevy::math::UVec2;

use crate::render::atlas::{
    AtlasBitmap, AtlasEntry, AtlasEntryKind, AtlasFormat, AtlasKey, AtlasWarmupRequest,
};

/// The sentinel key: `[Mask kind byte, 0]` — the Mask kind is the reserved
/// "sampled exactly like a glyph" coverage kind (types.rs), the leading
/// kind byte makes glyph-key aliasing structurally impossible, and the
/// 2-byte length + sub-id 0 are reserved for THIS stamp (future
/// clip-path/mask-image keys carry longer content-derived payloads).
pub fn solid_stamp_key() -> AtlasKey {
    AtlasKey::from_bytes(&[AtlasEntryKind::Mask.key_byte(), 0])
}

/// The stamp bitmap: one full-coverage texel. Value 255 ⇒ the sampled
/// coverage is exactly 1.0 and the instance tint passes through unchanged
/// (alpha-as-color, § 4.1).
pub fn solid_stamp_bitmap() -> AtlasBitmap {
    AtlasBitmap {
        size: UVec2::ONE,
        format: AtlasFormat::CoverageR8,
        data: vec![255],
    }
}

/// The startup warmup push (§ 4.3 "warmup-pinned"): resident before any
/// first paint, so a first-frame caret/line-through never races a cold
/// atlas (gate #2).
pub fn solid_stamp_warmup_request() -> AtlasWarmupRequest {
    AtlasWarmupRequest {
        key: solid_stamp_key(),
        format: AtlasFormat::CoverageR8,
        bitmap: solid_stamp_bitmap(),
    }
}

/// The instance `uv_rect` for a stamp: the cell MIDPOINT replicated, so the
/// interpolated `atlas_uv` (coverage.wgsl `mix(min, max, v.uv)`) is
/// constant and every fragment samples the center texel — exact under the
/// pinned Nearest sampler or any future filter (T6 decision 9; supersedes
/// § 4.3's bilinear note — erratum 3).
pub fn stamp_uv(entry: &AtlasEntry) -> [f32; 4] {
    let c = entry.uv.center();
    [c.x, c.y, c.x, c.y]
}
