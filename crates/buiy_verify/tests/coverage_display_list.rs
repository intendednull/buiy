//! Tier 2 enrollment driver — display-list snapshots across the matrix
//! (coverage.md § Enrollment, Task 4.4). Pure-CPU, headless.
//!
//! One body, driven across `catalog() × Matrix::cpu_snapshots().cells()`: every
//! cell snapshots the CPU display-list dump (extracted nodes in paint order +
//! packed buckets in draw order) keyed by `CoverageKey::stem()`. No per-widget
//! test code — adding a fixture enrolls it into this tier with zero edits here.
//! The dump is CPU-deterministic, so the `.snap`s are byte-stable and reviewable
//! in-repo (a token-resolution regression surfaces as `#ff00ffff`, a z-sort
//! regression as a line reorder).
//!
//! Driven over [`Matrix::cpu_snapshots`] (single DPR — the display list is
//! logical-px, so DPR is inert here; the dpr-invariance is asserted once in
//! `coverage_dpr_invariance.rs`) via [`enroll_snapshots`], which skips the cells
//! a system-color-only fixture cannot paint without the magenta sentinel — so no
//! committed display-list `.snap` baselines `#ff00ffff` as the expected color
//! (the `no_committed_button_snapshot_baselines_the_sentinel` guard enforces it).

use std::time::Duration;

use buiy_verify::coverage::{Matrix, enroll_snapshots};
use buiy_verify::snapshot::assert_display_list_snapshot_at;

/// The Tier-2 fan-out: snapshot every paintable cell's display-list dump at the
/// fixed virtual instant `t=0`, keyed `<stem>@0`. `assert_display_list_snapshot_at`
/// runs the app, extracts nodes through the production `extracted_node_for`, and
/// dumps them — so a paint/clip/group/color regression in any cell shows as a
/// `.snap` diff.
#[test]
fn display_list_snapshots() {
    let enrolled = enroll_snapshots(&Matrix::cpu_snapshots(), |mut app, key| {
        assert_display_list_snapshot_at(&mut app, &key.stem(), &[Duration::ZERO]);
    });
    assert!(
        enrolled > 0,
        "the snapshot tier must enroll at least one cell"
    );
}
