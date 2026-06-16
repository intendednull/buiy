//! Coverage-by-construction (coverage.md): derive the per-widget tests from the
//! BSN/widget catalog instead of hand-writing them.
//!
//! A [`Fixture`] corpus (the catalog rows, authored once) crossed with a global
//! [`Matrix`] of axes (theme × viewport × forced-colors × dpr) is taken as a
//! Cartesian product at test time, so adding **one** fixture auto-enrolls it
//! across **every** tier (layout snapshot, display-list snapshot, invariant
//! scenes, golden corpus) with no edit to any test file. The same fixture
//! corpus also feeds [`forced_colors::live_catalog_paint`], so gate #11's
//! live-catalog half falls out of the same enrollment.
//!
//! - [`fixture`] — the [`Fixture`] row + the [`fixture!`](crate::fixture) macro
//!   + the `inventory` [`catalog`].
//! - [`matrix`] — the [`Matrix`] / [`Cell`] axes + their Cartesian product.
//! - [`key`] — the [`CoverageKey`] (`Cell × Fixture`, `Eq + Hash`) + `stem`.
//! - [`enroll`] — [`build_app`] (one cell → a deterministic app) +
//!   [`enroll_all`] (one tier body, driven across `catalog × cells`).
//! - [`forced_colors`] — the gate-#11 live-catalog producer.

pub mod enroll;
pub mod fixture;
pub mod forced_colors;
pub mod key;
pub mod matrix;

/// The registered fixture corpus. Each `#[path]` module is a `fixture!`
/// registration; declaring it here is what compiles its `inventory::submit!`
/// into the crate so [`fixture::catalog`] enumerates it. The files also live
/// under `crates/buiy_verify/fixtures/<widget>/<state>.rs` for the
/// `insta::glob!` snapshot fan-out, so `verify_catalog_matches_glob` can assert
/// the two views agree. New fixture = new file + one `#[path]` line here.
///
/// The `#[path]` is relative to THIS file's directory (`src/coverage/`), so
/// `../../fixtures/...` reaches `crates/buiy_verify/fixtures/...`.
#[path = "../../fixtures/button/resting.rs"]
mod fixture_button_resting;

pub use enroll::{build_app, enroll_all, enroll_fixtures};
pub use fixture::{Fixture, catalog, sorted_catalog};
pub use forced_colors::{live_catalog_paint, paint_for_fixtures};
pub use key::{Backend, CoverageKey, ParsedStem};
pub use matrix::{CELL_CEILING_PER_FIXTURE, Cell, Matrix, ThemeAxis, Viewport};
