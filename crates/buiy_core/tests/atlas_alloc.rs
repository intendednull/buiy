//! Headless unit tests for the BuiyAtlas allocator + LRU + pooling logic.
//! Pure-CPU (guillotiere is CPU); no wgpu adapter required, so these gate on CI.
use buiy_core::render::atlas::AtlasFormat;

#[test]
fn atlas_module_is_reachable() {
    // Compile-time proof the module + a public type are wired through
    // render::mod. Real allocator tests land in Task 4+.
    let f = AtlasFormat::CoverageR8;
    assert_ne!(f, AtlasFormat::ColorRgba8);
}

use bevy::math::{Rect, URect, UVec2, Vec2};
use buiy_core::render::atlas::{AtlasBitmap, AtlasEntry, AtlasKey};

#[test]
fn atlas_key_is_eq_and_hash_by_content() {
    use std::collections::HashMap;
    let a = AtlasKey::from_bytes(&[1, 2, 3]);
    let b = AtlasKey::from_bytes(&[1, 2, 3]);
    let c = AtlasKey::from_bytes(&[1, 2, 4]);
    assert_eq!(a, b, "equal bytes -> equal key");
    assert_ne!(a, c);
    let mut m: HashMap<AtlasKey, u32> = HashMap::new();
    m.insert(a, 7);
    assert_eq!(m.get(&b), Some(&7), "equal key hashes to same slot");
}

#[test]
fn atlas_bitmap_carries_size_format_and_data() {
    let bmp = AtlasBitmap {
        size: UVec2::new(2, 1),
        format: AtlasFormat::CoverageR8,
        data: vec![0xAB, 0xCD],
    };
    assert_eq!(bmp.size, UVec2::new(2, 1));
    assert_eq!(bmp.data.len(), 2);
}

#[test]
fn atlas_entry_is_copy_and_carries_uv_and_px() {
    let e = AtlasEntry {
        page: 0,
        format: AtlasFormat::CoverageR8,
        uv: Rect::new(0.0, 0.0, 0.5, 0.5),
        px: URect::new(0, 0, 16, 16),
    };
    let e2 = e; // Copy
    assert_eq!(e2.page, 0);
    assert_eq!(e2.px, URect::new(0, 0, 16, 16));
    let _ = Vec2::ZERO; // keep the Vec2 import honest
}

use buiy_core::render::atlas::AtlasConfig;

#[test]
fn atlas_config_v1_defaults_match_spec() {
    let c = AtlasConfig::default();
    // Spec § 2.2: default page size 1024x1024.
    assert_eq!(c.page_size, 1024, "default page size 1024 (spec § 2.2)");
    // Spec § 2.4: v1 default page_budget = 8 pages.
    assert_eq!(c.page_budget, 8, "v1 default page_budget = 8 (spec § 2.4)");
    // eviction_grace is a frame count; v1 picks a small nonzero default so
    // idle transient entries drain (spec § 2.4 step 3). Tuned value deferred.
    assert!(c.eviction_grace >= 1, "grace is at least one frame");
}

use buiy_core::render::atlas::AtlasPage;

#[test]
fn page_allocates_a_rect_of_requested_size() {
    let mut page = AtlasPage::new(1024);
    let r: URect = page
        .try_alloc(URect::new(0, 0, 16, 32).size())
        .expect("fits");
    assert_eq!(r.width(), 16);
    assert_eq!(r.height(), 32);
    assert!(r.max.x <= 1024 && r.max.y <= 1024, "stays inside the page");
}

#[test]
fn page_alloc_returns_none_when_request_exceeds_page() {
    let mut page = AtlasPage::new(64);
    assert!(
        page.try_alloc(bevy::math::UVec2::new(128, 128)).is_none(),
        "a request larger than the page cannot fit"
    );
}

#[test]
fn page_deallocate_frees_space_for_reuse() {
    // Fill the page with one big alloc, free it, re-alloc the same size.
    let mut page = AtlasPage::new(64);
    let id = page
        .alloc_id(bevy::math::UVec2::new(64, 64))
        .expect("first fits");
    assert!(
        page.try_alloc(bevy::math::UVec2::new(64, 64)).is_none(),
        "page is full after the 64x64 alloc"
    );
    page.free(id);
    assert!(
        page.try_alloc(bevy::math::UVec2::new(64, 64)).is_some(),
        "after free the space is reusable (guillotiere deallocate)"
    );
}

#[test]
fn page_is_empty_reports_residency() {
    let mut page = AtlasPage::new(64);
    assert!(page.is_empty());
    let id = page.alloc_id(bevy::math::UVec2::new(8, 8)).unwrap();
    assert!(!page.is_empty());
    page.free(id);
    assert!(
        page.is_empty(),
        "page empty again after the only alloc frees"
    );
}

use buiy_core::render::atlas::LruQueue;

fn k(b: u8) -> AtlasKey {
    AtlasKey::from_bytes(&[b])
}

#[test]
fn lru_pops_least_recently_touched_first() {
    let mut lru = LruQueue::default();
    lru.touch(k(1), 0);
    lru.touch(k(2), 0);
    lru.touch(k(3), 0);
    // Re-touch k(1): now order from LRU is k(2), k(3), k(1).
    lru.touch(k(1), 1);
    assert_eq!(lru.pop_lru(), Some(k(2)));
    assert_eq!(lru.pop_lru(), Some(k(3)));
    assert_eq!(lru.pop_lru(), Some(k(1)));
    assert_eq!(lru.pop_lru(), None);
}

#[test]
fn lru_touch_is_idempotent_on_membership() {
    let mut lru = LruQueue::default();
    lru.touch(k(7), 0);
    lru.touch(k(7), 1);
    lru.touch(k(7), 2);
    assert_eq!(lru.len(), 1, "re-touching does not duplicate the entry");
    assert_eq!(lru.pop_lru(), Some(k(7)));
    assert_eq!(lru.pop_lru(), None);
}

#[test]
fn lru_grace_expired_lists_keys_untouched_past_grace() {
    let mut lru = LruQueue::default();
    lru.touch(k(1), 10); // last touched frame 10
    lru.touch(k(2), 50); // last touched frame 50
    // At frame 100 with grace 60: k(1) (idle 90) expired; k(2) (idle 50) not.
    let expired = lru.grace_expired(100, 60);
    assert_eq!(expired, vec![k(1)]);
}

#[test]
fn lru_remove_drops_a_specific_key() {
    let mut lru = LruQueue::default();
    lru.touch(k(1), 0);
    lru.touch(k(2), 0);
    lru.remove(&k(1));
    assert_eq!(lru.len(), 1);
    assert_eq!(lru.pop_lru(), Some(k(2)));
}

use buiy_core::render::atlas::BuiyAtlas;
use std::cell::Cell;

fn cov(w: u32, h: u32) -> AtlasBitmap {
    AtlasBitmap {
        size: UVec2::new(w, h),
        format: AtlasFormat::CoverageR8,
        data: vec![0xFF; (w * h) as usize],
    }
}

#[test]
fn get_or_insert_is_idempotent_no_reblit_on_hit() {
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let key = AtlasKey::from_bytes(b"glyph-A");
    let calls = Cell::new(0);

    let e1 = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, || {
        calls.set(calls.get() + 1);
        cov(16, 16)
    });
    // Second call with an equal key: closure must NOT run (no rasterize, no
    // blit), and the returned entry is identical. Spec § 3, § 7 (a).
    let e2 = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, || {
        calls.set(calls.get() + 1);
        cov(16, 16)
    });
    assert_eq!(
        calls.get(),
        1,
        "closure runs exactly once across two inserts"
    );
    assert_eq!(e1, e2, "equal key -> identical AtlasEntry");
    assert_eq!(e1.px.width(), 16);
    assert_eq!(e1.px.height(), 16);
}

#[test]
fn get_probe_does_not_touch_lru_and_sees_residency() {
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let key = AtlasKey::from_bytes(b"glyph-B");
    assert!(atlas.get(&key).is_none(), "absent before insert");
    let e = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, || cov(8, 8));
    assert_eq!(atlas.get(&key), Some(e), "resident after insert");
    assert_eq!(atlas.live_entry_count(), 1);
}

#[test]
fn uv_rect_is_normalized_against_page_size() {
    let mut atlas = BuiyAtlas::new(AtlasConfig::default()); // 1024 page
    let e = atlas.get_or_insert(AtlasKey::from_bytes(b"g"), AtlasFormat::CoverageR8, || {
        cov(512, 256)
    });
    // px (0,0)-(512,256) over a 1024 page -> uv (0,0)-(0.5,0.25).
    assert!((e.uv.max.x - 0.5).abs() < 1e-6);
    assert!((e.uv.max.y - 0.25).abs() < 1e-6);
}

#[test]
fn formats_do_not_share_a_page() {
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    atlas.get_or_insert(
        AtlasKey::from_bytes(b"cov"),
        AtlasFormat::CoverageR8,
        || cov(8, 8),
    );
    atlas.get_or_insert(
        AtlasKey::from_bytes(b"col"),
        AtlasFormat::ColorRgba8,
        || AtlasBitmap {
            size: UVec2::new(8, 8),
            format: AtlasFormat::ColorRgba8,
            data: vec![0xFF; 8 * 8 * 4],
        },
    );
    assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 1);
    assert_eq!(atlas.page_count(AtlasFormat::ColorRgba8), 1);
}

// A tiny-budget config that forces pressure quickly: one 64x64 page per
// format, grace 2 frames.
fn pressure_config() -> AtlasConfig {
    AtlasConfig {
        page_size: 64,
        page_budget: 1,
        eviction_grace: 2,
    }
}

#[test]
fn eviction_drops_least_recently_used_under_budget_pressure() {
    let mut atlas = BuiyAtlas::new(pressure_config());
    // Four 32x32 cells exactly tile a 64x64 page (budget = 1 page).
    let keys: Vec<AtlasKey> = (0..4).map(|i| AtlasKey::from_bytes(&[i])).collect();
    for k in &keys {
        atlas.get_or_insert(k.clone(), AtlasFormat::CoverageR8, || cov(32, 32));
    }
    assert_eq!(atlas.live_entry_count(), 4, "page is exactly full");
    assert_eq!(
        atlas.page_count(AtlasFormat::CoverageR8),
        1,
        "still one page"
    );

    // Touch keys 1,2,3 so key 0 is the LRU victim.
    for k in &keys[1..] {
        atlas.touch_existing(k);
    }
    // A fifth cell forces eviction of the LRU (key 0), NOT a budget-busting
    // 2nd page. Spec § 2.4 step 2.
    let k4 = AtlasKey::from_bytes(&[4]);
    atlas.get_or_insert(k4.clone(), AtlasFormat::CoverageR8, || cov(32, 32));
    assert_eq!(
        atlas.page_count(AtlasFormat::CoverageR8),
        1,
        "eviction kept us at budget, no new page"
    );
    assert!(atlas.get(&keys[0]).is_none(), "LRU victim evicted");
    assert!(atlas.get(&k4).is_some(), "new entry resident");
    assert_eq!(atlas.live_entry_count(), 4);
}

#[test]
fn budget_exceeded_only_when_lru_exhausted() {
    // One cell that fills the whole page; a second of the same size cannot
    // fit even after evicting the first IF the first is the one being
    // re-requested. Here we insert a 64x64 (fills page), then a *new* 64x64:
    // eviction frees the first, the second fits -> still one page.
    let mut atlas = BuiyAtlas::new(pressure_config());
    let k0 = AtlasKey::from_bytes(b"big0");
    let k1 = AtlasKey::from_bytes(b"big1");
    atlas.get_or_insert(k0.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
    atlas.get_or_insert(k1.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
    assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 1);
    assert!(atlas.get(&k0).is_none(), "first evicted to make room");
    assert!(atlas.get(&k1).is_some());
}

#[test]
fn grace_drain_returns_idle_entries_to_baseline() {
    // The headless half of gate #15: after a scripted insert -> idle cycle,
    // live-entry count returns to baseline and page count does not grow
    // monotonically. Spec § 2.4 step 3, § 7.
    let mut atlas = BuiyAtlas::new(pressure_config());
    let baseline = atlas.live_entry_count(); // 0
    atlas.get_or_insert(
        AtlasKey::from_bytes(b"transient"),
        AtlasFormat::CoverageR8,
        || cov(16, 16),
    );
    assert_eq!(atlas.live_entry_count(), 1);

    // Idle: advance frames past the grace window without touching the entry,
    // draining each frame.
    for _ in 0..(pressure_config().eviction_grace + 1) {
        atlas.begin_frame();
        atlas.drain_grace_expired();
    }
    assert_eq!(
        atlas.live_entry_count(),
        baseline,
        "transient entry drained back to baseline after idle"
    );
    assert_eq!(
        atlas.page_count(AtlasFormat::CoverageR8),
        1,
        "page count did not grow monotonically"
    );
}

#[test]
fn emptied_page_is_pooled_and_reused_not_reallocated() {
    let mut atlas = BuiyAtlas::new(AtlasConfig {
        page_size: 64,
        page_budget: 8,
        eviction_grace: 0, // drain immediately on idle
    });
    // Fill page 0 fully, then add one cell that needs a 2nd page.
    let k0 = AtlasKey::from_bytes(b"fills-page-0");
    atlas.get_or_insert(k0.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
    let k1 = AtlasKey::from_bytes(b"needs-page-1");
    atlas.get_or_insert(k1.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
    assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 2);

    // The pooled-page identity token before any recycling.
    let pool_before = atlas.pooled_page_count(AtlasFormat::CoverageR8);
    assert_eq!(pool_before, 0, "nothing pooled yet");

    // Evict everything on page 1 -> it empties -> pooled, not dropped.
    atlas.begin_frame();
    atlas.evict_for_test(&k1);
    atlas.collect_emptied_pages();
    assert_eq!(
        atlas.pooled_page_count(AtlasFormat::CoverageR8),
        1,
        "emptied page returned to the pool, not freed"
    );

    // A new page-growth request reuses the pooled page instead of allocating.
    let k2 = AtlasKey::from_bytes(b"reuses-pool");
    atlas.get_or_insert(k2.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
    assert_eq!(
        atlas.pooled_page_count(AtlasFormat::CoverageR8),
        0,
        "pooled page taken back into service (reused, not reallocated)"
    );
}

use buiy_core::render::atlas::{AtlasWarmupQueue, AtlasWarmupRequest};

#[test]
fn warmup_drain_forces_requested_entries_resident_before_paint() {
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let mut queue = AtlasWarmupQueue::default();
    let key = AtlasKey::from_bytes(b"ascii-A");
    queue.push(AtlasWarmupRequest {
        key: key.clone(),
        format: AtlasFormat::CoverageR8,
        bitmap: cov(16, 16),
    });
    assert!(atlas.get(&key).is_none(), "cold before warmup");
    assert_eq!(queue.len(), 1);

    // The drain (mechanism this spec owns) forces residency and empties the
    // queue. In-app this runs pre-paint via `warmup_atlas`. Spec § 2.3.
    atlas.drain_warmup(&mut queue);
    assert!(atlas.get(&key).is_some(), "resident after warmup drain");
    assert_eq!(queue.len(), 0, "queue drained");
}

#[test]
fn warmup_drain_is_idempotent_for_duplicate_requests() {
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let mut queue = AtlasWarmupQueue::default();
    let key = AtlasKey::from_bytes(b"dup");
    for _ in 0..3 {
        queue.push(AtlasWarmupRequest {
            key: key.clone(),
            format: AtlasFormat::CoverageR8,
            bitmap: cov(16, 16),
        });
    }
    atlas.drain_warmup(&mut queue);
    assert_eq!(
        atlas.live_entry_count(),
        1,
        "duplicate warmups dedup to one entry"
    );
}

use buiy_core::render::atlas::AtlasEntryKind;

#[test]
fn entry_kind_maps_to_format_per_spec() {
    // Glyph + mask are CoverageR8; icon + gradient are ColorRgba8 (spec § 6).
    assert_eq!(AtlasEntryKind::Glyph.format(), AtlasFormat::CoverageR8);
    assert_eq!(AtlasEntryKind::Mask.format(), AtlasFormat::CoverageR8);
    assert_eq!(AtlasEntryKind::Icon.format(), AtlasFormat::ColorRgba8);
    assert_eq!(AtlasEntryKind::Gradient.format(), AtlasFormat::ColorRgba8);
}

#[test]
fn reserved_mask_entry_uses_the_glyph_alpha_path() {
    // A generated mask *is* a CoverageR8 entry sampled like a glyph — same
    // get_or_insert, same primitive (spec § 6 / § 4.1). Proves the reserved
    // kind needs no new atlas machinery.
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let key = AtlasKey::from_bytes(b"mask-1");
    let e = atlas.get_or_insert(key, AtlasEntryKind::Mask.format(), || cov(8, 8));
    assert_eq!(e.format, AtlasFormat::CoverageR8);
}
