//! The golden comparison entry points (`goldens.md` § "`assert_golden`").
//!
//! [`check_golden`] compares a freshly captured `actual` against the stored
//! **multi-positive** baseline set for a key and returns a structured
//! [`GoldenOutcome`] (pass / fail / blessed) — the no-panic core used by the
//! harness's own tests and the coverage matrix driver. [`assert_golden`] is the
//! panicking wrapper a test calls: it fails closed on a missing or non-matching
//! corpus and, under `BUIY_BLESS=1`, blesses instead (modeled exactly on
//! `BUIY_ACCEPT_SHAPING`, never a silent overwrite).
//!
//! ## Multi-positive semantics
//!
//! A key maps to a *set* of accepted PNGs, not one (Skia-Gold "many positives
//! per config"). `check_golden` compares `actual` against each positive and
//! passes if **any** `Diff::passes(budget)`. This absorbs the residual GPU AA
//! jitter the determinism pin reduces but does not eliminate. On a fail it
//! carries the *best* (smallest-`Diff`) candidate so the triage report shows the
//! closest baseline, not an arbitrary one.

use super::GoldenKey;
use super::ledger::{BlessLedger, Positive};
use super::report::{TriageCard, TriageReport};
use crate::metric::{CompareOpts, Diff, FuzzBudget, compare};
use image::RgbaImage;

/// The default corpus root (`crates/buiy_verify/tests/goldens/`) and the
/// triage-report output dir (`target/buiy-goldens/`), resolved from the crate
/// manifest so they are stable regardless of the test's CWD.
pub(crate) fn default_corpus_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

/// Number of committed positive baselines for `key` in the default corpus
/// (`tests/goldens/`). `0` ⇒ the key is un-blessed (**bless-on-demand**): a
/// matrix/coverage driver should treat the cell as *pending*, not a failure. A
/// non-zero count means a committed golden exists, so a fresh capture MUST
/// still match it — the fail-closed contract holds for blessed keys. This lets
/// the GPU coverage lane stay green over an intentionally-partial residue
/// corpus while still catching drift on every cell that has been blessed.
pub fn committed_positives(key: &GoldenKey) -> usize {
    let dir = key.dir(&default_corpus_root());
    BlessLedger::load_or_empty(&ledger_path(&dir), key)
        .map(|l| l.positives.len())
        .unwrap_or(0)
}

fn report_dir() -> std::path::PathBuf {
    // `CARGO_TARGET_DIR` honored if set; else the workspace `target/`. We keep
    // it simple and stable: `<manifest>/../../target/buiy-goldens`.
    std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
        .join("buiy-goldens")
}

/// The structured result of a golden comparison (no panic). `assert_golden`
/// wraps this with the fail-closed panic + bless behavior.
#[derive(Debug)]
pub enum GoldenOutcome {
    /// `actual` matched a stored positive within `budget`. Carries which
    /// positive matched and its `Diff` (the smallest, since match is
    /// any-positive).
    Pass {
        /// Index of the matched positive (`<stem>.<matched_positive>.png`).
        matched_positive: usize,
        /// The `Diff` against the matched positive.
        diff: Diff,
    },
    /// No positive matched (or the corpus was empty). `best` is the closest
    /// candidate `(index, Diff)` if any positive exists; `report` is the written
    /// HTML triage report path.
    Fail {
        /// The closest stored positive `(index, Diff)`, or `None` for an empty
        /// corpus (the missing-golden case).
        best: Option<(usize, Diff)>,
        /// Path to the written HTML triage report.
        report: std::path::PathBuf,
    },
    /// `BUIY_BLESS=1`: wrote a new (or replaced an existing) positive. Never
    /// reached in CI (the env is unset there, mirroring `BUIY_ACCEPT_SHAPING`).
    Blessed {
        /// Index of the written positive.
        positive: usize,
        /// `true` if a new positive was appended; `false` if one was replaced.
        was_new: bool,
    },
}

/// How a check should treat the corpus: compare-and-gate, or bless `actual` as
/// a positive. Resolving the bless decision into an explicit value (instead of
/// reading `BUIY_BLESS` deep in the comparison) keeps the policy out of the
/// process-global env so the harness's own tests — and the Phase-4 coverage
/// matrix driver — can drive bless/assert deterministically without env races.
/// The env is read **once**, at the public entry point ([`check_golden`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlessMode {
    /// Compare against the corpus and gate (the CI / default path).
    Assert,
    /// Write `actual` as a positive. `Some(i)` replaces positive `i`; `None`
    /// appends a new one (`BUIY_BLESS` set, optional `BUIY_BLESS_REPLACE=<i>`).
    Bless {
        /// `Some(i)` overwrites positive `i`; `None` appends a new positive.
        replace: Option<usize>,
    },
}

/// Resolve the bless mode from the environment — the **single** place
/// `BUIY_BLESS` / `BUIY_BLESS_REPLACE` are read (accept-FILE switch, modeled on
/// `BUIY_ACCEPT_SHAPING`).
fn mode_from_env() -> BlessMode {
    if std::env::var_os("BUIY_BLESS").is_some() {
        BlessMode::Bless {
            replace: std::env::var("BUIY_BLESS_REPLACE")
                .ok()
                .and_then(|v| v.parse().ok()),
        }
    } else {
        BlessMode::Assert
    }
}

/// Compare `actual` against the stored multi-positive baseline for `key` at the
/// default corpus root, gated by `budget`. Under `BUIY_BLESS=1` this *blesses*
/// (writes `actual` as a positive + updates the ledger) and returns
/// [`GoldenOutcome::Blessed`]. Otherwise it returns [`Pass`](GoldenOutcome::Pass)
/// on an any-positive match, or [`Fail`](GoldenOutcome::Fail) (writing the
/// diff-PNG + HTML triage report) on a miss or empty corpus.
pub fn check_golden(key: &GoldenKey, actual: &RgbaImage, budget: &FuzzBudget) -> GoldenOutcome {
    check_golden_in(
        &default_corpus_root(),
        &report_dir(),
        mode_from_env(),
        key,
        actual,
        budget,
    )
}

/// The corpus-root- and mode-parameterized core of [`check_golden`] — lets the
/// harness's own tests (and the Phase-4 coverage matrix driver) bless/assert
/// against an explicit corpus root + report dir + [`BlessMode`], with **no**
/// env races. `corpus_root` holds the `<key.slug()>/<stem>.<n>.png` positives +
/// `<stem>.toml` ledgers; `report_root` receives the diff-PNG + HTML triage
/// report on a fail.
pub fn check_golden_in(
    corpus_root: &std::path::Path,
    report_root: &std::path::Path,
    mode: BlessMode,
    key: &GoldenKey,
    actual: &RgbaImage,
    budget: &FuzzBudget,
) -> GoldenOutcome {
    let dir = key.dir(corpus_root);
    let ledger_path = ledger_path(&dir);

    if let BlessMode::Bless { replace } = mode {
        return bless(&dir, &ledger_path, replace, key, actual, budget);
    }

    let ledger = BlessLedger::load_or_empty(&ledger_path, key)
        .unwrap_or_else(|e| panic!("corrupt golden ledger {ledger_path:?}: {e}"));

    // Compare against every positive; pass on the FIRST that clears the budget,
    // tracking the smallest-Diff candidate for the report on a miss.
    let mut best: Option<(usize, Diff)> = None;
    for (i, positive) in ledger.positives.iter().enumerate() {
        let png_path = dir.join(&positive.file);
        let baseline = load_png(&png_path)
            .unwrap_or_else(|e| panic!("golden positive {png_path:?} unreadable: {e}"));
        // emit_diff_image only on the candidate we end up reporting; here we run
        // the cheap (no heatmap) compare to gate, and recompute the heatmap for
        // the best candidate below only if we fail.
        let diff = compare(actual, &baseline, &CompareOpts::default());
        if diff.passes(budget) {
            return GoldenOutcome::Pass {
                matched_positive: i,
                diff,
            };
        }
        let smaller = best
            .as_ref()
            .map(|(_, bd)| diff_score(&diff) < diff_score(bd))
            .unwrap_or(true);
        if smaller {
            best = Some((i, diff));
        }
    }

    // FAIL (miss or empty corpus): write the diff-PNG + append a triage card.
    let report = emit_failure_report(report_root, &dir, key, actual, &ledger, budget, &best);
    GoldenOutcome::Fail { best, report }
}

/// A scalar ranking for "closest baseline": differing pixels dominate, channel
/// delta breaks ties. Lower is closer.
fn diff_score(d: &Diff) -> u64 {
    (d.differing_pixels as u64) << 8 | d.max_channel_delta as u64
}

/// Bless `actual`: write it as a positive PNG + record it in the ledger. With
/// `replace = Some(i)` it overwrites positive `i`; otherwise it appends a new
/// positive. **The human then reviews the PNG in the PR and commits it.**
fn bless(
    dir: &std::path::Path,
    ledger_path: &std::path::Path,
    replace: Option<usize>,
    key: &GoldenKey,
    actual: &RgbaImage,
    budget: &FuzzBudget,
) -> GoldenOutcome {
    std::fs::create_dir_all(dir).expect("create golden corpus dir");
    let mut ledger = BlessLedger::load_or_empty(ledger_path, key).expect("load ledger for bless");

    let stem = slug_stem(key);
    let (index, was_new) = match replace {
        Some(i) if i < ledger.positives.len() => (i, false),
        _ => (ledger.positives.len(), true),
    };
    let file = format!("{stem}.{index}.png");
    actual
        .save(dir.join(&file))
        .expect("write blessed golden PNG");

    let positive = Positive {
        file,
        blessed_commit: git_head_commit(),
        blessed_at: now_rfc3339(),
        budget: *budget,
        reason: std::env::var("BUIY_BLESS_REASON").unwrap_or_else(|_| "blessed".into()),
    };
    if was_new {
        ledger.positives.push(positive);
    } else {
        ledger.positives[index] = positive;
    }
    ledger.save(ledger_path).expect("write golden ledger");
    GoldenOutcome::Blessed {
        positive: index,
        was_new,
    }
}

/// Compare `actual` against the corpus and **panic** on a non-bless failure with
/// the bless instruction (fail closed; the `BUIY_ACCEPT_SHAPING` panic shape).
/// Under `BUIY_BLESS=1` it blesses and returns. This is the entry point a
/// `#[test]` calls.
pub fn assert_golden(key: &GoldenKey, actual: &RgbaImage, budget: &FuzzBudget) {
    match check_golden(key, actual, budget) {
        GoldenOutcome::Pass { .. } | GoldenOutcome::Blessed { .. } => {}
        GoldenOutcome::Fail { best, report } => panic_fail(key, best.as_ref(), &report),
    }
}

/// [`assert_golden`] against an explicit corpus root + report dir + mode — the
/// no-env-race variant the harness's own fail-closed test drives.
pub fn assert_golden_in(
    corpus_root: &std::path::Path,
    report_root: &std::path::Path,
    mode: BlessMode,
    key: &GoldenKey,
    actual: &RgbaImage,
    budget: &FuzzBudget,
) {
    match check_golden_in(corpus_root, report_root, mode, key, actual, budget) {
        GoldenOutcome::Pass { .. } | GoldenOutcome::Blessed { .. } => {}
        GoldenOutcome::Fail { best, report } => panic_fail(key, best.as_ref(), &report),
    }
}

/// The fail-closed panic message (shared by `assert_golden` and the corpus-root
/// test variant), pointing at the triage report and the bless command.
fn panic_fail(key: &GoldenKey, best: Option<&(usize, Diff)>, report: &std::path::Path) -> ! {
    let slug = key.slug();
    match best {
        None => panic!(
            "no golden committed for `{slug}` — run\n  \
             BUIY_BLESS=1 cargo test -p buiy_verify --test goldens -- --ignored \
             --test-threads=1\nthen REVIEW the captured PNG and commit it. \
             Triage report: {report:?}"
        ),
        Some((i, diff)) => panic!(
            "golden `{slug}` diverged from every positive (closest = positive {i}: \
             differing_pixels={dp}, max_channel_delta={mcd}). A pixel change is a \
             rendering change; if intended, regenerate with\n  \
             BUIY_BLESS=1 cargo test -p buiy_verify --test goldens -- --ignored \
             --test-threads=1\nreview the diff, and commit. Triage report: {report:?}",
            dp = diff.differing_pixels,
            mcd = diff.max_channel_delta,
        ),
    }
}

/// Write the diff-PNG for the closest candidate and append a card to the run's
/// HTML triage report. Returns the report path.
fn emit_failure_report(
    report_root: &std::path::Path,
    corpus_dir: &std::path::Path,
    key: &GoldenKey,
    actual: &RgbaImage,
    ledger: &BlessLedger,
    budget: &FuzzBudget,
    best: &Option<(usize, Diff)>,
) -> std::path::PathBuf {
    std::fs::create_dir_all(report_root).ok();
    let stem = slug_stem(key);

    // Recompute the diff WITH the heatmap against the closest baseline (the gate
    // pass above ran without a heatmap to stay cheap).
    let (baseline_img, diff) = match best {
        Some((i, _)) => {
            let png = corpus_dir.join(&ledger.positives[*i].file);
            let baseline = load_png(&png).unwrap_or_else(|_| RgbaImage::new(1, 1));
            let d = compare(
                actual,
                &baseline,
                &CompareOpts {
                    emit_diff_image: true,
                    ..CompareOpts::default()
                },
            );
            (baseline, d)
        }
        // Missing-golden: no baseline to diff against. Use a blank baseline and
        // a saturated-style diff so the card still renders.
        None => (
            RgbaImage::new(actual.width().max(1), actual.height().max(1)),
            compare(
                actual,
                &RgbaImage::new(actual.width().max(1), actual.height().max(1)),
                &CompareOpts {
                    emit_diff_image: true,
                    ..CompareOpts::default()
                },
            ),
        ),
    };

    // Write the standalone diff-PNG next to the report.
    let diff_png_path = report_root.join(format!("{stem}.diff.png"));
    let diff_png_bytes = if let Some(img) = &diff.diff_image {
        let _ = img.save(&diff_png_path);
        png_bytes(img)
    } else {
        Vec::new()
    };

    let report_path = report_root.join("report.html");
    let mut report = TriageReport::open_or_create(&report_path);
    report.push(TriageCard {
        key: key.clone(),
        actual_png: png_bytes(actual),
        baseline_png: png_bytes(&baseline_img),
        diff_png: diff_png_bytes,
        diff,
        budget: *budget,
    });
    report.write().ok();
    report_path
}

// --- small fs / format helpers -------------------------------------------------

/// The `<stem>` of a key's slug (the path tail, e.g. `light__md__fc0__lavapipe__dpr1`),
/// used to name `<stem>.<n>.png` and `<stem>.toml` inside the key dir.
fn slug_stem(key: &GoldenKey) -> String {
    key.slug()
        .rsplit('/')
        .next()
        .expect("slug always has a tail")
        .to_string()
}

/// The ledger path inside a key's corpus dir.
fn ledger_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join(format!(
        "{}.toml",
        dir.file_name().and_then(|s| s.to_str()).unwrap_or("ledger")
    ))
}

fn load_png(path: &std::path::Path) -> image::ImageResult<RgbaImage> {
    Ok(image::open(path)?.to_rgba8())
}

fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("encode PNG");
    buf.into_inner()
}

/// `git rev-parse HEAD` at bless time, or `"unknown"` if git is unavailable
/// (the bless still proceeds — the commit is provenance, not a gate).
fn git_head_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// An RFC3339 UTC timestamp WITHOUT pulling a date crate: `SystemTime` since the
/// epoch formatted as `1970-01-01T00:00:00Z + N seconds` is overkill; we emit
/// the epoch-second form `"<unix>s"` is not RFC3339, so compute the calendar
/// date by hand. Kept dependency-free (no `chrono`/`time`) per the spec's
/// minimal-dep ethos.
fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// Convert a Unix timestamp (seconds) to an RFC3339 UTC string. Civil-date
/// algorithm (Howard Hinnant's `days_from_civil` inverse) — dependency-free.
fn rfc3339_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Hinnant civil_from_days (epoch 1970-01-01 = day 0).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_matches_known_epoch_dates() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
        // 2026-06-15T00:00:00Z = 1_781_481_600 (verified against a calendar).
        assert_eq!(rfc3339_from_unix(1_781_481_600), "2026-06-15T00:00:00Z");
        // A non-midnight instant.
        assert_eq!(
            rfc3339_from_unix(1_781_481_600 + 3661),
            "2026-06-15T01:01:01Z"
        );
    }

    #[test]
    fn committed_positives_is_zero_for_an_unblessed_key() {
        // A key deliberately absent from the committed corpus has no ledger ⇒ 0
        // positives ⇒ the coverage matrix driver treats it as pending, not a
        // failure. (The blessed-key path is exercised by the GPU golden lane.)
        let key = GoldenKey {
            widget: "definitely-not-a-real-widget-xyz".into(),
            state: "none".into(),
            theme: "dark".into(),
            viewport: "sm".into(),
            forced_colors: false,
            backend: crate::golden::Backend::Lavapipe,
            dpr: buiy_core::render::golden::Dpr::X1,
        };
        assert_eq!(committed_positives(&key), 0);
    }
}
