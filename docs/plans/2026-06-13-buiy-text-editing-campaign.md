# Buiy Text-Editing Campaign (E1–E6)

**Date:** 2026-06-13
**Status:** active
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md)
**Readiness:** [reports/2026-06-13-text-editing-design-readiness.md](../reports/2026-06-13-text-editing-design-readiness.md)

> **For agentic workers:** this is a *campaign* plan, not a bite-sized TDD plan
> — the R-series / T-series precedent (one plan file per phase, each
> independently landable). Per-phase TDD plans come later, **one per phase**,
> written when that phase starts. Phases run as sequential Workflows; the
> orchestrator stays in the loop between them.

**Goal:** Implement the editor surface the text-rendering campaign (T1–T9)
deliberately left to a successor: the `TextEditState` machine over
`cosmic_text::Editor<'static>`, focus-gated keyboard + mouse input, the caret &
selection model, IME composition, clipboard, composition-aware undo/redo, and
the `TextInput` widget — phase by phase: substrate → input → caret/selection →
clipboard/undo → IME → lifecycle/widget/closure. Each phase lands with the
headless gate green and any GPU assertions `#[ignore]`d on the established lane.

**Gate invariant (every phase, every commit):** the headless
`cargo test --workspace` gate (NO `--ignored`; `xvfb-run -a` only where an X
server is needed) stays green — CI has no adapter. The GPU lane is **additive**
— `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` on the GPU host,
green before a phase merges (CLAUDE.md § GPU lane). New dependencies (`arboard`,
E4) require `cargo deny check` at adoption.

**Campaign shape (decided).** Single editing campaign E1–E6 at T-series
granularity, consuming T7's painting primitives and the T1–T8 rendering
substrate. **Runner-ups rejected:** one monolithic editing plan — the editor
interlocks focus, input routing, IME, and undo across entirely different seams;
the render campaign's own lesson is that exploratory multi-seam work needs the
orchestrator between phases. Folding editing into the rendering T-series — the
campaign-shape decision in
[2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) already
rejected this: editing needs focus + input routing, and bevy-cosmic-edit's
archive is the standing warning against widening the bridge surface in one bite.

**Pre-campaign decisions — all resolved in the readiness pass (2026-06-13):**

1. **Edit→layout frame-ordering (spec OQ#1) — resolved: accepted one-frame
   latency.** Editor input lands in `BuiySet::Input`, two sets after
   `BuiySet::Layout` (where `TextSync`/`TextCommit` live), so an edit publishes
   N→N+1; caret geometry / `ime_position` / auto-scroll come current the same
   frame the edit's `TextCommit` publishes. Same-frame re-entry is **rejected**
   (it needs a fourth Taffy site beyond the ≤2× cap). **E2 realizes this and
   adds the Input-driven N→N+1 latency fixture** — the T8 `text_typing_latency`
   fixture proves only the sync-side path and must not be cited as editor-path
   proof.
2. **Painting requires no new GPU work — confirmed.** Selection rects (quad-tier
   via `ExtractedTextQuads`), selected-text recolor (`GlyphAlphaInstance.color`),
   the caret (glyph-tier solid stamp), and the preedit underline (quad-tier) all
   ride T6–T8's GPU-verified paths and the T7 `CaretVisual`/`SelectionVisual`
   seats. E3/E5 produce *state + geometry* only.
3. **The editing spec is design-only.** Every editor type named below
   (`TextEditState`, `EditCommand`, `UndoStack`, `TextSelection`, the markers,
   `PreeditSpan`, IME machinery) is a build target, not existing code (spec § 1
   status banner). The architectural seams they consume ARE built and verified.
4. **arboard HTML/image read-side is unverified.** E4 ships **plain-text**
   cut/copy/paste only; HTML + image clipboard flavors are the named follow-up
   slice (spec § 13), gated on confirming arboard's HTML read support first.
5. **`TextBufferAccess` (spec OQ#4) resolved** in measure-and-layout § 2.3
   (display component + optional editor, editor-preferred), compatible with
   `BufferRef::Owned` — E1 builds on it.

**Named deferrals (within F, not dropped — spec § 13):** multi-range selection
*behavior* (the type is multi-range-shaped from E3; v1 behavior is single-range);
HTML + image clipboard flavors. **Out (E-tier):** the rich-text edit surface,
document virtualization.

**Lock-in containment (every phase).** All Buiy systems talk to the thin
`TextEditState` facade; **no system outside the `text::edit` module names an
`Editor`, `Edit`, `Action`, or `Change` type** (spec § 2.1). A reviewer check
each phase: `grep` the diff for those types outside `text/edit/`.

**Message taxonomy (spec § 11) emits incrementally, transition-only.** Each
phase emits the Messages its transitions make available — E2 `TextChanged`
(+ the internal `EditCommand::Submit`), E3 `SelectionChanged`/`CaretMoved`, E4
`EditUndone`/`EditRedone`, E5 `CompositionStart`/`Update`/`End` — and **E6
finalizes** the host-facing `EditSubmitted` and audits the taxonomy for
completeness. No phase half-builds another's Message; events emit on transition,
not per frame, so volume stays bounded.

---

## Phases

### E1 — Editor substrate + Buffer ownership

- **Deliverable:** the `TextEditState` component wrapping
  `cosmic_text::Editor<'static>` constructed with `BufferRef::Owned(Buffer)`
  (the only shape that allows mutation — `Borrowed` self-borrows, `Arc` forbids
  mutation); the policy markers `ReadOnly` / `Disabled` / `SingleLine` /
  `Placeholder(String)` (decomposed, not aggregated); the `TextBufferAccess`
  accessor (measure-and-layout § 2.3) re-exposing `with_buffer`/`with_buffer_mut`
  that **prefer the editor's owned Buffer** when `TextEditState` is present, so
  the measure seam and glyph producer shape through the authoritative buffer
  (editor-optional / buffer-required — entities with only a display `TextBuffer`
  pay nothing); the thin `text::edit` module facade. Mechanism in `buiy_core`;
  no widget, no input, no behavior yet. (spec §§ 2.1, 2.2, 2.2a, 2.3;
  measure-and-layout § 2.3.)
- **Dependencies:** the merged text-rendering substrate (T1–T9). None within
  this campaign.
- **Test surface:** headless — an entity with `TextEditState` shapes through the
  editor-owned `Buffer` (the accessor returns it, not the display component);
  a display-only entity is unaffected (accessor returns the `TextBuffer`); the
  measure call-count / `ComputedTextLayout` for an editor entity matches the
  equivalent display entity (the seam is transparent); the markers compile and
  gate; **the facade boundary holds** — a compile/grep check that no symbol
  outside `text::edit` names `Editor`/`Edit`/`Action`/`Change`. No adapter.

### E2 — Input translation + editing operations + the latency gate

- **Deliverable:** the `EditCommand` enum (spec § 3 shape — `Motion(Motion,
  extend)`, `Insert`, `Backspace`, `Delete`, `Enter`, `Cut/Copy/Paste`,
  `Undo/Redo`, `SelectAll`, `Escape`, `Submit`); the **data-driven per-platform
  keymap table** keyed on `(modifiers, logical Key)` → `EditCommand`, selected
  at startup (Ctrl on Linux/Windows, Cmd/Option on macOS — a data swap, not
  scattered `cfg`); the `KeyboardInput` Message → `EditCommand` → cosmic
  `Action` lowering system in `BuiySet::Input`, **focus-gated** on
  `FocusedEntity` pointing at a non-`Disabled` `TextEditState`; character
  insertion via the event's layout-resolved `text` field iterated as chars;
  `repeat` honored for motions/deletes; grapheme-correct `Backspace`/`Delete`
  (inherited from `Action`); the `SingleLine` policy (Enter ⇒
  `EditCommand::Submit` — internal; the host-facing `EditSubmitted` Message is
  finalized in E6; never `Action::Enter`; `Wrap::None`; newline-stripped paste).
  The logical-value read (`value()` over the editor buffer — pre-IME this is the
  full buffer text; E5 refines it to exclude the preedit range, invariant (b))
  and the `TextChanged` Message on logical-value change are born here, since
  edits originate in this phase. The OQ#1 one-frame path is realized here (no new
  Taffy **compute pass** — scheduling is unchanged). It does, however, need the
  editor edit to **dirty-mark** its node into the existing measure pipeline: an
  `Action` into the editor-owned buffer trips none of the `TextSyncTriggers`
  (the `Text` component is unchanged), so `apply_keyboard_edits` must
  `invalidate_intrinsics()` + `mark_dirty_for_entity()` — exactly `sync_one`'s
  gesture for a `Text` edit — to enter measure → commit → extract. That seam is
  the editor-input latency gate's load-bearing fact (the E2 plan's M1); the
  Input-driven N→N+1 fixture is its regression guard. (spec §§ 3, 3.1, 3.2, 3.3,
  11.)
- **Dependencies:** E1.
- **Test surface:** headless — keymap table tests per platform (key + modifiers
  → `EditCommand`); character insertion from synthetic `KeyboardInput.text`;
  grapheme-correct delete fixtures (emoji ZWJ, combining marks); `SingleLine`
  policy (Enter ⇒ `EditCommand::Submit`, newline-stripped paste); `repeat`
  semantics; `TextChanged` emits once per logical-value change and `value()`
  reflects the edit; **the new Input-driven N→N+1 latency fixture** (edit applied
  in `BuiySet::Input`,
  glyph publish asserted at frame N+1 — the OQ#1 gate, distinct from T8's
  sync-path fixture). No adapter (synthetic Messages need no winit window).

### E3 — Caret + selection model + painting

- **Deliverable:** the `TextSelection` type (multi-range-**shaped**:
  `primary: SelectionRange` + `secondary: SmallVec<[…; 2]>`, v1 single-range
  *behavior* — `secondary` always empty), mirroring its primary into
  `Edit::set_selection` so `Action`-driven drag-extend / `delete_selection`
  keep working; the caret model (logical `Cursor` position, **visual** motion
  inherited from `Editor` per UAX #9 — the keymap never computes BiDi); caret
  geometry via `LayoutRun::cursor_position` + line metrics, the **BiDi split
  caret** = a secondary `CaretVisual` rect + a second stamp (CPU geometry only);
  selection geometry via `Edit::selection_bounds` + `LayoutRun::highlight`
  (mixed-direction multi-rects automatic — zero Buiy-side BiDi math); mouse
  `Click`/`DoubleClick`/`TripleClick`/`Drag` through picking coordinates with
  `Word`/`Line` granularity; painting wired to the **T7 seats** —
  selection rects at quad seat via `ExtractedTextQuads`, per-cluster recolor /
  `::selection` foreground via `GlyphAlphaInstance.color` (atlas untouched), the
  caret stamp emitted after run glyphs; **the per-entity blink phase activated**
  — E1's `CaretBlink` field (built but inert) gains its edit / caret-move
  phase-reset here, and T7's deliberately global+stateless `write_caret_blink`
  (`text/visual.rs:61-75`, whose module doc defers the per-entity reset to "the
  editing campaign's `CaretBlink` state") is reworked to be **phase-relative to
  that per-entity origin** (reduced-motion steady path preserved) — new behavior,
  not a reset of the existing writer; the `SelectionChanged` / `CaretMoved`
  Messages. (spec §§ 4.1, 4.2, 4.3, 5, 11; `text/visual.rs`.)
- **Dependencies:** E2.
- **Test surface:** headless — selection geometry rect counts on mixed-direction
  fixtures (`"hello עולם world"` → correct disjoint rects) + split-caret presence
  on a direction boundary; mouse hit-test → `Word`/`Line` granularity; caret
  visual motion across a BiDi run (logical index vs visual step); `CaretMoved` /
  `SelectionChanged` emit on transition only. **GPU lane (`#[ignore]`,
  additive):** a caret + selection golden on a mixed-direction fixture on the
  existing readback harness (the T7 `text_selection_caret_gpu` precedent).

### E4 — Clipboard + undo/redo with composition-aware grouping

- **Deliverable:** the clipboard facade — `arboard` 3.6.x as a direct dep
  (`cargo deny check` at adoption) behind a `ClipboardProvider` Resource
  trait-object so tests inject a fake and the dep stays swappable; `Cut`/`Copy`
  (`copy_selection`) / `Paste` (through the § 3.3 newline policy), **plain text
  only** (HTML/image deferred — decision 4). The undo engine — a Buiy-owned
  two-stack model over the verified `Change` substrate (`Change::reverse` +
  `Edit::apply_change`); `UndoUnit { change, caret_before/after,
  selection_before/after, group: GroupKind }`; grouping rules (consecutive
  typing coalesces by time window + caret adjacency into a `TypingRun`,
  same-direction deletes into a `DeleteRun`, any motion/click/discrete command
  seals the open group); undo restores the `_before` pair, redo the `_after`;
  redo clears on any new edit; depth-bounded (config, default 1000). The
  `Undo`/`Redo` `EditCommand`s; `EditUndone`/`EditRedone` Messages. Undo lives
  in core, on by default (the bevy-cosmic-edit "cannot be optional" warning).
  (spec §§ 7, 8, 11.)
- **Dependencies:** E2 (the `Cut`/`Copy`/`Paste`/`Undo`/`Redo` commands route
  through the keymap) and E3's `TextSelection` **type** (the `UndoUnit` stores
  `selection_before/after`). Independent of E3's painting/mouse *behavior*, but
  lands after it to keep the sequence linear.
- **Test surface:** headless — clipboard via the fake `ClipboardProvider`
  (round-trip cut/copy/paste, newline-strip on single-line paste); **the undo
  property test** (for arbitrary edit scripts, `apply_change(reverse(c))` after
  `c` is identity on the buffer text, and undo-all restores the initial value +
  caret); grouping fixtures (a typing run undoes as one unit; a motion seals the
  group; a delete run coalesces; redo clears on new edit); depth bound. `cargo
  deny check` green. No adapter.

### E5 — IME composition

- **Deliverable:** the **display-splice** preedit model (spec § 6.1, superseding
  the prior-art overlay) — on `Ime::Preedit`, splice the preedit string into the
  editor's display `Buffer` at the caret as a metadata-marked `Attrs` span; each
  subsequent `Preedit` replaces the span; empty `Preedit` / `Ime::Disabled` /
  focus loss / `Escape` removes it. **The four invariants (§ 6.2):** (a) undo
  never sees preedit (no `start_change` wraps a splice); (b) value reads /
  `TextChanged` exclude the preedit byte range — **E2's `value()` accessor is
  refined here** to subtract the live preedit span (E6's placeholder check then
  consumes the refined accessor); (c) `Ime::Commit` = one undo unit
  (delete span + insert committed text inside a single change pair, using E4's
  stack); (d) no orphans. The preedit underline (quad-tier, decoration-and-paint
  § 8 seat) + the in-preedit cursor from `Preedit.cursor`. Popup positioning via
  `Window.ime_enabled` (true while a focused, non-`ReadOnly`, non-`Disabled`
  editor exists) + `Window.ime_position` (caret rect bottom-left in logical
  window coords, written on every caret move / preedit update) — with the known
  bevy_winit 10×10 exclusion-area limitation accepted for v1. The
  `CompositionStart`/`Update`/`End` Messages. (spec §§ 6.1, 6.2, 6.3.)
- **Dependencies:** E3 (caret geometry for `ime_position`), E4 (the undo stack
  for invariant (c)).
- **Test surface:** headless — the IME state-machine table over
  `Preedit`/`Commit`/`Enabled`/`Disabled` sequences asserting the § 6.2
  invariants (undo stack empty during composition; value excludes preedit;
  commit = exactly one `UndoUnit`; no orphan span after Disabled / focus loss /
  Escape); preedit reflow (composing mid-line shifts following text — the reason
  splice beats overlay); `ime_position` math (buffer-local → window space).
  **GPU lane (`#[ignore]`, additive):** the caret + selection + preedit-underline
  golden on a mixed-direction fixture (spec § 12). The real-IME manual matrix
  (CJK + dead keys per platform) is named CI-impossible (§ 12).

### E6 — Focus/lifecycle + placeholder + auto-scroll + widget + closure

- **Deliverable:** focus & lifecycle (spec § 10) — all editing systems gate on
  `FocusedEntity` → a non-`Disabled` `TextEditState`; on focus gain: caret
  visible, blink reset, `ime_enabled` true (unless `ReadOnly`); on focus loss:
  open undo group seals, preedit removed, `ime_enabled` false, caret hides,
  **selection retained** (web parity). `Placeholder` rendering (§ 10) — when the
  logical value is empty (preedit excluded) the string renders as a display-only
  Buffer via the `::placeholder` token, never entering the editor Buffer.
  Auto-scroll-into-view (§ 9) — pan via the layout `ScrollOffset` (x single-line
  / y multi-line; the Buffer never scrolls internally — `ScrollOffset` does not
  invalidate Taffy), clamping the caret rect into the clip viewport after each
  move/edit; `PageUp`/`PageDown`/`Action::Scroll` lower to `ScrollOffset` deltas.
  The **§ 11 Message taxonomy finalized** — the host-facing `EditSubmitted`
  added and the full taxonomy audited for completeness (`TextChanged` born in
  E2, `SelectionChanged`/`CaretMoved` in E3, `EditUndone`/`EditRedone` in E4,
  `Composition*` in E5). The
  **`buiy_widgets::TextInput::new(...) -> impl Bundle`** (the `Button::new`
  precedent) composing core components with catalog policy (sizes, tokens,
  submit-on-Enter, focus-on-click — widget policy, never core auto-focus).
  **Campaign closure:** the both-lane gate, the spec § 13 v1 checklist walked
  bullet-by-bullet, the spec README status flip (`editing-and-ime.md` →
  implemented), docs/README + follow-ups updates, and the close-out. (spec §§ 9,
  10, 11, 2.3, 13.)
- **Dependencies:** E3 (caret geometry for auto-scroll), E5 (lifecycle seals the
  preedit). E4 (lifecycle seals the undo group).
- **Test surface:** headless — focus gain/loss transitions (caret visibility,
  `ime_enabled`, undo-group seal, selection retention); placeholder shows iff
  value empty (preedit-excluded) and vanishes on first real/preedit char;
  auto-scroll clamp math (caret stays in viewport with margin; large content
  shapes fully — no virtualization at F); the `TextInput` bundle composes and
  focuses-on-click; the § 11 taxonomy emits on transition. **GPU lane
  (`#[ignore]`, additive):** a `TextInput` end-to-end golden (focused, with
  placeholder vs typed value). **Closure:** both lanes green; the
  `text::edit` facade-boundary grep clean across the whole campaign diff.

---

## Execution loop (per phase)

The established T-series loop, unchanged:

1. **Plan** — a fresh agent writes the bite-sized TDD plan for the phase from
   this campaign plan + the spec sections it names
   (`docs/plans/2026-06-13-buiy-text-editing-eN-<name>.md`).
2. **Review the plan** — fresh skeptical review; resolve majors before coding
   (the plan catches the real design gaps — every T-phase plan found one).
3. **Execute** — the phase-driver Workflow (fresh-agent TDD implement → skeptical
   correctness+spec review → fix majors, per task, no commits).
4. **Gate (orchestrator)** — the full headless gate + the additive GPU lane,
   run by the orchestrator, not the agents.
5. **Commit → push → PR → CI → rebase-merge.** Each phase is one PR.
6. **Errata** — mechanical spec inaccuracies found while implementing are
   recorded as the phase's errata block here and folded into the spec at closure
   (the T-series discipline).

**Campaign exit (after E6):** both lanes green, the spec `editing-and-ime.md`
status flipped to implemented, the F-tier editor surface from
[foundation text.md § 3.5](../specs/2026-05-07-buiy-foundation/text.md) delivered,
the named deferrals (multi-range behavior, HTML/image clipboard) filed as the
next slice.
