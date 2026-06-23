//! Render-pipeline integration tests — the **render** subsystem group binary
//! (testing audit #9, T5.1). Consolidates the per-file `render_*.rs`
//! integration tests (extract/prepare/instance/buckets/clip/SDF/shaders/
//! compositor/msaa/golden/…) into ONE binary so `tests/support/mod.rs` and
//! the bevy link are compiled once for this group instead of once per file.
//! Each former `tests/<file>.rs` now lives at `tests/render/<file>.rs`,
//! included via an explicit `#[path]` below (this file is a test-binary crate
//! root, so a bare `mod foo;` would resolve to `tests/foo.rs`, not
//! `tests/render/foo.rs`). The shared support harness is owned here at the
//! binary root; the submodules reach it via `crate::support`.

#[path = "support/mod.rs"]
mod support;

#[path = "render/render_border_sdf.rs"]
mod render_border_sdf;
#[path = "render/render_buckets.rs"]
mod render_buckets;
#[path = "render/render_capture_app_gpu.rs"]
mod render_capture_app_gpu;
#[path = "render/render_capture_quiescence.rs"]
mod render_capture_quiescence;
#[path = "render/render_clip_rects.rs"]
mod render_clip_rects;
#[path = "render/render_clip_schedule_order.rs"]
mod render_clip_schedule_order;
#[path = "render/render_color_token.rs"]
mod render_color_token;
#[path = "render/render_components_registry.rs"]
mod render_components_registry;
#[path = "render/render_compositor.rs"]
mod render_compositor;
#[path = "render/render_compositor_gpu.rs"]
mod render_compositor_gpu;
#[path = "render/render_contrast.rs"]
mod render_contrast;
#[path = "render/render_effect_groups.rs"]
mod render_effect_groups;
#[path = "render/render_extract.rs"]
mod render_extract;
#[path = "render/render_extract_background.rs"]
mod render_extract_background;
#[path = "render/render_focus_ring.rs"]
mod render_focus_ring;
#[path = "render/render_focus_ring_gpu.rs"]
mod render_focus_ring_gpu;
#[path = "render/render_forced_colors_analyzer.rs"]
mod render_forced_colors_analyzer;
#[path = "render/render_forced_colors_swap.rs"]
mod render_forced_colors_swap;
#[path = "render/render_golden_config.rs"]
mod render_golden_config;
#[path = "render/render_golden_harness.rs"]
mod render_golden_harness;
#[path = "render/render_gpu_harness.rs"]
mod render_gpu_harness;
#[path = "render/render_group_contiguity_gpu.rs"]
mod render_group_contiguity_gpu;
#[path = "render/render_instance.rs"]
mod render_instance;
#[path = "render/render_msaa.rs"]
mod render_msaa;
#[path = "render/render_paint_order.rs"]
mod render_paint_order;
#[path = "render/render_paint_skip.rs"]
mod render_paint_skip;
#[path = "render/render_prepare.rs"]
mod render_prepare;
#[path = "render/render_primitive_dedup.rs"]
mod render_primitive_dedup;
#[path = "render/render_primitive_descriptor.rs"]
mod render_primitive_descriptor;
#[path = "render/render_primitive_key.rs"]
mod render_primitive_key;
#[path = "render/render_shader_wgsl.rs"]
mod render_shader_wgsl;
#[path = "render/render_shadow_oracle.rs"]
mod render_shadow_oracle;
#[path = "render/render_smoke.rs"]
mod render_smoke;
#[path = "render/render_specialize_gpu.rs"]
mod render_specialize_gpu;
#[path = "render/render_text_quads.rs"]
mod render_text_quads;
#[path = "render/render_theme_switch.rs"]
mod render_theme_switch;
#[path = "render/render_transform_bridge.rs"]
mod render_transform_bridge;
#[path = "render/render_view_uniform.rs"]
mod render_view_uniform;
