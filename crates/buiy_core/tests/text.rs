//! Text rendering / shaping integration tests — the **text** subsystem group
//! binary (testing audit #9, T5.1). Consolidates the per-file text-RENDER
//! `text_*.rs` integration tests (shaping/glyph/measure/font/decoration/
//! extract/sync/script-fallback/atlas-key/…) into ONE binary so
//! `tests/support/mod.rs` and the bevy link are compiled once for this group
//! instead of once per file. The editor-surface text tests live in the
//! sibling `text_edit` binary. Each former `tests/<file>.rs` now lives at
//! `tests/text/<file>.rs`, included via an explicit `#[path]` below (this
//! file is a test-binary crate root, so a bare `mod foo;` would resolve to
//! `tests/foo.rs`, not `tests/text/foo.rs`). The shared support harness is
//! owned here at the binary root; the submodules reach it via `crate::support`.

#[path = "support/mod.rs"]
mod support;

#[path = "text/text_atlas_key.rs"]
mod text_atlas_key;
#[path = "text/text_auto_scroll.rs"]
mod text_auto_scroll;
#[path = "text/text_commit.rs"]
mod text_commit;
#[path = "text/text_components.rs"]
mod text_components;
#[path = "text/text_decoration.rs"]
mod text_decoration;
#[path = "text/text_decoration_extract.rs"]
mod text_decoration_extract;
#[path = "text/text_decoration_gpu.rs"]
mod text_decoration_gpu;
#[path = "text/text_default_font.rs"]
mod text_default_font;
#[path = "text/text_direction.rs"]
mod text_direction;
#[path = "text/text_engine.rs"]
mod text_engine;
#[path = "text/text_extract.rs"]
mod text_extract;
#[path = "text/text_facade_boundary.rs"]
mod text_facade_boundary;
#[path = "text/text_font_asset.rs"]
mod text_font_asset;
#[path = "text/text_font_display.rs"]
mod text_font_display;
#[path = "text/text_fontdb_semantics.rs"]
mod text_fontdb_semantics;
#[path = "text/text_glyph_math.rs"]
mod text_glyph_math;
#[path = "text/text_golden_suite_gpu.rs"]
mod text_golden_suite_gpu;
#[path = "text/text_gpu.rs"]
mod text_gpu;
#[path = "text/text_input_latency.rs"]
mod text_input_latency;
#[path = "text/text_keymap.rs"]
mod text_keymap;
#[path = "text/text_measure.rs"]
mod text_measure;
#[path = "text/text_message_taxonomy.rs"]
mod text_message_taxonomy;
#[path = "text/text_registry.rs"]
mod text_registry;
#[path = "text/text_resolver.rs"]
mod text_resolver;
#[path = "text/text_script_fallback.rs"]
mod text_script_fallback;
#[path = "text/text_shaping_snapshots.rs"]
mod text_shaping_snapshots;
#[path = "text/text_sync.rs"]
mod text_sync;
#[path = "text/text_system_scan.rs"]
mod text_system_scan;
#[path = "text/text_touch_pass.rs"]
mod text_touch_pass;
#[path = "text/text_transform_extract.rs"]
mod text_transform_extract;
#[path = "text/text_typing_churn.rs"]
mod text_typing_churn;
#[path = "text/text_typing_latency.rs"]
mod text_typing_latency;
