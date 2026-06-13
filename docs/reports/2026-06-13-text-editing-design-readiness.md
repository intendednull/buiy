# Text-editing design-readiness review — `editing-and-ime.md`

**Date:** 2026-06-13
**Verdict:** ready-with-patches — no blockers; apply the spec patches below, then plan.

This report synthesizes three independent verifiers (A: spec-vs-code audit; B:
frame-ordering open question OQ#1; C: no-new-GPU-work painting claim) that
examined
[`docs/specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md`](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md)
against the current `main` codebase.

---

## Bottom line

The editing-and-ime spec is **design-only** and architecturally sound. Every
integration point it leans on (focus + Tab, `ScrollOffset` non-invalidation,
`BuiySet` ordering, paint rank quad<glyph, the independent glyph damage gate,
straight-alpha per-instance tint, the `Button::new` bundle precedent) is
positively verified in code. Every editing-tier type it describes
(`TextEditState`, `EditCommand`, `UndoStack`, `TextSelection`, the
`ReadOnly/Disabled/SingleLine/Placeholder` markers, `PreeditSpan` and the IME
machinery) does **not** exist yet — correctly, because they are this campaign's
implementation targets, deferred from T6–T8. The painting claim holds: selection
rects, recolor, caret, and preedit underline all ride GPU-verified quad/glyph
paths with **no new GPU work**.

OQ#1 (frame-ordering) is **resolved**, not blocked: accepted **one-frame
latency** for the editor input path. There are **no disagreements between
verifiers** requiring human adjudication — they corroborate. There are **no
blockers**. Four spec patches (three text edits in two files) should land before
the implementation plan is written so the plan doesn't inherit a stale open
question or an over-cited fixture claim.

---

## Blockers

**None.** Nothing prevents writing the implementation plan. The single `major`
finding in verifier A ("these types don't exist") is *expected* — they are the
deferred build targets, not a verification failure. It is handled as a spec patch
(Patch 1), not a blocker.

---

## Spec patches to apply before planning

Apply these verbatim. They do not change code; they correct the spec's
self-description and close OQ#1. **The orchestrator applies these — do not let the
plan-writer rediscover them.**

### Patch 1 — mark the type/IME sections design-only (verifier A `major`)

`editing-and-ime.md` describes `TextEditState`, `EditCommand`, `UndoStack`,
`TextSelection`, the policy markers, `PreeditSpan`, and IME logic as if built;
none exist. Add a prominent status banner so readers (and the plan-writer) don't
mistake §§ 2.2/3/4/6/8 for verified code.

**File:** `docs/specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md`

**Old text** (lines 39–40, end of the § 1 intro, immediately before the closing
fence of the opening section):

```
existing GPU-verified paths — quads via the batched node, glyph recolor via
`GlyphAlphaInstance.color` ([render § 4.1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md),
`crates/buiy_core/src/render/atlas/primitive.rs:30-48`). No new GPU work is
required by anything in this file (§ 5).
```

**New text:**

```
existing GPU-verified paths — quads via the batched node, glyph recolor via
`GlyphAlphaInstance.color` ([render § 4.1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md),
`crates/buiy_core/src/render/atlas/primitive.rs:30-48`). No new GPU work is
required by anything in this file (§ 5).

> **Status: design-only (deferred build targets).** As of 2026-06-13, none of
> the editor state machine described here is implemented. `TextEditState`,
> `EditCommand`, `UndoStack`, `TextSelection`, the `ReadOnly` / `Disabled` /
> `SingleLine` / `Placeholder` markers, `PreeditSpan`, and the IME machinery
> (§§ 2.2, 3, 4, 6, 8) are **this campaign's implementation targets**, not
> verified code — `TextEditState` is explicitly deferred at
> `crates/buiy_core/src/text/components.rs`. The **painting surfaces** they drive
> (`CaretVisual`, `SelectionVisual`) and every architectural seam (focus, Tab,
> `ScrollOffset`, picking, `BuiySet` order, paint rank, damage gates) ARE built
> and verified (T6–T8); see the readiness report
> `docs/reports/2026-06-13-text-editing-design-readiness.md`.
```

### Patch 2 — record OQ#1 as RESOLVED (verifier B `major`)

The current OQ#1 says the file "takes either answer without structural change."
Verifier B shows the answer is **determinate**: one-frame latency, because
`BuiySet::Input` runs two sets after `BuiySet::Layout` (where `TextSync` /
`TextCommit` live) and nothing re-enters Layout after Input. Same-frame re-entry
is rejected — it needs a fourth Taffy compute site beyond the ≤2× cap.

**File:** `docs/specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md`

**Old text** (lines 547–555):

```
1. **Frame-ordering for edit→layout.** `BuiySet` chains Layout → … → Input → …
   → Render (`crates/buiy_core/src/lib.rs:57-87`), so edits mutate the Buffer
   *after* layout ran this frame: content-size changes and caret geometry either
   re-enter layout same-frame (within the architecture's 2×-Taffy-per-frame cap)
   or show one-frame latency. This must be settled **jointly with
   [measure-and-layout.md](measure-and-layout.md)** (which owns the
   measure function and `shape_as_needed` scheduling); this file takes either
   answer without structural change, but the typing-latency gate (§ 12) depends
   on it.
```

**New text:**

```
1. **Frame-ordering for edit→layout — RESOLVED (accepted one-frame latency).**
   `BuiySet` chains Layout → Style → Input → … → Render
   (`crates/buiy_core/src/lib.rs:64-95`), and `TextSync` / `TextCommit` live
   *inside* Layout (`pipeline.rs:80-101`). Editor input lands in `BuiySet::Input`,
   two sets **after** Layout, so a keystroke mutates the Buffer after this frame's
   layout already ran; the change is picked up by **next-frame** `TextSync` and
   flows TextSync → measure → TextCommit → `extract_buiy_glyphs`, publishing one
   frame later (N → N+1). Caret geometry, `ime_position` (§ 6.3), and auto-scroll
   (§ 9) come current the **same** frame the edit's TextCommit publishes, because
   the caret reads the `ComputedTextLayout` written in that TextCommit — no extra
   caret lag. This inherits the accepted one-frame latency pinned by
   [architecture.md § 5.1](architecture.md) (a `BuiySet::Style` value reaches
   shaping next-frame TextSync; the editor Input path is the same structure one
   set down).

   **Same-frame re-entry is rejected (blocker-class wrong).** It would need a
   fourth Taffy compute site beyond the architecture's 2×-per-frame cap; the only
   same-frame Layout re-runs (`cq_flip_rerun`, `cq_descendant_rerun`) are
   container-query driven and never fire from Input
   (`measure-and-layout.md` § 4.3). The editing campaign **must not** attempt
   same-frame re-entry — one-frame latency needs **zero** new machinery.

   **Gate caveat.** The T8 typing-latency fixture
   (`crates/buiy_core/tests/text_typing_latency.rs:80-109`) mutates the `Text`
   component *before* Layout, so it proves the **sync-side** path one-frame, not
   the editor Input path. The editor-input latency gate needs its **own**
   Input-driven N→N+1 fixture (edit applied in `BuiySet::Input`, glyph publish
   asserted at N+1); do not cite the T8 fixture as editor-path proof.
```

### Patch 3 — measure-and-layout cross-pin for OQ#1 (verifier B `minor`)

OQ#1 was to be settled *jointly* with `measure-and-layout.md`, but that file's
open-questions list carries no edit-to-layout frame-ordering entry. Add a
one-line cross-pin near § 4.3 (the compute-helper + ≤2× ceiling owner) so the
joint settlement is reflected on both sides.

**File:** `docs/specs/2026-06-09-buiy-text-rendering-design/measure-and-layout.md`

**Old text** (lines 361–362, the closing sentence of § 4.3's runner-up note):

```
against on the style side. Also rejected: a third text-driven re-run pass —
text needs no extra pass.
```

**New text:**

```
against on the style side. Also rejected: a third text-driven re-run pass —
text needs no extra pass.

**Edit-to-layout latency cross-pin (jointly with
[editing-and-ime.md](editing-and-ime.md) OQ#1).** Editor edits land in
`BuiySet::Input`, after these three compute sites all ran; there is no fourth
pass to re-enter, so the editor edit→layout path is **one frame** (N → N+1) by
construction, not same-frame. This is the resolution of editing-and-ime.md's
OQ#1 — recorded here because § 4.3 owns the ≤2× ceiling that forecloses same-frame
re-entry.
```

> **Optional (verifier C `minor`, non-blocking):** in `editing-and-ime.md` § 5,
> the split-caret note should state that a split (mixed-direction) caret needs a
> **secondary `CaretVisual` rect + a second stamp** — CPU geometry only, still no
> GPU work — and the glyph damage gate it references is at `prepare.rs:230-283`
> (not `157-216`). This is a precision fix to a correct claim, safe to fold into
> the plan rather than block on; left out of the verbatim patches above because it
> does not gate planning.

---

## Open question #1 — frame-ordering resolution

**Status: RESOLVED. Chosen answer: accepted one-frame latency for the editor
input path (N → N+1).**

**Why it is determinate (not "either answer").** The `BuiySet` chain is
`Layout → Style → Input → … → Render`, executed once per `Update` with no
loop-back (`lib.rs:64-95`). `TextSync` and `TextCommit` run *inside*
`BuiySet::Layout` (`pipeline.rs:80-101`). Editor input is Input-tier work
(`BuiySet::Input`, the tier where `handle_tab` and the button's `emit_on_press`
already sit, `focus.rs:56`), which runs **two sets after** Layout. So a keystroke
handled in Input mutates the Buffer *after* this frame's layout finished:

- Frame N: keystroke applied in Input → Buffer mutated (post-Layout).
- Frame N+1: `TextSync` sees the dirty Buffer at frame start → measure →
  `TextCommit` → `extract_buiy_glyphs` publishes the glyphs.

Caret geometry, `ime_position`, and auto-scroll do **not** lag an extra frame:
they read the `ComputedTextLayout` written in the same `TextCommit` that publishes
the edit, so they come current the frame the edit lands on screen.

**Why same-frame re-entry is rejected.** It would require a fourth Taffy compute
site beyond the architecture's 2×-per-frame cap. The only same-frame Layout
re-runs that exist (`cq_flip_rerun` step 5, `cq_descendant_rerun` step 9,
`layout/systems.rs`) are container-query driven, capped at one each per frame, and
never fire from Input. Achieving same-frame edit→layout would need a rejected
structural change; one-frame latency needs zero new machinery. This mirrors
`architecture.md § 5.1`, which already pins the symmetric `BuiySet::Style`
one-frame case.

**What the T8 typing-latency fixture actually proves.**
`text_typing_latency.rs:80-109` mutates the `Text` **component** directly between
frames (asserts at 87/101/106), so `TextSync` sees it at the next frame start and
the chain runs **same-frame** from that point. That exercises the **sync-side**
path (edit applied *before* Layout), not the editor **Input** path (edit applied
*after* Layout). It proves the sync path is one-frame; it **cannot** stand in for
the editor path. The editing campaign must add a **new Input-driven N→N+1
fixture** that applies an edit in `BuiySet::Input` and asserts glyph publication
one frame later.

**Verifier agreement:** B resolved this; A and C did not contradict it (A confirms
the `BuiySet` order; C confirms the painting path the resolution publishes
through). No human adjudication needed.

---

## Confirmed-OK (load-bearing claims verified)

These were positively verified against code — coverage was real, not assumed.

**Integration seams (verifier A):**

- `FocusedEntity(Option<Entity>)` and `FocusVisible(bool)` gate editor access —
  `focus.rs:38,45`.
- Tab traversal reads Shift+Tab via `ButtonInput<KeyCode>`, updates
  `FocusedEntity`, sets `FocusVisible` true — `focus.rs:60-73`.
- Paint rank `Quad=1 < Glyph=2` (selection quads paint under text) —
  `buckets.rs:43-53`.
- Independent glyph damage gate (`glyphs.is_changed()` uploads the glyph buffer
  alone — efficient caret blink) — `prepare.rs:230-283`.
- `GlyphAlphaInstance.color` is straight-alpha linear, never premultiplied;
  shader outputs `color.a*coverage` — `atlas/primitive.rs:41-47`.
- `ScrollOffset` does **not** invalidate `ResolvedLayout` (invariant test exists)
  — `components.rs:516-526`, `layout_scroll_offset_no_invalidate.rs`.
- `Button::new` returns `impl Bundle` (the `TextInput` widget precedent) —
  `button.rs:35-58`.
- Bevy 0.18 Event/Message split; `OnPress` is a `Message` — `button.rs:6-8,26`.
- `BuiySet` order `Layout → Style → Input → Animate → Picking → A11yUpdate →
  Render`, chained once — `lib.rs:61-94`.
- `CaretVisual` (visible + rect) and `SelectionVisual` (cursor endpoints) exist
  as built T7 painting components — `text/components.rs:384,421`.

**Frame ordering (verifier B):**

- `BuiySet` chains with no loop-back; Input is two sets after Layout; `TextSync` /
  `TextCommit` are inside Layout — `lib.rs:64-95`, `pipeline.rs:80-101`.
- Editor input is Input-tier by precedent (`handle_tab`, `emit_on_press`) —
  `focus.rs:56`, so editor mutation is post-Layout.
- `architecture.md § 5.1` already pins the symmetric one-frame `BuiySet::Style`
  case — the editor path inherits it.
- The only same-frame Layout re-runs are `cq_flip_rerun` / `cq_descendant_rerun`,
  container-query driven, ≤1 each per frame — `layout/systems.rs`. Nothing
  re-enters Layout after Input.

**No-new-GPU-work painting (verifier C):**

- Selection rects emit `TextQuad` via `push_selection_quad`, packed like a node
  quad (`pack_text_quad`) — quad tier, rank 1, under text, no new GPU.
- Per-cluster recolor / selection-fg is a per-instance `GlyphAlphaInstance.color`
  tint over R8 coverage — never touches the atlas.
- Caret is a `GlyphAlphaInstance` stamped on the warmup-pinned 1×1 value-255
  `CoverageR8` texel, emitted **last** (after run glyphs) — `node.rs:289-345`
  draws quad buffer then glyph buffer.
- Preedit underline is a quad-tier `TextQuad`; every quad routes to layer 0 in
  v1, so a quad caret is impossible by construction.

---

## Readiness verdict

**ready-with-patches.** There are no blockers: every architectural seam the
editor design depends on is verified in code, the painting path is confirmed to
need no new GPU work, and the one genuinely-open question (OQ#1 frame-ordering) is
now determinate — accepted one-frame latency, with same-frame re-entry rejected on
the ≤2×-Taffy cap. The three verifiers corroborate rather than conflict, so no
fact needs human adjudication. The four spec patches above are housekeeping the
plan must not inherit stale: (1) a design-only status banner so §§ 2.2/3/4/6/8
aren't read as built code, (2) recording OQ#1 as RESOLVED in `editing-and-ime.md`,
(3) the matching cross-pin in `measure-and-layout.md` § 4.3, plus the optional
split-caret/anchor precision fix. Apply the patches, then the implementation plan
can be written — with one new requirement baked in: the editor-input latency gate
needs its **own** Input-driven N→N+1 fixture, because the existing T8
typing-latency fixture proves only the sync-side path.
