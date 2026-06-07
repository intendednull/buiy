//! Seam value types: the opaque content-addressed key, the CPU bitmap
//! handed in on a miss, and the resident-handle entry read back.
//! Spec atlas-and-text-seam.md § 2.1, § 3.

use bevy::math::{Rect, URect, UVec2};
use smallvec::SmallVec;

use super::AtlasFormat;

/// Content-addressed, **opaque to the atlas**. The producer (text) defines
/// what the bytes mean; the atlas treats it as an `Eq + Hash` identity for
/// dedup + eviction. For glyphs, `buiy-text-rendering-design` builds it from
/// `(FontId, subpixel_bucket, glyph_id, px_size)` — that construction is the
/// text spec's concern, not this one. Spec § 3.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AtlasKey(pub SmallVec<[u8; 24]>);

impl AtlasKey {
    /// Build a key from a byte slice (the common producer-side path).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(SmallVec::from_slice(bytes))
    }
}

/// A CPU coverage/color bitmap handed to the atlas on a miss. `R8` for
/// glyph/mask, `Rgba8` for icon/gradient. The atlas wraps it as a Bevy
/// `Image` for the blit and never interprets it. Spec § 3.
pub struct AtlasBitmap {
    pub size: UVec2,
    pub format: AtlasFormat,
    pub data: Vec<u8>,
}

/// The value the seam reads back after an insert (or a `get` probe).
/// Spec § 2.1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtlasEntry {
    /// Index into `pages[format]`.
    pub page: u16,
    pub format: AtlasFormat,
    /// Normalized `[0,1]` UV rect into that page.
    pub uv: Rect,
    /// Pixel rect, for the subpixel-snap math text needs (spec § 4.3).
    pub px: URect,
}

/// Tunable atlas knobs. **Units are pinned here; tuned numbers are deferred**
/// to `buiy-verification-design` (spec § 2.4 "Open — tuned budget numbers").
#[derive(Clone, Copy, Debug)]
pub struct AtlasConfig {
    /// Edge length of each square page, in texels. Spec § 2.2 default: 1024.
    pub page_size: u32,
    /// **Maximum page count** per format (a count of `page_size`² pages, not
    /// a byte figure — pages are uniform-sized so a count *is* the memory
    /// cap). When an allocation would push a format's page set past this,
    /// eviction runs first; only if the LRU queue is exhausted and the entry
    /// still does not fit does a page append exceed the budget (the budget
    /// bounds steady state, never correctness). Spec § 2.4 v1 default: 8.
    pub page_budget: u16,
    /// An entry untouched for this many frames is eviction-eligible even
    /// without pressure, so an idle fixture's transient entries drain back
    /// out (spec § 2.4 step 3 — the clause that makes "return to baseline"
    /// hold). Tuned value deferred.
    pub eviction_grace: u32,
}

impl Default for AtlasConfig {
    fn default() -> Self {
        Self {
            page_size: 1024,
            page_budget: 8,
            eviction_grace: 60,
        }
    }
}

/// What an atlas entry represents. Glyph + Icon ship F-tier (spec § 4);
/// Gradient + Mask are **reserved C-tier** entry *kinds* (spec § 6) — they
/// ride the same atlas, allocator, eviction policy, and bind group, so the
/// deferred shaders add only a producer + a key constructor, never new atlas
/// machinery. **No baking/mask shader in v1.**
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AtlasEntryKind {
    /// Single-channel coverage glyph (F-tier, alpha-as-color, § 4.1).
    Glyph,
    /// Full-color icon / color-emoji bitmap (F-tier, § 4.2).
    Icon,
    /// Baked gradient color strip (C-tier reserved, § 6).
    Gradient,
    /// Generated `clip-path`/`mask-image` coverage (C-tier reserved, § 6) —
    /// sampled exactly like a glyph (a `GlyphAlphaInstance` with a mask key).
    Mask,
}

impl AtlasEntryKind {
    /// The page format this kind lives in (spec § 2.2, § 6). Coverage kinds
    /// are `R8`; color kinds are `Rgba8`.
    pub fn format(self) -> AtlasFormat {
        match self {
            AtlasEntryKind::Glyph | AtlasEntryKind::Mask => AtlasFormat::CoverageR8,
            AtlasEntryKind::Icon | AtlasEntryKind::Gradient => AtlasFormat::ColorRgba8,
        }
    }
}
