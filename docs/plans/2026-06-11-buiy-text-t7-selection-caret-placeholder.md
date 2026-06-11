# Buiy Text T7: Selection + Caret + Placeholder Painting — Implementation Plan

**Date:** 2026-06-11
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/decoration-and-paint.md](../specs/2026-06-09-buiy-text-rendering-design/decoration-and-paint.md) §§ 5–8 + [glyph-pipeline.md](../specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md) §§ 6.2–6.3 + [verification.md](../specs/2026-06-09-buiy-text-rendering-design/verification.md) §§ 1.2–1.3, 3.1
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T7 (depends on T6; the implementer starts from a branch with T1–T6 merged — T6 landed @ `1efc393`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The painting primitives the successor `buiy-text-editing` campaign
consumes — selection rectangles, selected-text re-tint, the caret, and
`::placeholder` — through the already-built, GPU-verified render primitives.
Land the `CaretVisual`/`SelectionVisual` paint-input state components
(decoration-and-paint § 6.3's pinned shapes) with the virtual-clock blink
evaluated in a render-prep writer in the `Animate→Picking` window —
**edge-only writes** (idempotent when the phase doesn't flip; an
unconditional per-frame write would keep `Changed<CaretVisual>` perpetually
hot and kill extract retention) and `prefers-reduced-motion` ⇒ steady
visible; `LayoutRun::highlight`-driven selection rects (`color.selection.bg`)
as quad-tier instances at § 4.4 **seat 2** via the existing
`ExtractedTextQuads` carrier — the multi-rect mixed-BiDi contract inherited
from upstream, zero Buiy-side BiDi math; per-cluster selected-text re-tint
(`color.selection.fg`) as pure per-instance `GlyphAlphaInstance.color`
overrides (the atlas is never touched); the caret as a solid-stamp
glyph-tier instance at **seat 6** (pre-phase decision 2 — T6's warmup-pinned
1×1 `CoverageR8` stamp, reused verbatim) with the `caret-color` resolution
chain; and `::placeholder` as same-pipeline tinted text (one theme token, no
new paint path). The `Changed<CaretVisual>`/`Changed<SelectionVisual>` probe
union members glyph-pipeline § 6.2 reserved become real, and carrier
publication becomes **value-compared** so a blink edge touches
`ExtractedGlyphs` only — the § 6.3 damage property, CPU half.

**Architecture:** The cosmic-text boundary holds. `buiy_core::text` gains the
two paint-input state components (`components.rs` — `SelectionVisual` carries
`cosmic_text::Cursor` endpoints, legal in the text module exactly like
`TextBuffer`'s `Buffer`), a new `visual.rs` (the blink writer + the pure
blink/caret-rect helpers + the `CaretBlinkInterval` resource), and the
producer growth in `extract.rs` (selection pass, re-tint, caret stamp, union
members, value-compared publish). The render side gains one author-set
component (`CaretColor`, beside `TextColor` in `render/components.rs`), the
token-resolution helpers + token-name constants (`render/color.rs` — the
`resolve_token` `CurrentColor`-arm idiom extended to the selection pair, so
forced-colors resolves `Highlight`/`HighlightText` with zero new mechanism),
default-theme tokens (`theme.rs`), and a `PartialEq` derive on
`GlyphAlphaInstance` (the value-compared publish needs it). **No new GPU
buffer, pipeline, shader, draw branch, atlas entry kind, or carrier type** —
selection rects ride the T6 `TextQuad` splice, the caret rides the T6 stamp.

**v1 slice (decoration-and-paint §§ 5–8, verbatim):** single-range selection
painting (the multi-range *type* is the editing campaign's § 4.2);
grapheme-accurate selection rects per run via `highlight()` with upstream's
reference-render extensions (internal-empty-line full-width rect; last-rect
extension to the line edge on multi-line selections, RTL-aware); per-cluster
re-tint (a partially selected ligature re-tints whole while its rect stays
grapheme-accurate — the accepted upstream-matching tradeoff, § 5.2); one
caret stamp per entity (`visible` + content-box-local rect), width snapped by
the § 3.3 rule at emission; the global square-wave blink off the app clock;
`caret-color` → theme caret token → `currentColor`; placeholder = the
`color.text.placeholder` token through the unchanged pipeline.

**Where T7 ends (honesty pins — named seams, not built):**

- **T7 paints FROM state; the editing campaign owns mutating it.** Actions,
  keymaps, IME, focus-driven caret movement, cursor logical→visual
  resolution (incl. the BiDi split caret = two stamps later), selection
  endpoint production (`Editor::selection_bounds()`), and the
  placeholder-when-empty swap all belong to `buiy-text-editing`
  (editing-and-ime.md §§ 2–13). In T7, tests and examples write
  `CaretVisual`/`SelectionVisual` directly; the editing campaign's
  render-prep systems become their production writers with **no shape
  change** — the components are exactly the § 6.3/§ 5.1 pinned interface.
- **Blink phase is global** — a pure square wave of the app clock
  (§ 6.3's literal pin). The per-entity blink-timer **reset on every edit
  and caret move** (editing-and-ime § 5/§ 10) rides the editing campaign's
  `CaretBlink` state; T7's writer never needs to change for it (the editor
  will hold `visible: true` through its own edge writes).
- **The blink period default is a plugin resource**
  (`CaretBlinkInterval`, 500 ms half-period), not a theme token — the
  Phase-0 `Theme` has no motion scale; the token indirection is
  `buiy-theme-tokens-design`'s seam (§ 6.3: "the blink period is a
  theme/animation value, not pinned here").
- **`SelectionVisual` is single-range-shaped in v1.** The multi-range
  generalization (a `SmallVec` of ordered pairs) is additive when the
  editing campaign's `TextSelection` (§ 4.2) lands behavior for it.
- **Caret-blink GPU buffer-reupload assert is T8's** (campaign T8: "blink
  frame touches the glyph buffer only"). T7 lands the **CPU half**
  (verification § 1.2's damage row): a blink edge rebuilds
  `ExtractedGlyphs` and leaves `ExtractedTextQuads` untouched, via the
  value-compared publish.
- **Glyph stamps in effect groups (T8).** A caret inside an `Opacity(0.5)`
  card does not dim (glyph draws bypass group compositing until T8's
  partition) — same § 4.5 asymmetry T6 pinned for line-through. No new pin
  needed; T6's adjacent-assertion test already guards the seam.
- **Placeholder forced-colors mapping (`GrayText`)** is a
  `buiy-theme-tokens-design` seam: under the wholesale forced-colors theme
  swap, `color.text.placeholder` misses loudly (magenta) like every named
  token — the selection pair is special-cased here only because the
  `Highlight`/`HighlightText` system keys exist and the `CurrentColor`
  prefer-when-present idiom already established the pattern.
- **Placeholder non-selectability is structural, not enforced**: a
  placeholder entity simply carries no `CaretVisual`/`SelectionVisual`
  (§ 7: "a placeholder is never selectable"); the editing campaign owns
  guaranteeing that when it owns who carries the components.
- **Preedit underline (§ 8)** is consumed unchanged by the editing
  campaign — a forced `Single` underline over the preedit range through
  T6's machinery. Nothing to build in T7; named so nobody invents a new
  decoration kind for it.

**Tech stack:** cosmic-text 0.19.0 (default features — no change), existing
workspace deps only. **No new dependencies** — if a task appears to need
one, STOP: that contradicts the charter. (`cargo deny check` not required:
no dep changes.)

**Test reality:** components, the blink writer, token resolution, selection
rect numbers (incl. mixed-BiDi multi-rect via the committed Hebrew fixture
font), re-tint colors, caret rect snap math, and the damage discipline are
all headless (the producer runs on the adapterless
`TextExtractHarness`; the blink writer is a plain `Update` system driven by
`Time<Virtual>`). The GPU lane carries one new file: the mixed-BiDi
`::selection` golden and the caret-blink fixed-clock pair — the campaign's
exact T7 GPU surface. Every GPU test keeps `#[ignore]` and builds on
`tests/support/mod.rs`.

---

## The gate (run BOTH lanes at every task boundary)

T7 ships producer/render changes and GPU tests, so the per-task gate is the
headless gate **plus the GPU lane** (this host has the RX 6700 XT / RADV;
Vulkan render-to-texture needs no display):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace -j 2
```

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

Expected: both green. The headless gate must stay green **independently**
(CI has no adapter); the GPU lane is additive and must pass on this host
before the phase merges. A GPU-needing test without `#[ignore]` panics the
headless run at adapter init — self-policing.

---

## Orientation: verified facts this plan builds on

Source-verified against the vendored crate
(`~/.cargo/registry/src/*/cosmic-text-0.19.0/`) and the as-built T1–T6 tree
at `1efc393`. **The selection/caret semantics below are THE normative mirror
target — the exact-number tests in Tasks 3–4 pin them.**

### 1. `LayoutRun::highlight` — the real signature and its caller contract (buffer.rs:66–113)

```rust
pub fn highlight(
    &self,
    cursor_start: Cursor,
    cursor_end: Cursor,
) -> impl Iterator<Item = (f32, f32)>   // per-run (x_left, width) spans
```

— an **iterator of spans**, exactly as decoration-and-paint § 5.1 pins (the
campaign-charter shorthand `-> Option<(f32, f32)>` is wrong; § 5.1 governs).
Semantics, verified line by line:

- **Grapheme-proportional:** each glyph cluster's width is split evenly
  across its graphemes (`c_w = glyph.w / grapheme_count`), so a partially
  selected ligature contributes a *fractional* span — the § 5.1
  "splits ligature clusters proportionally" claim, confirmed.
- **Multi-span for mixed BiDi:** spans accumulate over glyphs in **visual**
  order and flush whenever an unselected grapheme interrupts — pure LTR/RTL
  runs yield ≤ 1 span; a logical range straddling a direction boundary
  yields **multiple disjoint spans** (one per intersected BiDi segment).
  Zero-width spans are dropped (`width > 0.0` guard on flush).
- **The selection predicate is half-open** `[start, end)` per grapheme:
  `(start.line != line_i || c_end > start.index) && (end.line != line_i ||
  c_start < end.index)` — so a **collapsed** pair (`start == end`) on a
  grapheme boundary yields no spans.
- **CALLER MUST LINE-GATE — the unstated contract.** For a run on a line
  *outside* `[start.line, end.line]` both disjuncts hold vacuously and the
  predicate degenerates to all-selected. Upstream's own reference render
  gates first (`if line_i >= start.line && line_i <= end.line`,
  edit/editor.rs:103); the producer mirrors that gate. **Calling
  `highlight` unguarded paints full-line selections on every line — the
  bug this Orientation entry exists to prevent.**

### 2. Upstream's reference selection render — the behaviors Buiy mirrors (edit/editor.rs:93–142)

Per run (after the line gate), `rect = (x, line_top, w, line_height)` per
span, plus two reference behaviors that live in `Editor::render`, NOT in
`highlight`:

- **Internal fully-selected empty lines** (`highlights.is_empty() &&
  run.glyphs.is_empty() && end.line > line_i`): one full-width rect, x = 0
  to the buffer's set width (`buffer.size().0`).
- **Multi-line extension:** on a selected line that is not the selection's
  last (`end.line > line_i`), the **last** span extends to the line edge —
  `max = buffer width` for LTR runs, `min = 0` when `run.rtl` — so the
  selected newline is visible, web-style.

`Buffer::size(&self) -> (Option<f32>, Option<f32>)` (buffer.rs:813);
`TextCommit` always commits `set_size(Some(w), Some(h))` (T3 decision 9),
so the width is `Some` at extract — the producer still falls back to
`computed.size.x` defensively rather than copying upstream's
`unwrap_or(0.0)` (which would silently zero the extension).

### 3. The cursor-position family — what T7 consumes vs what it doesn't

- `LayoutRun::cursor_position(&self, cursor: &Cursor) -> Option<f32>`
  (buffer.rs:120–142): visual x within the run; RTL glyphs place at
  `glyph.x + glyph.w − offset`, LTR at `glyph.x + offset`; past-the-end
  lands after the last glyph; `None` when the cursor is not on this run.
- `Buffer::cursor_position(&self, cursor: &Cursor) -> Option<(f32, f32)>`
  (buffer.rs:1247–1251): `(x, line_top)` of the first matching run.
- `Editor::cursor_position() -> Option<(i32, i32)>` (edit/editor.rs:861;
  the int cast is a private helper, edit/mod.rs:30–33) — the § 6.1 spec
  mention. **T7 consumes NONE of these**: the caret rect arrives *authored*
  in `CaretVisual` (the § 6.3 carrier — geometry resolution is the editing
  model's job, § 6.1 "the caret position comes from the editing model").
  They are listed so the editing campaign's writer has its verified
  sources, and to pin the erratum that the f32-precise per-run form (not
  the `Editor` i32 cast) is the right production source.
- `Cursor { line: usize, index: usize, affinity: Affinity }`
  (cursor.rs:3–11), `Affinity { Before (default), After }` — `highlight`
  and the re-tint predicate read only `line`/`index`; affinity matters to
  the editing campaign's run-boundary resolution, carried through
  `SelectionVisual` untouched. `selection_bounds()` (edit/mod.rs:217–256)
  returns the **normalized** ordered pair — `SelectionVisual` pins the same
  normalization as its invariant. `Motion`/`Selection`/`Action` are
  editing-campaign types; **nothing in T7 names them.**

### 4. The selected-glyph re-tint predicate (edit/editor.rs:153–164)

Upstream re-tints per **glyph** (cluster granularity — § 5.2's accepted
tradeoff), with the same line gate plus byte intersection:

```rust
line_i >= start.line && line_i <= end.line
    && (start.line != line_i || glyph.end > start.index)
    && (end.line != line_i || glyph.start < end.index)
```

Upstream applies it **over** `glyph.color_opt` (a rich-text span color is
re-tinted too) and short-circuits when `text_color == selected_text_color`
— a pure optimization Buiy drops (equal resolved colors produce identical
output; the token compare is meaningless after resolution).

### 5. The as-built producer (the emission site, text/extract.rs @ 1efc393)

- 15 system params (the 16-cap headroom T6 reserved for T7); the `texts`
  fan is a 10-item query tuple, the `changed` union a 10-member `Or` —
  T7 adds 3 to each (13 ≤ Bevy's 15-tuple limit; the next phase to grow
  either must start nesting).
- The § 6.2 ledger comment names exactly this join: "Union members that
  join later, in lockstep with their carriers: `Changed<CaretVisual>` /
  `Changed<SelectionVisual>` (T7)".
- Per-entity context already computed: `origin = gt.translation().truncate()
  + computed.content_offset`, `eff_clip`/`clip`, `resolved_entity_color` +
  linearized `entity_color`, `blocked` (`PendingFontBlock`), and the
  frame-cached `scale_factor`. The per-frame `stamp_entry:
  Option<AtlasEntry>` (one lock-free residency probe, self-healing
  `get_or_insert`) and the per-instance `new_keys.push(solid_stamp_key())`
  touch-pass join are T6's — the caret reuses both verbatim.
- **Publication is currently unconditional on dirty frames**
  (`glyphs.glyphs = new_glyphs; text_quads.quads = new_quads;` — T6
  decision 12's wholesale republish). This **conflicts** with
  decoration-and-paint § 6.3's damage property ("a blink frame changes only
  `ExtractedGlyphs`, so the quad buffer is retained") and verification
  § 1.2's damage row ("a caret-blink frame touches *only*
  `ExtractedGlyphs`"): a blink edge fires the union → both carriers
  republish → the quad buffer re-uploads every blink. Decision 4 resolves
  it (value-compared publish). `GlyphAlphaInstance` derives only
  `Clone, Copy, Pod, Zeroable` today — the compare needs `PartialEq`
  (plain `[f32; 4]`s + `u32`; derivable).
- T6's emission order within the run loop: decoration quads are pushed
  *inside* the per-run walk. Selection rects must land **before** every
  decoration quad of the entity (seat 2 < seat 3) — decision 5 makes the
  selection pass a separate per-entity pre-pass over `layout_runs()`
  (iteration only — no shaping, no locks).

### 6. The render-prep window + the edge-only write idiom

`write_clip_rects` / `write_effect_groups` / `write_paint_skip` all run in
`Update`, `.after(BuiySet::Animate).before(BuiySet::Picking)`
(render/mod.rs:119–149) with reconcile guards so a steady frame issues zero
writes (clip.rs:328–352's `reconcile_one`). The blink writer joins that
window from `BuiyTextPlugin` (main-world, headless-safe — no RenderApp
dependency). For a `Query<&mut CaretVisual>`, Bevy's `Mut` marks the
component changed only on `DerefMut` — so `if caret.visible != phase {
caret.visible = phase; }` IS the edge-only write: no flip, no tick.
`Res<Time>` in `Update` is virtual-backed; tests step it via
`Time<Virtual>::advance_by` (the text_font_display.rs:330–336 idiom — "the
stepped-clock discipline — no sleeps"). `GoldenConfig::fixed_clock`
(golden.rs:19–22) is exactly this: the GPU pair captures at chosen virtual
instants (verification § 3.1).

### 7. Theme + token state (theme.rs, render/color.rs @ 1efc393)

- `Theme.colors` is a flat string-keyed map; `default_light_theme()` has
  **no** `color.selection.*`, `color.caret`, or `color.text.placeholder`
  entries — Task 1 adds the selection pair + placeholder (a producer
  resolving a missing token warns + paints magenta, color.rs:146–153).
- `forced_colors_theme()` carries **exactly the 16 system-color keys**
  ("the keys, not the values, are the contract") and
  `apply_forced_colors_theme` swaps the Theme **wholesale**
  (forced_colors.rs:30–56) — under forced colors every named token misses.
  `resolve_token`'s `CurrentColor` arm already established the escape:
  *prefer the system key when present* (CanvasText, color.rs:132–142).
  Decision 6 extends exactly that idiom to `Highlight`/`HighlightText`.
- `UserPreferences.prefers_reduced_motion` exists (theme.rs:51–58);
  consumers take `Option<Res<UserPreferences>>` so plugins stay
  self-sufficient without `ThemePlugin` (the `apply_forced_colors_theme`
  precedent — the T1-era standalone text tests carry no theme stack).

### 8. Fixtures + harnesses

- `TextExtractHarness` (tests/support/extract_harness.rs): adapterless
  extract with `GlyphChangeLog`/`TextQuadChangeLog` mirroring the prepare
  gates — `changed_frames()` / `quad_changed_frames()` are the damage
  assertion currency. It registers `BuiyTextPlugin` + `CorePlugin`, so the
  blink writer runs inside `harness.frame()`'s `app.update()`.
- The committed Hebrew fixture font
  (`tests/fixtures/fonts/NotoSansHebrew-hebrew.ttf`, OFL) + the
  `"hello עולם world"` mixed-BiDi corpus line + the
  `[Fira Sans, Noto Sans Hebrew]` stack are T5's
  (text_shaping_snapshots.rs:46/67/84); `support::register_fixture_font`
  registers via the `FontRegistry` bytes path — headless and GPU both.
- GPU idioms: `gpu_render_app`/`render_to_image`/`spawn_capture_camera`/
  `finish_and_run`/`wait_for_text_ready`/`readback_rgba`; band/column
  classification with chroma-orthogonal test tokens; "re-capture IS the
  golden" determinism via `perceptual_diff` (text_decoration_gpu.rs).

---

## Decisions (with runner-ups) — read before implementing

1. **`SelectionVisual` carries the normalized endpoint pair, not rect
   lists.** `SelectionVisual { start: cosmic_text::Cursor, end:
   cosmic_text::Cursor }`, invariant `start ≤ end` in logical order — the
   `selection_bounds()` output shape verbatim. § 5.1 is explicit: "**the
   endpoints reach the producer** through the render-prep-written
   `SelectionVisual` state (§ 6.3's window), so emission itself stays in
   the `ExtractSchedule` producer" — the producer derives rects via
   `highlight()` *inside the run walk it already owns*, and the re-tint
   predicate reads the same endpoints. § 6.3's parenthetical ("the § 5
   rect list plus re-tint ranges") is recorded as a T7 erratum: a rect
   list + re-tint ranges are two *derived* copies of the one endpoint
   source — they'd force a second run walk in render-prep, and "re-tint
   ranges" ARE the endpoints restated.
   **Rejected runner-up:** resolved-geometry payload (rect list + byte
   ranges) — duplicates one source of truth across two components,
   recomputes geometry outside the producer's damage gate, and still goes
   stale against reshape (a wrap-width change moves rects; endpoints
   survive). **Also rejected:** Buiy-shaped `(line, index)` mirror structs
   instead of `cosmic_text::Cursor` — the component lives in
   `buiy_core::text` (the cosmic boundary module, the `TextBuffer`
   precedent), the editing campaign's pinned types already use `Cursor`
   (editing-and-ime § 4.2), and a mirror would lose `affinity`, which the
   editor needs round-tripped.

2. **`CaretVisual` is the § 6.3 shape verbatim; the rect is
   content-box-local logical px.** `CaretVisual { visible: bool, rect:
   bevy::math::Rect }` — `rect = (caret_x, line_top, caret_w,
   line_height)` in buffer-local coordinates (§ 6.1's terms are run-local);
   the producer folds `origin` and applies the § 3.3 snap at emission, like
   every other text visual. The component is *machinery state* (written by
   tests now, the editing campaign later) — not reflect-registered (the
   `ComputedTextLayout` convention).
   **Rejected runner-ups:** (a) window-space rect — goes stale on every
   node move and couples the writer to transforms (the producer already
   rebuilds on `Changed<GlobalTransform>` with the origin fresh);
   (b) a cursor-shaped caret input (`Cursor` + T7-side `cursor_position`
   resolution) — hands T7 the logical→visual caret resolution that
   editing-and-ime § 4.1 owns (incl. the affinity-pair split caret), i.e.
   designing editing; (c) pre-snapped width — the § 3.3 rule needs
   `scale_factor`, which only the producer tracks (the cached-`f32`
   probe); render-prep snapping would re-write `CaretVisual` on every
   monitor change for no reason.

3. **The blink writer manages exactly one field: `visible`.**
   `write_caret_blink` runs in the `Animate→Picking` window; phase =
   `prefers_reduced_motion || blink_phase(time.elapsed(), interval)`
   (square wave: integer division of elapsed by the half-period, even =
   visible — so t=0 is visible, matching "caret becomes visible" on
   insert); per entity, write only on a flip (`Mut` DerefMut-on-change —
   Orientation § 6). Reduced-motion is therefore steady-visible with
   **zero** writes after the first reconcile (text.md:90 via § 6.3).
   `CaretBlinkInterval(Duration)` is the half-period resource, default
   500 ms, zero ⇒ steady visible (defensive, documented).
   **Rejected runner-ups:** (a) unconditional per-frame write — keeps
   `Changed<CaretVisual>` hot, rebuilds the producer every frame, kills
   the O(0) steady state (the exact anti-pattern § 6.3 names);
   (b) a per-entity timer with edit/move reset — that is the editing
   campaign's `CaretBlink` state (honesty pin); (c) a `Timer`-resource
   tick system — a square wave *of the clock* is stateless and exactly
   § 6.3's words; a timer adds reset/drift semantics for nothing.

4. **Value-compared carrier publication** (refines T6 decision 12; the
   § 6.3 damage property requires it): the producer still **rebuilds**
   both carriers wholesale on any dirty frame (one damage decision —
   unchanged), but **publishes** each only when its content differs:
   `if glyphs.glyphs != new { glyphs.glyphs = new; }` and likewise for
   `text_quads`. A blink edge then marks `ExtractedGlyphs` changed (the
   caret instance toggles) while `ExtractedTextQuads` compares equal and
   keeps its tick — the quad buffer is retained (prepare.rs:176's triple
   gate sees no change), satisfying verification § 1.2's "a caret-blink
   frame touches *only* `ExtractedGlyphs`". Equal inputs produce
   bit-identical f32 outputs (same math, same order), so derive-`PartialEq`
   equality is deterministic. Cost: one O(instances) compare per **dirty**
   frame — steady frames never reach it. Needs `PartialEq` on
   `GlyphAlphaInstance` (plain POD; added in Task 4).
   **Rejected runner-ups:** (a) keep unconditional republish — every blink
   re-uploads the quad buffer, violating § 6.3 and gate #14; (b) split the
   damage decision (separate caret-only probe rebuilding only glyphs) —
   two probe unions to keep coherent, and a caret move + text edit in one
   frame would need both anyway; the spec pins ONE decision; (c) publish
   glyphs unconditionally and compare only quads — asymmetric for no
   gain; the same guard makes a quad-only change (e.g. a selection-rect
   move on an unblinking frame… impossible today but free) retain glyphs.

5. **Selection rects are a per-entity pre-pass over `layout_runs()`,
   before the decoration+glyph walk** — strict § 4.4 carrier order
   (seat 2 selection < seat 3 underline/overline for the WHOLE entity,
   not per run). A per-run interleave (sel₁, deco₁, sel₂, deco₂) would
   let run 1's underline paint *under* run 2's selection rect where line
   boxes touch (an underline offset can cross the next line's `line_top`)
   — a CSS painting-order violation in exactly the overlap case the seat
   table exists for. The pre-pass is iteration only: no shaping, no
   locks, no raster work; the collapsed-selection case
   (`start == end`) skips the pass entirely (a collapsed selection is a
   caret, not a selection — and skipping also removes the
   mid-grapheme-cursor re-tint edge upstream's predicate admits).
   **Rejected runner-up:** buffer selection quads through the main walk
   like T6 buffers strikes — works, but the strikes buffer is per-RUN
   (bounded, SmallVec); selection quads would buffer per-ENTITY across
   the whole walk for no benefit over a second cheap iteration.

6. **Selection colors resolve through `Highlight`/`HighlightText` when
   present, else the `color.selection.bg`/`.fg` tokens** — helpers
   `resolve_selection_bg/fg(theme)` in `render/color.rs`, mirroring
   `resolve_token`'s `CurrentColor` arm byte-for-byte (prefer the system
   key when the active theme carries it — i.e. under forced colors —
   else the named token, magenta-on-miss). This is CSS forced-colors
   behavior for `::selection` with zero new mechanism and zero new theme
   keys. Token names are `pub const`s beside the helpers.
   **Rejected runner-ups:** (a) add `color.selection.*` to
   `forced_colors_theme()` — its 16-key map is a pinned contract ("the
   keys, not the values"); (b) a `ColorToken::Selection` variant —
   resolution-order logic does not belong in the token data type (the
   variant set is CSS-shaped); (c) leave forced-colors to magenta — the
   established idiom covers it for two hash probes.

7. **`caret-color` chain: explicit `CaretColor` → the `color.caret` theme
   key *if present* → the resolved entity foreground** (`currentColor`) —
   § 6.2's order, with the middle tier a **presence check**
   (`Theme::color` returns `Option`), not a magenta-on-miss resolve.
   `color.caret` is deliberately **NOT** added to the default theme: CSS
   `caret-color: auto` means the caret matches the text color, and a
   default-theme entry would silently break that parity for every app;
   the key exists for themes that want a distinct caret. `CaretColor(pub
   ColorToken)` is author-set → lives beside `TextColor` in
   `render/components.rs`, reflect-registered. Under forced colors the
   auto chain lands on `CanvasText` via the existing `CurrentColor` arm —
   correct for free.
   **Rejected runner-up:** seed `color.caret` in `default_light_theme()`
   — breaks `auto` parity (tier 3 would never fire) and pins a palette
   value this spec explicitly does not own.

8. **Mirror upstream's two reference-render selection behaviors**
   (Orientation § 2): the internal-empty-line full-width rect and the
   last-rect line-edge extension (LTR → buffer width, RTL → 0), with
   `buffer.size().0.unwrap_or(computed.size.x)` as the width source
   (upstream's `unwrap_or(0.0)` would silently drop the extension; commit
   guarantees `Some`, the fallback is defense).
   **Rejected runner-up:** spans-only (skip both) — multi-line selections
   would render with invisible newlines and zero-width empty lines,
   visibly wrong against every editor; the behaviors are mechanical
   derivations of the endpoints, squarely T7 painting.

9. **Caret and selection rects paint normally under `font-display:
   Block`; selected-glyph re-tint inherits the glyph zero-alpha.** Block
   hides the *text's ink* (font-assets § 7: layout-identical,
   paint-invisible) — the caret and the selection background are editor
   chrome, not ink: browsers keep the caret visible in a focused input
   whose font is still loading, and a selection highlight over
   not-yet-visible text is exactly what re-selection during load looks
   like on the web. The re-tinted glyphs themselves stay zero-alpha (the
   existing glyph arm runs after the re-tint pick).
   **Rejected runner-up:** zero-alpha everything under Block (the T6
   decoration rule applied uniformly) — decorations ARE ink (they track
   the text's extent and style); chrome is not, and a vanishing caret in
   a focused field is a real UX bug.

10. **Placeholder is one token + one helper, no pipeline branch** (§ 7
    verbatim): `color.text.placeholder` in the default theme and
    `TextColor::placeholder()` returning
    `TextColor(ColorToken::Token("color.text.placeholder"))` — the
    discoverable constructor the editing campaign's swap logic will use.
    The "same pipeline" claim is pinned by a harness test (a
    placeholder-tinted entity's instances differ from a normal entity's
    ONLY in `color`), not by new machinery.
    **Rejected runner-up:** a `Placeholder` marker component with
    producer-side tint override — duplicates `TextColor`'s job and builds
    half of the editing campaign's § 10 swap (which owns *when* the
    placeholder shows).

---

## File structure

```
crates/buiy_core/src/
├── text/
│   ├── components.rs        M  CaretVisual + SelectionVisual (Task 1)
│   ├── visual.rs            C  CaretBlinkInterval + blink_phase + write_caret_blink + caret_stamp_rect (Tasks 2, 4)
│   ├── extract.rs           M  union growth, selection pass + re-tint, caret stamp, value-compared publish (Tasks 3–4)
│   └── mod.rs               M  module decl, exports, resource init, writer registration (Tasks 1–2)
├── render/
│   ├── components.rs        M  CaretColor (Task 1)
│   ├── color.rs             M  token-name consts + resolve_selection_bg/fg + resolve_caret_color (Task 1)
│   ├── atlas/primitive.rs   M  + PartialEq on GlyphAlphaInstance (Task 4)
│   └── mod.rs               M  register_type::<CaretColor> (Task 1)
├── theme.rs                 M  color.selection.bg/.fg + color.text.placeholder defaults (Task 1)
crates/buiy_core/tests/
├── text_caret_selection.rs  C  components/tokens/resolution + blink writer + selection + caret + placeholder (Tasks 1–5)
├── render_components_registry.rs M  CaretColor registration row (Task 1)
└── text_selection_caret_gpu.rs    C  mixed-BiDi ::selection golden + caret-blink fixed-clock pair, #[ignore] (Task 6)
docs/
├── README.md                M  T7 catalog entry (Task 7)
└── plans/2026-06-09-buiy-text-campaign.md  M  status flip + T7 errata (Task 7)
```

---

### Task 1: Paint-input components + tokens + resolution helpers (headless)

The state surface and the color plumbing: `CaretVisual`/`SelectionVisual`
(the § 6.3/§ 5.1 pinned shapes), `CaretColor`, the three default-theme
tokens, `TextColor::placeholder()`, and the resolution helpers with their
forced-colors arms.

**Files:**
- Create: `crates/buiy_core/tests/text_caret_selection.rs`
- Modify: `crates/buiy_core/src/text/components.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/src/render/components.rs`
- Modify: `crates/buiy_core/src/render/color.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/src/theme.rs`
- Modify: `crates/buiy_core/tests/render_components_registry.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_caret_selection.rs` (this file grows
across Tasks 1–5 — start it with the Task-1 sections):

```rust
//! T7 selection + caret + placeholder painting — the paint-input state
//! components, token resolution, the blink writer (Task 2), selection
//! emission (Task 3), caret emission + damage (Task 4), placeholder
//! (Task 5). Spec: decoration-and-paint §§ 5–8; glyph-pipeline §§ 6.2–6.3.

mod support;

use bevy::math::Rect;
use bevy::prelude::*;
use buiy_core::render::color::{
    CARET_COLOR_TOKEN, ColorToken, PLACEHOLDER_COLOR_TOKEN, SELECTION_BG_TOKEN,
    SELECTION_FG_TOKEN, resolve_caret_color, resolve_selection_bg, resolve_selection_fg,
};
use buiy_core::render::components::{CaretColor, TextColor};
use buiy_core::text::{CaretVisual, SelectionVisual};
use buiy_core::theme::{Theme, default_light_theme, forced_colors_theme};
use cosmic_text::Cursor;

// --- Task 1: components, tokens, resolution --------------------------------

#[test]
fn default_theme_carries_the_t7_tokens_but_not_color_caret() {
    let t = default_light_theme();
    assert!(t.color(SELECTION_BG_TOKEN).is_some(), "selection bg token");
    assert!(t.color(SELECTION_FG_TOKEN).is_some(), "selection fg token");
    assert!(t.color(PLACEHOLDER_COLOR_TOKEN).is_some(), "placeholder token");
    // Decision 7: caret-color `auto` parity — NO default caret entry, the
    // chain falls through to currentColor.
    assert!(t.color(CARET_COLOR_TOKEN).is_none(), "no default caret token");
}

#[test]
fn selection_colors_resolve_named_tokens_in_a_normal_theme() {
    let t = default_light_theme();
    assert_eq!(resolve_selection_bg(&t), t.color(SELECTION_BG_TOKEN).unwrap());
    assert_eq!(resolve_selection_fg(&t), t.color(SELECTION_FG_TOKEN).unwrap());
}

#[test]
fn selection_colors_prefer_system_keys_under_forced_colors() {
    // Decision 6: the wholesale forced-colors swap leaves no named tokens;
    // Highlight/HighlightText are the CSS ::selection system pair — the
    // resolve_token CurrentColor idiom, extended.
    use buiy_core::render::color::SystemColorKeyword;
    let t = forced_colors_theme();
    assert_eq!(
        resolve_selection_bg(&t),
        t.color(SystemColorKeyword::Highlight.token()).unwrap()
    );
    assert_eq!(
        resolve_selection_fg(&t),
        t.color(SystemColorKeyword::HighlightText.token()).unwrap()
    );
}

#[test]
fn caret_color_chain_explicit_then_theme_key_then_current() {
    let mut t = default_light_theme();
    let current = Color::srgb(0.1, 0.2, 0.3);

    // Tier 3: no explicit token, no theme key → currentColor.
    assert_eq!(resolve_caret_color(None, &t, current), current);

    // Tier 2: the theme caret key, when a theme opts in (presence check,
    // never a magenta miss).
    t.colors.insert(CARET_COLOR_TOKEN.into(), Color::srgb(0.9, 0.0, 0.0));
    assert_eq!(
        resolve_caret_color(None, &t, current),
        Color::srgb(0.9, 0.0, 0.0)
    );

    // Tier 1: an explicit CaretColor token wins over both.
    t.colors.insert("my.caret".into(), Color::srgb(0.0, 0.9, 0.0));
    let explicit = ColorToken::Token("my.caret".into());
    assert_eq!(
        resolve_caret_color(Some(&explicit), &t, current),
        Color::srgb(0.0, 0.9, 0.0)
    );
}

#[test]
fn caret_visual_defaults_visible_with_zero_rect() {
    // The § 6.3 shape verbatim; insert-visible matches "caret becomes
    // visible" on focus gain (editing § 10) and the t=0 blink phase.
    let cv = CaretVisual::default();
    assert!(cv.visible);
    assert_eq!(cv.rect, Rect::default());
}

#[test]
fn selection_visual_normalizes_on_construction() {
    // The selection_bounds() invariant (start ≤ end), enforced by the
    // ordered constructor so producer-side code never re-sorts.
    let a = Cursor::new(2, 5);
    let b = Cursor::new(1, 9);
    let sv = SelectionVisual::new(a, b);
    assert_eq!((sv.start.line, sv.start.index), (1, 9));
    assert_eq!((sv.end.line, sv.end.index), (2, 5));
    // Same line, indices swapped.
    let sv = SelectionVisual::new(Cursor::new(1, 7), Cursor::new(1, 3));
    assert_eq!((sv.start.index, sv.end.index), (3, 7));
    assert!(SelectionVisual::new(a, a).is_collapsed());
}

#[test]
fn text_color_placeholder_is_the_token_constructor() {
    let TextColor(token) = TextColor::placeholder();
    assert_eq!(token, ColorToken::Token(PLACEHOLDER_COLOR_TOKEN.into()));
}
```

Add the `CaretColor` row to `tests/render_components_registry.rs` (follow
the file's existing per-component registration assertion pattern — read it
first; it asserts `register_type` happened for every author-set render
component).

- [ ] **Step 2: Run the tests, confirm they fail** (the components and
  helpers do not exist → compile failure counts as RED for the new file;
  keep the existing suites green).

- [ ] **Step 3: Implement**

In `crates/buiy_core/src/text/components.rs` (new section after
`TextDecorations`; `cosmic_text::Cursor` joins the existing cosmic imports):

```rust
/// The caret's paint-input state (decoration-and-paint § 6.3 — the pinned
/// shape, verbatim): whether the caret currently paints, and its rect in
/// CONTENT-BOX-LOCAL logical px (`(caret_x, line_top)` → `(caret_x +
/// caret_w, line_top + line_height)`, § 6.1's terms). The producer folds
/// the entity origin and applies the § 3.3 physical-px snap to (x, width)
/// at emission — the rect here is unsnapped, scale-agnostic geometry.
///
/// WRITERS: `rect` is authored by the editing model (the successor
/// `buiy-text-editing` campaign; tests/examples until then — T7 paints
/// FROM state, it does not own editing); `visible` is managed by the
/// [`write_caret_blink`] render-prep writer (edge-only — § 6.3). Presence
/// = "an editor wants a caret here"; REMOVAL hides it (focus loss).
/// Machinery state — not reflect-registered (the ComputedTextLayout
/// convention).
///
/// [`write_caret_blink`]: super::visual::write_caret_blink
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct CaretVisual {
    /// Blink-phase visibility (square wave of the app clock; steady true
    /// under prefers-reduced-motion). The producer emits no stamp when
    /// false.
    pub visible: bool,
    /// Content-box-local caret rect, logical px, unsnapped.
    pub rect: bevy::math::Rect,
}

impl Default for CaretVisual {
    /// Visible (matches the t=0 blink phase and editing § 10's
    /// "caret becomes visible" on focus gain), zero rect.
    fn default() -> Self {
        Self {
            visible: true,
            rect: bevy::math::Rect::default(),
        }
    }
}

/// The selection's paint-input state (decoration-and-paint § 5.1: "the
/// endpoints reach the producer through the render-prep-written
/// `SelectionVisual` state"): the NORMALIZED endpoint pair — the
/// `Editor::selection_bounds() -> Option<(Cursor, Cursor)>` output shape
/// verbatim. The producer derives the § 5.1 rects per run via
/// `LayoutRun::highlight` and the § 5.2 re-tint per glyph from these same
/// endpoints (one source of truth; § 6.3's "rect list plus re-tint
/// ranges" phrasing is a T7 erratum — see the campaign plan).
///
/// Presence = "a selection exists"; REMOVAL clears it. A collapsed pair
/// (`start == end`) paints nothing (a collapsed selection is a caret).
/// v1 is single-range; the multi-range generalization is additive with
/// the editing campaign's `TextSelection` (editing-and-ime § 4.2).
/// Machinery state — not reflect-registered (carries `cosmic_text::Cursor`,
/// which is legal here: this module IS the cosmic boundary, the
/// `TextBuffer` precedent).
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct SelectionVisual {
    /// Logically-first endpoint (`start ≤ end` — the constructor enforces).
    pub start: Cursor,
    /// Logically-last endpoint.
    pub end: Cursor,
}

impl SelectionVisual {
    /// Build from an UNORDERED endpoint pair, normalizing to
    /// `start ≤ end` ((line, index) lexicographic — the
    /// `selection_bounds()` ordering).
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

In `crates/buiy_core/src/render/components.rs` (beside `TextColor`):

```rust
/// CSS `caret-color` (decoration-and-paint § 6.2; text.md:90–91, F): the
/// explicit tier-1 override of the caret tint. Resolution order, applied
/// by the glyph producer at extract (`resolve_caret_color`): this token →
/// the `color.caret` theme key when the active theme carries one
/// (presence-checked, never a magenta miss) → the entity's resolved
/// foreground (`caret-color: auto` — CSS parity; the default theme
/// deliberately ships NO `color.caret`). The value lands in the stamp's
/// per-instance color: changing it is a re-tint, never an atlas mutation.
#[derive(Component, Reflect, Clone, PartialEq, Debug)]
#[reflect(Component)]
pub struct CaretColor(pub ColorToken);
```

In `crates/buiy_core/src/render/color.rs` (after `resolve_token`):

```rust
/// `::selection` background token name (decoration-and-paint § 5.1; the
/// palette value is `buiy-theme-tokens-design`'s).
pub const SELECTION_BG_TOKEN: &str = "color.selection.bg";
/// `::selection` foreground (selected-text re-tint) token name (§ 5.2).
pub const SELECTION_FG_TOKEN: &str = "color.selection.fg";
/// The opt-in theme caret token (§ 6.2's middle tier). Deliberately NOT
/// in the default theme — `caret-color: auto` (= currentColor) parity.
pub const CARET_COLOR_TOKEN: &str = "color.caret";
/// `::placeholder` foreground token name (§ 7).
pub const PLACEHOLDER_COLOR_TOKEN: &str = "color.text.placeholder";

/// `::selection` background (§ 5.1): prefer the CSS `Highlight` system key
/// when the active theme carries it (the forced-colors case — the
/// wholesale swap leaves no named tokens), else the named token. The
/// `resolve_token` CurrentColor arm's prefer-when-present idiom, extended.
pub fn resolve_selection_bg(theme: &Theme) -> Color {
    if theme.color(SystemColorKeyword::Highlight.token()).is_some() {
        resolve_named(SystemColorKeyword::Highlight.token(), theme)
    } else {
        resolve_named(SELECTION_BG_TOKEN, theme)
    }
}

/// `::selection` foreground (§ 5.2): `HighlightText` under forced colors,
/// else the named token. See [`resolve_selection_bg`].
pub fn resolve_selection_fg(theme: &Theme) -> Color {
    if theme
        .color(SystemColorKeyword::HighlightText.token())
        .is_some()
    {
        resolve_named(SystemColorKeyword::HighlightText.token(), theme)
    } else {
        resolve_named(SELECTION_FG_TOKEN, theme)
    }
}

/// `caret-color` (§ 6.2): explicit token → the `color.caret` theme key if
/// present (presence check — an opt-in tier, not a magenta miss) →
/// `current` (the entity's resolved foreground = `caret-color: auto`).
pub fn resolve_caret_color(explicit: Option<&ColorToken>, theme: &Theme, current: Color) -> Color {
    if let Some(token) = explicit {
        return resolve_token(token, theme);
    }
    theme.color(CARET_COLOR_TOKEN).unwrap_or(current)
}
```

And on `TextColor` (render/components.rs):

```rust
impl TextColor {
    /// `::placeholder` styling (decoration-and-paint § 7): placeholder
    /// text is ordinary text whose foreground resolves to the placeholder
    /// token — same Buffer machinery, same producer, same decoration
    /// seats; the one difference is this tint. (A placeholder is never
    /// selectable — it simply carries no CaretVisual/SelectionVisual; the
    /// editing campaign owns the when-empty swap.)
    pub fn placeholder() -> Self {
        Self(ColorToken::Token(
            crate::render::color::PLACEHOLDER_COLOR_TOKEN.into(),
        ))
    }
}
```

In `crates/buiy_core/src/theme.rs`, `default_light_theme()` grows (values
are Phase-0 placeholders — the authoritative palette is
`buiy-theme-tokens-design`'s, same caveat as every existing entry):

```rust
    t.colors.insert(
        "color.selection.bg".into(),
        Color::srgb(0.20, 0.45, 0.95), // the accent blue — web-typical highlight
    );
    t.colors
        .insert("color.selection.fg".into(), Color::WHITE);
    t.colors.insert(
        "color.text.placeholder".into(),
        Color::srgb(0.55, 0.55, 0.55),
    );
    // NO "color.caret" entry — caret-color: auto parity (T7 decision 7).
```

Wiring: `text/mod.rs` exports `CaretVisual`, `SelectionVisual` from
`components`; `render/mod.rs` adds `.register_type::<components::CaretColor>()`
to the author-set block (`CaretVisual`/`SelectionVisual` are deliberately
not registered — machinery state). `render/mod.rs`'s color re-exports grow
the four consts + three helpers if the module re-exports individually
(match the existing `resolve_token` export style).

- [ ] **Step 4: Run the new tests — green.**
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(text): T7.1 — caret/selection paint-input components + tokens`.

---

### Task 2: The blink writer — `text/visual.rs` (headless)

The render-prep system: the square wave evaluated at the state write,
edge-only, reduced-motion steady. This is the subtle part of the phase —
the idempotency discipline IS the damage discipline.

**Files:**
- Create: `crates/buiy_core/src/text/visual.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/tests/text_caret_selection.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/buiy_core/tests/text_caret_selection.rs`:

```rust
use buiy_core::text::{CaretBlinkInterval, blink_phase};
use buiy_core::theme::UserPreferences;
use std::time::Duration;

// --- Task 2: blink_phase, the pure square wave ------------------------------

#[test]
fn blink_phase_is_a_square_wave_starting_visible() {
    let half = Duration::from_millis(500);
    assert!(blink_phase(Duration::ZERO, half), "t=0 visible");
    assert!(blink_phase(Duration::from_millis(499), half));
    assert!(!blink_phase(Duration::from_millis(500), half), "edge: hidden");
    assert!(!blink_phase(Duration::from_millis(999), half));
    assert!(blink_phase(Duration::from_millis(1000), half), "full period");
    // Zero interval = steady visible (defensive, documented).
    assert!(blink_phase(Duration::from_secs(7), Duration::ZERO));
}

// --- Task 2: the writer — edge-only, reduced-motion steady ------------------

/// Minimal headless app with the text plugin (the writer registers there)
/// and a change-tick probe counting `Changed<CaretVisual>` per frame.
fn blink_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.init_resource::<CaretEdges>();
    // After Picking ⇒ after the writer — sees the settled value + tick.
    app.add_systems(
        Update,
        count_caret_edges.after(buiy_core::BuiySet::Picking),
    );
    app
}

#[derive(Resource, Default)]
struct CaretEdges(usize);

fn count_caret_edges(q: Query<(), Changed<CaretVisual>>, mut edges: ResMut<CaretEdges>) {
    edges.0 += q.iter().count();
}

fn advance(app: &mut App, ms: u64) {
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(ms));
    app.update();
}

#[test]
fn blink_writes_only_on_phase_edges() {
    let mut app = blink_app();
    let e = app.world_mut().spawn(CaretVisual::default()).id();
    app.update(); // insertion frame: Added counts as one edge
    let after_spawn = app.world().resource::<CaretEdges>().0;
    assert!(after_spawn >= 1);

    // Mid-phase steps: the square wave does not flip → ZERO writes.
    advance(&mut app, 100);
    advance(&mut app, 100);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn);
    assert!(app.world().get::<CaretVisual>(e).unwrap().visible);

    // Cross the 500 ms edge → exactly one write, visible flips false.
    advance(&mut app, 400);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn + 1);
    assert!(!app.world().get::<CaretVisual>(e).unwrap().visible);

    // Steady in the hidden phase → zero writes again.
    advance(&mut app, 100);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn + 1);

    // The next edge flips back.
    advance(&mut app, 400);
    assert_eq!(app.world().resource::<CaretEdges>().0, after_spawn + 2);
    assert!(app.world().get::<CaretVisual>(e).unwrap().visible);
}

#[test]
fn reduced_motion_pins_steady_visible() {
    let mut app = blink_app();
    app.world_mut()
        .resource_mut::<UserPreferences>()
        .prefers_reduced_motion = true;
    let e = app.world_mut().spawn(CaretVisual::default()).id();
    app.update();
    let baseline = app.world().resource::<CaretEdges>().0;
    // Cross what would be three blink edges: no flips, no writes.
    for _ in 0..3 {
        advance(&mut app, 600);
        assert!(app.world().get::<CaretVisual>(e).unwrap().visible);
    }
    assert_eq!(app.world().resource::<CaretEdges>().0, baseline);
}

#[test]
fn reduced_motion_flip_during_hidden_phase_is_one_edge_to_visible() {
    let mut app = blink_app();
    let e = app.world_mut().spawn(CaretVisual::default()).id();
    app.update();
    advance(&mut app, 700); // into the hidden phase
    assert!(!app.world().get::<CaretVisual>(e).unwrap().visible);
    app.world_mut()
        .resource_mut::<UserPreferences>()
        .prefers_reduced_motion = true;
    app.update(); // the steady state takes over: one edge to true
    assert!(app.world().get::<CaretVisual>(e).unwrap().visible);
}
```

- [ ] **Step 2: Run — RED** (`visual.rs` does not exist).

- [ ] **Step 3: Implement**

Create `crates/buiy_core/src/text/visual.rs`:

```rust
//! Editor visual state, render-prep half (decoration-and-paint § 6.3):
//! the caret-blink writer. Caret visibility is a SQUARE-WAVE FUNCTION OF
//! THE APP CLOCK, evaluated here — in the `Animate→Picking` window the
//! other render-prep passes share (write_clip_rects / write_paint_skip) —
//! and written EDGE-ONLY: `Mut` marks the component changed only on
//! DerefMut, so a non-flipping frame issues zero ticks and the glyph
//! producer's § 6.2 union stays cold (the O(0) steady state; an
//! unconditional write would rebuild ExtractedGlyphs every frame).
//!
//! Reduced motion (text.md:90): the caret is STEADY — phase pinned true,
//! no blink. The blink PERIOD is a plugin resource (the Phase-0 Theme has
//! no motion scale; the token indirection is buiy-theme-tokens-design's
//! seam). Per-entity phase reset on edit/caret-move is the editing
//! campaign's `CaretBlink` state — this writer is deliberately global and
//! stateless (§ 6.3's literal "square-wave function of the app clock").

use std::time::Duration;

use bevy::math::{Rect, Vec2};
use bevy::prelude::*;

use crate::theme::UserPreferences;

use super::components::CaretVisual;
use super::decoration::{snap_thickness, snap_y};

/// The blink HALF-period (time spent in each phase; a full cycle is 2×).
/// Default 500 ms — the conventional desktop rate. Zero ⇒ steady visible.
/// Swap the resource to retheme; the theme-token indirection is
/// `buiy-theme-tokens-design`'s seam (§ 6.3: "a theme/animation value,
/// not pinned here").
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct CaretBlinkInterval(pub Duration);

impl Default for CaretBlinkInterval {
    fn default() -> Self {
        Self(Duration::from_millis(500))
    }
}

/// The square wave: even half-periods are visible (t=0 ⇒ visible — a
/// fresh caret shows immediately). Integer micros, no float drift. A zero
/// interval is steady visible (defensive — a misconfigured resource must
/// not divide by zero or strobe).
pub fn blink_phase(elapsed: Duration, half_period: Duration) -> bool {
    if half_period.is_zero() {
        return true;
    }
    // ERRATUM (T2): this snippet's `as_micros()` truncates a 1–999 ns
    // half-period to 0 — `is_zero()` is nanosecond-exact, so the guard
    // does not cover the division and `Duration::from_nanos(500)` panics
    // with divide-by-zero, violating the doc contract above. Implemented
    // as `(elapsed.as_nanos() / half_period.as_nanos()).is_multiple_of(2)`
    // (clippy also prefers `is_multiple_of` over `% 2 == 0`).
    (elapsed.as_micros() / half_period.as_micros()) % 2 == 0
}

/// Render-prep (main world, `Update`,
/// `.after(BuiySet::Animate).before(BuiySet::Picking)` — the
/// write_clip_rects window): drive every `CaretVisual.visible` from the
/// blink phase, edge-only. `UserPreferences` is `Option` so the plugin
/// stays self-sufficient without `ThemePlugin` (the
/// apply_forced_colors_theme precedent).
pub fn write_caret_blink(
    time: Res<Time>,
    prefs: Option<Res<UserPreferences>>,
    interval: Res<CaretBlinkInterval>,
    mut carets: Query<&mut CaretVisual>,
) {
    let steady = prefs.is_some_and(|p| p.prefers_reduced_motion);
    let phase = steady || blink_phase(time.elapsed(), interval.0);
    for mut caret in &mut carets {
        // Edge-only: DerefMut (and the change tick) ONLY on a flip.
        if caret.visible != phase {
            caret.visible = phase;
        }
    }
}

/// The caret stamp rect (§ 6.1 + § 3.3), pure: fold the entity origin,
/// snap x to the physical grid and floor the width to whole physical px
/// (min 1) — the decoration snap rule rotated 90°: for a horizontal rule
/// the THIN axis is y/height; for the caret bar it is x/width. y/height
/// stay unsnapped (the caret spans the full line box — not a hairline
/// dimension). `snap_y` is an axis-agnostic scalar grid snap (named for
/// its T6 underline call site).
pub fn caret_stamp_rect(origin: Vec2, rect: Rect, scale_factor: f32) -> [f32; 4] {
    [
        snap_y(origin.x + rect.min.x, scale_factor),
        origin.y + rect.min.y,
        snap_thickness(rect.width(), scale_factor),
        rect.height(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// § 3.3 applied to the caret (the snap_thickness/snap_y math is
    /// pinned by T6's tests; these pin the composition + axis choice).
    #[test]
    fn caret_rect_snaps_x_and_floors_width() {
        let r = Rect::new(12.3, 0.0, 13.3, 19.2); // 1 px wide, unsnapped x
        // scale 1.0: x 22.3 → 22.0; w 1.0 → 1.0; y/h untouched.
        assert_eq!(
            caret_stamp_rect(Vec2::new(10.0, 20.0), r, 1.0),
            [22.0, 20.0, 1.0, 19.2]
        );
        // scale 1.5: x 22.3 → 33.45 phys → 33 → 22.0; w 1.0 → 1.5 phys →
        // round 2 phys → 4/3 logical (the § 3.3 pin: never 1.5 phys px).
        let [x, y, w, h] = caret_stamp_rect(Vec2::new(10.0, 20.0), r, 1.5);
        assert_eq!([x, y, h], [33.0 / 1.5, 20.0, 19.2]);
        assert_eq!(w, 2.0 / 1.5);
    }
}
```

In `crates/buiy_core/src/text/mod.rs`: `mod visual;` +
`pub use visual::{CaretBlinkInterval, blink_phase, caret_stamp_rect, write_caret_blink};`
and in `BuiyTextPlugin::build` (after the TextSync/TextCommit block):

```rust
        // T7 (decoration-and-paint § 6.3): the caret-blink render-prep
        // writer — the same Animate→Picking window as write_clip_rects /
        // write_paint_skip, so extract reads a settled CaretVisual.
        // Main-world, headless-safe (no RenderApp dependency).
        app.init_resource::<CaretBlinkInterval>();
        app.add_systems(
            Update,
            visual::write_caret_blink
                .after(crate::BuiySet::Animate)
                .before(crate::BuiySet::Picking),
        );
```

- [ ] **Step 4: Run the new tests — green.**
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(text): T7.2 — the caret-blink render-prep writer (edge-only)`.

---

### Task 3: Selection emission — rects at seat 2 + per-cluster re-tint (headless)

The producer half of § 5: the union grows `Changed<SelectionVisual>` (+ its
removal stream), the per-entity selection pre-pass emits `highlight()`-driven
quads, and the glyph loop re-tints selected clusters. Includes the
mixed-BiDi multi-rect contract as resolved numbers.

**Files:**
- Modify: `crates/buiy_core/src/text/extract.rs`
- Modify: `crates/buiy_core/tests/text_caret_selection.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/buiy_core/tests/text_caret_selection.rs` (these drive the
`TextExtractHarness`; selection rects are identified in
`ExtractedTextQuads` by color — insert a chroma-orthogonal value for
`color.selection.bg` into the harness theme, the T6 GPU-token idiom):

```rust
use buiy_core::render::extract::TextQuad;
use buiy_core::text::{ComputedTextLayout, FontSize, Text, TextBuffer, TextDecorations};
use support::extract_harness::TextExtractHarness;

const SEL_BG: Color = Color::srgb(1.0, 0.0, 0.0);
const SEL_FG: Color = Color::srgb(0.0, 0.0, 1.0);

fn set_selection_tokens(app: &mut App) {
    let mut theme = app.world_mut().resource_mut::<Theme>();
    theme.colors.insert(SELECTION_BG_TOKEN.into(), SEL_BG);
    theme.colors.insert(SELECTION_FG_TOKEN.into(), SEL_FG);
}

fn selection_quads(h: &TextExtractHarness) -> Vec<TextQuad> {
    h.text_quads()
        .quads
        .iter()
        .copied()
        .filter(|q| q.color == SEL_BG)
        .collect()
}

// --- Task 3: rect derivation — exact numbers against the same buffer -------

#[test]
fn selection_rects_match_highlight_spans_folded_by_origin() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi there".into()),
        SelectionVisual::new(Cursor::new(0, 1), Cursor::new(0, 5)),
    )).id();
    h.settle();

    // Expected: the SAME buffer, the SAME upstream API — proves the
    // producer's gating, origin fold, and seat plumbing (the span MATH is
    // upstream's; the line gate + fold are ours).
    let world = h.app.world();
    let buffer = world.get::<TextBuffer>(e).unwrap();
    let computed = world.get::<ComputedTextLayout>(e).unwrap();
    let origin = world.get::<GlobalTransform>(e).unwrap().translation().truncate()
        + computed.content_offset;
    let sel = world.get::<SelectionVisual>(e).unwrap();
    let mut expected = Vec::new();
    for run in buffer.buffer.layout_runs() {
        if run.line_i < sel.start.line || run.line_i > sel.end.line {
            continue;
        }
        for (x, w) in run.highlight(sel.start, sel.end) {
            expected.push((
                Vec2::new(origin.x + x, origin.y + run.line_top),
                Vec2::new(w, run.line_height),
            ));
        }
    }
    assert!(!expected.is_empty(), "the fixture selects something");
    let got: Vec<_> = selection_quads(&h)
        .iter()
        .map(|q| (q.position, q.size))
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn mixed_bidi_selection_yields_multiple_disjoint_rects() {
    // THE campaign contract (text.md:89): a logical range straddling the
    // BiDi boundary maps to ≥ 2 visually disjoint rects — upstream's
    // highlight() does the math, the producer must not flatten it.
    // "hello עולם world": bytes 0..6 "hello ", 6..14 עולם, 14.. " world".
    // Select mid-Hebrew → mid-"world": logical [10, 18).
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    support::register_fixture_font(&mut h.app, "Noto Sans Hebrew", "NotoSansHebrew-hebrew.ttf");
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default().width_px(400.0).height_px(100.0),
        Text("hello עולם world".into()),
        buiy_core::text::FontFamily(buiy_core::text::FontStack(vec![
            buiy_core::text::FamilyEntry::Named("Fira Sans".into()),
            buiy_core::text::FamilyEntry::Named("Noto Sans Hebrew".into()),
        ])),
        FontSize(20.0),
        SelectionVisual::new(Cursor::new(0, 10), Cursor::new(0, 18)),
    ));
    h.settle();

    let mut quads = selection_quads(&h);
    assert!(
        quads.len() >= 2,
        "mixed-BiDi selection must paint disjoint rects, got {}",
        quads.len()
    );
    // All on one line band, sorted by x, strictly disjoint with a gap.
    quads.sort_by(|a, b| a.position.x.total_cmp(&b.position.x));
    for pair in quads.windows(2) {
        assert_eq!(pair[0].position.y, pair[1].position.y, "one line");
        assert!(
            pair[0].position.x + pair[0].size.x < pair[1].position.x,
            "visually disjoint (the unselected Hebrew remainder sits between)"
        );
    }
}

// --- Task 3: the seat + the reference-render behaviors ----------------------

#[test]
fn selection_rects_precede_decoration_quads_for_the_entity() {
    // § 4.4 seat order: 2 (selection) < 3 (underline) — for the WHOLE
    // entity (decision 5's pre-pass), so the carrier segment is
    // [sel…, deco…].
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi there".into()),
        TextDecorations {
            line: buiy_core::text::DecorationLines::UNDERLINE,
            ..Default::default()
        },
        SelectionVisual::new(Cursor::new(0, 0), Cursor::new(0, 8)),
    ));
    h.settle();
    let quads = &h.text_quads().quads;
    let first_deco = quads.iter().position(|q| q.color != SEL_BG).expect("underline");
    let last_sel = quads.iter().rposition(|q| q.color == SEL_BG).expect("selection");
    assert!(last_sel < first_deco, "all selection quads before all decoration quads");
}

#[test]
fn multiline_selection_extends_to_the_line_edge_and_fills_empty_lines() {
    // Upstream's reference behaviors (Orientation § 2): non-final selected
    // lines extend their last rect to the buffer width (LTR); a fully
    // selected INTERNAL empty line paints one full-width rect.
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default().width_px(200.0).height_px(120.0),
        // Pre-line keeps the \n\n: line 0 "ab", line 1 "" (internal), line 2 "cd".
        Text("ab\n\ncd".into()),
        buiy_core::text::WhiteSpace::PreLine,
        SelectionVisual::new(Cursor::new(0, 1), Cursor::new(2, 1)),
    )).id();
    h.settle();

    let world = h.app.world();
    let buffer = world.get::<TextBuffer>(e).unwrap();
    let width = buffer.buffer.size().0.expect("committed width");
    let computed = world.get::<ComputedTextLayout>(e).unwrap();
    let origin = world.get::<GlobalTransform>(e).unwrap().translation().truncate()
        + computed.content_offset;
    let quads = selection_quads(&h);
    assert_eq!(quads.len(), 3, "one rect per line");
    // Line 0 (non-final): right edge == origin.x + buffer width.
    assert_eq!(quads[0].position.x + quads[0].size.x, origin.x + width);
    // Line 1 (internal empty): full width from x = origin.x.
    assert_eq!(quads[1].position.x, origin.x);
    assert_eq!(quads[1].size.x, width);
    // Line 2 (final): ends at the grapheme edge, NOT the line edge.
    assert!(quads[2].position.x + quads[2].size.x < origin.x + width);
}

#[test]
fn collapsed_selection_paints_nothing() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi".into()),
        SelectionVisual::new(Cursor::new(0, 1), Cursor::new(0, 1)),
    ));
    h.settle();
    assert!(selection_quads(&h).is_empty());
}

// --- Task 3: per-cluster re-tint --------------------------------------------

#[test]
fn selected_glyphs_re_tint_to_the_selection_fg() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi there".into()),
        // Select "i th" → bytes [1, 5): glyphs for 'i','t','h' re-tint
        // (the space emits no instance).
        SelectionVisual::new(Cursor::new(0, 1), Cursor::new(0, 5)),
    )).id();
    h.settle();

    let fg = LinearRgba::from(SEL_FG);
    let fg = [fg.red, fg.green, fg.blue, fg.alpha];
    let tinted: Vec<bool> = h.glyphs().glyphs.iter().map(|g| g.color == fg).collect();
    // 7 painted glyphs ("Hithere"): H i t h e r e → i,t,h selected.
    assert_eq!(tinted, [false, true, true, true, false, false, false]);
    // The selection state never touches the atlas (re-tint is per-instance):
    // every uv in the rebuilt set is identical to a no-selection rebuild —
    // cheap proxy: remove the selection, re-settle, compare uv sequences.
    let uvs_selected: Vec<[f32; 4]> = h.glyphs().glyphs.iter().map(|g| g.uv).collect();
    h.app.world_mut().entity_mut(e).remove::<SelectionVisual>();
    h.frame();
    let uvs_plain: Vec<[f32; 4]> = h.glyphs().glyphs.iter().map(|g| g.uv).collect();
    assert_eq!(uvs_selected, uvs_plain, "the atlas/uv set is selection-invariant");
}

// --- Task 3: damage --------------------------------------------------------

#[test]
fn selection_changes_fire_the_union_and_removal_clears() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    let e = h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi there".into()),
    )).id();
    h.settle();
    let g0 = h.changed_frames();

    // Steady: no rebuild.
    h.frame();
    assert_eq!(h.changed_frames(), g0);

    // Insert → rebuild (Changed includes Added).
    h.app.world_mut().entity_mut(e)
        .insert(SelectionVisual::new(Cursor::new(0, 0), Cursor::new(0, 2)));
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 1);
    assert!(!selection_quads(&h).is_empty());

    // Mutate endpoints → rebuild.
    h.app.world_mut().get_mut::<SelectionVisual>(e).unwrap().end = Cursor::new(0, 5);
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 2);

    // Remove → the removal stream fires a rebuild that clears the rects.
    h.app.world_mut().entity_mut(e).remove::<SelectionVisual>();
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 3);
    assert!(selection_quads(&h).is_empty());
    // And back to steady.
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 3);
}
```

- [ ] **Step 2: Run — RED.**

- [ ] **Step 3: Implement** (`crates/buiy_core/src/text/extract.rs`)

1. **Fan growth** — the `texts` query gains (13-item tuple; Bevy's limit is
   15 — note in the comment that the next grower must nest):

```rust
                // T7: the editor visual paint-inputs (decoration-and-paint
                // §§ 5–6) + the explicit caret-color tier.
                Option<&CaretVisual>,
                Option<&SelectionVisual>,
                Option<&CaretColor>,
```

2. **Union growth** — the `changed` `Or` gains (and the ledger comment's
   "join later" line flips to "joined in T7"):

```rust
                    // T7 (decoration-and-paint § 6.3): the render-prep-
                    // written editor visual state — a caret-blink edge or
                    // a selection endpoint change re-emits; steady phases
                    // rebuild nothing.
                    Changed<CaretVisual>,
                    Changed<SelectionVisual>,
                    Changed<CaretColor>,
```

   and the `removed` tuple grows two streams (caret hide on focus loss /
   selection clear are REMOVALS — unlike the style carriers, removal here
   is the hide mechanism, so the T2-erratum-1 exclusion does not apply;
   say so in the comment):

```rust
        Extract<RemovedComponents<CaretVisual>>,
        Extract<RemovedComponents<SelectionVisual>>,
```

   drained at the top with the others
   (`let caret_removed = removed.3.read().count() > 0;` etc.) and OR'd
   into `dirty`.

3. **The selection pre-pass** (decision 5) — inside the entity loop, after
   `deco_override` and **before** the run walk:

```rust
        // T7 § 5.1: the selection pre-pass — seat 2 quads for the WHOLE
        // entity before any seat-3 decoration quad (§ 4.4; a per-run
        // interleave could paint an underline under the next line's
        // selection where line boxes touch). Iteration only — no locks.
        // Collapsed selections paint nothing (a collapsed selection is a
        // caret; skipping also removes upstream's mid-grapheme re-tint
        // edge). Paints normally under Block (chrome, not ink —
        // decision 9).
        let selection = selection_visual.filter(|s| !s.is_collapsed());
        let mut selection_fg: Option<[f32; 4]> = None;
        if let Some(sel) = selection {
            let bg = resolve_selection_bg(theme);
            selection_fg = Some(linear_color(resolve_selection_fg(theme)));
            if bg.alpha() > 0.0 {
                // Upstream's reference width source for the line-edge
                // extension + empty-line fill; commit guarantees Some
                // (T3 decision 9) — computed.size.x is defense, not a
                // path (upstream's unwrap_or(0.0) would drop the
                // extension silently).
                let full_w = buffer.buffer.size().0.unwrap_or(computed.size.x);
                for run in buffer.buffer.layout_runs() {
                    // THE caller contract (Orientation § 1): highlight's
                    // predicate degenerates to all-selected outside
                    // [start.line, end.line] — gate first, like upstream.
                    if run.line_i < sel.start.line || run.line_i > sel.end.line {
                        continue;
                    }
                    let spans: SmallVec<[(f32, f32); 4]> =
                        run.highlight(sel.start, sel.end).collect();
                    if spans.is_empty() && run.glyphs.is_empty() && sel.end.line > run.line_i {
                        // Internal fully-selected empty line: full width.
                        push_selection_quad(
                            &mut new_quads, entity, origin, 0.0, full_w, &run, bg, eff_clip,
                        );
                    } else {
                        let len = spans.len();
                        for (idx, (x, w)) in spans.into_iter().enumerate() {
                            let (mut min_x, mut max_x) = (x, x + w);
                            // Non-final selected line: the last rect
                            // extends to the line edge (newline made
                            // visible) — RTL-aware, upstream verbatim.
                            if idx + 1 == len && sel.end.line > run.line_i {
                                if run.rtl {
                                    min_x = 0.0;
                                } else {
                                    max_x = full_w;
                                }
                            }
                            push_selection_quad(
                                &mut new_quads, entity, origin, min_x, max_x - min_x, &run,
                                bg, eff_clip,
                            );
                        }
                    }
                }
            }
        }
```

   with the small free function (beside `linear_color`):

```rust
/// One § 5.1 selection rect: a highlight span unioned with the run's
/// (line_top, line_height), origin-folded, at quad seat 2. No § 3.3 snap:
/// selection rects are tall boxes, not hairlines (the snap rule exists
/// for sub-pixel-thin analytics; box-edge AA here matches node
/// backgrounds).
#[allow(clippy::too_many_arguments)]
fn push_selection_quad(
    quads: &mut Vec<TextQuad>,
    entity: Entity,
    origin: Vec2,
    x: f32,
    w: f32,
    run: &cosmic_text::LayoutRun,
    color: Color,
    clip: Option<ClipRect>,
) {
    if w <= 0.0 {
        return;
    }
    quads.push(TextQuad {
        entity,
        position: Vec2::new(origin.x + x, origin.y + run.line_top),
        size: Vec2::new(w, run.line_height),
        color,
        clip,
    });
}
```

4. **Re-tint** — in the glyph loop, the color pick becomes (Orientation
   § 4's predicate; over `color_opt` like upstream; Block's zero-alpha arm
   unchanged and applied after):

```rust
                // T7 § 5.2: per-CLUSTER re-tint — a glyph whose bytes
                // intersect the selection paints with the selection fg
                // (over any rich-text span color, upstream-verbatim); the
                // atlas is never touched. Granularity is the cluster: a
                // partially selected ligature re-tints whole while its
                // RECT stays grapheme-accurate (§ 5.2's accepted
                // tradeoff). Upstream's text_color != selected_text_color
                // short-circuit is dropped — equal resolved colors emit
                // identical instances.
                let selected = selection.is_some_and(|sel| {
                    run.line_i >= sel.start.line
                        && run.line_i <= sel.end.line
                        && (sel.start.line != run.line_i || glyph.end > sel.start.index)
                        && (sel.end.line != run.line_i || glyph.start < sel.end.index)
                });
                let mut color = match (selected, selection_fg) {
                    (true, Some(fg)) => fg,
                    _ => glyph.color_opt.map(span_color).unwrap_or(entity_color),
                };
```

   (`selection`/`selection_fg` are hoisted before the run walk so the
   pre-pass and the glyph loop read one resolution.)

5. Destructure the three new query items in the entity loop; thread
   `caret_visual`/`caret_color` through unused-for-now (Task 4 consumes
   them) or add them in Task 4 — keep this task selection-only if the
   compiler allows (it does: add only `Option<&SelectionVisual>` here and
   the other two in Task 4, keeping each diff minimal and RED/GREEN
   honest).

- [ ] **Step 4: Run the new tests — green.** Also re-run
  `text_touch_pass.rs` (the seam contract: `TextQuad` still carries no
  cosmic type — `SelectionVisual` stays main-world) and `text_extract.rs`
  (the existing retention suite must be unperturbed).
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(text): T7.3 — selection rects at seat 2 + per-cluster re-tint`.

---

### Task 4: Caret emission + value-compared publish + the blink damage contract (headless)

The producer half of § 6: the caret stamp at seat 6 with the § 3.3 snap and
the `caret-color` chain — plus the publication refinement that makes the
§ 6.3 damage property true: **a blink edge rebuilds `ExtractedGlyphs` and
leaves `ExtractedTextQuads` untouched; a steady phase rebuilds nothing.**

**Files:**
- Modify: `crates/buiy_core/src/text/extract.rs`
- Modify: `crates/buiy_core/src/render/atlas/primitive.rs`
- Modify: `crates/buiy_core/tests/text_caret_selection.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/buiy_core/tests/text_caret_selection.rs`:

```rust
use buiy_core::render::atlas::GlyphAlphaInstance;
use buiy_core::text::{caret_stamp_rect, solid_stamp_key};

/// The caret instance: a stamp (uv min == max — the midpoint-replicated
/// uv_rect) emitted LAST for the entity (seat 6).
fn caret_instance(h: &TextExtractHarness) -> Option<GlyphAlphaInstance> {
    h.glyphs()
        .glyphs
        .last()
        .copied()
        .filter(|g| g.uv[0] == g.uv[2] && g.uv[1] == g.uv[3])
}

// --- Task 4: geometry, color chain, seat ------------------------------------

#[test]
fn caret_emits_one_snapped_stamp_after_all_glyphs() {
    let mut h = TextExtractHarness::new();
    let e = h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi".into()),
        CaretVisual {
            visible: true,
            rect: Rect::new(12.3, 0.0, 13.3, 19.2),
        },
    )).id();
    h.settle();

    let world = h.app.world();
    let computed = world.get::<ComputedTextLayout>(e).unwrap();
    let origin = world.get::<GlobalTransform>(e).unwrap().translation().truncate()
        + computed.content_offset;
    let caret = caret_instance(&h).expect("a caret stamp");
    // § 6.1 + § 3.3 via the pure helper (its own snap math is unit-pinned
    // in visual.rs) — this asserts the producer composes it correctly.
    assert_eq!(
        caret.rect,
        caret_stamp_rect(origin, Rect::new(12.3, 0.0, 13.3, 19.2), 1.0)
    );
    // Seat 6: the caret is the LAST instance for the entity.
    assert_eq!(
        h.glyphs().glyphs.iter().position(|g| g.rect == caret.rect),
        Some(h.glyph_count() - 1)
    );
    // § 6.3 residency: the stamp key joined the touch-pass set.
    assert!(h.resident_keys().contains(&solid_stamp_key()));
    // caret-color: auto ⇒ the entity's resolved foreground
    // (color.text.primary in the default theme).
    let auto = LinearRgba::from(
        h.app.world().resource::<Theme>().color("color.text.primary").unwrap(),
    );
    assert_eq!(caret.color, [auto.red, auto.green, auto.blue, auto.alpha]);
}

#[test]
fn caret_color_chain_resolves_at_emission() {
    let mut h = TextExtractHarness::new();
    let e = h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi".into()),
        CaretVisual { visible: true, rect: Rect::new(0.0, 0.0, 1.0, 19.2) },
    )).id();
    h.settle();

    // Tier 2: a theme that opts into color.caret re-tints on theme change
    // (theme.is_changed() is already in the union).
    h.app.world_mut().resource_mut::<Theme>().colors
        .insert(CARET_COLOR_TOKEN.into(), Color::srgb(0.9, 0.0, 0.0));
    h.frame();
    let red = LinearRgba::from(Color::srgb(0.9, 0.0, 0.0));
    assert_eq!(caret_instance(&h).unwrap().color[0], red.red);

    // Tier 1: an explicit CaretColor wins.
    h.app.world_mut().resource_mut::<Theme>().colors
        .insert("my.caret".into(), Color::srgb(0.0, 0.9, 0.0));
    h.app.world_mut().entity_mut(e)
        .insert(CaretColor(ColorToken::Token("my.caret".into())));
    h.frame();
    let green = LinearRgba::from(Color::srgb(0.0, 0.9, 0.0));
    assert_eq!(caret_instance(&h).unwrap().color[1], green.green);
}

#[test]
fn invisible_or_removed_caret_emits_nothing() {
    let mut h = TextExtractHarness::new();
    let e = h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi".into()),
        CaretVisual { visible: false, rect: Rect::new(0.0, 0.0, 1.0, 19.2) },
    )).id();
    h.settle();
    assert!(caret_instance(&h).is_none());

    // Flip visible on → the Changed member fires, the stamp appears.
    h.app.world_mut().get_mut::<CaretVisual>(e).unwrap().visible = true;
    h.frame();
    assert!(caret_instance(&h).is_some());

    // REMOVAL hides (focus loss) — the removal stream fires the rebuild.
    h.app.world_mut().entity_mut(e).remove::<CaretVisual>();
    h.frame();
    assert!(caret_instance(&h).is_none());
}

#[test]
fn empty_text_still_carries_a_caret() {
    // Text("") has a synthetic glyph-less run (T3 erratum 5) and no glyph
    // instances — the caret is rect-authored, so it paints regardless.
    let mut h = TextExtractHarness::new();
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text(String::new()),
        CaretVisual { visible: true, rect: Rect::new(0.0, 0.0, 1.0, 19.2) },
    ));
    h.settle();
    assert_eq!(h.glyph_count(), 1, "exactly the caret stamp");
    assert!(caret_instance(&h).is_some());
}

// --- Task 4: THE damage contract (§ 6.3 / verification § 1.2) ---------------

#[test]
fn blink_edges_rebuild_glyphs_only_and_steady_phases_rebuild_nothing() {
    let mut h = TextExtractHarness::new();
    set_selection_tokens(&mut h.app);
    // A fixture with BOTH carriers live: an underline (quad tier) + a
    // caret (glyph tier) — so the test can see one move without the other.
    h.app.world_mut().spawn((
        buiy_core::Node,
        buiy_core::layout::Style::default(),
        Text("Hi there".into()),
        TextDecorations {
            line: buiy_core::text::DecorationLines::UNDERLINE,
            ..Default::default()
        },
        CaretVisual { visible: true, rect: Rect::new(2.0, 0.0, 3.0, 19.2) },
    ));
    h.settle();
    let g0 = h.changed_frames();
    let q0 = h.quad_changed_frames();
    let quads_before = h.text_quads().quads.clone();

    // "No blink edge → zero producer reruns": mid-phase virtual-clock
    // steps flip nothing, the writer writes nothing, the union stays
    // cold, NEITHER carrier is touched.
    for _ in 0..3 {
        h.app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(Duration::from_millis(100));
        h.frame();
    }
    assert_eq!(h.changed_frames(), g0, "no edge → no glyph rebuild");
    assert_eq!(h.quad_changed_frames(), q0, "no edge → no quad rebuild");

    // Cross the edge: the writer flips visible → Changed<CaretVisual>
    // fires → ONE glyph rebuild (the caret stamp drops out)… and the
    // value-compared publish leaves the quad carrier UNTOUCHED (its
    // content is identical) — the § 6.3 damage property, CPU half.
    h.app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(300));
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 1, "blink edge → glyph rebuild");
    assert_eq!(h.quad_changed_frames(), q0, "blink edge → quads RETAINED");
    assert_eq!(h.text_quads().quads, quads_before, "quad content identical");
    assert!(caret_instance(&h).is_none(), "hidden phase: no stamp");

    // And back: the next edge re-emits the stamp, quads still retained.
    h.app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(500));
    h.frame();
    assert_eq!(h.changed_frames(), g0 + 2);
    assert_eq!(h.quad_changed_frames(), q0);
    assert!(caret_instance(&h).is_some());
}
```

- [ ] **Step 2: Run — RED.**

- [ ] **Step 3: Implement**

In `crates/buiy_core/src/render/atlas/primitive.rs`: add `PartialEq` to
`GlyphAlphaInstance`'s derive (`#[derive(Clone, Copy, PartialEq, Pod,
Zeroable)]` — plain `[f32; 4]`s + `u32`; bit-equality is what decision 4
wants: identical rebuilds produce bit-identical instances). Document on the
type: "PartialEq backs the producer's value-compared publish
(decoration-and-paint § 6.3 damage)".

In `crates/buiy_core/src/text/extract.rs`:

1. Destructure `caret_visual`/`caret_color` (added to the fan in Task 3's
   step if not already; otherwise grow the query here).

2. **Caret emission** — after the entity's run walk (after the
   `debug_assert_eq!(runs, …)` is fine; seat 6 = after every glyph and
   line-through stamp of the entity):

```rust
        // T7 § 6.1 — seat 6: the caret paints last, over glyphs and
        // line-through, as a solid-stamp instance (pre-phase decision 2).
        // Painted under Block too (chrome, not ink — decision 9: browsers
        // keep the caret in a focused field whose font is loading).
        if let Some(cv) = caret_visual
            && cv.visible
        {
            // § 6.2: explicit token → theme caret key (presence check) →
            // currentColor. Re-tint only — never an atlas mutation.
            let color = resolve_caret_color(
                caret_color.map(|c| &c.0),
                theme,
                resolved_entity_color,
            );
            if color.alpha() > 0.0 {
                let entry = *stamp_entry.get_or_insert_with(|| {
                    atlas.get_or_insert(
                        solid_stamp_key(),
                        AtlasFormat::CoverageR8,
                        solid_stamp_bitmap,
                    )
                });
                if entry.page > 0 {
                    warn_once_page_overflow();
                }
                new_glyphs.push(GlyphAlphaInstance {
                    rect: caret_stamp_rect(origin, cv.rect, scale_factor),
                    uv: stamp_uv(&entry),
                    color: linear_color(color),
                    clip,
                    page: entry.page as u32,
                });
                // § 6.3: the stamp key joins the un-gated touch pass —
                // a retained caret idling past eviction_grace must not
                // lose its cell.
                new_keys.push(solid_stamp_key());
            }
        }
```

3. **The value-compared publish** (decision 4) — the publication block
   becomes:

```rust
    // Publish — wholesale REBUILD under the one § 6.2 damage decision
    // (unchanged), VALUE-COMPARED publication per carrier (T7, refining
    // T6 decision 12): a blink edge changes only the glyph content, so
    // the quad carrier keeps its tick and the GPU quad buffer is retained
    // (decoration-and-paint § 6.3's damage property; prepare gates each
    // buffer independently). Equal inputs produce bit-identical f32
    // outputs, so derive-PartialEq equality is deterministic. One
    // O(instances) compare per DIRTY frame — steady frames return above.
    if glyphs.glyphs != new_glyphs {
        glyphs.glyphs = new_glyphs;
    }
    if text_quads.quads != new_quads {
        text_quads.quads = new_quads;
    }
    resident.keys = new_keys;
```

   (the window-vanished early-return clear above already guards on
   non-empty, so it stays as is). Update the module/system doc comments:
   the § 6.2 ledger's "join later (T7)" line flips to "joined in T7", and
   the publish comment cites this plan's decision 4.

- [ ] **Step 4: Run the new tests — green.** Re-run `text_extract.rs` in
  full — the value-compared publish must not regress any existing
  retention/rebuild count (rebuild frames whose content genuinely changed
  still mark the carriers; only content-identical republishes go quiet —
  if an existing test asserted a changed-frame for a content-identical
  rebuild, fix the TEST's expectation and note it in the commit body: the
  new behavior is the spec-pinned one).
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(text): T7.4 — caret stamp at seat 6 + value-compared carrier publish`.

---

### Task 5: `::placeholder` — the same-pipeline contract (headless)

§ 7 verbatim: a placeholder is text with a different tint. The token and
constructor landed in Task 1; this task pins the "exactly one difference"
contract end-to-end so the editing campaign can consume it blind.

**Files:**
- Modify: `crates/buiy_core/tests/text_caret_selection.rs`

- [ ] **Step 1: Write the failing test** (it may pass immediately if
  Tasks 1–4 are correct — that is the point: § 7 demands NO new machinery;
  the test is the contract, RED only against a broken token/constructor):

```rust
// --- Task 5: ::placeholder — same pipeline, one tint ------------------------

#[test]
fn placeholder_is_identical_to_normal_text_except_color() {
    // § 7: same Buffer machinery, same producer, same seats — the ONLY
    // difference is the foreground token. Two identical fixtures, one
    // with TextColor::placeholder(): instance streams must differ in
    // `color` alone (rect/uv/clip/page identical), proving no second
    // paint path exists to keep correct.
    let spawn = |h: &mut TextExtractHarness, color: TextColor| {
        h.app.world_mut().spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Search…".into()),
            color,
        )).id()
    };

    let mut normal = TextExtractHarness::new();
    spawn(&mut normal, TextColor::default());
    normal.settle();

    let mut placeholder = TextExtractHarness::new();
    spawn(&mut placeholder, TextColor::placeholder());
    placeholder.settle();

    let a = &normal.glyphs().glyphs;
    let b = &placeholder.glyphs().glyphs;
    assert_eq!(a.len(), b.len());
    let expected = {
        let theme = placeholder.app.world().resource::<Theme>();
        let lin = LinearRgba::from(theme.color(PLACEHOLDER_COLOR_TOKEN).unwrap());
        [lin.red, lin.green, lin.blue, lin.alpha]
    };
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.rect, y.rect, "same geometry");
        assert_eq!(x.uv, y.uv, "same atlas cells — the atlas is tint-blind");
        assert_eq!(x.clip, y.clip);
        assert_eq!(x.page, y.page);
        assert_eq!(y.color, expected, "the one difference: the tint");
    }
}
```

- [ ] **Step 2: Run** — if RED, the Task-1 token/constructor or the
  pipeline is broken; fix THERE (this task adds no production code by
  design — § 7's rejected runner-up is exactly "a dedicated placeholder
  paint path").
- [ ] **Step 3: Run BOTH gate lanes — green.** Commit:
  `test(text): T7.5 — the ::placeholder same-pipeline contract`.

---

### Task 6: GPU lane — the mixed-BiDi `::selection` golden + the caret-blink fixed-clock pair (`#[ignore]`)

Pixels: the campaign's exact T7 GPU surface. Both tests build on
`tests/support/mod.rs` and the band/column-classification idiom
(re-capture IS the golden — the text_decoration_gpu.rs discipline).

**Files:**
- Create: `crates/buiy_core/tests/text_selection_caret_gpu.rs`

- [ ] **Step 1: Write the tests** (all
  `#[ignore = "needs a wgpu adapter; run on the GPU lane"]`):

```rust
//! GPU end-to-end selection + caret tests (T7): real entities through
//! TextSync → TextCommit → render-prep (blink) → extract → the § 4.6
//! splice + glyph draw → pixels. decoration-and-paint §§ 5–6;
//! verification §§ 1.3, 3.1 (fixed_clock drives the blink pair).
//! Run: cargo test -p buiy_core --test text_selection_caret_gpu -- --ignored --test-threads=1
```

Shared setup: theme tokens — `color.selection.bg` = pure red,
`color.selection.fg` = pure blue, text token = white (chroma-orthogonal
triple; the T6 idiom); `wait_for_text_ready` before every capture.

1. `mixed_bidi_selection_paints_disjoint_rects_and_retints` — W=256, H=64;
   `register_fixture_font(&mut app, "Noto Sans Hebrew",
   "NotoSansHebrew-hebrew.ttf")`; the T5 corpus line
   `"hello עולם world"` with the `[Fira Sans, Noto Sans Hebrew]` stack at
   20 px; `SelectionVisual::new(Cursor::new(0, 10), Cursor::new(0, 18))`
   (mid-Hebrew → mid-"world" — the disjoint case; byte map in the test
   comment). Assertions on a mid-line pixel row (derive the row from the
   white/blue ink envelope):
   - **≥ 2 disjoint horizontal red runs** (column-coalesced) — the
     multi-rect contract as pixels;
   - **blue glyph ink exists inside red runs** (the re-tint painted over
     the selection bg — glyph tier over quad tier), and **white ink exists
     outside them** (unselected text untinted);
   - re-capture determinism: a second identical capture has
     `perceptual_diff < 1e-4`.
2. `caret_blink_fixed_clock_pair` — W=128, H=64; `"Hi"` at 40 px (the T6
   no-descender fixture), `CaretVisual { visible: true, rect:
   Rect::new(80.0, 0.0, 81.0, 48.0) }` (a column safely right of the glyph
   ink), `CaretColor` = the red test token. The § 3.1 pair, at two chosen
   virtual instants:
   - capture A at t≈0 (visible phase): a red column exists at the expected
     x band (scan columns; coalesce) spanning ≈ the line-box rows; record
     its column range;
   - `app.world_mut().resource_mut::<Time<Virtual>>()
     .advance_by(Duration::from_millis(500))` + `finish_and_run(&mut app, 2)`
     → capture B (hidden phase): **zero red pixels** in that column range;
     the white glyph ink is byte-identical between A and B outside the
     caret columns (the blink touched nothing else);
   - advance 500 ms again → capture C: `perceptual_diff(A, C) < 1e-4`
     (the pair is periodic — the fixed clock pins both phases exactly).

- [ ] **Step 2: Run the GPU lane — green** (these are end-to-end over
  Tasks 1–5's work; RED here means a real pipeline gap, not a test gap).
- [ ] **Step 3: Run BOTH gate lanes — green.** Commit:
  `test(text): T7.6 — mixed-BiDi ::selection golden + caret-blink pair (GPU)`.

---

### Task 7: Docs flip + errata + self-review

**Files:**
- Modify: `docs/README.md`
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`

- [ ] **Step 1: Campaign plan** — flip the phase-status table row
  `T7 | Selection + caret + placeholder painting | proposed → landed`, and
  append the errata block to the T7 section (the established "T*n* errata
  for the spec edit pass" pattern — superseding context, not a silent
  contradiction), seeded from this plan + anything further found while
  implementing:

  1. *decoration-and-paint § 6.3's "`SelectionVisual` — the § 5 rect list
     plus re-tint ranges"* misstates the payload: § 5.1's own mechanism
     line governs ("the **endpoints** reach the producer through the
     render-prep-written `SelectionVisual` state") — as built,
     `SelectionVisual` is the normalized `(Cursor, Cursor)` endpoint pair
     (the `selection_bounds()` output shape), and the producer derives
     rects AND re-tint from it inside the run walk it already owns; a
     rect-list payload would be derived data going stale against reshape,
     and "re-tint ranges" are the endpoints restated.
  2. *glyph-pipeline § 6.2's "rebuild `ExtractedTextQuads` alongside" /
     T6 decision 12's unconditional republish* conflicts with § 6.3's
     damage property ("a blink frame changes only `ExtractedGlyphs`") and
     verification § 1.2's damage row: as built, the rebuild stays
     wholesale under the one damage decision but publication is
     **value-compared** per carrier — a content-identical rebuild keeps
     the carrier's tick, so a blink edge re-uploads the glyph buffer only
     (T7 plan decision 4; `GlyphAlphaInstance` gained `PartialEq` for it).
  3. *§ 6.1's `Editor::cursor_position() -> Option<(i32, i32)>`* — exists
     (the int cast is a private helper, edit/mod.rs:30) but is NOT the
     painting input: the rect arrives authored in `CaretVisual`, and the
     editing campaign's f32-precise geometry source is
     `LayoutRun::cursor_position -> Option<f32>` + run metrics (already
     pinned by editing-and-ime § 4.1); the i32 mention should not be read
     as a producer contract.
  4. *§ 5.1's `highlight` framing* omits the **caller line-gate
     contract**: for a run on a line outside `[start.line, end.line]` the
     predicate degenerates to all-selected (source-verified;
     upstream's own render gates at edit/editor.rs:103) — the producer
     gates first. Also unstated: the two reference-render behaviors Buiy
     mirrors live in `Editor::render`, not in `highlight` (internal-
     empty-line full-width rect; last-rect extension to the line edge on
     non-final selected lines, RTL-aware).
  5. *Block interaction*: § 7 of font-assets zero-alphas text ink (and T6
     extended that to decorations); as built the caret and selection
     **background** paint normally under `font-display: Block` (editor
     chrome, not ink — a focused loading input keeps its caret, web
     parity), while the selected-glyph re-tint inherits the glyph
     zero-alpha (T7 plan decision 9).

- [ ] **Step 2: docs/README.md** — add the T7 plan line to the text-plans
  catalog block (after the T6 line, same format):
  `- [Buiy text T7 — Selection + caret + placeholder painting](plans/2026-06-11-buiy-text-t7-selection-caret-placeholder.md) — the editing campaign's painting primitives: `CaretVisual`/`SelectionVisual` paint-input state (§ 6.3 pinned shapes; SelectionVisual = the normalized `(Cursor, Cursor)` endpoint pair) + the edge-only caret-blink render-prep writer in the Animate→Picking window (square wave of the virtual clock, `CaretBlinkInterval` 500 ms half-period, reduced-motion steady), `highlight()`-driven selection rects at quad seat 2 via `ExtractedTextQuads` (caller line-gate; upstream's empty-line + line-edge extensions; mixed-BiDi multi-rect inherited), per-cluster re-tint (`color.selection.fg`, atlas untouched), the caret as a § 3.3-snapped solid-stamp at seat 6 with the `caret-color` chain (explicit → `color.caret` if present → currentColor), `Highlight`/`HighlightText` forced-colors preference, value-compared carrier publish (blink re-uploads glyphs only — CPU half; GPU half = T8), `::placeholder` = one token through the unchanged pipeline. GPU: mixed-BiDi `::selection` golden + the caret-blink fixed-clock pair. `[landed]``
- [ ] **Step 3: Self-review.** Re-read the diff against decoration-and-paint
  §§ 5–8 and glyph-pipeline §§ 6.2–6.3: every **F** marker in §§ 5–8
  either landed or is a named editing-campaign/T8 seam; the § 4.4 seat
  table holds for the full entity (2 < 3 across runs, 6 last); the § 6.2
  ledger comments now name zero open joins; no cosmic type crossed the
  render seam (`text_touch_pass.rs` green); the blink writer issues zero
  writes on steady frames (the CaretEdges tests). Confirm the campaign's
  T7 test surface is fully covered (selection-rect emission incl.
  mixed-BiDi multi-rect as resolved numbers; re-tint instance-color
  assertions; caret rect numbers + blink as a function of a stepped
  virtual clock — headless; the mixed-BiDi `::selection` golden + the
  caret-blink fixed-clock pair — GPU). Dispatch a fresh-context review
  subagent over the full T7 diff (the requesting-code-review skill).
- [ ] **Step 4: Run BOTH gate lanes one final time — green.** Commit:
  `docs(text): T7 — campaign status flip + errata`.
