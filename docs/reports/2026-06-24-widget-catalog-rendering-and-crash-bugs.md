# Widget-catalog: the live-run crash + the invisible-content rendering bug

**Date:** 2026-06-24
**Trigger:** running the `buiy_gallery` binary (`cargo run -p buiy_gallery`) for the
first time — it **panicked**, and once the panic was fixed, the gallery rendered
with **no visible content text** (empty checkboxes, label-less buttons, blank
rows). Both had passed the entire headless gate (1653 green tests). This report
records the two root causes, the fixes, and — the point the trigger made — *why
the testing never caught either*.

---

## Bug 1 — the crash: an editor buffer reaches extract UNSHAPED

### Symptom
`crates/buiy_core/src/text/extract.rs` — `debug_assert_eq!(runs,
computed.lines.len(), "TextBuffer dirty-unshaped at extract (mutated after
TextCommit?)")` fired in the live windowed run (and on every keystroke into the
"What needs to be done?" field).

### Root cause
Editor edits (`apply_keyboard_edits`, `apply_ime`, undo/redo, the preedit splice)
run in `BuiySet::Input`, which is **two sets after `BuiySet::Layout`** — i.e.
**after** `TextCommit` (the last layout step). `apply_change` un-shapes the
editor-owned buffer and **defers** the reshape to *next* frame's `TextCommit`
(`EditOutcome::reshaped == false`, the documented OQ#1 "one-frame latency"). So an
edit frame *ends* with the editor buffer unshaped. The render extract reads
**every** text entity on **any** damage frame (extract.rs § 6.2 dirty-gate), so a
keystroke coinciding with *any* other damage — the first character's empty↔
non-empty `PlaceholderActive` toggle (which is *in* extract's `Changed` union),
a sibling row, a theme tick — reads the transiently-unshaped editor and trips the
invariant → live crash. The old extract dirty-gate "mitigation" is all-or-nothing
and cannot protect a single transiently-unshaped entity; skipping its paint would
blank-flicker the editor on every keystroke (a damage frame rebuilds the whole
glyph buffer).

### Why no test caught it
The headless gate runs `MinimalPlugins` + synthetic single-shot `app.update()`s.
It **never instantiates the render world**, so `extract_buiy_glyphs` — and its
coherence assert — **never ran in CI**. The editor tests all used `set_value`
(which shapes immediately, holding the font lock), never the deferred real
`apply_keyboard_edits` path *combined with a concurrent damage source*.

### Fix
`commit::reshape_edited_editors` (`crates/buiy_core/src/text/commit.rs`), scheduled
`.after(BuiySet::Input).before(write_caret_and_selection)`: it reshapes any editor
buffer an edit left unshaped (at the box the last commit set — content changed,
not geometry) and rewrites `ComputedTextLayout`/`ResolvedBaseline` to match. The
buffer is coherent before extract AND the caret reads the fresh shape, so caret +
glyphs now come current the SAME frame. Lock-free guard (the extract invariant
itself: `layout_runs().count() != lines.len()`) → a steady frame does no work.

This removes the one-frame **glyph** latency (an *accepted cost*, OQ#1, not a
requirement); only the **box layout** still re-measures next frame. Coherence and
the old latency are mutually exclusive (extract rebuilds all entities on any
damage), so same-frame shaping is the correct resolution. `text_input_latency`'s
contract test was updated accordingly.

### Gap closed
`commit::debug_assert_shape_coherence` (debug-only, scheduled in `Last`) — the
main-world mirror of the extract assert. It runs in **every** headless test that
pumps frames, so the dirty-at-frame-end class is now caught by the headless gate
without a render world. It immediately red-flagged 26 editor tests, pinpointing
the bug.

---

## Bug 2 — invisible content: `Text` did not `#[require(Node)]`

### Symptom
With the crash fixed, the gallery rendered the input *placeholder* but **nothing
else**: empty checkbox boxes, label-less buttons, blank todo rows, a zero-width
"N items left". Only the placeholder (which rides the input entity's `Node`)
painted.

### Root cause
`Text` was a plain component with **no `#[require(Node)]`**, and the widget
scene-fns + the gallery authored labels as **bare `(Text(…) FontSize(…))`
children without a `Node`**. The whole pipeline (TextSync → measure → TextCommit
→ extract / the `painters_z` paint order) only acts on **layout nodes**. A `Text`
on a non-`Node` entity is silently never measured, never shaped, never in the
paint order — it has an accessible name yet paints nothing. The headless layout
snapshots had **baked in** the absence (`RowCheckbox size=18,18` with no label
child, `ItemsLeft size=0,48`), so they stayed green.

A second, related shape: `button()`/`Button::new()`/`menu_button()`/
`dialog_invoker` set `A11yLabel` (the accessible name) but had **no visible
`Text` child at all** → label-less boxes.

### Fix
1. **`#[require(Node)]` on `Text`** (`text/components.rs`) — text is content in a
   box; requiring `Node` makes "this text participates in layout & paint"
   structural, so a scene-authored bare label is renderable by construction.
2. **Labelled controls became flex-rows** (`checkbox`/`switch`/`slider`): the
   visible control box (the 18×18 mark / the switch pill / the slider rail) moved
   onto a child marker (`CheckboxMark`/`SwitchTrack`/`SliderTrack`) and the label
   sits **beside** it as a pick-through sibling — instead of being trapped inside
   the fixed-size box where it wrapped one glyph per line. Click anywhere on the
   row toggles (pick-through preserved). The checkbox mark now toggles its **glyph
   text** (`✓`/`–`/empty) rather than `CssVisibility`, so the box always shows.
3. **Buttons render a centered label child** (`button`/`Button::new`/`menu_button`/
   `dialog_invoker`) and are **content-width** (a fixed 120px box oversized "All"
   and overflowed dense footers).
4. Gallery: the completed-row label used a **missing theme token**
   (`color.text.disabled` → magenta sentinel) → switched to `color.text.secondary`;
   the TodoMVC card widened so the footer fits on one line; the modal's bare
   buttons → `Button::new`.

### Gaps closed
- `bare_text_requires_node_and_lays_out_and_shapes` (`tests/text/text_commit.rs`) —
  a bare `Text` must materialize a `Node`, get a `ResolvedLayout` at non-zero
  size, and a `ComputedTextLayout` with real glyph geometry. RED before the fix.
- `todomvc_content_text_is_laid_out_and_shaped_so_it_paints`
  (`examples/buiy_gallery/tests/todomvc_layout.rs`) — every non-empty content
  label in the *live* TodoMVC tree (rows + filter/×/Clear buttons + status) is
  laid out at non-zero size AND shaped to glyphs — the paint precondition the
  layout snapshot only implies. Gallery-grounded ("the example IS the fixture").

### The deeper gap
The widget-catalog campaign was verified **headlessly only** (layout snapshots,
a11y tree, the in-process driver). Those gates are blind to "does text actually
paint": the layout snapshots *encoded* the missing labels as the expected state,
and no GPU/visual check ever ran on the gallery content. That is the honest answer
to "why didn't testing catch this".

---

## Verification (this change)
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -D warnings`,
  `RUSTDOCFLAGS=-D warnings cargo doc` — all clean.
- Headless gate: **1656/1656** (`cargo nextest run --workspace --locked`), incl.
  the new coherence + content-render guards.
- GPU lane (RX 6700 XT): the 2 `text_caret_selection_e3_gpu` tests fail **on the
  RX 6700 XT** — but that is RX flakiness (the `is_white_ink`/perceptual checks are
  calibrated for the pinned lavapipe and the RX result is non-deterministic). I
  initially **mis-diagnosed** these as a "pre-existing calibration" wash. They were
  a **real regression** — see *CI cross-platform* below.
- Live: all 5 gallery screens render their content text; typing into the input no
  longer crashes (verified via synthetic XTEST input + screenshots).

## CI cross-platform — a real lavapipe regression the local gates missed
Opening PR #80 ran the **pinned-lavapipe GPU lane** on the campaign for the first
time (the campaign branch had never been CI'd). It FAILED on
`e3_editor_driven_caret_paints_a_bar_right_of_the_ink`: the editor renders **no
glyph ink** (`maxwhite=0`), while origin/main passes the same test on the same
lavapipe. A genuine regression — invisible to both the headless gate (the test is
`#[ignore]`, GPU-only) and my local RX 6700 XT (flaky for this test).

Root-caused by **reconstructing CI's exact pinned lavapipe locally, sudo-less**
(gfx-rs Mesa 24.3.4 tarball + Arch's LLVM 18.1.8 + `VK_ICD_FILENAMES`), then
`git bisect`ing on a **CPU-independent** signal (ink PRESENCE, not the
calibration-bound exact pixels) to **C2 `008efa6` (editor text-integrity)**. C2's
anti-clobber `TextSync` stopped `set_text`-ing editor-owned buffers (to stop a
FontsGeneration sweep clobbering typed content) — which also **killed the initial
`Text`→buffer SEED**. C2 moved the *headless* editor tests onto the `EditCommand`
seed channel but missed these two `#[ignore]` GPU goldens. Production is
unaffected (`TextInput` seeds via `set_value`; gallery editors use placeholders).

**Fix** (`250d39d`): seed both e3 `capture` helpers via `EditCommand::Insert` (the
production seed path). **CI re-run: fully green** — all 9 jobs (GPU pinned
lavapipe + Windows/macOS/Ubuntu + MSRV + lint/doc/deny).

**Lesson:** a GPU lane that only runs on CI is unverified until the branch is
actually pushed to a PR; a long-lived campaign integration branch should be CI'd
incrementally, not first at merge time.

## Post-landing adversarial bug-hunt (4 read-only review rounds → converged)
After the fixes above, a multi-round adversarial bug-hunt ran (each round: a
fan-out of read-only reviewers over the full diff by risk dimension → adversarial
per-finding verification → confirmed-bug synthesis). It surfaced **6 further real
bugs the headless gate missed**, each fixed with a regression guard:

1. **(HIGH)** `reshape_edited_editors` had no ordering vs `focus_lifecycle` — a
   *second* post-`TextCommit` editor-buffer mutator (it `remove_preedit`s on
   focus-loss-with-IME, un-shaping the buffer). The scheduler could run the
   reshape *before* it → the same unshaped-at-extract crash. Fixed by
   `.after(focus_lifecycle)` + the `focus_loss_with_live_preedit_…` test (RED
   without the edge — verified by forcing the order).
2. **(MEDIUM)** `menu_button` double-label — the scene-fn injected a label *and*
   the gallery authored a manual `Text("Edit")` → two overlapping labels at
   negative x. Fixed (removed the manual one; dropped the distorting flex-center).
3. **(MEDIUM)** `tooltip_trigger` had no visible label (the "?" rendered as an
   empty box) — the missing-label class. Fixed (added a pick-through glyph child).
4. **(MEDIUM)** Slider thumb hung 6px below the rail centre (the thin-track nesting
   lacked cross-axis centering). Fixed (`SliderTrack` flex `align_items: Center`) +
   a `thumb_is_vertically_centered_on_the_rail` test.
5. **(LOW)** Switch thumb 2px high (the slider's centering fix was not mirrored to
   the parallel `SwitchTrack`). Fixed + a switch y-center test.
6. **(MEDIUM)** `Disclosure` header stacked vertically (it never got the flex-row
   treatment). Fixed: flex-row `[caret, label]` header + `Position::Relative` root
   + the controlled panel `Position::Absolute` (below, out of flow), keeping caret
   + panel as direct children so the visual/wiring systems are unchanged.

The 4th round and a final fresh-context holistic agent found **zero functional
bugs** — only stale doc comments from this change (a follow-up that said the
disclosure was un-restructured; checkbox comments referencing the removed
`CssVisibility::Hidden` mechanism), now corrected — the convergence signal.

## Verification (final)
- fmt + clippy (`-D warnings`) + rustdoc (`-D warnings`) — all clean.
- Headless gate: **1660/1660** (`cargo nextest run --workspace --locked`), incl.
  every new regression guard (the `Last` coherence invariant, the bare-`Text`
  lays-out invariant, the gallery content-paints test, the same-frame-publish +
  focus-loss-preedit + slider/switch y-center tests).
- **Coverage round** (the loop's "full coverage" pass): a proptest
  (`text_coherence_property`) fuzzes arbitrary scripts of editor edits + focus
  changes + IME preedits and asserts the buffer is NEVER unshaped at frame end —
  exhaustively exercising `reshape_edited_editors` + its `focus_lifecycle`
  ordering. RED-verified: disabling the reshape system makes the proptest fail at
  the `Last` invariant (`commit.rs:254`). It models the real IME invariant (a
  composition owns the keyboard) rather than fuzzing unreachable states — see the
  pre-existing `value()` edge in *Remaining*.
- GPU lane (RX 6700 XT): no new failures (the only 2,
  `text_caret_selection_e3_gpu`, are the pre-existing lavapipe-calibrated
  `is_white_ink` failures — identical on the pre-change base, green on CI lavapipe).
- Live: all 5 gallery screens render their content + are visually clean; typing
  into the input no longer crashes (synthetic XTEST + screenshots).

## Remaining (minor, follow-ups in `docs/plans/follow-ups.md`)
- The `✓` (U+2713) checkbox mark and `▸` disclosure caret render as **tofu** —
  the embedded font is the **latin subset** (FiraSans-latin), which lacks them.
  Needs a fuller embedded glyph or opt-in system fonts. (The destroy `×`/U+00D7 is
  in-subset and renders fine.)
- S3 overlay: closed/anchored overlays overlap in the single captured frame — the
  pre-existing popover single-frame-positioning fragility (a `painters_z` ordering
  that resolves over multiple live frames).
- The `button.resting.*` CPU coverage fixture now baselines a content-width-empty
  button (the verify-headless harness has no font system, so its `Text` measures
  0×0); give that fixture an explicit width/label so it stays a meaningful sample.
- **(Pre-existing, surfaced by the new coherence proptest — NOT a regression.)**
  `TextEditState::value()` slices the buffer text by the live preedit byte range
  without bounds-checking, so a *stale* preedit span (buffer shorter than
  `span.end()`) panics. Unreachable under the standard IME model (the platform IME
  owns the keyboard during composition, so a raw keyboard edit never runs against a
  live preedit — `apply_keyboard_edits` only removes the span on `Escape`/commit/
  focus-loss). The property test models that invariant rather than fuzzing the
  impossible state. If a platform can ever deliver a raw edit mid-composition, make
  `value()` total (clamp via `text.get(..)`) or have `apply_keyboard_edits`
  cancel/commit the composition before a raw edit. Deferred (design call, adjacent
  to this change's scope).
