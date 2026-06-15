//! GPU end-to-end TEXT tests (T4): real entities through TextSync →
//! measure → TextCommit → extract_buiy_glyphs → BuiyAtlas → the coverage
//! draw — the real-producer replacement for atlas_gpu.rs's deleted
//! test-as-producer fills (glyph-pipeline § 12 GPU a/b/c). All #[ignore]:
//! need a wgpu adapter (CLAUDE.md GPU lane).
//!
//! Run: cargo test -p buiy_core --test text_gpu -- --ignored --test-threads=1
#![allow(deprecated)] // TEMPORARY (Phase 1a.9): perceptual_diff deprecated; this file migrates to buiy_verify::metric::compare in 1a.10, which removes this allow.

mod support;

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::{AtlasBitmap, AtlasConfig, AtlasFormat, AtlasKey, BuiyAtlas};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::TextColor;
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::{
    FamilyEntry, FontFamily, FontSize, FontStack, GenericFamily, ResidentTextKeys, Text,
};
use std::borrow::Cow;

const W: u32 = 128;
const H: u32 = 64;
const TOKEN: &str = "test.text";

/// One big themed line ("Hi", 40 px — thick stems guarantee full-coverage
/// interior texels) under a sized column root. Returns the text entity
/// (the churn twin mutates it).
fn spawn_text_fixture(app: &mut App, color: Color) -> Entity {
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TOKEN.into(), color);
    }
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(40.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TOKEN))),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .height_px(H as f32),
        ))
        .add_child(text);
    text
}

/// Build app → fixture → capture the first text-ready frame.
fn capture(color: Color) -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = support::gpu_render_app(W, H);
    spawn_text_fixture(&mut app, color);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    // wait_for_fonts, realized (Task 6): producer emitted + queue drained +
    // every key resident — warm_atlas is structural (§ 6.4).
    support::wait_for_text_ready(&mut app, 60);
    support::readback_rgba(&mut app, target)
}

/// Brightest painted pixel ≈ a full-coverage texel of the tint.
fn brightest(pixels: &[u8]) -> [u8; 4] {
    pixels
        .chunks_exact(4)
        .max_by_key(|p| p[0] as u32 + p[1] as u32 + p[2] as u32)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .unwrap()
}

// --- (a) gate-#2 hello-text: first painted frame, deterministic. ----------
#[test]
#[ignore = "needs a wgpu adapter; gate-#2 hello-text inline golden (glyph-pipeline § 12 GPU a)"]
fn hello_text_first_frame_is_deterministic_and_tinted() {
    let tint = Color::srgba(0.10, 0.85, 0.30, 1.0);
    let frame_a = capture(tint);

    // Backdrop reads the opaque-black clear; something painted.
    assert_eq!(support::px(&frame_a, W, W - 2, H - 2), [0, 0, 0, 255]);
    assert!(
        frame_a.chunks_exact(4).any(|p| p != [0, 0, 0, 255]),
        "the glyphs painted at least one pixel"
    );

    // Alpha-as-color: a full-coverage stroke-interior texel reads exactly
    // the linearized instance tint (atlas stores coverage, never color).
    let lin = LinearRgba::from(tint);
    let expected = support::expected_full_coverage_srgb([lin.red, lin.green, lin.blue, lin.alpha]);
    let got = brightest(&frame_a);
    const TOL: i32 = 4;
    for ch in 0..3 {
        assert!(
            (got[ch] as i32 - expected[ch] as i32).abs() <= TOL,
            "brightest texel channel {ch}: got {} expected {} (±{TOL}) — full \
             pixel got={got:?} expected={expected:?}",
            got[ch],
            expected[ch],
        );
    }

    // gate-#2 determinism: an independent fresh capture matches (the
    // stored-PNG machinery stays deferred; the re-capture IS the golden).
    let frame_b = capture(tint);
    let diff = perceptual_diff(&frame_a, &frame_b);
    assert!(
        diff < 1e-4,
        "two fresh captures diverged: perceptual_diff = {diff}"
    );
}

// --- (b) retint byte-identity with REAL text (§ 7's contract). ------------
#[test]
#[ignore = "needs a wgpu adapter; retint byte-identity with real text (glyph-pipeline § 12 GPU b)"]
fn retint_real_text_leaves_atlas_byte_identical() {
    let mut app = support::gpu_render_app(W, H);
    spawn_text_fixture(&mut app, Color::srgba(0.85, 0.10, 0.10, 1.0));
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let frame_a = support::readback_rgba(&mut app, target.clone());
    let atlas_a = coverage_page0_bytes(&app);

    // Theme swap: theme.is_changed() re-fires the § 6.2 gate; instances
    // re-emit with the new color; the atlas must not move.
    app.world_mut()
        .resource_mut::<buiy_core::theme::Theme>()
        .colors
        .insert(TOKEN.into(), Color::srgba(0.10, 0.20, 0.90, 1.0));
    for _ in 0..3 {
        app.update();
    }
    let frame_b = support::readback_rgba(&mut app, target);
    let atlas_b = coverage_page0_bytes(&app);

    assert_eq!(
        atlas_a, atlas_b,
        "CoverageR8 page byte-identical across the retint — tint is \
         per-instance, never a key input (§ 5.1/§ 7)"
    );
    assert!(
        perceptual_diff(&frame_a, &frame_b) > 5e-4,
        "the retint is visible in the framebuffer (byte-identity is not vacuous)"
    );
}

fn coverage_page0_bytes(app: &App) -> Vec<u8> {
    app.get_sub_app(RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<BuiyAtlas>()
        .page_pixels(AtlasFormat::CoverageR8, 0)
        .expect("a coverage page exists after the producer ran")
        .to_vec()
}

// --- (c) eviction-under-retention: the § 6.3 hazard, both halves. ---------
#[test]
#[ignore = "needs a wgpu adapter; eviction-under-retention regression (glyph-pipeline § 12 GPU c)"]
fn touch_pass_prevents_stale_uv_corruption() {
    let mut app = support::gpu_render_app(W, H);
    // Short grace so idling past it is cheap. Replace BEFORE any frame runs.
    {
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        render_app
            .world_mut()
            .insert_resource(BuiyAtlas::new(AtlasConfig {
                page_size: 1024,
                page_budget: 8,
                eviction_grace: 3,
            }));
    }
    spawn_text_fixture(&mut app, Color::srgba(0.90, 0.90, 0.20, 1.0));
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let frame_a = support::readback_rgba(&mut app, target.clone());

    // Half 1 — touch pass ON (production shape): idle ≫ grace, keys stay
    // resident, pixels stay put.
    for _ in 0..12 {
        app.update();
    }
    let keys: Vec<AtlasKey> = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app
            .world()
            .resource::<ResidentTextKeys>()
            .keys
            .clone()
    };
    assert!(!keys.is_empty());
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        for key in &keys {
            assert!(
                atlas.get(key).is_some(),
                "touch pass kept the visible key resident"
            );
        }
    }
    let frame_b = support::readback_rgba(&mut app, target.clone());
    assert!(
        perceptual_diff(&frame_a, &frame_b) < 1e-4,
        "retained frames render identically"
    );

    // Half 2 — the hazard a DISABLED touch pass would allow, simulated
    // (decision 7: no prod flag — we force the eviction directly): evict a
    // still-referenced key, insert a same-size filler (guillotiere reuses
    // the freed cell — asserted), never damage the main world, and watch
    // the retained instances' stale UVs sample the filler.
    let victim = keys[0].clone();
    let old_px = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app
            .world()
            .resource::<BuiyAtlas>()
            .get(&victim)
            .unwrap()
            .px
    };
    {
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        let mut atlas = render_app.world_mut().resource_mut::<BuiyAtlas>();
        atlas.evict_for_test(&victim);
        let size = old_px.size();
        let filler = atlas.get_or_insert(
            AtlasKey::from_bytes(b"eviction-hazard-filler"),
            AtlasFormat::CoverageR8,
            move || AtlasBitmap {
                size,
                format: AtlasFormat::CoverageR8,
                data: vec![0xFF; (size.x * size.y) as usize],
            },
        );
        assert_eq!(
            filler.px, old_px,
            "the filler reused the freed cell — the aliasing the hazard is made of"
        );
    }
    for _ in 0..2 {
        app.update();
    }
    // Guard: no rebuild re-rasterized the victim (retention really held).
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        assert!(
            render_app
                .world()
                .resource::<BuiyAtlas>()
                .get(&victim)
                .is_none(),
            "no rebuild occurred during the hazard window"
        );
    }
    let frame_c = support::readback_rgba(&mut app, target);
    assert!(
        perceptual_diff(&frame_a, &frame_c) > 1e-4,
        "stale UVs sampled the filler — the silent corruption § 6.3's \
         un-gated touch pass exists to prevent"
    );
}

// --- (d) T5: the multi-script golden (campaign: "1–2 goldens"). ----------
#[test]
#[ignore = "needs a wgpu adapter; T5 multi-script golden (verification § 1.3 pixels row)"]
fn multi_script_text_renders_deterministically() {
    // Two RTL lines through the fixture fonts — Arabic (joining) and the
    // mixed-BiDi string (Hebrew + Latin) — registered via the production
    // bytes path. Inline-golden discipline (the T4 stored-PNG deferral
    // stands): capture twice in two independent app instances, assert
    // byte-stability + non-emptiness. Glyph correctness lives headless in
    // the corpus; THIS test proves the pixels lane end-to-end (resolver →
    // set_rich_text → rasterize → atlas → draw) with non-Latin faces.
    fn capture_bidi() -> Vec<u8> {
        let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
        let mut app = support::gpu_render_app(W, H);
        // Finish BEFORE registering: `register_fixture_font` settles one
        // update, and a pre-finish update would run the render schedule
        // without the device/PipelineCache (both land in `finish`).
        support::finish_and_run(&mut app, 0);
        support::register_fixture_font(&mut app, "Noto Sans Arabic", "NotoSansArabic-arabic.ttf");
        support::register_fixture_font(&mut app, "Noto Sans Hebrew", "NotoSansHebrew-hebrew.ttf");

        {
            let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
            theme
                .colors
                .insert(TOKEN.into(), Color::srgba(0.92, 0.92, 0.92, 1.0));
        }
        // The joining-RTL line: every glyph sits on the Arabic fixture face.
        let arabic = app
            .world_mut()
            .spawn((
                Node,
                Style::default(),
                Text(String::from("السلام عليكم")),
                FontFamily(FontStack(vec![
                    FamilyEntry::Named(String::from("Noto Sans Arabic")),
                    FamilyEntry::Generic(GenericFamily::SansSerif),
                ])),
                FontSize(20.0),
                TextColor(ColorToken::Token(Cow::Borrowed(TOKEN))),
            ))
            .id();
        // The verification § 2.2 mixed-BiDi string. Latin must hit the
        // embedded face: the stack leads with the Hebrew fixture and the
        // resolver's coverage split sends "hello"/"world" to sans-serif.
        let bidi = app
            .world_mut()
            .spawn((
                Node,
                Style::default(),
                Text(String::from("hello עולם world")),
                FontFamily(FontStack(vec![
                    FamilyEntry::Named(String::from("Noto Sans Hebrew")),
                    FamilyEntry::Generic(GenericFamily::SansSerif),
                ])),
                FontSize(20.0),
                TextColor(ColorToken::Token(Cow::Borrowed(TOKEN))),
            ))
            .id();
        app.world_mut()
            .spawn((
                Node,
                Style::default()
                    .flex_column()
                    .width_px(W as f32)
                    .height_px(H as f32),
            ))
            .add_child(arabic)
            .add_child(bidi);

        let target = support::render_to_image(&mut app, W, H);
        support::spawn_capture_camera(&mut app, target.clone());
        support::wait_for_text_ready(&mut app, 60);
        support::readback_rgba(&mut app, target)
    }
    let a = capture_bidi();
    let b = capture_bidi();
    assert!(
        !a.chunks_exact(4).all(|p| p == &a[0..4]),
        "something painted"
    );
    assert!(
        perceptual_diff(&a, &b) < 1e-4,
        "two independent captures are byte-stable (deterministic fonts + resolver)"
    );
}

// --- (e) T5: THE rebuild-storm bound (font-assets §§ 3.2, 10). ------------
#[test]
#[ignore = "needs a wgpu adapter; T5 rebuild-storm bound (one frame of misses, baseline restored)"]
fn font_db_rebuild_storm_is_bounded() {
    // A fresh-db swap reissues EVERY fontdb ID: every AtlasKey goes stale
    // at once. Bounded, not broken: one frame of misses re-rasterizes,
    // old entries grace-evict, page count and entry count return to
    // baseline, pixels never change (same font bytes, same shaping).
    let mut app = support::gpu_render_app(W, H);
    {
        // Tight grace so the settle window is test-sized (the T4 fixture's
        // AtlasConfig override pattern).
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        render_app
            .world_mut()
            .insert_resource(BuiyAtlas::new(AtlasConfig {
                page_size: 1024,
                page_budget: 8,
                eviction_grace: 3,
            }));
    }
    spawn_text_fixture(&mut app, Color::srgba(0.9, 0.9, 0.2, 1.0));
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let frame_before = support::readback_rgba(&mut app, target.clone());
    let (entries_before, pages_before, keys_before) = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        (
            atlas.live_entry_count(),
            atlas.page_count(AtlasFormat::CoverageR8),
            render_app
                .world()
                .resource::<ResidentTextKeys>()
                .keys
                .clone(),
        )
    };

    // Trigger the swap through the production path: a completed scan task
    // carrying a FRESH registered-baseline db (same bytes — pixels must
    // not move; only the IDs do).
    let task = bevy::tasks::AsyncComputeTaskPool::get()
        .spawn(async move { buiy_core::text::registered_fonts_db() });
    app.world_mut()
        .insert_resource(buiy_core::text::PendingSystemFontScan(Some(task)));

    // The storm frame(s): swap applies → generation+lineage bump → sweep
    // reshape → producer rebuild, interner reseat, full re-rasterize.
    app.update();
    app.update();
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        let keys_after = &render_app.world().resource::<ResidentTextKeys>().keys;
        assert!(!keys_after.is_empty());
        assert!(
            keys_after.iter().all(|k| !keys_before.contains(k)),
            "every key re-seated (fresh lineage = fresh font u32s)"
        );
        assert!(
            atlas.live_entry_count() > entries_before,
            "old entries still grace-resident mid-storm (the double-resident window)"
        );
    }

    // Settle past the grace window: baseline restored, pixels identical.
    for _ in 0..8 {
        app.update();
    }
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        assert_eq!(
            atlas.live_entry_count(),
            entries_before,
            "entry count returned to baseline"
        );
        assert_eq!(
            atlas.page_count(AtlasFormat::CoverageR8),
            pages_before,
            "page count returned to baseline (the campaign's bound)"
        );
    }
    let frame_after = support::readback_rgba(&mut app, target);
    assert!(
        perceptual_diff(&frame_before, &frame_after) < 1e-4,
        "the storm is invisible: same bytes, same shaping, same pixels"
    );
}

// --- (f) T9: the gate-#15 GPU churn twin (verification §§ 1.3, 4). --------
#[test]
#[ignore = "needs a wgpu adapter; gate-#15 typing-churn fixture (verification §§ 1.3, 4)"]
fn typing_churn_is_bounded_and_invisible() {
    // The headless fixture (text_typing_churn.rs) owns the gate-#15
    // entry/page/key mechanism every PR (T9 plan D2); THIS twin re-asserts
    // the § 1.3 pixels half through the REAL rasterize → upload → draw
    // path: the frame after churn-and-settle is byte-stable against the
    // frame before churn (stale-UV/upload corruption under churn is
    // GPU-observable only), and the GPU-side counters return to baseline.
    let mut app = support::gpu_render_app(W, H);
    {
        // Tight grace so the settle window is test-sized (the T4 fixture's
        // AtlasConfig override pattern). Replace BEFORE any frame runs.
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        render_app
            .world_mut()
            .insert_resource(BuiyAtlas::new(AtlasConfig {
                page_size: 1024,
                page_budget: 8,
                eviction_grace: 3,
            }));
    }
    let text = spawn_text_fixture(&mut app, Color::srgba(0.2, 0.8, 0.9, 1.0));
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let frame_before = support::readback_rgba(&mut app, target.clone());
    let (entries_before, pages_before) = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        (
            atlas.live_entry_count(),
            atlas.page_count(AtlasFormat::CoverageR8),
        )
    };

    // The edit loop — text_typing_churn.rs's sequence: every string is
    // letter-disjoint from the "Hi" baseline AND from every other step, so
    // each edit inserts fresh atlas keys (real churn, never re-touches of
    // resident entries); the last edit returns to the baseline string (the
    // ε = 0 premise below).
    let edits = ["dgq", "hkx", "mvz", "rtw", "ufy", "jpn", "els", "Hi"];
    for (i, s) in edits.iter().copied().enumerate() {
        app.world_mut().get_mut::<Text>(text).expect("Text").0 = String::from(s);
        app.update();
        if i == edits.len() / 2 {
            // Non-vacuity, built in: mid-loop the atlas really grew past
            // the baseline (fresh inserts + grace-resident stale entries).
            let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
            assert!(
                render_app
                    .world()
                    .resource::<BuiyAtlas>()
                    .live_entry_count()
                    > entries_before,
                "mid-loop entry count exceeds baseline — the fixture churned"
            );
        }
    }

    // Settle past the grace window: every churned key goes untouched and
    // grace-evicts. ε = 0 (T9 plan D2): the loop ended ON the baseline
    // string, so the counters must return EXACTLY (the rebuild-storm
    // assert_eq idiom).
    for _ in 0..8 {
        app.update();
    }
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        assert_eq!(
            atlas.live_entry_count(),
            entries_before,
            "entry count returned to baseline after the churn (gate #15)"
        );
        assert_eq!(
            atlas.page_count(AtlasFormat::CoverageR8),
            pages_before,
            "page count returned to baseline (pages pooled, never leaked)"
        );
    }

    // The pixels half: same final text, same pixels — the churn is
    // invisible through the real upload/draw path.
    let frame_after = support::readback_rgba(&mut app, target);
    let diff = perceptual_diff(&frame_before, &frame_after);
    assert!(
        diff < 1e-4,
        "the churn is invisible: frame byte-stable across churn-and-settle \
         (perceptual_diff = {diff})"
    );
}
