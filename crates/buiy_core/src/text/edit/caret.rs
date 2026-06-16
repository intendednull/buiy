//! E3 — the caret + selection geometry writer (editing-and-ime §§ 4.1, 4.3, 5,
//! 11). One render-prep system, `write_caret_and_selection`, runs in the
//! `after(BuiySet::Input) .. before(BuiySet::Picking)` window — two sets after
//! Layout, so it reads THIS frame's `ComputedTextLayout` (the OQ#1 "caret comes
//! current the same frame the edit publishes"). It mirrors each focused, non-
//! Disabled editor's selection OUT into `TextSelection` + the T7 paint seats
//! (`CaretVisual`, `SelectionVisual`), resets the blink phase on a caret/
//! selection transition, and emits `CaretMoved` / `SelectionChanged` on
//! transition only. E3 paints the SINGLE primary caret; the BiDi split caret is
//! a named deferral (cosmic 0.19 exposes no dual-caret API — see the plan's
//! Architecture + the follow-up filed in Task 7).
//!
//! It reads the editor through `TextEditState`'s facade accessors
//! (`mirror_selection` / `with_buffer`) and names only the pure-data cosmic
//! `Cursor` and `Buffer` types, so it stays inside the `text::edit` facade (the
//! boundary tripwire).

use bevy::math::Rect;
use bevy::prelude::*;
use cosmic_text::Cursor;

use super::selection::TextSelection;
use super::state::TextEditState;
use crate::text::{CaretVisual, PreeditVisual, SelectionVisual};

/// Emitted when the caret position changes without a selection change
/// (editing-and-ime § 11 row `CaretMoved`). Payload: the entity + the new
/// cursor.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaretMoved(pub Entity, pub Cursor);

/// Emitted when the selection transitions (editing-and-ime § 11 row
/// `SelectionChanged`). Payload: the entity + the new (multi-range-shaped)
/// selection.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct SelectionChanged(pub Entity, pub TextSelection);

/// The caret bar width in logical px (snapped to >=1 physical px at paint by
/// `caret_stamp_rect`). A thin vertical bar; 1.0 logical is the convention.
const CARET_W: f32 = 1.0;

/// The caret rect for `caret` in CONTENT-BOX-LOCAL coords (logical px), from the
/// run that owns the cursor: `cursor_position` gives x, `line_top`/`line_height`
/// give y/height (§ 4.1). Returns `None` if no run owns the cursor (degenerate /
/// not-yet-shaped buffer).
///
/// `pub(crate)` so `ime.rs`'s `write_ime_window` reuses it for the caret rect
/// the IME popup anchors to (`Window.ime_position`, editing-and-ime § 6.3).
pub(crate) fn caret_rect_for(buffer: &cosmic_text::Buffer, caret: &Cursor) -> Option<Rect> {
    for run in buffer.layout_runs() {
        if let Some(x) = run.cursor_position(caret) {
            return Some(Rect::new(
                x,
                run.line_top,
                x + CARET_W,
                run.line_top + run.line_height,
            ));
        }
    }
    None
}

/// Render-prep: drive every focused, non-`Disabled` editor's caret + selection
/// paint seats from the editor state, emit transition Messages, reset blink.
#[allow(clippy::type_complexity)]
pub fn write_caret_and_selection(
    mut commands: Commands,
    time: Res<Time>,
    focused: Option<Res<crate::FocusedEntity>>,
    mut editors: Query<
        (
            Entity,
            &mut TextEditState,
            Option<&CaretVisual>,
            Option<&SelectionVisual>,
            Option<&PreeditVisual>,
        ),
        Without<super::state::Disabled>,
    >,
    mut caret_moved: MessageWriter<CaretMoved>,
    mut selection_changed: MessageWriter<SelectionChanged>,
) {
    let Some(focused) = focused else { return };
    let Some(focused_entity) = focused.0 else {
        return;
    };

    for (entity, mut state, prev_caret, prev_sel, prev_preedit) in &mut editors {
        // Only the focused editor shows a caret (§ 10). A non-focused editor
        // keeps its SelectionVisual (web-parity retention is E6; here a
        // non-focused editor simply isn't recomputed — its seats persist).
        if entity != focused_entity {
            continue;
        }

        let new_sel = state.mirror_selection();
        let caret = new_sel.primary.active;

        // --- Caret geometry into CaretVisual ---------------------------------
        // The geometry is read from the editor buffer's SHAPED layout runs. An
        // editing edit (BuiySet::Input) lazily UN-shapes the buffer this frame —
        // `Action::Insert` resets the line's layout, and reshape is deferred to
        // next frame's measure → TextCommit (the OQ#1 / M1 one-frame latency).
        // On that transiently-unshaped frame `layout_runs()` yields nothing for
        // the edited line, so `caret_rect_for` finds no run owning the caret.
        //
        // Do NOT synthesize a degenerate fallback rect here: writing a bogus
        // CaretVisual would (1) be wrong geometry and (2) — because
        // `Changed<CaretVisual>` is a glyph-producer damage trigger
        // (extract.rs § 6.2) — pull the entity into THIS frame's extract, where
        // the producer reads the same unshaped buffer (`layout_runs()` = 0) and
        // trips the architecture § 3.2 "mutated after TextCommit" tripwire
        // (extract.rs:644). Instead, leave every paint seat untouched and defer
        // the whole recompute to N+1, the SAME frame measure/commit reshapes and
        // the edited glyphs publish — so caret, selection, and glyphs come
        // current together, with no lag beyond the already-accepted one frame.
        let Some(new_rect) = state.with_buffer(|buffer| caret_rect_for(buffer, &caret)) else {
            continue;
        };

        let caret_changed = prev_caret.map(|c| c.rect) != Some(new_rect);
        if caret_changed {
            // Preserve the current visibility (the blink writer owns it); a NEW
            // caret defaults visible.
            let visible = prev_caret.map(|c| c.visible).unwrap_or(true);
            commands.entity(entity).insert(CaretVisual {
                visible,
                rect: new_rect,
            });
        }

        // --- Selection geometry into SelectionVisual -------------------------
        let prev_sel_endpoints = prev_sel.map(|s| (s.start, s.end));
        let (sel_present, new_sel_endpoints) = if new_sel.is_collapsed() {
            (false, None)
        } else {
            let (lo, hi) = new_sel.primary.ordered();
            (true, Some((lo, hi)))
        };
        if sel_present {
            let (lo, hi) = new_sel_endpoints.unwrap();
            commands.entity(entity).insert(SelectionVisual::new(lo, hi));
        } else if prev_sel.is_some() {
            commands.entity(entity).remove::<SelectionVisual>();
        }

        // --- Preedit underline geometry into PreeditVisual (E5) ---------------
        // Project the live span into the paint seat: a composition yields a
        // non-collapsed PreeditVisual over [start, end) on the span's line;
        // no composition removes it. The byte range indexes the SAME spliced
        // buffer the underline emitter reads (one source of truth).
        let new_preedit = state
            .preedit_span()
            .map(|p| (Cursor::new(p.line, p.start), Cursor::new(p.line, p.end())));
        match (new_preedit, prev_preedit) {
            (Some((lo, hi)), _) => {
                let v = PreeditVisual::new(lo, hi);
                // Compare against the existing seat so a steady composition does
                // not re-tick `Changed<PreeditVisual>` every frame (the
                // selection seat's idiom).
                if prev_preedit.copied() != Some(v) {
                    commands.entity(entity).insert(v);
                }
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<PreeditVisual>();
            }
            (None, None) => {}
        }

        // --- Transition detection + Messages + blink reset -------------------
        let selection_transitioned = {
            let prev_norm = prev_sel_endpoints.map(|(s, e)| ((s.line, s.index), (e.line, e.index)));
            let new_norm = new_sel_endpoints.map(|(s, e)| ((s.line, s.index), (e.line, e.index)));
            prev_norm != new_norm
        };

        if selection_transitioned {
            selection_changed.write(SelectionChanged(entity, new_sel.clone()));
            state.blink.reset(time.elapsed());
            state.selection = new_sel;
        } else if caret_changed {
            // Caret moved without a selection change (§ 11 CaretMoved).
            caret_moved.write(CaretMoved(entity, caret));
            state.blink.reset(time.elapsed());
            state.selection = new_sel;
        }
    }
}
