//! `BuiyLayoutStep::TextCommit` — reshape at final width (measure §§ 4.2,
//! 5.3, 6; architecture §§ 3.3, 4.2). Lock site #2: taken LAZILY, once,
//! only when at least one buffer reshapes — steady frames never lock.
//!
//! The trigger shape (decision 7, supersedes the § 5.1 row — recorded as
//! T3 erratum): iterate ALL text entities with a cheap reconcile guard
//! (per-line `set_align` compare + `buffer.size()` equality) instead of a
//! Changed<> filter. The guard is what catches the probe-left buffer: a
//! measure call always leaves `height_opt = None`, commit always sets
//! `Some`, so a measured-this-frame buffer can never compare equal even
//! when its resolved size didn't change.

use bevy::prelude::*;
use cosmic_text::FontSystem;
use std::sync::MutexGuard;

use crate::layout::LayoutTree;

use super::components::{ComputedTextLayout, ComputedTextLine, ResolvedBaseline, TextAlign};
use super::edit::TextBufferAccess;
use super::font_system::SharedFontSystem;

/// Per-frame count of buffers `text_commit` actually reshaped (spec § 8
/// item 4's "zero buffer relayouts", made assertable; the
/// `TextSyncAppliedCount` precedent). Overwritten per invocation.
#[derive(Resource, Default, Debug)]
pub struct TextCommitReshapeCount(pub usize);

/// The `BuiyLayoutStep::TextCommit` body — the new FINAL layout step.
/// Geometry is read from the Taffy tree (current this frame, even after
/// the step-9 re-run), never from `ResolvedLayout` (border-box; lacks
/// padding). Steady-state cost: one hash lookup + one `Layout` read + an
/// O(lines) align compare + one tuple compare per text entity — no lock,
/// no shaping, no writes.
#[allow(clippy::type_complexity)]
pub fn text_commit(
    mut commands: Commands,
    tree: Option<NonSend<LayoutTree>>,
    fonts: Option<Res<SharedFontSystem>>,
    mut reshaped: ResMut<TextCommitReshapeCount>,
    mut texts: Query<(
        Entity,
        // The editor-first accessor (E1): an editor entity reshapes its
        // editor-owned buffer; a display entity reshapes `TextBuffer.buffer`.
        // Both write the SAME `ComputedTextLayout`/`ResolvedBaseline` outputs
        // — the glyph damage signal is identical, so extract needs no new
        // trigger member.
        TextBufferAccess,
        Option<&TextAlign>,
        Option<&ComputedTextLayout>,
        Option<&ResolvedBaseline>,
    )>,
) {
    reshaped.0 = 0;
    let (Some(tree), Some(fonts)) = (tree, fonts) else {
        // Standalone BuiyTextPlugin (no LayoutPlugin): nothing was
        // measured, nothing to commit.
        return;
    };
    let mut font_system: Option<MutexGuard<'_, FontSystem>> = None;
    for (entity, mut access, align, existing_layout, existing_baseline) in texts.iter_mut() {
        let Some(&node) = tree.by_entity.get(&entity) else {
            // Text on a non-Node entity (or GC'd this frame): no layout.
            continue;
        };
        let Ok(layout) = tree.tree.layout(node) else {
            continue;
        };
        // Pre-clamp to match set_size's internal `.max(0.0)`, or a
        // degenerate box (border+padding > size) would never compare
        // equal and reshape forever.
        let content = layout.content_box_size();
        let target = (Some(content.width.max(0.0)), Some(content.height.max(0.0)));
        // T4: the content origin the producer folds (decision 2). Part of
        // the steady-state guard: a padding change with a constant content
        // box moves the offset without moving the buffer target.
        let content_offset = Vec2::new(
            layout.border.left + layout.padding.left,
            layout.border.top + layout.padding.top,
        );

        // § 5.3 — text-align at commit, per line. set_align is internally
        // guarded (returns true only on change) and resets only that
        // line's layout; resolve_dirty's external-invalidation branch
        // makes the shape pass below pick the reset up. The accessor's
        // `with_buffer_mut` bypasses change detection (measure § 7 — commit
        // writes are not damage on the buffer; damage keys on the OUTPUT
        // components below).
        let align = align.copied().unwrap_or_default().to_cosmic();
        let align_changed = access.with_buffer_mut(|buffer| {
            let mut changed = false;
            for line in buffer.lines.iter_mut() {
                changed |= line.set_align(align);
            }
            changed
        });

        let offset_stale =
            existing_layout.is_none_or(|current| current.content_offset != content_offset);
        let size_stale = access.with_buffer(|buffer| buffer.size() != target);
        // The reshape guard (Bug 2, § 2.2): extract asserts
        // `layout_runs().count() == computed.lines.len()` (extract.rs:712). A
        // buffer unshaped AFTER its last commit (a FontsGeneration sweep's
        // set_metrics/attr-reset → DirtyFlags::RELAYOUT; a future Display::None
        // escape) leaves layout_runs() short of the committed line count and
        // reaches extract unshaped (debug_assert panic / silent-no-paint in
        // release). Re-detect with the SAME comparison extract makes, so the two
        // cannot diverge. Gated on `existing_layout.is_some` — inert on a
        // never-committed buffer (zero added work to the first-commit path),
        // only an O(lines) walk on already-committed entities (the same walk
        // computed_outputs already runs on a reshape below).
        let shape_stale = existing_layout.is_some_and(|computed| {
            access.with_buffer(|buffer| buffer.layout_runs().count() != computed.lines.len())
        });
        // § 4.2's steady-state short-circuit (+ the T4 offset term + the shape guard).
        if !align_changed && !offset_stale && !size_stale && !shape_stale {
            continue;
        }

        // Lock site #2 — first reshape of the frame takes the lock.
        let font_system = font_system.get_or_insert_with(|| fonts.lock());
        // `set_size(Some(w), Some(h))` — spec § 4.2 verbatim — keeps
        // cosmic's height windowing (decision 9, T3 erratum): lines past
        // the content-box height do not lay out (`shape_until_scroll`
        // stops at `scroll_end`; `LayoutRunIter` also cuts at
        // `height_opt`), so `overflow: visible` text taller than its box
        // is absent from `ComputedTextLayout` and from T4's emission
        // until the overflow seam is revisited with overflow painting.
        //
        // § 6 outputs — idempotent-insert (write_resolved_layout's
        // discipline): tick only when the value actually changed.
        let (computed, baseline) = access.with_buffer_mut(|buffer| {
            buffer.set_size(target.0, target.1);
            buffer.shape_until_scroll(font_system, false);
            computed_outputs(buffer, content_offset)
        });
        reshaped.0 += 1;
        if existing_layout.is_none_or(|current| *current != computed) {
            commands.entity(entity).insert(computed);
        }
        match (baseline, existing_baseline) {
            (Some(new), current) if current != Some(&new) => {
                commands.entity(entity).insert(new);
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<ResolvedBaseline>();
            }
            _ => {}
        }
    }
}

/// Post-`Input` editor reshape — the coherence repair for the one-frame edit
/// path. Editor edits (`apply_keyboard_edits`, `apply_ime`, undo/redo, the
/// preedit splice) run in `BuiySet::Input`, AFTER `TextCommit` (the last Layout
/// step), and lazily UN-shape the editor-owned buffer without reshaping it
/// (`apply_change` sets redraw; `EditOutcome::reshaped == false`). That leaves
/// the buffer unshaped at frame end, so the render extract — which reads EVERY
/// text entity on any damage frame (extract.rs § 6.2 dirty-gate) — could read a
/// transiently unshaped editor and trip the `layout_runs().count() ==
/// ComputedTextLayout.lines.len()` invariant (extract.rs / the
/// `debug_assert_shape_coherence` mirror), crashing the live app the moment a
/// keystroke coincided with ANY other damage (the empty↔non-empty
/// `PlaceholderActive` toggle on the first char, a sibling row, a theme tick).
/// The pre-existing extract dirty-gate "mitigation" is all-or-nothing — it
/// cannot protect a single transiently-unshaped entity, and skipping the paint
/// would blank-flicker the editor on every keystroke (a damage frame rebuilds
/// the whole glyph buffer).
///
/// This runs `.after(BuiySet::Input).before(write_caret_and_selection)`: it
/// reshapes any editor whose buffer an edit unshaped — at the SAME content box
/// the last `TextCommit` set (an edit changes content, not the laid-out box; the
/// box re-measures next frame, the already-accepted one-frame LAYOUT latency) —
/// and rewrites `ComputedTextLayout`/`ResolvedBaseline` to match, so the buffer
/// is coherent before extract AND the caret writer (next) reads the fresh shape
/// (caret + glyphs now come current the SAME frame). Lock-free guard: the cheap
/// `layout_runs().count() != lines.len()` probe — identical to the extract
/// invariant — so a steady (non-editing) frame takes no font lock and does no
/// work.
#[allow(clippy::type_complexity)]
pub fn reshape_edited_editors(
    mut commands: Commands,
    fonts: Option<Res<SharedFontSystem>>,
    mut editors: Query<
        (
            Entity,
            TextBufferAccess,
            &ComputedTextLayout,
            Option<&ResolvedBaseline>,
        ),
        With<super::edit::TextEditState>,
    >,
) {
    let Some(fonts) = fonts else {
        // No font engine (a standalone editor-less harness): nothing to reshape.
        return;
    };
    let mut font_system: Option<MutexGuard<'_, FontSystem>> = None;
    for (entity, mut access, computed, existing_baseline) in editors.iter_mut() {
        // The extract invariant, checked lock-free: a buffer an edit unshaped
        // has fewer laid-out runs than its committed line count.
        let shape_stale =
            access.with_buffer(|buffer| buffer.layout_runs().count() != computed.lines.len());
        if !shape_stale {
            continue;
        }
        let content_offset = computed.content_offset;
        // Lock once per editing frame (the TextCommit lazy-lock idiom).
        let font_system = font_system.get_or_insert_with(|| fonts.lock());
        let (new_computed, baseline) = access.with_buffer_mut(|buffer| {
            // Reshape at the box the last commit already set — the edit changed
            // content, not geometry, so set_size would be a no-op.
            buffer.shape_until_scroll(font_system, false);
            computed_outputs(buffer, content_offset)
        });
        if *computed != new_computed {
            commands.entity(entity).insert(new_computed);
        }
        match (baseline, existing_baseline) {
            (Some(new), current) if current != Some(&new) => {
                commands.entity(entity).insert(new);
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<ResolvedBaseline>();
            }
            _ => {}
        }
    }
}

/// Debug-only main-world mirror of the extract coherence assert
/// (extract.rs:712). Runs in `Last` — after every main-world mutator (the whole
/// `Update` set chain Layout→…→Render), at the exact state `extract_buiy_glyphs`
/// reads in the render world. For every committed text entity it asserts the
/// SAME invariant extract does: `layout_runs().count() == ComputedTextLayout
/// .lines.len()`. A buffer that any post-`TextCommit` system left unshaped
/// (mutated after the last layout step, so it never re-committed this frame)
/// trips here with rich context — in EVERY headless test that pumps frames,
/// closing the gap that let a live-only extract panic ship: the headless gate
/// runs no render world, so the extract assert never fired in CI. Compiled out
/// of release (`#[cfg(debug_assertions)]` at the registration site); a no-op on
/// the steady frame (one O(lines) walk per committed text entity, no lock).
#[cfg(debug_assertions)]
pub fn debug_assert_shape_coherence(
    texts: Query<(
        Entity,
        super::edit::TextBufferAccessReadOnly,
        &ComputedTextLayout,
        Option<&super::components::Text>,
    )>,
) {
    for (entity, access, computed, text) in texts.iter() {
        let runs = access.with_buffer(|buffer| buffer.layout_runs().count());
        debug_assert_eq!(
            runs,
            computed.lines.len(),
            "TextBuffer dirty-unshaped at frame end (mutated after TextCommit, never \
             re-committed): entity={entity:?} editor={} text={:?} — extract_buiy_glyphs \
             would assert-fire (extract.rs) on this same state",
            access.has_edit(),
            text.map(|t| t.0.as_str()),
        );
    }
}

/// Fold the settled runs into the § 6 output pair.
///
/// Baseline presence keys on GLYPHS, not runs (decision 15's "no laid-out
/// runs", lowered against the engine): cosmic pushes a synthetic glyph-less
/// `LayoutLine` for every empty `BufferLine` (shape.rs:3025–3051, "create a
/// visual line for empty lines"), so an empty `Text("")` buffer still
/// yields one run — its `line_y` is a centering artifact of a zero-ascent
/// strut, not a baseline. The synthetic line DOES stay in
/// `ComputedTextLayout` (real `line_top`/`line_height` geometry — caret
/// math and the measure-side height fold both count it).
fn computed_outputs(
    buffer: &cosmic_text::Buffer,
    content_offset: Vec2,
) -> (ComputedTextLayout, Option<ResolvedBaseline>) {
    let mut lines = Vec::new();
    let mut size = Vec2::ZERO;
    let mut has_glyphs = false;
    for run in buffer.layout_runs() {
        has_glyphs |= !run.glyphs.is_empty();
        size.x = size.x.max(run.line_w);
        size.y += run.line_height;
        lines.push(ComputedTextLine {
            line_y: run.line_y,
            line_top: run.line_top,
            line_height: run.line_height,
            line_w: run.line_w,
            rtl: run.rtl,
        });
    }
    let baseline = match (lines.first(), lines.last()) {
        (Some(first), Some(last)) if has_glyphs => Some(ResolvedBaseline {
            first: first.line_y,
            last: last.line_y,
        }),
        _ => None,
    };
    (
        ComputedTextLayout {
            lines,
            size,
            content_offset,
        },
        baseline,
    )
}
