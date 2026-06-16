**Date:** 2026-06-14
**Status:** active
**Subject:** Lessons for Buiy from wgpu's CI / GPU test infrastructure — the closest determinism model; what it Validates, what to Avoid, what to Borrow

# Lessons for Buiy

This is the consult-this-when-designing file. The other files in this folder are evidence; this file is decisions. wgpu's test infrastructure is **uniquely transplantable** because Buiy renders on the same wgpu abstraction — the `VK_DRIVER_FILES` determinism recipe, the `FailureCase` expectation primitive, and the `nv_flip` metric are all *directly reusable*, not merely instructive. Most Buiy golden/reftest-infra decisions reduce to "how closely do we copy wgpu's stack?" — this file enumerates the answers. It feeds [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md), especially Tiers 4–5.

## Validates

These Buiy design choices are confirmed by wgpu's experience and stack shape:

- **Reftests-first, goldens-last and few.** wgpu image-compares only its *examples*, sitting atop a deep harness of non-visual GPU tests; goldens are the thin top of the pyramid, and *correctness* is carried separately by CTS. Buiy's reftests-first ordering — keep goldens last and minimal — is the same shape wgpu arrived at. See [image-compare.md](image-compare.md), [open-problems.md § 3](open-problems.md).
- **A perceptual metric, not exact-pixel.** wgpu *replaced* its exact-ish outlier-count model with perceptual FLIP ([PR #3830 "Migrate to nv-flip for image comparison"](https://github.com/gfx-rs/wgpu/pull/3830), merged 2023-06-08) precisely because scattered sub-perceptual AA/rounding noise tripped the count across drivers. The brittleness is documented in [PR #2767](https://github.com/gfx-rs/wgpu/pull/2767) (RADV POLARIS12: *"just a few dots"* tripping `Outlier count 464 over limit 460`) and [issue #2760](https://github.com/gfx-rs/wgpu/issues/2760) (WARP unexpectedly *passing* — an outcome the count model can't express per-driver). Buiy's plan to use a perceptual tolerance instead of bit-equality is validated by the system closest to it abandoning the alternative.
- **Pin the rasterizer, don't track the distro.** wgpu's abandonment of `ppa:oibaf/graphics-drivers` ([#2594](https://github.com/gfx-rs/wgpu/issues/2594)) is direct evidence that a rolling software rasterizer is a *moving reference image* — every unrelated upstream regression reddens CI. Buiy's intent to pin a single rasterizer version is the correct conclusion. See [determinism-rasterizer.md](determinism-rasterizer.md).
- **Process-per-test isolation.** nextest is the right runner for any test that creates a `wgpu::Device`; a device crash/validation abort must not poison a shared process. Buiy already lives in the nextest-friendly Rust ecosystem. See [gpu-test-harness.md](gpu-test-harness.md).
- **Per-backend expectations beat global disabling.** The `FailureCase` model lets one test body assert a different correct outcome per GPU. This validates designing Buiy's reftest tier around declarative per-backend expectations rather than `#[ignore]` or `cfg`. See [gpu-test-harness.md](gpu-test-harness.md).

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| Trusting lavapipe pixels as ground truth for *correctness* | lavapipe self-warns at init: `"WARNING: lavapipe is not a conformant vulkan implementation, testing use only."` (Mesa `lvp_device.c`); ships version-pinned bugs ([#8727](https://github.com/gfx-rs/wgpu/issues/8727), Known-Driver-Issues wiki) | Goldens prove **no-change, not correct**. Pair them with Buiy's lower tiers (layout-number / display-list snapshots, metamorphic invariants) which carry the correctness load. wgpu leans on CTS for conformance, separately. [open-problems.md § 3](open-problems.md) |
| Letting goldens be Tier 1 | wgpu image-compares only examples, atop a deep non-visual harness | Keep goldens last and few; catch most regressions in Tiers 1–4 so Tier 5 is a minimal residue. [image-compare.md](image-compare.md) |
| Copying a `LP_NUM_THREADS` determinism story | **Not** in `install-mesa/action.yml`; Mesa docs do not call it a determinism knob | Determinism comes from the **pinned Mesa version**, not thread count. Do not export `LP_NUM_THREADS` expecting FP determinism. [determinism-rasterizer.md](determinism-rasterizer.md) |
| Using the upstream Mesa ICD path | "The ICD provided by the mesa build is hardcoded to the build environment" | **Write your own ICD JSON** pointing at the unpacked `libvulkan_lvp.so`, then export `VK_DRIVER_FILES=$PWD/icd.json`. [determinism-rasterizer.md](determinism-rasterizer.md) |
| Substring-keyed expectations as the default | `adapter("llvmpipe")` / `validation_error(msg)` break silently when adapter names or messages get reworded across driver bumps | Prefer structured keys (enum'd backend + stable adapter-class id); pin message text in the module that emits it. [open-problems.md § 2](open-problems.md) |
| Treating `.flaky()` / `Ignore` as a permanent home | Provides zero regression signal — passes whether code is right or wrong | Quarantine with an owner + expiry; count quarantined tests as debt, not green. [open-problems.md § 1](open-problems.md) |
| Implicit golden bootstrapping | `compare_image_output` mints a missing baseline (its `None =>` arm writes the test image and returns) — a deleted golden hides a regression | Make first-run minting an explicit opt-in flag; fail loudly when an expected golden is absent. [image-compare.md](image-compare.md), [open-problems.md § 4](open-problems.md) |
| Trusting a third-party Windows rasterizer build | Windows Mesa pulled from non-gfx-rs [pal1000/mesa-dist-win](https://github.com/pal1000/mesa-dist-win) | If Buiy needs deterministic DX12 pixels, build its own Windows Mesa or pin a hash of the third-party artifact. [open-problems.md § 6](open-problems.md) |
| Comparing goldens across backends | The pinned-rasterizer guarantee holds within one backend only | One golden per backend cell, or run goldens on a single pinned backend. [open-problems.md § 8](open-problems.md) |
| Lifting stale APIs/paths from external write-ups | `Skip`/`device`/`environment` `FailureCase` variants and `docs/testing/integration_tests.md` are **not** in current source | Verify against trunk before copying. [gpu-test-harness.md](gpu-test-harness.md), [open-problems.md § 9](open-problems.md) |

## Borrow

Concrete subsystems and patterns worth direct adaptation — licenses align (`nv-flip` is MIT OR Apache-2.0 OR Zlib; FLIP BSD-3-Clause; both compatible with Buiy's MIT OR Apache-2.0):

1. **The `FailureCase` model verbatim — including the unexpected-pass-panics rule.** Buiy's reftest tier should record expected outcomes declaratively, scoped to `backend × adapter-substring × driver`, and *panic when an expected failure unexpectedly passes* so fixing a backend forces removal of the stale expectation. This keeps the expectation list honest as Buiy's renderer matures — far stronger than `#[ignore]`. This is the single highest-value borrow. See [gpu-test-harness.md](gpu-test-harness.md).

2. **Skip-on-unmet-precondition (`TestParameters`).** Gate a test on required features/limits/downlevel caps so unsupported adapters **skip rather than fail**. Maps directly to Buiy's optional GPU features. See [gpu-test-harness.md](gpu-test-harness.md).

3. **The pinned-rasterizer recipe, consuming `gfx-rs/ci-build` artifacts directly.** Three pieces: (a) a single `MESA_VERSION` + release-tag pin (reuse gfx-rs's prebuilt tarball — no need to build your own Mesa), (b) a composite action that downloads it and **writes its own ICD**, (c) `VK_DRIVER_FILES` + `WGPU_ADAPTER_NAME` to make adapter choice deterministic and hardware-proof. Bump the pin deliberately, in a tracked issue, regenerating goldens in the same PR. See [determinism-rasterizer.md](determinism-rasterizer.md).

4. **`VK_DRIVER_FILES` + `WGPU_ADAPTER_NAME` for adapter selection.** `VK_DRIVER_FILES` forces the Vulkan loader to see *only* lavapipe so a test can't accidentally pick a hardware GPU; `WGPU_ADAPTER_NAME` (case-insensitive substring) nails the exact device. Reuse as-is. See [determinism-rasterizer.md](determinism-rasterizer.md).

5. **`nv_flip` + mean as Buiy's perceptual metric.** The wgpu-ecosystem-native, gfx-rs-maintained, license-clean choice (directly answers verification Open Q #3). FLIP error map → `FlipPool::mean()` (the authors' recommended summary) → a per-test `Mean`/`Percentile` threshold in the empirical `[0.01, 0.1]` range. Avoids the AA-flake problem that kills exact-pixel goldens. **Runner-up named:** a pure-Rust pixelmatch-YIQ port is cheaper to vendor (no `nv-flip-sys` C++ FFI) but YIQ has no edge-contrast term → more AA false-positives. FLIP recommended; choose YIQ only if the FFI cost is unacceptable. See [image-compare.md](image-compare.md).

6. **The per-backend diff-artifact naming.** On mismatch, colorize the FLIP error map with `magma_lut()` and write `{stem}-{backend}-{name}-{driver}-difference.png`. Buiy needs this disambiguation the moment it runs goldens on more than one adapter. See [image-compare.md](image-compare.md).

7. **The `wgpu-info`-style sweep harness.** A wrapper that runs the same test command once per (adapter × backend), setting `WGPU_BACKEND` / `WGPU_ADAPTER_NAME` each run. Lets Buiy exercise its full backend matrix from one invocation. See [gpu-test-harness.md](gpu-test-harness.md).

8. **The outlier-count model as a documented cautionary precursor.** Cite wgpu's abandoned `max_outliers` approach in Buiy's strategy doc as the *negative* example — count-per-pixel tolerance is brittle across drivers — to justify the perceptual choice. See [image-compare.md](image-compare.md).

## How to use this file

When designing a Buiy golden/reftest harness component, find the relevant Avoid row and read its source file to understand the trap, then find the relevant Borrow item for the wgpu primitive to adapt. Because Buiy is on wgpu, several borrows are near-copy-paste — verify each against trunk + live crate docs before lifting concrete code (wgpu pre-`docs/testing.md` consolidation and the dropped `FailureCase` variants are both reminders that the source moves). Promote any decision into a Buiy spec under `docs/specs/`; this file captures what we learn from wgpu, not Buiy's own commitments.

## Sources

- `tests/src/expectations.rs` (`FailureCase`, `FailureBehavior`): https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/expectations.rs
- `tests/src/params.rs` (`TestParameters`): https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/params.rs
- `tests/src/image.rs` (`nv_flip`, `ComparisonType`): https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/tests/src/image.rs
- `install-mesa/action.yml` (pin + `VK_DRIVER_FILES`): https://github.com/gfx-rs/wgpu/blob/trunk/.github/actions/install-mesa/action.yml
- `gfx-rs/ci-build`: https://github.com/gfx-rs/ci-build
- wgpu issues #2594 / #8544 / #8727 / #2760 and PRs #2767 / #3830 (outlier-model brittleness + the FLIP migration): https://github.com/gfx-rs/wgpu/issues/2594 · https://github.com/gfx-rs/wgpu/issues/8544 · https://github.com/gfx-rs/wgpu/issues/8727 · https://github.com/gfx-rs/wgpu/issues/2760 · https://github.com/gfx-rs/wgpu/pull/2767 · https://github.com/gfx-rs/wgpu/pull/3830
- lavapipe non-conformance warning (Mesa `lvp_device.c`, `fprintf(stderr, …)`): https://gitlab.freedesktop.org/mesa/mesa/-/blob/main/src/gallium/frontends/lavapipe/lvp_device.c
- `nv-flip` (docs.rs / crates.io / repo): https://docs.rs/nv-flip/latest/nv_flip/ · https://crates.io/crates/nv-flip · https://github.com/gfx-rs/nv-flip-rs
- `docs/testing.md`: https://github.com/gfx-rs/wgpu/blob/trunk/docs/testing.md
- Sibling files: [gpu-test-harness.md](gpu-test-harness.md), [determinism-rasterizer.md](determinism-rasterizer.md), [image-compare.md](image-compare.md), [open-problems.md](open-problems.md), [glossary.md](glossary.md)
- Buiy strategy report: [../../reports/2026-06-14-visual-bug-detection-strategy.md](../../reports/2026-06-14-visual-bug-detection-strategy.md)
