//! Coverage harness self-tests (coverage.md § Verification #1–#5, Task 4.5).
//!
//! The coverage layer is meta-machinery, so it is tested by asserting its
//! *enumeration and keying*, independent of any tier's pass/fail. All pure-CPU,
//! headless. The forced-colors live-catalog teeth test (#4) lives in
//! `coverage_forced_colors.rs`; the other four are here.

use std::collections::HashSet;
use std::path::Path;

use buiy_verify::coverage::{
    Backend, CELL_CEILING_PER_FIXTURE, CoverageKey, Fixture, Matrix, build_app, enroll_all,
    enroll_fixtures, sorted_catalog,
};

/// Walk the on-disk fixture directory (`crates/buiy_verify/fixtures/<widget>/
/// <state>.rs`) and return the `(widget, state)` set — the same set
/// `insta::glob!("fixtures/*/*.rs")` would fan out over. The widget is the
/// directory name, the state is the file stem.
fn glob_fixture_keys() -> HashSet<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut keys = HashSet::new();
    for widget_dir in std::fs::read_dir(&root).expect("fixtures/ dir exists") {
        let widget_dir = widget_dir.unwrap().path();
        if !widget_dir.is_dir() {
            continue;
        }
        let widget = widget_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        for state_file in std::fs::read_dir(&widget_dir).unwrap() {
            let p = state_file.unwrap().path();
            if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                let state = p.file_stem().unwrap().to_string_lossy().to_string();
                keys.insert((widget.clone(), state));
            }
        }
    }
    keys
}

/// Verification #1 — `catalog()` (inventory) and the `glob!` fixture-directory
/// walk enumerate the IDENTICAL `name × state` set. Guards the dual-source-of-
/// truth drift: a fixture file with no `fixture!`, or a `fixture!` not declared
/// as a `#[path]` module, breaks here.
#[test]
fn verify_catalog_matches_glob() {
    let inventory: HashSet<(String, String)> = sorted_catalog()
        .iter()
        .map(|f| (f.name.to_string(), f.state.to_string()))
        .collect();
    let glob = glob_fixture_keys();
    assert!(
        !inventory.is_empty(),
        "the inventory catalog must be non-empty"
    );
    assert_eq!(
        inventory, glob,
        "the inventory `catalog()` and the on-disk fixture directory must \
         enumerate the identical (widget, state) set"
    );
}

/// Verification #2 — over `catalog() × Matrix::ci_default().cells()`, every
/// `CoverageKey::stem()` is unique and round-trips. A collision means two cells
/// would share a baseline (the silent-overwrite bug). `CoverageKey` derives
/// `Eq + Hash` (because `dpr: Dpr` is), so the KEYS themselves — not just their
/// stems — collect into a `HashSet`.
#[test]
fn verify_keys_unique() {
    let matrix = Matrix::ci_default();
    let mut keys: HashSet<CoverageKey> = HashSet::new();
    let mut stems: HashSet<String> = HashSet::new();
    let mut count = 0usize;

    for fx in sorted_catalog() {
        for cell in matrix.cells() {
            for backend in [Backend::Cpu, Backend::Lavapipe] {
                let key = CoverageKey::for_cell(fx, &cell, backend);
                assert!(keys.insert(key), "duplicate CoverageKey: {key:?}");
                let stem = key.stem();
                assert!(stems.insert(stem.clone()), "duplicate stem: {stem}");
                // Round-trip: parse the stem back, it must recompute identically.
                let parsed = CoverageKey::from_stem(&stem)
                    .unwrap_or_else(|| panic!("stem failed to parse: {stem}"));
                assert_eq!(parsed.stem(), stem, "stem must round-trip: {stem}");
                count += 1;
            }
        }
    }
    assert_eq!(
        count,
        sorted_catalog().len() * matrix.cells_per_fixture() * 2,
        "every (fixture, cell, backend) produced exactly one key"
    );
}

/// Verification #3 — the product size per fixture is below the named CI ceiling.
/// Tripping it forces an explicit budget decision (storage-migration trigger,
/// report Open Q #6), never a silent corpus blow-up.
#[test]
fn verify_cell_count_under_ceiling() {
    let per_fixture = Matrix::ci_default().cells_per_fixture();
    assert!(
        per_fixture <= CELL_CEILING_PER_FIXTURE,
        "cells/fixture ({per_fixture}) exceeds the CI ceiling \
         ({CELL_CEILING_PER_FIXTURE}); widen the budget consciously or trim an axis"
    );
}

/// Verification #5 — enrollment fan-out totality. A stub tier body pushing its
/// `CoverageKey` into a `Vec` asserts `enroll_all` invokes the body exactly
/// `fixtures × cells` times with NO duplicate key — the Cartesian product is
/// total and non-redundant. Also proves `build_app` yields a usable App per cell
/// (the body receiving it is enough; a panic in `build_app` would red here).
#[test]
fn enrollment_fan_out() {
    let matrix = Matrix::ci_default();
    let expected = sorted_catalog().len() * matrix.cells_per_fixture();

    let seen = std::cell::RefCell::new(Vec::<CoverageKey>::new());
    enroll_all(&matrix, |_app, key| {
        seen.borrow_mut().push(key);
    });
    let seen = seen.into_inner();

    assert_eq!(
        seen.len(),
        expected,
        "enroll_all must invoke the body exactly fixtures × cells times"
    );
    let unique: HashSet<CoverageKey> = seen.iter().copied().collect();
    assert_eq!(
        unique.len(),
        seen.len(),
        "enroll_all must not invoke the body with a duplicate key"
    );
}

/// `build_app` directly: one cell builds a CPU app whose synthetic primary
/// window carries the cell viewport at the cell DPR, and the active theme is the
/// cell's. A focused unit on the enrollment substrate (separate from the
/// fan-out totality above).
#[test]
fn build_app_pins_viewport_theme_and_dpr() {
    use bevy::window::{PrimaryWindow, Window};
    use buiy_core::theme::Theme;

    let matrix = Matrix::ci_default();
    let fx = sorted_catalog()[0];
    // First cell is (light, phone, fc=false, dpr=X1) by axis-declaration order.
    let cell = matrix.cells().next().unwrap();
    let mut app = build_app(fx, &cell);
    app.update();

    // The active theme is the light theme the first cell selects.
    assert!(
        app.world()
            .resource::<Theme>()
            .color("color.surface.primary")
            .is_some(),
        "light cell installs the brand-token theme"
    );

    // The synthetic primary window carries the cell viewport, scaled by DPR.
    let mut q = app
        .world_mut()
        .query_filtered::<&Window, bevy::prelude::With<PrimaryWindow>>();
    let window = q.single(app.world()).unwrap();
    assert!(
        (window.resolution.scale_factor() - cell.dpr.as_f32()).abs() < 1e-6,
        "window scale_factor must equal the cell DPR"
    );
}

/// A second `#[cfg(test)]`-only fixture (NOT `fixture!`-registered, so it never
/// enters the real catalog) used to prove the auto-enroll-by-construction
/// property below.
fn spawn_synthetic_widget(app: &mut bevy::app::App) {
    use bevy::prelude::*;
    app.world_mut().spawn(Camera2d);
    app.world_mut().spawn((
        Name::new("synthetic"),
        buiy_core::components::Node,
        buiy_core::layout::Style::default()
            .width_px(10.0)
            .height_px(10.0),
    ));
}

static SYNTHETIC_FIXTURE: Fixture = Fixture {
    name: "synthetic",
    state: "resting",
    spawn: spawn_synthetic_widget,
    paints_cell: None,
};

/// The decisive coverage property: adding **one** fixture grows the enrolled
/// corpus by exactly `|axes|` cells — `Matrix::cells_per_fixture()` — with no
/// change to any tier body. Driven over an explicit slice (one fixture vs. two)
/// so the growth is observed directly: the delta MUST equal the axis product.
#[test]
fn adding_one_fixture_grows_corpus_by_axes() {
    let matrix = Matrix::ci_default();
    let axes = matrix.cells_per_fixture();

    let base = sorted_catalog();
    let count = |fixtures: &[&'static Fixture]| -> usize {
        let n = std::cell::Cell::new(0usize);
        enroll_fixtures(fixtures, &matrix, |_app, _key| n.set(n.get() + 1));
        n.get()
    };

    // The real catalog, then the real catalog + one new fixture.
    let mut plus_one = base.clone();
    plus_one.push(&SYNTHETIC_FIXTURE);

    let before = count(&base);
    let after = count(&plus_one);

    assert_eq!(
        after - before,
        axes,
        "adding one fixture must enroll exactly |axes| ({axes}) new cells — \
         the auto-enroll-by-construction guarantee"
    );
    assert_eq!(
        before,
        base.len() * axes,
        "the base corpus is exactly fixtures × cells_per_fixture"
    );
}
