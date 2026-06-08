//! GPU end-to-end atlas tests: the coverage-glyph (alpha-as-color) pipeline.
//! Every test here needs a wgpu adapter (real GPU or lavapipe), so all are
//! `#[ignore]` exactly like render_smoke.rs / render_golden_harness.rs. The
//! headless allocator/LRU/pooling/warmup contract is covered adapter-free in
//! atlas_alloc.rs; this file proves the GPU half (upload + sample + alpha-as-
//! color + warmup determinism + gate-#15 idle settle) on the RX 6700 XT.
//!
//! These tests play the **producer** the text seam owns (`buiy-text-rendering-
//! design`, unbuilt): they build an `AtlasBitmap` and emit `GlyphAlphaInstance`s
//! directly, exactly as the headless `atlas_alloc.rs` tests build `AtlasBitmap`
//! directly — no standing in-crate glyph producer (that would pre-empt the seam).
//!
//! Run locally with a GPU/lavapipe:
//!   cargo test -p buiy_core --test atlas_gpu -- --ignored --nocapture --test-threads=1

mod support;

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::render::atlas::{
    AtlasBitmap, AtlasFormat, AtlasKey, AtlasWarmupQueue, AtlasWarmupRequest, BuiyAtlas,
    GlyphAlphaInstance,
};
use buiy_core::render::prepare::ExtractedGlyphs;

const NEG_INF: f32 = f32::NEG_INFINITY;
const POS_INF: f32 = f32::INFINITY;

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

/// Push a CoverageR8 warmup request into the render world's `AtlasWarmupQueue`
/// (drained pre-paint by `warmup_atlas`). The test is the producer (§ 2.3).
fn warmup_coverage(app: &mut App, key: &AtlasKey, bitmap: AtlasBitmap) {
    let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
    render_app
        .world_mut()
        .resource_mut::<AtlasWarmupQueue>()
        .push(AtlasWarmupRequest {
            key: key.clone(),
            format: AtlasFormat::CoverageR8,
            bitmap,
        });
}

/// Set the render world's `ExtractedGlyphs` to `instances` (marks it changed, so
/// `prepare_buiy_instances` re-uploads the glyph buffer). The test is the
/// producer emitting one `GlyphAlphaInstance` per visible glyph (§ 5 step 3).
fn set_glyphs(app: &mut App, instances: Vec<GlyphAlphaInstance>) {
    let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
    render_app
        .world_mut()
        .resource_mut::<ExtractedGlyphs>()
        .glyphs = instances;
}

/// Read the resident CoverageR8 page-0 CPU texels out of the render world's
/// `BuiyAtlas` (the §7/§4.1 byte-identity source).
fn coverage_page0_bytes(app: &App) -> Vec<u8> {
    let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
    render_app
        .world()
        .resource::<BuiyAtlas>()
        .page_pixels(AtlasFormat::CoverageR8, 0)
        .expect("a coverage page exists after warmup")
        .to_vec()
}

/// One unclipped glyph instance painting `bitmap-cell uv_rect` at logical px
/// `rect` with linear-light `color`. `uv` is the `AtlasEntry.uv` rect.
fn glyph(rect: [f32; 4], uv: bevy::math::Rect, color: [f32; 4]) -> GlyphAlphaInstance {
    GlyphAlphaInstance {
        rect,
        uv: [uv.min.x, uv.min.y, uv.max.x, uv.max.y],
        color,
        // Unclipped: the `[±INFINITY]` sentinel (identical encoding to
        // `PackedInstance`), so the coverage shader's clip discard never fires.
        clip: [NEG_INF, NEG_INF, POS_INF, POS_INF],
        page: 0,
    }
}

/// Index one RGBA8 pixel out of an un-padded `w*h*4` readback buffer.
fn px(pixels: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// The expected sRGB8 the target stores for a full-coverage texel of linear
/// `color` over the opaque-black clear: SrcOver in linear space (dst=0), then
/// the `Rgba8UnormSrgb` write encodes linear→sRGB. coverage==1, so
/// `out = color` (straight-alpha), composited `a*color + (1-a)*0`.
fn expected_full_coverage_srgb(color: [f32; 4]) -> [u8; 4] {
    let a = color[3];
    let lin = LinearRgba::new(color[0] * a, color[1] * a, color[2] * a, 1.0);
    let s = Srgba::from(lin);
    [
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        255,
    ]
}

// --- (1) Upload + sample: a warmed coverage entry paints with its tint. ------
#[test]
#[ignore = "needs a wgpu adapter; GPU upload + sampling draw (spec § 7 'On GPU')"]
fn warmed_glyph_uploads_and_samples_with_tint() {
    const W: u32 = 64;
    const H: u32 = 64;
    const CELL: u32 = 16;

    let mut app = support::gpu_render_app(W, H);

    // Producer step 2: warm a known 16x16 full-0xFF coverage bitmap.
    let key = AtlasKey::from_bytes(b"glyph-tint-test");
    warmup_coverage(&mut app, &key, full_coverage(CELL));

    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // Finish so the device + atlas + warmup drain materialize, then look up the
    // warmed entry's UV rect to fill the instance.
    support::finish_and_run(&mut app, 1);
    let entry = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app
            .world()
            .resource::<BuiyAtlas>()
            .get(&key)
            .expect("warmed entry resident after the pre-paint drain")
    };

    // A known linear tint placed at logical px [16,16] sized 16x16. Producer
    // step 3: emit one GlyphAlphaInstance.
    let tint = [0.20, 0.80, 0.40, 1.0];
    let rect = [16.0, 16.0, CELL as f32, CELL as f32];
    set_glyphs(&mut app, vec![glyph(rect, entry.uv, tint)]);

    support::finish_and_run(&mut app, 3);
    let pixels = support::readback_rgba(&mut app, target);
    assert_eq!(pixels.len(), (W * H * 4) as usize);

    // Full-coverage interior texel (well inside the 16x16 cell at [16,16]) reads
    // the tint; a backdrop texel (outside the cell) reads the opaque-black clear.
    let inside = px(&pixels, W, 24, 24);
    let backdrop = px(&pixels, W, 4, 4);
    let expected = expected_full_coverage_srgb(tint);
    println!("inside (24,24)   = {inside:?}");
    println!("backdrop (4,4)   = {backdrop:?}");
    println!("expected tint    = {expected:?}");

    assert_eq!(
        backdrop,
        [0, 0, 0, 255],
        "zero-coverage backdrop reads the opaque-black clear"
    );
    const TOL: i32 = 4;
    for ch in 0..3 {
        assert!(
            (inside[ch] as i32 - expected[ch] as i32).abs() <= TOL,
            "full-coverage texel channel {ch}: got {} expected {} (±{TOL}) — \
             coverage(1.0) * tint must equal the instance color (alpha-as-color \
             § 4.1). full got={inside:?} expected={expected:?}",
            inside[ch],
            expected[ch],
        );
    }
}

// --- (2) Alpha-as-color: re-tinting a glyph never regenerates the atlas. ------
#[test]
#[ignore = "needs a wgpu adapter; atlas byte-identity across two themes (spec § 7)"]
fn retint_same_glyph_leaves_atlas_byte_identical() {
    const W: u32 = 64;
    const H: u32 = 64;
    const CELL: u32 = 16;

    let mut app = support::gpu_render_app(W, H);
    let key = AtlasKey::from_bytes(b"glyph-retint-test");
    warmup_coverage(&mut app, &key, full_coverage(CELL));

    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);

    let entry = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app
            .world()
            .resource::<BuiyAtlas>()
            .get(&key)
            .unwrap()
    };
    let rect = [16.0, 16.0, CELL as f32, CELL as f32];

    // Frame A: theme-A tint (reddish).
    let tint_a = [0.80, 0.10, 0.10, 1.0];
    set_glyphs(&mut app, vec![glyph(rect, entry.uv, tint_a)]);
    support::finish_and_run(&mut app, 3);
    let frame_a = support::readback_rgba(&mut app, target.clone());
    let atlas_a = coverage_page0_bytes(&app);
    let inside_a = px(&frame_a, W, 24, 24);

    // Frame B: theme-B tint (blueish). Only the instance color changes — the
    // atlas (coverage page) MUST stay byte-identical (§ 4.1, § 7).
    let tint_b = [0.10, 0.20, 0.90, 1.0];
    set_glyphs(&mut app, vec![glyph(rect, entry.uv, tint_b)]);
    support::finish_and_run(&mut app, 3);
    let frame_b = support::readback_rgba(&mut app, target.clone());
    let atlas_b = coverage_page0_bytes(&app);
    let inside_b = px(&frame_b, W, 24, 24);

    println!("frame A inside  = {inside_a:?}  (tint A {tint_a:?})");
    println!("frame B inside  = {inside_b:?}  (tint B {tint_b:?})");
    println!(
        "atlas bytes A==B = {} ({} bytes)",
        atlas_a == atlas_b,
        atlas_a.len()
    );

    // The atlas coverage page is byte-identical: the retint never touched it.
    assert_eq!(
        atlas_a, atlas_b,
        "CoverageR8 page must be byte-identical across the two tints — a re-tint \
         re-emits instances with a new color and never regenerates the atlas \
         (alpha-as-color § 4.1 / § 7)"
    );
    // ...AND the framebuffer differs: the tint actually took effect (so the
    // byte-identity is not vacuous — both frames really painted the glyph).
    assert_ne!(
        inside_a, inside_b,
        "the framebuffer must differ between the two tints (the retint is visible)"
    );
    let exp_a = expected_full_coverage_srgb(tint_a);
    let exp_b = expected_full_coverage_srgb(tint_b);
    const TOL: i32 = 4;
    for ch in 0..3 {
        assert!((inside_a[ch] as i32 - exp_a[ch] as i32).abs() <= TOL);
        assert!((inside_b[ch] as i32 - exp_b[ch] as i32).abs() <= TOL);
    }
}

// --- (3) Warmup determinism: first painted frame shows the glyph. ------------
#[test]
#[ignore = "needs a wgpu adapter; gate #2 warmup-determinism golden (spec § 2.3, § 7)"]
fn warmup_makes_first_frame_match_golden() {
    const W: u32 = 64;
    const H: u32 = 64;
    const CELL: u32 = 16;
    let key = AtlasKey::from_bytes(b"warmup-glyph");
    let tint = [0.90, 0.90, 0.20, 1.0];
    let rect = [16.0, 16.0, CELL as f32, CELL as f32];

    // The control fixes the entry's UV without depending on warmup ordering:
    // a full-page single cell at [0,0] over a 1024 page is uv [0,0]-(CELL/1024).
    let uv_for = |size: u32| {
        let inv = 1.0 / size as f32;
        bevy::math::Rect::new(0.0, 0.0, CELL as f32 * inv, CELL as f32 * inv)
    };

    // --- With warmup: the FIRST painted frame shows the glyph. ---
    let inside_warm = {
        let mut app = support::gpu_render_app(W, H);
        warmup_coverage(&mut app, &key, full_coverage(CELL));
        let target = support::render_to_image(&mut app, W, H);
        support::spawn_capture_camera(&mut app, target.clone());
        // Emit the instance BEFORE the first paint (the producer pushes both the
        // warmup request and the instance pre-paint). The 1024 default page puts
        // the first cell at uv origin.
        support::finish_and_run(&mut app, 1);
        set_glyphs(&mut app, vec![glyph(rect, uv_for(1024), tint)]);
        // Read back the first painted frame.
        let pixels = support::readback_rgba(&mut app, target);
        px(&pixels, W, 24, 24)
    };

    let expected = expected_full_coverage_srgb(tint);
    println!("warm first-frame inside (24,24) = {inside_warm:?}");
    println!("expected tint                   = {expected:?}");
    const TOL: i32 = 4;
    for ch in 0..3 {
        assert!(
            (inside_warm[ch] as i32 - expected[ch] as i32).abs() <= TOL,
            "warmup made the glyph present on the first painted frame: channel \
             {ch} got {} expected {} (±{TOL}); full got={inside_warm:?}",
            inside_warm[ch],
            expected[ch],
        );
    }

    // --- Control: WITHOUT warmup the glyph is absent on frame 1 (warmup is
    // load-bearing). We emit the instance but never warm the atlas, so no
    // coverage page exists, the `@group(1)` bind group is never built, and the
    // glyph draw is skipped — the pixel reads the clear. ---
    let inside_cold = {
        let mut app = support::gpu_render_app(W, H);
        let target = support::render_to_image(&mut app, W, H);
        support::spawn_capture_camera(&mut app, target.clone());
        support::finish_and_run(&mut app, 1);
        set_glyphs(&mut app, vec![glyph(rect, uv_for(1024), tint)]);
        let pixels = support::readback_rgba(&mut app, target);
        px(&pixels, W, 24, 24)
    };
    println!("cold (no-warmup) inside (24,24) = {inside_cold:?}");
    assert_eq!(
        inside_cold,
        [0, 0, 0, 255],
        "without warmup the glyph has no resident coverage, so the first frame \
         reads the clear — warmup is load-bearing (gate #2 § 2.3)"
    );
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
