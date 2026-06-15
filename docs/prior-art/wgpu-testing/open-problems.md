**Date:** 2026-06-14
**Status:** active
**Subject:** What wgpu's CI / GPU test infrastructure structurally does NOT solve — the limits Buiy inherits if it copies the stack

# Open problems — what wgpu's test stack does not solve

wgpu's harness is the closest determinism model Buiy has, but it has hard structural limits. These are boundaries, not gaps in effort; Buiy inherits each one if it copies the stack, and several need an explicit Buiy-side mitigation.

## 1. Flakiness is encoded as a first-class state, not fixed

The harness openly treats some failures as permanently non-deterministic: `.flaky()` ("Test is flaky with the given configuration. Do not assert failure") and `FailureBehavior::Ignore` ("useful for tests that flake in a very specific way"). This is a pragmatic admission that some driver behavior cannot be made deterministic — but it means an `Ignore`d case provides **no regression-catching signal at all**: it passes whether the code is right or wrong. The wpt-reftests folder documents the same hazard with intermittent reftests forced into a `0`-inclusive fuzz range. **Buiy mitigation:** treat `Ignore`/`.flaky()` as a quarantine with an owner and an expiry, not a permanent home — count quarantined tests as a debt metric, not green.

## 2. Substring matching is brittle across version bumps

Expectations are keyed on **adapter-name substrings** (`adapter("llvmpipe")`) and **error-message substrings** (`validation_error(msg)`, `.with_message(...)`, case-insensitive). A reworded validation message or a renamed adapter silently breaks the match: the harness either stops asserting the expected failure or trips on an unexpected one. Driver-version bumps are exactly when both strings change. **Buiy mitigation:** prefer structured keys (an enum'd backend + a stable adapter-class id) over free-text substrings wherever Buiy controls the message; pin the message text in the same module that emits it.

## 3. The reference rasterizer is "testing use only" and ships its own bugs

lavapipe self-warns at init — Mesa's `src/gallium/frontends/lavapipe/lvp_device.c` does `fprintf(stderr, "WARNING: lavapipe is not a conformant vulkan implementation, testing use only.\n")`. The wgpu [Known-Driver-Issues wiki](https://github.com/gfx-rs/wgpu/wiki/Known-Driver-Issues) lists Mesa segfaults (query-pool reset with acceleration-structure info), and live issues show feature gaps — [#8727](https://github.com/gfx-rs/wgpu/issues/8727) "SPIR-V writing for mesh shaders is broken on llvmpipe", and the [#8544](https://github.com/gfx-rs/wgpu/issues/8544) ray-tracing limit workaround that had to wait for Mesa 25.2.7. **Consequence:** golden pixels produced by the pinned rasterizer prove *no-change*, **not correctness**. They cannot be ground truth. wgpu carries correctness separately via CTS (conformance), run through `cargo xtask cts`. **Buiy mitigation:** pair goldens with Buiy's lower tiers (layout-number / display-list snapshots, metamorphic invariants) which carry the correctness load; never let a golden be the only assertion about a behavior.

## 4. Goldens silently mint themselves on first run

When no reference exists, `read_png` returns `None` and `compare_image_output`'s `None =>` arm writes the current image as the new baseline and returns. A **missing or deleted golden silently passes** by regenerating itself — so an accidental `rm` of a baseline hides a regression instead of failing. **Buiy mitigation:** make first-run minting explicit (an opt-in flag), and fail loudly when an expected golden is absent.

## 5. Pinning trades flakes for a manual upgrade treadmill

The frozen-Mesa recipe removes day-to-day flakes but creates a **manual** cost: every bump needs a new `gfx-rs/ci-build` release *and* an edit to `install-mesa/action.yml`, and behavior changes must be chased by hand (e.g. restoring `Limits::blas_max_primitive_count` only after Mesa 25.2.7 fixed the underlying bug — [#8544](https://github.com/gfx-rs/wgpu/issues/8544)). The pin can also lag a real fix the project needs. **Buiy mitigation:** accept the treadmill as the price of determinism; bump in a tracked issue and regenerate goldens in the same PR.

## 6. Supply-chain trust gap on Windows

The Linux Mesa is built by gfx-rs itself, but the **Windows** build is pulled from a **third-party** repo, [pal1000/mesa-dist-win](https://github.com/pal1000/mesa-dist-win) — not gfx-rs-controlled. Anyone copying the recipe inherits trust in that third party for the Windows reference binary. **Buiy mitigation:** if Buiy needs deterministic DX12 pixels, prefer building its own Windows Mesa or pin a hash of the third-party artifact.

## 7. `nv_flip` is FFI to a C++ library

The metric is `nv-flip` → `nv-flip-sys` → a C++ FLIP implementation: a native build-graph dependency in CI (a C++ toolchain, a `-sys` crate). It is fine, and license-clean, but it is not pure Rust. **Runner-up:** a pixelmatch-YIQ port is cheaper to vendor but a weaker perceptual model (no edge-contrast term → more AA false-positives). Tradeoff named in [image-compare.md](image-compare.md) and [lessons.md](lessons.md).

## 8. Determinism assumes a fixed backend per CI lane

The whole pinned-rasterizer guarantee holds **within one backend**. The harness disambiguates failures per backend (the `{backend}-{name}-{driver}` diff naming) precisely because different backends produce different pixels. A golden compared across Vulkan-vs-DX12 would reintroduce the variance the pin removes. **Buiy mitigation:** one golden per (backend) cell, or run goldens on a single pinned backend only.

**No software-Metal reference — macOS goldens are not deterministic under this model.** The pinned-rasterizer recipe covers Vulkan (lavapipe), GL (llvmpipe), and DX12 (WARP) only. There is no software Metal rasterizer: macOS Metal goldens would run on a real Apple GPU/driver and so are *not* bit-stable across machines or OS versions. This is load-bearing for Buiy, which targets macOS — Buiy cannot get deterministic Metal pixels from this recipe and must either route macOS visual tests through MoltenVK→lavapipe (paying a translation layer) or accept that the Metal backend has no golden tier and rely on its lower tiers there.

## 9. Documentation drift

A current standalone `docs/testing/integration_tests.md` could **not** be located (404 on trunk) — testing docs are consolidated in [`docs/testing.md`](https://github.com/gfx-rs/wgpu/blob/trunk/docs/testing.md). Older per-file links in external write-ups are stale. Verify against trunk before lifting any path. (Likewise the `Skip`/`device`/`environment` `FailureCase` variants some write-ups mention are **not** in current source.)

## Not covered by this folder

These are out of scope for the prior-art (not faults in wgpu's stack) but a Buiy designer sizing the golden tier should source them elsewhere:

- **CI cost / throughput.** No figures on how long the GPU suite or image-compare run takes, what runner class CI uses, or per-test overhead — no budget anchor for sizing Buiy's golden tier.
- **Golden-image storage cost.** PNGs are committed in-repo (see [image-compare.md](image-compare.md)) but the repo-bloat / Git-LFS-vs-not tradeoff that bites every screenshot suite is not analyzed here.
- **Per-test threshold selection.** [image-compare.md](image-compare.md) gives the `[0.01, 0.1]` empirical range but not wgpu's actual manual tuning loop for picking a per-test threshold — the operationally hard part.

## Sources

- lavapipe non-conformance warning (Mesa source, `fprintf(stderr, …)`): https://gitlab.freedesktop.org/mesa/mesa/-/blob/main/src/gallium/frontends/lavapipe/lvp_device.c
- wgpu `docs/testing.md`: https://github.com/gfx-rs/wgpu/blob/trunk/docs/testing.md
- Known-Driver-Issues wiki: https://github.com/gfx-rs/wgpu/wiki/Known-Driver-Issues
- wgpu issues #8544 / #8727: https://github.com/gfx-rs/wgpu/issues/8544 · https://github.com/gfx-rs/wgpu/issues/8727
- `tests/src/expectations.rs` (flaky / Ignore / substring matchers): https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/expectations.rs
- `tests/src/image.rs` (implicit golden mint): https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/tests/src/image.rs
- pal1000/mesa-dist-win (third-party Windows Mesa): https://github.com/pal1000/mesa-dist-win
- Sibling files: [gpu-test-harness.md](gpu-test-harness.md), [determinism-rasterizer.md](determinism-rasterizer.md), [image-compare.md](image-compare.md), [lessons.md](lessons.md), [glossary.md](glossary.md)
