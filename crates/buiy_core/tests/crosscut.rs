//! Cross-cutting integration tests — the **crosscut** subsystem group binary
//! (testing audit #9, T5.1). Consolidates the per-file a11y / atlas / picking /
//! focus / theme / components / plugin-smoke / system-set-order / animation
//! integration tests into ONE binary so `tests/support/mod.rs` and the bevy
//! link are compiled once for this group instead of once per file. Each former
//! `tests/<file>.rs` now lives at `tests/crosscut/<file>.rs`, included via an
//! explicit `#[path]` below (this file is a test-binary crate root, so a bare
//! `mod foo;` would resolve to `tests/foo.rs`, not `tests/crosscut/foo.rs`).
//! The shared support harness is owned here at the binary root; the submodules
//! reach it via `crate::support`.

#[path = "support/mod.rs"]
mod support;

#[path = "crosscut/a11y.rs"]
mod a11y;
#[path = "crosscut/a11y_translate.rs"]
mod a11y_translate;
#[path = "crosscut/atlas_alloc.rs"]
mod atlas_alloc;
#[path = "crosscut/atlas_gpu.rs"]
mod atlas_gpu;
#[path = "crosscut/atlas_primitive.rs"]
mod atlas_primitive;
#[path = "crosscut/atlas_register.rs"]
mod atlas_register;
#[path = "crosscut/components.rs"]
mod components;
#[path = "crosscut/focus.rs"]
mod focus;
#[path = "crosscut/picking.rs"]
mod picking;
#[path = "crosscut/picking_backend.rs"]
mod picking_backend;
#[path = "crosscut/plugin_smoke.rs"]
mod plugin_smoke;
#[path = "crosscut/snapshot_animation.rs"]
mod snapshot_animation;
#[path = "crosscut/system_set_order.rs"]
mod system_set_order;
#[path = "crosscut/theme.rs"]
mod theme;
#[path = "crosscut/theme_forced_colors.rs"]
mod theme_forced_colors;
