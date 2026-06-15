//! Buiy verification harness. Phase 0 ships the perceptual metric, AccessKit
//! tree snapshot, and WCAG 2 contrast linter. Full harness (15 CI gates)
//! lives in `buiy-verification-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md.

pub mod a11y;
pub mod contrast;
pub mod coverage;
pub mod determinism;
pub mod golden;
pub mod invariant;
pub mod metric;
pub mod reftest;
pub mod snapshot;
pub mod support;
