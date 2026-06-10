//! § 6.3 — the eviction-under-retention hazard, CPU half (verification
//! § 12 headless d): against the real device-free `BuiyAtlas`, a retained
//! visible glyph survives > eviction_grace frames; an off-screen one
//! drains. Plus the § 12 (e) seam contract: no cosmic-text type crosses
//! into the render module.

mod support;

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::AtlasConfig;
use buiy_core::text::Text;
use support::extract_harness::TextExtractHarness;

const GRACE: u32 = 3;

fn harness() -> TextExtractHarness {
    TextExtractHarness::with_atlas_config(AtlasConfig {
        page_size: 1024,
        page_budget: 8,
        eviction_grace: GRACE,
    })
}

fn spawn_text(h: &mut TextExtractHarness, s: &str) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from(s))))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    text
}

#[test]
fn retained_visible_keys_survive_past_eviction_grace() {
    let mut h = harness();
    spawn_text(&mut h, "warm");
    h.settle();
    let keys = h.resident_keys();
    assert!(!keys.is_empty());

    // Steady frames ≫ grace: NO rebuild happens (Task 4 pins that), so
    // without the un-gated touch pass these keys would grace-evict while
    // the retained instances still sample them — the silent-corruption
    // hazard. The touch pass is the only per-frame text work.
    let settled = h.changed_frames();
    for _ in 0..(GRACE * 4) {
        h.frame();
    }
    assert_eq!(
        h.changed_frames(),
        settled,
        "no rebuild occurred (retention held)"
    );
    for key in &keys {
        assert!(
            h.atlas().get(key).is_some(),
            "visible key survived idle > eviction_grace — the § 6.3 touch pass"
        );
    }
}

#[test]
fn offscreen_keys_drain_after_grace() {
    let mut h = harness();
    // "warm" and "cold" share no letters, so their glyph-key sets are
    // disjoint — the prune assertion below cannot be revived by re-inserts.
    let text = spawn_text(&mut h, "warm");
    h.settle();
    let keys = h.resident_keys();
    assert!(!keys.is_empty());

    // Despawn: the rebuild empties ResidentTextKeys, so nothing touches the
    // old keys — they must drain within the grace window (gate #15's
    // return-to-baseline depends on exactly this).
    h.app.world_mut().entity_mut(text).despawn();
    h.frame();
    assert!(h.resident_keys().is_empty());
    for _ in 0..(GRACE + 2) {
        h.frame();
    }
    for key in &keys {
        assert!(
            h.atlas().get(key).is_none(),
            "off-screen key drained after grace (no touch pass member keeps it warm)"
        );
    }
    // Decision 3 invariant: the bearing cache prunes to residency — the
    // prune runs on REBUILD frames, so force one (the disjoint "cold" text)
    // and observe the drained keys leave the cache.
    spawn_text(&mut h, "cold");
    h.frame();
    let meta = h.render.resource::<buiy_core::text::GlyphMetaCache>();
    for key in &keys {
        assert!(
            !meta.0.contains_key(key),
            "bearing cache pruned to residency"
        );
    }
}

/// verification § 1.2's seam-contract row, half 1: the whole producer flow
/// is expressible against the render seam types alone — an `AtlasKey` is
/// opaque bytes, residency is `get_or_insert` with an `AtlasBitmap`, and
/// the output is a `GlyphAlphaInstance`. No cosmic-text type appears in
/// this function's signature or body.
#[test]
fn seam_speaks_only_render_types() {
    use bevy::math::UVec2;
    use buiy_core::render::atlas::{
        AtlasBitmap, AtlasConfig, AtlasFormat, AtlasKey, BuiyAtlas, GlyphAlphaInstance,
    };

    fn stub_producer(atlas: &mut BuiyAtlas) -> Vec<GlyphAlphaInstance> {
        let key = AtlasKey::from_bytes(&[0u8, 1, 2, 3]); // opaque to the atlas
        let entry = atlas.get_or_insert(key, AtlasFormat::CoverageR8, || AtlasBitmap {
            size: UVec2::splat(4),
            format: AtlasFormat::CoverageR8,
            data: vec![0xFF; 16],
        });
        vec![GlyphAlphaInstance {
            rect: [0.0, 0.0, 4.0, 4.0],
            uv: [
                entry.uv.min.x,
                entry.uv.min.y,
                entry.uv.max.x,
                entry.uv.max.y,
            ],
            color: [1.0, 1.0, 1.0, 1.0],
            clip: [
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::INFINITY,
            ],
            page: entry.page as u32,
        }]
    }

    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    assert_eq!(stub_producer(&mut atlas).len(), 1);
}

/// Half 2 — the drift tripwire: the render atlas module never imports
/// cosmic-text (the construction of glyph keys is the text module's;
/// atlas/mod.rs's seam doc says "no cosmic-text type ever crosses").
#[test]
fn render_atlas_sources_never_name_cosmic_text() {
    for src in [
        include_str!("../src/render/atlas/mod.rs"),
        include_str!("../src/render/atlas/types.rs"),
        include_str!("../src/render/atlas/atlas.rs"),
        include_str!("../src/render/atlas/lru.rs"),
        include_str!("../src/render/atlas/page.rs"),
        include_str!("../src/render/atlas/primitive.rs"),
        include_str!("../src/render/atlas/warmup.rs"),
        include_str!("../src/render/atlas/gpu.rs"),
    ] {
        assert!(
            !src.contains("cosmic_text"),
            "a cosmic-text type crossed into the render atlas seam"
        );
    }
}
