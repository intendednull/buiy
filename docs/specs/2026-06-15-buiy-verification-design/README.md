# Buiy verification design — the visual-bug-detection pyramid

**Date:** 2026-06-15
**Status:** draft
**Realizes:** the strategy report [`reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md) (five-tier pyramid, reftests-first) and the foundation verification gates #2 (visual), #5 (layout snapshots), #11 (forced-colors), #12 (proptest invariants) in `specs/2026-05-07-buiy-foundation/verification.md`.

## Thesis

Detect visual bugs with a **five-tier pyramid, reftests-first** — push detection *down* to the cheapest, most-deterministic rung that can see the bug, so the flaky, expensive pixel tier shrinks to the irreducible rasterization residue. Layout-number snapshots (Tier 1) and holistic CPU display-list/paint-order snapshots (Tier 2) catch geometry and composition bugs with zero GPU; proptest invariants (Tier 3) cover relational properties over an unbounded scene space with no oracle; **reftests (Tier 4)** — render a feature two equivalent ways in one process and assert `==`/`!=`, all platform variance cancelling — are Buiy's highest-leverage pixel mechanism and the one wholly absent from the tree; and stored goldens (Tier 5) are demoted to only what no feature-free reference can reach (SDF AA beyond the CPU cross-check, shadow kernel, glyph/color-emoji fidelity, blend/gamma). The whole stack rides one AA-aware two-axis perceptual metric and one deterministic-capture builder, both built on the landed `GoldenConfig` flake triad and the existing headless capture path — the renderer and capture already exist; what is new is the corpus discipline, the relational tiers, and the unified metric.

## Architecture

**Crate boundary.** The harness has a pure half and a GPU half, split by what each needs:

- **`buiy_verify`** (depends on `buiy_core + bevy + image + proptest + serde`, adds `insta`, a perceptual-metric crate, `inventory`, `toml`, `base64`) is the harness home. It holds everything app-independent: the metric, the snapshot dump formatters, the proptest generators/predicates, the reftest pairing/aggregation logic, the golden persistence + triage, the `DeterministicApp` builder, and the coverage matrix.
- **`buiy_core::render::golden`** holds the *device-coupled* capture only. The shared seam is promoted out of `crates/buiy_core/tests/support/mod.rs` into `render/golden.rs` src as `capture_to_image(&mut App, &GoldenConfig) -> image::RgbaImage`, callable by `buiy_verify`. `buiy_core` **cannot** depend on `buiy_verify` (the harness depends on core, not the reverse), so the naive `perceptual_diff` (L1) is *deprecated in place* and its callers migrate outward to `buiy_verify::metric`.

**`buiy_verify` module layout** (one module per concern, each its own child file):

```
buiy_verify::
  metric        — AA-aware two-axis perceptual diff (the shared primitive for tiers 4 & 5)
  snapshot      — Tier 1 layout-number + Tier 2 display-list/paint-order dumps (insta)
  invariant     — Tier 3 proptest scene generators + predicate fns (no oracle)
  reftest       — Tier 4 RefCase / reftest! / run_reftest + the CPU-vs-GPU SDF cross-check
  golden        — Tier 5 assert_golden persistence, multi-positive corpus, HTML triage
  determinism   — DeterministicApp builder, GoldenConfig extensions, lavapipe CI pin
  coverage      — Matrix Cartesian product over BSN fixtures, auto-enrolling every tier
```

**Crate-dependency note.** The only new GPU dependency `buiy_core` gains is `image = "0.25"` (already a workspace dep) for `capture_to_image`. Every other new crate (`insta`, the perceptual-metric crate, `inventory`, `toml`, `base64`) lands in `buiy_verify` and is gated by `cargo deny check` before it merges (CLAUDE.md supply-chain check; `deny.toml` is allow-list-only, so a new transitive license fails CI by design and must be added explicitly, never via an exception hack).

## Tier table

| Tier | What it catches | Where it lives | Headless / GPU | Foundation gate(s) |
|---|---|---|---|---|
| **1 — layout-number snapshots** | geometry: wrong size/position, intrinsic-size, container-query math, Taffy-bridge bugs | `buiy_verify::snapshot` (`assert_layout_snapshot`) over `ResolvedLayout` | **Headless** (pure CPU) | **#5** |
| **2 — display-list / paint-order snapshots** | composition: paint *order* (z-sort, tooltip-behind-modal), paint *set* (cull, missing-token sentinel), paint *params* (wrong token/radius/transform), stacking-context formation; + forced-colors token flow | `buiy_verify::snapshot` (`assert_display_list_snapshot` + `PackedInstance` byte-hex) over `ExtractedNodes`/`InstanceBuckets` | **Headless** (pure CPU) | **#11** (forced-colors token flow, via `coverage`) |
| **3 — metamorphic / property invariants** | relations with no oracle: paint-order totality, transform round-trips, top-layer dominance, finiteness, BiDi caret round-trip | `buiy_verify::invariant` (proptest generators + predicate fns) | **Headless** (pure CPU) | **#12** |
| **4 — reftests + CPU-vs-GPU cross-check** | the CSS-subset surface relationally: flex/grid↔absolute, container queries, anchors, content-visibility, writing modes, transforms, stacking, clipping, forced-colors visual residual; SDF corner AA (via the CPU oracle) | `buiy_verify::reftest` (`RefCase`/`reftest!`/`run_reftest`), capture via `buiy_core` | **GPU** (`#[ignore]`) | **#2** (relational half) |
| **5 — golden / screenshot regression** | true rasterization only: SDF AA, shadow blur kernel, glyph/color-emoji atlas, effect compositor, blend/gamma, forced-colors `BoxShadow` draw-skip | `buiy_verify::golden` (`assert_golden`, stored `tests/goldens/` corpus), capture via `buiy_core` | **GPU** (`#[ignore]`) | **#2** (residue half) |

Cross-cutting: **`metric`** is shared by tiers 4 and 5; **`determinism`** (the `DeterministicApp` builder + lavapipe CI pin) underpins tiers 4 and 5 and realizes the source-of-truth half of gate #2's determinism requirement; **`coverage`** Cartesian-products every fixture across all five tiers and wires the live-catalog half of gate #11.

## Verification of the harness

The harness is load-bearing test infrastructure, so each tier carries its **own** non-snapshot meta-tests proving it tests what it claims — a property suite that never fails is worthless, a snapshot that passes vacuously is worse than none. The discipline, per child file:

- **metric** (`metric.md` § Verification): known-answer unit tests — identity ⇒ zero diff; a single wrong-by-200 pixel is caught at *every* frame size N ∈ {16, 256, 2048} (the exact §4 regression L1/RMSE fail); AA on/off pins the sibling test; two-axis independence proves both axes bind; dimension/empty ⇒ explicit `Err` (not the silent `1.0`). Pure CPU, no GPU lane.
- **snapshot** (`snapshots.md` § Verification): the dump is `assert_eq!`-equal across two apps spawned in *different entity order* (proves `Name`-keyed order-invariance, a plain assert so it cannot pass vacuously); the hex round-trips bytes; a format-version tripwire forces a conscious re-bless; each migration is checked behavior-preserving against the old per-field asserts (the half-size sign regression must still fail).
- **invariant** (`invariants.md` § Verification): **mutation fixtures** — a hand-built scene that violates exactly one relation must be rejected, plus a known-good control that passes (the Tier-3 analogue of the half-size sign-bug regression). The `top_layer_dominates` mutation fixture also pins deviation #3: it fails if anyone "fixes" the predicate to compare the enum discriminant.
- **reftest** (`reftests.md` § Verification): an aggregation truth table on stub `Diff`s (no GPU); a `match` of a scene with *itself* passes at `(0,0)` while a `match` of two different scenes *fails* (proves the harness can fail — guards a vacuous green); the independence lint is itself RED/GREEN-tested.
- **golden** (`goldens.md` § Verification): in-memory match/mismatch + multi-positive + bless round-trip + fail-closed-on-empty-corpus + report self-containment, all pure CPU; one end-to-end golden per residue class on the GPU lane.
- **determinism** (`determinism.md` § Verification): idempotent-capture (two fresh apps diff at `(0,0)`) *and* knob-sensitivity negatives (flipping DPR / font-mode / MSAA *changes* the bytes — proves the knobs are load-bearing, not no-ops); quiescence assertions fire on an injected never-loading asset.
- **coverage** (`coverage.md` § Verification): `catalog()` (inventory) and the `glob!` walk enumerate the identical set; every `CoverageKey::stem()` is unique and round-trips; a deliberately-broken fixture must produce a forced-colors violation through the *live* producer (proving it observes real paint, not a stale descriptor).

All headless meta-tests run under `cargo test --workspace` with **no** `--ignored`; the GPU meta-tests are `#[ignore]` on the real-adapter lane (`cargo test -- --ignored --test-threads=1`).

## Build order

The phasing belongs to the plan, not this spec; the **priority** is fixed by the report roadmap and is reftests-first:

1. **metric + reftests** — the unified two-axis AA-aware metric (replacing the L1 `perceptual_diff` and the RMSE `compare_images`) and the `reftest!` harness + CPU-vs-GPU SDF cross-check on the existing capture path. Highest leverage; zero golden storage; both unblock the most.
2. **snapshots + invariants** — add `insta`, the `Display` dump formatters + `PackedInstance` byte-hex, and the proptest generators/predicates. Pure-CPU, deterministic, closes gate #5 and #12 and adds the missing Tier-2 display-list gate.
3. **goldens + determinism** — the stored-PNG corpus + `BUIY_BLESS` persistence, per-fixture fuzz budgets, the Ahem layout-determinism mode, and the lavapipe CI rasterizer pin below the landed flake triad. Smallest tier, deliberately last.
4. **coverage** — the BSN-catalog → (theme × viewport × forced-colors × DPR) Cartesian matrix that auto-enrolls new fixtures across all tiers, and the re-point of `forced_colors_analyzer` to the live catalog (gate #11).

The v1 CI gate is steps 1–2 (reftests + metric under the existing `cargo test` gate, plus the pure-CPU snapshots and invariants); step 3 (stored goldens) is v1.1.

## Child files — reading order + table of contents

Read in dependency order: the metric is the shared primitive every pixel tier consumes, so it comes first; the pure-CPU tiers next; the GPU tiers and their determinism substrate after; coverage last because it composes all of them.

1. [`metric.md`](metric.md) — `buiy_verify::metric`: the AA-aware two-axis perceptual diff (`Diff`, `FuzzBudget`, `compare`, `Diff::passes`), pixelmatch-YIQ + AA-sibling exclusion, advisory MSSIM, the migration of the two naive metrics. **Read first.**
2. [`snapshots.md`](snapshots.md) — `buiy_verify::snapshot`: Tier 1 layout-number + Tier 2 display-list/paint-order `insta` dumps (purpose-built `Display`, not raw `Debug`/serde) + the `PackedInstance` byte-hex check.
3. [`invariants.md`](invariants.md) — `buiy_verify::invariant`: Tier 3 proptest scene generators + predicate fns (`paint_order_is_total`, `transform_roundtrips`, `top_layer_dominates`, `all_finite`, `bidi_caret_roundtrips`), pure-CPU.
4. [`reftests.md`](reftests.md) — `buiy_verify::reftest`: Tier 4 `RefCase`/`reftest!`/`run_reftest`, the reference-independence discipline + lint, and the CPU-vs-GPU SDF cross-check (Tier 4.5). GPU.
5. [`goldens.md`](goldens.md) — `buiy_verify::golden`: Tier 5 `assert_golden` persistence, multi-positive corpus, `BUIY_BLESS` workflow, HTML triage report, storage migration, the Ahem/real-font split. GPU.
6. [`determinism.md`](determinism.md) — `buiy_verify::determinism` + `buiy_core::render::golden`: `DeterministicApp`, the `GoldenConfig` extensions (font mode, DPR, MSAA/dither), the quiescence flush, the lavapipe CI pin vs. the local real-GPU lane.
7. [`coverage.md`](coverage.md) — `buiy_verify::coverage`: the BSN-fixture single-source-of-truth, the `Matrix` Cartesian product auto-enrolling every tier, and the live-catalog wiring of `forced_colors_analyzer`.

## Prior art

Each tier draws on a researched external system; consult these `lessons.md` decision files when implementing:

- [`prior-art/wpt-reftests/lessons.md`](../../prior-art/wpt-reftests/lessons.md) — Tier 4: reftest `==`/`!=`, reference independence, two-axis fuzzy, `reftest-wait` settle, multiple-references aggregation.
- [`prior-art/vello/lessons.md`](../../prior-art/vello/lessons.md) — Tier 4.5: CPU-vs-GPU SDF cross-check (Buiy's oracle is *stronger* — one shared analytic function, kept permanently), and the FLIP-vs-pixelmatch per-tier-metric tension.
- [`prior-art/wgpu-testing/lessons.md`](../../prior-art/wgpu-testing/lessons.md) — `determinism`: the lavapipe pin recipe (`VK_DRIVER_FILES`, `WGPU_ADAPTER_NAME`, the `LP_NUM_THREADS` myth), the perceptual-metric migration, per-backend expectations.
- [`prior-art/skia-gold/lessons.md`](../../prior-art/skia-gold/lessons.md) — Tier 5: the `(widget, state, theme, viewport, backend, dpr)` key schema, multi-positive baselines, durable accept ledger, commit-keyed object store, local HTML triage, expiring ignores, stale-positive pruning.
- [`prior-art/flutter-golden-testing/lessons.md`](../../prior-art/flutter-golden-testing/lessons.md) — Tier 5: the Ahem/box-glyph layout-determinism font (UPM-1024), the two-tier obscure/real split, the engine-side shadow killswitch, `--update-goldens` curated accept, color-emoji as the irreducible golden.

The open questions the report raised are resolved as decisions in [`open-questions.md`](open-questions.md).

## Resolved during synthesis

Cross-file inconsistencies reconciled while assembling this entry point (where a child file's claim was changed, it is noted; otherwise the synthesis adopts the child's flagged finding into the canonical contract):

1. **`compose_transform` line is `:3775`, not `:3691`.** The SHARED API CONTRACT cited `:3691`; `invariants.md` flagged `:3775` (deviation #1) and it is verified on `origin/main` (`grep`-confirmed: `pub(super) fn compose_transform` at line 3775, `tier_rank` at 4113). The contract line is stale; **`:3775` is canonical**. No child-file edit needed (`invariants.md` already cites `:3775`).
2. **`PackedInstance.rect_size[1]` is POSITIVE on `main`.** The contract implied a deliberately-negative packed height (y-flip in the instance); `invariants.md` deviation #2 verified the y-flip moved into the per-view uniform (`render/instance.rs`), so packed height stays positive. Favorable: `all_finite_packed` asserts `rect_size[1] ≥ 0` directly with no un-flip. **Adopted.**
3. **`tier_rank` is promoted to `pub fn buiy_core::layout::top_layer_paint_rank(TopLayer) -> u8`.** `invariants.md` deviation #3: the `TopLayer` enum's *declared* order (`None, Modal, Popover, Tooltip, Fullscreen`) is NOT the paint order; the paint rank lives in a private closure `tier_rank` (`Fullscreen→0 … None→u8::MAX`). `top_layer_dominates` must compare via the rank, never the discriminant — so the rank is promoted to a single public source of truth consumed by both the layout sort and the invariant. **A small `buiy_core` surface add, accepted** (see `open-questions.md` § Contract reconciliation).
4. **`capture_to_image` is a re-runnable primitive, not one-shot-per-App.** `reftests.md` (Contract deviations) flagged that a reftest needs *two* captures sharing one `wgpu::Device` in one process, while `golden.md`/`determinism.md` spec a single `capture_to_image(&mut App, &GoldenConfig) -> RgbaImage`. Reconciled in favor of the existing signature: `capture_to_image` re-targets the offscreen camera and re-reads-back on *each* call against an already-built `App`, so reftest calls it twice on one `DeterministicApp::build()` output; `DeterministicApp::capture(self, fixture)` is the build+spawn+one-capture convenience wrapper goldens use. No new `capture_scene` shape is introduced. **Reconciled; see `open-questions.md` § Contract reconciliation.**
5. **`snapshot` resolves the contract's serde-"or" to the Display-dump branch only.** `snapshots.md` (Contract deviations) takes the purpose-built `Display` formatter exclusively and adds **no** serde derives to render types (the report's explicit anti-pattern is raw Debug/serde snapshots). `assert_display_list_snapshot` consequently takes `&NameLookup` (a `World`-free entity→`Name` map), not the contract's bare `(nodes, name)`. **Adopted into the contract.**
6. **`LP_NUM_THREADS` dropped as a determinism knob; `VK_ICD_FILENAMES` → `VK_DRIVER_FILES`.** `determinism.md` deviations 1 & 2, confirmed by `prior-art/wgpu-testing/lessons.md` (the `LP_NUM_THREADS` myth; the deprecated ICD env var). The contract's mention of `LP_NUM_THREADS` as a determinism setting is corrected — determinism comes from the *pinned Mesa version*. **Adopted.**
