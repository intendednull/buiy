# Buiy Text Campaign (T1–T9)

**Date:** 2026-06-09
**Status:** proposed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/README.md](../specs/2026-06-09-buiy-text-rendering-design/README.md)

> **For agentic workers:** this is a *campaign* plan, not a bite-sized TDD plan
> — the R-series precedent (one plan file per phase, each independently
> landable). Per-phase TDD plans come later, **one per phase**, written when
> that phase starts. Phases run as sequential Workflows; the orchestrator stays
> in the loop between them.

**Goal:** Implement the text-rendering spec phase by phase: engine → component
→ measure → first pixels → correctness → decoration → selection/caret →
compositor seam → closure. Each phase lands with the headless gate green and
its GPU assertions `#[ignore]`d on the established lane.

**Gate invariant (every phase, every commit):** the headless
`xvfb-run -a cargo test --workspace` gate (NO `--ignored`) stays green — CI has
no adapter. The GPU lane is **additive** —
`cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` on the GPU host,
green before a phase merges (CLAUDE.md § GPU lane).

**Campaign shape (decided).** Single rendering campaign T1–T9 at R-series
granularity. **Runner-ups rejected:** one monolithic text plan (exploratory
multi-seam work needs the orchestrator between phases — the render campaign's
own lesson); two interleaved campaigns (rendering + editing) — editing needs
focus + input routing, entirely different seams, and bevy-cosmic-edit's archive
is the standing warning against widening the bridge surface in one bite. The
**editor/IME/undo state machine**
([editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md)
§§ 2–13) is therefore a named **successor campaign** (`buiy-text-editing`)
consuming T7's painting primitives — not a T-phase here.

**Pre-phase decisions — all resolved in review round 1 (2026-06-09):**

1. `shape-run-cache` ON vs OFF — **resolved: OFF** (architecture § 7 is the
   decision record; font-assets § 1 + measure § 3.2 edited to match; runner-up
   ON-with-trim rejected as unmeasured complexity). T1's feature list is
   default features only.
2. The caret paint seat — **resolved: the stamp model** (editing-and-ime § 5
   rewritten to decoration-and-paint § 4.2's glyph-tier solid stamp; blink =
   a `CaretVisual` edge through the independent glyph damage gate). T7 is
   unblocked.
3. The SwashCache trim vs `get_image_uncached` contradiction — **resolved:
   uncached** (architecture § 1.3 rewritten — the caching path is unused, the
   atlas is the one bitmap cache, no trim exists to build).

---

## Phases

### T1 — Engine foundation

- **Deliverable:** `cosmic-text = "0.19"` direct dependency, **default
  features only** — `shape-run-cache` OFF per pre-phase decision 1 (`cargo
  deny check` first — supply-chain gate); the `buiy_core::text` module
  skeleton;
  `SharedFontSystem(Arc<Mutex<FontSystem>>)` built via
  `new_with_locale_and_db` over the embedded default font (Fira Sans latin
  subset behind the default-on `default_font` feature) with pinned generic
  families + the deterministic `BuiyFallback`; the render-world `SwashCache`
  resource; opt-in background system-font scan with the § 3.1-style rebuild
  swap + `FontsGeneration` reshape trigger.
  (architecture §§ 1–2, font-assets §§ 1, 4–5.)
- **Dependencies:** none (pre-T1 decision 1 resolved).
- **Test surface:** headless only — two `FontSystem` constructions on a
  zero-system-font host resolve every default identically; the scan-swap emits
  exactly one `FontsGeneration` bump; no adapter anywhere.

### T2 — Text component + Buffer lifecycle

- **Deliverable:** the retained `TextBuffer` component (0.19 lazy-setter
  contract; `Shaping::Advanced` hard-pinned; despawn cleanup free); the
  `FontFamily`/`FontSize`/`FontWeight` phase-1 components + plugin defaults;
  the `BuiyLayoutStep::TextSync` step (trigger union, whitespace-collapse
  pre-pass, intrinsics invalidation, `mark_dirty`); the idempotent
  `ComputedTextLayout` output type; `TextColor` (the graduated
  `Visual.foreground_token`).
  (architecture §§ 3–5.1, measure-and-layout § 4.1, font-assets § 8,
  glyph-pipeline § 7.)
- **Dependencies:** T1.
- **Test surface:** headless — `tests/layout_pipeline_order.rs` grows
  `TextSync`; trigger-set tests per the § 5.1 rows; Changed-gated reshape
  (`Changed<TextBuffer>` never fires — the bypass discipline). (The
  `ComputedTextLayout` idempotency test moved to T3 — the component is only
  *written* by `TextCommit`, which lands there.)

### T3 — Measure + wrap/align

- **Deliverable:** `LayoutTree` migrates `TaffyTree<()>` → `TaffyTree<Entity>`
  with edge-triggered `set_node_context`; one shared
  `compute_roots_with_text_measure` helper replacing plain `compute_layout` at
  **all three** compute sites (`systems.rs:2625/3602/2876`); the measure
  protocol (cached min/max-content intrinsics, definite-width relayout fold);
  `BuiyLayoutStep::TextCommit` (reshape at final width, `Align`,
  `ResolvedBaseline` + `ComputedTextLayout` writes); line-height → `Metrics`,
  white-space/text-wrap → `Wrap` + balance/pretty/stable greedy degrade,
  text-align at commit; intrinsic keywords on text leaves;
  `TextMeasureCallCount`.
  (measure-and-layout §§ 2–7, architecture § 4.2.)
- **Dependencies:** T2.
- **Test surface:** headless layout snapshots — content sizing, wrap-on-shrink,
  intrinsic keywords (CJK/no-break fixtures), steady-state zero measure calls,
  cq re-entrancy with `LayoutTaffyComputeCount` ≤ 2× roots,
  `shape_until_scroll` total-height pin, `ComputedTextLayout` idempotency
  (steady frame → tick unchanged; moved here from T2 — `TextCommit` is the
  writer), pipeline-order assertion grows `TextCommit`.

### T4 — First pixels

- **Deliverable:** `extract_buiy_glyphs` in `ExtractSchedule`
  `.after(maintain_atlas)` — the `physical()` 4-bin quantization, the 19 B
  structured `AtlasKey` + `FontKeyInterner`, `get_image_uncached`-on-miss
  rasterize closure (lock site #3), `GlyphAlphaInstance` emission (rect math,
  straight-alpha `TextColor` resolve, self-inclusive clip); the
  retain-with-probe damage gate + the un-gated `ResidentTextKeys` touch pass;
  color emoji = skip + rate-limited warn (the C-tier `IconInstance` seam stays
  named, **not** built — the blueprint's "IconInstance split" is superseded by
  glyph-pipeline § 9); delete `tests/atlas_gpu.rs`'s test-as-producer fill in
  favor of real-entity fixtures; the `hello_text` example; the
  `wait_for_fonts` predicate + `warm_atlas` drain realized for the golden
  harness (verification § 3). Producer ASCII pre-warm stays **out** (optional
  latency work, glyph-pipeline § 6.4). **Sequencing note (architecture
  OQ 2):** glyphs land in the flat global list (one batch after quads);
  cross-layer shadow < quad < glyph interleave across overlapping stacking
  layers is the render spec's buckets/`painters_z` work — layered-fixture
  z-order artifacts are expected until it lands and are not T4 bugs.
- **Dependencies:** T2, T3.
- **Test surface:** headless — `AtlasKey` round-trip/uniqueness, interner
  stability, rect math vs hand-computed `Placement` fixtures (incl. fractional
  scale), damage-gate retention per trigger, touch-pass survival >
  `eviction_grace`, the stub-atlas seam contract test. GPU lane — the
  `hello_text` gate-#2 golden; retint byte-identity with real text;
  eviction-under-retention regression (touch pass disabled → corruption
  caught, enabled → prevented).

### T5 — Fonts, fallback, and BiDi correctness

- **Deliverable:** the `BuiyFont` asset + `BuiyFontLoader` (sfnt invariant,
  Modified = remove+re-add); `FontRegistry` (strong handles, in-place
  `load_font_source` add, rebuild-on-remove via `into_locale_and_db`, the
  AtlasKey-never-persisted rule); the Buiy-owned `FontStack` resolver
  (fontdb `Query` matching, coverage span-splitting, unicode-range filter,
  `FontFallbackIter` last resort); `font-display` Swap (default) + Block
  (zero-alpha emission); per-node direction via the strong-mark prepend (measure-and-layout § 5.4); the curated
  multi-script shaping-snapshot corpus + per-script OFL fixture fonts.
  (font-assets §§ 2–3, 6–8, verification § 2.2.)
- **Dependencies:** T4.
- **Test surface:** mostly headless — register→resolve→query round-trip,
  unregister-rebuild leak/staleness assertions over N hot-reload cycles,
  font-display transitions, unicode-range span-splitting, multi-script shaping
  snapshots (Latin/Arabic/Devanagari/CJK/emoji-ZWJ/mixed-BiDi). GPU lane —
  1–2 goldens incl. the rebuild-storm bound (one frame of misses, page count
  returns to baseline).

### T6 — Decoration painting

- **Deliverable:** the node-level decoration component →
  `Attrs.text_decoration` mapping; the pure emission function
  `LayoutRun.decorations` → instances mirroring `render_decoration`'s math in
  f32 logical px with the physical-px min-thickness rule;
  underline/overline → quad bucket **via the `ExtractedTextQuads` carrier**
  (decoration-and-paint § 4.6: produced by `extract_buiy_glyphs`, packed into
  the existing quad instance buffer with per-entity order + partition
  contiguity preserved), line-through → solid-stamp glyph instance
  emitted after the run's glyphs; the 1×1 solid-white `CoverageR8` stamp as a
  **warmup-pinned** atlas entry (the one committed `AtlasWarmupQueue` push of
  this campaign — note: `AtlasWarmupQueue` is a render-world resource, so the
  startup stamp push must ride the render architecture § 1.1 finish-ordering
  seam to reach a live `RenderApp`); the `DecorationLineStyle` C-tier
  reservation.
  (decoration-and-paint §§ 2–4, 9.)
- **Dependencies:** T4.
- **Test surface:** headless exact-number emission tests (y, thickness incl.
  fractional-scale floor, double-underline gap, color precedence — the
  upstream-drift guard). GPU lane — one golden per decoration kind (underline,
  double, overline, line-through).

### T7 — Selection + caret + placeholder painting

- **Deliverable:** `LayoutRun::highlight`-driven selection rects
  (`color.selection.bg`, quad seat 2, via `ExtractedTextQuads`); per-cluster
  selected-text re-tint (`color.selection.fg`); the caret solid-stamp +
  `caret-color` resolution; the `CaretVisual`/`SelectionVisual` render-prep
  state components with the virtual-clock blink evaluated at the state write
  (edge-only, decoration-and-paint § 6.3) and reduced-motion steady state;
  `::placeholder` as same-pipeline tinted text. (The caret-seat conflict is
  resolved — pre-phase decision 2: glyph-tier stamp.) This phase's outputs are
  the painting primitives the successor `buiy-text-editing` campaign consumes.
  (decoration-and-paint §§ 5–8.)
- **Dependencies:** T6.
- **Test surface:** headless — selection-rect emission incl. the multi-rect
  mixed-BiDi contract as resolved numbers, re-tint instance-color assertions,
  caret rect numbers + blink as a function of a stepped virtual clock. GPU
  lane — the mixed-BiDi `::selection` golden; the caret-blink fixed-clock
  pair.

### T8 — Glyphs in effect groups + damage hardening

- **Deliverable:** partition `ExtractedGlyphs` by effect-group ranges +
  the `Glyph@Rgba16Float` specialization so text inside `Opacity(0.5)` dims
  (closes the follow-ups.md "glyphs bypass effect-group compositing" entry,
  the `TODO(text-seam)` at the glyph draw); the caret-blink-only-reupload
  assertion (blink frame touches the glyph buffer only); the typing-latency
  fixture (gate #14's text component).
  (glyph-pipeline § 11.2, decoration-and-paint §§ 4.5, 6.3,
  verification §§ 1.3, 4.)
- **Dependencies:** T4 (the compositor itself is already built — R9 + the GPU
  campaign).
- **Test surface:** GPU lane — text-in-effect-group composite golden (only
  claimable now); blink-damage assert; the latency fixture's budget wiring
  (numbers stay with `buiy-verification-design`).

### T9 — Verification closure + docs flip

- **Deliverable:** golden suite expansion (widget × state × theme × viewport
  text fixtures); the gate-#15 typing-churn fixture (edit loop → idle → atlas
  entry count returns within ε); decide/land the optional ASCII pre-warm (the
  architecture § 2.3 what-to-warm, deferred from T4); CLAUDE.md + docs/README
  catalog updates; the prior-art errata patch (verification § 5 ledger applied
  to `docs/prior-art/cosmic-text/` + `bevy-cosmic-edit/`); follow-ups.md flip;
  spec Status flips from proposed.
- **Dependencies:** all of T1–T8.
- **Test surface:** the full two-lane suite green; gate-#2/#14/#15 fixtures in
  place; `--accept` curation workflow exercised once end-to-end.

---

## Phase status

| Phase | Title | Status |
|---|---|---|
| T1 | Engine foundation | proposed |
| T2 | Text component + Buffer lifecycle | proposed |
| T3 | Measure + wrap/align | proposed |
| T4 | First pixels | proposed |
| T5 | Fonts, fallback, and BiDi correctness | proposed |
| T6 | Decoration painting | proposed |
| T7 | Selection + caret + placeholder painting | proposed |
| T8 | Glyphs in effect groups + damage hardening | proposed |
| T9 | Verification closure + docs flip | proposed |

Successor campaign (not phased here): **buiy-text-editing** — the
`TextEditState`/keymap/IME/clipboard/undo/`TextInput` implementation of
[editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md),
starting after T7.
