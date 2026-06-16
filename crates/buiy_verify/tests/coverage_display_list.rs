//! Tier 2 enrollment driver — display-list snapshots across the matrix
//! (coverage.md § Enrollment, Task 4.4). Pure-CPU, headless.
//!
//! One body, driven across `catalog() × Matrix::ci_default().cells()`: every
//! cell snapshots the CPU display-list dump (extracted nodes in paint order +
//! packed buckets in draw order) keyed by `CoverageKey::stem()`. No per-widget
//! test code — adding a fixture enrolls it into this tier with zero edits here.
//! The dump is CPU-deterministic, so the `.snap`s are byte-stable and reviewable
//! in-repo (a token-resolution regression surfaces as `#ff00ffff`, a z-sort
//! regression as a line reorder).

use std::time::Duration;

use buiy_verify::coverage::{Matrix, enroll_all};
use buiy_verify::snapshot::assert_display_list_snapshot_at;

/// The Tier-2 fan-out: snapshot every cell's display-list dump at the fixed
/// virtual instant `t=0`, keyed `<stem>@0`. `assert_display_list_snapshot_at`
/// runs the app, extracts nodes through the production `extracted_node_for`, and
/// dumps them — so a paint/clip/group/color regression in any cell shows as a
/// `.snap` diff.
#[test]
fn display_list_snapshots() {
    enroll_all(&Matrix::ci_default(), |mut app, key| {
        assert_display_list_snapshot_at(&mut app, &key.stem(), &[Duration::ZERO]);
    });
}
