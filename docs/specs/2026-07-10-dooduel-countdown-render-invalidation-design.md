# Countdown-number render invalidation — a geometry-stable text change never re-extracts its glyphs

**Date:** 2026-07-10 · **Status:** design → fix · **Layer:** framework (`buiy_core` text render)
· **Surfaced by:** Dooduel QA cycle-1 finding F1 (the in-game countdown number freezing).

## The bug (verdict: real framework render bug, NOT a harness artifact)

The in-game countdown **number** (`view/in_game.rs` `timer_view` → `text!("{}", secs)`) visibly
freezes at a phase-start value for a whole phase while the progress **ring**
(`icon::<Msg>(ring_path(frac), …)`) beside it keeps animating, then the number "teleports" to
the correct value the instant an unrelated model change lands (a guess, a phase change).

**Adjudication — reproduced on the LIVE windowed app with ZERO input.** Running
`cargo run -p dooduel` on a real display (AMD RX 6700 XT), driven to the in-game phases and then
left untouched, the header timer captured once per second read:
`4, 3, 2, 0` (end of a drawing phase, ticking) then **`9, 9, 9, 9, 9, 9, 9, 9, 9`** (a whole
picking phase, frozen) while the ring arc drained a full circle, then `80` (next drawing phase).
The app runs in Bevy's **continuous** update mode (~80% CPU while idle — verified), so the clock
ticks every frame regardless of input; the ring animating proves the model's `secs` IS
advancing. The number and the ring both derive from the **same** `Countdown::secs`
(`fraction() = secs/total`), so they cannot diverge in the model — the divergence is in the
**render path**. This is not a `qa_seat` offscreen-readback staleness artifact: the live window
shows it too.

Why it ticks during the *end of drawing* but freezes during *picking*: the drawing-end frames
carry bot-guess chat activity (new chat rows = a structural glyph-damage escalation to a **Full**
re-extract), which re-emits every text entity including the number as a side effect. Picking has
no chat activity, so nothing forces a Full re-extract and the number's own per-second change is
invisible to the extract gate (below). This is exactly the reported "teleports the instant a
guess lands."

## Root cause (`crates/buiy_core/src/text/extract.rs`, `components.rs`)

`extract_buiy_glyphs` re-extracts a text entity's glyphs only when its dirty-gate `Or<(…)>` union
fires (`extract.rs:583–636`). That union **deliberately excludes `Changed<TextBuffer>`** and
relies on `Changed<ComputedTextLayout>` as *"the text-changed signal"* (`extract.rs:580–582`).

But `ComputedTextLayout` (`components.rs:658–670`) carries **only layout geometry** — per-line
metrics (`line_y/top/height/width/rtl`), `size`, `content_offset` — and **no glyph identity**.
Its write is value-compare-guarded (`components.rs:650–657`: "bump the change tick only when the
value actually changed… the `PartialEq` derive IS that guard").

So when a `Text`'s content changes to a new string that lays out to the **same geometry**, the
idempotent `ComputedTextLayout` write does **not** tick `Changed`, the extract skips the entity,
and the **stale glyphs stay on screen**. The countdown is the pathological trigger: its display
font is **Caveat** (`theme.rs:178`) at 32px, and consecutive countdown values (esp. equal digit
counts) shape to a byte-identical `ComputedTextLayout` — so the digit swap is invisible to the
gate. The measure/shape chain *does* run (`sync.rs:70` re-shapes on `Changed<Text>`; the new
glyphs are in the buffer) — only the extract's re-emission is gated out.

The ring is unaffected because it is an `icon()` (an `ImageNode` on the separate **icon-producer**
extract path), which re-rasters/re-extracts whenever its content-addressed path changes each
second — hence "ring current, number stale, in the same frame."

**This exact bug class is already documented — and fixed — for the placeholder buffer** in the
same union (`extract.rs:621–627`): editing a text-input's string re-shapes the `PlaceholderBuffer`
"with… NO `ComputedTextLayout` tick (the empty editor value is unchanged), so without these the
screen keeps the stale placeholder glyphs" — fixed by adding `Changed<Placeholder>`/`FontSize`
terms. The ordinary display-`Text` content path was never given the analogous term.

## Approaches

**A — add `Changed<Text>` to the extract trigger union (CHOSEN).**
`Text` is the source-of-truth content component on every display text node (co-resident with
`TextBuffer`, which the query already filters on). The view reconciler's `set_text`
(`buiy_view/reconcile.rs:1364`) mutates it **iff the string differs**, so `Changed<Text>` fires
**exactly** on a content change and is O(0) on steady frames — it preserves the extract's
steady-state-does-nothing contract, strictly better than the rejected `Changed<TextBuffer>`
(which the file notes ticks on measure/commit bypass writes). This mirrors the placeholder
precedent one-for-one: the term joins the nested `Or` group that already exists for
"content changed but `ComputedTextLayout` is idempotent" (respecting Bevy's 15-element
filter-tuple cap). Entities without `Text` (editor buffers) simply don't match that term and
still re-extract via their existing triggers.

**B — put glyph identity into `ComputedTextLayout` (rejected).** Add a content hash / the shaped
glyph run so `PartialEq` catches content changes. Rejected: it bloats a *layout* component with a
*render* concern, changes its semantics for every consumer (caret math, picking, a11y bounds), and
pays a hash cost on the hot commit path — wrong layer for a one-term gate fix.

**C — re-add `Changed<TextBuffer>` (rejected).** The file already documents why it was removed:
measure/commit writes bypass its ticks and it is noisy, defeating the O(0) steady state.

## Fix + test

- **Fix:** add `Changed<Text>` to the `extract_buiy_glyphs` dirty-gate union, nested into the
  existing content-changed `Or` group (extract.rs ~588–633).
- **Test (lowest tier that observes it — headless, no GPU):** in the adapterless
  `TextExtractHarness` (`tests/support/extract_harness.rs`), spawn a display `Text`, settle,
  capture the extracted glyphs, then mutate the `Text` content to a **geometry-stable** value
  (assert `ComputedTextLayout` is unchanged between the two states — proving the gate that used to
  miss), settle again, and assert the extracted glyphs now reflect the **new** content (a
  `changed_frames` bump + a differing glyph set). RED before the fix (stale glyphs), GREEN after.
  Monospace-shaped or width-verified content keeps the change geometry-stable independent of the
  test font.

## Related cycle-1 findings (verdicts only — separate roots, not fixed here)

- **F1b (canvas lingers between turns):** REAL app behavior, not a harness/readback artifact. The
  game canvas clears on **Drawing-phase entry** (`paint.rs:541` `if drawing && !was_drawing`),
  not on the turn/Picking transition, so the previous drawing persists (mostly under the waiting
  scrim) through the next drawer's Picking. Different root from F1. Small app-reducer change if we
  want it cleared at turn boundary; out of this task's scope.
- **F4 (dashed room-code box):** INTENDED "sketchy" aesthetic — design `2.5px dashed var(--ink)`
  (`join.rs:15,27`, `lobby.rs:31,45`). Minor real nit: the inner `text_input` (`.fill_width()`,
  `join.rs:23`) renders narrower than its dashed wrapper, so the field looks offset inside the
  frame. S5, verdict-only.
