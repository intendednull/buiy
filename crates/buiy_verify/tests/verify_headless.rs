//! Consolidated headless integration-test binary for `buiy_verify`.
//!
//! Audit #9 (test-binary consolidation, plan T5.1): the former per-file
//! integration tests are grouped into two thin binaries to eliminate the
//! linear-in-binary-count link cost. This binary holds every test module whose
//! tests all run in the headless gate; the sibling `verify_gpu` binary holds
//! every module that emits at least one `#[ignore]` (real-adapter / lavapipe)
//! test. The split is by the *runner's* ignored set, not a source grep —
//! some `#[ignore]` attributes are macro-generated (the `reftest!` GPU
//! reftests) and several modules mention `#[ignore]` only in doc comments, so
//! grep mis-classifies them.
//!
//! Each module is the unmodified former `tests/<name>.rs`, now included via an
//! explicit `#[path]` from `tests/verify_headless/`. (An integration-test root
//! file resolves bare `mod` paths against `tests/` itself, not a subdirectory
//! named after the file, so the `#[path]` is required.) The modules reference
//! the library crate (`buiy_verify::support`, `buiy_verify::coverage`, …) via
//! external crate paths, which work unchanged inside a submodule.

#[path = "verify_headless/a11y.rs"]
mod a11y;
#[path = "verify_headless/content_presence.rs"]
mod content_presence;
#[path = "verify_headless/contrast.rs"]
mod contrast;
#[path = "verify_headless/coverage_display_list.rs"]
mod coverage_display_list;
#[path = "verify_headless/coverage_dpr_invariance.rs"]
mod coverage_dpr_invariance;
#[path = "verify_headless/coverage_invariants.rs"]
mod coverage_invariants;
#[path = "verify_headless/coverage_layout.rs"]
mod coverage_layout;
#[path = "verify_headless/coverage_meta.rs"]
mod coverage_meta;
#[path = "verify_headless/determinism_ahem.rs"]
mod determinism_ahem;
#[path = "verify_headless/dialog_modal_c5d.rs"]
mod dialog_modal_c5d;
#[path = "verify_headless/golden_keys.rs"]
mod golden_keys;
#[path = "verify_headless/golden_persistence.rs"]
mod golden_persistence;
#[path = "verify_headless/golden_report.rs"]
mod golden_report;
#[path = "verify_headless/invariant_bidi.rs"]
mod invariant_bidi;
#[path = "verify_headless/invariant_mutations.rs"]
mod invariant_mutations;
#[path = "verify_headless/invariant_predicates.rs"]
mod invariant_predicates;
#[path = "verify_headless/menu_dismiss_c5c.rs"]
mod menu_dismiss_c5c;
#[path = "verify_headless/metric.rs"]
mod metric;
#[path = "verify_headless/overlay_dismiss_c5b.rs"]
mod overlay_dismiss_c5b;
#[path = "verify_headless/pointer_events_c3b.rs"]
mod pointer_events_c3b;
#[path = "verify_headless/pointer_focus_c3d.rs"]
mod pointer_focus_c3d;
#[path = "verify_headless/pointer_offset_regression.rs"]
mod pointer_offset_regression;
#[path = "verify_headless/pointer_press_smoke.rs"]
mod pointer_press_smoke;
#[path = "verify_headless/reftest_independence.rs"]
mod reftest_independence;
#[path = "verify_headless/scene_generator_smoke.rs"]
mod scene_generator_smoke;
#[path = "verify_headless/scroll_c5a.rs"]
mod scroll_c5a;
#[path = "verify_headless/sdf_oracle.rs"]
mod sdf_oracle;
#[path = "verify_headless/smoke.rs"]
mod smoke;
#[path = "verify_headless/snapshot_display_list.rs"]
mod snapshot_display_list;
#[path = "verify_headless/snapshot_dump.rs"]
mod snapshot_dump;
#[path = "verify_headless/snapshot_instance_hex.rs"]
mod snapshot_instance_hex;
#[path = "verify_headless/snapshot_layout.rs"]
mod snapshot_layout;
#[path = "verify_headless/visual.rs"]
mod visual;
