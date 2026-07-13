//! GPU end-to-end TEXT tests (T4): real entities through TextSync →
//! measure → TextCommit → extract_buiy_glyphs → BuiyAtlas → the coverage
//! draw — the real-producer replacement for atlas_gpu.rs's deleted
//! test-as-producer fills (glyph-pipeline § 12 GPU a/b/c). All #[ignore]:
//! need a wgpu adapter (CLAUDE.md GPU lane).
//!
//! Run: cargo test -p buiy_core --test text_gpu -- --ignored --test-threads=1

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::atlas::{
    AtlasBitmap, AtlasConfig, AtlasFormat, AtlasKey, BuiyAtlas, GlyphAlphaInstance,
};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Icon, TextColor};
use buiy_core::render::golden::GoldenConfig;
use buiy_core::render::icon_producer::{ExtractedIcons, icon_atlas_key};
use buiy_core::render::icon_raster::ICON_VIEWBOX;
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::text::{
    FamilyEntry, FontFamily, FontSize, FontStack, GenericFamily, ResidentTextKeys, Text,
};
use buiy_verify::metric::{CompareOpts, FuzzBudget, compare};

const W: u32 = 128;
const H: u32 = 64;

/// Wrap a raw RGBA readback (W×H) as an `RgbaImage` for `metric::compare`.
fn img(bytes: &[u8]) -> image::RgbaImage {
    image::RgbaImage::from_raw(W, H, bytes.to_vec()).expect("readback length == W*H*4")
}

/// The stable-recapture spelling: two fresh captures of the same scene must
/// agree bit-exactly within the pinned rasterizer (metric.md § re-capture
/// determinism). `FuzzBudget::EXACT` is `(0, 0)`.
fn assert_stable(a: &[u8], b: &[u8], msg: &str) {
    let d = compare(&img(a), &img(b), &CompareOpts::default());
    assert!(d.passes(&FuzzBudget::EXACT), "{msg}");
}

/// The anti-test spelling: two captures must NOT match at the exact budget —
/// proof the input change actually moved pixels (metric.md § anti-tests).
fn assert_differs(a: &[u8], b: &[u8], msg: &str) {
    let d = compare(&img(a), &img(b), &CompareOpts::default());
    assert!(!d.passes(&FuzzBudget::EXACT), "{msg}");
}

/// One big themed line ("Hi", 40 px — thick stems guarantee full-coverage
/// interior texels) under a sized column root. Returns the text entity
/// (the churn twin mutates it).
fn spawn_text_fixture(app: &mut App, color: Color) -> Entity {
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(40.0),
            TextColor(ColorToken::Custom(color)),
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
    let mut app = crate::support::gpu_render_app(W, H);
    spawn_text_fixture(&mut app, color);
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    // wait_for_fonts, realized (Task 6): producer emitted + queue drained +
    // every key resident — warm_atlas is structural (§ 6.4).
    crate::support::wait_for_text_ready(&mut app, 60);
    crate::support::readback_rgba(&mut app, target)
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
    assert_eq!(
        crate::support::px(&frame_a, W, W - 2, H - 2),
        [0, 0, 0, 255]
    );
    assert!(
        frame_a.chunks_exact(4).any(|p| p != [0, 0, 0, 255]),
        "the glyphs painted at least one pixel"
    );

    // Alpha-as-color: a full-coverage stroke-interior texel reads exactly
    // the linearized instance tint (atlas stores coverage, never color).
    let lin = LinearRgba::from(tint);
    let expected =
        crate::support::expected_full_coverage_srgb([lin.red, lin.green, lin.blue, lin.alpha]);
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
    assert_stable(
        &frame_a,
        &frame_b,
        "two fresh captures diverged (must be bit-exact within the pinned rasterizer)",
    );
}

// --- (b) retint byte-identity with REAL text (§ 7's contract). ------------
#[test]
#[ignore = "needs a wgpu adapter; retint byte-identity with real text (glyph-pipeline § 12 GPU b)"]
fn retint_real_text_leaves_atlas_byte_identical() {
    let mut app = crate::support::gpu_render_app(W, H);
    let text = spawn_text_fixture(&mut app, Color::srgba(0.85, 0.10, 0.10, 1.0));
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame_a = crate::support::readback_rgba(&mut app, target.clone());
    let atlas_a = coverage_page0_bytes(&app);

    // Recolor swap: Changed<TextColor> re-fires the § 6.2 gate; instances
    // re-emit with the new color; the atlas must not move.
    app.world_mut()
        .get_mut::<TextColor>(text)
        .expect("the fixture's TextColor")
        .0 = ColorToken::Custom(Color::srgba(0.10, 0.20, 0.90, 1.0));
    for _ in 0..3 {
        app.update();
    }
    let frame_b = crate::support::readback_rgba(&mut app, target);
    let atlas_b = coverage_page0_bytes(&app);

    assert_eq!(
        atlas_a, atlas_b,
        "CoverageR8 page byte-identical across the retint — tint is \
         per-instance, never a key input (§ 5.1/§ 7)"
    );
    assert_differs(
        &frame_a,
        &frame_b,
        "the retint is visible in the framebuffer (byte-identity is not vacuous)",
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
    let mut app = crate::support::gpu_render_app(W, H);
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
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame_a = crate::support::readback_rgba(&mut app, target.clone());

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
    let frame_b = crate::support::readback_rgba(&mut app, target.clone());
    assert_stable(&frame_a, &frame_b, "retained frames render identically");

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
    let frame_c = crate::support::readback_rgba(&mut app, target);
    assert_differs(
        &frame_a,
        &frame_c,
        "stale UVs sampled the filler — the silent corruption § 6.3's \
         un-gated touch pass exists to prevent",
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
        let mut app = crate::support::gpu_render_app(W, H);
        // Finish BEFORE registering: `register_fixture_font` settles one
        // update, and a pre-finish update would run the render schedule
        // without the device/PipelineCache (both land in `finish`).
        crate::support::finish_and_run(&mut app, 0);
        crate::support::register_fixture_font(
            &mut app,
            "Noto Sans Arabic",
            "NotoSansArabic-arabic.ttf",
        );
        crate::support::register_fixture_font(
            &mut app,
            "Noto Sans Hebrew",
            "NotoSansHebrew-hebrew.ttf",
        );

        let ink = Color::srgba(0.92, 0.92, 0.92, 1.0);
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
                TextColor(ColorToken::Custom(ink)),
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
                TextColor(ColorToken::Custom(ink)),
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

        let target = crate::support::render_to_image(&mut app, W, H);
        crate::support::spawn_capture_camera(&mut app, target.clone());
        crate::support::wait_for_text_ready(&mut app, 60);
        crate::support::readback_rgba(&mut app, target)
    }
    let a = capture_bidi();
    let b = capture_bidi();
    assert!(
        !a.chunks_exact(4).all(|p| p == &a[0..4]),
        "something painted"
    );
    assert_stable(
        &a,
        &b,
        "two independent captures are byte-stable (deterministic fonts + resolver)",
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
    let mut app = crate::support::gpu_render_app(W, H);
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
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame_before = crate::support::readback_rgba(&mut app, target.clone());
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
    let frame_after = crate::support::readback_rgba(&mut app, target);
    assert_stable(
        &frame_before,
        &frame_after,
        "the storm is invisible: same bytes, same shaping, same pixels",
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
    let mut app = crate::support::gpu_render_app(W, H);
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
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame_before = crate::support::readback_rgba(&mut app, target.clone());
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
    let frame_after = crate::support::readback_rgba(&mut app, target);
    assert_stable(
        &frame_before,
        &frame_after,
        "the churn is invisible: frame byte-stable across churn-and-settle",
    );
}

// ===========================================================================
// Multi-page coverage atlas bind — RED census + recreate proofs
// (docs/plans/2026-07-09-multipage-coverage-atlas-bind.md, Tasks 0.1 + 0.2).
//
// The coverage atlas is paged; a glyph/icon resident on page ≥1 carries its
// page in `GlyphAlphaInstance.page`. PRE-FIX the GPU binds only page 0 and
// `coverage.wgsl` discards `page`, so page-≥1 content samples page 0 at a
// foreign UV and renders wrong (the Dooduel "empty chat pill"). These tests
// force overflow with a tiny 64-texel page budget + solid full-coverage
// fillers (deterministic page control, and a crisp RED: a page-≥1 UV
// mis-sampled against the solid page 0 paints a SOLID over-count, not the real
// sparse glyph), pin a probe glyph + a stroke icon on page ≥1, and compare
// their lit-pixel footprint against the SAME content rendered ALONE on page 0
// (correct at any code state — page 0 is always bound right). Post-fix the
// footprints match; pre-fix they diverge.
// ===========================================================================

/// A 64-texel page so a handful of solid 32² fillers overflow page 0 cheaply
/// (the `crosscut/atlas_gpu.rs` `AtlasConfig` trick), with a huge grace so
/// nothing drains mid-test. `page_budget` stays 8 (never approached).
const OVERFLOW_PAGE: u32 = 64;

/// The stroke-icon fixture forced onto a page ≥1 (§ 5.1 icon coverage): an "X"
/// of two line segments — SPARSE ink, so a solid mis-sample (pre-fix) vastly
/// over-counts vs the real strokes.
const PROBE_ICON_PATH: &str = "M6 6 L18 18 M18 6 L6 18";
const PROBE_ICON_STROKE: f32 = 2.4;

fn overflow_atlas_config() -> AtlasConfig {
    AtlasConfig {
        page_size: OVERFLOW_PAGE,
        page_budget: 8,
        eviction_grace: 1_000_000,
    }
}

/// [`crate::support::gpu_render_app`] with the coverage atlas swapped for the
/// tiny-page overflow config (replaced BEFORE `finish`, the T4 fixture idiom).
fn overflow_app(w: u32, h: u32) -> App {
    let mut app = crate::support::gpu_render_app(w, h);
    {
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        render_app
            .world_mut()
            .insert_resource(BuiyAtlas::new(overflow_atlas_config()));
    }
    app
}

/// Insert `n` full-coverage (0xFF) 32×32 `CoverageR8` cells directly into the
/// render-world atlas (the eviction-hazard test's direct-insert idiom). Four
/// exactly tile a 64² page → page 0 is fully SOLID, so a page-≥1 UV
/// mis-sampled against it (the pre-fix bug) reads coverage 1.0 everywhere.
fn insert_solid_fillers(app: &mut App, n: u8) {
    let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
    let mut atlas = render_app.world_mut().resource_mut::<BuiyAtlas>();
    for i in 0..n {
        atlas.get_or_insert(
            AtlasKey::from_bytes(&[0xF1, i]),
            AtlasFormat::CoverageR8,
            || AtlasBitmap {
                size: UVec2::new(32, 32),
                format: AtlasFormat::CoverageR8,
                data: vec![0xFF; 32 * 32],
            },
        );
    }
}

/// Spawn a single absolutely-positioned white text node at `(left, top)`
/// (the `render_patch_upload_gpu.rs` absolute-row idiom), parented under a bare
/// root so layout resolves.
fn spawn_abs_text(app: &mut App, s: &str, size: f32, left: f32, top: f32) {
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default().absolute().inset(Inset {
                top: Sizing::Length(Length::px(top)),
                left: Sizing::Length(Length::px(left)),
                ..default()
            }),
            Text(String::from(s)),
            FontSize(size),
            TextColor(ColorToken::Custom(Color::WHITE)),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_child(text);
}

/// Spawn the stroke-icon fixture absolutely at `(left, top)` in a `size×size`
/// box (the `render_icon_gpu.rs` idiom), tinted white.
fn spawn_abs_icon(app: &mut App, size: u16, left: f32, top: f32) {
    let icon = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(top)),
                    left: Sizing::Length(Length::px(left)),
                    ..default()
                })
                .width_px(size as f32)
                .height_px(size as f32),
            Icon {
                path_d: PROBE_ICON_PATH.to_string(),
                stroke_width: PROBE_ICON_STROKE,
                size_px: size,
                viewbox: ICON_VIEWBOX,
                fill: false,
                color: ColorToken::Custom(Color::WHITE),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_child(icon);
}

/// Drive frames until the icon producer has emitted its instance (icons carry
/// no `ResidentTextKeys`, so `wait_for_text_ready` cannot gate them). Panics
/// past `max`.
fn wait_for_icon_ready(app: &mut App, max: usize) {
    for _ in 0..max {
        app.update();
        if let Some(ic) = crate::support::render_world_resource::<ExtractedIcons>(app)
            && !ic.icons.is_empty()
        {
            return;
        }
    }
    panic!("icon never became atlas-resident within {max} frames");
}

/// The one glyph instance in a single-glyph scene (asserts exactly one).
fn sole_glyph(app: &App) -> GlyphAlphaInstance {
    let g = crate::support::render_world_resource::<ExtractedGlyphs>(app).expect("ExtractedGlyphs");
    assert_eq!(g.glyphs.len(), 1, "exactly one glyph instance in the scene");
    g.glyphs[0]
}

/// The atlas page of the scene's single probe glyph, via its own resident key
/// (§ 5.1 "precondition on the probe specifically" — NOT aggregate page_count).
fn sole_glyph_page(app: &App) -> u16 {
    let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
    let keys = &render_app.world().resource::<ResidentTextKeys>().keys;
    assert_eq!(keys.len(), 1, "exactly one probe glyph key");
    render_app
        .world()
        .resource::<BuiyAtlas>()
        .get(&keys[0])
        .expect("probe glyph resident")
        .page
}

fn atlas_page_count(app: &App) -> usize {
    app.get_sub_app(RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<BuiyAtlas>()
        .page_count(AtlasFormat::CoverageR8)
}

/// Count lit pixels (any channel `>= 128`, adapter-robust) inside a logical-px
/// rect `[x, y, w, h]` of the capture — the AA-robust footprint (§ 5.1: a COUNT
/// in a pinned rect, not "non-zero ink" which false-passes when a page-≥1 UV
/// lands on other ink).
fn lit_count_in_rect(pixels: &[u8], img_w: u32, img_h: u32, rect: [f32; 4]) -> usize {
    let x0 = rect[0].floor().clamp(0.0, img_w as f32) as u32;
    let y0 = rect[1].floor().clamp(0.0, img_h as f32) as u32;
    let x1 = (rect[0] + rect[2]).ceil().clamp(0.0, img_w as f32) as u32;
    let y1 = (rect[1] + rect[3]).ceil().clamp(0.0, img_h as f32) as u32;
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = crate::support::px(pixels, img_w, x, y);
            if crate::support::channel_lit(p[0])
                || crate::support::channel_lit(p[1])
                || crate::support::channel_lit(p[2])
            {
                count += 1;
            }
        }
    }
    count
}

/// Render one glyph ALONE (guaranteed page 0) at a fixed absolute rect; return
/// its instance rect + lit-pixel footprint. The reference the overflow render
/// is matched against — correct at any code state.
fn glyph_alone_footprint(
    s: &str,
    size: f32,
    left: f32,
    top: f32,
    cw: u32,
    ch: u32,
) -> ([f32; 4], usize) {
    let mut app = overflow_app(cw, ch);
    let target = crate::support::render_to_image(&mut app, cw, ch);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    spawn_abs_text(&mut app, s, size, left, top);
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    assert_eq!(atlas_page_count(&app), 1, "the alone glyph sits on page 0");
    assert_eq!(sole_glyph_page(&app), 0, "the alone glyph sits on page 0");
    let rect = sole_glyph(&app).rect;
    let pixels = crate::support::readback_rgba(&mut app, target);
    let count = lit_count_in_rect(&pixels, cw, ch, rect);
    assert!(count > 0, "the alone glyph paints ink on page 0");
    (rect, count)
}

/// Render the stroke icon ALONE (page 0) at a fixed absolute box; return its
/// instance rect + lit-pixel footprint (the icon reference).
fn icon_alone_footprint(size: u16, left: f32, top: f32, cw: u32, ch: u32) -> ([f32; 4], usize) {
    let mut app = overflow_app(cw, ch);
    let target = crate::support::render_to_image(&mut app, cw, ch);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    spawn_abs_icon(&mut app, size, left, top);
    crate::support::finish_and_run(&mut app, 1);
    wait_for_icon_ready(&mut app, 60);
    let rect = {
        let ic =
            crate::support::render_world_resource::<ExtractedIcons>(&app).expect("ExtractedIcons");
        assert_eq!(ic.icons.len(), 1, "exactly one icon instance");
        ic.icons[0].rect
    };
    {
        let key = icon_atlas_key(
            PROBE_ICON_PATH,
            PROBE_ICON_STROKE,
            size,
            ICON_VIEWBOX,
            false,
        );
        let page = app
            .get_sub_app(RenderApp)
            .expect("RenderApp")
            .world()
            .resource::<BuiyAtlas>()
            .get(&key)
            .expect("icon resident")
            .page;
        assert_eq!(page, 0, "the alone icon sits on page 0");
    }
    let pixels = crate::support::readback_rgba(&mut app, target);
    let count = lit_count_in_rect(&pixels, cw, ch, rect);
    assert!(count > 0, "the alone icon paints ink on page 0");
    (rect, count)
}

/// Footprint-match tolerance: ±5% of the reference, floor ±4 px (identical
/// rasterization at an identical absolute position should match near-exactly).
fn footprint_tol(reference: usize) -> usize {
    (reference / 20).max(4)
}

// --- (0.1) GPU ink census: a probe glyph + an icon on page ≥1 render right. --
#[test]
#[ignore = "needs a wgpu adapter; multi-page coverage census (probe+icon on page ≥1 render their own texels)"]
fn coverage_pages_beyond_zero_render_their_own_texels() {
    const CW: u32 = 160;
    const CH: u32 = 120;
    const PROBE: &str = "L"; // tall, sparse — a solid mis-sample over-counts hard
    const PROBE_SIZE: f32 = 40.0;
    const PROBE_LEFT: f32 = 20.0;
    const PROBE_TOP: f32 = 16.0;
    const ICON_SIZE: u16 = 40;
    const ICON_LEFT: f32 = 20.0;
    const ICON_TOP: f32 = 70.0;

    // References: probe + icon rendered alone on page 0 (correct at any state).
    let (probe_rect, probe_ref) =
        glyph_alone_footprint(PROBE, PROBE_SIZE, PROBE_LEFT, PROBE_TOP, CW, CH);
    let (icon_rect, icon_ref) = icon_alone_footprint(ICON_SIZE, ICON_LEFT, ICON_TOP, CW, CH);

    // Overflow scene: fill page 0 SOLID, then the probe + icon are forced onto
    // page ≥1 (their own sole carriers).
    let mut app = overflow_app(CW, CH);
    let target = crate::support::render_to_image(&mut app, CW, CH);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    spawn_abs_text(&mut app, PROBE, PROBE_SIZE, PROBE_LEFT, PROBE_TOP);
    spawn_abs_icon(&mut app, ICON_SIZE, ICON_LEFT, ICON_TOP);
    crate::support::finish_and_run(&mut app, 0);
    insert_solid_fillers(&mut app, 4); // four 32² cells tile page 0 solid
    assert_eq!(
        atlas_page_count(&app),
        1,
        "the four fillers tile exactly one page before the producer runs"
    );
    crate::support::wait_for_text_ready(&mut app, 60);
    wait_for_icon_ready(&mut app, 60);

    // Precondition (§ 5.1): the probe glyph AND the icon really overflowed —
    // else the test is vacuous.
    assert!(
        sole_glyph_page(&app) > 0,
        "the probe glyph must be forced onto a page ≥1 (tune the scene if not)"
    );
    let icon_page = {
        let key = icon_atlas_key(
            PROBE_ICON_PATH,
            PROBE_ICON_STROKE,
            ICON_SIZE,
            ICON_VIEWBOX,
            false,
        );
        app.get_sub_app(RenderApp)
            .expect("RenderApp")
            .world()
            .resource::<BuiyAtlas>()
            .get(&key)
            .expect("icon resident")
            .page
    };
    assert!(icon_page > 0, "the icon must be forced onto a page ≥1");

    let pixels = crate::support::readback_rgba(&mut app, target);
    let probe_got = lit_count_in_rect(&pixels, CW, CH, probe_rect);
    let icon_got = lit_count_in_rect(&pixels, CW, CH, icon_rect);

    // The decisive assertions: page-≥1 content renders its OWN texels, so its
    // footprint matches the page-0 alone reference. Pre-fix each samples the
    // SOLID page 0 → a solid over-count → these FAIL.
    assert!(
        probe_got.abs_diff(probe_ref) <= footprint_tol(probe_ref),
        "probe glyph on page {} must render its own texels: got {probe_got} lit px, \
         reference (page 0) {probe_ref} (pre-fix samples the solid page 0 → over-count)",
        sole_glyph_page(&app),
    );
    assert!(
        icon_got.abs_diff(icon_ref) <= footprint_tol(icon_ref),
        "icon on page {icon_page} must render its own texels: got {icon_got} lit px, \
         reference (page 0) {icon_ref} (pre-fix samples the solid page 0 → over-count)",
    );
}

// --- (0.2) The array recreate re-uploads ALL pages (the § 5.2 second growth). -
#[test]
#[ignore = "needs a wgpu adapter; coverage array recreate re-uploads all pages (clean page 0 survives a 1→2 growth)"]
fn coverage_array_recreate_reuploads_clean_pages() {
    const CW: u32 = 160;
    const CH: u32 = 80;
    // Node A: a small glyph that shares page 0 with the fillers. Node B: a
    // tall, sparse glyph (height 40 > the 32-tall quadrant) forced onto page 1
    // — the SECOND growth, fired while page 0 is resident + CLEAN.
    const A: &str = "x";
    const A_SIZE: f32 = 22.0;
    const A_LEFT: f32 = 12.0;
    const A_TOP: f32 = 14.0;
    const B: &str = "L";
    const B_SIZE: f32 = 52.0;
    const B_LEFT: f32 = 96.0;
    const B_TOP: f32 = 12.0;

    // Alone references (each on page 0).
    let (a_rect, a_ref) = glyph_alone_footprint(A, A_SIZE, A_LEFT, A_TOP, CW, CH);
    let (b_rect, b_ref) = glyph_alone_footprint(B, B_SIZE, B_LEFT, B_TOP, CW, CH);

    // Phase 1: node A onto page 0 alongside three solid fillers (they leave one
    // 32² quadrant free for A's small cell). Settle so page 0 uploads + goes
    // CLEAN, with only one page live.
    let mut app = overflow_app(CW, CH);
    let target = crate::support::render_to_image(&mut app, CW, CH);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    spawn_abs_text(&mut app, A, A_SIZE, A_LEFT, A_TOP);
    crate::support::finish_and_run(&mut app, 0);
    insert_solid_fillers(&mut app, 3); // three quadrants solid; one left for A
    crate::support::wait_for_text_ready(&mut app, 60);
    assert_eq!(
        atlas_page_count(&app),
        1,
        "phase 1: node A + fillers all fit page 0 (only one page live)"
    );
    // Idle frames so page 0's dirty flag clears BEFORE the growth fires.
    for _ in 0..4 {
        app.update();
    }

    // Phase 2: node B is forced onto a fresh page 1 (page 0 is full) — the GPU
    // array grows 1→2 while page 0 is CLEAN, so the recreate MUST re-upload
    // page 0 from scratch (a fresh array texture has no residual contents).
    spawn_abs_text(&mut app, B, B_SIZE, B_LEFT, B_TOP);
    crate::support::wait_for_text_ready(&mut app, 60);
    assert_eq!(
        atlas_page_count(&app),
        2,
        "phase 2: node B forced a SECOND page (the 1→2 growth under test)"
    );

    let pixels = crate::support::readback_rgba(&mut app, target);
    let a_got = lit_count_in_rect(&pixels, CW, CH, a_rect);
    let b_got = lit_count_in_rect(&pixels, CW, CH, b_rect);

    // Post-fix (correct): BOTH render. Node A (page 0, clean) proves the
    // recreate re-uploaded ALL pages — a dirty-gated recreate would drop clean
    // page 0 and node A would blank. Node B (page 1) proves the multi-page
    // bind; pre-fix it samples the solid page 0 → a solid over-count → FAIL.
    assert!(
        a_got.abs_diff(a_ref) <= footprint_tol(a_ref),
        "page-0 node A must survive the array recreate (re-upload-ALL, not \
         dirty-gated): got {a_got} lit px, reference {a_ref}",
    );
    assert!(
        b_got.abs_diff(b_ref) <= footprint_tol(b_ref),
        "page-1 node B must render its own page: got {b_got} lit px, reference \
         {b_ref} (pre-fix samples the solid page 0 → over-count)",
    );
}
