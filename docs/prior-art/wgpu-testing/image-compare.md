**Date:** 2026-06-14
**Status:** active
**Subject:** wgpu's image-comparison harness — the nv-flip crate, the FLIP perceptual metric, mean/percentile thresholds, the magma diff artifact, and the superseded outlier-count model

# Image comparison: `nv_flip` + the golden-image harness

wgpu compares rendered output against stored golden PNGs using its own [`tests/src/image.rs`](https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/tests/src/image.rs) module, which delegates the actual perceptual comparison to the **`nv-flip`** crate. This is the most directly transplantable comparison model for Buiy because both render on wgpu.

## The `nv-flip` crate (verified)

Per the crates.io API ([`crates.io/api/v1/crates/nv-flip`](https://crates.io/api/v1/crates/nv-flip)), the newest and only current version is **0.1.2, published 2023-07-16T03:35:23Z**; full history is 0.1.0 and 0.1.1 (both 2023-06-04) then 0.1.2. License is **`MIT OR Apache-2.0 OR Zlib`**, repo [`github.com/gfx-rs/nv-flip-rs`](https://github.com/gfx-rs/nv-flip-rs), ~591.9k total downloads *(live snapshot, not a pinned fact)*. **It is maintained by the gfx-rs org — the wgpu maintainers themselves.** The companion low-level FFI crate is `nv-flip-sys`. Description: *"High-Level bindings to Nvidia Labs's ꟻLIP image comparison and error visualization library."* The crate wraps NVIDIA's **FLIP** — the perceptual difference metric from the *"FLIP: A Difference Evaluator for Alternating Images"* paper (HPG 2020). FLIP itself is BSD-3-Clause.

## The FLIP metric and its output

`nv_flip::flip(reference, test, DEFAULT_PIXELS_PER_DEGREE)` produces a per-pixel **error map**, documented as *"the per-pixel visual difference between the two images between 0 and 1"* (0.0 = identical, 1.0 = maximal perceptual error). The error map is a `FlipImageFloat`.

Public API (docs.rs v0.1.2):
- structs `FlipImageRgb8`, `FlipImageFloat`, `FlipHistogram`, `FlipPool`
- functions `flip()`, `magma_lut()`, `pixels_per_degree()`
- constant `DEFAULT_PIXELS_PER_DEGREE`

The viewer-distance parameter (**pixels-per-degree**) is what makes FLIP *perceptual* rather than a raw pixel diff — it models how far the observer sits from the display, so anti-aliasing and sub-pixel rounding noise below the perceptual threshold do not register as error, while a genuine visual regression does. This edge-contrast-amplified model is the property that killed the older outlier-count approach (below).

## Reducing the map to pass/fail: `FlipPool`

wgpu feeds the error map into `nv_flip::FlipPool::from_image(&error_map_flip)`, a *"histogram-like value pool for determining if [the] error map has significant differences."* It exposes `mean()`, `get_percentile(p, true)`, `min_value()`, `max_value()`. The nv-flip docs explicitly recommend the mean: *"if you are to use a single number to represent the error, [the FLIP authors] recommend the mean."*

## The current assertion model (Mean / Percentile)

`image.rs` defines:

```rust
pub enum ComparisonType {
    Mean(f32),
    Percentile { percentile: f32, threshold: f32 },
}
```

- `Mean(x)` fails if the **mean** error exceeds `x`.
- `Percentile { percentile, threshold }` fails if the given percentile (in `[0,1]`) exceeds `threshold`.

Failure messages: `"\tExpected Mean ({:.6}) to be under expected maximum ({}): {}"` and `"\tExpected {}% ({:.6}) to be under expected maximum ({}): {}"`. The harness prints the error distribution at percentiles **[25, 50, 75, 95, 99]** (`pool.get_percentile(p/100.0, true)`), runs **every** check in the list (`all_passed &= check.check(&mut pool)`), and on any failure panics with `"Image data mismatch: {}"` where `{}` is the path to a written **diff image**.

Practical thresholds live in the **[0.01, 0.1] range** for mean/percentile error, per the crate docs — Buiy should treat these as **empirically tuned per-test, not universal**. *(Flag: this range is crate-doc guidance, not a hard constant in `image.rs`; the actual thresholds are per-test call-site args, which could not be enumerated from a single source.)*

## The diff-map artifact

On mismatch, wgpu colorizes the FLIP error map with the magma colormap and writes it to disk:

```rust
error_map_flip.apply_color_lut(&nv_flip::magma_lut());
```

saved as `"{file_stem}-{renderer}-difference.png"`, where `renderer` is `"{backend}-{sanitized_name}-{sanitized_driver}"`. This **per-backend naming** is how wgpu disambiguates failures across GPUs/drivers — a pattern Buiy will need if it ever runs golden tests on more than one adapter.

## Golden-image storage (and the implicit-bootstrap wart)

References are read via `read_png(&path, width, height)` returning `Option<Vec<u8>>`; the harness validates width/height, RGBA color type, and 8-bit depth, then **strips alpha** (FLIP compares RGB). PNGs are committed in-repo alongside the example/test sources.

**Wart:** `read_png` itself only returns `None` when no reference exists; the minting happens one level up in `compare_image_output` (image.rs:155–179), whose `None =>` arm **writes the current test image as the new baseline via `write_png` and returns early** — i.e. **golden bootstrapping is implicit on first run**. A missing or deleted golden silently "passes" by minting itself. Buiy should make first-run minting **explicit and gated** (a flag, not the default) so a deleted golden fails loudly instead of being silently regenerated. See [open-problems.md](open-problems.md).

## The superseded outlier-count model (cautionary precursor)

Before the FLIP rewrite, wgpu used a raw per-pixel **outlier count**: count pixels whose per-channel delta exceeds a tolerance; fail if too many exceed a limit. [PR #2767](https://github.com/gfx-rs/wgpu/pull/2767) ("Increase max_outliers on wgpu water example reftest.", Jim Blandy / @jimblandy, merged 2022-06-14) *raised* `max_outliers` on the water reftest — it did **not** replace the model. Its body documents the exact flake: on *"AMD RADV POLARIS12"* the test panicked with

> `"Image data mismatch! Outlier count 464 over limit 460. Max difference 213"`

— i.e. *"N outlier pixels over limit M, max channel difference D"* — and on inspection the diff was *"just a few dots here and there."* This is the evidence the outlier model was **brittle across drivers**: scattered sub-perceptual noise tripped a hard count. A separate symptom is [issue #2760](https://github.com/gfx-rs/wgpu/issues/2760) ("Windows 11's WARP Passes the Water Example Image Comparison Test", @cwfitzgerald) — an unexpected-*pass* the outlier model could not express as a per-driver expectation. The model was later **replaced** by perceptual FLIP in [PR #3830](https://github.com/gfx-rs/wgpu/pull/3830) ("Migrate to nv-flip for image comparison", merged 2023-06-08), precisely because perceptual mean/percentile thresholds **tolerate scattered sub-perceptual noise that an outlier count cannot**.

**For Buiy's strategy doc:** cite the outlier-count model as the cautionary precursor, and adopt the FLIP mean/percentile model as the target.

## Implications for Buiy

`nv_flip` + **mean** is the wgpu-ecosystem-native perceptual metric (directly answers verification Open Q #3 in the strategy report). It is already license-compatible (`MIT OR Apache-2.0 OR Zlib`; FLIP is BSD-3-Clause) and avoids the AA-flake problem that kills exact-pixel goldens and that killed wgpu's own outlier-count model. The one cost: `nv_flip` is **FFI to a C++ library** (`nv-flip-sys`) — a build-graph and native-dependency cost in CI. If Buiy wants pure-Rust, a pixelmatch-YIQ port is the runner-up — cheaper to vendor, but YIQ has no edge-contrast term and so will produce more AA false-positives. Tradeoff named; FLIP recommended. See [lessons.md](lessons.md).

## Sources

- crates.io: https://crates.io/crates/nv-flip · API: https://crates.io/api/v1/crates/nv-flip
- docs.rs: https://docs.rs/nv-flip/latest/nv_flip/
- repo: https://github.com/gfx-rs/nv-flip-rs
- wgpu `tests/src/image.rs`: https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/tests/src/image.rs
- wgpu PR #2767 (raise max_outliers — the RADV POLARIS12 outlier-flake evidence, pre-FLIP): https://github.com/gfx-rs/wgpu/pull/2767
- wgpu issue #2760 (WARP unexpectedly passes water example — per-driver-outcome the outlier model can't express): https://github.com/gfx-rs/wgpu/issues/2760
- wgpu PR #3830 (migrate to nv-flip — the actual outlier→FLIP replacement): https://github.com/gfx-rs/wgpu/pull/3830
- FLIP paper: "FLIP: A Difference Evaluator for Alternating Images" (HPG 2020), NVIDIA Labs
- Sibling files: [gpu-test-harness.md](gpu-test-harness.md), [determinism-rasterizer.md](determinism-rasterizer.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md), [glossary.md](glossary.md)
