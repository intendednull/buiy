# Buiy Text T9: Verification Closure + Docs Flip — Implementation Plan

**Date:** 2026-06-11
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/verification.md](../specs/2026-06-09-buiy-text-rendering-design/verification.md) §§ 1.3, 3.3, 4, 5 + [architecture.md](../specs/2026-06-09-buiy-text-rendering-design/architecture.md) § 2.3 + [README.md](../specs/2026-06-09-buiy-text-rendering-design/README.md) (Status block + Open-questions ledger)
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T9, the FINAL phase (depends on all of T1–T8; the implementer starts from a branch with T1–T8 merged — T8 landed @ `12cb830`)
**Closes:** the text campaign. After this plan the campaign's phase table is all-landed, the spec Status flips from proposed, and the prior-art folders carry their owed correction pass.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal (the campaign T9 charter, realized):** golden suite expansion (widget ×
state × theme × viewport text fixtures); the gate-#15 typing-churn fixture
(edit loop → idle → atlas entry count returns within ε); decide the optional
ASCII pre-warm (architecture § 2.3, deferred from T4 — **decided here: REJECT**,
D1); CLAUDE.md + docs/README catalog updates; the prior-art errata patch
(verification § 5 ledger applied to `docs/prior-art/cosmic-text/` +
`bevy-cosmic-edit/`); follow-ups.md flip; spec Status flips from proposed; the
full two-lane suite + gate-#2/#14/#15 fixtures + the accept-curation workflow
exercised once end-to-end. T9 additionally consolidates the campaign plan's
accumulated **T1–T8 errata blocks into the spec files themselves** — every
erratum applied to its named section as an as-landed note (supersede, never
silently contradict), with the README ledger updated to match.

**Where T9 ends (honesty pins — this phase CLOSES, it does not build):**

- **ZERO new production features.** The one feature-shaped decision (the ASCII
  pre-warm) is decided **reject** (D1), so the only production-code touch in
  the entire phase is one stale doc comment in `golden.rs` (Task 7). If any
  task appears to need a production change, STOP — that contradicts the
  charter.
- **No stored-PNG `--accept` machinery.** `GoldenConfig.accept` stays a
  declared flag; pixel goldens stay inline + double-capture (the established
  discipline since the render GPU campaign deferred the stored-PNG machinery
  to `buiy-verification-design`). The curation workflow T9 exercises is the
  one that EXISTS: the `BUIY_ACCEPT_SHAPING` `.snap` flow (D4).
- **No editing-campaign work.** `TextEditState`, keymap, IME, undo, real
  selection/caret *drivers* are `buiy-text-editing`'s. T9's state fixtures
  author `CaretVisual`/`SelectionVisual` directly, the T7 test idiom.
- **Numbers stay with `buiy-verification-design`** — tolerance budgets,
  perf/leak thresholds, the canonical CI GPU class, wall-clock latency. T9
  commits mechanisms and exact structural counts (ε = 0 as-built, D2), never
  budget values.
- **The Status flip is qualified** (D7): the *rendering* surface is
  implemented (T1–T9); `editing-and-ime.md` remains target-state for the
  successor campaign. An unqualified "implemented" would misrepresent it.

**Tech stack:** existing workspace deps only. **No new dependencies, no
version bumps** (`cargo deny check` not required: no dep changes).

**Test reality:** Tasks 1–4 (the golden matrix) and Task 6 (GPU churn) are
`#[ignore]` GPU-lane tests built on `tests/support/mod.rs`. Task 5 (headless
churn) joins the every-PR gate on the adapterless `TextExtractHarness` — the
gate-#15 protection CI actually runs. Tasks 7–12 are docs (+ one comment),
verified by grep and by the unchanged two-lane gate.

---

## The gate (run BOTH lanes at every task boundary)

T9 adds GPU tests and one headless test; the per-task gate is the headless
gate **plus the GPU lane** (this host has the RX 6700 XT / RADV; Vulkan
render-to-texture needs no display):

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

Verified against the as-built tree at `12cb830` (T1–T8 landed).

### 1. The golden idiom as-built: inline + double-capture, no stored PNGs

The render GPU campaign deferred the stored-PNG `--accept` machinery ("image
dep, `tests/goldens/`, budget") to verification-design
(2026-06-07-render-gpu-verify-campaign.md Phase 3;
`tests/render_golden_harness.rs:50–54` pins the scope split). Every text
golden since T4 follows the inline discipline: **capture in a fresh app,
assert inline expected pixels, capture again in a second fresh app, assert
`perceptual_diff < 1e-4`** — "the re-capture IS the golden"
(`tests/text_gpu.rs:109–116`). `GoldenConfig.accept`
(`src/render/golden.rs:30–33`) is a declared flag with no machinery behind
it; `GoldenConfig::deterministic()` pins the triad with `accept: false`, and
`render_golden_harness.rs::accept_flag_routes_through_config` pins that
default headlessly. T9's matrix fixtures follow the same idiom exactly.

### 2. The one curated-update workflow as-built: `BUIY_ACCEPT_SHAPING`

The headless shaping snapshots (`tests/text_shaping_snapshots.rs`, T5) are
the project's real accept-curation flow: stored `.snap` files at
`crates/buiy_core/tests/fixtures/shaping/*.snap`, a labeled unified diff +
loud panic on divergence, and `BUIY_ACCEPT_SHAPING=1` as the explicit
regeneration gate with "REVIEW the generated file, and commit it" baked into
the failure message (`text_shaping_snapshots.rs:294–321`). This is what
"the `--accept` curation workflow exercised once end-to-end" means as-built
(D4); Task 12 exercises it.

### 3. Warmup + eviction semantics (the D1 evidence base)

- `warm_atlas` is satisfied **structurally** for text: the producer inserts
  at extract, before Prepare's upload and the node draw, so the first painted
  frame already has every visible glyph resident (glyph-pipeline § 6.4;
  `golden.rs:76–80` — whose comment still says the production ASCII pre-warm
  is "deferred — text campaign T9", flipped by Task 7).
- `AtlasWarmupQueue` (`render/atlas/warmup.rs`) has exactly ONE production
  producer as-built: T6's 1×1 solid-stamp push. `fonts_ready` requires the
  queue drained + every visible key resident via the no-LRU-touch
  `BuiyAtlas::get`.
- **Grace eviction is unconditional, not pressure-driven**: entries untouched
  for more than `eviction_grace` frames (default **60**,
  `atlas/types.rs:67–73`) are evicted by `maintain_atlas` regardless of page
  pressure (`atlas/lru.rs:45–51` `grace_expired`; pinned headless by
  `text_touch_pass.rs::offscreen_keys_drain_after_grace` and on the GPU by
  `text_gpu.rs::font_db_rebuild_storm_is_bounded`'s baseline return). Only
  keys in `ResidentTextKeys` get the per-frame touch. **Consequence: a
  startup ASCII pre-warm's unused entries drain ~1 s after startup.**
- T8's gate-#14 fixture proved edit→publish in **one frame including
  rasterize-on-miss** (`tests/text_typing_latency.rs`): the rasterize happens
  inside the same extract that publishes. A pre-warm can save only sub-frame
  CPU time, never a frame.

### 4. Harnesses + idioms (the seats for Tasks 1–6)

- **`TextExtractHarness`** (`tests/support/extract_harness.rs`): main `App`
  without `RenderPlugin` + a manual render `World` running `maintain_atlas`
  then `extract_buiy_glyphs` + change probes. Provides
  `with_atlas_config(AtlasConfig)`, `frame()`, `settle()`, `glyph_count()`,
  `changed_frames()`, `resident_keys()`, `atlas()` (→ `live_entry_count()`,
  `page_count(AtlasFormat)`, `get(&key)`).
- **GPU support** (`tests/support/mod.rs`): `gpu_render_app(w, h)` (primary
  window sized to the capture target — the view uniform derives from it),
  `render_to_image`, `spawn_capture_camera`, `finish_and_run`,
  `wait_for_text_ready`, `register_fixture_font`, `px`,
  `expected_full_coverage_srgb`, `readback_rgba`.
- **Files to mirror:** `text_gpu.rs` (capture/brightest/double-capture,
  atlas-count baselines, the rebuild-storm loop shape),
  `text_selection_caret_gpu.rs` (authored `CaretVisual`/`SelectionVisual`,
  `rows_where`/`cols_where`/`bands` ink-band asserts, theme-token injection),
  `text_decoration_gpu.rs` (decoration fixtures).
- **Scale-factor model:** glyph instance rects are LOGICAL (physical raster
  divided back by scale, `text/extract.rs:82–92`); the producer reads
  `Window::scale_factor()` from the primary window and value-compares it
  (`last_scale_factor`). Tests set scale via
  `Window::resolution` / `set_scale_factor` (the `text_extract.rs:61–75`
  idiom). For a GPU capture at scale ≠ 1.0 the capture image must match the
  window's PHYSICAL size (Task 4 verifies `physical_width()` before sizing
  the target).

### 5. Theme tokens available to fixtures (`src/theme.rs:62–86`)

`default_light_theme()` ships `color.surface.primary`/`secondary`,
`color.text.primary`/`secondary`/`placeholder`, `color.accent`,
`color.selection.bg`/`fg`; deliberately NO `color.caret` (T7 decision 7 —
caret-color: auto). Fixtures may insert extra test tokens (the `TOKEN`
idiom in `text_gpu.rs:26–34`).

### 6. What T8 already applied to the specs (Task 8 verifies, does not redo)

T8's own spec touches landed with T8: glyph-pipeline § 11.2 (entity_runs +
partition-at-prepare) and § 11.3 ("Since T8 this holds per region"),
decoration-and-paint § 4.5 (dependency lifted) and § 6.3 (the
`BufferUploadStats` GPU pin), verification § 1.3 rows (caret-blink damage,
glyph-in-effect-group = "Landed (T8)") and § 4 (effect-group constraint
lifted), follow-ups.md (glyphs-bypass entry → LANDED; degraded-groups entry
filed). **The T1–T7 errata blocks are NOT yet applied** — e.g.
decoration-and-paint § 6.3 still reads "`SelectionVisual` — the § 5 rect
list plus re-tint ranges" (T7 erratum 1), font-assets § 4 still names
`new_with_fonts` (T1 erratum 1). Task 8 is that consolidation.

### 7. The prior-art stale lines (Task 10's surgical targets)

Verified by grep at `12cb830`:

| File:line | Stale claim |
|---|---|
| `prior-art/cosmic-text/integration.md:15` | "Implements neither `Send` nor `Sync` ergonomically in practice" |
| `prior-art/cosmic-text/integration.md:47` | "`FontSystem` (which is non-Sync; the resource pins it to a thread)" |
| `prior-art/cosmic-text/critiques.md:45` | "`FontSystem` is non-`Sync` and non-`Clone`" (non-`Clone` holds; non-`Sync` is false in 0.19) |
| `prior-art/cosmic-text/lessons.md:17` | "The non-`Sync` constraint means pin it to the UI thread or wrap in `Arc<Mutex<>>`" |
| `prior-art/cosmic-text/lessons.md:38` | "`Editor::with_selection_bounds(\|rects\|)`" (no such method in 0.19) |
| `prior-art/cosmic-text/editing.md:60` | `with_selection_bounds` as THE selection-rect API |
| `prior-art/cosmic-text/editing.md:36` | "`Scroll { lines: i32 }`" (0.19: `Action::Scroll { pixels: f32 }`) |
| `prior-art/cosmic-text/editing.md:26` | the `Motion` variant list (0.19 has 22 variants; re-verify the list) |
| `prior-art/cosmic-text/README.md:56` | critiques summary repeats "FontSystem non-Sync" |
| `prior-art/cosmic-text/README.md` key-facts table | "swash 0.2.6" (the 0.19.0 lock resolves **0.2.8** — T4 erratum 5) |
| `prior-art/cosmic-text/capabilities.md` | the "build the selection sweep yourself" stance (superseded by `LayoutRun::highlight`) |
| `prior-art/bevy-cosmic-edit/architecture.md:113` | describes `with_selection_bounds` (historical description of the archived crate — note, don't rewrite, D6) |

Plus: grep both folders for code sketches passing a font system to
`set_text(` (the verification § 5 row-3 drift) and annotate each hit.

### 8. The docs surfaces to flip (Tasks 9 + 11)

- **Spec README** (`docs/specs/2026-06-09-buiy-text-rendering-design/README.md`):
  header `**Status:** proposed` (line 4) + the `## Status` section (which
  still ends "No code exists yet; `cosmic-text` is not in `Cargo.lock`") +
  the Open-questions ledger (annotations listed in Task 9).
- **docs/README.md**: spec row at line ~92 `[draft]`, campaign row at ~96
  `[draft]`, T1–T8 rows `[landed]`, no T9 row yet. Status vocabulary:
  `[draft]`/`[active]`/`[landed]`/`[superseded]`.
- **CLAUDE.md**: the GPU-lane paragraph already names "the text glyph
  producer" (line 57); the one-offs list (line ~79–82) lacks `hello_text`
  (the example EXISTS: `examples/hello_text`, in the workspace) and lacks
  the `BUIY_ACCEPT_SHAPING` regeneration command.
- **Campaign plan**: header `**Status:** proposed`; phase table T9 row
  `proposed`.
- **follow-ups.md**: both text entries already flipped/filed at T8 (the
  LANDED glyphs-bypass entry; the open degraded-groups entry, quad-path
  scope — NOT T9's). T9 *adds* two entries (pre-warm rejection, Task 7;
  stored-PNG golden machinery, Task 11).

### 9. The Phase-0 `Button` has no text and no state styling (drives D3)

`buiy_widgets::Button::new(label)` spawns marker + Node + Style + Background
token + Border + Focusable + A11yRole + `A11yLabel` — **no `Text` child, no
hover/pressed styling** (`crates/buiy_widgets/src/button.rs`; hover feeds
only `OnPress` emission). Growing it a rendered label is widget-catalog
work, a production feature this phase must not add.

---

## Decisions (with runner-ups) — read before implementing

**D1 — the ASCII pre-warm: REJECT (record-and-close, no code).** The
architecture § 2.3 what-to-warm commitment ("the ASCII range of the default
theme font at the primary window's scale factor") is **superseded by the
T4–T8 evidence**:

1. *Determinism never needed it.* Every golden T4–T8 passed with warmup
   satisfied structurally (Orientation § 3); glyph-pipeline § 6.4 predicted
   exactly this ("buys nothing for goldens") and the campaign confirmed it.
2. *The latency win is sub-frame and unmeasured.* T8's gate-#14 fixture
   proved one frame from edit to publish INCLUDING rasterize-on-miss; a
   pre-warm saves only intra-frame swash CPU, and no latency budget exists
   yet (numbers are `buiy-verification-design`'s).
3. *Structurally self-defeating as-built.* Grace eviction is unconditional
   (Orientation § 3): pre-warmed keys no visible text uses are never in
   `ResidentTextKeys`, get no touch, and drain `eviction_grace` (~1 s) after
   startup. A useful pre-warm therefore also needs pinning — and
   pin/refcount semantics were already rejected (glyph-pipeline § 6.3
   runner-ups: duplicates the LRU; `grace = ∞` breaks gate #15).
4. *The what-to-warm enumeration is unanchored.* "Default theme font" needs
   the theme font-token system (C-tier, not built) and a size enumeration
   that doesn't exist (`FontSize` is per-entity); § 6.4 flagged this
   coupling when rejecting the mandatory variant.

**Decision: reject for v1, the `shape-run-cache` precedent verbatim —
"revisit on measurement, not speculation."** The seam stays named
(`AtlasWarmupQueue` + T6's stamp push as the worked example); the re-open
trigger is a *measured* first-keystroke-latency miss against a
`buiy-verification-design` budget, and any revival must solve the
grace-drain problem (3) explicitly. **Runner-up rejected — land minimal**
(warm ASCII of the embedded default font at 16 px on startup): its entries
evict within ~1 s (3), its benefit is unmeasured sub-frame CPU (2), and it
hard-codes the size/family enumeration (4). Task 7 records this in
architecture § 2.3 (as-landed note), glyph-pipeline § 6.4, verification
§ 3.3, the `golden.rs:80` comment, the README OQ ledger (Task 9), and a
follow-ups.md entry carrying the re-open trigger.

**D2 — gate-#15 seat: headless mechanism + GPU end-to-end twin.**
verification § 1.3 seats "Atlas churn (gate #15)" on the GPU lane, but the
entry-count property is fully CPU-visible (`BuiyAtlas` is device-free; the
extract harness runs `maintain_atlas`) and § 1.1's own principle says lowest
layer wins — and CI **never runs the GPU lane**, so a GPU-only gate-#15
fixture would protect nothing per-PR. As-landed: the **headless churn
fixture is the gate** (Task 5, every-PR), and one lean GPU twin (Task 6)
re-asserts the § 1.3 pixels half — pixels byte-stable across the
churn-and-settle through the REAL upload/draw path, which the headless
harness cannot see. ε **= 0 as-built**: the churn loop ends on the baseline
string, so the resident key set — and with it `live_entry_count` — must
return exactly (the rebuild-storm precedent uses `assert_eq` for the same
reason); the spec's ε allowance is for future fixtures whose end state isn't
the start state. Task 8 records the seat split as a verification § 1.3
as-landed note (the T8 D7 precedent for gate #14). **Runner-ups rejected:**
GPU-only (per-PR blind), headless-only (the leak gate's pixel/upload half —
stale-UV corruption under churn — is GPU-observable only).

**D3 — the golden matrix: 4 fixtures, one per charter axis, all
fixture-assembled.** Widget = a button-shaped composite assembled in the
fixture (Background token + Border radius + `Text` label), because the real
`Button` has no label and styling it is widget-catalog scope (Orientation
§ 9). State = placeholder vs filled+selected+caret, authored through the
T7 state components — the placeholder tint has **no golden yet** (T7's GPU
set covers selection + blink only), so this is the highest-value state pair
that exists without editing-campaign machinery. Theme = one fixture (text +
underline via the `TextDecorations.color` token seat) captured under two
palettes, plus the swap-equals-cold-capture assertion. Viewport = the same
logical fixture at `scale_factor` 2.0 (physical re-raster, logical
geometry). **Runner-ups rejected:** grow `Button` a `Text` child (production
feature, forbidden); a real hover-state fixture (no hover styling exists to
capture); more multi-script GPU renders (the corpus is headless-pinned;
`text_gpu.rs` test (d) already proves the non-Latin pixels lane).

**D4 — "the `--accept` curation workflow exercised once" = the
`BUIY_ACCEPT_SHAPING` flow.** As-built there is no stored-PNG `--accept`
(Orientation §§ 1–2); the render campaign deferred it and T4 re-affirmed the
inline discipline. Inventing machinery to satisfy the phrase would violate
the no-new-machinery pin. Task 12 exercises the real flow end-to-end
(delete → loud failure → regenerate → byte-identical → restore); Task 8 adds
the verification § 4 as-landed note saying exactly this; Task 11 files the
stored-PNG machinery as a follow-ups entry owned by
`buiy-verification-design` (it becomes worth building when the canonical CI
GPU class exists, § 4.1).

**D5 — errata application convention.** Each campaign erratum lands at its
named spec section as a short appended note — `**As landed (T_n,
2026-06-11):** …` — that supersedes the sentence it corrects without
deleting the original reasoning (the docs-system supersede rule). Two
exceptions edit in place with a parenthetical: pure factual slips with no
design content (the swash `0.2.7`→`0.2.8` Sources line; compute-site line
numbers → system names). The campaign plan's errata blocks stay untouched
(they are the journey record; the spec is the target-state record).

**D6 — prior-art patch convention.** Surgical correction blockquotes at each
stale line — `> **Correction (text campaign T9, 2026-06-11):** verified
against cosmic-text 0.19 — … See
[text verification.md § 5](path)` — never rewriting the surrounding prose.
`bevy-cosmic-edit` describes an ARCHIVED crate's historical behavior: its
`with_selection_bounds` mention gets a "the 0.19 contract is…" note, not an
edit (history stays honest). Each folder README gains one dated line
recording the correction pass; verification § 5 gains "applied 2026-06-11
(T9)".

**D7 — the Status flip wording.** Spec README header →
`**Status:** implemented (rendering, T1–T9)`; the `## Status` section
records: rendering surface implemented by the campaign (link), the
GPU-verified two-lane proof, and **`editing-and-ime.md` remains
target-state** for `buiy-text-editing`. docs/README spec row → `[landed]`
with the summary amended to say editing/IME is the named successor. The
campaign plan header → `landed`. **Runner-up rejected:** flipping
editing-and-ime.md separately to its own status header — the folder has one
README-owned Status by convention; the qualification lives there.

---

## File structure

New files:

- `crates/buiy_core/tests/text_golden_suite_gpu.rs` — Tasks 1–4 (the
  gate-#2 matrix expansion; all `#[ignore]`).
- `crates/buiy_core/tests/text_typing_churn.rs` — Task 5 (headless,
  every-PR).

Touched files:

- `crates/buiy_core/tests/text_gpu.rs` — Task 6 (the GPU churn twin joins
  the atlas-lifecycle tests it mirrors).
- `crates/buiy_core/tests/support/mod.rs` — Task 4 (a scaled app builder).
- `crates/buiy_core/src/render/golden.rs` — Task 7 (ONE comment line).
- `docs/specs/2026-06-09-buiy-text-rendering-design/*.md` — Tasks 7–9.
- `docs/prior-art/cosmic-text/*`, `docs/prior-art/bevy-cosmic-edit/*` —
  Task 10.
- `CLAUDE.md`, `docs/README.md`, `docs/plans/follow-ups.md`,
  `docs/plans/2026-06-09-buiy-text-campaign.md` — Tasks 7 + 11.

---

## Tasks

### Task 1: The golden-suite file + the widget-card fixture (GPU)

The gate-#2 matrix's **widget** axis: themed text on a themed widget surface
— quad-under-glyph inside one card, both colors token-resolved.

- [ ] Create `crates/buiy_core/tests/text_golden_suite_gpu.rs` with the
  standard header (`//! Gate-#2 golden-suite expansion (campaign T9):
  widget × state × theme × viewport text fixtures, inline-golden discipline
  (the stored-PNG deferral stands — verification § 4 as-landed)…`),
  `mod support;`, and the run line
  `cargo test -p buiy_core --test text_golden_suite_gpu -- --ignored --test-threads=1`.
- [ ] Write `widget_card_text_is_deterministic_and_token_tinted`
  (`#[ignore = "needs a wgpu adapter; gate-#2 widget-axis golden (text campaign T9)"]`):
  - Fixture (a `capture_card()` helper returning `Vec<u8>`, the
    `text_gpu.rs::capture` shape): `gpu_render_app(W, H)` (e.g. 160×96);
    root column sized to the window; child card node
    `Style::default().width_px(120.0).height_px(32.0).padding(8.0)` +
    `Background { color: ColorToken::Token("color.surface.secondary") }` +
    `Border { radius: Corners::all(Radius::circular(6.0)), .. }`; card child
    `Text("Save")` + `FontSize(16.0)` +
    `TextColor(ColorToken::Token("color.text.primary"))`. All tokens ship in
    `default_light_theme()` (Orientation § 5) — no insertion needed.
  - Assertions: (a) backdrop pixel outside the card == opaque-black clear;
    (b) a card-interior pixel away from the label ≈ the resolved
    `color.surface.secondary` (linearize + `expected_full_coverage_srgb`,
    small ±tolerance — the `text_gpu.rs` TOL idiom); (c) dark glyph ink
    exists within the card rect (`rows_where`/`cols_where` band idiom from
    `text_selection_caret_gpu.rs`, predicate ≈ `color.text.primary`);
    (d) double-capture determinism: a second fresh `capture_card()` has
    `perceptual_diff < 1e-4`.
- [ ] Non-vacuity check: temporarily flip one expected channel by 32 and
  confirm the assertion fails for the right reason; revert.
- [ ] Run the new test on the GPU lane; run both gates.

### Task 2: The state pair — placeholder vs filled + selection + caret (GPU)

The **state** axis. The placeholder tint has no golden yet; the filled state
re-exercises the T7 primitives in one combined frame.

- [ ] In `text_golden_suite_gpu.rs`, write
  `input_state_pair_placeholder_vs_selected`
  (`#[ignore = "needs a wgpu adapter; gate-#2 state-axis golden pair (text campaign T9; decoration-and-paint §§ 5–7)"]`):
  - `capture_placeholder()`: a text node `Text("Search")` + `FontSize(20.0)`
    + `TextColor::placeholder()` under a sized column (the
    `color.text.placeholder` token ships in the default theme). No
    `CaretVisual`/`SelectionVisual` — a placeholder is never selectable
    (decoration-and-paint § 7).
  - `capture_filled()`: the same node with
    `TextColor(ColorToken::Token("color.text.primary"))`, plus an authored
    `SelectionVisual::new(start, end)` spanning the first ~3 clusters and an
    authored `CaretVisual { visible: true, rect }` — mirror
    `text_selection_caret_gpu.rs`'s construction (cursor/rect authoring is
    the fixture's job until `buiy-text-editing`; rect values may be derived
    from `ComputedTextLayout` exactly as that file does).
  - Assertions: (a) each capture double-capture deterministic
    (`perceptual_diff < 1e-4` against its own fresh re-capture); (b) the
    pair differs (`perceptual_diff(placeholder, filled) > 5e-4`); (c) the
    placeholder frame's brightest ink texel ≈ the placeholder grey
    (`expected_full_coverage_srgb` of the token, ± TOL) and contains **no**
    `color.selection.bg` pixels; (d) the filled frame contains a
    `color.selection.bg` band (`rows_where` + `bands`).
- [ ] Run the new test on the GPU lane; run both gates.

### Task 3: The theme pair — one fixture, two palettes (GPU)

The **theme** axis: text ink + decoration color both re-resolve, and the
live-swap path lands on the same pixels as a cold capture under the new
palette (the retint test's byte-identity, promoted to a full golden pair
that includes a decoration quad).

- [ ] In `text_golden_suite_gpu.rs`, write
  `themed_text_pair_and_swap_equivalence`
  (`#[ignore = "needs a wgpu adapter; gate-#2 theme-axis golden pair (text campaign T9)"]`):
  - One fixture fn parameterized by palette: insert test tokens `test.fg`
    and `test.deco` into the theme; spawn `Text("Theme")` + `FontSize(24.0)`
    + `TextColor(Token("test.fg"))` +
    `TextDecorations { line: UNDERLINE, color: Some(Token("test.deco")), .. }`
    (the `color: Option<ColorToken>` seat, `text/components.rs:366` — T6's
    tier-1 token).
  - Palette A (e.g. light grey ink / accent-blue underline) and palette B
    (clearly different hues).
  - Assertions: (a) fresh-A vs fresh-A `perceptual_diff < 1e-4`; (b) fresh-B
    differs from fresh-A (`> 5e-4`); (c) **swap-equals-cold**: in one app,
    capture under A, mutate the theme tokens to B, settle 3 frames,
    recapture — the swapped frame matches the fresh-B capture
    (`perceptual_diff < 1e-4`; same logical fixture, same fonts, only tokens
    moved — the `theme.is_changed()` re-emit path IS the cold path's
    pixels); (d) underline band present in both (a horizontal run of
    deco-colored pixels below the ink rows — `rows_where` idiom).
- [ ] Run the new test on the GPU lane; run both gates.

### Task 4: The viewport pair — scale factor 2.0 (GPU)

The **viewport** axis: same logical content, physical re-rasterization —
"shape logical, rasterize physical" proven end-to-end at the pixels lane.

- [ ] Add a scaled builder to `tests/support/mod.rs`:
  `gpu_render_app_scaled(logical_w, logical_h, scale_factor)` — identical
  plugin stack to `gpu_render_app`, with the primary window's resolution
  built from the logical size plus
  `with_scale_factor_override(scale_factor)`. Doc-comment the contract: the
  capture image must be sized to the window's **physical** size. **Verify at
  implementation** (Bevy's `WindowResolution::new` semantics): after app
  build, assert `window.physical_width() == logical_w * scale` inside the
  test before creating the target — if the constructor takes physical units,
  adjust the builder (and its doc) accordingly; do not guess.
- [ ] In `text_golden_suite_gpu.rs`, write
  `scaled_viewport_rerasterizes_at_physical_scale`
  (`#[ignore = "needs a wgpu adapter; gate-#2 viewport-axis golden (text campaign T9; glyph-pipeline §§ 3–5 end-to-end)"]`):
  - Capture the SAME logical fixture (`Text("Hi")`, `FontSize(20.0)`, token
    tint, sized column) twice: once via `gpu_render_app(W, H)` at scale 1.0,
    once via `gpu_render_app_scaled(W, H, 2.0)` with a `2W × 2H` capture
    image.
  - Assertions: (a) the scaled capture is double-capture deterministic;
    (b) ink present in both; (c) **the physical re-raster**: the maximum
    atlas cell height among `ResidentTextKeys` entries in the scaled app ≈
    2× the unscaled app's maximum (±1 px rounding) — read via
    `ResidentTextKeys` + `BuiyAtlas::get(key).px` from the `RenderApp`
    world, the `text_gpu.rs` resource-reading idiom; (d) the scaled frame's
    brightest texel still matches the tint (alpha-as-color invariant holds
    at scale).
- [ ] Run the new test on the GPU lane; run both gates (the headless gate
  proves the support addition compiles adapter-free).

### Task 5: Gate-#15 — the headless typing-churn fixture (every-PR)

The leak gate's mechanism, seated where CI can run it (D2): edit loop churns
disjoint glyph keys through the device-free atlas, idle drains back to the
exact baseline.

- [ ] Create `crates/buiy_core/tests/text_typing_churn.rs` (header: gate
  #15, verification §§ 1.3 + 4, D2 seat note; `mod support;`).
- [ ] Write `typing_churn_returns_atlas_to_baseline` (no `#[ignore]` —
  headless):
  - `TextExtractHarness::with_atlas_config(AtlasConfig { page_size: 1024,
    page_budget: 8, eviction_grace: 3 })` (the `text_touch_pass.rs` GRACE
    idiom). Spawn `Text("abc")` under a sized column; `settle()`.
  - Baseline: `live_entry_count()`, `page_count(AtlasFormat::CoverageR8)`,
    and the `resident_keys()` set.
  - The edit loop: ~8 frames, each setting `Text` to a string of glyphs
    **disjoint from the baseline and from each other** (e.g. `"dgq"`,
    `"hkx"`, `"mvz"`, `"rtw"`, `"ufy"`, `"jpn"`, `"els"`, then back to
    `"abc"` — comment why disjointness matters: each step inserts fresh
    keys, so the churn is real). One `h.frame()` per edit.
  - Mid-loop assert: `live_entry_count() > baseline_entries` (the fixture
    actually churned — non-vacuity built in).
  - Idle: `4 × GRACE` frames of `h.frame()` (the idle-settle window,
    atlas-and-text-seam § 2.4).
  - Assertions: `live_entry_count() == baseline_entries` (**ε = 0**, D2 —
    comment the rationale: the loop ends on the baseline string);
    `page_count == baseline_pages` (pages pooled, never leaked); the
    `resident_keys()` set equals the baseline set.
- [ ] Run `cargo test -p buiy_core --test text_typing_churn` headless; run
  both gates.

### Task 6: Gate-#15 — the GPU churn twin (pixels stable through real churn)

The § 1.3 row's pixels half (D2): the same churn through the REAL
rasterize → upload → draw path; the frame after settle is byte-stable
against the frame before churn, and the GPU-side counters return to
baseline. Mirrors `font_db_rebuild_storm_is_bounded` closely.

- [ ] In `tests/text_gpu.rs`, write `typing_churn_is_bounded_and_invisible`
  (`#[ignore = "needs a wgpu adapter; gate-#15 typing-churn fixture (verification §§ 1.3, 4)"]`):
  - `gpu_render_app(W, H)` + the tight-grace `AtlasConfig` override
    (`eviction_grace: 3`, inserted into the `RenderApp` world BEFORE any
    frame — the established pattern). Spawn the standard `"Hi"` fixture;
    capture `frame_before` + record `live_entry_count`/`page_count` from the
    `RenderApp` world.
  - The edit loop: mutate the main-world `Text` through the same
    disjoint-string sequence as Task 5 (ending back on `"Hi"`), one
    `app.update()` per edit; mid-loop assert entry growth.
  - Settle ~8 frames; assert `live_entry_count` and `page_count` equal the
    baseline (the rebuild-storm `assert_eq` idiom).
  - Readback `frame_after`; assert `perceptual_diff(frame_before,
    frame_after) < 1e-4` (the churn is invisible — same final text, same
    pixels; this is the stale-UV/upload-path half headless cannot see).
- [ ] Run on the GPU lane; run both gates.

### Task 7: The ASCII pre-warm decision, recorded (docs + one comment)

D1's reject, applied everywhere the deferral is named. Docs task —
verify-by-grep.

- [ ] `docs/specs/…/architecture.md` § 2.3: append the as-landed decision
  note — the what-to-warm commitment is superseded: **no production
  pre-warm ships** (rejected T9 as unmeasured; D1's four evidence lines in
  one paragraph: structural determinism, sub-frame-only win per the gate-#14
  fixture, unconditional grace eviction drains unused warm keys in ~1 s
  without a pin mechanism that § 6.3 already rejected, no theme-font/size
  enumeration exists to warm from); the seam stays `AtlasWarmupQueue` with
  T6's stamp as the worked example; re-open trigger = a measured
  first-keystroke-latency miss against a `buiy-verification-design` budget.
- [ ] `docs/specs/…/glyph-pipeline.md` § 6.4: append one line — the
  optional pre-warm was adjudicated at T9: rejected; link architecture
  § 2.3's as-landed note.
- [ ] `docs/specs/…/verification.md` § 3.3: annotate "the default font's
  ASCII range in production" — superseded by the T9 rejection (link); the
  fixture glyph set + the stamp remain the only pushes.
- [ ] `crates/buiy_core/src/render/golden.rs:80`: the comment
  "(deferred — text campaign T9)" → "(rejected — text campaign T9;
  architecture § 2.3)". No behavior change.
- [ ] `docs/plans/follow-ups.md`: add
  `## Text — production ASCII pre-warm (rejected as unmeasured)` —
  Originated: text campaign T9 (architecture § 2.3 deferral from T4);
  Status: rejected, not deferred; the re-open trigger + the grace-drain
  constraint any revival must solve; spec touchpoint architecture § 2.3.
- [ ] Verify by grep: `grep -rn "deferred — text campaign T9"
  crates/ docs/specs/` → no hits; `grep -n "rejected" docs/specs/…/architecture.md`
  shows the § 2.3 note.
- [ ] Run both gates (the comment edit must not disturb them).

### Task 8: Spec errata consolidation — the T1–T7 blocks into the section files

Apply every campaign-plan erratum to its named spec section per D5. For
each: read the current spec text FIRST (some sentences moved in review
rounds; T8's were pre-applied — Orientation § 6), then append the as-landed
note. The campaign plan blocks are the source text; do not paraphrase facts,
compress them.

- [ ] **font-assets.md**: T1.1 (§ 4 — `new_with_fonts` scans system fonts;
  registered-only = `new_with_locale_and_db_and_fallback`); T1.2 (§ 1 —
  "one direct dependency" + the `unicode_script`/`sys-locale` reality);
  T5.1 (§ 3.2 — in-place rebuilds KEEP fontdb IDs, fresh-db rebuilds reissue
  equal values → `FontDbLineage` + interner reseat); T5.3 (§§ 2–3 —
  `FontKey` dropped; declared family name is the registry identity); T5.4
  (§ 6 — `FontFallbackIter` verified public-but-internal; Buiy never
  constructs one); T5.5 (coverage via `with_face_data` + re-exported skrifa
  charmap — `unicode_codepoints()` is feature-gated off); T7.5 (§ 7 — caret
  + selection background paint under Block; selected-glyph re-tint inherits
  zero-alpha).
- [ ] **architecture.md**: T2.1 (§ 5.1 — the carrier-removal trigger gap:
  pin the exclusion or the `RemovedComponents` arms as-built; record which);
  T5.2 (§ 1.2 — rescope "exactly three lock sites" to steady-frame; list
  `swap_font_db` + `apply_font_registry` as rare-event sites); T3.1's
  § 5.1-row half (the TextCommit trigger summary defers to the as-built
  reconcile-guard sweep).
- [ ] **measure-and-layout.md**: T3.1 (§ 4.1 — the trigger union misses
  measure-touched buffers; as-built `height_opt = None` catch-all +
  reconcile guard); T3.2 (§ 2.3 — `TextBufferAccess` deferred to
  `buiy-text-editing`; direct `&mut TextBuffer` until then); T3.3 (§ 4.2 —
  height windowing: overflow-visible text cut at `height_opt`; the named
  overflow seam); T3.4 (compute-site references pinned to system names
  `taffy_compute`/`cq_flip_rerun`/`cq_descendant_rerun`, not line numbers —
  in-place edit per D5); T3.5 (§ 6 — baseline keys on GLYPHS; the synthetic
  empty-line run stays as geometry); T5.6 (§ 5.4 — marks prepended per
  NON-EMPTY line only; empty-line caret direction = editing-campaign seam).
- [ ] **glyph-pipeline.md**: T4.1 (§ 2 step 0/§ 6.1 — producer binds
  `&TextBuffer`; the T3.2 deferral cross-ref); T4.2 (§ 5.2 — bearings via
  the producer-owned `GlyphMetaCache`, not `AtlasEntry.px`); T4.3 (§ 2
  step 4 — residency probe + prebuilt-bitmap closure, lock lazy
  once-per-frame); T4.4 (§ 5.1 — content origin =
  `ComputedTextLayout.content_offset`); T4.5 (Sources — swash **0.2.8**,
  in-place edit); T4.6 + T7.2 (§ 6.2 — the T6/T7 union members are now
  LANDED carriers: drop the "do not exist yet" framing; record the
  value-compared per-carrier publish that reconciles § 6.2's wholesale
  rebuild with § 6.3's blink-damage property).
- [ ] **decoration-and-paint.md**: T6.1 (§ 3.2 — `DecorationSpan.color_opt`
  is the span TEXT color, tier 2; the `-color` property is the per-kind
  `*_color_opt` in `span.data`; as-built tier 1 = `TextDecorations.color`
  resolved at extract); T6.2 (§ 2.2 — the component carries
  `style: DecorationLineStyle`; § 9 realized as the enum); T6.3 (§ 4.3 —
  sampler is pinned Nearest; the stamp uv is the midpoint replicated); T6.4
  (tiers 1–2 structurally dormant; the pure mirror tests all three anyway);
  T7.1 (§ 6.3 — `SelectionVisual` is the normalized `(Cursor, Cursor)`
  endpoint pair, NOT "the § 5 rect list plus re-tint ranges"); T7.3 (§ 6.1 —
  `cursor_position() -> Option<(i32, i32)>` is not the painting input);
  T7.4 (§ 5.1 — the caller line-gate contract + the two `Editor::render`
  reference behaviors Buiy mirrors).
- [ ] **verification.md**: the § 1.3 churn-row as-landed note (the D2 seat
  split: headless mechanism = the every-PR gate, GPU twin = the pixels
  half); the § 4 "`--accept` curation" bullet as-landed note (D4: the
  as-built workflow is `BUIY_ACCEPT_SHAPING` over the `.snap` corpus;
  `GoldenConfig.accept` remains declared; pixel goldens are inline +
  double-capture; stored-PNG machinery filed in follow-ups, owned by
  `buiy-verification-design`); § 5 ledger header gains
  "applied 2026-06-11 (T9)" once Task 10 lands (sequence note: edit may
  trail Task 10 — fine, Task 11 re-verifies).
- [ ] Verify the T8 items are already present (Orientation § 6) — spot-check
  glyph-pipeline § 11.2/§ 11.3 and decoration § 4.5/§ 6.3; apply only what
  is missing.
- [ ] Verify by grep: `grep -c "As landed" docs/specs/2026-06-09-buiy-text-rendering-design/*.md`
  — every section file ≥ its task-list count above;
  `grep -n "0.2.7\|new_with_fonts\|rect list plus re-tint"` over the spec
  folder → only as-landed-note contexts remain (or zero hits where edited
  in place).
- [ ] Run both gates (docs only; `cargo doc` guards intra-doc links in
  `golden.rs` etc. are unaffected).

### Task 9: Spec README — Status flip + Open-questions ledger

- [ ] Header: `**Status:** proposed` → `**Status:** implemented (rendering,
  T1–T9)` (D7).
- [ ] `## Status` section: prepend the as-landed paragraph — rendering
  surface implemented by the campaign (link
  `2026-06-09-buiy-text-campaign.md`, T1–T9 all landed, two-lane proof per
  verification §§ 1, 4); `editing-and-ime.md` remains target-state for the
  named successor campaign `buiy-text-editing`; supersede (do not delete)
  the "No code exists yet; `cosmic-text` is not in `Cargo.lock`" sentence
  with a dated correction.
- [ ] Open-questions ledger annotations (italic resolution notes, the
  existing house style):
  - architecture OQ 1 (prior-art staleness) → *resolved: correction pass
    applied at T9 (Task 10; verification § 5 ledger).*
  - architecture OQ 2 (cross-layer interleave) → *carries forward* —
    still render-spec buckets/`painters_z` work; note T8 narrowed it
    (per-region quad-then-glyph holds).
  - measure OQs 1–4 → *carry forward* (resource-shape simplification,
    stretch keywords, multi-window, per-buffer memory) — annotate only if
    an erratum touched them (T3.2/T3.3 cross-refs).
  - font-assets OQ 1 → *stands as recorded* (the pin held through T1–T8).
  - glyph-pipeline OQs 1–2 → already annotated resolved; verify.
  - editing OQ 1 (edit→layout frame ordering) → *carries to
    `buiy-text-editing`*; note the T8 gate-#14 fixture pinned the
    display-path half (one frame, Update-mutation → publish).
  - editing OQ 2 (prior-art drift) → *resolved: T9 correction pass
    (Task 10).*
  - editing OQ 3 (arboard HTML) → *carries to the successor.*
  - editing OQ 4 (`TextBufferAccess`) → annotate the T3-erratum deferral
    (the accessor itself moved to the editing campaign).
  - decoration OQs 1–2 → already annotated resolved; verify.
  - synthesis OQ 1 (shape-run-cache) → already resolved; verify.
  - synthesis OQ 2 (ASCII warmup phasing) → *resolved at T9: production
    pre-warm rejected (architecture § 2.3 as-landed, D1); the stamp push
    (T6) was the campaign's only warmup producer.*
- [ ] Verify by grep: `grep -n "Status" docs/specs/2026-06-09-buiy-text-rendering-design/README.md`
  shows the flip; `grep -c "resolved" …/README.md` grew by the ledger
  updates; no remaining `No code exists yet` without a superseding note.
- [ ] Run both gates.

### Task 10: The prior-art errata patch (cosmic-text + bevy-cosmic-edit)

Apply the verification § 5 ledger + the editing-OQ-2 facts to every stale
line in Orientation § 7, per D6 (correction blockquotes, no rewrites).

- [ ] `cosmic-text/integration.md` (:15, :47): correction note — 0.19
  docs.rs lists `impl Send for FontSystem` AND `impl Sync for FontSystem`;
  the `Arc<Mutex<>>` pattern stands on the `&mut`-only API (shaping takes
  `&mut FontSystem`), not on missing marker traits; link verification § 5.
- [ ] `cosmic-text/critiques.md` (:45): correction note — non-`Clone` holds;
  non-`Sync` is false in 0.19; the serialization pressure is real but its
  cause is the `&mut` API.
- [ ] `cosmic-text/lessons.md` (:17 FontSystem-singleton row, :38
  selection-rendering row): correction notes — the singleton lesson stands
  on `&mut` access, not `Sync`; the selection contract is
  `Editor::selection_bounds() -> Option<(Cursor, Cursor)>` + per-run
  `LayoutRun::highlight(start, end)` (buffer.rs:58–113 @ 0.19.0).
- [ ] `cosmic-text/editing.md`: (:60) `with_selection_bounds` → the real
  pair, noting Buiy's producer-side line-gate finding (decoration-and-paint
  § 5.1 as-landed); (:36) `Scroll { lines: i32 }` → 0.19
  `Action::Scroll { pixels: f32 }`; (:26) re-verify the `Motion` list
  against 0.19's 22 variants and annotate the delta.
- [ ] `cosmic-text/capabilities.md`: the "build the selection sweep
  yourself" stance → superseded-by-`highlight` note (the ledger's
  "also stale" row).
- [ ] `cosmic-text/README.md`: (:56) reword the critiques summary line
  ("API friction (Buffer/Editor split, `&mut FontSystem` serialization,
  Attrs lifetimes)"); key-facts table swash row → 0.2.8-as-locked
  annotation; add one dated line: "2026-06-11 — correction pass (text
  campaign T9): 0.19-verified errata applied per
  [text verification.md § 5](../specs/2026-06-09-buiy-text-rendering-design/verification.md)".
- [ ] `grep -rn "set_text(" docs/prior-art/cosmic-text/ docs/prior-art/bevy-cosmic-edit/`
  — annotate every sketch passing a font system (the lazy-setter 0.19
  signature: `set_text(&mut self, text, attrs: &Attrs, shaping,
  alignment: Option<Align>)`).
- [ ] `bevy-cosmic-edit/architecture.md` (:113): the historical
  `with_selection_bounds` description gets the "0.19 contract is…" note
  (D6 — no rewrite of what the archived crate did);
  `bevy-cosmic-edit/README.md` gets the dated correction-pass line.
- [ ] Verify by grep: every Orientation § 7 line now has a `Correction
  (text campaign T9` within 5 lines; no UN-annotated `with_selection_bounds`
  or `non-Sync`/`non-\`Sync\`` claims remain in either folder.
- [ ] Run both gates.

### Task 11: CLAUDE.md + docs/README catalog + follow-ups + campaign flip

- [ ] **CLAUDE.md** one-offs (after the `hello_button` line): add
  `cargo run --example hello_text` — visual smoke test of the text stack;
  add the curated shaping-snapshot regeneration one-off:
  `BUIY_ACCEPT_SHAPING=1 cargo test -p buiy_core --test
  text_shaping_snapshots` — regenerate `.snap` shaping snapshots (curated:
  review the diff before committing). Verify the GPU-lane paragraph's text
  mention still covers the as-built surface; extend the parenthetical to
  "…the text pipeline (glyph producer, decorations, selection/caret, effect
  groups, golden suite)" if the lighter wording reads stale.
- [ ] **docs/README.md**: add the T9 plan row under the text plans
  (`[landed]`, 1–3 line summary: golden matrix, gate-#15 churn pair,
  pre-warm rejected, errata consolidation + prior-art patch, Status flip);
  flip the spec row `[draft]` → `[landed]` and amend its summary to name
  editing/IME as the `buiy-text-editing` successor; flip the campaign row
  `[draft]` → `[landed]`.
- [ ] **follow-ups.md**: add `## Render / verification — stored-PNG golden
  machinery (--accept)` — Originated: render GPU campaign Phase 3 deferral,
  carried by the text golden suite (inline + double-capture discipline);
  what exists (`GoldenConfig.accept` flag, `perceptual_diff`, the
  `BUIY_ACCEPT_SHAPING` `.snap` precedent), what's missing (image dep,
  `tests/goldens/`, per-fixture tolerance budgets); owner:
  `buiy-verification-design`, sensible once the canonical CI GPU class
  exists (render verification § 4.1). Verify the two existing text entries'
  states (glyphs-bypass `LANDED`; degraded-groups open, quad-path scope —
  untouched by T9) and the Task-7 pre-warm entry.
- [ ] **Campaign plan** (`2026-06-09-buiy-text-campaign.md`): phase-table T9
  row → `landed`; header `**Status:** proposed` → `landed`; append a short
  **T9 errata block** in the T9 section recording the two charter
  deviations as-landed (gate-#15 seated headless-+-GPU-twin per D2; the
  "`--accept` workflow" realized as `BUIY_ACCEPT_SHAPING` per D4 — the
  stored-PNG machinery was never built) plus the pre-warm outcome
  (rejected, D1).
- [ ] Verify by grep: `grep -n "t9-verification-closure" docs/README.md`;
  `grep -n "hello_text\|BUIY_ACCEPT_SHAPING" CLAUDE.md`;
  `grep -n "T9 | Verification closure" docs/plans/2026-06-09-buiy-text-campaign.md`
  shows `landed`.
- [ ] Run both gates.

### Task 12: Closure verification — the full two-lane suite + the curation workflow, exercised

The campaign's exit gate. Evidence before assertions
(superpowers:verification-before-completion).

- [ ] Run the FULL headless gate (fmt + clippy + doc + xvfb test) — green.
- [ ] Run the FULL GPU lane
  (`cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`) — green;
  capture the test-count summary in the task notes.
- [ ] **Gate-fixture inventory check** against verification § 4 — confirm by
  listing the test names: gate #2 = hello-text (`text_gpu` a) + one golden
  per decoration kind (`text_decoration_gpu`) + mixed-BiDi `::selection` +
  caret-blink pair (`text_selection_caret_gpu`) + multi-script render
  (`text_gpu` d) + the T9 matrix (`text_golden_suite_gpu`, 4 fixtures);
  gate #14 = `text_typing_latency` (headless) + the blink-reupload GPU pin;
  gate #15 = `text_typing_churn` (headless) + the GPU churn twin + the
  rebuild-storm bound. Any missing name is a STOP.
- [ ] **Exercise the curation workflow once end-to-end** (D4): delete one
  committed snapshot (e.g.
  `crates/buiy_core/tests/fixtures/shaping/latin.snap` — confirm the exact
  name via `text_shaping_snapshots.rs::snapshot_path`); run
  `cargo test -p buiy_core --test text_shaping_snapshots` and confirm the
  loud no-snapshot failure message names the regeneration command; run
  `BUIY_ACCEPT_SHAPING=1 cargo test -p buiy_core --test
  text_shaping_snapshots`; verify `git diff --exit-code` reports the
  regenerated file **byte-identical** to the committed one (deterministic
  corpus + pinned fonts — the workflow round-trips); confirm `git status`
  clean.
- [ ] Self-review against the campaign T9 charter, bullet by bullet
  (deliverable + test surface), and against this plan's honesty pins (zero
  production features — `git diff --stat` on `src/` shows only the
  `golden.rs` comment; no new deps — `Cargo.lock` untouched).
- [ ] Final both-lane gate run if anything moved since the last one.

---

## Execution order + dependencies

Tasks 1–4 are independent of 5–6; both groups precede the docs flips so the
Status flip describes reality. Task 7 is independent docs. Task 8 before
Task 9 (the ledger annotations reference applied errata). Task 10 before the
verification § 5 "applied" stamp is final (Task 8 notes the sequencing).
Task 11 after 7–10 (it records their outcomes). Task 12 last, always.

| Order | Task | Lane |
|---|---|---|
| 1 | 1–4 golden matrix | GPU |
| 2 | 5–6 gate-#15 churn pair | headless + GPU |
| 3 | 7 pre-warm decision | docs (+1 comment) |
| 4 | 8 spec errata consolidation | docs |
| 5 | 9 README Status + ledger | docs |
| 6 | 10 prior-art patch | docs |
| 7 | 11 CLAUDE.md / catalog / follow-ups / campaign | docs |
| 8 | 12 closure verification | both lanes |
