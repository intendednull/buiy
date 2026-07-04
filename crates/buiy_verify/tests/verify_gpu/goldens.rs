//! Tier-5 end-to-end goldens per residue class (Phase 3.9, verification-design
//! `goldens.md` § Verification #7). All `#[ignore]` — they need a wgpu adapter
//! (real GPU locally / pinned lavapipe in CI). The headless gate stays green
//! WITHOUT these.
//!
//! Run (assert against the committed corpus):
//!     cargo test -p buiy_verify --test verify_gpu -- --ignored --test-threads=1 goldens
//!
//! Bless / re-bless the committed corpus (then REVIEW the PNG diff + commit):
//!     BUIY_BLESS=1 cargo test -p buiy_verify --test verify_gpu -- --ignored \
//!         --test-threads=1 goldens
//!
//! ## What each test proves
//!
//! * `golden_round_trip_on_real_adapter` — the **self-verifying** machinery
//!   proof (needs no committed PNG): capture a deterministic scene, bless it to
//!   a TEMP corpus, re-capture + assert it passes, then assert a
//!   deliberately-different image FAILS and emits a diff-PNG + HTML triage
//!   report containing the expected sections. This is the bless→pass→fail→report
//!   cycle on the real adapter.
//! * `golden_ahem_layout_class` — the **committed** Ahem layout-class golden:
//!   double-asserts byte-identity across two fresh captures AND equality to the
//!   stored positive (the box-font collapse holds).
//! * `golden_sdf_corner` — the committed residue golden for the irreducible SDF
//!   corner AA rim. Its committed-baseline EXACT comparison is **adapter-gated**
//!   (`support::on_pinned_lavapipe`): it runs only on the pinned lavapipe (the
//!   rasterizer the corpus is blessed against) and skips-as-pending on any other
//!   adapter (this host's RX diverges — cross-rasterizer pixels are
//!   non-comparable). The non-vacuous paint check runs on every adapter.
//!
//! * `golden_shadow_blur_kernel` — the committed drop-shadow Gaussian blur-kernel
//!   residue golden (an offset `BoxShadow` whose AA falloff is Tier-5's). The
//!   `BoxShadow` extract/draw path landed (`resolve_shadows` in `extract.rs`,
//!   `shadow.wgsl`), so this is now a real committed golden — adapter-gated on
//!   pinned lavapipe like `golden_sdf_corner`.
//!
//! The color-emoji fidelity golden remains a deferred follow-up: it waits on the
//! color-glyph render leg (`SwashContent::Color` is still `SkipColorEmoji`; no
//! color `IconInstance` producer/shader) and a pinned bundled COLR/CBDT emoji
//! font (goldens.md § "Color emoji is the canonical irreducible golden").

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::{
    Background, Border, BorderSide, BoxShadow, Corners, LineStyle, Radius, Shadow, TextColor,
};
use buiy_core::render::golden::Dpr;
use buiy_core::text::{FamilyEntry, FontFamily, FontSize, FontStack, Text};
use buiy_verify::determinism::DeterministicApp;
use buiy_verify::golden::{
    Backend, BlessMode, GoldenKey, GoldenOutcome, assert_golden, assert_golden_in, check_golden_in,
};
use buiy_verify::metric::{CompareOpts, FuzzBudget, compare};
use buiy_verify::support::on_pinned_lavapipe;
use image::{Rgba, RgbaImage};

/// The one pinned rasterizer cell today (CLAUDE.md: local RX 6700 XT / CI
/// lavapipe both compare rasterizer-internally; the corpus is keyed `Lavapipe`
/// as the canonical CI rasterizer — one golden per cell).
const BACKEND: Backend = Backend::Lavapipe;

fn key(widget: &str, state: &str, theme: &str, viewport: &str, dpr: Dpr) -> GoldenKey {
    GoldenKey {
        widget: widget.into(),
        state: state.into(),
        theme: theme.into(),
        viewport: viewport.into(),
        // The residue goldens capture at default user preferences (forced-colors
        // off); a forced-colors residue fixture would be a separate cell.
        forced_colors: false,
        backend: BACKEND,
        dpr,
    }
}

// --- fixtures ------------------------------------------------------------------

/// A rounded opaque fill on a black ground — exercises the SDF corner AA rim,
/// the irreducible residue Tier-5 owns (the CPU oracle cross-checks coverage but
/// not the GPU's analytic rim pixels).
fn rounded_rect(app: &mut App) {
    let fill = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(6.0)),
                    left: Sizing::Length(Length::px(6.0)),
                    ..default()
                })
                .width_px(36.0)
                .height_px(28.0),
            Background {
                color: ColorToken::Custom(Color::srgb(0.20, 0.65, 0.90)),
            },
            // The rounded corners (SDF rim residue) live on `Border.radius`;
            // a zero-width border still rounds the background fill's clip.
            Border {
                radius: Corners::all(Radius::circular(8.0)),
                ..Default::default()
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[fill]);
}

/// Ahem box-glyph text on black — the layout-determinism class: every glyph a
/// solid em-square, so the capture is byte-identical across hosts.
fn ahem_text(app: &mut App) {
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontFamily(FontStack(vec![FamilyEntry::Named(String::from("Ahem"))])),
            FontSize(24.0),
            TextColor(ColorToken::Custom(Color::srgb(0.95, 0.40, 0.20))),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(48.0)
                .height_px(48.0),
        ))
        .add_child(text);
}

// ---------------------------------------------------------------------------
// The self-verifying round-trip — the headline GPU proof. Needs NO committed
// PNG: blesses to a temp corpus, asserts pass, then asserts a different image
// fails + emits a triage report. (goldens.md § Verification, the bless→pass→
// fail→report cycle.)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn golden_round_trip_on_real_adapter() {
    let corpus = temp_dir("round-trip-corpus");
    let report_root = temp_dir("round-trip-report");
    let key = key("rect", "default", "dark", "sm", Dpr::X1);

    // 1) Capture a deterministic scene and BLESS it to the temp corpus.
    let captured = DeterministicApp::new(48, 40).capture(rounded_rect);
    // Non-vacuous: the scene actually painted (not a blank frame).
    assert!(
        captured.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the rounded-rect fixture painted at least one non-clear pixel"
    );
    let blessed = check_golden_in(
        &corpus,
        &report_root,
        BlessMode::Bless { replace: None },
        &key,
        &captured,
        &FuzzBudget::EXACT,
    );
    assert!(
        matches!(
            blessed,
            GoldenOutcome::Blessed {
                positive: 0,
                was_new: true
            }
        ),
        "bless wrote positive 0, got {blessed:?}"
    );

    // 2) Re-capture the SAME scene and ASSERT it passes (no bless) at EXACT.
    //    The determinism pin makes the re-capture bit-identical, so the stored
    //    golden matches at (0, 0).
    let recaptured = DeterministicApp::new(48, 40).capture(rounded_rect);
    assert_golden_in(
        &corpus,
        &report_root,
        BlessMode::Assert,
        &key,
        &recaptured,
        &FuzzBudget::EXACT,
    );

    // 3) A deliberately-DIFFERENT image must FAIL and emit a diff-PNG + HTML
    //    triage report containing the expected sections.
    let mut tampered = recaptured.clone();
    paint_block(&mut tampered, [255, 0, 255, 255]); // a 6×6 magenta block, unmissable
    let outcome = check_golden_in(
        &corpus,
        &report_root,
        BlessMode::Assert,
        &key,
        &tampered,
        &FuzzBudget::EXACT,
    );
    let report_path = match outcome {
        GoldenOutcome::Fail {
            best: Some((0, diff)),
            report,
        } => {
            assert!(diff.differing_pixels >= 36, "the magenta block differs");
            report
        }
        other => panic!("expected Fail against the blessed positive, got {other:?}"),
    };

    // The diff-PNG was written next to the report.
    let stem = key.slug().rsplit('/').next().unwrap().to_string();
    let diff_png = report_root.join(format!("{stem}.diff.png"));
    assert!(
        diff_png.exists(),
        "the diff-PNG heatmap was written: {diff_png:?}"
    );
    // It decodes as a real image of the capture's dimensions.
    let decoded = image::open(&diff_png).expect("diff-PNG decodes").to_rgba8();
    assert_eq!(decoded.dimensions(), tampered.dimensions());

    // The HTML triage report exists and contains the expected sections.
    assert!(report_path.exists(), "the HTML triage report was written");
    let html = std::fs::read_to_string(&report_path).expect("report readable");
    assert!(html.contains("<!doctype html>"), "well-formed HTML");
    assert!(
        html.contains(&key.slug()),
        "the failing cell is labeled by its key"
    );
    assert!(
        html.contains("data:image/png;base64,"),
        "PNGs are base64-inlined"
    );
    assert!(
        html.contains("diff heatmap"),
        "the diff-heatmap view is present"
    );
    assert!(
        html.contains("type=\"range\""),
        "the overlay slider is present"
    );
    // Offline-first: no network reference.
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "the report is offline-first (no external URL)"
    );
}

// ---------------------------------------------------------------------------
// Committed residue goldens. These ASSERT against the in-git corpus under
// tests/goldens/ and are BLESSED once with BUIY_BLESS=1 (the PNG reviewed +
// committed). They fail closed if the corpus is missing — run the bless command
// above, REVIEW the PNG, and commit.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn golden_ahem_layout_class() {
    // The Ahem layout-class golden DOUBLE-ASSERTS the box-font collapse:
    //   (a) two fresh captures are byte-identical (re-capture determinism), AND
    //   (b) the capture equals the stored positive.
    let a = DeterministicApp::new(48, 48).capture(ahem_text);
    let b = DeterministicApp::new(48, 48).capture(ahem_text);
    let diff = compare(&a, &b, &CompareOpts::default());
    assert!(
        diff.passes(&FuzzBudget::EXACT),
        "two fresh Ahem captures diverged (the box-font collapse must hold) — \
         differing_pixels={}",
        diff.differing_pixels,
    );
    assert!(
        a.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the Ahem text painted (non-vacuous)"
    );
    // (b) equality to the stored positive (fails closed if unblessed).
    assert_golden(
        &key("text-ahem", "default", "dark", "sm", Dpr::X1),
        &a,
        &FuzzBudget::EXACT,
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn golden_sdf_corner() {
    // The SDF corner AA residue: a rounded fill whose analytic rim pixels are
    // exactly what Tier-5 owns (beyond the CPU coverage oracle).
    let img = DeterministicApp::new(48, 40).capture(rounded_rect);
    // The rasterizer-internal, adapter-AGNOSTIC leg: the fixture actually
    // painted (a non-blank frame). This runs on EVERY adapter — it is the part
    // of the test that is sound off lavapipe (a blank-capture regression fails
    // here regardless of host).
    assert!(
        img.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the rounded rect painted (non-vacuous)"
    );

    // The committed-baseline EXACT comparison is keyed against the PINNED
    // lavapipe corpus (commit b869eba: this host's RX 6700 XT diverges by
    // max_channel_delta=35 — cross-rasterizer pixels are non-comparable). Gate
    // it on the selected adapter actually being lavapipe; OTHERWISE skip-as-
    // pending (mirror matrix_goldens), so the documented local `--ignored` lane
    // on real hardware no longer hard-fails (audit #7; determinism.md § "the
    // local lane does not compare against the stored lavapipe baseline").
    if !on_pinned_lavapipe() {
        eprintln!(
            "golden_sdf_corner: selected adapter is not the pinned lavapipe — \
             SKIPPING the committed-baseline EXACT comparison (cross-rasterizer \
             pixels are non-comparable; the stored corpus is blessed against \
             lavapipe only). The non-vacuous paint check above still ran."
        );
        return;
    }
    assert_golden(
        &key("rect-rounded", "default", "dark", "sm", Dpr::X1),
        &img,
        &FuzzBudget::EXACT,
    );
}

/// A 24×24 light box at (6,6) casting an OFFSET drop shadow (+12,+12, blur 6) in
/// a bright color, so the shadow's Gaussian blur falloff paints on the black
/// clear in the bottom-right region the box does not cover — the residue this
/// golden owns. Mirrors `render_border_shadow_gpu`'s fixture geometry.
fn box_shadow(app: &mut App) {
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(6.0)),
                    left: Sizing::Length(Length::px(6.0)),
                    ..default()
                })
                .width_px(24.0)
                .height_px(24.0),
            Background {
                color: ColorToken::Custom(Color::srgb(0.85, 0.85, 0.85)),
            },
            BoxShadow(vec![Shadow {
                color: ColorToken::Custom(Color::srgba(0.20, 0.55, 0.95, 0.95)),
                offset_x: Length::px(12.0),
                offset_y: Length::px(12.0),
                blur: Length::px(6.0),
                spread: Length::px(0.0),
                inset: false,
            }]),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn golden_shadow_blur_kernel() {
    // The drop-shadow Gaussian blur-kernel residue: the AA falloff of an offset
    // BoxShadow is what Tier-5 owns — the algebraic CPU shadow oracle and the
    // adapter-tolerant "region darkens" GPU check pin the geometry, not the
    // rasterized blur pixels.
    let img = DeterministicApp::new(48, 48).capture(box_shadow);
    // The adapter-AGNOSTIC leg (runs on EVERY adapter): the fixture painted a
    // non-blank frame — a blank-capture / shadow-vanished regression fails here
    // regardless of host.
    assert!(
        img.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the box + shadow painted (non-vacuous)"
    );

    // The committed-baseline EXACT comparison is keyed against the PINNED
    // lavapipe corpus (this host's RX diverges — cross-rasterizer pixels are
    // non-comparable). Gate it on the selected adapter actually being lavapipe;
    // otherwise skip-as-pending (mirror golden_sdf_corner / matrix_goldens).
    if !on_pinned_lavapipe() {
        eprintln!(
            "golden_shadow_blur_kernel: selected adapter is not the pinned \
             lavapipe — SKIPPING the committed-baseline EXACT comparison. The \
             non-vacuous paint check above still ran."
        );
        return;
    }
    assert_golden(
        &key("shadow", "default", "dark", "sm", Dpr::X1),
        &img,
        &FuzzBudget::EXACT,
    );
}

// --- helpers -------------------------------------------------------------------

/// Paint a 6×6 block of `rgba` into the top-left — an unmissable deliberate
/// difference for the fail path.
fn paint_block(img: &mut RgbaImage, rgba: [u8; 4]) {
    let (w, h) = img.dimensions();
    for y in 0..6.min(h) {
        for x in 0..6.min(w) {
            img.put_pixel(x, y, Rgba(rgba));
        }
    }
}

/// A unique temp dir per call (no `tempfile` dep; mirrors the persistence
/// tests' pattern).
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("buiy-goldens/{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// --- F1: the bordered-rounded card (first-establishment reference) -------------

/// One solid ink border side (the card's visible outline color).
fn card_ink_side() -> BorderSide {
    BorderSide {
        color: ColorToken::Custom(Color::srgb(0.10, 0.12, 0.16)),
        style: LineStyle::Solid,
    }
}

/// A widget card: a rounded opaque FILL under a VISIBLE (3px) rounded border —
/// the bordered-rounded class. Until F3 lands the fill's own corner radius the
/// fill corners are SQUARE under the rounded band (the "ears" F4b fixes), so this
/// fixture is the first-establishment reference the ears fix will later re-bless.
/// Uses `ColorToken::Custom` for fill + border so it resolves in every theme.
fn bordered_rounded_card(app: &mut App) {
    let card = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(6.0)),
                    left: Sizing::Length(Length::px(6.0)),
                    ..default()
                })
                .width_px(36.0)
                .height_px(28.0)
                .border(3.0),
            Background {
                color: ColorToken::Custom(Color::srgb(0.85, 0.90, 0.98)),
            },
            Border {
                left: card_ink_side(),
                right: card_ink_side(),
                top: card_ink_side(),
                bottom: card_ink_side(),
                radius: Corners::all(Radius::circular(8.0)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[card]);
}

// The bordered-rounded establishment golden (F1). The standing regression guard
// for the rounded-fill corner is the Tier-4 SDF cross-check (zero stored bytes,
// tests/verify_gpu/sdf_cross_check_gpu.rs); this stored golden is scoped to the
// first-establishment-vs-design capture the cross-check cannot see (spec §2.1 /
// finding H1). The committed baseline captures TODAY's faithful state — the fill
// corners are square under the rounded band (the "ears") until F3 lands the
// fill's own corner radius — so it is a documented re-bless target for F3 (fill
// radius) and F4b (ears fix). Blessed against the pinned lavapipe rasterizer
// (Mesa 24.3.4 / LLVM 18.1.8); off lavapipe the exact comparison is skipped and
// only the non-vacuous paint check runs. Re-bless with:
//   BUIY_BLESS=1 cargo test -p buiy_verify --test verify_gpu -- --ignored \
//       --test-threads=1 goldens::golden_card_bordered
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn golden_card_bordered() {
    let img = DeterministicApp::new(48, 40).capture(bordered_rounded_card);
    // Non-vacuous on EVERY adapter: the card painted (border ink + fill), so a
    // blank-capture regression fails here regardless of host.
    assert!(
        img.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the bordered-rounded card painted (non-vacuous)"
    );

    // The committed-baseline EXACT comparison is keyed to the pinned lavapipe
    // corpus; on any other adapter (e.g. this host's RX 6700 XT / RADV) the rim
    // AA pixels are non-comparable, so skip-as-pending there (the non-vacuous
    // paint check above still ran). Mirrors golden_sdf_corner.
    if !on_pinned_lavapipe() {
        eprintln!(
            "golden_card_bordered: selected adapter is not the pinned lavapipe — \
             SKIPPING the committed-baseline EXACT comparison (cross-rasterizer \
             pixels are non-comparable). The non-vacuous paint check above ran."
        );
        return;
    }
    assert_golden(
        &key("card-bordered", "default", "dark", "sm", Dpr::X1),
        &img,
        &FuzzBudget::EXACT,
    );
}
