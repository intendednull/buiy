**Date:** 2026-06-14
**Status:** active
**Subject:** Vello's sparse-strips family — `vello_cpu` / `vello_hybrid` / `vello_common`, SIMD `Level` detection, and the `u8` vs `f32` pipelines

# The sparse-strips family — the part most relevant to Buiy's oracle

The flagship `vello` crate's compute pipeline ([`architecture.md`](architecture.md)) has four standing problems: GPU-allocation robustness, no-GPU / underpowered-GPU targets, web compatibility, and glyph caching (see [`open-problems.md`](open-problems.md)). To address them, Linebender built a **second-generation "sparse strips" architecture** shared by a family of crates that each consume the same scene format but execute different rendering pipelines:

- **`vello_cpu`** — pure software rasterizer. "A CPU-based renderer for Vello, optimized for SIMD and multithreaded execution" ([docs.rs/vello_cpu](https://docs.rs/vello_cpu)).
- **`vello_hybrid`** — CPU strip generation + GPU fine rasterization. "runs the most compute intensive portions of rendering on the GPU … wide compatibility with most devices, so long as they have a GPU, including running well on the web." It rasterizes sparse strips with a *fragment* shader (two triangles per strip; the fragment reads strip alpha → solid color; hardware does the final blend), so it runs on WebGL2 and low-end GPUs that cannot run the compute pipeline ([sparse strip path rendering, vello#670](https://github.com/linebender/vello/issues/670)).
- **`vello_common`** — shared infrastructure (geometry, paints, glyph plumbing) re-exported by both.

The method is documented in a published **ETH Zürich master's thesis** on high-performance CPU rendering of 2D graphics, attributed to **Laurenz Stampl** per the [Linebender Oct 2025 blog](https://linebender.org/blog/tmil-22/). **Uncertain:** this author/institution attribution is single-sourced to the blog; it could not be independently re-verified against the thesis PDF itself. Treat the name as single-sourced.

## `vello_cpu` API surface

The primary interface is **`RenderContext`** with `set_paint()`, `fill_path()`, `stroke_path()`, `glyph_run()` ([docs.rs/vello_cpu](https://docs.rs/vello_cpu)). Two patterns matter for Buiy:

### 1. The SIMD `Level` enum (runtime detection)

`vello_cpu` exposes a `Level` enum for **runtime SIMD detection** — it picks the best available instruction set (x86, aarch64, wasm) at runtime rather than requiring a target-feature build. This is the same shape Buiy would want if its CPU SDF oracle ever needs to vectorize per-pixel evaluation: detect the level once, dispatch the inner loop accordingly, keep a scalar fallback as the definition-of-correct.

### 2. The `u8` vs `f32` pipelines (the oracle precision knob)

`vello_cpu` has **two pipelines, switchable at runtime** (landed per the [Linebender Dec 2025 update](https://linebender.org/blog/tmil-24/): "Added features to Vello CPU to switch between `u8` and `f32` pipelines"):

| Pipeline | `RenderMode` | Role |
|---|---|---|
| `u8` | `OptimizeSpeed` | fast, lower precision |
| `f32` | `OptimizeQuality` | "slower but has more accurate results, and is **especially useful for rendering test snapshots**" |

So the **higher-precision `f32` CPU path is the intended snapshot/oracle generator** — this is precisely the role Buiy wants its CPU SDF port to play. The lesson: an oracle should be the *most accurate* available evaluation of the spec, even if it is slow, because it is only run in tests. Buiy's CPU SDF, evaluated in `f32` with the same AA coverage step the WGSL uses, is the direct analog of `vello_cpu`'s `f32` pipeline.

## Stated warts (verbatim / paraphrased)

- "the API is still likely to change and not stable yet."
- Filters and image-resources are **experimental**.
- "multi-threading with large thread counts (more than 4) might give diminishing returns, **especially when making heavy use of layers and clip paths**."
- All sparse-strips crates are still `0.0.x` (latest **0.0.9**, 2026-05-30) — an explicit "do not depend on stability" signal.

## Implications for Buiy

`vello_cpu` is the existence proof that a CPU reference rasterizer **born specifically to backstop a GPU renderer** is a workable design — the exact analogy to Buiy promoting its CPU SDF port to an oracle (see [`lessons.md`](lessons.md) Borrow). But note the architectural mismatch: `vello_cpu` is a full sparse-strip rasterizer (anti-aliased path fill, strokes, glyphs, clips, layers) — porting *that* would be enormous. Buiy's oracle is far simpler: a per-pixel evaluation of an analytic SDF. Borrow the *role and precision posture* (`f32`, accuracy over speed, runtime SIMD `Level`), not the rasterizer. And do **not** take a runtime dependency on `vello_cpu` itself — the `0.0.x` versioning makes its output a moving target.

## Sources

- docs.rs/vello_cpu: https://docs.rs/vello_cpu
- lib.rs/crates/vello_cpu: https://lib.rs/crates/vello_cpu
- Linebender "This Month in… " Oct 2025 (sparse strips / thesis attribution): https://linebender.org/blog/tmil-22/
- Linebender Dec 2025 (`u8`/`f32` pipelines, hybrid on web): https://linebender.org/blog/tmil-24/
- Sparse strip path rendering issue: https://github.com/linebender/vello/issues/670
- GitHub releases (`0.0.x` versions): https://github.com/linebender/vello/releases
