//! Tier 3 enrollment driver — metamorphic / property invariants across the
//! matrix (coverage.md § Enrollment, Task 4.4; gate #12). Pure-CPU, headless.
//!
//! One body, driven across `catalog() × Matrix::ci_default().cells()`: every
//! cell asserts the Tier-3 relations hold on the realized live scene. The
//! `proptest`-generated invariant suite (`invariant_*.rs`) covers the unbounded
//! synthetic scene space; THIS driver asserts the same relations on the *live
//! catalog* scenes, so a fixture that produces a non-finite or mis-ordered box
//! is caught by construction across every axis combination.

use bevy::prelude::*;
use buiy_core::components::ResolvedLayout;
use buiy_verify::coverage::{Matrix, enroll_all, sorted_catalog};

/// Predicate (finiteness): every resolved-layout box of every enrolled cell has
/// finite `position`/`size`. A NaN/Inf from a degenerate axis combination
/// (e.g. a viewport-relative size at an extreme DPR) would surface here. This
/// is the live-scene analogue of the `all_finite` Tier-3 predicate
/// (invariants.md), applied per matrix cell.
#[test]
fn every_enrolled_cell_has_finite_layout() {
    // `enroll_all` takes `impl Fn`, so the per-cell counter uses interior
    // mutability (a `Cell`), not a captured `mut`.
    let cells = std::cell::Cell::new(0usize);
    enroll_all(&Matrix::ci_default(), |mut app, key| {
        app.update();
        let world = app.world_mut();
        let mut q = world
            .try_query::<(&ResolvedLayout, Option<&Name>)>()
            .expect("ResolvedLayout is registered by LayoutPlugin");
        let mut boxes = 0usize;
        for (layout, name) in q.iter(world) {
            let label = name.map(|n| n.as_str().to_string()).unwrap_or_default();
            assert!(
                layout.position.is_finite() && layout.size.is_finite(),
                "cell {} entity `{label}` has a non-finite box: pos={:?} size={:?}",
                key.stem(),
                layout.position,
                layout.size
            );
            boxes += 1;
        }
        assert!(
            boxes > 0,
            "cell {} must realize at least one laid-out box",
            key.stem()
        );
        cells.set(cells.get() + 1);
    });
    // Derive the expected count from the catalog × matrix, NOT a hardcoded 24 —
    // adding a fixture must NOT require editing this assert (the central
    // "zero test edits to add a fixture" guarantee). The literal cell count is
    // pinned once, in matrix.rs's `cells_per_fixture` unit test.
    let expected = sorted_catalog().len() * Matrix::ci_default().cells_per_fixture();
    assert_eq!(
        cells.get(),
        expected,
        "every fixture must enroll into all {expected} ci_default cells"
    );
}

/// Predicate (non-negative extent): no enrolled cell produces a negative-sized
/// box (a layout-solver regression). Sizes are `>= 0` by the box model;
/// asserting it per cell catches an axis combination that would otherwise yield
/// a collapsed/inverted box only at one DPR or viewport.
#[test]
fn every_enrolled_cell_has_non_negative_extent() {
    enroll_all(&Matrix::ci_default(), |mut app, key| {
        app.update();
        let world = app.world_mut();
        let mut q = world
            .try_query::<&ResolvedLayout>()
            .expect("ResolvedLayout is registered by LayoutPlugin");
        for layout in q.iter(world) {
            assert!(
                layout.size.x >= 0.0 && layout.size.y >= 0.0,
                "cell {} produced a negative-sized box: {:?}",
                key.stem(),
                layout.size
            );
        }
    });
}
