//! Layout integration tests — the **layout** subsystem group binary (testing
//! audit #9, T5.1). Consolidates the per-file `layout*.rs` integration tests
//! (flex/grid/table/multicol/anchor/sticky/overflow/transforms/topology/…)
//! into ONE binary so `tests/support/mod.rs` and the bevy link are compiled
//! once for this group instead of once per file. Each former `tests/<file>.rs`
//! now lives at `tests/layout/<file>.rs`, included via an explicit `#[path]`
//! below (this file is a test-binary crate root, so a bare `mod foo;` would
//! resolve to `tests/foo.rs`, not `tests/layout/foo.rs`). The shared support
//! harness is owned here at the binary root; the submodules reach it via
//! `crate::support`.

#[path = "support/mod.rs"]
mod support;

#[path = "layout/layout.rs"]
mod layout;
#[path = "layout/layout_anchor_positioning.rs"]
mod layout_anchor_positioning;
#[path = "layout/layout_box_sizing.rs"]
mod layout_box_sizing;
#[path = "layout/layout_container_queries.rs"]
mod layout_container_queries;
#[path = "layout/layout_containment.rs"]
mod layout_containment;
#[path = "layout/layout_content_visibility.rs"]
mod layout_content_visibility;
#[path = "layout/layout_degenerate_sizes.rs"]
mod layout_degenerate_sizes;
#[path = "layout/layout_fixed.rs"]
mod layout_fixed;
#[path = "layout/layout_flex_distribution.rs"]
mod layout_flex_distribution;
#[path = "layout/layout_grid.rs"]
mod layout_grid;
#[path = "layout/layout_grid_stubs.rs"]
mod layout_grid_stubs;
#[path = "layout/layout_multicol.rs"]
mod layout_multicol;
#[path = "layout/layout_overflow.rs"]
mod layout_overflow;
#[path = "layout/layout_pipeline_order.rs"]
mod layout_pipeline_order;
#[path = "layout/layout_post_taffy_gate.rs"]
mod layout_post_taffy_gate;
#[path = "layout/layout_post_taffy_overrides_clear.rs"]
mod layout_post_taffy_overrides_clear;
#[path = "layout/layout_scroll_offset_no_invalidate.rs"]
mod layout_scroll_offset_no_invalidate;
#[path = "layout/layout_stacking.rs"]
mod layout_stacking;
#[path = "layout/layout_sticky.rs"]
mod layout_sticky;
#[path = "layout/layout_style_equivalence.rs"]
mod layout_style_equivalence;
#[path = "layout/layout_table.rs"]
mod layout_table;
#[path = "layout/layout_table_multicol_stubs.rs"]
mod layout_table_multicol_stubs;
#[path = "layout/layout_topology.rs"]
mod layout_topology;
#[path = "layout/layout_transforms.rs"]
mod layout_transforms;
#[path = "layout/layout_writing_modes.rs"]
mod layout_writing_modes;
