# Verification follow-ups campaign — design

**Date:** 2026-07-01
**Status:** active (spec — gated)
**Realizes:** closes the actionable `buiy_verify` follow-ups tracked in
[`docs/plans/follow-ups.md`](../plans/follow-ups.md) and the verification spec's
[`open-questions.md`](2026-06-15-buiy-verification-design/open-questions.md).
**Builds on:** the verification design
[`2026-06-15-buiy-verification-design/`](2026-06-15-buiy-verification-design/README.md)
(the landed five-tier pyramid).
**Implemented by:** [`docs/plans/2026-07-01-verification-followups-campaign.md`](../plans/2026-07-01-verification-followups-campaign.md).

## 1. Problem & thesis

`buiy_verify` — the five-tier visual-bug pyramid (layout snapshots → display-list
snapshots → proptest invariants → reftests → GPU goldens) — is landed and green.
What remains is a **scattered set of deferred/blocked follow-ups**, most annotated
back in June with "blocked on X" notes. Since then the text, editing, widget-catalog,
parity, render-GPU, perf, wasm, MVU and MT-safety campaigns all landed, so **many of
those blockers are stale**.

A triage pass (2026-07-01) re-verified **all 21** verification follow-ups against
`origin/main` (`f37c6fa`), reading live code rather than trusting the notes. This
campaign acts on the result: **build everything with genuine, non-redundant value;
prune the notes that describe already-landed work; and re-triage (not build) the
items proven speculative or externally blocked.**

Guiding rule (project + verification-design doctrine): *push detection to the lowest
tier that can observe the bug*, and **do not ship tested-but-unused machinery** — the
verification design's own deferral discipline (`open-questions.md` Decisions 3/6,
`goldens.md` Storage staging) is load-bearing here.

## 2. Scope decision — all 21 items

| id | title | verified status | decision |
|----|-------|-----------------|----------|
| **V14** | button coverage fixture is content-width-empty | open-doable-now | **BUILD** (W1) |
| **V13** | catalog-wide `content_is_present` enroll auto-check | open-doable-now | **BUILD** (W1) |
| **V10** | CPU SDF oracle ↔ shader DRY | open-doable-now | **BUILD** (W1) |
| **V17** | golden triage ignore primitives (tracking gap) | open-doable-now | **BUILD** (W1, doc-only part) |
| **V19** | retire adapter-brittle `is_white_ink` | open-needs-gpu-host | **BUILD** (W2) |
| **V1** | forced-colors BoxShadow visual reftest | open-needs-gpu-host | **BUILD** (W2) |
| **V4** | shadow-blur-kernel golden | open-needs-gpu-host | **BUILD** (W2) |
| **V6** | bless forced-colors-safe residue cells | open-needs-gpu-host | **BUILD** (W2) |
| **V8** | z-index invariant calls production producer | already-landed | **PRUNE** (W1) |
| **V9** | quiescence conditions 2–4 headless | already-landed | **PRUNE** (W1) |
| **V11** | rect-rounded lavapipe re-bless post wgpu29 | already-landed | **PRUNE** (W1) |
| **V12** | 0.19 lavapipe residue re-verify at next bump | trivial-note | **KEEP as-is** (W1, tidy only) |
| **V15** | PositionKind generator axis | already-landed | **PRUNE** (W1) |
| **V2** | multi-reference reftest aggregation | open-doable-now | **DEFER** (no consumer) |
| **V3** | golden-prune bin | open-doable-now | **DEFER** (no consumer) |
| **V16** | gallery matrix + AUTHORING.md + fixtures | open-doable-now | **DEFER → C8** (owned elsewhere) |
| **V18** | FLIP perceptual metric behind `flip` feature | open-doable-now | **DEFER** (precondition unmet) |
| **V5** | color-emoji golden | still-blocked | **CONFIRM-BLOCKED** (needs color-glyph render leg) |
| **V7** | object-store golden migration | still-blocked | **CONFIRM-BLOCKED** (corpus 590 B ≪ 50 MB trigger) |
| **V20** | contrast linter exhaustive walk | still-blocked | **CONFIRM-BLOCKED** (needs typed theme tokens) |
| **V21** | multi-root / cross-window invariant | still-blocked | **CONFIRM-BLOCKED** (needs per-window layout) |
| **H1** | render-test hygiene (added by the spec gate) | verification-adjacent | **BUILD** (W1) |

**BUILD = 8 + H1, PRUNE/KEEP = 5, DEFER = 4, CONFIRM-BLOCKED = 4.** Every non-BUILD
item still gets a tracker edit (its deliverable is *tracker truth*, not code).

**H1 — render-test hygiene** (both evidence + scope reviewers surfaced this; tracked in
`follow-ups.md` ~L1562). Two doable-now, headless, `buiy_core`-side test cosmetics a
maximalist "everything on verification" reading expects: (a) the stale test name
`clip_aabb_pipeline_registers_with_stride_52` in `crates/buiy_core/tests/render/render_smoke.rs`
whose asserted stride is now **68** (rename + confirm the constant), and (b) three `.snap`
files carrying a stale `source: tests/render_instance.rs` header. These sit just outside
the strict `buiy_verify` boundary but are verification-adjacent and adjacent to V10 (which
already touches `render_instance.rs`), so they ride W1. *Verify the "68" empirically before
renaming.*

## 3. What each BUILD item is (target state + rejected alternative)

### W1 — Headless (PR #1: fully CI-verifiable without a GPU)

**V14 + V13 — one slice: make the shared `build_app` text-capable.** These two were
specced separately; the spec gate proved they are **one change** (and the original
"separate stack" framing rested on a false premise — see the design note below).

The **only** CPU-snapshot coverage fixture (`fixtures/button/resting.rs`) renders a
`16×48` box around a **`0×0` label** — it passes green while testing nothing about text
measurement. Root cause: `build_app` (`coverage/enroll.rs`) builds only `MinimalPlugins
+ CorePlugin + LayoutPlugin`, and **neither inserts a `FontRegistry` nor a
`SharedFontSystem`** — both come only from `BuiyTextPlugin`. Without `SharedFontSystem`
the Taffy text-measure takes its no-op arm (`text/measure.rs`) and the label can never
measure nonzero, no matter what font is "registered."

**The fix (V14):** add `BuiyTextPlugin { system_fonts: false, .. }` to the shared
`build_app` and make **Ahem the sole resolvable family** (mirroring the GPU capture
stack's `FontMode::Ahem` determinism discipline, `determinism.rs`), then stage the
deterministic Ahem font via `determinism::stage_ahem` (which registers the bytes with **no**
`app.update()`, preserving `build_app`'s "no update yet" contract) and point the fixture
label at `determinism::AHEM_FAMILY` ("Ahem"). The button label then measures, and
the `button.resting.*` layout + display-list snapshots re-bless (label `0×0` → measured;
the button may re-center its child). **Treat the rebless count as expected≈12,
verify-empirically** — the exact set is governed by `paints_cell`/`snapshots_cell`, not
pre-committed.

**V13 rides the same stack — no separate `build_text_app` needed.** Once `build_app` is
text-capable, add the `enroll_content_presence` driver + a catalog-wide test in the
existing `tests/verify_headless/content_presence.rs` iterating `sorted_catalog() ×
Matrix::cpu_snapshots`, updating once (TextSync→measure→commit), asserting
`content_is_present` for every `scene_is_text_bearing` cell. `content_is_present` +
`glyph_census` + the bless-guard already exist and are tested; they needed exactly the
`SharedFontSystem` that `build_app` now has. After V14 the button fixture *is*
text-bearing, so this check has real teeth (non-vacuous) with the single existing fixture
— it does **not** depend on the deferred gallery fixtures (V16).

*Why unify (not a separate stack):* the Tier-2 display-list dump reads **only**
`ResolvedLayout + Background` boxes (`snapshot.rs`), no glyph rows — so the text
pipeline's *sole* effect on the structured snapshots is the label's measured box, which
*is* V14's intended rebless. With one fixture in the catalog, "perturb every baseline"
is that one fixture's cells regardless. Adapter-safe: `BuiyTextPlugin` is CPU cosmic-text
shaping; its render-world hook is skipped without a `RenderApp`, so `build_app`'s
pure-CPU/no-adapter invariant holds.
*Rejected:* (a) a hardcoded `Style` width for V14 — violates the real-`button()`-bundle
fixture philosophy and exercises no measurement; (b) a separate `build_text_app` for V13
— the spec-gate refuted its justification (there is no measure-only text path; the Taffy
measure *is* cosmic-text shaping gated on `SharedFontSystem`), and it adds a parallel
stack for a check the unified `build_app` serves directly. **Blast-radius check the plan
must run:** every test that enrolls the button via `build_app` (layout, display-list,
dpr-invariance, invariants) — confirm the property tiers still hold and only the
snapshot tiers rebless.

**V10 — SDF oracle DRY.** Hoist one canonical `pub fn sdf_rounded_rect` into `buiy_core`'s
render module and replace the **three** duplicate Rust copies (`reftest.rs:261`,
`tests/render/render_instance.rs`, `tests/render/render_border_sdf.rs`) with imports, so
the ports can't silently diverge. *Explicitly low residual value* (the real Rust↔WGSL
numeric drift is un-catchable headless and already covered by the CI lavapipe
cross-check; the 2026-06-18 audit downgraded this to a DRY nit) — we do the DRY, we do
**not** add a fragile WGSL-text-twin guard test (net-negative maintenance).
*Rejected:* the "string-extract the WGSL body and compare" guard — brittle, catches
nothing the lavapipe lane doesn't.

**V17 (part 1 only) — record the ignore primitives.** The do-now, high-value half is a
single doc edit: record *time-boxed-ignore* and *flaky-auto-ignore* in the
deferred-golden-primitives list of `follow-ups.md`, closing the Task-4.7 tracking gap.
The **machinery is deferred** (correctly — it needs a larger corpus and CI failure-history
persistence that does not exist).

### W2 — GPU host (PR #2: needs a real wgpu adapter; RX 6700 XT is available here)

**V19 — adapter-robust ink detection.** Replace the **five** absolute-`>=180`-family ink
predicates across three GPU text tests (`is_white_ink` in all three, plus `is_blue_ink` in
`text_selection_caret_gpu.rs` and the colored-ink predicate in `text_ime_preedit_gpu.rs`)
with a shared **channel-parametric** detector ("channel *c* meaningfully above the measured
black background"), so per-color discrimination (blue selection vs. white ink) survives — a
scalar brightness test would collapse it. `>=180` assumes lavapipe's coverage/gamma; a different rasterizer paints the
white ink dimmer, so `cols_where()` returns empty and `.expect("glyph ink painted")`
**panics on the RX today** (a live failing test). Each test's *semantic geometric*
assertion (caret right of ink; selection bands; preedit ink present) is preserved.
*Rejected:* the note's "route through `buiy_verify::golden` multi-positive corpus" —
a whole-frame golden over-captures for a geometric caret-position test and loses the
semantic content; push back on that framing.

**V1 — forced-colors BoxShadow visual reftest.** The named blocker (unlanded BoxShadow
extract/draw) is **false** on `origin/main` (`resolve_shadows` wired `extract.rs:1005`,
`shadow.wgsl` landed, forced-colors suppression returns empty). Replace the assertion-free
`boxshadow_visual_reftest_is_blocked` placeholder with a real Tier-4 reftest:
`test` = bordered box + BoxShadow under `prefs.forced_colors=true`; `reference` = same box,
no shadow; `reftest!(match, …)` — the forced-colors draw-skip means they must rasterize
identically. Reftests are **adapter-agnostic** (variance cancels in-process), so this is
verifiable on the RX without lavapipe. *Value: low* (the draw-skip is already observed
headless at Tiers 1–2) — belt-and-suspenders that closes the matrixed forced-colors cell.

**V4 — shadow-blur-kernel golden.** Add a `box_shadow` fixture + `golden_shadow_blur_kernel`
test in `tests/verify_gpu/goldens.rs` modeled on `golden_sdf_corner` (deterministic
capture, non-vacuous paint check, adapter-gated `assert_golden` under
`on_pinned_lavapipe()`), bless one PNG on **pinned lavapipe**, commit under
`tests/goldens/shadow/`. Adds the only pixel-level coverage of `shadow.wgsl`'s Gaussian
AA falloff (today only an algebraic CPU oracle + an adapter-tolerant "region darkens"
check). *Rejected:* blessing on the RX — the corpus is lavapipe-pinned; an RX-blessed PNG
would adapter-skip in CI and gate nothing.

**V6 — bless forced-colors-safe residue cells.** Make `matrix_goldens` honor the fixture
paintability predicate (skip `!fx.snapshots_cell(&cell)` in both assert and BUIY_BLESS
paths, mirroring the CPU-tier skip) so blessing can never bake the magenta light-theme
sentinel; then bless the **12** `theme==ForcedColors` button PNGs on lavapipe. Flips
`matrix_goldens` from `asserted==0` (aspirational) to `asserted>0`. The 12 *light-theme*
cells stay skip-as-pending until the default widget is forced-colors-safe (owned by
widget-catalog, not here). *Rejected:* blessing all 24 cells now — 12 render the sentinel
because `button.rs:94` still uses the brand `color.surface.secondary` token.

## 4. Wave / PR / merge structure

- **Wave 1 (PR #1) — headless hardening + tracker truth.** V14+V13 (one slice), V10, H1,
  V17-part1, all PRUNE/KEEP/DEFER/CONFIRM-BLOCKED **doc edits**, and the **`docs/README.md`
  index entries** for this spec + the plan (per `organizing-buiy-docs`). Verifiable entirely
  by the headless gate (`cargo test --workspace`, no `--ignored`). This PR makes the tracker
  honest in one pass.
- **Wave 2 (PR #2) — GPU verification.** V19, V1 (RX-verifiable), V4, V6 (need pinned
  lavapipe blessing). Each ships its own tracker flip. Verified on the GPU `--ignored`
  lane (both `buiy_core` and `buiy_verify` legs).

Two PRs because the fault line is real: W1 is CI-provable without hardware; W2 needs the
GPU lane and — for V4/V6 — a **pinned-lavapipe** blessing host. Splitting keeps W1
mergeable immediately and isolates the lavapipe-pinning risk to W2.

**Lavapipe wrinkle (W2, V4+V6):** this host has an RX 6700 XT (RADV), not lavapipe. The
committed goldens must be lavapipe-blessed to gate in CI. Plan: reconstruct the CI-pinned
lavapipe (Mesa 24.3.4 per `install-mesa`) locally to bless, exactly as the widget-catalog
campaign did. If pinned lavapipe can't be reproduced here, V4/V6 code + the RX-side
self-validation land, but the blessed-PNG commit is flagged for the CI GPU host — **this
is the one place human/CI attention may be warranted**; V19+V1 are unaffected.

## 5. Verification strategy (per the project's own pipeline)

- **W1:** the full headless gate — `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo doc -D warnings`, `cargo test --workspace` (nextest). V14's rebless is reviewed as
  a *diff* (label `0×0` → measured; no other field moves). V13's auto-check is proven
  **non-vacuous** (it must RED if the button label is blanked — a RED-first check).
- **W2:** the GPU `--ignored` lane on the RX for V19 (the panic must be gone and the
  semantic asserts hold) and V1 (reftest passes). V4/V6 self-validate on the RX (renders,
  double-capture stable) and are blessed/compared on pinned lavapipe.
- **Gates:** fresh-context review after this spec, after the plan, and after each wave —
  logic/correctness/spec-alignment, and **run**, not just read (`verification-before-completion`).
- Fan-outs run under `reliable-agent-fleet` (contract every agent, count returns, retry holes).

## 6. Non-goals (deferred / blocked — with why, so the tracker is honest)

- **V2, V3, V18 — deferred (no consumer).** All three are *buildable headless today* but
  ship tested-but-unused machinery: V2 (multi-ref reftest) has no ≥2-reference pairing to
  consume it; V3 (golden-prune) prints "nothing to prune" at 2 cells; V18 (FLIP) adds C++ FFI
  surface for a metric whose precondition (pixelmatch budgets proving insufficient) is unmet.
  The verification design's deferral discipline forbids building them now. Tracker gets a
  dated "re-verified 2026-07-01, still no consumer — defer" note. *If the user later elects
  to build them anyway despite no consumer, the spec-gate gradation is: **V3** is the only
  benign one (advisory pure-Rust bin, can't rot vacuously); **V2** is the most dangerous
  (a multi-reference reftest with no real pairing is a permanently-vacuous green in the
  highest-risk tier — the exact anti-pattern the design's Decision 1 fights); **V18** should
  stay deferred regardless (adds `nv-flip-sys` C++ FFI + CI build cost for an
  empirically-unneeded metric).*
- **V16 — deferred to the widget-catalog C8 child.** `Matrix::gallery_screen()`, the
  gallery-screens-as-fixtures cross-crate enrollment, and `AUTHORING.md` are **designed C8
  deliverables** (`widget-gallery-exemplar.md` §3.3/§3.5/§4/build-step 7, coordinated with
  C7), not verification-harness follow-ups, and are heavily redundant with the gallery's
  existing hand-written layout/inspector/interaction gates. Building them here would fork
  C8's work and ship an unused `Matrix` constructor. Its one campaign-native piece — the
  text-capable enroll — is delivered by **V13**. *Caveat (spec-gate):* the `buiy_gallery`
  crate **already landed on main**, so V16 is buildable-now, not dependency-blocked — it is
  a scope/ownership defer, not a technical one. To keep "→ C8" from becoming a black hole,
  the tracker edit points V16 at the **live C8 entry** in `follow-ups.md` (the "Gallery
  authoring guide + matrix enrollment" item, ~L1644), not just "deferred."
- **V5, V7, V20, V21 — blocked (dependency re-confirmed unmet on `f37c6fa`, 2026-07-01).**
  V5 needs the whole **color-glyph** render leg (`text/extract.rs:892` still maps
  `SwashContent::Color → SkipColorEmoji`; the **color** `IconInstance` atlas leg is unwired
  — note a *vector-icon* coverage producer/draw does exist, so it is specifically the color
  bitmap path that is missing; and no COLR/CBDT font is bundled — only monochrome NotoEmoji). V7's trigger (>50 MB or
  >500 positives) is unmet (corpus 590 B / 2 positives). V20 needs typed theme tokens
  (`Theme.colors` still `HashMap<String,Color>`; no `buiy-theme-tokens-design`). V21 needs
  per-window layout (`LayoutTree` still a single global tree; no `buiy-window-and-surface-design`).
  Each tracker note is refreshed with the dated re-confirmation + evidence.

## 7. Rejected campaign-level alternatives

- **One mega-PR.** Rejected: mixes hardware-gated (GPU-lavapipe) work with headless work,
  blocking the immediately-mergeable half behind the lavapipe wrinkle.
- **Build the speculative trio (V2/V3/V18) "for completeness".** Rejected on the project's
  own "speculative no" rule and the verification design's explicit deferral gates — dead
  code is a maintenance liability, not coverage.
- **Do V16 here.** Rejected as scope-creep into the widget-catalog campaign (§6).
