//! E5 — IME composition (editing-and-ime §§ 6.1, 6.2, 6.3, 11). The preedit
//! is SPLICED into the editor's display Buffer at the caret as a
//! metadata-marked `Attrs` span (spec § 6.1, superseding the prior-art
//! overlay): it reflows the line and participates in real shaping. The four
//! invariants (§ 6.2) hold by construction:
//!   (a) the splice/remove use DIRECT BufferLine surgery — no `Change`, so
//!       undo never sees preedit;
//!   (b) `value()` (state.rs) subtracts the live preedit byte range;
//!   (c) `Ime::Commit` deletes the span then inserts the committed text in
//!       ONE start_change/finish_change pair → one Composition undo unit;
//!   (d) empty Preedit / Disabled / focus-loss / Escape remove the span.
//!
//! This file NAMES `Action` (the commit insert lowering) and `Attrs` (the
//! span marker), so it MUST stay inside the `text::edit` facade — the
//! boundary tripwire (`tests/text_facade_boundary.rs`).

use std::time::Duration;

use bevy::prelude::*;
use bevy::window::{Ime, PrimaryWindow, Window};
use cosmic_text::{Action, Attrs, AttrsList, BufferLine, Cursor, Edit, FontSystem};

use super::caret::caret_rect_for;
use super::input::TextChanged;
use super::state::{Disabled, ReadOnly, TextEditState};
use super::undo::{GroupKind, UndoUnit};
use crate::FocusedEntity;
use crate::layout::LayoutTree;
use crate::text::{ComputedTextLayout, SharedFontSystem};

/// Sentinel `Attrs::metadata` marking the spliced preedit span in the
/// display buffer (spec § 6.1 "metadata-marked Attrs span"). The default
/// buffer metadata is `0`, so this distinctive value flags the composing
/// run. Correctness uses the recorded byte range (`PreeditSpan`), not this
/// marker; the marker is for span-walk fidelity.
pub const PREEDIT_METADATA: usize = 0xE5_DEAD;

/// The live IME composition (editing-and-ime § 6). Present iff a composition
/// is active. Records exactly what removal + the value()-exclusion need: the
/// line and the byte range the spliced text occupies, plus the in-preedit
/// cursor (from `Ime::Preedit.cursor`) for the composition caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreeditSpan {
    /// The buffer line the preedit was spliced into.
    pub line: usize,
    /// First byte of the preedit within the line's text.
    pub start: usize,
    /// Byte length of the spliced preedit text. The span is
    /// `[start, start + len)`.
    pub len: usize,
    /// The in-preedit cursor (`Ime::Preedit.cursor`): a `(begin, end)` byte
    /// range INTO the preedit string. `None` = hide the composition caret.
    pub cursor: Option<(usize, usize)>,
}

impl PreeditSpan {
    /// The span's end byte within the line (`start + len`).
    pub fn end(&self) -> usize {
        self.start + self.len
    }
}

impl TextEditState {
    /// `true` while an IME composition is active.
    pub fn has_preedit(&self) -> bool {
        self.preedit.is_some()
    }

    /// The live preedit span, if composing (read by the geometry writer).
    pub fn preedit_span(&self) -> Option<&PreeditSpan> {
        self.preedit.as_ref()
    }

    /// Test/inspection: the FULL buffer text including any live preedit
    /// (contrast `value()`, which excludes it). Stays inside the facade.
    pub fn buffer_text_for_test(&self) -> String {
        self.with_buffer(|buffer| {
            let mut out = String::new();
            for (i, line) in buffer.lines.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                out.push_str(line.text());
            }
            out
        })
    }

    /// Test/inspection: the resolved DEFAULT weight of a buffer line's attrs
    /// (the M1 attrs-preservation regression probe — the line's resolved
    /// weight must survive a compose+cancel). Stays inside the facade (names
    /// `cosmic_text::Weight` only to return it).
    pub fn line_default_weight_for_test(&self, line: usize) -> u16 {
        self.editor
            .with_buffer(|buffer| buffer.lines[line].attrs_list().defaults().weight.0)
    }

    /// Splice (or replace) the preedit `value` at the caret as a
    /// metadata-marked span — DIRECT BufferLine surgery, NO `Change`
    /// (invariant a). Removes any prior span first, so each Preedit replaces
    /// the last. Records the new `PreeditSpan` (with the in-preedit
    /// `cursor`). Reshape is deferred (the line's caches reset; the caller
    /// dirty-marks for next-frame measure).
    pub fn splice_preedit(
        &mut self,
        font_system: &mut FontSystem,
        value: &str,
        cursor: Option<(usize, usize)>,
    ) {
        let _ = font_system; // splice needs no FontSystem (reshape is deferred)
        self.remove_preedit_inner();
        if value.is_empty() {
            return; // an empty preedit is a removal, not a splice
        }
        let caret = self.editor.cursor();
        let line = caret.line;
        let start = caret.index;
        let len = value.len();
        self.splice_text_into_line(line, start, value);
        // Place the editor cursor at the end of the preedit so subsequent
        // replaces splice at the same logical point and the composition
        // caret base is correct.
        self.editor.set_cursor(Cursor::new(line, start + len));
        self.preedit = Some(PreeditSpan {
            line,
            start,
            len,
            cursor,
        });
    }

    /// Remove the live preedit span (invariant d) — DIRECT surgery, NO
    /// `Change`. Restores the editor cursor to the span start. No-op if
    /// nothing is composing.
    pub fn remove_preedit(&mut self, font_system: &mut FontSystem) {
        let _ = font_system;
        self.remove_preedit_inner();
    }

    /// Internal: delete the recorded span's bytes from its line via direct
    /// surgery and clear `self.preedit`. Shared by splice (replace),
    /// remove, and commit.
    fn remove_preedit_inner(&mut self) {
        let Some(span) = self.preedit.take() else {
            return;
        };
        self.delete_range_from_line(span.line, span.start, span.end());
        self.editor.set_cursor(Cursor::new(span.line, span.start));
    }

    /// Splice `text` into `line` at byte `at`, marking the inserted range
    /// with the preedit metadata. Direct BufferLine::set_text — bypasses the
    /// editor's `Change` recording (invariant a; verified: cosmic records a
    /// Change only inside insert_at/delete_range while `self.change` is
    /// `Some`, `edit/editor.rs:304,428`).
    ///
    /// **Preserves the line's RESOLVED attrs (M1).** TextSync seeds each
    /// editor line with resolved per-span attrs — weight, the resolved font
    /// family, and the T6 decoration line bits (`span_attrs`, `sync.rs:530-565`).
    /// A bare `AttrsList::new(&Attrs::new())` would reshape the line in the
    /// WRONG font/weight and drop decorations, and — because the splice never
    /// touches the `Text` component — TextSync would never re-seed it, so the
    /// corruption would persist after cancel. So we carry the existing line's
    /// `defaults()` and REINDEX every existing span across the splice point
    /// (cosmic's own `insert_at` preserves attrs via `split_off`/`append`,
    /// `editor.rs:360-432`; we rewrite one line in a single `set_text`, so a
    /// range-shift of the spans is the equivalent), then ADD the preedit
    /// metadata span on top of the inserted range only.
    fn splice_text_into_line(&mut self, line: usize, at: usize, text: &str) {
        let editor = &mut self.editor;
        editor.with_buffer_mut(|buffer| {
            let bl: &mut BufferLine = &mut buffer.lines[line];
            let mut new_text = String::with_capacity(bl.text().len() + text.len());
            new_text.push_str(&bl.text()[..at]);
            new_text.push_str(text);
            new_text.push_str(&bl.text()[at..]);
            let shift = text.len();
            // Rebuild from the existing line's defaults, not bare Attrs.
            let old = bl.attrs_list();
            let mut attrs_list = AttrsList::new(&old.defaults());
            for (range, attrs) in old.spans_iter() {
                // Shift any byte at/after the splice point right by `shift`.
                // A span straddling `at` extends to cover the inserted text
                // (it inherits the surrounding run's attrs — the cosmic
                // `get_span(cursor.index - 1)` default for an insert).
                let s = if range.start >= at {
                    range.start + shift
                } else {
                    range.start
                };
                let e = if range.end > at {
                    range.end + shift
                } else {
                    range.end
                };
                attrs_list.add_span(s..e, &attrs.as_attrs());
            }
            // The preedit metadata span on the inserted range (on top; the
            // surrounding attrs already cover it via the straddle above, so
            // this only stamps the marker).
            attrs_list.add_span(at..at + shift, &Attrs::new().metadata(PREEDIT_METADATA));
            bl.set_text(new_text, bl.ending(), attrs_list);
        });
    }

    /// Delete `[start, end)` from `line` via direct BufferLine::set_text —
    /// no `Change` (invariant a). Preserves the line's resolved attrs (M1):
    /// rebuilds from `defaults()` and maps every existing span back across
    /// the removed range (the inverse of the splice shift), so removing a
    /// preedit leaves the surrounding font/weight/decorations intact.
    fn delete_range_from_line(&mut self, line: usize, start: usize, end: usize) {
        let editor = &mut self.editor;
        editor.with_buffer_mut(|buffer| {
            let bl: &mut BufferLine = &mut buffer.lines[line];
            let mut new_text = String::with_capacity(bl.text().len());
            new_text.push_str(&bl.text()[..start]);
            new_text.push_str(&bl.text()[end..]);
            let gap = end - start;
            let old = bl.attrs_list();
            let mut attrs_list = AttrsList::new(&old.defaults());
            for (range, attrs) in old.spans_iter() {
                // Map each endpoint back across the removed `[start, end)`:
                // before `start` unchanged; inside clamps to `start`; after
                // `end` shifts left by `gap`.
                let map = |b: usize| -> usize {
                    if b <= start {
                        b
                    } else if b >= end {
                        b - gap
                    } else {
                        start
                    }
                };
                let (s, e) = (map(range.start), map(range.end));
                if e > s {
                    attrs_list.add_span(s..e, &attrs.as_attrs());
                }
            }
            bl.set_text(new_text, bl.ending(), attrs_list);
        });
    }

    /// Commit `value`: remove the preedit span, then insert the committed
    /// text inside ONE start_change/finish_change pair recorded as a single
    /// `GroupKind::Composition` undo unit (invariant c). Seals any open
    /// coalescing run first so the commit is never folded into prior typing.
    pub fn commit_preedit(&mut self, font_system: &mut FontSystem, value: &str, now: Duration) {
        self.remove_preedit_inner();
        if value.is_empty() {
            return;
        }
        self.undo.seal();
        let caret_before = self.editor.cursor();
        let selection_before = self.mirror_selection();

        self.editor.start_change();
        for ch in value.chars() {
            self.editor.action(font_system, Action::Insert(ch));
        }
        let change = self.editor.finish_change().unwrap_or_default();

        let caret_after = self.editor.cursor();
        let selection_after = self.mirror_selection();
        self.undo.record_grouped(
            UndoUnit {
                change,
                caret_before,
                caret_after,
                selection_before,
                selection_after,
                group: GroupKind::Composition,
            },
            now,
        );
    }
}

/// Emitted when a composition begins (editing-and-ime § 11; § 6.3 transition
/// empty→nonempty preedit). Payload: the entity.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositionStart(pub Entity);

/// Emitted when an active composition's preedit updates (§ 11; § 6.3
/// nonempty→nonempty). Payload: the entity + the current preedit string.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct CompositionUpdate(pub Entity, pub String);

/// Emitted when a composition ends (§ 11; § 6.3 Commit or cancel). Payload:
/// the entity + the committed string (empty on cancel).
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct CompositionEnd(pub Entity, pub String);

/// The focus-gated IME system (editing-and-ime §§ 6.1, 6.2, 11). Runs in
/// `BuiySet::Input` alongside `apply_keyboard_edits`. Reads every `Ime`
/// Message for the focused, non-`Disabled`, non-`ReadOnly` editor, drives the
/// splice/commit/remove primitives, dirty-marks for next-frame re-measure
/// (M1, exactly `apply_keyboard_edits`'s seam), and emits the Composition
/// taxonomy. `TextChanged` fires only on Commit (the one value change).
///
/// Option params (the inert-harness discipline): a bare `BuiyTextPlugin`
/// without `FocusPlugin`/`LayoutPlugin`/`Ime` infra no-ops instead of
/// panicking at param validation.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn apply_ime(
    events: Option<MessageReader<Ime>>,
    focused: Option<Res<FocusedEntity>>,
    time: Res<Time>,
    fonts: Res<SharedFontSystem>,
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut editors: Query<(&mut TextEditState, Has<ReadOnly>), Without<Disabled>>,
    mut start: MessageWriter<CompositionStart>,
    mut update: MessageWriter<CompositionUpdate>,
    mut end: MessageWriter<CompositionEnd>,
    mut changed: MessageWriter<TextChanged>,
) {
    let Some(mut events) = events else { return };
    let Some(focused) = focused else {
        events.clear();
        return;
    };
    let Some(entity) = focused.0 else {
        events.clear();
        return;
    };
    let Ok((mut state, read_only)) = editors.get_mut(entity) else {
        events.clear();
        return;
    };
    // ReadOnly editors take no IME (spec § 2.2 — IME stays disabled). Drain.
    if read_only {
        events.clear();
        return;
    }

    // Collect the frame's Ime messages before locking the FontSystem.
    let ime_events: Vec<Ime> = events.read().cloned().collect();
    if ime_events.is_empty() {
        return;
    }

    let now = time.elapsed();
    let mut span_changed = false;
    let mut value_changed = false;

    let mut font_system = fonts.lock();
    for ev in ime_events {
        match ev {
            Ime::Preedit { value, cursor, .. } => {
                if value.is_empty() {
                    // Cancel: remove the span (invariant d). End if one was active.
                    if state.has_preedit() {
                        state.remove_preedit(&mut font_system);
                        end.write(CompositionEnd(entity, String::new()));
                        span_changed = true;
                    }
                } else {
                    let was_composing = state.has_preedit();
                    state.splice_preedit(&mut font_system, &value, cursor);
                    if was_composing {
                        update.write(CompositionUpdate(entity, value));
                    } else {
                        start.write(CompositionStart(entity));
                    }
                    span_changed = true;
                }
            }
            Ime::Commit { value, .. } => {
                state.commit_preedit(&mut font_system, &value, now);
                end.write(CompositionEnd(entity, value));
                span_changed = true;
                value_changed = true;
            }
            Ime::Disabled { .. } => {
                if state.has_preedit() {
                    state.remove_preedit(&mut font_system);
                    end.write(CompositionEnd(entity, String::new()));
                    span_changed = true;
                }
            }
            Ime::Enabled { .. } => {} // winit handshake; preedit follows
        }
    }
    drop(font_system);

    // M1 dirty-mark: the splice/commit reshaped the editor-owned buffer but
    // the `Text` component is unchanged, so TextSyncTriggers do not fire.
    // Drop the intrinsics cache + Taffy-dirty the node so next frame's
    // measure → TextCommit → extract republish (N→N+1) — exactly
    // `apply_keyboard_edits`'s seam (`input.rs:563-571`).
    if span_changed {
        state.invalidate_intrinsics();
        if let Some(tree) = tree.as_deref_mut() {
            tree.mark_dirty_for_entity(entity);
        }
    }
    // TextChanged fires only when the LOGICAL value changed (commit, never
    // preedit — invariant b / § 11 "never preedit").
    if value_changed {
        changed.write(TextChanged(entity));
    }
}

/// Set `Window.ime_enabled` + `Window.ime_position` (editing-and-ime § 6.3).
/// Runs in render-prep (after BuiySet::Input, before write_caret_blink), so
/// the `ime_position` write reads THIS frame's caret geometry.
///
/// **`ime_enabled` is decided from focus + markers ALONE** — true iff a
/// focused, non-ReadOnly, non-Disabled `TextEditState` EXISTS (spec § 6.3
/// "while a focused … editor exists"). It must NOT be contingent on layout
/// readiness: on the frame an editor first focuses, its `ComputedTextLayout`
/// may not have committed yet, but IME should already be enabled. So the
/// enable decision uses a marker-only query; `ime_position` uses a SECOND,
/// geometry-bearing fetch that is simply skipped (position left as-is) until
/// the layout exists.
///
/// `ime_position` is the caret rect's bottom-left in LOGICAL WINDOW coords
/// (content-box-local rect + GlobalTransform + content_offset; the pointer.rs
/// origin idiom, reversed). The bevy_winit 10x10 exclusion area is the
/// accepted v1 limit (system.rs:503-512). Option/inert-harness params.
#[allow(clippy::type_complexity)]
pub fn write_ime_window(
    focused: Option<Res<FocusedEntity>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    // Enable decision: focus + markers only (NO geometry).
    enable_q: Query<Has<ReadOnly>, (With<TextEditState>, Without<Disabled>)>,
    // Position write: geometry-bearing, fetched only when present.
    geom_q: Query<(&TextEditState, &GlobalTransform, &ComputedTextLayout), Without<Disabled>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let focused_entity = focused.as_ref().and_then(|f| f.0);

    // ime_enabled: a focused, non-ReadOnly editor EXISTS (layout-independent).
    let enable = matches!(
        focused_entity.and_then(|e| enable_q.get(e).ok()),
        Some(read_only) if !read_only
    );
    if window.ime_enabled != enable {
        window.ime_enabled = enable;
    }
    if !enable {
        return;
    }

    // ime_position: only once geometry has committed for the focused editor.
    if let Some((state, gt, layout)) = focused_entity.and_then(|e| geom_q.get(e).ok()) {
        let caret = state.caret();
        if let Some(rect) = state.with_buffer(|b| caret_rect_for(b, &caret)) {
            let origin = gt.translation().truncate() + layout.content_offset;
            // Bottom-left of the caret rect (the popup anchors below the caret).
            let pos = Vec2::new(rect.min.x + origin.x, rect.max.y + origin.y);
            if window.ime_position != pos {
                window.ime_position = pos;
            }
        }
    }
}
