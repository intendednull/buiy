# Buiy — text-rendering design

**Date:** 2026-06-09
**Status:** implemented (rendering T1–T9 + editing E1–E6)
**Parent:** [`2026-05-07-buiy-foundation`](../2026-05-07-buiy-foundation/README.md) — sub-spec graduated from [foundation/text.md](../2026-05-07-buiy-foundation/text.md) and the foundation roadmap row `buiy-text-rendering-design`.
**Plan:** [`2026-06-09-buiy-text-campaign.md`](../../plans/2026-06-09-buiy-text-campaign.md) (T1–T9 phase breakdown; per-phase TDD plans follow, one per phase).

## Purpose

Define the target shape of Buiy's text stack on cosmic-text 0.19: engine
ownership across Bevy's two worlds, the Taffy measure seam, font assets and
family-stack resolution, the glyph producer that fills the **built and
GPU-verified** render atlas seam, decoration/selection/caret painting, and the
editing + IME surface. The render-side consumer — `BuiyAtlas`,
`AtlasWarmupQueue`, the 68 B `GlyphAlphaInstance`, the live glyph draw branch
in paint order shadow < quad < glyph — already exists
([render atlas-and-text-seam.md](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md));
this spec designs everything that produces into it and nothing that redesigns it.

### The contract, in one sentence

**Text shapes in the main world inside `BuiySet::Layout`, settles into
idempotent output components, and is consumed render-side by one
`ExtractSchedule` producer that owns residency — no text changed means zero
shaping, zero extract rebuild, and an O(visible-keys) atlas touch pass only.**

## Scope

**F-tier first.** This spec owns the F rows of
[foundation text.md §§ 3.4–3.5](../2026-05-07-buiy-foundation/text.md) in full:
font family/size/weight + registration (source, format, unicode-range,
font-display), line-height, white-space/text-wrap, text-align,
text-decoration-line/-color, `::selection` / `::placeholder`, mixed-direction
selection rectangles, caret styling/blink/`caret-color`, preedit rendering, and
the complete F editing surface (editor states, BiDi caret model, IME
composition, clipboard, undo/redo with composition grouping, auto-scroll).

**C-tier seams are named, not designed:** woff2 (loader sfnt invariant),
variable axes beyond weight (upstream #406), font synthesis, metric overrides,
theme font tokens, overflow-wrap, text-indent, vertical-align, rich-text
`AttrsList` authoring, color emoji (`IconInstance` + `ColorRgba8` pages),
`text-decoration-style`/-thickness/-offset/-position, skip-ink, multi-range
selection *behavior*, HTML/image clipboard, font-display Fallback/Optional.
**E-tier is named only:** rich-text edit surface, document virtualization,
text-emphasis.

## Children

Reading order: architecture first (it pins the seams every sibling assumes),
then measure-and-layout, then any topic in any order.

- [architecture.md](architecture.md) — engine ownership (`SharedFontSystem`,
  render-world `SwashCache`), `FontSystem` lifecycle + `FontsGeneration`, the
  retained `TextBuffer` / idempotent `ComputedTextLayout` state, the
  `TextSync`/`TextCommit` layout steps and the `extract_buiy_glyphs` producer
  placement, the three trigger tables + `ExtractedGlyphs` retention, the
  scale-factor model, `buiy_core::text` + the `cosmic-text = "0.19"` pin.
- [measure-and-layout.md](measure-and-layout.md) — the Taffy measure seam:
  `TaffyTree<()>` → `TaffyTree<Entity>`, the measure protocol (content-box
  contract, cached min/max-content intrinsics), reshape-at-final-width in
  `TextCommit`, one shared compute helper at all three compute sites (≤2×
  ceiling preserved), line-height/white-space/text-align mapping,
  `ResolvedBaseline`, the O(0) change-detection contract.
- [font-assets.md](font-assets.md) — the `@font-face` analogue: `BuiyFont`
  asset + loader, `FontRegistry` (strong handles; add in-place,
  rebuild-on-remove), the embedded deterministic default font, opt-in
  background system-font discovery, the Buiy-owned `FontStack` resolver with
  unicode-range and font-display Swap/Block.
- [glyph-pipeline.md](glyph-pipeline.md) — the glyph producer end-to-end:
  shaped `Buffer` → `physical()` 4-bin subpixel quantization → 19 B `AtlasKey`
  (`FontKeyInterner`) → `get_image_uncached`-on-miss → `get_or_insert` →
  `GlyphAlphaInstance`; the damage gate, the un-gated `ResidentTextKeys` touch
  pass (the eviction-under-retention hazard), `TextColor`, clip, the
  color-emoji C-tier seam.
- [decoration-and-paint.md](decoration-and-paint.md) — painting every
  non-glyph text visual: inherited `DecorationSpan` metrics with Buiy-owned
  f32 emission + physical-px min-thickness, paint-order seats (quads under,
  1×1 solid-white CoverageR8 stamps over), `::selection` via
  `LayoutRun::highlight` + per-cluster re-tint, the caret stamp + blink +
  reduced-motion, `::placeholder`, the preedit-underline painting primitive.
- [editing-and-ime.md](editing-and-ime.md) — the F-tier editor surface:
  `TextEditState` over `cosmic_text::Editor<'static>`, the
  `KeyboardInput` → `EditCommand` → `Action` keymap, the BiDi caret +
  multi-range-shaped `TextSelection`, IME display-splice preedit with four
  normative invariants, arboard clipboard facade, the two-stack `UndoUnit`
  model, auto-scroll via `ScrollOffset`, the Message taxonomy.
- [verification.md](verification.md) — the proof strategy: headless geometry
  vs GPU-lane pixels, embedded pinned fonts + the curated multi-script corpus,
  the `GoldenConfig` flake triad realized (`fixed_clock` / `wait_for_fonts` /
  `warm_atlas`), campaign gate discipline (#2/#14/#15), the prior-art errata
  ledger.

## Architectural pillars (one line each)

1. **One engine, one lock.** `SharedFontSystem(Arc<Mutex<FontSystem>>)` cloned
   into both worlds; exactly three lock sites (Taffy measure, `TextCommit`
   shaping, atlas-miss rasterization); `SwashCache` is a render-world resource
   (architecture § 1).
2. **Registered fonts only at startup.** `new_with_locale_and_db` + the
   embedded default font + pinned generic families; the issue-#505 system scan
   is opt-in, background, and swapped in with a `FontsGeneration` reshape
   (architecture § 2, font-assets §§ 4–5).
3. **Retained `TextBuffer`, lazy setters, `Shaping::Advanced` hard-pinned.**
   The per-entity `Buffer` mutates in place (per-line caches = the
   typing-latency win); shaping defers to lock-bearing sites;
   `ComputedTextLayout` is written idempotently (architecture § 3).
4. **Text rides the layout chain.** `TextSync` (pre-`SyncStyles`, dirty-marks
   Taffy) and `TextCommit` (post-`CqDescendantReRun`, reshape at final width)
   keep all layout — including line layout — settled when `BuiySet::Layout`
   ends (architecture § 4, measure-and-layout § 4).
5. **Measure through Taffy's own seam.** `TaffyTree<Entity>` node context, one
   shared `compute_roots_with_text_measure` helper at all three compute sites,
   cached intrinsics; the ≤2× compute ceiling and the O(0) steady state hold
   (`bypass_change_detection` + `TextMeasureCallCount`) (measure-and-layout
   §§ 2–7).
6. **The render seam is consumed, never redesigned.** `extract_buiy_glyphs`
   (`ExtractSchedule`, after `maintain_atlas`) fills the GPU-verified producer
   slot; retain-with-probe damage plus the un-gated `ResidentTextKeys` touch
   pass forecloses the stale-uv eviction hazard (architecture § 5,
   glyph-pipeline § 6).
7. **Shape logical, rasterize physical.** `LayoutGlyph::physical(offset,
   scale)` 4-bin x-subpixel quantization → the structured 19 B `AtlasKey` via
   `FontKeyInterner`; `get_image_uncached` on miss (one cache: the atlas); a
   scale change re-rasterizes, never reshapes (architecture § 6,
   glyph-pipeline §§ 3–5).
8. **Buiy owns the family stack above cosmic-text.** The `FontStack` resolver
   + `FontRegistry` (strong handles; in-place add, rebuild-on-remove because
   `font_cache` has no purge API), unicode-range enforced in the resolver,
   font-display Swap/Block on existing render machinery (font-assets §§ 3–7).
9. **Alpha-as-color painting reuse — no new primitives.** Selection/underline/
   overline are quad-tier under glyphs; line-through and the caret are 1×1
   solid-white `CoverageR8` stamps over glyphs riding the independent glyph
   damage gate; `::selection` re-tints at emission and never touches the atlas
   (decoration-and-paint §§ 3–6).
10. **Editor wrapped, not rebuilt.** `cosmic_text::Editor<'static>` behind the
    `TextEditState` facade; per-platform keymap → `EditCommand` → `Action`;
    winit owns the IME state machine, preedit is a display-splice with four
    normative invariants; Buiy owns undo with composition grouping
    (editing-and-ime §§ 2–8).
11. **Headless geometry, GPU pixels.** Every CPU-derivable property proves in
    the adapterless every-PR gate; the `#[ignore]` GPU lane carries only
    pixels; embedded pinned fonts + the realized flake triad make text goldens
    deterministic (verification §§ 1–3).
12. **`buiy_core::text`, `cosmic-text = "0.19"` pinned directly.** The
    layout↔text measure cycle blocks a crate split; the pin is never ridden
    transitively; Buiy stays on cosmic-text as Bevy 0.19 moves to Parley
    (architecture § 7, font-assets § 1).

## Relationships

- **Foundation [text.md](../2026-05-07-buiy-foundation/text.md)** owns the
  WHAT (the F/C/E row inventory). This spec is the HOW for every F row and the
  named seam for every C row; tier deviations are recorded per file, none
  silent.
- **Render
  [atlas-and-text-seam.md](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md)**
  owns the consumer half (atlas, primitives, draw, eviction contract) — built
  and GPU-verified (R10 + the
  [2026-06-07 GPU-verify campaign](../../plans/2026-06-07-render-gpu-verify-campaign.md)).
  This spec fills the producer slot it reserved; the seam contract (no
  cosmic-text type crosses into render's public types) is test-pinned.
- **Prior art:** [cosmic-text](../../prior-art/cosmic-text/) and
  [bevy-cosmic-edit](../../prior-art/bevy-cosmic-edit/) were the launchpads;
  several of their claims are superseded by 0.19-verified facts — the errata
  ledger is [verification.md § 5](verification.md#5-prior-art-errata-ledger),
  and the folders are owed a correction pass (campaign phase T9).
- **The campaign plan**
  [2026-06-09-buiy-text-campaign.md](../../plans/2026-06-09-buiy-text-campaign.md)
  phases the rendering work T1–T9. The editing/IME *implementation*
  (editing-and-ime.md §§ 2–13) is a named **successor campaign**
  (`buiy-text-editing`) consuming T7's painting primitives.

## Synthesis notes — canonical names

The six section files were authored in parallel from per-area blueprints; the
2026-06-09 synthesis pass unified naming. Canonical names (do not reintroduce
the drafts' variants):

| Canonical | Superseded draft names |
|---|---|
| `architecture.md` | engine-and-pipeline.md |
| `measure-and-layout.md` | measure-and-layout-seam.md |
| `font-assets.md` | fonts-and-registration.md |
| `glyph-pipeline.md` | glyph-producer.md |
| `editing-and-ime.md` | editing.md |
| `decoration-and-paint.md` | decoration-and-selection.md |
| `SharedFontSystem` | `BuiyFontSystem` |
| `BuiyLayoutStep::TextCommit` | `TextFinalize` |
| `extract_buiy_glyphs` | `extract_buiy_text` |

Other load-bearing names, defined once: `TextBuffer`, `ComputedTextLayout`,
`TextDirection` (measure § 5.4),
`FontsGeneration` (architecture); `ResolvedBaseline` (measure § 6); `TextBufferAccess`
(measure-and-layout); `FontRegistry`, `BuiyFont`, `FontStack` (font-assets);
`FontKeyInterner`, `ResidentTextKeys`, `TextColor` (glyph-pipeline);
`ExtractedTextQuads`, `CaretVisual`, `SelectionVisual` (decoration-and-paint,
added in review round 1); `TextEditState`, `EditCommand` (editing-and-ime).

## Status

**Status: implemented (rendering T1–T9 + editing E1–E6) — rendering as landed
2026-06-11, editing as landed 2026-06-13.** The whole text subsystem this spec
designs — rendering *and* editing — is built. The rendering surface landed
through phases T1–T9 of the
[text campaign](../../plans/2026-06-09-buiy-text-campaign.md); the editor/IME
surface landed through phases E1–E6 of the
[text-editing campaign](../../plans/2026-06-13-buiy-text-editing-campaign.md).
Both are proven on the two-lane suite ([verification.md](verification.md) §§ 1,
4) — the headless geometry gate every PR plus the `#[ignore]` GPU pixels lane
(the gate-#2 goldens including the T9 widget × state × theme × viewport matrix
and the E6 `TextInput` golden, the gate-#14 typing-latency fixture, the gate-#15
churn pair).
[editing-and-ime.md](editing-and-ime.md) is **implemented** — the
`buiy-text-editing` campaign (E1–E6) landed the F-tier editor surface
(`TextEditState` over `Editor<'static>`, the focus-gated keymap, the BiDi caret +
multi-range-shaped selection, the IME display-splice, the arboard clipboard, the
two-stack undo with composition grouping, auto-scroll via `ScrollOffset`,
placeholder, the § 11 taxonomy, and the `buiy_widgets::TextInput` bundle). The
named deferrals (multi-range selection *behavior*, HTML/image clipboard, the
BiDi split-caret secondary indicator, compose-over-selection) are filed in
[follow-ups.md](../../plans/follow-ups.md). The paragraphs below are the
proposal-time record, kept for history.

**Status: proposed** *(superseded 2026-06-11 by the as-landed paragraph
above)*. Review round 1: FIX-THEN-SHIP, fixes applied
2026-06-09. Review round 2: FIX-THEN-SHIP (2 majors — both round-1-fix
drift: the `Changed<Window>` scale probe, the `painters_z` merge key), fixes
applied 2026-06-09. Review round 3: FIX-THEN-SHIP (1 major — the § 5.4
direction lowering switched from a whole-line isolate wrap, which UAX #9 P2
ignores for the paragraph level, to a strong-mark prepend; 1 README drift),
fixes applied 2026-06-09. Review round 4: SHIP-AS-IS. Synthesized
2026-06-09 from six parallel section
blueprints over verified cosmic-text 0.19 facts and the as-built render seam.
The open questions below are annotated with their resolution status; the
unresolved remainder must be settled before the corresponding campaign phases
start. No code exists yet; `cosmic-text` is not in `Cargo.lock`.
*(Superseded 2026-06-11, T9: `buiy_core::text` is built and GPU-verified —
T1 pinned `cosmic-text = "0.19"` into `Cargo.lock`.)*

**Review round 1: FIX-THEN-SHIP, fixes applied 2026-06-09.** The reviewer's
contradictions and gaps were resolved in place — `shape-run-cache` OFF, the
SwashCache caching path adjudicated unused, glyph-pipeline § 6.2 made the one
normative extract probe union, the `ExtractedTextQuads` quad carrier and the
`CaretVisual`/`SelectionVisual` render-prep state pinned, editing-and-ime § 5
rewritten to the stamp caret, the white-space/text-align value tables and the
font-provenance and accessor pins added. Per-entry annotations below.

## Open questions

Collected **verbatim** from the section files (resolution status annotated in
italics where the synthesis pass or a sibling file already answers one). The
synthesis pass did not adjudicate design conflicts — they are listed for the
orchestrator.

### From [architecture.md](architecture.md)

1. **Prior-art staleness — needs a correction pass.** The cosmic-text folder
   ([integration.md](../../prior-art/cosmic-text/integration.md) canonical
   shape, [lessons.md](../../prior-art/cosmic-text/lessons.md)
   FontSystem-singleton row) states `FontSystem` is non-`Sync` ("pin to UI
   thread or `Arc<Mutex>`"). **Falsified for 0.19** by docs.rs auto-traits
   (`impl Sync for FontSystem`); § 1.1 rests on the verified fact, not the
   folder. The folder needs a correction pass (worth-promoting finding per
   `using-prior-art`) or the next reader re-derives `NonSend` wrongly.
   *Scheduled as campaign phase T9 (the errata ledger,
   [verification.md § 5](verification.md#5-prior-art-errata-ledger)).*
   *Resolved: the correction pass was applied at T9 (2026-06-11) — the
   [verification.md § 5](verification.md#5-prior-art-errata-ledger) ledger
   landed on both prior-art folders as dated correction blockquotes; the
   staleness this question flagged is resolved.*
2. **Cross-layer glyph/quad interleaving.** `ExtractedGlyphs` is a flat global
   list drawn as one glyph batch after the quad batch (prepare.rs ~:46–51: "in
   paint order … after the quad draw"). Correct per-layer
   shadow < quad < glyph interleave **across overlapping stacking layers** is
   buckets/`painters_z` work owned by the render spec — until it lands, text
   producing into the flat list can surface as a z-order artifact on layered
   fixtures. Not a contradiction with this file's placement decisions, but a
   sequencing dependency the plan must order against.
   *Review round 1: the sequencing note is now recorded in the campaign's T4
   (layered-fixture z-order artifacts are expected, not T4 bugs). Still open
   as render-spec work.*
   *T9: carries forward — still the render spec's buckets/`painters_z` work.
   T8 narrowed it: quad-then-glyph order now holds per region (within each
   effect-group target and within the flat complement,
   [glyph-pipeline.md](glyph-pipeline.md) § 11.2–11.3); the interleave
   across overlapping stacking layers remains open.*

### From [measure-and-layout.md](measure-and-layout.md)

1. **FontSystem resource shape — blueprint vs. engine pin.** This area's
   exploration recommended a plain `Resource BuiyFontSystem(FontSystem)`
   (FontSystem is verified `Send + Sync` in 0.19, so `ResMut` already
   serializes main-world access), but the engine-ownership decision (area 0,
   [architecture.md](architecture.md)) pins
   `SharedFontSystem(Arc<Mutex<FontSystem>>)` because the RenderApp needs a
   clone for atlas-miss rasterization. This file follows the pin (§ 3.4) and
   flags rather than relitigates; if the render world ever stops needing the
   `FontSystem` (e.g. pre-rasterized warmup only), the plain-Resource shape is
   the simplification to revisit.
2. **Intrinsic-keyword fidelity under stretch.** Because taffy 0.10.1's
   `Dimension` cannot express min-/max-content (verified, § 3.3), keyword
   resolution lives in the measure closure and loses to a parent-resolved
   `known_dimensions` (cross-axis stretch). The blueprint's "makes
   `Sizing::MinContent/MaxContent` real" therefore holds for the main-axis /
   shrink-to-fit cases the F-tier needs, with the stretch case documented as
   `auto`-equivalent until a Taffy version carries the keywords natively.
3. **Multi-window measuring.** `taffy_compute` sizes roots against the first
   window (`systems.rs:2611-2615`) — a pre-existing limitation text inherits;
   text measured against the wrong viewport on secondary windows is out of
   scope here but named so it is not mistaken for a text bug.
4. **Per-buffer memory.** One `Buffer` per text node retains `shape_opt` +
   `layout_opt` caches; thousands of nodes may need a trim policy. (The
   SwashCache stays empty by construction — glyph-pipeline § 3.2's uncached
   path; `BufferLine` caches are this seam's.)

### From [font-assets.md](font-assets.md)

1. **Engine-ownership pin vs. this area's exploration recommendation.** This
   area's blueprint recommended a plain main-world `BuiyFontSystem(FontSystem)`
   `Resource` (FontSystem is verified Send+Sync in 0.19 — the "non-Sync"
   wording in [prior-art/cosmic-text/critiques.md](../../prior-art/cosmic-text/critiques.md)
   § FontSystem lifetime is STALE, describing the `&mut`-only API, not the
   marker traits) and explicitly rejected `Arc<Mutex<FontSystem>>` as "hiding
   the serialization instead of scheduling it"; it also placed `SwashCache` in
   the main world. The decided engine ownership (area 0,
   [architecture.md](architecture.md)) is
   `SharedFontSystem(Arc<Mutex<FontSystem>>)` cloned into the RenderApp, with
   `SwashCache` as a render-world resource. **This file is written against the
   pinned decision** (§ 3 paths lock `SharedFontSystem`; the § 3.1 rebuild
   swaps in place under one lock hold), recording the contradiction rather
   than silently dropping it: revisiting the lock later costs § 3 only
   mechanical `lock()`→`ResMut` edits, no semantic changes.
   *T9: stands as recorded — the `SharedFontSystem(Arc<Mutex<FontSystem>>)`
   pin held through T1–T8 (the T5 erratum only rescoped "exactly three lock
   sites" to steady-frame, architecture § 1.2 as-landed); the plain-Resource
   shape remains the named revisit if the render world ever stops needing
   the `FontSystem`.*

### From [glyph-pipeline.md](glyph-pipeline.md)

1. **Resource naming drift:** this area's exploration blueprint names the
   shared engine resource `BuiyFontSystem(Arc<Mutex<FontSystem>>)`, while the
   decided engine-ownership pin names it `SharedFontSystem`. This file adopts
   **`SharedFontSystem`** (the pinned decision wins);
   [architecture.md](architecture.md) is the naming owner — reconcile there if
   it lands differently.
   *Resolved: architecture.md landed `SharedFontSystem`; the folder is
   consistent.*
2. **Cross-area input contract (tracked, not contradictory):** § 2 step 0
   assumes a sibling area delivers a *shaped* `Buffer` component before extract
   (0.19's deferred-shaping API means someone must call `shape_until_scroll`
   with `&mut FontSystem` in the main world). If the measure seam does not
   guarantee shaping for the **final** post-layout wrap width, the producer's
   input contract is unmet — [measure-and-layout.md](measure-and-layout.md)
   must pin this explicitly.
   *Satisfied: measure-and-layout.md § 4.2 pins the `TextCommit`
   reshape-at-final-width pass, and architecture.md § 3.2 adds the
   `debug_assert!` tripwire at extract.*

### From [editing-and-ime.md](editing-and-ime.md)

1. **Frame-ordering for edit→layout.** `BuiySet` chains Layout → … → Input → …
   → Render (`crates/buiy_core/src/lib.rs:57-87`), so edits mutate the Buffer
   *after* layout ran this frame: content-size changes and caret geometry either
   re-enter layout same-frame (within the architecture's 2×-Taffy-per-frame cap)
   or show one-frame latency. This must be settled **jointly with
   [measure-and-layout.md](measure-and-layout.md)** (which owns the
   measure function and `shape_as_needed` scheduling); this file takes either
   answer without structural change, but the typing-latency gate (§ 12) depends
   on it.
   *T9: carries to `buiy-text-editing` — the successor settles edit→layout
   frame ordering. The T8 gate-#14 fixture already pinned the display-path
   half: one frame from an Update-phase `Text` mutation to publish,
   rasterize-on-miss included (`tests/text_typing_latency.rs`).*
   *E6: realized — the OQ#1 one-frame path landed in E2 (the Input-driven
   N→N+1 latency fixture); caret geometry, `ime_position`, and auto-scroll
   come current the same frame the edit's `TextCommit` publishes
   (editing-and-ime § 9, OQ#1). Resolved.*
2. **Prior-art drift needs a correction note.** Verified against 0.19:
   `Editor::with_selection_bounds` does not exist (real pair:
   `selection_bounds()` + `LayoutRun::highlight`); `Action::Scroll` takes
   `{ pixels: f32 }`, not `{ lines: i32 }`; `FontSystem` and `Editor` ARE
   `Send + Sync`; `Motion` has 22 variants. The `docs/prior-art/cosmic-text/`
   folder should receive a correction note (outside this spec folder's write
   scope), and sibling files citing those claims must re-verify.
   *Scheduled as campaign phase T9 via the
   [verification.md § 5](verification.md#5-prior-art-errata-ledger) ledger.*
   *Resolved: the T9 correction pass (2026-06-11) applied the ledger to both
   prior-art folders.*
3. **arboard HTML read-side** is unverified (§ 7) — confirm before scheduling
   the HTML-clipboard slice.
   *T9: carries to the successor campaign (`buiy-text-editing`).*
   *E6: still deferred — v1 ships plain-text clipboard (E4); the HTML/image
   slice is filed in [follow-ups.md](../../plans/follow-ups.md), gated on
   confirming arboard's HTML read first.*
4. **Shared-accessor type for the editor-owned Buffer** (§ 2.2a): the concrete
   QueryData shape is pinned by [measure-and-layout.md](measure-and-layout.md);
   if that file lands a Buffer-as-separate-component model incompatible with
   `BufferRef::Owned`, the two files must reconcile before implementation.
   *Resolved in review round 1: measure-and-layout § 2.3 pins
   `TextBufferAccess` (display component + optional editor, editor-preferred),
   explicitly compatible with `BufferRef::Owned`; editing-and-ime § 2.2a now
   names it.*
   *T3 erratum (measure-and-layout § 2.3 as-landed): the accessor itself is
   deferred to `buiy-text-editing` — its `edit` arm binds the
   not-yet-existing `TextEditState`; until the editor lands, display paths
   bind `&mut TextBuffer` / `&TextBuffer` directly, and the reconciliation
   this question asked about moves to the editing campaign with it.*

### From [decoration-and-paint.md](decoration-and-paint.md)

1. **Caret seat conflict with [editing-and-ime.md § 5](editing-and-ime.md).**
   That file pins the caret as "a 1–2 px **quad** … emitted on the next layer"
   and blink as "a CPU-side visibility toggle on the emitted quad." Two
   problems: (a) per-layer interleave does not exist — v1 routes everything to
   `layer 0` and "real layers are the paint-order phase's job"
   (buckets.rs:9–11, 146–153), so with the fixed rank `quad 1 < glyph 2`
   (buckets.rs:42–53) a quad caret paints **under** glyphs today, and a "next
   layer" hack would misuse the `painters_z` stacking index for a within-node
   ordering concern; (b) a quad caret rides the quad damage gate, so every
   blink would re-upload the whole quad buffer — the glyph-tier stamp blinks
   through the independent glyph gate alone (prepare.rs:157–216). This file's
   § 4.2 seats (caret + line-through = glyph-tier solid stamps) stand on those
   two as-built facts; editing-and-ime.md § 5's caret bullet should be revised
   to consume them. Same applies to its split caret (= two stamps, not two
   quads).
   *Resolved in review round 1 (2026-06-09): editing-and-ime.md § 5 rewritten
   to the decoration-and-paint § 4.2 stamp model — caret = glyph-tier solid
   stamp over text (split caret = two stamps), blink = a `CaretVisual` edge
   through the independent glyph damage gate (decoration-and-paint § 6.3),
   selection = quad-tier under text. T7 is unblocked.*
2. **Sibling file-name divergence.** editing-and-ime.md linked to a
   `decoration-and-selection.md` that does not exist — this file
   (`decoration-and-paint.md`) is the painting owner it means. For the
   synthesizer/README pass to reconcile.
   *Resolved in the 2026-06-09 synthesis pass: every sibling link now targets
   `decoration-and-paint.md`.*

### Found in the synthesis pass (not author-flagged)

1. **`shape-run-cache` feature: ON vs OFF.** [font-assets.md § 1](font-assets.md)
   pinned `features = default + shape-run-cache` ("most embedders should turn
   it on", prior-art critiques) and [measure-and-layout.md § 3.2](measure-and-layout.md)
   assumed it as a second-level cache; [architecture.md § 7](architecture.md)
   pins it **OFF** in v1 (grows `FontSystem`-side without bound, against gate
   #15 — "revisit on measurement, not speculation"). Direct contradiction
   between recorded decisions with opposite rationales.
   *Resolved in review round 1: **OFF** for v1 — architecture § 7 is the
   winner and the decision record. The retained `TextBuffer`'s per-line
   `shape_opt` cache is the amortization that matters (measure's
   multi-width probes never re-shape); the run cache's win is unmeasured and
   its memory unbounded. font-assets § 1 and measure-and-layout § 3.2 were
   edited to match. Runner-up ON-with-trim rejected as unmeasured complexity;
   turning ON later is a one-line feature flip.*
2. **ASCII warmup phasing.** [architecture.md § 2.3](architecture.md) commits
   the what-to-warm (ASCII of the default theme font) as F;
   [glyph-pipeline.md § 6.4](glyph-pipeline.md) keeps the v1 producer slice
   warmup-free (first-frame determinism is structural) and rejects a mandatory
   phase-1 ASCII pre-warm; [decoration-and-paint.md § 4.3](decoration-and-paint.md)
   warmup-pins the 1×1 solid stamp. Reconcilable (the stamp push is
   decoration's, the ASCII pre-warm a later latency optimization), but the
   campaign must sequence it explicitly — see the plan's T6/T9 notes.
   *Resolved at T9: the production ASCII pre-warm is **rejected**
   (architecture § 2.3 as-landed — unmeasured win; unconditional grace
   eviction drains unpinned warm keys in ~1 s; no theme-font/size
   enumeration exists to warm from). The 1×1 solid-stamp push (T6) was the
   campaign's only warmup producer; the re-open trigger is a measured
   first-keystroke-latency miss against a `buiy-verification-design`
   budget.*

### Found in review round 1 (2026-06-09) — all resolved

1. **SwashCache trim vs `get_image_uncached`.** architecture § 1.3 mandated a
   gate-#15 trim of `SwashCache.image_cache` while
   [glyph-pipeline.md § 3.2](glyph-pipeline.md) pinned the uncached path (the
   cache never grows; no phase implemented a trim). *Adjudicated for
   uncached: the atlas — content-addressed, deduplicating, LRU-bounded — is
   the one bitmap cache; architecture § 1.3 now records the caching path as
   unused (resource kept for API access only) and the trim obligation is
   gone. Runner-up `get_image` + trim rejected as a second cache duplicating
   the atlas's eviction policy. Recorded as resolved in the campaign's
   pre-phase list.*
2. **Extract probe unions diverged.** architecture § 5.1 row 3 and
   glyph-pipeline § 6.2 listed different trigger unions, and neither carried
   the scale-factor probe consistently. *glyph-pipeline § 6.2 is now THE
   normative union — grown by the scale-factor probe (the cached-f32 value
   compare per round 2 — never `Changed<Window>`) and the caret/selection
   visual state; architecture
   § 5.1 row 3 is a summary deferring to it.*
3. **Decoration/selection quad carrier + caret emission site were
   unspecified.** The as-built quad path packs one background quad per entity;
   text needs N rects per node, and nothing pinned where caret state is
   evaluated. *Pinned: `ExtractedTextQuads` (decoration-and-paint § 4.6 —
   produced by `extract_buiy_glyphs`, packed into the existing quad buffer
   with per-entity ordering and partition contiguity preserved) and
   `CaretVisual`/`SelectionVisual` render-prep state components with the
   blink clock evaluated at the state write, edge-only (decoration-and-paint
   § 6.3); both joined the § 6.2 probe union.*
