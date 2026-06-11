//! The structured glyph `AtlasKey` (glyph-pipeline § 4): fixed-layout
//! little-endian bytes built from the verified `CacheKey` fields plus the
//! `AtlasEntryKind::Glyph` discriminant, with `fontdb::ID` interned to a
//! stable `u32` (the id's repr is private and version-fragile; the interner
//! costs one HashMap lookup and survives fontdb upgrades — § 4's rejected
//! runner-up (b)). Content addressing requires EQUALITY, not hashing —
//! a hashed-u64 key (rejected runner-up (a)) would silently alias two
//! glyphs' coverage on collision.
//!
//! cosmic-text types stay on THIS side of the seam: the render atlas only
//! ever sees the opaque byte key (atlas/mod.rs seam doc).

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use bevy::prelude::Resource;
use cosmic_text::{CacheKey, SubpixelBin, fontdb};

use crate::render::atlas::{AtlasEntryKind, AtlasKey};

/// Exact byte length of a structured glyph key:
/// `[kind u8][font u32][glyph_id u16][font_size_bits u32][x_bin u8][y_bin u8][weight u16][flags u32]`.
pub const GLYPH_KEY_LEN: usize = 19;

/// Render-world interner: `fontdb::ID` → stable `u32` seat (glyph-pipeline
/// § 4). Seats are MONOTONIC for the life of the resource — never reused —
/// because old atlas entries keyed under old seats stay grace-resident
/// after any font-set change; a reused seat would alias them (stale-bitmap
/// corruption, the § 3.2 hazard). The ID map is valid only within one
/// database lineage (fontdb IDs are slotmap keys — equal VALUES name
/// different faces across databases), so `begin_lineage` clears it whenever
/// `FontDbLineage` advances; `next` survives the clear.
#[derive(Resource, Default)]
pub struct FontKeyInterner {
    ids: HashMap<fontdb::ID, u32>,
    next: u32,
    lineage: u64,
}

impl FontKeyInterner {
    /// The stable `u32` seat for `font` — allocated on first sight within
    /// the current lineage, identical until the next lineage change.
    pub fn intern(&mut self, font: fontdb::ID) -> u32 {
        match self.ids.entry(font) {
            Entry::Occupied(seat) => *seat.get(),
            Entry::Vacant(slot) => {
                let seat = self.next;
                self.next += 1;
                *slot.insert(seat)
            }
        }
    }

    /// Synchronize with the main world's `FontDbLineage`: on a change,
    /// clear the ID map (every fontdb ID was reissued — the § 3.2 storm)
    /// while keeping `next` monotonic. Returns true when a clear happened.
    pub fn begin_lineage(&mut self, lineage: u64) -> bool {
        if self.lineage == lineage {
            return false;
        }
        self.ids.clear();
        self.lineage = lineage;
        true
    }

    /// Number of fonts interned in the current lineage.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when no font has been interned in the current lineage.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// `SubpixelBin` → stable byte. Explicit match — the upstream enum carries
/// no guaranteed discriminants.
fn bin_byte(bin: SubpixelBin) -> u8 {
    match bin {
        SubpixelBin::Zero => 0,
        SubpixelBin::One => 1,
        SubpixelBin::Two => 2,
        SubpixelBin::Three => 3,
    }
}

/// Build the structured glyph `AtlasKey` from a quantized `CacheKey`
/// (glyph-pipeline § 4). 19 B — fits `AtlasKey`'s `SmallVec<[u8; 24]>`
/// inline capacity, so the hot path never heap-allocates. `weight` and
/// `flags` are in the key because both are shape-affecting `CacheKey`
/// inputs; `y_bin` is carried even though § 5.1 makes it structurally zero
/// (one byte buys layout stability if vertical binning ever changes).
pub fn glyph_atlas_key(cache_key: &CacheKey, interner: &mut FontKeyInterner) -> AtlasKey {
    let mut bytes = [0u8; GLYPH_KEY_LEN];
    bytes[0] = AtlasEntryKind::Glyph.key_byte();
    bytes[1..5].copy_from_slice(&interner.intern(cache_key.font_id).to_le_bytes());
    bytes[5..7].copy_from_slice(&cache_key.glyph_id.to_le_bytes());
    bytes[7..11].copy_from_slice(&cache_key.font_size_bits.to_le_bytes());
    bytes[11] = bin_byte(cache_key.x_bin);
    bytes[12] = bin_byte(cache_key.y_bin);
    bytes[13..15].copy_from_slice(&cache_key.font_weight.0.to_le_bytes());
    bytes[15..19].copy_from_slice(&cache_key.flags.bits().to_le_bytes());
    AtlasKey::from_bytes(&bytes)
}
