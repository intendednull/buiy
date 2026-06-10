//! The structured glyph `AtlasKey` scheme (glyph-pipeline § 4): 19 B
//! fixed-layout little-endian bytes from the verified `CacheKey` fields,
//! kind-partitioned, font-id interned. Headless — no adapter anywhere.

use buiy_core::render::atlas::AtlasEntryKind;
use buiy_core::text::{FontKeyInterner, GLYPH_KEY_LEN, glyph_atlas_key, registered_fonts_db};
use cosmic_text::{CacheKey, CacheKeyFlags, SubpixelBin, fontdb};
use std::collections::HashSet;

/// One face id from the embedded deterministic db (default_font is a
/// default-on feature, so the db always has exactly one face here).
fn embedded_face() -> fontdb::ID {
    registered_fonts_db()
        .faces()
        .next()
        .expect("the embedded default font is registered")
        .id
}

fn key_with(font_id: fontdb::ID, glyph_id: u16, size: f32, x_bin: SubpixelBin) -> CacheKey {
    CacheKey {
        font_id,
        glyph_id,
        font_size_bits: size.to_bits(),
        x_bin,
        y_bin: SubpixelBin::Zero,
        font_weight: fontdb::Weight(400),
        flags: CacheKeyFlags::empty(),
    }
}

#[test]
fn kind_bytes_partition_the_key_space() {
    // The leading byte is the producer partition (§ 4): four kinds, four
    // distinct stable bytes. Renumbering is a cache-invalidation bug.
    let bytes = [
        AtlasEntryKind::Glyph.key_byte(),
        AtlasEntryKind::Icon.key_byte(),
        AtlasEntryKind::Gradient.key_byte(),
        AtlasEntryKind::Mask.key_byte(),
    ];
    assert_eq!(bytes.iter().collect::<HashSet<_>>().len(), 4);
    assert_eq!(AtlasEntryKind::Glyph.key_byte(), 0, "glyph byte is pinned");
}

#[test]
fn glyph_key_is_19_bytes_inline_and_byte_exact() {
    let mut interner = FontKeyInterner::default();
    let key = glyph_atlas_key(
        &key_with(embedded_face(), 7, 20.0, SubpixelBin::One),
        &mut interner,
    );
    assert_eq!(key.0.len(), GLYPH_KEY_LEN);
    assert!(!key.0.spilled(), "19 B must fit SmallVec<[u8; 24]> inline");
    // [kind=0][font=0 u32][glyph=7 u16][20.0f32 bits][x_bin=1][y_bin=0]
    // [weight=400 u16][flags=0 u32], all little-endian.
    assert_eq!(
        key.0.as_slice(),
        &[
            0, // kind: Glyph
            0, 0, 0, 0, // interned font 0
            7, 0, // glyph_id
            0x00, 0x00, 0xA0, 0x41, // 20.0f32.to_bits() LE
            1,    // x_bin One
            0,    // y_bin Zero
            0x90, 0x01, // weight 400 LE
            0, 0, 0, 0, // flags
        ]
    );
}

#[test]
fn distinct_cache_keys_make_distinct_atlas_keys() {
    let mut interner = FontKeyInterner::default();
    let font = embedded_face();
    let variants = [
        key_with(font, 7, 20.0, SubpixelBin::Zero),
        key_with(font, 8, 20.0, SubpixelBin::Zero), // glyph_id
        key_with(font, 7, 25.0, SubpixelBin::Zero), // size
        key_with(font, 7, 20.0, SubpixelBin::Two),  // x_bin
        CacheKey {
            font_weight: fontdb::Weight(700),
            ..key_with(font, 7, 20.0, SubpixelBin::Zero)
        },
        CacheKey {
            flags: CacheKeyFlags::FAKE_ITALIC,
            ..key_with(font, 7, 20.0, SubpixelBin::Zero)
        },
    ];
    let keys: HashSet<_> = variants
        .iter()
        .map(|ck| glyph_atlas_key(ck, &mut interner))
        .collect();
    assert_eq!(
        keys.len(),
        variants.len(),
        "every shape-affecting field is in the key"
    );
}

#[test]
fn interner_is_stable_and_monotonic() {
    // Two distinct ids in ONE db: fontdb does not dedup a re-loaded source,
    // so loading the embedded face's own Source again yields a second id.
    let mut db = registered_fonts_db();
    let first = db.faces().next().unwrap().id;
    let source = db.faces().next().unwrap().source.clone();
    let second = db.load_font_source(source)[0];
    assert_ne!(first, second);

    let mut interner = FontKeyInterner::default();
    let a0 = interner.intern(first);
    let b0 = interner.intern(second);
    let a1 = interner.intern(first);
    assert_eq!(a0, 0, "sequential from zero");
    assert_eq!(b0, 1);
    assert_eq!(a0, a1, "stable across calls — the content-address contract");
    assert_eq!(interner.len(), 2, "monotonic, never evicted");

    // Same CacheKey through the same interner ⇒ identical key bytes
    // (round-trip half of § 12 a).
    let mut i2 = FontKeyInterner::default();
    i2.intern(first);
    let ck = key_with(first, 3, 16.0, SubpixelBin::Three);
    assert_eq!(
        glyph_atlas_key(&ck, &mut interner),
        glyph_atlas_key(&ck, &mut i2)
    );
}
