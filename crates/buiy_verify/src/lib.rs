//! Buiy's visual-bug verification harness — a **five-tier pyramid**, reftests-
//! first: catch bugs in cheap, deterministic, headless structured tiers and
//! shrink the expensive flaky pixel tier to the irreducible rasterization
//! residue. A single [`fixture`](coverage::Fixture) (`widget × state`) authored
//! once auto-enrolls across every tier and the full coverage matrix.
//!
//! | Tier | Entry point | Catches | GPU |
//! |---|---|---|---|
//! | 1 Layout snapshot | [`snapshot::assert_layout_snapshot`] | wrong position/size/tree | no |
//! | 2 Display-list snapshot | [`snapshot::assert_display_list_snapshot`] | wrong color/clip/packing/paint membership | no |
//! | 3 Invariant / metamorphic | [`invariant`] predicates + proptest | properties true for ALL scenes | no |
//! | 4 Reftest + SDF cross-check | [`reftest!`](crate::reftest!) / [`reftest::run_sdf_cross_check`] | `==`/`!=` of equivalent inputs; CPU↔GPU SDF | `#[ignore]` |
//! | 5 Golden | [`golden::assert_golden`] | SDF AA, shadow, atlas, compositor, forced-colors *visual* | `#[ignore]` |
//!
//! The perceptual [`metric`] (vendored pixelmatch) underlies Tiers 4–5;
//! [`determinism`] pins the capture so the pixel tiers are reproducible;
//! [`coverage`] is the fixture catalog + `Matrix` + `enroll_all`. [`a11y`] /
//! [`contrast`] are the AccessKit-tree + WCAG-2 linters.
//!
//! ## How to use this — start here
//!
//! Pick a tier, add a fixture, write a test, or bless a golden: the
//! **`using-buiy-verification` skill** (`.claude/skills/using-buiy-verification/`)
//! is the task-oriented how-to. The design / target state lives in
//! `docs/specs/2026-06-15-buiy-verification-design/` (one file per tier); the
//! rationale in `docs/reports/2026-06-14-visual-bug-detection-strategy.md`. Gate
//! commands (headless vs the GPU `--ignored` lane) are in the workspace
//! `CLAUDE.md` § Build & Test.

pub mod a11y;
pub mod contrast;
pub mod coverage;
pub mod determinism;
pub mod golden;
pub mod invariant;
pub mod metric;
pub mod pointer;
pub mod reftest;
pub mod snapshot;
pub mod support;
