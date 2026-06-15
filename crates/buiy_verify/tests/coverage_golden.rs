//! Tier 5 enrollment driver — goldens across the matrix (coverage.md §
//! Enrollment, Task 4.4). **`#[ignore]` — needs a wgpu adapter** (real GPU
//! locally / pinned lavapipe in CI). The headless gate stays green WITHOUT it.
//!
//! Run (assert against the committed corpus):
//!     cargo test -p buiy_verify --test coverage_golden -- --ignored --test-threads=1
//!
//! Bless / re-bless (then REVIEW the PNG diffs + commit):
//!     BUIY_BLESS=1 cargo test -p buiy_verify --test coverage_golden -- --ignored \
//!         --test-threads=1
//!
//! This is the decisive coverage property at the GPU tier: it iterates the SAME
//! `Matrix::ci_default()` cells the CPU tiers enroll, captures each fixture on
//! the real adapter through [`DeterministicApp`], and asserts a golden keyed by
//! the cell's [`GoldenKey`]. Adding a fixture enrolls it here too — no per-cell
//! test code. No golden PNGs are committed yet (the corpus is blessed on a GPU
//! host); until then this lane is bless-on-demand.

use buiy_verify::coverage::{Backend as CovBackend, CoverageKey, Matrix, sorted_catalog};
use buiy_verify::determinism::DeterministicApp;
use buiy_verify::golden::{Backend, GoldenKey, assert_golden, committed_positives};
use buiy_verify::metric::FuzzBudget;

/// Map a coverage cell's [`CoverageKey`] to the GPU [`GoldenKey`]: same trace
/// identity, with the rasterizer set to the pinned CI lane (`Lavapipe`). The
/// CPU `CoverageKey.backend` (`Cpu`) is replaced — a golden is captured on a
/// real rasterizer, never on `cpu`.
fn golden_key(cov: &CoverageKey) -> GoldenKey {
    GoldenKey {
        widget: cov.widget.into(),
        state: cov.state.into(),
        theme: cov.theme.into(),
        viewport: cov.viewport.into(),
        backend: Backend::Lavapipe,
        dpr: cov.dpr,
    }
}

/// Per-cell fuzz budget. Today every cell uses the EXACT budget (the Ahem /
/// no-AA fixtures are byte-stable); a future SDF/shadow fixture widens its own
/// budget consciously (the metric's fuzz-budget discipline), keyed off
/// `cov.widget`. Kept central so widening is one reviewed edit.
fn budget_for(_cov: &CoverageKey) -> FuzzBudget {
    FuzzBudget::EXACT
}

#[test]
#[ignore = "GPU lane — needs a wgpu adapter; run with --ignored --test-threads=1 (CLAUDE.md GPU lane)"]
fn matrix_goldens() {
    // Tier-5 goldens are the *minimal rasterization residue*, not every coverage
    // cell — the corpus is blessed on demand (this file's header; goldens.md §
    // Storage). So this driver is **bless-on-demand**: a cell with NO committed
    // baseline is *pending* (skipped), while a cell that HAS been blessed must
    // still match its golden on a fresh capture (fail-closed via `assert_golden`).
    // Result: the GPU lane is green over the current 2-class residue corpus, yet
    // any blessed cell that drifts fails loudly. `BUIY_BLESS=1` captures + blesses
    // every cell (the `assert_golden` env path), so re-blessing still spans the
    // full matrix.
    let blessing = std::env::var_os("BUIY_BLESS").is_some();
    let matrix = Matrix::ci_default();
    let mut asserted = 0usize;
    let mut pending = 0usize;

    for fx in sorted_catalog() {
        for cell in matrix.cells() {
            let cov = CoverageKey::for_cell(fx, &cell, CovBackend::Cpu);
            let key = golden_key(&cov);

            // Skip un-blessed cells (no GPU capture) unless we're actively
            // blessing. A drifting *blessed* cell still fails below.
            if !blessing && committed_positives(&key) == 0 {
                pending += 1;
                continue;
            }

            // Build the GPU capture app at the cell viewport + DPR, install the
            // cell theme + forced-colors preference, spawn the fixture, capture.
            let det = DeterministicApp::new(cell.viewport.w, cell.viewport.h).dpr(cell.dpr);
            let cfg = det.config();
            let mut app = det.build();
            app.insert_resource(cell.theme.build());
            let mut prefs = buiy_core::theme::UserPreferences::default();
            prefs.forced_colors = cell.forced_colors;
            app.insert_resource(prefs);
            (fx.spawn)(&mut app);

            let img = buiy_core::render::golden::capture_to_image(&mut app, &cfg);
            assert_golden(&key, &img, &budget_for(&cov));
            asserted += 1;
        }
    }

    eprintln!("matrix_goldens: {asserted} asserted against the committed corpus, {pending} pending bless-on-demand");
    // The enrollment itself must have produced cells (guards a silently-empty
    // catalog/matrix regressing this coverage property to a vacuous pass).
    assert!(
        asserted + pending > 0,
        "coverage matrix produced zero cells — catalog or Matrix::ci_default() is empty"
    );
}
