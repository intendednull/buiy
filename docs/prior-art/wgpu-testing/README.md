**Date:** 2026-06-14
**Status:** active
**Subject:** wgpu's CI / GPU test infrastructure — the closest determinism model for Buiy (folder index + entry point)

# wgpu's CI / GPU test infrastructure

Buiy renders on **wgpu**, so wgpu's own test suite — the `wgpu_test` harness, its pinned-software-rasterizer determinism recipe, and its `nv_flip` perceptual image-compare — is the most directly transplantable prior art in this corpus. Everything in this folder runs on the same wgpu abstraction Buiy targets; the determinism contract wgpu engineered (same CPU-rasterizer bits everywhere → reproducible pixels) is exactly the contract Buiy's golden/reftest tiers need. This is the *infrastructure* prior-art behind [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md): wgpu does not invent a new visual-testing *methodology* (that's the reftest/Gold folders), it shows how to make GPU output **reproducible enough to test at all**, and how to express **per-backend expected outcomes** instead of globally disabling a test.

The three load-bearing pieces, each its own file:

1. **`gpu_test` harness** — the `#[gpu_test]` macro, `TestParameters` (feature/limit gating → skip-not-fail), and `FailureCase` (per-backend × adapter-substring × driver expectations, where an *unexpected pass panics*). This is the strongest idea here for Buiy's reftest tier.
2. **Pinned software rasterizer** — lavapipe/llvmpipe/WARP frozen at a single `MESA_VERSION`, vendored via `gfx-rs/ci-build`, selected via `VK_DRIVER_FILES` + `WGPU_ADAPTER_NAME`. This is the determinism contract.
3. **`nv_flip` image compare** — perceptual FLIP error map → mean/percentile threshold, magma diff artifact, implicit golden bootstrapping. This is the metric.

The decision content (what Buiy should Borrow / Avoid / treat as Validated) lives in [lessons.md](lessons.md).

## Key facts (verified 2026-06-14 against the cited primary sources)

| Fact | Value | Source |
|---|---|---|
| Latest wgpu | **v29.0.3**, released **2026-05-02** | [crates.io API](https://crates.io/api/v1/crates/wgpu) |
| Harness crate | `wgpu_test` — in-tree under `tests/`, **not published** to crates.io | [wgpu.rs/doc/wgpu_test](https://wgpu.rs/doc/wgpu_test/index.html) |
| Test unit | a `static GpuTestConfiguration` annotated `#[gpu_test]` | [wgpu_test docs](https://wgpu.rs/doc/wgpu_test/index.html) |
| Gating | `TestParameters` — unmet feature/limit/downlevel → **skip, not fail** | [`tests/src/params.rs`](https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/params.rs) |
| Per-backend expectations | `FailureCase` — `backend()` / `adapter(substr)` / `validation_error()` / `panic()` / `.flaky()` | [`tests/src/expectations.rs`](https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/expectations.rs) |
| Behavior enum | `FailureBehavior::{AssertFailure (default), Ignore}` — **AssertFailure: an unexpected pass panics** | [`expectations.rs`](https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/expectations.rs) |
| Runner | `cargo-nextest` (process-per-test isolation), driven by `cargo xtask test` | [`docs/testing.md`](https://github.com/gfx-rs/wgpu/blob/trunk/docs/testing.md) |
| Backend sweep | `wgpu-info <cmd>` runs `<cmd>` once per (adapter × backend), setting `WGPU_BACKEND` / `WGPU_ADAPTER_NAME` | [lib.rs/crates/wgpu-info](https://lib.rs/crates/wgpu-info) |
| Reference rasterizer (Vk) | **lavapipe** (`libvulkan_lvp.so`, `lvp_icd.x86_64.json`) | [install-mesa action](https://github.com/gfx-rs/wgpu/blob/trunk/.github/actions/install-mesa/action.yml) |
| Reference rasterizer (GL) | **llvmpipe** (`GALLIUM_DRIVER=llvmpipe`) | install-mesa action |
| Reference rasterizer (DX12) | Microsoft **WARP** (`d3d10warp.dll`, via `cargo xtask install-warp`) | install-warp action |
| Pinned Mesa version | `MESA_VERSION: "25.2.7"`, ci-binary-build `build26` (Nov 18) | install-mesa action; [ci-build releases](https://github.com/gfx-rs/ci-build/releases) |
| Pin host | `gfx-rs/ci-build` — builds Mesa from `archive.mesa3d.org` on a tag, attaches tarball to a GH Release | [ci-build artifacts.yml](https://github.com/gfx-rs/ci-build) |
| Adapter selection | `VK_DRIVER_FILES=$PWD/icd.json` + `WGPU_ADAPTER_NAME` (case-insensitive substring) | [Mesa envvars](https://docs.mesa3d.org/envvars.html), [wgpu util](https://docs.rs/wgpu/latest/wgpu/util/fn.initialize_adapter_from_env.html) |
| Image-compare crate | **`nv-flip` 0.1.2** (2023-07-16), MIT OR Apache-2.0 OR Zlib | [crates.io/nv-flip](https://crates.io/crates/nv-flip), [docs.rs](https://docs.rs/nv-flip/latest/nv_flip/) |
| Metric | NVIDIA **ꟻLIP** per-pixel error map ∈ [0,1]; summary = **mean** (authors' recommendation) | [docs.rs/nv-flip](https://docs.rs/nv-flip/latest/nv_flip/) |
| Assertion model | `ComparisonType::{Mean(f32), Percentile{percentile, threshold}}` | [`tests/src/image.rs`](https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/tests/src/image.rs) |
| Diff artifact | magma-colormapped error map → `{stem}-{backend}-{name}-{driver}-difference.png` | `tests/src/image.rs` |
| Superseded model | raw **outlier-count** (`max_outliers`) — brittle across drivers, **replaced by FLIP in [PR #3830](https://github.com/gfx-rs/wgpu/pull/3830)** ("Migrate to nv-flip for image comparison") | brittleness evidence: [PR #2767](https://github.com/gfx-rs/wgpu/pull/2767), [issue #2760](https://github.com/gfx-rs/wgpu/issues/2760) |

## Contents

Each file is independently skimmable with its own `## Sources`.

| File | Subject |
|---|---|
| [README.md](README.md) | This index — what wgpu's test infra is, key facts, reading order. |
| [lessons.md](lessons.md) | **The decision file.** `## Validates` / `## Avoid` / `## Borrow` — where Buiy implications live. Start here when designing. |
| [gpu-test-harness.md](gpu-test-harness.md) | The `#[gpu_test]` macro, `TestParameters` skip-gating, `FailureCase` per-backend expectations, the unexpected-pass-panics rule, `wgpu-info` sweep, nextest/xtask runner. |
| [determinism-rasterizer.md](determinism-rasterizer.md) | Why CPU rasterizers are the reference, the abandoned daily-PPA wart, `gfx-rs/ci-build` pinning, `VK_DRIVER_FILES` adapter selection, the `LP_NUM_THREADS` myth, the upgrade-treadmill cost. |
| [image-compare.md](image-compare.md) | `nv-flip` / FLIP metric, `FlipPool` mean/percentile reduction, `ComparisonType` assertions, magma diff artifact, implicit golden bootstrapping, the superseded outlier-count model. |
| [open-problems.md](open-problems.md) | What wgpu's stack structurally does *not* solve: flakiness as a first-class state, substring brittleness, lavapipe non-conformance, silent golden minting, the manual pin treadmill, FFI cost. |
| [glossary.md](glossary.md) | System-specific terms: `gpu_test`, `TestParameters`, `FailureCase`, lavapipe/llvmpipe/WARP, ICD, FLIP, `FlipPool`, nextest, `wgpu-info`. |

## Reading order

1. **[lessons.md](lessons.md)** — the decisions. Start here if you are designing Buiy's reftest/golden harness.
2. **[gpu-test-harness.md](gpu-test-harness.md)** — the `FailureCase` model is the single most transplantable idea; read it first for the referent.
3. **[determinism-rasterizer.md](determinism-rasterizer.md)** — the pinned-rasterizer recipe that makes any pixel test possible at all.
4. **[image-compare.md](image-compare.md)** — the perceptual metric Buiy's Tier-4/5 comparison should adopt.
5. **[open-problems.md](open-problems.md)** — the limits, so Buiy doesn't over-trust the stack.
6. **[glossary.md](glossary.md)** — reference when a term is unclear.

## Framing disclosure

This folder is written from Buiy's stance: an ECS-native (Bevy 0.18) retained-mode Rust GUI library with a custom `wgpu` pipeline, designing a reftests-first visual-bug-detection pyramid. Because Buiy is *built on the same wgpu*, this is the only prior-art folder whose mechanisms are nearly copy-pasteable rather than adapted — the `VK_DRIVER_FILES` recipe, the `FailureCase` primitive, and `nv_flip` itself are all directly reusable, and the `gfx-rs/ci-build` artifacts can be consumed as-is. "Implications for Buiy" lines therefore lean toward direct reuse. The evidence files describe wgpu's systems on their own terms and surface unflattering facts verbatim (lavapipe's "testing use only" self-warning, the abandoned daily-PPA, the substring-brittleness wart); Buiy implications are confined to clearly-labelled subsections and to [lessons.md](lessons.md). One dossier claim — that wgpu pins `LP_NUM_THREADS` for FP determinism — is **flagged as not how wgpu does it**; see [determinism-rasterizer.md](determinism-rasterizer.md).

## How to use

**Framing disclosure.** These docs are written from Buiy's stance — an AccessKit-first, wgpu + Taffy + cosmic-text, parallel-to-bevy_ui retained-mode engine building a reftests-first layered visual-bug-detection strategy. The "Implications for Buiy" / lessons framing reads wgpu's CI / GPU test infrastructure through that lens; readers auditing whether that strategy is itself right should weigh the corpus accordingly — it is a learn-from artifact, not a neutral catalog.

## Sources

- wgpu crate (crates.io API): https://crates.io/api/v1/crates/wgpu
- `wgpu_test` docs: https://wgpu.rs/doc/wgpu_test/index.html
- `tests/src/params.rs`: https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/params.rs
- `tests/src/expectations.rs`: https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/expectations.rs
- `tests/src/image.rs`: https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/tests/src/image.rs
- `docs/testing.md`: https://github.com/gfx-rs/wgpu/blob/trunk/docs/testing.md
- `install-mesa/action.yml`: https://github.com/gfx-rs/wgpu/blob/trunk/.github/actions/install-mesa/action.yml
- `gfx-rs/ci-build`: https://github.com/gfx-rs/ci-build
- `nv-flip` (crates.io / docs.rs / repo): https://crates.io/crates/nv-flip · https://docs.rs/nv-flip/latest/nv_flip/ · https://github.com/gfx-rs/nv-flip-rs
- wgpu outlier→FLIP migration: PR #3830 (the replacement) https://github.com/gfx-rs/wgpu/pull/3830 ; brittleness evidence PR #2767 https://github.com/gfx-rs/wgpu/pull/2767 · issue #2760 https://github.com/gfx-rs/wgpu/issues/2760
- Mesa envvars: https://docs.mesa3d.org/envvars.html
- Sibling files: [gpu-test-harness.md](gpu-test-harness.md), [determinism-rasterizer.md](determinism-rasterizer.md), [image-compare.md](image-compare.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md), [glossary.md](glossary.md)
- Sibling prior art: [../wpt-reftests/](../wpt-reftests/), [../skia-gold/](../skia-gold/), [../vello/](../vello/)
- Buiy strategy report: [../../reports/2026-06-14-visual-bug-detection-strategy.md](../../reports/2026-06-14-visual-bug-detection-strategy.md)
