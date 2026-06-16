**Date:** 2026-06-14
**Status:** active
**Subject:** Vello's classic GPU-compute pipeline — `Scene` → `Encoding` → a fixed sequence of WGSL compute stages

# The classic compute pipeline (the `vello` crate)

This file documents the *flagship* `vello` crate's renderer. Buiy should read it mainly to understand **what to NOT copy** (see [`lessons.md`](lessons.md) Avoid): the compute-centric architecture solves a problem Buiy does not have. The transferable lesson lives in [`cpu-gpu-testing.md`](cpu-gpu-testing.md) and [`sparse-strips.md`](sparse-strips.md), not here.

## The scene-description front end

A `Scene` exposes a canvas-like API — `fill()`, `stroke()`, `push_layer()` — that does not draw immediately. Instead it appends into an internal **`Encoding`**, a compact binary format owned by the `vello_encoding` crate. The encoding is split into several *parallel streams* so the GPU can process each independently ([DeepWiki architecture](https://deepwiki.com/linebender/vello/1.1-architecture)):

- `tag_stream` — per-path-segment tags
- `path_stream` — packed coordinates
- `draw_stream` — draw objects (brushes, blends)
- `transform_stream` — affine transforms
- `linewidth_stream` — stroke widths

This stream-of-arrays layout is deliberate: it lets each compute stage scan one homogeneous array rather than chasing a tagged-union tree, which is what makes the GPU prefix-sum approach tractable.

## Recording and executing commands

At render time `WgpuEngine` records a list of `Command`s against a `ResourcePool`/`BindMap`, then submits them through `wgpu`. The command vocabulary is small ([DeepWiki](https://deepwiki.com/linebender/vello/1.1-architecture)):

- `Upload` / `UploadUniform` — push buffers to the GPU
- `Dispatch` / `DispatchIndirect` — run a compute shader (indirect = workgroup count comes from a GPU buffer, needed because some stage sizes are only known on-GPU)
- `Download` — read a buffer back to the CPU

## The fixed stage sequence

The pipeline is a chain of WGSL compute shaders dispatched in a fixed order (WGSL file names, per DeepWiki):

```
pathtag_reduce → pathtag_scan   (prefix-sum over path-segment tags)
bbox_clear
flatten                         (curves → line segments)
draw_reduce → draw_leaf
clip_reduce → clip_leaf
binning                         (segments → tiles)
tile_alloc
backdrop
coarse                          (per-tile command lists)
fine_*                          (final antialiased pixels)
```

Two structural ideas dominate:

1. **GPU prefix-sums.** Inherently sequential work (assigning each path segment its cumulative position, resolving nested clips) is parallelized via reduce-then-scan prefix-sum passes — the `*_reduce` / `*_scan` / `*_leaf` pairs. This is the technique Raph Levien has written about extensively; it is also the source of the portability problems (see [`open-problems.md`](open-problems.md)).
2. **Sort-middle, coarse-then-fine tiling.** `binning` sorts segments into screen tiles; `coarse` builds a per-tile command list; `fine_*` walks each tile's list to produce the final antialiased pixels. This is a "sort-middle" architecture in GPU-rendering terms.

The fine stage *ideally* samples all scene images in a single pass, but the docs note "that's not really possible in WebGPU 1.0" — a stated limitation, not a solved problem ([README](https://github.com/linebender/vello/blob/main/README.md)).

## Substrate the pipeline sits on

Vello is built on the Linebender 2D substrate (the same crates Buiy studies from the render-target angle in [`../xilem-masonry/linebender-stack.md`](../xilem-masonry/linebender-stack.md)):

- **`kurbo`** — curves and affines (`Circle`, `Affine`, path flattening).
- **`peniko`** — `Color`, `Fill`, brushes, blend/compose primitives.
- **`color`** — color-space-aware interpolation.
- **`skrifa`** — font/glyph outlines (incl. VARC variable-composite glyphs); `vello` 0.9.0 builds on skrifa 0.42 (latest is 0.43.x as of 2026-06).

README confirms `kurbo`, `peniko`, `wgpu` directly; `color` and `skrifa` are documented in release notes rather than the README.

## Implications for Buiy

Buiy's renderer is **instanced quads + a per-fragment SDF**, not a sort-middle compute pipeline. The entire `Scene` → `Encoding` → prefix-sum → tile machinery above is overkill for that model. The one thing worth internalizing is the *separation of concerns*: Vello has a renderer-agnostic scene description (`Encoding`) that multiple backends (`vello`, `vello_cpu`, `vello_hybrid`) can each consume and produce comparable output from. That comparability is what makes the CPU/GPU cross-check test ([`cpu-gpu-testing.md`](cpu-gpu-testing.md)) possible — and Buiy gets the same comparability for free, because its CPU oracle and GPU shader evaluate *the same closed-form SDF*.

## Sources

- DeepWiki Vello architecture: https://deepwiki.com/linebender/vello/1.1-architecture
- Vello README: https://github.com/linebender/vello/blob/main/README.md
- `vello_encoding` crate: https://crates.io/crates/vello_encoding
- "Requiem for piet-gpu-hal" (Raph Levien, on retiring the bespoke HAL for wgpu): https://github.com/raphlinus/raphlinus.github.io/issues/86
