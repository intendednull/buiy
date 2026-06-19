//! E3 — the caret + selection geometry writer (editing-and-ime §§ 4.1, 4.3, 5,
//! 11). One render-prep system, `write_caret_and_selection`, runs in the
//! `after(BuiySet::Input) .. before(BuiySet::Picking)` window — two sets after
//! Layout, so it reads THIS frame's `ComputedTextLayout` (the OQ#1 "caret comes
//! current the same frame the edit publishes"). It mirrors each focused, non-
//! Disabled editor's selection OUT into `TextSelection` + the T7 paint seats
//! (`CaretVisual`, `SelectionVisual`), resets the blink phase on a caret/
//! selection transition, and emits `CaretMoved` / `SelectionChanged` on
//! transition only. The PRIMARY caret is `cursor_position` from the run that
//! owns the cursor. The SECONDARY indicator (the BiDi split caret, §§ 4.1, 5)
//! now lands as `secondary_caret_rect_for`: at a direction boundary it is the
//! BEFORE glyph's (`end == index`) logical-end visual edge — the position
//! cosmic 0.19's single affinity-blind `cursor_glyph` cannot surface (it
//! resolves `index == glyph.start` BEFORE `index == glyph.end`, buffer.rs:
//! 151-174, so its one `cursor_position` only ever reports the start-glyph
//! edge). Non-boundary carets have no secondary (`None`).
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

/// The secondary indicator's height as a fraction of the line box (§ 4.1: the
/// split caret's SECONDARY mark is a shorter indicator, not a second full-height
/// bar — it tells the user which direction the next typed char flows without
/// reading as a second insertion point). A v1 visual choice; tests assert only
/// that it is `<=` the primary height, so a later tweak doesn't churn them.
const SECONDARY_CARET_H_FRAC: f32 = 0.5;

/// The caret rect for `caret` in CONTENT-BOX-LOCAL coords (logical px), from the
/// run that owns the cursor: `cursor_position` gives x, `line_top`/`line_height`
/// give y/height (§ 4.1). Returns `None` if no run owns the cursor (degenerate /
/// not-yet-shaped buffer).
///
/// `pub` so `ime.rs`'s `write_ime_window` reuses it for the caret rect the IME
/// popup anchors to (`Window.ime_position`, editing-and-ime § 6.3), and so the
/// E3 caret-geometry tests can pin it against a shaped buffer directly.
pub fn caret_rect_for(buffer: &cosmic_text::Buffer, caret: &Cursor) -> Option<Rect> {
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

/// The SECONDARY split-caret rect (§§ 4.1, 5) in CONTENT-BOX-LOCAL coords
/// (logical px), or `None` when `caret` does not sit on a bidirectional
/// DIRECTION BOUNDARY.
///
/// At a direction boundary two glyphs abut the caret's byte index: a BEFORE
/// glyph (`end == caret.index`) and an AFTER glyph (`start == caret.index`).
/// cosmic 0.19's `cursor_glyph` (buffer.rs:151-174) resolves the AFTER glyph
/// first (`index == glyph.start` before `index == glyph.end`), so the PRIMARY
/// caret (`caret_rect_for`) already lands at the AFTER glyph's edge. The
/// SECONDARY is the OTHER abutting glyph — the BEFORE glyph — at its
/// LOGICAL-END visual edge, per cosmic's own direction convention (buffer.rs:
/// 120-142, mirrored by `cursor_from_glyph_right` buffer.rs:191-197): an LTR
/// glyph's logical end is its right edge (`x + w`), an RTL glyph's is its left
/// edge (`x`). This is exactly the position cosmic's single affinity-blind
/// `cursor_position` cannot surface — hence a dedicated walk here.
///
/// Returns `None` unless BOTH glyphs exist within ONE `line_i`-matching run AND
/// they have OPPOSITE directions (`before.level.is_rtl() != after.level.is_rtl()`).
/// A run extremity (only one abutting glyph) or a same-direction join is a normal
/// caret with no second insertion point.
///
/// A logical line that SOFT-WRAPS emits MULTIPLE `LayoutRun`s sharing the same
/// `line_i` (cosmic 0.19 `LayoutRunIter`: one run per wrapped `layout_line`, all
/// with that line's `line_i`; each run's `glyphs` is the line-relative sub-slice
/// for its wrap segment). The caret's index may live on a CONTINUATION segment,
/// whose glyphs are in a LATER run — so a `line_i`-matching run that holds
/// NEITHER an abutting before NOR after glyph at the index is the WRONG wrap
/// segment, not a verdict. Mirror `caret_rect_for`'s all-runs scan: CONTINUE
/// past such a run; only conclude `None` after exhausting every run. (The
/// primary path gets this for free — `run.cursor_position` returns `None` for a
/// non-owning run and the loop continues; this walk must do the same explicitly.)
///
/// `pub` so `write_caret_and_selection` populates `CaretVisual.secondary` from
/// it (a second solid-stamp instance, CPU geometry only — no new GPU), and so
/// the E3 caret-geometry tests can pin it against a shaped buffer directly.
pub fn secondary_caret_rect_for(buffer: &cosmic_text::Buffer, caret: &Cursor) -> Option<Rect> {
    for run in buffer.layout_runs() {
        if run.line_i != caret.line {
            continue;
        }
        // The two abutting glyphs at the caret's byte index. A cluster that
        // STRADDLES the index (`start < index < end`) is not a boundary — the
        // caret is mid-cluster, a single insertion point.
        let before = run.glyphs.iter().find(|g| g.end == caret.index);
        let after = run.glyphs.iter().find(|g| g.start == caret.index);
        let (Some(before), Some(after)) = (before, after) else {
            // Neither/only-one abutting glyph: this is the WRONG wrap segment of
            // a soft-wrapped logical line (the owning run is later), or a genuine
            // run extremity. Either way, keep scanning — a LATER `line_i`-
            // matching run may own both glyphs. Only after exhausting every run
            // is `None` the verdict.
            continue;
        };
        if before.level.is_rtl() == after.level.is_rtl() {
            return None; // same direction → no split (this run owns the index)
        }
        // The BEFORE glyph's logical-end visual edge (cosmic's convention).
        let sec_x = if before.level.is_rtl() {
            before.x
        } else {
            before.x + before.w
        };
        return Some(Rect::new(
            sec_x,
            run.line_top,
            sec_x + CARET_W,
            run.line_top + run.line_height * SECONDARY_CARET_H_FRAC,
        ));
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
        // The SECONDARY split-caret indicator (§§ 4.1, 5): `Some` only when the
        // caret sits on a bidirectional direction boundary, else `None`.
        let secondary = state.with_buffer(|buffer| secondary_caret_rect_for(buffer, &caret));

        // Compare the PAIR: a boundary crossing that changes only the secondary
        // (same primary x) must still re-emit, else a stale secondary paints.
        let caret_changed =
            prev_caret.map(|c| (c.rect, c.secondary)) != Some((new_rect, secondary));
        if caret_changed {
            // Preserve the current visibility (the blink writer owns it); a NEW
            // caret defaults visible.
            let visible = prev_caret.map(|c| c.visible).unwrap_or(true);
            commands.entity(entity).insert(CaretVisual {
                visible,
                rect: new_rect,
                secondary,
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
