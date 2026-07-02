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
use buiy_verify::support::on_pinned_lavapipe;

/// Map a coverage cell's [`CoverageKey`] to the GPU [`GoldenKey`]: same trace
/// identity, with the rasterizer set to the pinned CI lane (`Lavapipe`). The
/// CPU `CoverageKey.backend` (`Cpu`) is replaced — a golden is captured on a
/// real rasterizer, never on `cpu`. Every OTHER axis — including
/// `forced_colors`, which produces a *different capture* — carries through, so
/// the mapping is injective (see `golden_key_is_injective_over_the_matrix`).
fn golden_key(cov: &CoverageKey) -> GoldenKey {
    GoldenKey {
        widget: cov.widget.into(),
        state: cov.state.into(),
        theme: cov.theme.into(),
        viewport: cov.viewport.into(),
        forced_colors: cov.forced_colors,
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

/// `golden_key` must be **injective** over `Matrix::ci_default()`: every
/// distinct coverage cell maps to a distinct [`GoldenKey`] slug. The
/// forced-colors axis is the trap — two cells that differ ONLY in
/// `forced_colors` produce *different captures* (the BoxShadow draw-skip reads
/// `UserPreferences::forced_colors`), so if the key collapses them, a
/// forced-colors regression silently passes against the other mode's baseline
/// once blessed. Headless (no GPU): it only exercises the pure key mapping.
#[test]
fn golden_key_is_injective_over_the_matrix() {
    use std::collections::HashSet;
    let matrix = Matrix::ci_default();
    let mut slugs = HashSet::new();
    let mut cells = 0usize;
    for fx in sorted_catalog() {
        for cell in matrix.cells() {
            let cov = CoverageKey::for_cell(fx, &cell, CovBackend::Cpu);
            let slug = golden_key(&cov).slug();
            assert!(
                slugs.insert(slug.clone()),
                "two coverage cells collapse onto one golden slug `{slug}` — \
                 a dropped axis (forced_colors?) would let a regression pass silently"
            );
            cells += 1;
        }
    }
    assert_eq!(
        slugs.len(),
        cells,
        "every one of the {cells} coverage cells must map to a distinct golden key"
    );
}

#[test]
#[ignore = "GPU lane — needs a wgpu adapter; run with --ignored --test-threads=1 (CLAUDE.md GPU lane)"]
fn matrix_goldens() {
    // Tier-5 goldens are the *minimal rasterization residue*, not every coverage
    // cell — the corpus is blessed on demand (this file's header; goldens.md §
    // Storage). So this driver is **bless-on-demand**: a cell is *compared* only
    // when (a) it has a committed baseline AND (b) we are on the pinned lavapipe
    // (the rasterizer the corpus is blessed against); otherwise it skips-as-
    // pending. A compared cell that drifts fails loudly (`assert_golden`).
    //
    // Non-vacuity contract (audit #14): a GREEN run must not silently mean "zero
    // pixels compared." The old `asserted + pending > 0` assert passed on
    // `pending` alone — green while comparing nothing. The reconciled rule
    // (below) makes **green-on-lavapipe-with-a-blessed-cell ⟹ ≥1 real
    // comparison**, while staying honestly green in the two legitimate
    // zero-compare cases: (i) no matrix cell blessed yet (the current
    // aspirational state — 5/6 residue classes have no golden), and (ii) off
    // lavapipe (this host's RX — every blessed cell adapter-skips because its
    // pixels are non-comparable).
    //
    // `BUIY_BLESS=1` captures + blesses every cell (the `assert_golden` env
    // path), so re-blessing still spans the full matrix.
    let blessing = std::env::var_os("BUIY_BLESS").is_some();
    let on_lavapipe = on_pinned_lavapipe();
    let matrix = Matrix::ci_default();
    let mut asserted = 0usize;
    let mut pending = 0usize;
    // Does the committed corpus bless ANY matrix cell? Computed independently of
    // the adapter gate so the global guard can tell "blessed corpus exists but
    // nothing compared" (a real vacuity bug on lavapipe) apart from "no cell
    // blessed yet" (the aspirational state, honestly green).
    let mut any_cell_blessed = false;
    // V6: cells the fixture cannot paint (the button's light-theme cells resolve
    // its system-color tokens to the magenta missing-token sentinel) are not part
    // of the golden corpus — counted separately so a green run's status is honest.
    let mut unpaintable = 0usize;

    for fx in sorted_catalog() {
        for cell in matrix.cells() {
            let cov = CoverageKey::for_cell(fx, &cell, CovBackend::Cpu);
            let key = golden_key(&cov);

            // V6: honor the fixture's paintability BEFORE any capture/bless/assert.
            // A cell the fixture cannot paint without the magenta missing-token
            // sentinel (the button's light-theme cells) must never be baked into
            // the corpus NOR asserted — the same skip the CPU snapshot tiers honor
            // (`enroll_snapshots`). This applies in the `BUIY_BLESS` path too, so
            // `matrix_goldens` can only ever bless the forced-colors-SAFE cells;
            // the light cells stay pending until the default widget is
            // forced-colors-safe (buiy-widget-catalog-design).
            if !fx.snapshots_cell(&cell) {
                unpaintable += 1;
                continue;
            }

            let cell_blessed = committed_positives(&key) > 0;
            any_cell_blessed |= cell_blessed;

            // Skip a cell that has no committed baseline (nothing to compare to)
            // OR that we cannot legitimately compare on this adapter (off
            // lavapipe ⇒ cross-rasterizer pixels, audit #7). Under BUIY_BLESS we
            // capture + bless every cell regardless, on whatever adapter the
            // operator chose to bless against.
            if !blessing && (!cell_blessed || !on_lavapipe) {
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

            // Capture FIRST — `capture_to_image` drives the app to quiescence
            // (TextSync → measure → commit → shape), so the bless-guard below sees
            // the ACTUAL shaped glyph count. (Running `glyph_census` on the
            // pre-update world false-refuses a text-bearing fixture at 0 glyphs
            // before its text has shaped — the button's "Save" label is
            // text-bearing, not the "non-text" the old comment assumed.)
            let img = buiy_core::render::golden::capture_to_image(&mut app, &cfg);

            // Bless-guard (C7 §2.4): refuse to record a baseline for a
            // text-bearing cell that silently shaped to zero glyphs — the same
            // `(text_bearing, glyph_count)` the content-presence invariant
            // computes, so the corpus boundary and the invariant agree. Run on the
            // POST-capture (shaped) world; the guard's teeth are proven by the
            // `bless_guard_refuses_zero_…` unit test.
            let (text_bearing, glyph_count) = buiy_verify::invariant::glyph_census(&mut app);
            buiy_verify::golden::bless_guard_check(text_bearing, glyph_count)
                .unwrap_or_else(|e| panic!("bless-guard refused cell {}: {e}", key.slug()));

            assert_golden(&key, &img, &budget_for(&cov));
            asserted += 1;
        }
    }

    // HONEST status line: `asserted` is the number of cells actually COMPARED
    // against a committed PNG; `pending` cells compared NOTHING (no baseline, or
    // skipped off lavapipe). A green run with `asserted == 0` is only legitimate
    // when there is genuinely nothing to compare here (no blessed cell, or this
    // adapter is not the canonical rasterizer) — the guard below enforces that.
    if asserted == 0 {
        eprintln!(
            "matrix_goldens: 0 cells COMPARED ({pending} pending). \
             on_lavapipe={on_lavapipe}, any_cell_blessed={any_cell_blessed}. \
             This run verified nothing at the GPU tier — legitimate ONLY because \
             no matrix cell is blessed yet (aspirational) or this adapter is not \
             the pinned lavapipe (off-canonical, cross-rasterizer). Bless a \
             residue cell AND run on lavapipe to gain real coverage."
        );
    } else {
        eprintln!(
            "matrix_goldens: {asserted} cells compared against the committed corpus, \
             {pending} pending, {unpaintable} unpaintable (on_lavapipe={on_lavapipe})"
        );
    }

    // NON-VACUITY GUARD (audit #14): on the canonical rasterizer, if the corpus
    // blesses ANY matrix cell, a green run MUST have compared at least one — else
    // green would mean "verified zero pixels" while a real baseline sits unused.
    // We do not fire when off lavapipe (every blessed cell adapter-skips — green
    // is honest) nor when no cell is blessed (the aspirational state — green is
    // honest). Bless path exempt: BUIY_BLESS writes rather than compares.
    if !blessing && on_lavapipe && any_cell_blessed {
        assert!(
            asserted > 0,
            "non-vacuity violated: on the pinned lavapipe with ≥1 blessed matrix \
             cell, the run compared ZERO cells — a green pass here would verify no \
             pixels against an existing baseline. (golden_key mapping or the \
             skip-as-pending gate is dropping every blessed cell.)"
        );
    }

    // Baseline sanity: the catalog × matrix must actually yield cells (catches a
    // silently-empty catalog/Matrix::ci_default()). This is NOT the non-vacuity
    // check — that is the lavapipe-gated guard above.
    assert!(
        asserted + pending > 0,
        "coverage matrix produced zero cells — catalog or Matrix::ci_default() is empty"
    );
}
