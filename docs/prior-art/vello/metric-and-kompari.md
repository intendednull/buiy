**Date:** 2026-06-14
**Status:** active
**Subject:** The comparison metric — `nv_flip` mean-error, the contested xilem tolerance-16 counter-position, and the Kompari convergence plan

# The comparison metric

This is the load-bearing file for Buiy's "which image-diff metric?" decision. The headline: **Linebender does not have one settled answer.** As of mid-2026 the org runs *two different metrics* — `vello_tests` on `nv_flip` mean-threshold, xilem on a tolerance-16 plain pixel diff — and is mid-flight on a third convergence tool, Kompari.

## `nv_flip` (NVIDIA ꟻLIP), mean-error threshold — what `vello_tests` uses

Both `src/snapshot.rs` and `src/compare.rs` compare images with the **`nv_flip`** crate (a Rust binding to NVIDIA's ꟻLIP perceptual difference metric) — **not** exact pixel match and **not** (yet) Kompari. The pattern in `compare.rs`:

```rust
let error_map = nv_flip::flip(expected, rendered, nv_flip::DEFAULT_PIXELS_PER_DEGREE);
let pool = FlipPool::from_image(&error_map);
```

`GpuCpuComparison` holds `statistics: Option<FlipPool>`, the two `ImageData` buffers, their paths, and `TestParams`. The single assertion is **`assert_mean_less_than(&mut self, value: f32)`**, which reads `stats.mean()` off the FLIP pool and fails if the mean error exceeds the caller-supplied bound. The harness notes "Mean should be less than 0.1 in almost all cases for a successful test" (*paraphrased, not byte-exact — see [`cpu-gpu-testing.md`](cpu-gpu-testing.md)*).

**Non-zero error is deliberately tolerated**, with this verbatim rationale: the difference "could potentially be non-zero (i.e. there is a slight difference between the GPU and CPU results) **due to fast math on the GPU or different precisions used in the renderers**." This is the crux of why a *perceptual continuous* metric beats *exact pixel match* for a GPU-vs-CPU oracle check: GPU fast-math and precision differences guarantee small per-pixel divergence even when both renderers are correct.

### FLIP, the algorithm

FLIP ("FLIP: A Difference Evaluator for Alternating Images," Andersson, Nilsson, Akenine-Möller, Oskarsson, Åström, Fairchild; *Proc. ACM Comput. Graph. Interact. Tech. (PACMCGIT)* 3(2), 2020) models the difference a human perceives when **flipping** between two images — the exact reftest viewing mode. The authors recommend the **mean** of the error map as the single summary number, which is exactly what `assert_mean_less_than` consumes.

### The Rust binding

The binding is **`nv-flip`** v0.1.2 (latest; published **2023-07-16** per the crates.io API — *note a secondary source misreported "March 2026"; the registry is authoritative*). API: `FlipImageRgb8::with_data` → `flip(ref, test, DEFAULT_PIXELS_PER_DEGREE = 67.0)` → `FlipPool::mean()`. It is pre-1.0 and unchanged since 2023, and it wraps a C++ library via `nv-flip-sys` (a build-time native-toolchain cost). License: MIT OR Apache-2.0 OR Zlib for the bindings (FLIP core is BSD-3-Clause) — all compatible with Buiy's MIT-OR-Apache-2.0.

## WART — `nv_flip` is contested *inside* Linebender (the xilem counter-position)

The sibling project **xilem REMOVED the `nv-flip` dependency** for its widget screenshot tests, in favor of a **plain pixel-by-pixel diff with tolerance = 16**, because FLIP produced **false negatives**: verbatim, "The nv_flip algorithm may consider dark grey and white to be very similar colors" ([xilem #893](https://github.com/linebender/xilem/issues/893)). Tolerance 16 "seems to be the sweet spot" and reportedly catches "swapping the stroke join, changing a widget's border width, moving text by a tenth of a pixel" ([xilem PR #904](https://github.com/linebender/xilem/pull/904)).

So, as of mid-2026, Linebender runs **two different metrics**:

| Project | Metric | Rationale |
|---|---|---|
| `vello_tests` | `nv_flip` mean-error threshold | tolerate GPU fast-math / precision noise in GPU-vs-CPU agreement |
| `xilem` (widget screenshots) | plain pixel diff, **tolerance 16** | FLIP had false negatives on dark-grey/white; catches sub-pixel widget changes |

**The split is not noise — it is a real signal about the failure mode.** FLIP is *perceptually forgiving by design*, which is exactly right when comparing two correct renderers (the divergence is sub-perceptual) and exactly wrong when the change you want to catch is itself sub-perceptual (a 1px border, a tenth-of-a-pixel text shift). The right metric depends on whether the test is an **oracle agreement check** (perceptual metric: tolerate the noise) or a **regression catch** (tight pixel tolerance: catch the small intentional-looking change).

## Kompari — the convergence plan

[`linebender/kompari`](https://github.com/linebender/kompari) is "a tool for reporting image differences … for use in snapshot testing" — a CLI + Rust crate, contributed by **Ada Böhm**, "currently in pre-alpha," whose stated goal is to "standardise and improve the developer experience of snapshot tests in Linebender (and beyond)." It produces static HTML diff reports and an HTTP server for interactively *blessing* snapshots. Vello "improved how its snapshot tests are handled in preparation for Kompari integration."

**Status (verified):** Kompari has **no published releases** (MSRV 1.85). **Uncertain:** whether Kompari has *replaced* `nv_flip` in `vello_tests` by June 2026 is unconfirmed — as read, the live `src/compare.rs` / `src/snapshot.rs` still call `nv_flip`.

## Implications for Buiy

For Buiy's **CPU-SDF-oracle agreement check** (GPU readback vs CPU SDF rasterization), the failure mode is Vello's, not xilem's: both paths compute the same analytic SDF, so divergence is GPU-fast-math / AA / precision noise. A **perceptual continuous metric (`nv-flip` mean-error)** is the right fit, and this resolves Buiy's open "pixelmatch-YIQ vs FLIP" question *toward FLIP for the oracle tier*. But heed the xilem lesson for Buiy's **golden-screenshot top tier**, where the goal is catching small intentional-looking regressions: there a tight pixel tolerance may catch what FLIP smooths over. Buiy should likely use **FLIP for the oracle tier and a tight pixel tolerance for the golden tier** — two metrics for two failure modes, exactly mirroring Linebender's accidental two-metric state, but chosen deliberately. Calibrate any threshold on a known-good Buiy frame; do **not** adopt Vello's number blindly — it is tuned to Vello's AA model.

**Caveat on the golden tier's tight tolerance.** Even Vello's *snapshot* tier uses a non-exact comparison "because of small differences between rendering on different platforms" ([`cpu-gpu-testing.md`](cpu-gpu-testing.md)). That cross-platform/cross-driver rendering variance is the standing reason golden-screenshot suites are notoriously flaky: a tolerance tight enough to catch a 1px regression is also tight enough to trip on driver-level AA differences. Buiy's tight-tolerance golden tier therefore needs its references pinned to a single fixed renderer/driver/OS in CI (or a per-platform reference set), not just a low pixel threshold — otherwise the tier flakes on the exact noise FLIP was chosen to absorb. (Full Borrow/Avoid framing in [`lessons.md`](lessons.md).)

## Sources

- `vello_tests/src/compare.rs`: https://github.com/linebender/vello/blob/main/vello_tests/src/compare.rs
- `vello_tests/src/snapshot.rs`: https://github.com/linebender/vello/blob/main/vello_tests/src/snapshot.rs
- FLIP paper: https://dl.acm.org/doi/10.1145/3406183
- `nv-flip-rs` bindings: https://github.com/gfx-rs/nv-flip-rs
- crates.io API for `nv-flip`: https://crates.io/api/v1/crates/nv-flip
- docs.rs/nv-flip: https://docs.rs/nv-flip/latest/nv_flip/
- xilem issue #893 (FLIP false negatives): https://github.com/linebender/xilem/issues/893
- xilem PR #904 (tolerance-16 pixel diff): https://github.com/linebender/xilem/pull/904
- Kompari README: https://github.com/linebender/kompari/blob/main/README.md
- Linebender Dec 2024 (Kompari): https://linebender.org/blog/tmil-12/
