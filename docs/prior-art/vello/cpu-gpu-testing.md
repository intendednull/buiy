**Date:** 2026-06-14
**Status:** active
**Subject:** Vello's `vello_tests` harness — three test tiers, GPU-as-source-of-truth, CPU-as-oracle cross-check, blessing flow

# The `vello_tests` crate — render-correctness testing

This is the directly-transferable file for Buiy's visual-bug-detection strategy. The metric details (`nv_flip`, the contested xilem counter-position, Kompari) live in [`metric-and-kompari.md`](metric-and-kompari.md); this file covers the harness structure and the test tiers.

## Crate layout (verified from the repo tree)

Vello's render-correctness tests live in a dedicated workspace crate, `vello_tests/`:

- `src/` — the harness (`lib.rs` plus `snapshot`/`compare` modules)
- `tests/` — `#[test]` cases
- `snapshots/` — reference PNGs (bulk fetched via **Git LFS**)
- `smoke_snapshots/` — a small subset committed directly (not LFS)
- `current/`, `comparisons/`, `debug_outputs/` — working/diagnostic outputs

`src/lib.rs` defines **`TestParams`** (`width`, `height`, `base_color`, `use_cpu`, `name`, `anti_aliasing`) and re-exports the comparison entry points from two modules:

- `snapshot` — `snapshot_test`, `snapshot_test_sync`, `smoke_snapshot_test_sync`
- `compare` — `compare_gpu_cpu`, `compare_gpu_cpu_sync`, `GpuCpuComparison`

Core rendering helpers: `get_scene_image`, `render_then_debug` / `render_then_debug_sync`, `encode_test_scene`, `write_png_to_file`.

## The three test tiers (per the crate README)

| Tier | What it does | Oracle |
|---|---|---|
| **1. Property tests** | Run a generated scene through both GPU and CPU; check invariants. | invariant assertions |
| **2. Snapshot / golden tests** | Render a scene; diff against a committed PNG. **GPU output is the source of truth**; the CPU path is also exercised. | checked-in PNG |
| **3. Comparison tests** | Render the *same* scene on GPU and CPU; assert they agree (validates GPU against the CPU reference). | the CPU renderer itself |

Tier 3 is the **golden-free CPU-as-oracle cross-check** — and the single most transferable idea for Buiy. Crucially, the README explicitly states the team "hope to largely phase these out in favour of additional snapshot tests" — i.e. the direct CPU-vs-GPU cross-check is treated as **transitional scaffolding**, not the long-term oracle. (See [`lessons.md`](lessons.md) for why Buiy's situation differs: Buiy's CPU and GPU paths evaluate the *same analytic SDF*, so their agreement is a more durable invariant against implementation drift than Vello's two-different-pipelines agreement — though, as that file notes, sharing one function means the cross-check cannot catch a bug in the SDF *itself*; that residual class is the golden/reftest tiers' job, not the oracle's.)

Note that even the snapshot tier (tier 2) uses "a non-exact comparison metric, because of small differences between rendering on different platforms" — Vello never asserts exact pixel equality anywhere.

## `vello_cpu`'s `f32` pipeline is the snapshot generator

The reference rasterizer is `vello_cpu` (the sparse-strips CPU renderer; see [`sparse-strips.md`](sparse-strips.md)). Its **`f32` pipeline** ("slower but has more accurate results, and is especially useful for rendering test snapshots") is the intended snapshot/oracle generator, backing `RenderMode::OptimizeQuality`. The higher-precision CPU path is deliberately the one used to produce references.

## The blessing / update flow (env-var driven)

Snapshot tests are self-updating via environment variables (per `src/snapshot.rs`):

- A **missing reference** plus `VELLO_TEST_CREATE` writes the new PNG into `snapshots/`; otherwise it writes to an update path and bails with instructions to set `VELLO_TEST_CREATE=all`.
- `VELLO_TEST_UPDATE` converts mismatches into overwrites of the reference.

This env-var blessing pattern (no special CLI, just `CARGO` + an env flag) is a clean, reusable shape for Buiy's snapshot tier.

## What could NOT be verified (carried flags)

- The single comparison assertion is `assert_mean_less_than(&mut self, value: f32)`, which reads `stats.mean()` off the FLIP pool and fails if the mean error exceeds the caller-supplied bound. The harness notes "**Mean should be less than 0.1 in almost all cases for a successful test.**" **This string is paraphrased from a WebFetch read of `compare.rs`, not byte-exact.**
- No `assert_all_less_than` / percentile assertion was found in `compare.rs` — only the **mean** assertion was present.
- Vello-specific **CI config** invoking this harness was not surfaced in search; do not assert CI specifics beyond "the harness exists and uses `nv_flip` mean-error."

## Implications for Buiy

Buiy can adopt Vello's harness *skeleton* wholesale, independent of the metric: **render A, render B, perceptual-diff, assert below threshold.** B can be a CPU oracle (oracle mode, Buiy's tier just above layout-number snapshots) or a checked-in PNG (golden mode, Buiy's top tier). One harness, two tiers. The `TestParams`-style config struct and the env-var blessing flow are both directly liftable shapes. The key adaptation: where Vello's tier-3 oracle is a *second, independently-implemented renderer* (hence "phase these out"), Buiy's oracle is a CPU port of the *same SDF function* the GPU evaluates — so Buiy's cross-check is a more durable invariant against implementation drift and need not be treated as scaffolding. (The flip side, per [`lessons.md`](lessons.md): a shared SDF that is wrong in the *spec* leaves both paths wrong identically, so this tier still needs the golden/reftest tiers above it.)

## Sources

- `vello_tests` tree: https://github.com/linebender/vello/tree/main/vello_tests
- `vello_tests/src/lib.rs`: https://github.com/linebender/vello/blob/main/vello_tests/src/lib.rs
- `vello_tests/README.md`: https://github.com/linebender/vello/blob/main/vello_tests/README.md
- `vello_tests/src/compare.rs`: https://github.com/linebender/vello/blob/main/vello_tests/src/compare.rs
- `vello_tests/src/snapshot.rs`: https://github.com/linebender/vello/blob/main/vello_tests/src/snapshot.rs
- DeepWiki testing & validation: https://deepwiki.com/linebender/vello/5.2-testing-and-validation
