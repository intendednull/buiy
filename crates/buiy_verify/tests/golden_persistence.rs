//! Tier-5 golden persistence self-tests (Phase 3.7, verification-design
//! `goldens.md` § Verification #1–#4). All pure-CPU — synthetic `RgbaImage`s in
//! memory, a per-test temp corpus root, an explicit [`BlessMode`] (so the bless
//! decision never touches the process-global `BUIY_BLESS` env and tests cannot
//! race each other). No GPU adapter, runs under the headless gate.
//!
//! #1 match/mismatch        — `check_golden` Pass on an identical image, Fail on
//!                            a one-pixel-over-budget image.
//! #2 multi-positive        — bless two positives; an image matching the SECOND
//!                            returns `Pass { matched_positive: 1 }`.
//! #3 bless round-trip      — bless to a temp corpus, re-check without bless
//!                            passes, and the ledger records commit/timestamp/reason.
//! #4 fail-closed           — empty corpus + Assert mode ⇒ `assert_golden_in`
//!                            panics with the bless instruction.

use buiy_core::render::golden::Dpr;
use buiy_verify::golden::{
    Backend, BlessLedger, BlessMode, GoldenKey, GoldenOutcome, assert_golden_in, check_golden_in,
};
use buiy_verify::metric::FuzzBudget;
use image::{Rgba, RgbaImage};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// A unique temp corpus root per call — avoids cross-test collisions without a
/// `tempfile` dep (mirrors `reftest.rs`'s `std::env::temp_dir()` pattern).
fn temp_root(tag: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "buiy-golden-test/{tag}-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp corpus root");
    dir
}

fn key() -> GoldenKey {
    GoldenKey {
        widget: "rect".into(),
        state: "default".into(),
        theme: "dark".into(),
        viewport: "sm".into(),
        backend: Backend::Lavapipe,
        dpr: Dpr::X1,
    }
}

/// A solid-color test image.
fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(rgba))
}

/// `base` with exactly one pixel's red channel bumped by `delta` — a single
/// over-budget pixel for the mismatch case.
fn one_pixel_off(base: &RgbaImage, delta: u8) -> RgbaImage {
    let mut img = base.clone();
    let p = img.get_pixel(0, 0).0;
    img.put_pixel(0, 0, Rgba([p[0].wrapping_add(delta), p[1], p[2], p[3]]));
    img
}

// ---------------------------------------------------------------------------
// #1 — match / mismatch
// ---------------------------------------------------------------------------

#[test]
fn match_and_mismatch() {
    let root = temp_root("match");
    let report = temp_root("match-report");
    let key = key();
    let img = solid(16, 16, [10, 120, 200, 255]);

    // Bless the baseline, then check WITHOUT bless.
    let blessed = check_golden_in(
        &root,
        &report,
        BlessMode::Bless { replace: None },
        &key,
        &img,
        &FuzzBudget::EXACT,
    );
    assert!(matches!(
        blessed,
        GoldenOutcome::Blessed {
            positive: 0,
            was_new: true
        }
    ));

    // Identical image PASSES at EXACT.
    let pass = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &img,
        &FuzzBudget::EXACT,
    );
    assert!(
        matches!(
            pass,
            GoldenOutcome::Pass {
                matched_positive: 0,
                ..
            }
        ),
        "identical image must pass against positive 0, got {pass:?}"
    );

    // One pixel over budget FAILS at EXACT, carrying the closest candidate.
    let off = one_pixel_off(&img, 200);
    let fail = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &off,
        &FuzzBudget::EXACT,
    );
    match fail {
        GoldenOutcome::Fail {
            best: Some((0, diff)),
            report,
        } => {
            assert_eq!(diff.differing_pixels, 1, "exactly one over-budget pixel");
            assert!(report.exists(), "the triage report was written");
        }
        other => panic!("expected Fail{{ best: Some((0, _)) }}, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// #2 — multi-positive: any positive matches; an image matching the SECOND
//      returns Pass { matched_positive: 1 }.
// ---------------------------------------------------------------------------

#[test]
fn multi_positive_any_matches() {
    let root = temp_root("multi");
    let report = temp_root("multi-report");
    let key = key();

    let p0 = solid(16, 16, [10, 120, 200, 255]);
    // A genuinely DIFFERENT second positive (whole image a different color), so
    // p1 cannot accidentally match p0 at EXACT.
    let p1 = solid(16, 16, [200, 30, 30, 255]);

    check_golden_in(
        &root,
        &report,
        BlessMode::Bless { replace: None },
        &key,
        &p0,
        &FuzzBudget::EXACT,
    );
    check_golden_in(
        &root,
        &report,
        BlessMode::Bless { replace: None },
        &key,
        &p1,
        &FuzzBudget::EXACT,
    );

    // The ledger now has two positives.
    let ledger = load_ledger(&root, &key);
    assert_eq!(ledger.positives.len(), 2, "two positives blessed");

    // An image identical to the SECOND positive passes, matching index 1.
    let outcome = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &p1,
        &FuzzBudget::EXACT,
    );
    assert!(
        matches!(
            outcome,
            GoldenOutcome::Pass {
                matched_positive: 1,
                ..
            }
        ),
        "image matching the second positive must report matched_positive: 1, got {outcome:?}"
    );

    // An image matching the FIRST still passes (matched_positive: 0) — proves
    // any-positive, not last-positive.
    let outcome0 = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &p0,
        &FuzzBudget::EXACT,
    );
    assert!(matches!(
        outcome0,
        GoldenOutcome::Pass {
            matched_positive: 0,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// #3 — bless round-trip: bless, re-check passes, ledger records provenance.
// ---------------------------------------------------------------------------

#[test]
fn bless_round_trip() {
    let root = temp_root("bless");
    let report = temp_root("bless-report");
    let key = key();
    let img = solid(20, 12, [44, 88, 132, 255]);

    let outcome = check_golden_in(
        &root,
        &report,
        BlessMode::Bless { replace: None },
        &key,
        &img,
        &FuzzBudget::EXACT,
    );
    assert!(matches!(
        outcome,
        GoldenOutcome::Blessed {
            positive: 0,
            was_new: true
        }
    ));

    // The PNG and the ledger exist on disk.
    let dir = key.dir(&root);
    assert!(
        dir.join("dark__sm__lavapipe__dpr1.0.png").exists(),
        "blessed PNG written"
    );

    let ledger = load_ledger(&root, &key);
    assert_eq!(ledger.positives.len(), 1);
    let pos = &ledger.positives[0];
    assert_eq!(pos.file, "dark__sm__lavapipe__dpr1.0.png");
    assert_eq!(pos.budget, FuzzBudget::EXACT);
    assert!(!pos.reason.is_empty(), "a reason was recorded");
    // RFC3339-shaped timestamp (the harness emits `YYYY-MM-DDThh:mm:ssZ`).
    assert!(
        pos.blessed_at.len() == 20 && pos.blessed_at.ends_with('Z') && pos.blessed_at.contains('T'),
        "RFC3339 timestamp recorded, got {:?}",
        pos.blessed_at
    );
    // A commit string was recorded (a real hash inside the repo, else "unknown").
    assert!(!pos.blessed_commit.is_empty(), "a commit was recorded");

    // Re-check WITHOUT bless now passes.
    let pass = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &img,
        &FuzzBudget::EXACT,
    );
    assert!(
        matches!(
            pass,
            GoldenOutcome::Pass {
                matched_positive: 0,
                ..
            }
        ),
        "the blessed image passes on re-check, got {pass:?}"
    );
}

#[test]
fn bless_replace_overwrites_positive() {
    let root = temp_root("replace");
    let report = temp_root("replace-report");
    let key = key();
    let original = solid(16, 16, [10, 10, 10, 255]);
    let replacement = solid(16, 16, [240, 240, 240, 255]);

    check_golden_in(
        &root,
        &report,
        BlessMode::Bless { replace: None },
        &key,
        &original,
        &FuzzBudget::EXACT,
    );
    let replaced = check_golden_in(
        &root,
        &report,
        BlessMode::Bless { replace: Some(0) },
        &key,
        &replacement,
        &FuzzBudget::EXACT,
    );
    assert!(
        matches!(
            replaced,
            GoldenOutcome::Blessed {
                positive: 0,
                was_new: false
            }
        ),
        "replace targets positive 0 in place, got {replaced:?}"
    );
    // Still ONE positive (replaced, not appended).
    assert_eq!(load_ledger(&root, &key).positives.len(), 1);
    // The replacement is now the baseline; the original no longer matches.
    let now = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &replacement,
        &FuzzBudget::EXACT,
    );
    assert!(matches!(
        now,
        GoldenOutcome::Pass {
            matched_positive: 0,
            ..
        }
    ));
    let stale = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &original,
        &FuzzBudget::EXACT,
    );
    assert!(
        matches!(stale, GoldenOutcome::Fail { .. }),
        "the replaced-out original no longer matches"
    );
}

// ---------------------------------------------------------------------------
// #4 — fail-closed: empty corpus + Assert ⇒ panic with the bless instruction.
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "no golden committed")]
fn fail_closed_on_empty_corpus() {
    let root = temp_root("empty");
    let report = temp_root("empty-report");
    let key = key();
    let img = solid(16, 16, [0, 0, 0, 255]);
    // No positive blessed ⇒ assert_golden_in must panic instructing the dev to
    // bless + review + commit (the BUIY_ACCEPT_SHAPING fail-closed shape).
    assert_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &img,
        &FuzzBudget::EXACT,
    );
}

#[test]
fn check_golden_missing_returns_fail_with_no_best() {
    // The structured (no-panic) view of the missing case: empty corpus ⇒ Fail
    // with best == None (the "missing" outcome the coverage driver consumes).
    let root = temp_root("missing");
    let report = temp_root("missing-report");
    let key = key();
    let img = solid(16, 16, [0, 0, 0, 255]);
    let outcome = check_golden_in(
        &root,
        &report,
        BlessMode::Assert,
        &key,
        &img,
        &FuzzBudget::EXACT,
    );
    match outcome {
        GoldenOutcome::Fail { best: None, report } => {
            assert!(
                report.exists(),
                "a triage report is still emitted for a missing golden"
            );
        }
        other => panic!("expected Fail{{ best: None }} for an empty corpus, got {other:?}"),
    }
}

// --- helpers -------------------------------------------------------------------

fn load_ledger(root: &std::path::Path, key: &GoldenKey) -> BlessLedger {
    let dir = key.dir(root);
    let stem = key.slug().rsplit('/').next().unwrap().to_string();
    let path = dir.join(format!("{stem}.toml"));
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ledger {path:?} unreadable: {e}"));
    toml::from_str(&body).expect("ledger parses")
}
