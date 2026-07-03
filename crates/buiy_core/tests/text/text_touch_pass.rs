//! § 6.3 — the eviction-under-retention hazard, CPU half (verification
//! § 12 headless d): against the real device-free `BuiyAtlas`, a retained
//! visible glyph survives > eviction_grace frames; an off-screen one
//! drains. Plus the § 12 (e) seam contract: no cosmic-text type crosses
//! into the render module.

use crate::support::extract_harness::TextExtractHarness;
use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::AtlasConfig;
use buiy_core::text::Text;

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

/// Stage C D5 — touch-before-insert under REAL page-budget pressure: on a
/// Patch frame, retained entities' keys carry frame-old LRU stamps, so the
/// patch's own inserts would pick them as pressure-eviction victims while
/// their instances stay retained (silent stale-UV sampling — a Patch never
/// re-derives retained uv/page, unlike a Full walk). D5 orders a touch of
/// every retained key BEFORE any emission insert, and deliberately does NOT
/// touch the changed entity's stale ranges — so under pressure the LRU
/// victims are exactly the patch's own garbage.
///
/// Deterministic fixture: `page_budget: 1` with a page the settled scene
/// fills near capacity, `eviction_grace` huge so grace expiry cannot fire —
/// any eviction inside the patch frame is pressure eviction, forced by the
/// victim's letter-disjoint re-emission. RED without the pre-touch: the
/// idle-frame touch pass runs in resident order (sibling keys first →
/// OLDEST seq), so an untouched-at-patch-time sibling is the first LRU
/// victim and its retained instances sample a freed/re-used cell.
#[test]
fn patch_inserts_evict_stale_keys_never_retained_ones() {
    use buiy_core::text::GlyphDamage;

    let mut h = TextExtractHarness::with_atlas_config(AtlasConfig {
        page_size: 36,
        page_budget: 1,
        eviction_grace: 10_000,
    });
    // Letter-disjoint fixtures: the sibling's keys, the victim's old keys,
    // and the victim's new keys never collide, so every set's residency is
    // independently observable.
    // Spawn the VICTIM's tree first: root contexts rank the later-spawned
    // tree first, so the retained sibling paints FIRST — its keys sit at the
    // OLDEST end of the LRU on the patch frame (touched earliest by the
    // previous frame's resident-order touch pass). That is the exact
    // ordering D5 exists for: without the pre-touch, the sibling — not the
    // victim's stale keys — would be the first pressure-eviction victim.
    let victim = spawn_text(&mut h, "dgq");
    let sibling = spawn_text(&mut h, "mnrwvzu");
    h.settle();

    // Non-vacuity 1: the settled scene fits — no pressure before the patch.
    let keys_before = h.resident_keys();
    assert!(!keys_before.is_empty());
    for key in &keys_before {
        assert!(
            h.atlas().get(key).is_some(),
            "fixture: the settled scene must fit the page without eviction \
             (page_size too small)"
        );
    }
    let run_range = |h: &TextExtractHarness, e| {
        h.glyphs()
            .entity_runs
            .iter()
            .find(|r| r.entity == e)
            .expect("run")
            .instances
            .clone()
    };
    let sib_range = run_range(&h, sibling);

    // The Patch: re-emit the victim with disjoint letters → fresh inserts
    // under a full page (1 changed of 2 runs = 50 % ≤ the D3 bail). The
    // fresh glyphs ("ces", ≤ 8×9 px cells) each fit a cell freed by
    // evicting one of the victim's taller stale glyphs ("dgq", 8–9 wide ×
    // 13–14 tall), so with D5 intact the pressure resolves ENTIRELY against
    // the stale set and never cascades into the retained sibling.
    h.app.world_mut().get_mut::<Text>(victim).unwrap().0 = String::from("ces");
    h.frame();
    assert!(
        matches!(h.glyph_damage(), GlyphDamage::Patch { .. }),
        "the edit must execute as a Patch (got {:?})",
        h.glyph_damage()
    );

    // Non-vacuity 2: the patch's inserts really pressured the page — SOME
    // pre-patch key was evicted (grace cannot fire at 10_000 frames, so
    // only pressure eviction removes keys inside this frame). WHICH keys
    // were the victims is exactly what the D5 pin below discriminates.
    assert!(
        keys_before.iter().any(|k| h.atlas().get(k).is_none()),
        "fixture: the patch's inserts must force pressure eviction \
         (page_size too large)"
    );

    // THE D5 pin: every RETAINED key survived the patch's own inserts, and
    // each retained instance still samples ITS entry (uv + page coherent —
    // not evicted, not evicted-and-re-baked elsewhere). Plain-text fixture:
    // keys are 1:1 with instances, so global indices align.
    let keys_after = h.resident_keys();
    let glyphs = h.glyphs();
    assert_eq!(run_range(&h, sibling), sib_range, "sibling run unmoved");
    for i in sib_range.start as usize..sib_range.end as usize {
        assert_eq!(keys_after[i], keys_before[i], "sibling keys retained");
        let entry = h.atlas().get(&keys_after[i]).unwrap_or_else(|| {
            panic!(
                "D5 violated: retained key {i} was pressure-evicted by the \
                 patch's own inserts (touch-before-insert ordering broken)"
            )
        });
        let inst = &glyphs.glyphs[i];
        assert_eq!(
            inst.uv,
            [
                entry.uv.min.x,
                entry.uv.min.y,
                entry.uv.max.x,
                entry.uv.max.y
            ],
            "retained instance {i} must still sample its own cell"
        );
        assert_eq!(inst.page, entry.page as u32);
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
            affine: [1.0, 0.0, 0.0, 1.0],
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
        include_str!("../../src/render/atlas/mod.rs"),
        include_str!("../../src/render/atlas/types.rs"),
        include_str!("../../src/render/atlas/atlas.rs"),
        include_str!("../../src/render/atlas/lru.rs"),
        include_str!("../../src/render/atlas/page.rs"),
        include_str!("../../src/render/atlas/primitive.rs"),
        include_str!("../../src/render/atlas/warmup.rs"),
        include_str!("../../src/render/atlas/gpu.rs"),
    ] {
        assert!(
            !src.contains("cosmic_text"),
            "a cosmic-text type crossed into the render atlas seam"
        );
    }
}
