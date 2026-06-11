//! GPU atlas-mechanics tests: gate-#15 idle-settle on a real adapter.
//! The former test-as-producer fills (warmup_coverage/set_glyphs emitting
//! GlyphAlphaInstances directly) were REPLACED in T4 by real-entity text
//! fixtures in tests/text_gpu.rs — the in-crate producer
//! (text::extract_buiy_glyphs) now owns that seam. The warmup queue's GPU
//! consumer coverage returns with T6's solid-stamp push.
//!
//! Run: cargo test -p buiy_core --test atlas_gpu -- --ignored --test-threads=1

mod support;

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::render::atlas::{AtlasBitmap, AtlasFormat, AtlasKey, BuiyAtlas};

/// A full-coverage (0xFF) `size×size` CoverageR8 bitmap — a known "glyph" whose
/// every texel is opaque, so a full-coverage interior texel reads exactly the
/// instance tint (`color * 1.0`) and lets us assert the alpha-as-color product.
fn full_coverage(size: u32) -> AtlasBitmap {
    AtlasBitmap {
        size: UVec2::new(size, size),
        format: AtlasFormat::CoverageR8,
        data: vec![0xFF; (size * size) as usize],
    }
}

// --- (4) Gate #15: atlas entries return within ε of baseline after idle. -----
#[test]
#[ignore = "needs a wgpu adapter; gate #15 atlas-entries-return-to-baseline fixture"]
fn gate15_atlas_entries_return_to_baseline_after_idle() {
    use buiy_core::render::atlas::AtlasConfig;

    const W: u32 = 64;
    const H: u32 = 64;

    let mut app = support::gpu_render_app(W, H);

    // Install a small-grace config so the idle-settle window is short. A tiny
    // 64-texel page set forces page growth as transient entries churn, so the
    // page-pool reuse path (§ 2.5) is exercised. The idle-settle window must
    // exceed max(eviction_grace, RT-pool 3 frames) — grace 2 here, so we idle
    // for 2+3+slack frames.
    {
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        render_app
            .world_mut()
            .insert_resource(BuiyAtlas::new(AtlasConfig {
                page_size: 64,
                page_budget: 8,
                eviction_grace: 2,
            }));
    }

    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);

    // Settle to the idle floor before capturing baseline: the T6 warmup-pinned
    // solid stamp (text::register_render_world) is resident on frame 1, and
    // with no live stamp instance it drains after `eviction_grace` like any
    // entry (warmup-pinned is not pin-forever — pinned by the headless test
    // `idle_stamp_evicts_and_reinserts_on_miss`). Idling past grace=2 (+slack)
    // here keeps `baseline` future-proof against further warmup pushes.
    for _ in 0..5 {
        app.update();
    }

    let baseline = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app
            .world()
            .resource::<BuiyAtlas>()
            .live_entry_count()
    };

    // Churn many transient 32x32 entries across several frames (each frame a
    // fresh key set), letting `maintain_atlas` (ExtractSchedule) advance the
    // frame clock + drain. Each batch's entries are touched only the frame they
    // insert, so after the grace window they drain.
    let mut max_pages = 0usize;
    for batch in 0..6u8 {
        for i in 0..4u8 {
            let k = AtlasKey::from_bytes(&[0xAA, batch, i]);
            let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
            let mut atlas = render_app.world_mut().resource_mut::<BuiyAtlas>();
            atlas.get_or_insert(k, AtlasFormat::CoverageR8, || full_coverage(32));
        }
        app.update();
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let pages = render_app
            .world()
            .resource::<BuiyAtlas>()
            .page_count(AtlasFormat::CoverageR8);
        max_pages = max_pages.max(pages);
    }
    let churned = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app
            .world()
            .resource::<BuiyAtlas>()
            .live_entry_count()
    };
    println!("baseline={baseline} churned={churned} max_pages={max_pages}");
    assert!(churned > baseline, "churn actually inserted entries");
    assert!(
        max_pages > 1,
        "churn forced page growth (so pooling is exercised)"
    );

    // Idle PAST max(grace=2, RT-pool 3) + slack, letting `maintain_atlas` drain
    // each frame without any new touches.
    for _ in 0..10 {
        app.update();
    }

    let (after, pages_after, pooled_after) = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        (
            atlas.live_entry_count(),
            atlas.page_count(AtlasFormat::CoverageR8),
            atlas.pooled_page_count(AtlasFormat::CoverageR8),
        )
    };
    println!("after idle: entries={after} pages={pages_after} pooled={pooled_after}");

    // Entries return within ε of baseline (ε=0: all transient entries drained).
    assert_eq!(
        after, baseline,
        "live-entry count returns to baseline after idle (gate #15 § 2.4 step 3)"
    );
    // Page count did not grow monotonically: emptied pages went to the pool, so
    // the live page set shrank back and the pool holds the recycled textures
    // (the GPU `Texture` handle is reused at the same page index, § 2.5).
    assert!(
        pages_after < max_pages,
        "live page count shrank from the churn peak ({max_pages} -> {pages_after}) \
         — emptied pages were collected, not retained"
    );
    assert!(
        pooled_after > 0,
        "emptied pages were pooled (texture reused, not reallocated — § 2.5)"
    );
}
