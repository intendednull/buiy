**Date:** 2026-06-14
**Status:** active
**Subject:** Glossary of wgpu-test-infrastructure terms — one line each, for readers of this folder

# Glossary

System-specific terms used across this folder. One line each; see the linked file for detail.

## Harness

- **`wgpu_test`** — wgpu's in-tree GPU integration-test harness crate (under `tests/`), **not published** to crates.io. ([gpu-test-harness.md](gpu-test-harness.md))
- **`#[gpu_test]`** — attribute macro that turns a `static GpuTestConfiguration` into a test running on all GPUs on the system.
- **`GpuTestConfiguration`** — the value a `#[gpu_test]` static holds: bundles `TestParameters` + the async test closure.
- **`TestingContext`** — the device, queue, and adapter info handed to a test closure.
- **`TestParameters`** — preconditions (required features/limits/downlevel caps/instance flags) + expectations (`skips`, `failures`); unmet preconditions → **skip, not fail**.
- **`FailureCase`** — a matcher (`backend()`, `adapter(substr)`, `validation_error()`, `panic()`, `.flaky()`) declaring an expected failure scoped to backend × adapter-substring × driver.
- **`FailureBehavior`** — enum: `AssertFailure` (default; an unexpected **pass panics**) vs `Ignore` (swallow a specific flake).
- **`.expect_fail(when)` / `.skip(when)`** — builder hooks attaching a `FailureCase` as a must-fail (run anyway) or must-fail-and-skip.
- **`execute_test`** — dispatcher that runs a `GpuTestConfiguration` against an adapter report.

## Tooling / runner

- **`cargo-nextest`** — the mandated test runner; gives **process-per-test isolation** so a device crash can't poison the run.
- **`cargo xtask test` / `cargo xtask cts`** — repo-root entry points; `xtask test` calls nextest, `xtask cts` runs the conformance suite.
- **`wgpu-info`** — sweep tool: runs a given command once per (adapter × backend), setting `WGPU_ADAPTER_NAME` / `WGPU_BACKEND` each run.
- **`WGPU_BACKEND` / `WGPU_ADAPTER_NAME` / `WGPU_DX12_COMPILER`** — env vars selecting backend (comma list), adapter (substring), and DX12 shader compiler.

## Rasterizers (the reference)

- **lavapipe** — Mesa's `swrast` software **Vulkan** driver (`libvulkan_lvp.so`, ICD `lvp_icd.x86_64.json`); wgpu's Vulkan reference. Self-warns at init (Mesa `lvp_device.c`): "WARNING: lavapipe is not a conformant vulkan implementation, testing use only."
- **llvmpipe** — Mesa's Gallium software **OpenGL/GLES** rasterizer; wgpu's GL reference (`GALLIUM_DRIVER=llvmpipe`).
- **WARP** — Microsoft's software **D3D** rasterizer (`d3d10warp.dll`); the DX12 reference, installed via `cargo xtask install-warp`.
- **Mesa** — the open-source graphics-driver project that builds lavapipe + llvmpipe; pinned at `MESA_VERSION` (currently `25.2.7`).
- **`gfx-rs/ci-build`** — repo that builds Mesa from `archive.mesa3d.org` on a tag and attaches a tarball to a GH Release; wgpu downloads from it. Current build tag: `build26`.
- **ICD** — *Installable Client Driver* — the JSON manifest the Vulkan loader reads to find a driver `.so`; wgpu writes its own because the upstream one has a build-host-absolute path.
- **`VK_DRIVER_FILES`** — env var pointing the Vulkan loader at a specific ICD JSON (modern replacement for deprecated `VK_ICD_FILENAMES`); forces lavapipe-only enumeration.
- **`GALLIUM_DRIVER` / `LIBGL_ALWAYS_SOFTWARE`** — Mesa env vars forcing software GL (llvmpipe).
- **`LP_NUM_THREADS`** — Mesa env var for llvmpipe render-thread count; **commonly mis-cited** as a wgpu determinism knob — it is **not** how wgpu achieves determinism. ([determinism-rasterizer.md](determinism-rasterizer.md))

## Image comparison

- **FLIP (ꟻLIP)** — NVIDIA's perceptual image-difference metric ("FLIP: A Difference Evaluator for Alternating Images", HPG 2020); models viewer distance so sub-perceptual noise doesn't register.
- **`nv-flip`** — Rust high-level bindings to FLIP (v0.1.2, MIT OR Apache-2.0 OR Zlib, gfx-rs-maintained); `nv-flip-sys` is its C++ FFI layer.
- **error map** — FLIP's per-pixel output, a `FlipImageFloat` with values in `[0,1]` (0 = identical, 1 = max perceptual error).
- **`FlipPool`** — histogram-like value pool over an error map exposing `mean()`, `get_percentile()`, `min_value()`, `max_value()`; **mean** is the authors' recommended summary.
- **pixels-per-degree** — FLIP's viewer-distance parameter (`DEFAULT_PIXELS_PER_DEGREE`) that makes the metric perceptual rather than a raw diff.
- **`ComparisonType`** — wgpu's assertion enum: `Mean(f32)` (fail if mean error > x) or `Percentile { percentile, threshold }`.
- **`magma_lut()`** — the magma colormap applied to an error map to produce the human-readable `*-difference.png` diff artifact.
- **outlier count / `max_outliers`** — the **superseded** pre-FLIP model: count per-channel-delta-exceeding pixels, fail if over a limit; brittle across drivers.

## Cross-cutting

- **CTS** — the (Vulkan/WebGPU) **conformance** test suite wgpu runs *separately* via `cargo xtask cts`; carries the correctness load that goldens cannot.
- **trunk** — wgpu's default branch; all `blob/trunk/...` source links in this folder resolve against it and may drift.

## Sources

- `wgpu_test` docs: https://wgpu.rs/doc/wgpu_test/index.html
- `tests/src/expectations.rs`, `tests/src/params.rs`, `tests/src/image.rs`: https://github.com/gfx-rs/wgpu/tree/trunk/tests/src
- `install-mesa/action.yml`: https://github.com/gfx-rs/wgpu/blob/trunk/.github/actions/install-mesa/action.yml
- Mesa envvars: https://docs.mesa3d.org/envvars.html
- lavapipe non-conformance warning (Mesa `lvp_device.c`): https://gitlab.freedesktop.org/mesa/mesa/-/blob/main/src/gallium/frontends/lavapipe/lvp_device.c
- `nv-flip`: https://docs.rs/nv-flip/latest/nv_flip/
- Sibling files: [README.md](README.md), [gpu-test-harness.md](gpu-test-harness.md), [determinism-rasterizer.md](determinism-rasterizer.md), [image-compare.md](image-compare.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md)
