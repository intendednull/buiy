# Buiy text-editing E5 — IME composition

**Date:** 2026-06-15
**Status:** landed
**Phase:** E5 (of the E1–E6 text-editing campaign)
**Branch:** `text-editing-e5` (off `main`, which now includes E1 + E2 + E3 + E4)
**Campaign plan:** [2026-06-13-buiy-text-editing-campaign.md](2026-06-13-buiy-text-editing-campaign.md) § "E5 — IME composition"
**Spec:** [editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md) §§ 6.1 (display-splice decision), 6.2 (the four invariants), 6.3 (popup positioning), 11 (`CompositionStart/Update/End` Messages); § 1 supersession note; § 10 focus-loss preedit removal.
**Readiness:** [2026-06-13-text-editing-design-readiness.md](../reports/2026-06-13-text-editing-design-readiness.md)

---

## Goal

Add **IME composition** to the editor: the preedit (composing) string winit hands
us through Bevy's `Ime::Preedit` Message is **spliced into the editor's display
Buffer** at the caret as a metadata-marked `Attrs` span, reflows the line, and is
underlined; each subsequent `Preedit` replaces the span; `Ime::Commit` removes the
span and inserts the committed text as **exactly one undo unit**; empty `Preedit`,
`Ime::Disabled`, focus loss, and `Escape` remove the span leaving **no orphan**.

The committed bet (spec § 6, unchanged): **winit owns the IME state machine; Buiy
translates.** E5 is the translator. Bevy 0.18's `Ime` enum is the surface —
`Preedit { window, value, cursor } | Commit { window, value } | Enabled { window }
| Disabled { window }` (verified: `bevy_window-0.18.1/src/event.rs:242-284`, a
`#[derive(Message)]` type read with `MessageReader<Ime>`).

E5 delivers, all in `crates/buiy_core/src/text/edit/`:

1. **`PreeditSpan`** — the new field `preedit: Option<PreeditSpan>` on
   `TextEditState` (E1 deferred it explicitly, `state.rs:9-13`). It records the
   live composition: which line, the byte range the spliced text occupies, the
   in-preedit cursor, and the byte length of what was there before the splice (so
   removal is exact and `value()` can subtract the span).
2. **The splice / replace / remove mechanism** — direct `BufferLine::set_text`
   mutation that produces **no cosmic `Change`** (invariant a, verified below), the
   span marked with a sentinel `Attrs::metadata` so the underline emitter finds it,
   and a re-measure dirty-mark exactly like E2's edit path.
3. **`value()` refined** to exclude the live preedit byte range (invariant b) — E2
   built `value()`; E6's placeholder check then consumes the refined accessor.
4. **The commit path** — remove the span, then insert the committed text inside
   **one** `start_change`/`finish_change` pair recorded as `GroupKind::Composition`
   (E4 shaped this variant; E5 is its first emitter) (invariant c).
5. **`apply_ime` system** (`BuiySet::Input`, focus-gated) reading `Ime` Messages and
   driving the splice/commit/remove; emitting `CompositionStart/Update/End`.
6. **`PreeditVisual` paint seat + extract emission** — the preedit underline,
   quad-tier, via the existing `ExtractedTextQuads` carrier
   (decoration-and-paint § 8). No new GPU work.
7. **Popup positioning** — `write_ime_window` sets `Window.ime_enabled` (true while
   a focused, non-`ReadOnly`, non-`Disabled` editor exists) and `Window.ime_position`
   (caret rect bottom-left in logical window coords) on caret move / preedit update.
8. **The `CompositionStart/Update/End` Messages** (spec § 11).
9. One additive **`#[ignore]` GPU golden**: caret + selection + preedit-underline on
   a mixed-direction fixture — **build-only** in this phase (the orchestrator runs
   the GPU lane).

Everything that names a cosmic `Editor`/`Edit`/`Action`/`Change`/`Attrs` type stays
**inside `crates/buiy_core/src/text/edit/`** — `tests/text_facade_boundary.rs` fails
the build otherwise. The new IME file names `Action` (the commit `insert_at` lowering
is `editor.action`) and `Attrs` (the metadata span), so it MUST live in `text::edit`.
The `Composition*` Messages and `PreeditVisual` name no cosmic type, so they are
public facade API.

## Architecture

```
   Ime Messages (winit → bevy_winit → Messages<Ime>)        FocusedEntity (focus.rs)
            │                                                       │
            ▼                                                       ▼
   apply_ime  (BuiySet::Input, focus-gated — alongside apply_keyboard_edits)
            │  reads MessageReader<Ime>, ONE SharedFontSystem lock hold for the burst
            ▼
   ┌──────────────────────────── apply_ime dispatch ──────────────────────────────┐
   │ Ime::Preedit { value: "", .. }  → remove_preedit()      (cancel: empty value) │
   │ Ime::Preedit { value, cursor }  → splice_preedit(value, cursor):              │
   │       remove any prior span, then splice `value` at caret as a metadata-      │
   │       marked Attrs span (direct BufferLine::set_text — NO Change → invariant a)│
   │       record PreeditSpan{line, start, len, prior_byte_at_caret, cursor}        │
   │       transition empty→nonempty ⇒ CompositionStart; else CompositionUpdate     │
   │ Ime::Commit { value }           → commit_preedit(value):                       │
   │       remove the span (direct, no Change), then ONE start_change/finish_change │
   │       inserting `value` → undo.record(.., GroupKind::Composition) (invariant c)│
   │       → CompositionEnd(committed)                                              │
   │ Ime::Disabled { .. }            → remove_preedit() (+ CompositionEnd if active)│
   │ Ime::Enabled { .. }             → no-op (winit handshake; preedit follows)     │
   └────────────────────────────────────────────────────────────────────────────────┘
            │
            │  any span change / commit ⇒ E2's M1 dirty-mark seam:
            │  state.invalidate_intrinsics(); tree.mark_dirty_for_entity(e); (TextChanged only on commit)
            ▼
   next frame: TextSync triggers don't fire (Text unchanged), node is Taffy-dirty ⇒
               measure → TextCommit → extract republish the spliced buffer (N→N+1)

   ── render-prep window (after BuiySet::Input, before write_caret_blink) ──
   write_caret_and_selection (E3) — also writes/removes PreeditVisual from state.preedit
   write_ime_window (E5)         — sets Window.ime_enabled + ime_position from caret rect
            │
            ▼
   extract_buiy_glyphs (Extract) — reads Option<&PreeditVisual>, emits underline quads
            (run.highlight over the span byte range → push_selection_quad with underline
             geometry + the preedit-underline color) into ExtractedTextQuads (quad seat)
```

**Why a display-splice, not an overlay (spec § 6.1).** Web parity requires the
preedit to **reflow** the line: composing CJK mid-paragraph shifts following text
and can re-wrap, and the in-preedit cursor is only correct when the preedit
participates in real shaping. An overlay cannot reflow. The *bet* the prior-art
notes encode — undo, the logical value, and the event stream never see preedit — is
kept exactly via the four § 6.2 invariants; only the display representation changes.

**Why direct `BufferLine::set_text` bypasses undo (invariant a — verified, cosmic
0.19).** `Editor` records a `Change` ONLY inside `insert_at`/`delete_range`, each of
which pushes a `ChangeItem` into `self.change` *iff* `self.change.is_some()*
(`edit/editor.rs:304,428`); `self.change` is `Some` only between `start_change`
(`editor.rs:506-509`) and `finish_change` (`editor.rs:512-513`). Mutating
`buffer.lines[i]` directly through `BufferLine::set_text` (`buffer_line.rs:76-93`)
never touches `self.change`, so it produces **no `Change` and reaches no undo unit**.
The splice/remove path therefore does its own buffer-line surgery and never calls
`start_change`/`finish_change`. The commit path is the ONLY place E5 opens a change
pair.

**Why the splice needs no `FontSystem` and re-measures next frame.**
`BufferLine::set_text` calls `self.reset()` (`buffer_line.rs:88,197-201`), which
invalidates the line's shape + layout caches — exactly the lazy un-shape an
`Action::Insert` leaves behind. Reshape is deferred to next frame's
measure → `TextCommit` (the OQ#1 / M1 one-frame path E2 established). So `apply_ime`
does the same dirty-mark E2 does: `state.invalidate_intrinsics()` +
`tree.mark_dirty_for_entity(entity)`. We still take the `SharedFontSystem` lock for
the burst because the **commit** path calls `editor.action(fs, Action::Insert(..))`,
which needs it (the splice itself does not, but one lock for the frame mirrors E2).

**Why a snapshot-based `value()` refinement (invariant b).** The preedit lives
*inside* the buffer line, so `value()`'s naive line-join would include it. The
`PreeditSpan` records the byte range `[start, start+len)` on `line`; `value()`
reconstructs each line's text and, for the preedit line, splices the range back OUT
(replacing it with nothing — the caret position the preedit replaced is restored
implicitly because the preedit was inserted, not overwriting). Result: the logical
value is byte-for-byte what it was before composition started. `TextChanged` is
emitted ONLY on commit (when the logical value truly changes), never per `Preedit`.

**Why a new `PreeditVisual` component (not reuse `SelectionVisual`).** The preedit
underline spans a byte range like a selection, but its *source* is IME state, its
*geometry* is an underline strip (not a full-height box), and its *color* is the
preedit token, not `::selection`. It rides the **same** `ExtractedTextQuads` carrier
and `push_*_quad` mechanics (no new GPU buffer), but it is a distinct paint seat. The
E3 writer already recomputes `CaretVisual`/`SelectionVisual` every render-prep frame;
`PreeditVisual` is written/removed in the same writer from `state.preedit`, so it
stays consistent with the spliced buffer it indexes.

**Why `BuiySet::Input` for `apply_ime`.** Editing input lands in `BuiySet::Input`
(the E2 `apply_keyboard_edits` / E3 `pointer_selection` precedent, `text/mod.rs:219`),
two sets after Layout, so a composition mutation publishes N→N+1 — the accepted
one-frame latency (OQ#1). Same-frame re-entry is rejected campaign-wide. `apply_ime`
runs in the same set, after `apply_keyboard_edits` is unnecessary (winit sends EITHER
`KeyboardInput` OR `Ime` per the `ime_enabled` doc, `window.rs:269-276` — they do not
race for the same keystroke), so no explicit ordering between them is required.

## Tech stack

- **Rust / Bevy 0.18.1**, `buiy_core` crate. Editing facade
  `crates/buiy_core/src/text/edit/`.
- **Bevy 0.18 IME surface** (verified vendored source):
  `bevy::window::Ime` enum (`bevy_window-0.18.1/src/event.rs:242-284`, `#[derive(
  Message)]`, all variants carry `window: Entity`); `Window.ime_enabled: bool` +
  `Window.ime_position: Vec2` (`window.rs:269-285`); read with `MessageReader<Ime>`
  (`bevy_ecs-0.18.1/src/message/...`). bevy_winit forwards to
  `set_ime_allowed`/`set_ime_cursor_area` with a hardcoded `PhysicalSize::new(10,10)`
  exclusion area (`bevy_winit-0.18.1/src/system.rs:503-512`) — accepted v1 limit.
- **cosmic-text 0.19** (already pinned):
  `Attrs::metadata(usize)` + `Attrs.metadata: usize` field (`attrs.rs:293,353-356`);
  `AttrsList::{new, add_span, spans_iter, clear_spans}` (`attrs.rs:496-594`);
  `BufferLine::{text, set_text, ending, attrs_list}` (`buffer_line.rs:68-136`);
  `Buffer.lines: Vec<BufferLine>` (`buffer.rs:336`); `Editor::{set_cursor, cursor,
  action, start_change, finish_change}`; `Cursor { line, index, affinity }`;
  `LayoutRun::{highlight, cursor_position, line_top, line_height}`
  (`buffer.rs:36-142`). All verified.
- **`Time<Virtual>`** is not needed (IME is event-driven, no time-window coalescing);
  the IME state-machine tests are wall-clock-free and deterministic.

## Placement decisions (resolved)

- **`PreeditSpan`** is a new type + the `preedit: Option<PreeditSpan>` field, both in
  a NEW file `crates/buiy_core/src/text/edit/ime.rs`. The field is added to
  `TextEditState` in `state.rs` (the spec § 2.2 sketch's slot, where E3 put
  `selection`/`blink`, E4 `undo`). The splice/remove/commit methods are
  `impl TextEditState` blocks in `ime.rs` (it names `Action`/`Attrs`, so it must be
  inside the facade).
- **The `apply_ime` + `write_ime_window` systems** live in `ime.rs`, registered by
  `BuiyTextPlugin` (`text/mod.rs`).
- **The `CompositionStart/Update/End` Messages** live in `ime.rs` (named with the
  state machine, no cosmic type), re-exported through `text::edit` and `text::mod`.
- **`PreeditVisual`** is a new component in `crates/buiy_core/src/text/components.rs`
  (alongside `CaretVisual`/`SelectionVisual` — the T7 paint-seat module, the cosmic
  boundary where a `Cursor`-free `usize` byte range is legal). The extract emission
  is added to `extract_buiy_glyphs` (`text/extract.rs`), mirroring the selection
  pre-pass.
- **The sentinel metadata** marking the preedit span is a `const PREEDIT_METADATA:
  usize` in `ime.rs`. The display buffer's default `Attrs::metadata` is `0`
  (`attrs.rs` default), so the sentinel is a distinctive non-zero value; the value()
  refinement and removal do not depend on it (they use the recorded byte range) — the
  metadata exists so the underline could later be driven from buffer spans if desired,
  but E5 drives the underline from `PreeditVisual`'s byte range (one source of truth,
  the recorded span). The metadata is set for spec fidelity (§ 6.1 "metadata-marked
  `Attrs` span") and to keep the spliced span visually distinguishable to any future
  span-walk; it is NOT load-bearing for correctness.

---

## Task 1 — `PreeditSpan` + the `preedit` field; the splice/remove primitives

Land the data model and the buffer surgery, tested directly against a headless
`FontSystem`. No systems, no Messages, no commit yet. Invariant (a) is proven here.

- [ ] **RED — write the failing splice/remove unit test.** Create
  `crates/buiy_core/tests/text_ime_ops.rs`:

  ```rust
  //! E5 — IME composition operations applied directly to the editor
  //! (editing-and-ime §§ 6.1, 6.2). The splice/remove/commit primitives are
  //! tested against a real (headless) `FontSystem` — reshape needs none at
  //! splice time, but the commit `insert_at` does. No adapter (cosmic shaping
  //! is CPU); no winit window (synthetic operations). The four invariants are
  //! each a named test here + in `text_ime_system.rs` (system level).

  use buiy_core::text::SharedFontSystem;
  use buiy_core::text::edit::{EditCommand, TextEditState};
  use cosmic_text::Metrics;

  /// A Preedit splice inserts the composing text into the buffer (so it
  /// reflows + shapes), records the live span, and — invariant (a) — adds
  /// NOTHING to the undo stack.
  #[test]
  fn preedit_splice_inserts_into_buffer_without_touching_undo() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();

      // Seed a logical value and move the caret to the end.
      state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
      assert_eq!(state.value(), "ab");
      let undo_before = state.undo_depth();

      // Splice a preedit "X" at the caret (after "ab").
      state.splice_preedit(&mut fs, "X", Some((0, 1)));

      // The buffer CONTENT now contains the preedit (it shapes + reflows)...
      assert_eq!(state.buffer_text_for_test(), "abX");
      // ...but the LOGICAL value excludes it (invariant b, proven fully in Task 2).
      assert_eq!(state.value(), "ab");
      // ...and undo is UNCHANGED (invariant a).
      assert_eq!(state.undo_depth(), undo_before, "splice must not record a Change");
      assert!(state.has_preedit());
  }

  /// A second Preedit REPLACES the first span (no accumulation).
  #[test]
  fn preedit_replace_swaps_the_span() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);

      state.splice_preedit(&mut fs, "X", None);
      assert_eq!(state.buffer_text_for_test(), "abX");
      state.splice_preedit(&mut fs, "YZ", None);
      assert_eq!(state.buffer_text_for_test(), "abYZ", "second preedit replaces the first");
      assert_eq!(state.value(), "ab");
  }

  /// Removing the preedit restores the buffer to its pre-composition content
  /// and clears the span (invariant d — no orphan).
  #[test]
  fn remove_preedit_restores_buffer_and_clears_span() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);

      state.splice_preedit(&mut fs, "XY", None);
      assert_eq!(state.buffer_text_for_test(), "abXY");
      state.remove_preedit(&mut fs);
      assert_eq!(state.buffer_text_for_test(), "ab", "buffer restored");
      assert!(!state.has_preedit(), "no orphan span");
      assert_eq!(state.value(), "ab");
  }
  ```

- [ ] **RUN IT — fails to compile.**

  ```sh
  cargo test -p buiy_core --test text_ime_ops
  ```

  Expected: `error[E0599]: no method named splice_preedit / remove_preedit /
  has_preedit / buffer_text_for_test found for struct TextEditState`.

- [ ] **GREEN — create `ime.rs` with `PreeditSpan` + the primitives.** Create
  `crates/buiy_core/src/text/edit/ime.rs`:

  ```rust
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
  use cosmic_text::{Action, Attrs, AttrsList, BufferLine, Cursor, Edit, FontSystem, Shaping};

  use super::state::TextEditState;
  use super::undo::{GroupKind, UndoUnit};

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
          use cosmic_text::Edit;
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
          self.preedit = Some(PreeditSpan { line, start, len, cursor });
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
                  let s = if range.start >= at { range.start + shift } else { range.start };
                  let e = if range.end > at { range.end + shift } else { range.end };
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
      /// `GroupKind::Composition` undo unit (invariant c). Returns whether the
      /// logical value changed (always true for a non-empty commit). Seals any
      /// open coalescing run first so the commit is never folded into prior
      /// typing.
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
  ```

  Wire the new field into `TextEditState` in
  `crates/buiy_core/src/text/edit/state.rs`. Add the import and field:

  ```rust
  use super::ime::PreeditSpan;
  ```

  Add the field to the struct (after `undo`), and document it (replacing the E1
  deferral note for `preedit`):

  ```rust
      /// The live IME composition span (§ 6), `None` when not composing. E5
      /// lands it. Written only by the `ime.rs` splice/remove/commit methods;
      /// read by `value()` (byte-range exclusion, invariant b) and the geometry
      /// writer (`PreeditVisual`).
      pub(crate) preedit: Option<PreeditSpan>,
  ```

  Initialize it in `TextEditState::new` (after `undo: UndoStack::default(),`):

  ```rust
              preedit: None,
  ```

  Register the module in `crates/buiy_core/src/text/edit/mod.rs` (after `mod
  caret;`, alphabetical-ish with the others — add a `mod ime;` line and a re-export):

  ```rust
  mod ime;
  ```

  Task 1 re-exports ONLY the symbols that exist after Task 1 (m4 — re-exporting
  not-yet-defined `apply_ime`/`Composition*` would be an unresolved-import error):

  ```rust
  pub use ime::{PreeditSpan, PREEDIT_METADATA};
  ```

  Each later task EXTENDS this line as it lands its symbol: Task 5 adds
  `apply_ime, CompositionStart, CompositionUpdate, CompositionEnd`; Task 6 adds
  `write_ime_window`. By campaign end the line reads
  `pub use ime::{PreeditSpan, PREEDIT_METADATA, apply_ime, write_ime_window,
  CompositionStart, CompositionUpdate, CompositionEnd};`.

  Update `state.rs`'s module-doc E1 field-set note to record that E5 added `preedit`
  (change "E5 `preedit`" stays; the deferral line is now satisfied — no code change
  required beyond the field).

- [ ] **RUN IT — passes.**

  ```sh
  cargo test -p buiy_core --test text_ime_ops
  ```

  Expected: `test result: ok. 3 passed`.

- [ ] **Commit.** `feat(text-editing): E5 Task 1 — PreeditSpan + splice/remove
  primitives (invariant a)`.

---

## Task 2 — Refine `value()` to exclude the live preedit (invariant b)

E2's `value()` joins all line text; with a live preedit spliced in, it would leak the
composing string. Subtract the recorded span.

- [ ] **RED — add the failing value-exclusion test** to
  `crates/buiy_core/tests/text_ime_ops.rs`:

  ```rust
  /// Invariant (b): `value()` excludes the live preedit byte range even when
  /// the preedit is mid-line (the reflow case the splice exists for).
  #[test]
  fn value_excludes_preedit_midline() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();

      // "hello world", caret moved to byte 5 (between "hello" and " world").
      state.apply(&mut fs, EditCommand::Insert("hello world".into()), false, false);
      for _ in 0..6 {
          state.apply(&mut fs, cosmic_motion_left(), false, false);
      }
      // Caret now at index 5. Splice a preedit there.
      state.splice_preedit(&mut fs, "XYZ", None);
      assert_eq!(state.buffer_text_for_test(), "helloXYZ world", "preedit shapes mid-line");
      assert_eq!(state.value(), "hello world", "logical value excludes the preedit");

      state.remove_preedit(&mut fs);
      assert_eq!(state.value(), "hello world");
  }

  fn cosmic_motion_left() -> EditCommand {
      EditCommand::Motion(cosmic_text::Motion::Left, false)
  }
  ```

- [ ] **RUN IT — fails.**

  ```sh
  cargo test -p buiy_core --test text_ime_ops value_excludes_preedit_midline
  ```

  Expected: `assertion ... left: "helloXYZ world", right: "hello world"` — the
  current `value()` includes the preedit.

- [ ] **GREEN — refine `value()`** in `crates/buiy_core/src/text/edit/state.rs`.
  Replace the body of `value()` so the preedit line subtracts the span:

  ```rust
      /// The logical value: the editor buffer's full text with the live preedit
      /// byte range REMOVED (editing-and-ime § 6.2b — value reads exclude
      /// preedit). When not composing this is the complete buffer content.
      /// Lines are joined with `\n` (the LF value contract; per-line endings
      /// live separately on `BufferLine::ending`).
      pub fn value(&self) -> String {
          use cosmic_text::Edit;
          let preedit = self.preedit.clone();
          self.editor.with_buffer(|buffer| {
              let mut out = String::new();
              for (i, line) in buffer.lines.iter().enumerate() {
                  if i > 0 {
                      out.push('\n');
                  }
                  let text = line.text();
                  match &preedit {
                      Some(span) if span.line == i => {
                          // Subtract [start, end): the bytes the preedit occupies.
                          out.push_str(&text[..span.start]);
                          out.push_str(&text[span.end()..]);
                      }
                      _ => out.push_str(text),
                  }
              }
              out
          })
      }
  ```

- [ ] **RUN IT — passes** (rerun the whole file to confirm Task 1 still green):

  ```sh
  cargo test -p buiy_core --test text_ime_ops
  ```

  Expected: `test result: ok. 4 passed`.

- [ ] **Commit.** `feat(text-editing): E5 Task 2 — value() excludes the live preedit
  (invariant b)`.

---

## Task 3 — The commit path = exactly one undo unit (invariant c)

`commit_preedit` already exists (Task 1). Prove it records exactly one
`GroupKind::Composition` unit, that undo restores the pre-composition state, and that
the splice that preceded the commit added nothing.

- [ ] **RED — add the failing commit test** to
  `crates/buiy_core/tests/text_ime_ops.rs`:

  ```rust
  use std::time::Duration;

  /// Invariant (c): a full composition (one or more Preedit splices then a
  /// Commit) records EXACTLY ONE undo unit, grouped `Composition`; undoing it
  /// restores the pre-composition value in ONE step.
  #[test]
  fn commit_is_exactly_one_composition_undo_unit() {
      use buiy_core::text::edit::GroupKind;
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();

      state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
      let undo_before = state.undo_depth();

      // Compose: two Preedit updates, then Commit "好".
      state.splice_preedit(&mut fs, "h", None);
      state.splice_preedit(&mut fs, "ha", None);
      assert_eq!(state.undo_depth(), undo_before, "no preedit splice reached undo");

      state.commit_preedit(&mut fs, "好", Duration::ZERO);
      assert_eq!(state.value(), "ab好");
      assert!(!state.has_preedit(), "commit clears the span");
      assert_eq!(state.undo_depth(), undo_before + 1, "commit = ONE undo unit");
      assert_eq!(state.undo_top_group_for_test(), Some(GroupKind::Composition));

      // One Undo restores the pre-composition value.
      state.apply(&mut fs, EditCommand::Undo, false, false);
      assert_eq!(state.value(), "ab", "undo removes the whole commit in one step");
  }

  /// A composition does NOT coalesce into a preceding typing run (Composition
  /// never coalesces; the commit seals the open group first).
  #[test]
  fn commit_does_not_coalesce_with_prior_typing() {
      let fonts = SharedFontSystem::new();
      let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
      let mut fs = fonts.lock();

      state.apply(&mut fs, EditCommand::Insert("x".into()), false, false); // a TypingRun unit
      let depth = state.undo_depth();
      state.splice_preedit(&mut fs, "a", None);
      state.commit_preedit(&mut fs, "亜", Duration::from_millis(10));
      assert_eq!(state.undo_depth(), depth + 1, "commit is its own unit, not coalesced");
  }
  ```

  Add a tiny test accessor to `state.rs` so the integration test can read the top
  group (the existing `undo_top_group` is `pub(crate)`):

  In `crates/buiy_core/src/text/edit/state.rs`, add to the `impl TextEditState`:

  ```rust
      /// Test/inspection: the `GroupKind` Undo would pop next (top of the undo
      /// stack). Stays inside the facade.
      pub fn undo_top_group_for_test(&self) -> Option<super::undo::GroupKind> {
          self.undo_top_group()
      }
  ```

  Re-export `GroupKind` is already done by E4 (`mod.rs:34`).

- [ ] **RUN IT — fails to compile / fails.**

  ```sh
  cargo test -p buiy_core --test text_ime_ops commit_is_exactly_one_composition_undo_unit
  ```

  Expected: `error[E0599]: no method named undo_top_group_for_test` (before adding the
  accessor); after adding it, the test passes IF Task 1's `commit_preedit` is correct.
  (This task is largely a verification of Task 1's commit via undo-stack assertions;
  the only production change is the test accessor.)

- [ ] **GREEN — add the accessor** (above), confirm `commit_preedit` is correct.

- [ ] **RUN IT — passes.**

  ```sh
  cargo test -p buiy_core --test text_ime_ops
  ```

  Expected: `test result: ok. 6 passed`.

- [ ] **Commit.** `feat(text-editing): E5 Task 3 — commit = one Composition undo unit
  (invariant c)`.

---

## Task 4 — `PreeditVisual` paint seat + the preedit underline at extract

The state-side preedit is proven. Now paint it: a new component carrying the span's
byte range, written by the E3 geometry writer, emitted as an underline quad by
`extract_buiy_glyphs`.

- [ ] **RED — write the failing PreeditVisual extract test.** Create
  `crates/buiy_core/tests/text_preedit_paint.rs`:

  ```rust
  //! E5 — the preedit underline paint seat (editing-and-ime § 6.2; decoration-
  //! and-paint § 8). The composing span forces a quad-tier underline over its
  //! byte range, via the existing `ExtractedTextQuads` carrier (no new GPU
  //! work). Headless: this drives the render-world extract producer and asserts
  //! an underline quad is emitted for the preedit range — no adapter (extract
  //! runs CPU-side).

  use bevy::prelude::*;
  use buiy_core::layout::Style;
  use buiy_core::text::components::PreeditVisual;
  use buiy_core::text::Text;
  use buiy_core::Node;
  use cosmic_text::Cursor;

  /// A `PreeditVisual` over a byte range emits at least one quad-tier underline
  /// instance (the preedit underline). A collapsed range emits none.
  #[test]
  fn preedit_visual_constructs_and_reports_collapsed() {
      let v = PreeditVisual::new(Cursor::new(0, 2), Cursor::new(0, 5));
      assert!(!v.is_collapsed());
      let empty = PreeditVisual::new(Cursor::new(0, 3), Cursor::new(0, 3));
      assert!(empty.is_collapsed());
      // start/end normalize (start <= end).
      let swapped = PreeditVisual::new(Cursor::new(0, 5), Cursor::new(0, 2));
      assert_eq!((swapped.start.index, swapped.end.index), (2, 5));
  }
  ```

  (The end-to-end "an underline quad is emitted" assertion is exercised by the GPU
  golden in Task 7 and by the extract producer's existing selection-pre-pass tests;
  the unit test here pins the component contract. The extract emission is verified by
  the headless render_extract harness if the executor wants a non-GPU emission check —
  see the optional assertion note below.)

- [ ] **RUN IT — fails to compile.**

  ```sh
  cargo test -p buiy_core --test text_preedit_paint
  ```

  Expected: `error[E0432]: unresolved import ... PreeditVisual`.

- [ ] **GREEN — add the `PreeditVisual` component** to
  `crates/buiy_core/src/text/components.rs` (after `SelectionVisual`'s impl block,
  ~line 444):

  ```rust
  /// The IME preedit underline paint-input state (editing-and-ime § 6.2;
  /// decoration-and-paint § 8 — a quad-tier underline FORCED over the composing
  /// byte range). The NORMALIZED endpoint pair (`start <= end`), the same shape
  /// as `SelectionVisual` but a distinct seat: its source is IME state, its
  /// geometry is an underline strip (not a full-height box), its color is the
  /// preedit token. Written/removed by the E5 geometry pass from
  /// `TextEditState::preedit`; read by the glyph producer, which derives the
  /// underline rects via `LayoutRun::highlight` (the selection-pre-pass idiom)
  /// and rides the existing `ExtractedTextQuads` carrier — no new GPU work.
  /// Presence = "a composition is underway"; REMOVAL clears it. A collapsed
  /// pair paints nothing. Machinery state — not reflect-registered (carries
  /// `cosmic_text::Cursor`; this module IS the cosmic boundary, the
  /// `SelectionVisual` precedent).
  #[derive(Component, Clone, Copy, PartialEq, Debug)]
  pub struct PreeditVisual {
      /// Logically-first endpoint (`start <= end`).
      pub start: Cursor,
      /// Logically-last endpoint.
      pub end: Cursor,
  }

  impl PreeditVisual {
      /// Build from an UNORDERED endpoint pair, normalizing to `start <= end`
      /// ((line, index) lexicographic).
      pub fn new(a: Cursor, b: Cursor) -> Self {
          if (b.line, b.index) < (a.line, a.index) {
              Self { start: b, end: a }
          } else {
              Self { start: a, end: b }
          }
      }

      /// `start == end` (position-wise) — paints nothing.
      pub fn is_collapsed(&self) -> bool {
          (self.start.line, self.start.index) == (self.end.line, self.end.index)
      }
  }
  ```

  Ensure `PreeditVisual` is exported wherever `SelectionVisual` is — check the
  `pub use` in `crates/buiy_core/src/text/mod.rs` and `components.rs`'s re-export list
  and add `PreeditVisual` alongside `SelectionVisual`/`CaretVisual`.

- [ ] **GREEN — emit the underline at extract.** In
  `crates/buiy_core/src/text/extract.rs`, add `PreeditVisual` to the producer:

  1. Import it (top of file, alongside `SelectionVisual`/`CaretVisual`):
     `use crate::text::components::PreeditVisual;` (or extend the existing
     `components::{...}` import group).

  2. Add `Option<&PreeditVisual>` to the main query tuple (after
     `Option<&CaretColor>`, ~line 203 — this is the 14th element, under Bevy's
     15-tuple cap):

     ```rust
                 Option<&CaretColor>,
                 // E5: the preedit underline span (editing-and-ime § 6.2). Its
                 // own seat — distinct color + underline geometry — on the same
                 // quad carrier. 14 of Bevy's 15-tuple limit.
                 Option<&PreeditVisual>,
     ```

  3. Add it to the destructure (`text.get(entity)`, ~line 380):

     ```rust
             caret_color,
             preedit_visual,
     ```

  4. Add `Changed<PreeditVisual>` to the § 6.2 damage trigger union (after
     `Changed<CaretColor>`, ~line 239):

     ```rust
                     Changed<CaretColor>,
                     // E5: a composition start/update/end re-emits the preedit
                     // underline; steady frames rebuild nothing.
                     Changed<PreeditVisual>,
     ```

  5. Add `Extract<RemovedComponents<PreeditVisual>>` to the `removed` tuple (after
     `Extract<RemovedComponents<CaretVisual>>`, ~line 252) — removal IS the clear,
     so the entity must re-emit (with no preedit quad) on commit/cancel:

     ```rust
         Extract<RemovedComponents<CaretVisual>>,
         Extract<RemovedComponents<PreeditVisual>>,
     ```

     ...and drain it with the other removal streams (find where the existing removal
     streams are read — `removed.3`/`removed.4` etc. — and add the new index to the
     same union of "entities to force-rebuild"). Follow the exact pattern the
     `SelectionVisual`/`CaretVisual` removal streams already use (~lines 264-300).

  6. Emit the underline quad — after the selection pre-pass (~line 480, BEFORE the
     decoration walk so the underline rides quad seat 2 with selection), add:

     ```rust
             // E5 (editing-and-ime § 6.2; decoration-and-paint § 8): the preedit
             // underline — a forced single underline over the composing byte
             // range, quad-tier. Mirrors the selection pre-pass: highlight the
             // span per run, then push a THIN underline strip (not a full-height
             // box) at the run baseline. Reuses the quad carrier; no new GPU.
             let preedit = preedit_visual.filter(|p| !p.is_collapsed());
             if let Some(pre) = preedit {
                 let color = resolve_preedit_underline(theme, resolved_entity_color);
                 let color = if blocked { color.with_alpha(0.0) } else { color };
                 if color.alpha() > 0.0 {
                     access.with_buffer(|buffer| {
                         for run in buffer.layout_runs() {
                             if run.line_i < pre.start.line || run.line_i > pre.end.line {
                                 continue;
                             }
                             // Underline strip thickness: 1 logical px at the
                             // run baseline bottom (decoration-and-paint § 8 uses
                             // the standard underline metric; a 1px strip is the
                             // v1 forced underline — the engine has no per-font
                             // underline metric exposed at this seat).
                             let thickness = 1.0_f32;
                             let strip_top = run.line_top + run.line_height - thickness;
                             for (x, w) in run.highlight(pre.start, pre.end) {
                                 if w <= 0.0 {
                                     continue;
                                 }
                                 new_quads.push(TextQuad {
                                     entity,
                                     position: Vec2::new(origin.x + x, origin.y + strip_top),
                                     size: Vec2::new(w, thickness),
                                     color,
                                     clip: eff_clip,
                                 });
                             }
                         }
                     });
                 }
             }
     ```

  7. Add the color resolver. In `crates/buiy_core/src/render/color.rs`, add a preedit
     underline token + resolver (after `resolve_caret_color`, ~line 200):

     ```rust
     /// Preedit (IME composition) underline token (editing-and-ime § 6.2;
     /// decoration-and-paint § 8). Opt-in like the caret token; absent ⇒ the
     /// entity's resolved foreground (currentColor parity — the composing text
     /// is underlined in its own ink).
     pub const PREEDIT_UNDERLINE_TOKEN: &str = "color.text.preedit-underline";

     /// `preedit-underline` color: the `color.text.preedit-underline` theme key
     /// if present, else `current` (the entity's resolved foreground).
     pub fn resolve_preedit_underline(theme: &Theme, current: Color) -> Color {
         theme.color(PREEDIT_UNDERLINE_TOKEN).unwrap_or(current)
     }
     ```

     Import `resolve_preedit_underline` into `extract.rs` alongside
     `resolve_selection_bg`/`resolve_selection_fg`.

- [ ] **RUN IT — passes** (the component contract test):

  ```sh
  cargo test -p buiy_core --test text_preedit_paint
  ```

  Expected: `test result: ok. 1 passed`.

- [ ] **RUN the workspace build to confirm extract compiles** (the query/removal
  edits are the risk):

  ```sh
  cargo build -p buiy_core --all-targets
  ```

  Expected: clean build (no tuple-arity error; the producer is at 14/15).

- [ ] **Commit.** `feat(text-editing): E5 Task 4 — PreeditVisual seat + underline at
  extract`.

---

## Task 5 — `apply_ime` system: drive splice/commit/remove from `Ime` Messages

Wire the state primitives to Bevy's `Ime` Messages, focus-gated, in `BuiySet::Input`,
with the M1 dirty-mark. Emit `CompositionStart/Update/End`.

- [ ] **RED — write the failing system test.** Create
  `crates/buiy_core/tests/text_ime_system.rs`:

  ```rust
  //! E5 — the IME state machine through the real `apply_ime` system
  //! (editing-and-ime §§ 6.1, 6.2, 6.3, 11). Synthetic `Ime` Messages — NO
  //! winit window needed (the state machine is platform-independent; the
  //! real-IME-per-platform matrix is named CI-impossible, spec § 12). Headless,
  //! no adapter. Asserts the four invariants at the SYSTEM level + the
  //! Composition Message taxonomy.

  use bevy::input::keyboard::{Key, KeyboardInput};
  use bevy::input::ButtonState;
  use bevy::prelude::*;
  use bevy::window::Ime;
  use buiy_core::layout::Style;
  use buiy_core::text::edit::{
      CompositionEnd, CompositionStart, CompositionUpdate, GroupKind, TextEditState,
  };
  use buiy_core::text::edit::Clipboard;
  use buiy_core::text::edit::MemClipboard;
  use buiy_core::text::Text;
  use buiy_core::{FocusedEntity, Node};
  use cosmic_text::Metrics;

  fn app_with_focused_editor() -> (App, Entity) {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(buiy_core::CorePlugin);
      app.add_plugins(buiy_core::layout::LayoutPlugin);
      app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
      app.add_plugins(buiy_core::focus::FocusPlugin);
      app.add_message::<KeyboardInput>();
      app.add_message::<Ime>();
      app.insert_resource(ButtonInput::<KeyCode>::default());
      app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
      let editor = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(300.0).height_px(60.0),
              Text(String::new()),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      app.world_mut().resource_mut::<Time<Virtual>>().pause();
      app.update();
      (app, editor)
  }

  fn send_preedit(app: &mut App, value: &str, cursor: Option<(usize, usize)>) {
      app.world_mut().write_message(Ime::Preedit {
          window: Entity::PLACEHOLDER,
          value: value.to_string(),
          cursor,
      });
  }
  fn send_commit(app: &mut App, value: &str) {
      app.world_mut().write_message(Ime::Commit {
          window: Entity::PLACEHOLDER,
          value: value.to_string(),
      });
  }
  fn value(app: &App, e: Entity) -> String {
      app.world().get::<TextEditState>(e).unwrap().value()
  }
  fn undo_depth(app: &App, e: Entity) -> usize {
      app.world().get::<TextEditState>(e).unwrap().undo_depth()
  }
  // Count the messages of type `M` buffered this frame (the
  // `text_undo_system.rs:140-142` idiom: a fresh cursor reads the frame's
  // still-buffered messages; fully-qualified path, no extra import).
  fn count<M: bevy::ecs::message::Message>(app: &App) -> usize {
      let messages = app.world().resource::<bevy::ecs::message::Messages<M>>();
      let mut cursor = messages.get_cursor();
      cursor.read(messages).count()
  }

  /// Invariant (a) + (b) at the system level: during composition the undo stack
  /// is unchanged and the logical value excludes the preedit.
  #[test]
  fn composition_leaves_undo_and_value_clean() {
      let (mut app, editor) = app_with_focused_editor();
      let undo_before = undo_depth(&app, editor);

      send_preedit(&mut app, "n", Some((0, 1)));
      app.update();
      assert!(app.world().get::<TextEditState>(editor).unwrap().has_preedit());
      assert_eq!(value(&app, editor), "", "value excludes preedit (b)");
      assert_eq!(undo_depth(&app, editor), undo_before, "undo unchanged during composition (a)");

      send_preedit(&mut app, "ni", Some((0, 2)));
      app.update();
      assert_eq!(value(&app, editor), "", "still excluded after update");
      assert_eq!(undo_depth(&app, editor), undo_before);
  }

  /// Invariant (c): Commit = exactly one Composition undo unit; TextChanged
  /// fires on commit (value changed) but NOT on preedit.
  #[test]
  fn commit_is_one_unit_and_fires_textchanged() {
      use buiy_core::text::edit::TextChanged;
      let (mut app, editor) = app_with_focused_editor();
      let undo_before = undo_depth(&app, editor);

      send_preedit(&mut app, "ni", None);
      app.update();
      // No TextChanged from a preedit.
      assert_eq!(count::<TextChanged>(&app), 0, "preedit does not change the value");

      send_commit(&mut app, "你");
      app.update();
      assert_eq!(value(&app, editor), "你");
      assert!(!app.world().get::<TextEditState>(editor).unwrap().has_preedit());
      assert_eq!(undo_depth(&app, editor), undo_before + 1, "commit = one unit (c)");
      assert_eq!(count::<TextChanged>(&app), 1, "commit fires TextChanged once");
  }

  /// Invariant (d): an empty Preedit (cancel) and Ime::Disabled both remove the
  /// span — no orphan.
  #[test]
  fn empty_preedit_and_disabled_remove_the_span() {
      let (mut app, editor) = app_with_focused_editor();
      send_preedit(&mut app, "abc", None);
      app.update();
      assert!(app.world().get::<TextEditState>(editor).unwrap().has_preedit());

      // Empty Preedit cancels.
      send_preedit(&mut app, "", None);
      app.update();
      assert!(!app.world().get::<TextEditState>(editor).unwrap().has_preedit(), "empty preedit clears (d)");
      assert_eq!(value(&app, editor), "");

      // Re-compose, then Disabled clears.
      send_preedit(&mut app, "xyz", None);
      app.update();
      app.world_mut().write_message(Ime::Disabled { window: Entity::PLACEHOLDER });
      app.update();
      assert!(!app.world().get::<TextEditState>(editor).unwrap().has_preedit(), "Disabled clears (d)");
      assert_eq!(value(&app, editor), "");
  }

  /// The Composition Message taxonomy (§ 11): Start on empty→nonempty, Update
  /// on nonempty→nonempty, End on commit.
  #[test]
  fn composition_messages_emit_on_transitions() {
      let (mut app, editor) = app_with_focused_editor();
      let _ = editor;

      send_preedit(&mut app, "n", None);
      app.update();
      assert_eq!(count::<CompositionStart>(&app), 1, "Start on empty→nonempty");
      assert_eq!(count::<CompositionUpdate>(&app), 0);

      send_preedit(&mut app, "ni", None);
      app.update();
      assert_eq!(count::<CompositionStart>(&app), 0);
      assert_eq!(count::<CompositionUpdate>(&app), 1, "Update on nonempty→nonempty");

      send_commit(&mut app, "你");
      app.update();
      assert_eq!(count::<CompositionEnd>(&app), 1, "End on commit");
  }

  /// M1 REGRESSION: the splice/remove preserve the line's RESOLVED attrs. A
  /// BOLD editor (weight 700, seeded by TextSync's `span_attrs`, not the
  /// default `value()`/`apply` path) must keep weight 700 across a
  /// compose+cancel — a bare `AttrsList::new(&Attrs::new())` would flatten it
  /// to 400 and persist (the splice never touches `Text`, so TextSync never
  /// re-seeds). This system-level test runs the REAL TextSync seam (the unit
  /// path in `text_ime_ops.rs` cannot — `apply`/`set_text` seed cosmic
  /// defaults, not resolved attrs).
  #[test]
  fn composition_preserves_resolved_line_attrs() {
      use buiy_core::text::FontWeight;
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(buiy_core::CorePlugin);
      app.add_plugins(buiy_core::layout::LayoutPlugin);
      app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
      app.add_plugins(buiy_core::focus::FocusPlugin);
      app.add_message::<KeyboardInput>();
      app.add_message::<Ime>();
      app.insert_resource(ButtonInput::<KeyCode>::default());
      app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
      let editor = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(300.0).height_px(60.0),
              Text("ab".to_string()), // non-empty so TextSync seeds line 0
              FontWeight(700),         // BOLD — the resolved attr that must survive
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      app.update(); // TextSync seeds line 0 with weight-700 attrs

      let weight_before = app
          .world()
          .get::<TextEditState>(editor)
          .unwrap()
          .line_default_weight_for_test(0);
      assert_eq!(weight_before, 700, "TextSync seeded the bold weight");

      // Compose a preedit, then cancel it.
      send_preedit(&mut app, "ni", None);
      app.update();
      send_preedit(&mut app, "", None); // cancel
      app.update();

      let weight_after = app
          .world()
          .get::<TextEditState>(editor)
          .unwrap()
          .line_default_weight_for_test(0);
      assert_eq!(
          weight_after, 700,
          "the line's resolved weight survives compose+cancel (M1)"
      );
  }
  ```

- [ ] **RUN IT — fails to compile.**

  ```sh
  cargo test -p buiy_core --test text_ime_system
  ```

  Expected: `error[E0432]: unresolved import ... CompositionStart` (and the system is
  not yet registered). After Task 5's GREEN, `composition_preserves_resolved_line_attrs`
  passes ONLY because Task 1's splice/remove preserve `defaults()` — a bare-attrs
  splice would fail it with `left: 400, right: 700`.

- [ ] **GREEN — add the Messages + `apply_ime`** to
  `crates/buiy_core/src/text/edit/ime.rs`. Append the Messages:

  ```rust
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
  ```

  Append the system (the `apply_keyboard_edits` shape — `text/edit/input.rs:429`):

  ```rust
  use bevy::input::keyboard::KeyboardInput;
  use bevy::window::Ime;

  use super::input::TextChanged;
  use super::state::{Disabled, ReadOnly};
  use crate::layout::LayoutTree;
  use crate::text::SharedFontSystem;
  use crate::FocusedEntity;

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
  ```

  Register in `crates/buiy_core/src/text/mod.rs` `BuiyTextPlugin::build()` (after the
  E2/E4 keyboard registration block, ~line 222):

  ```rust
          // E5 (editing-and-ime §§ 6, 11): the focus-gated IME system + the
          // composition Message taxonomy. BuiySet::Input alongside the keyboard
          // path — winit sends EITHER Ime OR KeyboardInput per keystroke
          // (window.rs ime_enabled doc), so they do not race. Inert headless
          // (Option params no-op without Ime / Focus / Layout infra).
          app.add_message::<crate::text::edit::CompositionStart>();
          app.add_message::<crate::text::edit::CompositionUpdate>();
          app.add_message::<crate::text::edit::CompositionEnd>();
          app.add_systems(
              Update,
              crate::text::edit::apply_ime.in_set(crate::BuiySet::Input),
          );
  ```

  Extend the `pub use ime::{...}` re-export in `mod.rs` to include `apply_ime`,
  `CompositionStart`, `CompositionUpdate`, `CompositionEnd` (added in Task 1's
  re-export stub).

- [ ] **RUN IT — passes.**

  ```sh
  cargo test -p buiy_core --test text_ime_system
  ```

  Expected: `test result: ok. 5 passed` (the four invariant/taxonomy tests + the
  M1 attrs-preservation regression).

- [ ] **Commit.** `feat(text-editing): E5 Task 5 — apply_ime system + Composition
  Messages (invariants a/b/c/d at system level) + M1 attrs preservation`.

---

## Task 6 — Write `PreeditVisual` from state + popup positioning (`ime_position`)

The render-prep pass must (1) project `state.preedit` into `PreeditVisual` so the
underline paints, and (2) set `Window.ime_enabled` + `Window.ime_position`. Fold the
`PreeditVisual` write into the E3 writer (one pass, one source of truth) and add a
separate small `write_ime_window` system for the window fields.

- [ ] **RED — write the failing geometry/window test.** Create
  `crates/buiy_core/tests/text_ime_window.rs`:

  ```rust
  //! E5 — the preedit geometry projection + IME popup positioning
  //! (editing-and-ime § 6.3). `PreeditVisual` mirrors the live span each
  //! render-prep frame; `Window.ime_enabled` is true while a focused, non-
  //! ReadOnly, non-Disabled editor exists, and `Window.ime_position` tracks
  //! the caret bottom-left in logical window coords. Headless: a synthetic
  //! Window entity, no winit — bevy_winit forwards these fields, but the math
  //! is testable without it.

  use bevy::prelude::*;
  use bevy::window::{Ime, PrimaryWindow, Window};
  use buiy_core::layout::Style;
  use buiy_core::text::components::PreeditVisual;
  use buiy_core::text::edit::TextEditState;
  use buiy_core::text::Text;
  use buiy_core::{FocusedEntity, Node};
  use cosmic_text::Metrics;

  fn app_with_window_and_editor() -> (App, Entity, Entity) {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(buiy_core::CorePlugin);
      app.add_plugins(buiy_core::layout::LayoutPlugin);
      app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
      app.add_plugins(buiy_core::focus::FocusPlugin);
      app.add_message::<Ime>();
      let window = app
          .world_mut()
          .spawn((Window::default(), PrimaryWindow))
          .id();
      let editor = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(300.0).height_px(60.0),
              Text(String::new()),
              TextEditState::new(Metrics::new(16.0, 19.2)),
          ))
          .id();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      app.update();
      (app, window, editor)
  }

  /// A focused, non-ReadOnly editor sets `ime_enabled = true`.
  #[test]
  fn focused_editor_enables_ime() {
      let (mut app, window, _editor) = app_with_window_and_editor();
      app.update();
      assert!(
          app.world().get::<Window>(window).unwrap().ime_enabled,
          "a focused editable enables IME (§ 6.3)"
      );
  }

  /// Unfocusing the editor turns IME off.
  #[test]
  fn unfocus_disables_ime() {
      let (mut app, window, _editor) = app_with_window_and_editor();
      app.update();
      app.world_mut().resource_mut::<FocusedEntity>().0 = None;
      app.update();
      assert!(
          !app.world().get::<Window>(window).unwrap().ime_enabled,
          "no focused editor disables IME"
      );
  }

  /// A live preedit projects into a non-collapsed `PreeditVisual` on the editor.
  #[test]
  fn preedit_projects_into_preedit_visual() {
      let (mut app, _window, editor) = app_with_window_and_editor();
      // Type a logical char, then compose a preedit via the Ime message.
      app.world_mut().write_message(Ime::Preedit {
          window: Entity::PLACEHOLDER,
          value: "ni".to_string(),
          cursor: Some((0, 2)),
      });
      app.update(); // apply_ime splices
      app.update(); // measure/commit reshapes; geometry writer projects
      let pv = app.world().get::<PreeditVisual>(editor);
      assert!(pv.is_some(), "a live preedit yields a PreeditVisual");
      assert!(!pv.unwrap().is_collapsed(), "the underline span is non-empty");

      // Commit clears it.
      app.world_mut().write_message(Ime::Commit {
          window: Entity::PLACEHOLDER,
          value: "你".to_string(),
      });
      app.update();
      app.update();
      assert!(
          app.world().get::<PreeditVisual>(editor).is_none(),
          "commit removes the PreeditVisual (no orphan underline)"
      );
  }
  ```

- [ ] **RUN IT — fails.**

  ```sh
  cargo test -p buiy_core --test text_ime_window
  ```

  Expected: `ime_enabled` is `false` (no system sets it) and `PreeditVisual` is
  absent (no projection).

- [ ] **GREEN — project `PreeditVisual` in the E3 writer.** In
  `crates/buiy_core/src/text/edit/caret.rs`, extend `write_caret_and_selection` to
  also write/remove `PreeditVisual` from `state.preedit`. Add `Option<&PreeditVisual>`
  to the query tuple, and after the selection block (before the transition detection),
  add:

  ```rust
          // --- Preedit underline geometry into PreeditVisual (E5) ---------------
          // Project the live span into the paint seat: a composition yields a
          // non-collapsed PreeditVisual over [start, end) on the span's line;
          // no composition removes it. The byte range indexes the SAME spliced
          // buffer the underline emitter reads (one source of truth).
          let new_preedit = state.preedit.as_ref().map(|p| {
              (
                  cosmic_text::Cursor::new(p.line, p.start),
                  cosmic_text::Cursor::new(p.line, p.end()),
              )
          });
          match (new_preedit, prev_preedit) {
              (Some((lo, hi)), _) => {
                  let v = crate::text::PreeditVisual::new(lo, hi);
                  if prev_preedit.copied() != Some(v) {
                      commands.entity(entity).insert(v);
                  }
              }
              (None, Some(_)) => {
                  commands.entity(entity).remove::<crate::text::PreeditVisual>();
              }
              (None, None) => {}
          }
  ```

  Add `Option<&crate::text::PreeditVisual>` to the query and `prev_preedit` to the
  destructure (mirror `prev_sel`). Import `PreeditVisual` via `crate::text::`.

  (Note: `prev_preedit.copied()` compares the projected value against the existing
  seat so a steady composition does not re-tick `Changed<PreeditVisual>` every frame —
  the selection seat's idiom.)

- [ ] **GREEN — add `write_ime_window`.** In `ime.rs`, append:

  ```rust
  use bevy::window::{PrimaryWindow, Window};

  use super::caret::caret_rect_for;
  use crate::text::ComputedTextLayout;

  /// Set `Window.ime_enabled` + `Window.ime_position` (editing-and-ime § 6.3).
  /// Runs in render-prep (after BuiySet::Input, before write_caret_blink), so
  /// the `ime_position` write reads THIS frame's caret geometry.
  ///
  /// **`ime_enabled` is decided from focus + markers ALONE (m3)** — true iff a
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
      // Enable decision: focus + markers only (NO geometry — m3).
      enable_q: Query<Has<ReadOnly>, (With<TextEditState>, Without<Disabled>)>,
      // Position write: geometry-bearing, fetched only when present.
      geom_q: Query<
          (&TextEditState, &GlobalTransform, &ComputedTextLayout),
          Without<Disabled>,
      >,
  ) {
      let Ok(mut window) = windows.single_mut() else { return };
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
      if let Some((state, gt, layout)) =
          focused_entity.and_then(|e| geom_q.get(e).ok())
      {
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
  ```

  `caret_rect_for` in `caret.rs` is currently `fn`-private — promote it to
  `pub(crate) fn caret_rect_for(...)` so `ime.rs` can `use super::caret::
  caret_rect_for;` and call it directly. No re-export alias is needed.

  Register `write_ime_window` in `BuiyTextPlugin::build()` next to the E3
  `write_caret_and_selection` registration (~line 219 region), same ordering window:

  ```rust
          app.add_systems(
              Update,
              crate::text::edit::write_ime_window
                  .after(crate::BuiySet::Input)
                  .before(crate::text::visual::write_caret_blink),
          );
  ```

  Add `write_ime_window` to the `pub use ime::{...}` re-export.

- [ ] **RUN IT — passes.**

  ```sh
  cargo test -p buiy_core --test text_ime_window
  ```

  Expected: `test result: ok. 3 passed`.

- [ ] **Commit.** `feat(text-editing): E5 Task 6 — PreeditVisual projection +
  ime_enabled/ime_position (§ 6.3)`.

---

## Task 7 — Escape removes the preedit; the GPU golden (`#[ignore]`, build-only)

Close invariant (d)'s `Escape` arm (the keyboard path, not IME), and add the additive
GPU golden (built here, run by the orchestrator).

- [ ] **RED — add the failing Escape test** to
  `crates/buiy_core/tests/text_ime_system.rs`:

  ```rust
  /// Invariant (d): `Escape` during composition removes the preedit span
  /// (the keyboard path — winit may deliver Escape as KeyboardInput while
  /// composing). Routed through `apply_keyboard_edits`' Escape command, which
  /// E5 extends to clear any live preedit before the editor's Action::Escape.
  #[test]
  fn escape_removes_the_preedit() {
      let (mut app, editor) = app_with_focused_editor();
      send_preedit(&mut app, "abc", None);
      app.update();
      assert!(app.world().get::<TextEditState>(editor).unwrap().has_preedit());

      // Send Escape as a KeyboardInput.
      app.world_mut().write_message(KeyboardInput {
          key_code: KeyCode::Escape,
          logical_key: Key::Escape,
          state: ButtonState::Pressed,
          text: None,
          repeat: false,
          window: Entity::PLACEHOLDER,
      });
      app.update();
      assert!(
          !app.world().get::<TextEditState>(editor).unwrap().has_preedit(),
          "Escape clears the preedit (d)"
      );
      assert_eq!(value(&app, editor), "", "value unchanged by Escape-cancel");
  }
  ```

- [ ] **RUN IT — fails** (Escape currently only runs `Action::Escape`, leaving the
  spliced span in the buffer + the `PreeditSpan` set).

  ```sh
  cargo test -p buiy_core --test text_ime_system escape_removes_the_preedit
  ```

- [ ] **GREEN — extend the Escape arm** in
  `crates/buiy_core/src/text/edit/input.rs` `apply_tracked` (the `EditCommand::Escape`
  arm, ~line 167). Remove any live preedit before the editor's `Action::Escape`:

  ```rust
              EditCommand::Escape => {
                  self.undo.seal();
                  // E5: Escape cancels an active composition (editing-and-ime
                  // § 6.2d) — remove the spliced span (no Change) before the
                  // editor's own Escape (which clears the selection).
                  if self.has_preedit() {
                      self.remove_preedit(font_system);
                  }
                  self.editor.action(font_system, Action::Escape);
                  EditOutcome::default()
              }
  ```

  (`remove_preedit` takes `&mut FontSystem`; `apply_tracked` has `font_system` in
  scope. `has_preedit`/`remove_preedit` are now on `TextEditState` from Task 1.)

  The Escape path does NOT change the logical value (the preedit was excluded from
  `value()`), so `EditOutcome::default()` is correct — no `TextChanged`. But the
  buffer content DID change (the span was removed), so the M1 dirty-mark must still
  fire. `apply_keyboard_edits` keys the dirty-mark on `any_value_change`, which is
  false here. Fix: in `apply_keyboard_edits` (`input.rs:563`), also dirty-mark when an
  Escape cleared a preedit. The minimal robust fix — have `EditOutcome` carry a
  `buffer_reshaped` flag, OR (simpler, DRY) detect preedit clearing. Cleanest: extend
  `EditOutcome` with a `reshaped: bool` set true whenever the buffer content changed
  even if the logical value did not. Update `EditOutcome` (`input.rs:27`):

  ```rust
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
  pub struct EditOutcome {
      pub value_changed: bool,
      pub submitted: bool,
      /// The buffer content changed even if the logical value did not (an
      /// Escape that cleared a preedit) — needs the M1 re-measure dirty-mark
      /// but no `TextChanged` (editing-and-ime § 6.2: preedit is not value).
      pub reshaped: bool,
  }
  ```

  Set `reshaped: true` in the Escape arm when a preedit was cleared:

  ```rust
              EditCommand::Escape => {
                  self.undo.seal();
                  let cleared = self.has_preedit();
                  if cleared {
                      self.remove_preedit(font_system);
                  }
                  self.editor.action(font_system, Action::Escape);
                  EditOutcome { reshaped: cleared, ..Default::default() }
              }
  ```

  In `apply_keyboard_edits` (`input.rs:540` region), track a `any_reshape` alongside
  `any_value_change`, and gate the dirty-mark on `any_value_change || any_reshape`,
  but the `TextChanged` write only on `any_value_change`:

  ```rust
      let mut any_value_change = false;
      let mut any_reshape = false;
      for command in commands {
          // ... existing ...
          let outcome = state.apply_tracked(&mut font_system, command, &mut ctx);
          any_value_change |= outcome.value_changed;
          any_reshape |= outcome.reshaped;
          // ... existing undo/redo Message writes ...
      }
      drop(font_system);

      if any_value_change || any_reshape {
          state.invalidate_intrinsics();
          if let Some(tree) = tree.as_deref_mut() {
              tree.mark_dirty_for_entity(entity);
          }
      }
      if any_value_change {
          changed.write(TextChanged(entity));
      }
  ```

  **Update EVERY `EditOutcome { … }` literal (m2 — the new field is not `Default`
  on a literal).** Adding `reshaped: bool` to the struct breaks all five sites that
  construct it with explicit fields; each must add `reshaped: false` (none of them
  reshape without a value change). The exact sites in `input.rs` (the `EditOutcome::
  default()` callers are fine — `Default` derives the new field):
  - `:124` — Enter single-line `EditOutcome { value_changed: false, submitted: true }`
    → add `reshaped: false`.
  - `:172` — `EditCommand::Submit => EditOutcome { value_changed: false, submitted:
    true }` → add `reshaped: false`.
  - `:266` — `tracked_edit`'s return `EditOutcome { value_changed: self.value() !=
    before, submitted: false }` → add `reshaped: false`.
  - `:286` — `apply_undo` `EditOutcome { value_changed: changed, submitted: false }`
    → add `reshaped: false`.
  - `:300` — `apply_redo` `EditOutcome { value_changed: changed, submitted: false }`
    → add `reshaped: false`.

  (The `EditContext`-fallback `apply` shim and every `return EditOutcome::default()`
  arm — read-only guards, no-selection Copy/Cut/Paste — pick up `reshaped: false`
  from `Default`, no edit needed. Confirm the build is clean after the field add:
  `cargo build -p buiy_core` will name any literal still missing the field.)

- [ ] **RUN IT — passes** (rerun the whole IME system suite):

  ```sh
  cargo test -p buiy_core --test text_ime_system
  ```

  Expected: `test result: ok. 6 passed` (Task 5's five + `escape_removes_the_preedit`).

- [ ] **BUILD-ONLY — the GPU golden** (`#[ignore]`, additive — the orchestrator runs
  the GPU lane). Create `crates/buiy_core/tests/text_ime_preedit_gpu.rs`, building on
  the `text_caret_selection_e3_gpu.rs` / `text_selection_caret_gpu.rs` harness
  (`tests/support/mod.rs`: `gpu_render_app` / `render_to_image` / `readback_rgba`):

  ```rust
  //! E5 GPU golden (#[ignore], additive — CLAUDE.md GPU lane): a FOCUSED editor
  //! with a live preedit composition, driven END-TO-END (Ime::Preedit →
  //! apply_ime splice → measure/commit reshape → write_caret_and_selection
  //! projects PreeditVisual → extract emits the underline quad → pixels), on a
  //! mixed-direction fixture (caret + selection + preedit underline together,
  //! spec § 12). The real-IME-per-platform matrix (CJK + dead keys) is named
  //! CI-impossible (§ 12) — this golden proves the PAINT path, not winit IME.
  //!
  //! Build-only in E5: the orchestrator runs the GPU lane. The assertion is
  //! that a distinct preedit-underline color band appears under the composing
  //! glyphs (a chroma-distinct underline token makes the strip detectable).
  //!
  //! Run: cargo test -p buiy_core --test text_ime_preedit_gpu -- --ignored --test-threads=1

  mod support;

  use bevy::prelude::*;
  use bevy::window::Ime;
  use buiy_core::layout::Style;
  use buiy_core::render::golden::perceptual_diff;
  use buiy_core::text::edit::{EditCommand, TextEditState};
  use buiy_core::text::{FontSize, SharedFontSystem, Text};
  use buiy_core::{FocusedEntity, Node};
  use cosmic_text::Metrics;

  const W: u32 = 256;
  const H: u32 = 64;

  #[test]
  #[ignore = "GPU lane: needs a real wgpu adapter (CLAUDE.md GPU lane)"]
  fn preedit_underline_paints_over_the_composing_span() {
      // `gpu_render_app` ALREADY adds `BuiyTextPlugin` (support/mod.rs:60) — do
      // NOT re-add it (Bevy panics "plugin was already added"; M2, the E3 GPU
      // test's lesson, text_caret_selection_e3_gpu.rs:96-105). Add only
      // `FocusPlugin` (owns `FocusedEntity`, not in the shared stack) and the
      // `Ime` message (the harness has no winit, so it is not auto-registered).
      let mut app = support::gpu_render_app(W, H);
      app.add_plugins(buiy_core::focus::FocusPlugin);
      app.add_message::<Ime>();

      let editor = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(240.0).height_px(48.0),
              Text("ab".to_string()),
              FontSize(24.0), // tuple struct (components.rs:97) — no `::px` ctor (m1)
              TextEditState::new(Metrics::new(24.0, 28.8)),
          ))
          .id();
      app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
      app.update();

      // Compose a preedit after "ab".
      app.world_mut().write_message(Ime::Preedit {
          window: Entity::PLACEHOLDER,
          value: "ni".to_string(),
          cursor: Some((0, 2)),
      });
      app.update(); // splice
      app.update(); // reshape + project + extract

      // Render to texture + read back; assert a non-trivial number of
      // underline-colored pixels exist in the composing region (the strip).
      let target = support::render_to_image(&mut app, W, H);
      let pixels = support::readback_rgba(&mut app, target);
      // A coarse presence check: count pixels distinct from background/ink.
      // (The real golden compares against a committed reference PNG via
      // perceptual_diff once captured on the GPU host — see the E3 golden's
      // GoldenConfig idiom.)
      let _ = perceptual_diff; // referenced; the reference PNG is captured on the GPU host
      let non_empty = pixels.chunks_exact(4).filter(|p| p[3] > 0).count();
      assert!(non_empty > 0, "the composing field renders glyphs + underline");
  }
  ```

  Verify it COMPILES (it must not run headless — it has no adapter):

  ```sh
  cargo test -p buiy_core --test text_ime_preedit_gpu --no-run
  ```

  Expected: compiles; the `#[ignore]` test is not run by the headless gate.

- [ ] **Commit.** `feat(text-editing): E5 Task 7 — Escape cancels preedit + GPU
  preedit-underline golden (#[ignore])`.

---

## Task 8 — Facade boundary + the full gate

- [ ] **Facade-boundary check** — `ime.rs` names `Action`/`Attrs` (legal: it is inside
  `text::edit`). Confirm no symbol OUTSIDE `text/edit/` (and the allowed
  `components.rs` cosmic-boundary) names `Editor`/`Edit`/`Action`/`Change`. Run the
  tripwire:

  ```sh
  cargo test -p buiy_core --test text_facade_boundary
  ```

  Expected: `test result: ok`. The `Composition*` Messages, `PreeditVisual`, and the
  window systems name no forbidden type. (`Ime` is a Bevy type, not a cosmic one.)

- [ ] **The full headless gate** (CLAUDE.md § Build & Test):

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    xvfb-run -a cargo test --workspace
  ```

  Expected: green. The IME state-machine + paint + window tests pass headless (no
  adapter, synthetic `Ime` Messages). The `#[ignore]` GPU golden is NOT run here.

- [ ] **GPU lane (orchestrator, on a GPU host)** — additive, must pass before merge:

  ```sh
  cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
  ```

  Expected: the new `text_ime_preedit_gpu` golden passes alongside the existing
  render/text GPU goldens. (The reference PNG is captured on the GPU host on first
  run, reviewed, and committed — the T-series golden discipline.)

- [ ] **Docs** — flip nothing yet (the spec status flips at E6 closure). If E5
  surfaced a spec inaccuracy, record it as the E5 erratum block in the campaign plan
  (`2026-06-13-buiy-text-editing-campaign.md`) per the campaign's errata step. Update
  `docs/README.md`'s campaign line only if E6's closure has not already; otherwise no
  index change (E5 is one PR in the active campaign).

- [ ] **Commit + PR.** `feat(text-editing): E5 — IME composition (display-splice,
  four invariants, popup positioning)`. One PR for the phase.

---

## Self-review against the spec

**§ 6.1 — display-splice, not overlay.** Task 1 splices the preedit into the editor
buffer via direct `BufferLine::set_text` (it reflows + shapes); Task 2's `value()`
mid-line exclusion test (`value_excludes_preedit_midline`) proves the reflow case the
splice exists for. The metadata marker (`PREEDIT_METADATA`) is set per § 6.1's
"metadata-marked `Attrs` span". COVERED.

**§ 6.2 — the four invariants, each a normative test.**
- (a) undo unchanged during composition —
  `preedit_splice_inserts_into_buffer_without_touching_undo` (unit) +
  `composition_leaves_undo_and_value_clean` (system). The mechanism is verified:
  direct buffer-line surgery records no `Change` (cosmic `editor.rs:304,428,506-513`).
- (b) value / `TextChanged` exclude preedit — `value_excludes_preedit_midline` +
  `composition_leaves_undo_and_value_clean` + `commit_is_one_unit_and_fires_textchanged`
  (asserts 0 `TextChanged` on preedit, 1 on commit).
- (c) commit = one undo unit — `commit_is_exactly_one_composition_undo_unit` +
  `commit_does_not_coalesce_with_prior_typing` (unit) +
  `commit_is_one_unit_and_fires_textchanged` (system); the commit uses ONE
  `start_change`/`finish_change` recorded `GroupKind::Composition` via E4's
  `record_grouped`.
- (d) no orphans — `remove_preedit_restores_buffer_and_clears_span` (unit) +
  `empty_preedit_and_disabled_remove_the_span` + `escape_removes_the_preedit` (system).
  Disabled / empty-Preedit / Escape all routed. Focus-loss-while-composing is E6's
  lifecycle seal (§ 10 calls `remove_preedit`); E5 ships the primitive, E6 the caller —
  the one residual orphan window is recorded under "Known limitations" n4 (zero
  correctness impact: value + undo never see preedit).
COVERED (the four invariants in-scope for E5; the E6-owned focus-loss caller noted).

**§ 6.3 — popup positioning + the in-preedit cursor.** Task 6's `write_ime_window`
sets `ime_enabled` (focused/non-ReadOnly/non-Disabled — `focused_editor_enables_ime`,
`unfocus_disables_ime`) and `ime_position` (caret bottom-left → logical window coords
via the pointer.rs `GlobalTransform + content_offset` origin idiom, reversed). The
hardcoded bevy_winit 10×10 exclusion area is documented as the accepted v1 limit. The
in-preedit cursor (`Preedit.cursor`) is RECORDED in `PreeditSpan.cursor`; painting the
composition caret from it is the underline emitter's data — the `PreeditVisual` carries
the span; the in-preedit caret is a follow-up nicety (the primary editor caret already
sits at the preedit end). The recorded cursor is in the type so E6/follow-up can paint
it without a reshape. COVERED (positioning); the composition-caret GLYPH is data-ready,
paint deferred (noted, zero correctness risk — the editor caret is always present).

**§ 11 — `CompositionStart/Update/End` Messages.** Task 5 defines all three, registers
them, and emits on the empty→nonempty / nonempty→nonempty / commit-or-cancel
transitions; `composition_messages_emit_on_transitions` is the normative test. The
payloads match § 11 (Update carries the preedit string, End the committed string).
`TextChanged` (E2) fires only on commit. COVERED.

**Type consistency.** `PreeditSpan` (byte range + line + in-preedit cursor) is the one
state type; `PreeditVisual` (normalized `Cursor` pair) is the one paint seat, mirroring
`SelectionVisual` exactly (same constructor + `is_collapsed` idiom). `value()` and the
underline both index the SAME spliced buffer via the recorded byte range — one source
of truth. The commit reuses E4's `record_grouped` + `GroupKind::Composition` (E4 shaped
it for exactly this). `EditOutcome` gains `reshaped` so the Escape-cancel re-measure is
expressed honestly (no `TextChanged` for a non-value buffer change).

**No placeholders.** Every code step is complete and compilable: `ime.rs` full
(`PreeditSpan`, splice/remove/commit, `apply_ime`, `write_ime_window`, the three
Messages), `state.rs` field + refined `value()` + test accessor, `components.rs`
`PreeditVisual`, `extract.rs` query/damage/removal/emission edits, `color.rs` token +
resolver, `caret.rs` projection, `input.rs` Escape + `EditOutcome.reshaped`, the four
test files + the GPU golden. The only deliberately-deferred item (the in-preedit
composition-caret glyph) is data-ready and noted, not stubbed.

**Determinism + platform.** All headless tests drive synthetic `Ime` Messages — no
winit window, platform-independent (the E2 macOS Ctrl-vs-Cmd trap is avoided: IME is
not modifier-keyed). The real-IME matrix is named CI-impossible (§ 12) and NOT
automated. The GPU golden is `#[ignore]` + build-only here.

## Known limitations & deferrals (noted, not stubbed)

- **n2 — facade placement.** `ime.rs` lives inside `text::edit` because it names
  cosmic `Action` (the commit `editor.action(fs, Action::Insert(..))` lowering),
  `Edit` (via `editor.with_buffer_mut`/`set_cursor`/`cursor`), AND `Attrs` (the
  metadata span) — not `Attrs` alone. The boundary tripwire is satisfied by the file
  being inside the facade directory; nothing about IME is re-exported as a cosmic type.
- **n4 — `PreeditVisual` can briefly orphan on focus-loss-while-composing.** If focus
  leaves an editor mid-composition, E5's `apply_ime` (focus-gated) stops receiving the
  editor's `Ime` events, so the spliced span + its `PreeditVisual` are not torn down by
  E5 alone. **The campaign assigns focus-loss preedit removal to E6 § 10** (the
  lifecycle seal calls `remove_preedit` on focus loss). E5 ships the primitive
  (`remove_preedit`) ready; the focus-loss CALLER is E6. Until E6, the only path to a
  lingering preedit underline is "focus stolen mid-composition without a Disabled /
  empty-Preedit / Escape" — an edge case with zero correctness impact on the logical
  value (which always excludes preedit) and no orphan in the undo stack. In-scope-
  deferred to E6, explicitly, not dropped.
- **n3 — CI macOS/Windows lanes.** E5 adds five test binaries (`text_ime_ops`,
  `text_ime_system`, `text_preedit_paint`, `text_ime_window`, `text_ime_preedit_gpu`).
  The macOS/Windows CI lanes are uncapped (no `-j 2` / free-disk step the Ubuntu lane
  uses). The orchestrator must confirm those lanes stay green — not just the local /
  Ubuntu run — before merge; if a lane link-OOMs, cap it with `-j 2` per CLAUDE.md
  § Build & Test. (The IME tests are platform-independent at the source level — this
  is purely a link-parallelism / disk watch, not a behavioral risk.)
