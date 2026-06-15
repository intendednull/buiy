//! Tier 4 — reftests + the CPU-vs-GPU SDF cross-check (reftests.md).
//!
//! A reftest renders a `test` and a `reference` scene with the SAME engine in
//! ONE process and asserts their bitmaps match (`==`) or differ (`!=`), never
//! against a stored baseline — so every platform-variance term (driver SDF
//! rounding, glyph-atlas AA, sRGB encode, clock) cancels in the diff. The
//! harness stores ZERO bytes. GPU-coupled cases are `#[ignore]`; the pairing /
//! aggregation logic and the independence lint are pure-CPU and gate headless.

/// Whether a [`RefCase`] passes on equality or on difference.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefKind {
    /// Pass iff `test` and `reference` render to the same bitmap within `fuzz`.
    Match,
    /// Pass iff they render DIFFERENTLY (a `!=` anti-test guards silent no-ops).
    Mismatch,
}

impl RefKind {
    /// Parse the `reftest!` macro's kind token (`stringify!($kind)`).
    /// Panics on any other token — the macro only ever passes these two.
    pub fn reftest_kind(token: &str) -> Self {
        match token {
            "match" => RefKind::Match,
            "mismatch" => RefKind::Mismatch,
            other => panic!("reftest! kind must be `match` or `mismatch`, got `{other}`"),
        }
    }
}

use crate::metric::{Diff, FuzzBudget};
use bevy::app::App;

/// One reftest pairing. `test` and `reference` each build a scene into a
/// fresh, deterministic `App` (spawn entities; do NOT drive frames —
/// `run_reftest` owns the capture loop). Co-locate the expectation with the
/// `#[test]` the `reftest!` macro generates.
pub struct RefCase {
    pub name: &'static str,
    pub kind: RefKind,
    /// Builds the scene exercising the feature under test.
    pub test: fn(&mut App),
    /// Builds the independent-oracle scene (see "Reference independence").
    pub reference: fn(&mut App),
    /// Per-pairing fuzz, à la Mozilla `fuzzy-if`. Default `(0,0)` once the
    /// determinism stack is in (determinism.md); widen with a documented reason.
    pub fuzz: FuzzBudget,
}

/// The result of running one [`RefCase`].
#[derive(Debug)]
pub struct RefOutcome {
    pub passed: bool,
    pub diff: Diff,
    /// On failure, a self-contained local HTML triage report (test | ref |
    /// diff). Path printed to stderr; never committed.
    pub report_path: Option<std::path::PathBuf>,
}

/// The pure pass-decision: `Match` passes iff the diff fits the budget;
/// `Mismatch` passes iff it does NOT (the feature must *do* something). Split
/// out of `run_reftest` so it gates headless via the aggregation truth table —
/// no GPU. The `(0,0)`-floor enforcement for `Mismatch` lives at macro
/// expansion time, so `evaluate_outcome` takes the budget as given.
pub fn evaluate_outcome(kind: RefKind, diff: &Diff, fuzz: &FuzzBudget) -> bool {
    match kind {
        RefKind::Match => diff.passes(fuzz),
        RefKind::Mismatch => !diff.passes(fuzz),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reftest_kind_parses_both_tokens() {
        assert_eq!(RefKind::reftest_kind("match"), RefKind::Match);
        assert_eq!(RefKind::reftest_kind("mismatch"), RefKind::Mismatch);
    }

    #[test]
    #[should_panic(expected = "must be `match` or `mismatch`")]
    fn reftest_kind_rejects_garbage() {
        let _ = RefKind::reftest_kind("nope");
    }

    #[test]
    fn refcase_is_constructible_with_zero_fuzz_default() {
        use crate::metric::FuzzBudget;
        use bevy::app::App;
        fn noop(_: &mut App) {}
        let case = RefCase {
            name: "constructs",
            kind: RefKind::Match,
            test: noop,
            reference: noop,
            fuzz: FuzzBudget::EXACT,
        };
        assert_eq!(case.name, "constructs");
        assert_eq!(case.fuzz, FuzzBudget::EXACT);
    }

    use crate::metric::Diff;

    /// A stub Diff with `n` differing pixels and `max_channel_delta = d`, no MSSIM.
    fn stub_diff(n: u32, d: u8) -> Diff {
        Diff {
            differing_pixels: n,
            max_channel_delta: d,
            total_pixels: 1024,
            mssim: None,
            diff_image: None,
            saturated: false,
        }
    }

    #[test]
    fn match_passes_within_fuzz_fails_outside() {
        assert!(evaluate_outcome(
            RefKind::Match,
            &stub_diff(0, 0),
            &FuzzBudget::EXACT
        ));
        assert!(!evaluate_outcome(
            RefKind::Match,
            &stub_diff(1, 200),
            &FuzzBudget::EXACT
        ));
        assert!(evaluate_outcome(
            RefKind::Match,
            &stub_diff(1, 8),
            &FuzzBudget {
                max_channel_delta: 8,
                max_diff_pixels: 1
            }
        ));
    }

    #[test]
    fn mismatch_passes_outside_fuzz_fails_within() {
        assert!(evaluate_outcome(
            RefKind::Mismatch,
            &stub_diff(50, 200),
            &FuzzBudget::EXACT
        ));
        // A scene that did NOT change (zero diff) FAILS a mismatch — the no-op guard.
        assert!(!evaluate_outcome(
            RefKind::Mismatch,
            &stub_diff(0, 0),
            &FuzzBudget::EXACT
        ));
    }
}
