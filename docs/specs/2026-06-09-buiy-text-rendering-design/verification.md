# Buiy text — verification

**Parent:** [README.md](README.md)

How text correctness is proven. Text inherits the render subsystem's hard
constraint — **CI runners have no wgpu adapter**
([render verification.md § 1](../2026-06-03-buiy-render-pipeline-design/verification.md#1-the-hard-constraint-no-gpu-on-ci-runners)) —
and adds one of its own: **fonts are an external, machine-varying input**, and
shaping output (glyph ids, advances, decoration metrics) is font-file-pinned.
This file owns the text answer to both: the headless/GPU proof split (§ 1), the
embedded deterministic font set (§ 2), the realization of the
`GoldenConfig` flake triad — `wait_for_fonts` and `warm_atlas` flip from
declared flags to implemented predicates here (§ 3) — the campaign gate
discipline (§ 4), and the prior-art errata ledger this spec supersedes (§ 5).

It owns the *mechanism*; per-fixture tolerance/perf/leak numbers stay owned by
`buiy-verification-design`, exactly as in the render spec. The campaign **phase
breakdown is not here** — phasing lives in the text-rendering campaign plan
under `docs/plans/` (see [README.md](README.md)), per the R-series precedent.

---

## 1. Proof layers for text

### 1.1 The split decision

**Decision.** Every text property is proven at the lowest layer that can see
it, mirroring [render verification.md § 2](../2026-06-03-buiy-render-pipeline-design/verification.md#2-the-proof-layers):
the **entire geometry side of text is headless, every-PR material**, and the
GPU `#[ignore]` lane carries only pixels. **F**

**Rationale.** `FontSystem` + `Buffer` + `layout_runs` + `decorations` +
`highlight` are pure CPU — no wgpu adapter anywhere in shaping, measure,
decoration math, selection rects, or caret geometry. This is the text analogue
of the gate-#5 layout-snapshot move that made `ClipRect` testable headless: a
wrong underline y is **one number** in a headless test, not a fuzzy diff in a
golden.

**Rejected runner-up:** golden-image-only verification (prove everything at
gate #2). Loses decisively: CI has no adapter, so golden-only means the
headless gate proves *nothing* about text and every regression waits for a
local GPU run; it also debugs terribly (which of 40 glyphs moved, and why, is
invisible in a perceptual diff).

### 1.2 The headless inventory

All of these run in the every-PR gate (`xvfb-run -a cargo test --workspace`,
no adapter, [CLAUDE.md § Build & Test](../../../CLAUDE.md)):

| Property | Test shape |
|---|---|
| Shaping correctness | **Shaping snapshots**: per fixture (multi-script corpus, § 2.2), snapshot `(glyph_id, x, y)` per glyph and diff resolved values — the text analogue of gate-#5 layout snapshots |
| Measure / wrap / align | Layout snapshots through the Taffy measure seam ([measure-and-layout.md](measure-and-layout.md)) |
| Attrs mapping | Component → `Attrs` (incl. `text_decoration`) round-trip assertions |
| Decoration emission | **Exact-number tests** over the pure emission function: underline/overline/line-through y, thickness (incl. the physical-px floor at fractional scale factors), double-underline gap, color precedence — pinning [decoration-and-paint.md § 3.2–3.3](decoration-and-paint.md#32-the-mirrored-math) and detecting upstream drift on a cosmic-text bump |
| Selection rects | `highlight()`-driven emission, including the **multi-rect mixed-BiDi** contract (text.md:89) as resolved numbers |
| Selected-text re-tint | Per-cluster tint switch asserted on emitted instance `color` values |
| Caret geometry + blink | Caret rect numbers; visibility as a function of a stepped virtual clock; reduced-motion = constant-true |
| Seam contract | The stub-atlas contract test promised in [atlas-and-text-seam.md § 7](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#7-verification): drive `get_or_insert` + emit `GlyphAlphaInstance`s against a stub, proving no cosmic-text type crosses into the render crate |
| Damage discipline | Change-detection probes: steady-state frame leaves `ExtractedGlyphs` untouched; a caret-blink frame touches *only* `ExtractedGlyphs` (the CPU half of § 1.3's GPU assert) — via the adapterless extract harness below |

**The adapterless extract harness (how the damage rows run headless).** The
damage-discipline rows exercise `extract_buiy_glyphs` without a wgpu adapter:
the test builds the main `App` **without** `RenderPlugin`, constructs a bare
render `World` as a manual `SubApp` carrying only the CPU-side resources the
producer touches (`BuiyAtlas` — device-free by design — `ExtractedGlyphs`,
`ExtractedTextQuads`, `FontKeyInterner`, `ResidentTextKeys`, `SwashCache`, and
the `SharedFontSystem` clone), and registers `extract_buiy_glyphs` in a
manually built `ExtractSchedule`. Stepping the harness runs extract against
the live main world exactly as the pipelined renderer would; `prepare`, queue,
and draw simply never exist, so nothing requests an adapter. The GPU-side
consequences of the same triggers (buffer re-upload gating) stay in the § 1.3
lane.

### 1.3 The GPU `#[ignore]` inventory

These run on the established additive lane —
`cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` on a host with a
real adapter ([CLAUDE.md § GPU lane](../../../CLAUDE.md)) — built on
[`tests/support/mod.rs`](../../../crates/buiy_core/tests/support/mod.rs)
(`gpu_test_app` / `gpu_render_app` / `render_to_image` / `readback_rgba`):

| Property | Test shape |
|---|---|
| Rasterization → atlas → draw | The real `SwashCache` raster path inserting into `BuiyAtlas` and drawing through the live glyph branch (extends the existing `atlas_gpu.rs` lane) |
| Pixel correctness | Goldens (§ 4): hello-world text; one golden per decoration kind; the mixed-BiDi `::selection` golden; the caret-blink fixed-clock pair |
| Warmup determinism | First painted frame matches its golden because warmup forced residency pre-paint (the existing warmup GPU test pattern, extended to text fixtures) |
| Re-tint byte-identity | Already green for the pipeline (render GPU campaign Phase 4, [2026-06-07-render-gpu-verify-campaign.md](../../plans/2026-06-07-render-gpu-verify-campaign.md)); re-asserted with real text for theme swap + `::selection` re-tint |
| Caret-blink damage | Landed (T8): a blink frame issues exactly one glyph `write_buffer` and zero quad uploads, observed through `BufferUploadStats` — `caret_blink_reuploads_the_glyph_buffer_only` (tests/text_selection_caret_gpu.rs) |
| Glyph-in-effect-group composite | Landed (T8): text inside `Opacity(0.5)` dims exactly once — `tests/text_effect_group_gpu.rs` (the composite golden + the partition wiring assert) + the flipped `text_decoration_gpu.rs` asymmetry test ([decoration-and-paint.md § 4.5](decoration-and-paint.md#45-named-dependencies-not-owned-here)) |
| Atlas churn (gate #15) | The typing-churn fixture: scripted edit loop, then idle; atlas entry count returns within ε of baseline |

The boundary stays self-policing: a GPU-needing test without `#[ignore]`
panics the headless run at adapter init
([render verification.md § 6](../2026-06-03-buiy-render-pipeline-design/verification.md#6-verification)).

---

## 2. Deterministic fonts

### 2.1 Decision: embedded pinned fonts; system scan opt-in

**Decision.** Test fixtures (and goldens) construct the `FontSystem` from an
explicit, in-repo, version-pinned font set —
`FontSystem::new_with_locale_and_db(locale, db)` over an explicit
`fontdb::Database` (or `new_with_fonts`) — with the system-font scan **opt-in
and default OFF in fixtures**. Engine-side construction policy (lazy init,
production opt-in scanning) is [architecture.md](architecture.md)'s;
this file pins the *fixture* policy. **F**

**Rationale.** Shaping output is font-file-pinned: glyph ids, advances, and
decoration metrics (read from the font's own tables —
[decoration-and-paint.md § 2.1](decoration-and-paint.md#21-the-019-surface))
all change when the font does. This is the UDHR-fixture lesson from
cosmic-text's own test corpus: a correctness fixture needs a pinned corpus
**and** pinned fonts. The same move dodges two documented pitfalls at once:
the fontconfig-alias trap (issue #499 — `Family::Name("monospace")` resolves
differently per machine, [prior-art lessons.md](../../prior-art/cosmic-text/lessons.md))
and the 1.3 s `FontSystem::new` system-mmap stall (issue #505). And it is what
makes `wait_for_fonts` *implementable* (§ 3.2): embedded registration is
synchronous, so "fonts ready" reduces to a checkable predicate instead of an
unanswerable "wait for the OS."

**Rejected runner-up:** system fonts + a fatter perceptual tolerance.
Tolerance can absorb AA jitter but not a different font's different glyph
*shapes* — a Cantarell-vs-DejaVu swap is a layout change, not pixel noise, and
no tolerance budget separates it from a real regression.

### 2.2 The fixture corpus

A **curated multi-script corpus**, not cosmic-text's ~8 MB ~500-language UDHR
set: one fixture string each for Latin, Arabic (joining/RTL), Devanagari
(reordering), CJK, emoji (ZWJ sequence), and mixed-BiDi
(`"hello עולם world"`-class), plus decoration/selection/caret fixtures built on
them. Each corpus entry exists to pin one shaping behavior the F-tier claims;
breadth beyond that is upstream's job to test, not Buiy's. Embedded fonts must
cover every corpus script (the font set and its licensing live with
[font-assets.md](font-assets.md)).

---

## 3. The flake triad, realized

The render spec committed the triad as typed configuration —
`GoldenConfig { fixed_clock, wait_for_fonts, warm_atlas, accept }`
([golden.rs:16–44](../../../crates/buiy_core/src/render/golden.rs)) — with
text named as the owner of what two of the three *mean*. This spec flips all
three from declared flag to implemented mechanism. A golden captured without
all three is not reproducible ([render verification.md § 4.3](../2026-06-03-buiy-render-pipeline-design/verification.md#43-the-flake-mitigation-triad)).

### 3.1 `fixed_clock` — the virtual clock drives blink

Text's only time-dependent visual is the caret blink
([decoration-and-paint.md § 6.3](decoration-and-paint.md#63-blink-the-virtual-clock-reduced-motion-damage)).
Because blink phase is a pure function of the app clock, driving the harness
from a fixed/virtual clock pins it exactly; the blink-pair golden captures the
caret-visible and caret-hidden phases at two chosen virtual instants. No text
mechanism is added — text just reads the clock the harness already fixes.

### 3.2 `wait_for_fonts` — from flag to predicate

With embedded fonts (§ 2.1), font registration is synchronous at `FontSystem`
construction, so "every referenced font is loaded and its glyphs are resident"
([golden.rs:21–23](../../../crates/buiy_core/src/render/golden.rs)) becomes a
**checkable predicate**, evaluated before capture:

1. the fixture's `FontSystem` was built from the embedded `fontdb::Database`
   (registration already complete — nothing asynchronous exists to wait on);
2. the `AtlasWarmupQueue` is drained
   ([warmup.rs:19–44](../../../crates/buiy_core/src/render/atlas/warmup.rs));
3. every glyph key the fixture emits is resident, probed via the
   **no-LRU-touch** `BuiyAtlas::get`
   ([atlas-and-text-seam.md § 3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#3-the-insertlookup-api--the-only-handle-the-seam-touches)
   — the probe exists precisely so this check does not perturb eviction order).

A `wait_for_fonts` over OS-discovered fonts has no such predicate — which is
half the reason § 2.1 rejected system fonts in fixtures.

### 3.3 `warm_atlas` — the drain before capture

The fixture's glyph set (plus the solid stamp,
[decoration-and-paint.md § 4.3](decoration-and-paint.md#43-the-solid-stamp)) is
pushed into `AtlasWarmupQueue` and drained pre-capture, so first-frame upload
latency and allocation order never perturb the image, and the gate-#15
steady-state baseline is established. The mechanism is built
([warmup.rs](../../../crates/buiy_core/src/render/atlas/warmup.rs)); text
supplies the *what-to-warm* — the fixture glyph set in tests, the default
font's ASCII range in production ([glyph-pipeline.md](glyph-pipeline.md)).

---

## 4. Campaign gates

The text campaign inherits the render campaign's gate discipline
([2026-06-07-render-gpu-verify-campaign.md](../../plans/2026-06-07-render-gpu-verify-campaign.md)):

- **Headless gate green at every commit.** `xvfb-run -a cargo test --workspace`
  (no `--ignored`) never instantiates an adapter; every § 1.2 test joins it.
  CI is adapterless — this gate is the *only* per-PR proof, which is why § 1
  pushes everything possible into it. Watch the `-j 2` link-memory note as the
  cosmic-text dep tree grows, and run `cargo deny check` before the dependency
  bump ([CLAUDE.md](../../../CLAUDE.md)).
- **GPU lane additive and green on the GPU host pre-merge.** Every § 1.3 test
  carries `#[ignore]` and builds on `tests/support/mod.rs`. The lane must pass
  on the GPU host before a phase merges; it never blocks headless CI.
- **Golden `--accept` curation.** Text goldens ride the established
  human-curated update workflow
  ([golden.rs:28–30](../../../crates/buiy_core/src/render/golden.rs),
  [render verification.md § 4.4](../2026-06-03-buiy-render-pipeline-design/verification.md#44-human-curated---accept-workflow));
  a re-shaped fixture is either a regression or a curated golden update, never
  an automatic overwrite.
- **Gate #2 (visual regression) — the text fixture set.** Hello-world text;
  one golden per decoration kind (underline, double underline, overline,
  line-through); the mixed-BiDi `::selection` golden; the caret-blink
  fixed-clock pair; multi-script corpus renders. Captured under
  `GoldenConfig::deterministic()`.
- **Gate #14 (perf) — text's component.** Text's contribution to the combined
  per-frame budget is protected structurally by the **independent glyph damage
  gate** ([prepare.rs:157–216](../../../crates/buiy_core/src/render/prepare.rs)):
  steady-state text re-uploads nothing, a caret blink re-uploads only the
  glyph buffer, and a typing-latency fixture measures the edit→pixels path.
  Numbers stay with `buiy-verification-design`.
- **Gate #15 (leak) — the typing-churn fixture.** A scripted edit loop churns
  glyph keys through the atlas, then idles; entry count must return within ε
  of baseline (the atlas's LRU grace applies — the idle-settle window rules in
  [atlas-and-text-seam.md § 2.4](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#24-lru-eviction--the-gate-15-contract)).
- **Golden anchor caveat.** Today's goldens are captured on the local
  RX 6700 XT (RADV/Vulkan), not yet a canonical CI GPU class
  ([render verification.md § 4.1](../2026-06-03-buiy-render-pipeline-design/verification.md#41-canonical-ci-gpu-class)).
  Text goldens inherit that gap: treat the local set as the **seed set**, with
  tolerance budgets owned by `buiy-verification-design`, and expect a one-time
  re-capture when the canonical runner exists.
- **Effect-group ordering constraint — lifted (T8).** The glyph buffer is
  partitioned by group ranges since
  [2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md](../../plans/2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md);
  editable-text-in-effect-group fixtures are claimable
  ([decoration-and-paint.md § 4.5](decoration-and-paint.md#45-named-dependencies-not-owned-here),
  [follow-ups.md "glyphs bypass effect-group compositing — LANDED"](../../plans/follow-ups.md)).

The per-phase sequencing of these gates — which fixture lands with which
phase — is the campaign plan's, not this file's.

---

## 5. Prior-art errata ledger

Three prior-art claims are **superseded** by 0.19-verified facts (per the
docs-system convention: supersede, don't silently contradict). The prior-art
folder should be patched to point here as part of the campaign's closure phase.

| Stale claim | Where | 0.19 fact |
|---|---|---|
| `Editor::with_selection_bounds(\|rects\|)` is the selection-rect API | [prior-art/cosmic-text/editing.md § Selection rendering](../../prior-art/cosmic-text/editing.md), [lessons.md § Avoid](../../prior-art/cosmic-text/lessons.md) | No such method. The contract is `Editor::selection_bounds() -> Option<(Cursor, Cursor)>` + per-run `LayoutRun::highlight(start, end)` (src/buffer.rs:58–113 @0.19.0) |
| `FontSystem` is non-`Sync` ("pin to the UI thread or `Arc<Mutex<>>`") | [lessons.md § Validates, "FontSystem singleton"](../../prior-art/cosmic-text/lessons.md) | docs.rs 0.19.0 lists `impl Send for FontSystem` **and** `impl Sync for FontSystem`. The `SharedFontSystem(Arc<Mutex<FontSystem>>)` design stands on `&mut` access needs (shaping takes `&mut FontSystem`), not on a missing `Sync` ([architecture.md](architecture.md)) |
| `Buffer::set_text(font_system, …)` takes the font system | implied across prior-art code sketches | 0.19: `set_text(&mut self, text, attrs: &Attrs, shaping, alignment: Option<Align>)` — **no** `font_system`; setters are lazy and shaping defers to `shape_until_scroll(&mut FontSystem, prune)`. Citing the old shape is the classic drift error |

Also stale but already handled in-line: the capabilities.md "build the
selection sweep yourself" stance, superseded by `LayoutRun::highlight`
([decoration-and-paint.md § 5.1](decoration-and-paint.md#51-rectangles-via-layoutrunhighlight)).

---

## Sources

- cosmic-text 0.19.0 API — <https://docs.rs/cosmic-text/0.19.0/> (`FontSystem` `Send`/`Sync` impls, `new_with_locale_and_db` / `new_with_fonts`, `Buffer::set_text`, `Editor`) (verified 2026-06-09)
- cosmic-text 0.19.0 source — <https://github.com/pop-os/cosmic-text/tree/0.19.0> (`src/buffer.rs:58–113` `LayoutRun::highlight`)
- cosmic-text issues #499 (fontconfig aliases) and #505 (`FontSystem::new` startup cost) — <https://github.com/pop-os/cosmic-text/issues/499>, <https://github.com/pop-os/cosmic-text/issues/505>
