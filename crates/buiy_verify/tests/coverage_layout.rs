//! Tier 1 enrollment driver — layout-number snapshots across the matrix
//! (coverage.md § Enrollment, Task 4.4; gate #5). Pure-CPU, headless.
//!
//! One body, driven across `catalog() × Matrix::ci_default().cells()`: every
//! cell snapshots the resolved-layout dump keyed by `CoverageKey::stem()`. There
//! is NO per-widget test code — adding `fixtures/<widget>/<state>.rs` enrolls
//! that widget into this tier with zero edits here (the decisive coverage
//! property). The `.snap`s are CPU-deterministic (no GPU, fixed clock, no
//! system fonts), so they are byte-stable and reviewable in-repo.

use buiy_verify::coverage::{Matrix, enroll_all};
use buiy_verify::snapshot::{assert_layout_snapshot, layout_dump};

/// The Tier-1 fan-out: snapshot every cell's layout dump, keyed by stem. First
/// run writes one `.snap` per cell (accepted via `cargo insta accept` /
/// `INSTA_UPDATE=always` — the dumps are deterministic); thereafter a layout
/// regression in any cell shows as a `.snap` diff.
#[test]
fn layout_snapshots() {
    enroll_all(&Matrix::ci_default(), |mut app, key| {
        assert_layout_snapshot(&mut app, &key.stem());
    });
}

/// Structural guard with teeth that does NOT depend on a blessed baseline: every
/// enrolled cell's layout dump carries the version header and names the widget
/// root. This is the non-vacuous companion to the snapshot fan-out — it fails
/// loudly if `enroll_all` ever yields an empty/malformed scene, independent of
/// whether the `.snap`s are present.
#[test]
fn every_enrolled_cell_has_a_well_formed_layout_dump() {
    use buiy_verify::snapshot::LAYOUT_DUMP_VERSION;
    // `enroll_all` takes `impl Fn`, so the per-cell counter uses a `Cell`.
    let cells = std::cell::Cell::new(0usize);
    enroll_all(&Matrix::ci_default(), |mut app, key| {
        app.update();
        let dump = layout_dump(app.world());
        assert_eq!(
            dump.lines().next(),
            Some(LAYOUT_DUMP_VERSION),
            "cell {} must carry the layout-dump version header",
            key.stem()
        );
        assert!(
            dump.contains(&format!("{} ", key.widget)),
            "cell {} layout dump must name the widget root `{}`, got:\n{dump}",
            key.stem(),
            key.widget
        );
        cells.set(cells.get() + 1);
    });
    assert_eq!(
        cells.get(),
        24,
        "the button enrolls into all 24 ci_default cells"
    );
}
