---
**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — text editing primitives (`Editor`, `Cursor`, `Selection`, `Action`)
---

# Editing

cosmic-text ships an editing layer on top of the `Buffer` (the layout-and-shape unit). The editing layer exposes cursor + selection state and an `Action` enum the embedder drives. **Undo history, IME composition state, and OS clipboard wiring are all the embedder's job** — cosmic-text exposes the editing primitives and the geometry to render them.

Cross-links: see [architecture.md](architecture.md) for the `Buffer` / `FontSystem` / `SwashCache` split, [shaping.md](shaping.md) for how Action::Insert reaches the shaper, [bidi.md](bidi.md) for how cursor traversal honors the BiDi run order, [capabilities.md](capabilities.md) for the feature matrix.

## Types

The core editing types live in `src/edit/`:

- **`Editor<'buffer>`** — a wrapper around `Buffer` that adds `Cursor`, `Selection`, and a current `Change`. Plain `Editor` does not own undo history; it tracks the current in-progress change and emits committed `Change` values that the embedder is expected to push onto its own stack.
- **`ViEditor<'syntax_system, 'buffer>`** — `Editor` plus modit (vi-keys parser) plus `cosmic_undo_2::Commands<Change>` plus syntect highlighting. Gated behind the `vi` feature. This is the only built-in editor with an actual undo stack.
- **`Cursor`** — `{ line: usize, index: usize, affinity: Affinity }`. `index` is a byte offset into the line's UTF-8 string; `affinity` resolves the visual ambiguity at line-wrap boundaries (does the cursor sit at the end of one visual line or the start of the next?).
- **`Selection`** — `None | Normal(Cursor) | Line(Cursor) | Word(Cursor)`. The selection's "anchor" is the variant's `Cursor`; the "active" end is `Editor::cursor()`. The variant determines what gets selected by drag-extend: per-grapheme (`Normal`), per-line (`Line`), per-word (`Word`).
- **`Action`** — the embedder-driven verb set (see below).
- **`Change`** / **`ChangeItem`** — a recordable edit. `Change { items: Vec<ChangeItem> }`, where each `ChangeItem` carries `start: Cursor`, `end: Cursor`, `text: String`, `insert: bool`. Reversing the flags gives the inverse change; this is the substrate the optional undo stack consumes.

## The `Action` enum

Verbatim variants (verified against `src/edit/editor.rs` in 0.19.0):

- `Motion(Motion)` — caret movement. `Motion` covers `Left`, `Right`, `Up`, `Down`, `Home`, `End`, `LeftWord`, `RightWord`, `ParagraphStart`, `ParagraphEnd`, `PageUp`, `PageDown`, `Vertical(i32)`, plus a `GotoLine(usize)` and `Soft{Home,End}`.
- `Escape` — clears selection.
- `Insert(char)` — single-char insert at cursor.
- `Enter` — newline with optional auto-indent inheritance from the preceding line.
- `Backspace` / `Delete` — grapheme-cluster-aware deletion in each direction.
- `Indent` / `Unindent` — whitespace-block adjustments.
- `Click { x, y }` — set cursor at hit-test position, clear selection.
- `DoubleClick { x, y }` — set cursor + start word selection.
- `TripleClick { x, y }` — set cursor + start line selection.
- `Drag { x, y }` — extend the active selection to the hit-test position (granularity follows the active `Selection` variant).
- `Scroll { lines: i32 }` — scroll without moving the cursor.

The embedder maps OS input events (keyboard, mouse, IME commit) to `Action` and calls `Editor::action(font_system, action)`. The editor mutates the underlying `Buffer` and re-shapes affected lines.

## Cursor movement: visual vs logical

cosmic-text's caret model is **logical-by-default with BiDi-aware visual stepping**:

- `Motion::Left` / `Motion::Right` step in **visual** order across BiDi runs — the caret follows the eye, not the codepoint order. The implementation walks the `LayoutRun`'s `glyphs: Vec<LayoutGlyph>` (which is in visual order; each glyph carries the logical byte range it represents) and picks the previous/next glyph's logical range as the new cursor position.
- `Motion::LeftWord` / `Motion::RightWord` use `unicode-segmentation`'s word boundary iterator, also visual-order across runs.
- `Motion::Up` / `Motion::Down` snap to the **visual** line above/below, preserving x-position. This means up-arrow inside a soft-wrapped paragraph moves to the previous wrapped line, not the previous logical line.
- `Motion::ParagraphStart` / `ParagraphEnd` are the logical-line endpoints (hard breaks only).
- `Affinity::Before` vs `Affinity::After` disambiguates the cursor position at a soft-wrap boundary, so a caret can be "at end of visual line N" vs "at start of visual line N+1" with the same logical byte index.

The implication for embedders: the keyboard-handler maps `ArrowLeft` → `Motion::Left` and the caret handles RTL correctly without the embedder ever computing BiDi state. See [bidi.md](bidi.md) for the rendering side.

## Selection model

`Selection::Normal(anchor)` is the standard cursor-and-anchor range. `Editor::selection_bounds()` returns the `(start, end)` cursor pair in logical order regardless of which end is the active cursor; embedders use that for "the selected text" queries.

`Editor::action(Action::DoubleClick { x, y })` sets `Selection::Word(anchor)`; subsequent `Drag` actions extend by whole words. `TripleClick` → `Selection::Line` extends by visual lines. This matches typical desktop click-select behavior. There's no built-in multi-range / column / block selection; an embedder wanting multi-cursor edits has to layer its own state above `Editor` (the COSMIC editor uses ViEditor's separate mode for block-visual instead).

## Selection rendering

cosmic-text **gives the embedder rectangles, not pixels**. `Editor::with_selection_bounds(|rects|)` is the geometric callback: cosmic-text computes one `(min, max)` rect per visual line covering the selected portion, in `Buffer`-local coordinates. The embedder paints them in its own color/blend pipeline. For mixed-direction lines cosmic-text emits multiple rects per line (one per BiDi run intersected by the selection), so visual selection on `"hello עולם world"` paints correctly without the embedder doing BiDi math.

## Multi-line, soft-wrap, hard-wrap

cosmic-text has had multi-line `Buffer` from the start. Hard newlines are `\n` in the source string and become separate `BufferLine`s; soft wraps are computed by the layout step against the current `Buffer::set_size(width, height)`. The `Editor` treats both uniformly for vertical motion (`Up`/`Down` walk visual lines), but `ParagraphStart`/`ParagraphEnd` are hard-line-aware. `Enter` always inserts a hard newline; there is no "split-at-soft-wrap" action.

## Indentation, auto-indent

`Action::Enter` honors `Editor::auto_indent` (a `bool` setter on the editor). When set, the new line is prefixed with the leading whitespace of the previous line. `Action::Indent` / `Unindent` insert or strip a tab-width worth of leading whitespace from the selected lines (or the current line if no selection). There is no language-aware indentation (no bracket matching, no continuation detection); that's the embedder's job.

## Undo / redo: the split

This is the single most-asked editing question.

- **Base `Editor` does not own an undo stack.** It exposes `Change` tracking via `start_change()` / `finish_change()` and emits committed `Change` values. The embedder is expected to push them onto its own stack and replay (with the `insert` flag flipped) for undo.
- **`ViEditor` does own an undo stack** — `cosmic_undo_2::Commands<Change>` — and exposes `undo()` / `redo()` methods. It's gated behind the `vi` feature, which also pulls in `modit` (vi-keys parser) and `syntect` (syntax highlighting). The undo crate `cosmic_undo_2` is a separate, very small ("Undo and redo done the right-way", 651 LOC, last published 2023-11-15) System76 crate that other editors can also use directly.
- **There is no composition-grouping logic in the base editor.** If the embedder wants IME pre-edit + commit grouped as a single undo unit (per the Buiy spec's [text.md § Composition commit + undo as a unit](../../specs/2026-05-07-buiy-foundation/text.md)), the embedder builds that on top of `Change` aggregation.

The pre-amble's "cosmic-text 0.12+ added an Action history" claim does not match what's in the repo. The `Change`/`ChangeItem` types and the `vi`-gated `cosmic_undo_2` integration are how cosmic-text models this. Plain `Editor` users get the building blocks, not a finished stack.

## IME composition: the embedder boundary

cosmic-text **does not handle IME composition state**. The brief is to expose editing primitives that an embedder can drive; IME pre-edit (the underlined provisional text from a CJK / Korean IME), the composition-clause structure, and the commit boundary are all the embedder's responsibility. The OS surface is winit's `Ime` event family (`Enabled`, `Preedit`, `Commit`, `Disabled`).

The boundary in practice:
- The embedder receives a winit `Ime::Preedit(text, cursor_range)` event.
- The embedder maintains its own "preedit segment" alongside the cosmic-text `Buffer` (typically as an attribute span flagged `is_preedit`, or as a separate render layer the embedder paints under the caret).
- On `Ime::Commit(text)`, the embedder fires `Action::Insert` for each char of the committed text, clears its preedit segment, and (per Buiy's spec) groups the whole composition into a single undo unit by aggregating the `Change`s.

Issue [#10](https://github.com/pop-os/cosmic-text/issues/10) ("IME support") has been open since October 24, 2022 with the entire body being "Will need a `winit` example to experiment with this." This is the most visible mark that IME is out-of-scope for the library itself. See [critiques.md § IME boundary](critiques.md#ime-boundary).

## Find / replace

cosmic-text has no built-in find/replace. The embedder iterates `Buffer::lines()` (each is `BufferLine` with the source `text()`), runs whatever search it wants (substring, regex via `regex` crate, ICU collation), and converts byte-offset matches into `Cursor { line, index, affinity }` pairs. Selection-and-replace is then a sequence of `Action::Click` / `Action::Drag` (or direct `Editor::set_selection`) plus `Action::Insert`. The COSMIC text editor builds its find/replace UI on this primitive.

## Spell check / grammar

Explicitly out of scope. cosmic-text does not link any dictionary library, does not expose any `is_misspelled` API, does not emit any squiggly-underline markup. Embedders that need spell check route the buffer text through their own pipeline (e.g. `spellbook`, `nuspell`, OS NSSpellChecker / SAPI / IBus); the typical render strategy is for the embedder to maintain a parallel "decoration spans" list and paint underlines in its own render pass. The Buiy spec [text.md § OS integration](../../specs/2026-05-07-buiy-foundation/text.md) explicitly treats spellcheck as an OS-bridge concern, which matches cosmic-text's stance.

## Sources

- `src/edit/editor.rs`, `src/edit/vi.rs` in `pop-os/cosmic-text` at 0.19.0 — https://github.com/pop-os/cosmic-text/tree/main/src/edit
- Issue #10 "IME support" — https://github.com/pop-os/cosmic-text/issues/10
- `cosmic_undo_2` crate — https://crates.io/crates/cosmic_undo_2
- `modit` crate (vi-keys parser) — https://crates.io/crates/modit
- Buiy foundation spec — `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/text.md`
