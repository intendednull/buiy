//! Enrollment — one body per tier, applied across `catalog × cells`
//! (coverage.md § "Enrollment").
//!
//! Enrollment is the verb: each tier provides **one** generic body and the
//! harness drives it across the whole corpus. No per-widget test code exists
//! anywhere. [`build_app`] turns one (fixture, cell) into a deterministic app;
//! [`enroll_all`] multiplies a tier body over `catalog × Matrix::cells`.
//!
//! ## Why a CPU app, not the GPU `DeterministicApp`
//!
//! The structured tiers ([layout](crate::snapshot::assert_layout_snapshot),
//! display-list, [invariant](crate::invariant)) are pure-CPU and headless — they
//! must NOT instantiate a wgpu adapter. So [`build_app`] builds the **CPU**
//! deterministic stack (`MinimalPlugins + CorePlugin + LayoutPlugin +
//! BuiyTextPlugin{system_fonts:false} + Theme`). The text plugin makes
//! text-bearing fixtures MEASURE — the Taffy text-measure needs its
//! `SharedFontSystem`, and `system_fonts:false` keeps host fonts out so the
//! measured metrics come only from `buiy_core`'s embedded default font (Fira
//! Sans, `default_font` feature) — host-stable across the CI matrix without any
//! Ahem staging (the CPU structured tiers snapshot layout *metrics*, not
//! rasterized pixels, so an em-box substitution buys nothing here; that
//! substitution belongs to the GPU golden tier's
//! [`DeterministicApp`](crate::determinism)). It then
//! pins the viewport + DPR through a synthetic `PrimaryWindow` (the same
//! component-only window the layout solver reads its viewport from), and
//! installs the cell's theme + forced-colors preference. The GPU golden tier
//! does its own capture through [`DeterministicApp`](crate::determinism) on the
//! built app — the `Dpr`→`f32` conversion happens HERE at the viewport
//! boundary (`cell.dpr.as_f32()`), and the milliscale `Dpr` stays the key.

use bevy::app::App;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use buiy_core::CorePlugin;
use buiy_core::layout::LayoutPlugin;
use buiy_core::text::BuiyTextPlugin;
use buiy_core::theme::UserPreferences;

use super::fixture::sorted_catalog;
use super::key::{Backend, CoverageKey};
use super::matrix::{Cell, Matrix};

/// Build a CPU-only deterministic [`App`] for one (fixture, cell): the theme the
/// cell's [`ThemeAxis`](super::matrix::ThemeAxis) selects installed as the
/// active `Theme`, a synthetic `PrimaryWindow` sized to the cell viewport at the
/// cell DPR (`scale_factor_override = cell.dpr.as_f32()`), `forced_colors` set
/// on `UserPreferences`, then the fixture spawned.
///
/// The DPR conversion happens here at the viewport boundary; the milliscale
/// `Dpr` remains the coverage key, the window `scale_factor` is the derived
/// `f32`. The returned app has had **no** `update()` run yet — each tier body
/// drives its own (`assert_layout_snapshot` runs one internally; the
/// display-list / invariant bodies query after their own update).
pub fn build_app(fx: &super::fixture::Fixture, cell: &Cell) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        // The text pipeline: the Taffy text-measure needs `SharedFontSystem`
        // (only `BuiyTextPlugin` inserts it), so WITHOUT this a text-bearing
        // fixture's label measures at `0×0` — the sole coverage fixture was a
        // padding-only box around an empty label (V14). `system_fonts: false`
        // keeps it deterministic (no host-font scan); the render-world half is
        // guarded on a `RenderApp` (absent under `MinimalPlugins`) and the
        // asset-loader on an `AssetServer` (also absent), so the CPU/no-adapter
        // invariant of `build_app` holds. This also lets the content-presence
        // check (V13) run on this stack (`content_is_present` is CPU-side).
        .add_plugins(BuiyTextPlugin {
            system_fonts: false,
        });

    // The cell's theme is the ACTIVE theme. We do not run the forced-colors
    // swap system here: `build_app` installs the resolved theme directly (the
    // ThemeAxis already chose light vs. forced), so the snapshot tiers see the
    // exact theme the cell names without depending on a swap-system frame.
    app.insert_resource(cell.theme.build());

    // The forced-colors preference mode axis. Recorded on UserPreferences so a
    // fixture / producer that reads it observes the cell's mode. (The theme is
    // already the forced variant when the axis selected it; this flag carries
    // the *preference* the shadow-suppression / producer logic reads.)
    // `UserPreferences` is `#[non_exhaustive]`, so set the field on a default.
    let mut prefs = UserPreferences::default();
    prefs.forced_colors = cell.forced_colors;
    app.insert_resource(prefs);

    // Synthetic primary window: the layout solver reads its viewport from a
    // plain `Query<&Window, With<PrimaryWindow>>` (no WindowPlugin needed). The
    // resolution is PHYSICAL (logical × scale); the scale-factor override pins
    // the DPR so logical reads back at the cell viewport.
    let scale = cell.dpr.as_f32();
    let resolution = WindowResolution::new(
        (cell.viewport.w as f32 * scale).round() as u32,
        (cell.viewport.h as f32 * scale).round() as u32,
    )
    .with_scale_factor_override(scale);
    app.world_mut().spawn((
        Window {
            resolution,
            ..Default::default()
        },
        PrimaryWindow,
    ));

    (fx.spawn)(&mut app);
    app
}

/// Drive a tier `body` across the entire corpus: every fixture in
/// [`sorted_catalog`] crossed with every [`Cell`] of `matrix`, in stable
/// `(fixture, cell)` order. The body receives the built [`App`] and the
/// [`CoverageKey`] (backend [`Backend::Cpu`] — the structured tiers) and does
/// the tier-specific assert.
///
/// Stable order (catalog sorted by `(name, state)`, cells in axis-declaration
/// order) makes the enrollment deterministic — the property the
/// `enrollment_fan_out` self-test pins: `body` runs exactly
/// `fixtures × cells` times with no duplicate key.
pub fn enroll_all(matrix: &Matrix, body: impl Fn(App, CoverageKey)) {
    enroll_fixtures(&sorted_catalog(), matrix, body);
}

/// Drive a tier `body` over an EXPLICIT fixture slice × `matrix.cells()` — the
/// seam [`enroll_all`] delegates to with the full [`sorted_catalog`]. Exposed so
/// the `adding_one_fixture_grows_corpus_by_axes` self-test can prove the
/// auto-enroll-by-construction property: a slice of `n` fixtures yields exactly
/// `n × matrix.cells_per_fixture()` invocations, so adding one fixture grows the
/// corpus by exactly `|axes|` cells.
pub fn enroll_fixtures(
    fixtures: &[&'static super::fixture::Fixture],
    matrix: &Matrix,
    body: impl Fn(App, CoverageKey),
) {
    for &fx in fixtures {
        for cell in matrix.cells() {
            let key = CoverageKey::for_cell(fx, &cell, Backend::Cpu);
            let app = build_app(fx, &cell);
            body(app, key);
        }
    }
}

/// Drive a CPU **snapshot** tier body across the corpus, honoring each fixture's
/// [`paints_cell`](super::fixture::Fixture::paints_cell) skip: a cell the
/// fixture cannot paint without the missing-token sentinel is **not** enrolled
/// (the snapshot tiers must never baseline `#ff00ffff` as the expected color —
/// audit 2026-06-18). The matrix is the caller's choice — the snapshot tiers
/// pass [`Matrix::cpu_snapshots`] (single DPR), so this is also where the DPR
/// collapse takes effect.
///
/// Distinct from [`enroll_all`] (which enrolls EVERY cell, for the invariant /
/// golden tiers that paint pixels or assert structure rather than baselining a
/// token-resolved color). Returns the number of cells actually enrolled, so a
/// driver can assert non-vacuity without re-deriving the skip.
pub fn enroll_snapshots(matrix: &Matrix, body: impl Fn(App, CoverageKey)) -> usize {
    let mut enrolled = 0usize;
    for fx in sorted_catalog() {
        for cell in matrix.cells() {
            if !fx.snapshots_cell(&cell) {
                continue;
            }
            let key = CoverageKey::for_cell(fx, &cell, Backend::Cpu);
            let app = build_app(fx, &cell);
            body(app, key);
            enrolled += 1;
        }
    }
    enrolled
}
