**Date:** 2026-06-14
**Status:** active
**Subject:** Vello / Linebender-specific terms used across this folder

# Glossary

Terms specific to Vello and the Linebender ecosystem, as used in the sibling files. General Rust/GPU terms are omitted.

- **Vello** — Linebender's "GPU compute-centric 2D renderer." The flagship `vello` crate; also the umbrella name for the family (`vello` / `vello_cpu` / `vello_hybrid`). See [`architecture.md`](architecture.md).

- **`Scene`** — Vello's canvas-like front-end API (`fill()`, `stroke()`, `push_layer()`) that appends into an `Encoding` rather than drawing immediately. See [`architecture.md`](architecture.md).

- **`Encoding`** — the compact binary scene format owned by the `vello_encoding` crate, split into parallel streams (`tag_stream`, `path_stream`, `draw_stream`, `transform_stream`, `linewidth_stream`). The renderer-agnostic representation all backends consume.

- **Sparse strips** — Linebender's second-generation rasterization architecture (a strip = a horizontal run of pixels with a coverage value). Shared by `vello_cpu` and `vello_hybrid`. `vello_hybrid` rasterizes each strip as two triangles via a *fragment* shader. See [`sparse-strips.md`](sparse-strips.md).

- **`vello` (flagship)** — the GPU-compute renderer; runs a prefix-sum compute pipeline. Requires WebGPU compute support.

- **`vello_cpu`** — pure-software sparse-strip rasterizer, "optimized for SIMD and multithreaded execution"; the reference rasterizer. DeepWiki frames it "for debugging purposes," not as a formal oracle.

- **`vello_hybrid`** — CPU strip generation + GPU fragment-shader fine rasterization; targets WebGL2 and resource-constrained GPUs / the web.

- **`vello_common`** — shared infrastructure (geometry, paints, glyph plumbing) re-exported by the sparse-strips crates.

- **`vello_tests`** — the dedicated test crate: `TestParams`, `snapshot`/`compare` modules, `snapshots/` (LFS) + `smoke_snapshots/` (committed). See [`cpu-gpu-testing.md`](cpu-gpu-testing.md).

- **`u8` / `f32` pipelines** — `vello_cpu`'s two render modes. `u8` = `OptimizeSpeed`; `f32` = `OptimizeQuality` (slower, more accurate, "especially useful for rendering test snapshots" — the oracle generator).

- **`Level` (enum)** — `vello_cpu`'s runtime SIMD-detection enum (x86 / aarch64 / wasm), with a scalar fallback.

- **`RenderContext`** — `vello_cpu`'s primary interface (`set_paint`, `fill_path`, `stroke_path`, `glyph_run`).

- **`RenderMode`** — selects `OptimizeSpeed` (`u8`) vs `OptimizeQuality` (`f32`).

- **Comparison test (tier 3)** — render the *same* scene on GPU and CPU, assert they agree; the CPU-as-oracle cross-check. Slated to be "phased out" in Vello because its two paths are different implementations.

- **`GpuCpuComparison`** — the `vello_tests` struct holding `statistics: Option<FlipPool>`, the two `ImageData` buffers + paths, and `TestParams`. Exposes `assert_mean_less_than`.

- **`nv_flip` / FLIP / ꟻLIP** — NVIDIA's FLIP perceptual image-difference metric, and its Rust binding crate (`nv-flip`, via `nv-flip-sys` over C++). Models the difference perceived when *flipping* between two images; summarized by the **mean** of its error map. See [`metric-and-kompari.md`](metric-and-kompari.md).

- **`FlipPool`** — the FLIP error-map statistics object; `.mean()` is the load-bearing summary.

- **`DEFAULT_PIXELS_PER_DEGREE`** — FLIP's viewing-distance constant (67.0); parameterizes the perceptual model.

- **Blessing** — accepting a new/changed render as the reference. In `vello_tests`, driven by env vars `VELLO_TEST_CREATE` (write missing reference) and `VELLO_TEST_UPDATE` (overwrite mismatches).

- **Kompari** — `linebender/kompari`, a pre-alpha snapshot-diff tool (HTML reports + interactive-blessing HTTP server) intended to converge Linebender's snapshot testing. No published releases. Contributed by Ada Böhm. See [`metric-and-kompari.md`](metric-and-kompari.md).

- **Conflation artifacts** — visible AA seams where adjacent primitives meet; one of Vello's four named open problems.

- **Prefix sum (scan)** — the parallel primitive Vello uses to turn sequential per-segment work into GPU-parallel work (`*_reduce` / `*_scan` / `*_leaf` stages). The source of its portability problems. See [`open-problems.md`](open-problems.md).

- **Sort-middle / coarse-then-fine** — Vello's tiling strategy: `binning` sorts segments into tiles, `coarse` builds per-tile command lists, `fine_*` produces final pixels.

- **piet-gpu / piet-gpu-hal** — Vello's predecessors. `piet-gpu-hal` was Linebender's bespoke GPU HAL, retired in favor of `wgpu`.

- **Linebender** — the informal volunteer collective (founded / informally led by Raph Levien) behind Vello, Parley, Xilem, Masonry, Kurbo, Peniko, Color, Skrifa, Kompari.

- **Parley** — Linebender's rich-text layout crate; Vello's text companion. Shapes via HarfRust as of v0.10.0. (Buiy uses cosmic-text instead.)

- **`kurbo` / `peniko` / `color` / `skrifa`** — the Linebender 2D substrate crates Vello builds on (curves+affines / paint primitives / color spaces / font outlines).

## Sources

- DeepWiki architecture: https://deepwiki.com/linebender/vello/1.1-architecture
- `vello_tests` source: https://github.com/linebender/vello/tree/main/vello_tests
- docs.rs/nv-flip: https://docs.rs/nv-flip/latest/nv_flip/
- Kompari README: https://github.com/linebender/kompari/blob/main/README.md
