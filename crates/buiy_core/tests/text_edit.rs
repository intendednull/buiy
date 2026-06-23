//! Text-editor-surface integration tests — the **text_edit** subsystem group
//! binary (testing audit #9, T5.1). Consolidates the per-file editor-surface
//! text tests (caret/selection/undo/redo/IME/clipboard/placeholder/mouse-
//! selection/preedit/submit/substrate/focus-lifecycle) into ONE binary so
//! `tests/support/mod.rs` and the bevy link are compiled once for this group
//! instead of once per file. The text-RENDER (shaping/glyph/decoration) tests
//! live in the sibling `text` binary. Each former `tests/<file>.rs` now lives
//! at `tests/text_edit/<file>.rs`, included via an explicit `#[path]` below
//! (this file is a test-binary crate root, so a bare `mod foo;` would resolve
//! to `tests/foo.rs`, not `tests/text_edit/foo.rs`). The shared support
//! harness is owned here at the binary root; submodules reach it via
//! `crate::support`.

#[path = "support/mod.rs"]
mod support;

#[path = "text_edit/text_caret_blink_focus.rs"]
mod text_caret_blink_focus;
#[path = "text_edit/text_caret_geometry.rs"]
mod text_caret_geometry;
#[path = "text_edit/text_caret_selection.rs"]
mod text_caret_selection;
#[path = "text_edit/text_caret_selection_e3_gpu.rs"]
mod text_caret_selection_e3_gpu;
#[path = "text_edit/text_clipboard_undo.rs"]
mod text_clipboard_undo;
#[path = "text_edit/text_edit_submit.rs"]
mod text_edit_submit;
#[path = "text_edit/text_edit_substrate.rs"]
mod text_edit_substrate;
#[path = "text_edit/text_editing_ops.rs"]
mod text_editing_ops;
#[path = "text_edit/text_effect_group_gpu.rs"]
mod text_effect_group_gpu;
#[path = "text_edit/text_focus_lifecycle.rs"]
mod text_focus_lifecycle;
#[path = "text_edit/text_font_reload_survival.rs"]
mod text_font_reload_survival;
#[path = "text_edit/text_ime_ops.rs"]
mod text_ime_ops;
#[path = "text_edit/text_ime_preedit_gpu.rs"]
mod text_ime_preedit_gpu;
#[path = "text_edit/text_ime_system.rs"]
mod text_ime_system;
#[path = "text_edit/text_ime_window.rs"]
mod text_ime_window;
#[path = "text_edit/text_mouse_selection.rs"]
mod text_mouse_selection;
#[path = "text_edit/text_placeholder.rs"]
mod text_placeholder;
#[path = "text_edit/text_placeholder_gpu.rs"]
mod text_placeholder_gpu;
#[path = "text_edit/text_preedit_paint.rs"]
mod text_preedit_paint;
#[path = "text_edit/text_selection_caret_gpu.rs"]
mod text_selection_caret_gpu;
#[path = "text_edit/text_selection_model.rs"]
mod text_selection_model;
#[path = "text_edit/text_set_value.rs"]
mod text_set_value;
#[path = "text_edit/text_undo_ops.rs"]
mod text_undo_ops;
#[path = "text_edit/text_undo_property.rs"]
mod text_undo_property;
#[path = "text_edit/text_undo_system.rs"]
mod text_undo_system;
