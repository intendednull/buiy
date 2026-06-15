//! Tier-5 end-to-end goldens per residue class (Phase 3.9, verification-design
//! `goldens.md` § Verification #7). All `#[ignore]` — they need a wgpu adapter
//! (real GPU locally / pinned lavapipe in CI). The headless gate stays green
//! WITHOUT these.
//!
//! Run (assert against the committed corpus):
//!     cargo test -p buiy_verify --test goldens -- --ignored --test-threads=1
//!
//! Bless / re-bless the committed corpus (then REVIEW the PNG diff + commit):
//!     BUIY_BLESS=1 cargo test -p buiy_verify --test goldens -- --ignored \
//!         --test-threads=1
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
//!   corner AA rim.
//!
//! The drop-shadow-kernel residue golden is a deferred follow-up: the
//! `BoxShadow` extract/draw path is not yet landed (`extract_buiy_nodes` has no
//! `BoxShadow` branch — docs/plans/follow-ups.md). The harness is ready for it
//! (a shadow fixture + one `assert_golden` call); only the renderer leg is
//! missing. The color-emoji fidelity golden likewise waits on a pinned bundled
//! emoji font (goldens.md § "Color emoji is the canonical irreducible golden").

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::{Background, Border, Corners, Radius, TextColor};
use buiy_core::render::golden::Dpr;
use buiy_core::text::{FamilyEntry, FontFamily, FontSize, FontStack, Text};
use buiy_verify::determinism::DeterministicApp;
use buiy_verify::golden::{
    Backend, BlessMode, GoldenKey, GoldenOutcome, assert_golden, assert_golden_in, check_golden_in,
};
use buiy_verify::metric::{CompareOpts, FuzzBudget, compare};
use image::{Rgba, RgbaImage};
use std::borrow::Cow;

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
        backend: BACKEND,
        dpr,
    }
}

// --- fixtures ------------------------------------------------------------------

/// A rounded opaque fill on a black ground — exercises the SDF corner AA rim,
/// the irreducible residue Tier-5 owns (the CPU oracle cross-checks coverage but
/// not the GPU's analytic rim pixels).
fn rounded_rect(app: &mut App) {
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("g.fill".into(), Color::srgb(0.20, 0.65, 0.90));
    }
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
                color: ColorToken::Token(Cow::Borrowed("g.fill")),
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
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("g.text".into(), Color::srgb(0.95, 0.40, 0.20));
    }
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontFamily(FontStack(vec![FamilyEntry::Named(String::from("Ahem"))])),
            FontSize(24.0),
            TextColor(ColorToken::Token(Cow::Borrowed("g.text"))),
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
    assert!(
        img.pixels().any(|p| p.0 != [0, 0, 0, 255]),
        "the rounded rect painted (non-vacuous)"
    );
    assert_golden(
        &key("rect-rounded", "default", "dark", "sm", Dpr::X1),
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
