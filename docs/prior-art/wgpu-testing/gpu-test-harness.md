**Date:** 2026-06-14
**Status:** active
**Subject:** wgpu's `wgpu_test` GPU integration-test harness — the `#[gpu_test]` macro, skip-gating, and per-backend `FailureCase` expectations

# The `wgpu_test` GPU test harness

`wgpu_test` is wgpu's in-tree GPU integration-test harness, documented at [wgpu.rs/doc/wgpu_test](https://wgpu.rs/doc/wgpu_test/index.html) and tersely self-described as "Test utilities for the wgpu repository." It is **not published to crates.io** — it lives under `tests/` in the repo. Documented here against the latest published wgpu, **v29.0.3 (released 2026-05-02)**, verified via the crates.io API (`max_stable_version` 29.0.3, `updated_at` 2026-05-02T03:12:40Z).

The harness exists to solve one problem Buiy shares: **run one test body across heterogeneous GPUs and record per-GPU expected outcomes**, instead of `cfg`-gating tests per platform or globally `#[ignore]`ing anything that fails on one backend.

## The `#[gpu_test]` macro

Each GPU test is a `static` of type `GpuTestConfiguration` annotated with `#[gpu_test]`. The macro "creates a test that will run on all gpus on a given system" by generating the harness `main`/registration glue. A test bundles three things:

- **`TestParameters`** — preconditions (features/limits) + expectations (`FailureCase`s).
- **an async closure** receiving a **`TestingContext`** — "Parameters and resources handed to the test function": the device, queue, and adapter info.

Tests are dispatched through `execute_test`, which "Execute[s] the given test configuration with the given adapter report"; `initialize_instance` / `initialize_adapter` / `initialize_device` perform per-adapter setup.

## `TestParameters` — gating (skip, not fail)

From [`tests/src/params.rs`](https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/params.rs), the struct carries:

```
required_features: Features
required_downlevel_caps: DownlevelCapabilities
required_limits: Limits
required_instance_flags: InstanceFlags
force_fxc: bool
skips: Vec<FailureCase>
failures: Vec<FailureCase>
disable_mtl_shader_validation: bool
```

Builder methods: `.features(..)`, `.downlevel_flags(..)`, `.limits(..)`, `.instance_flags(..)`, `.force_fxc(..)`, `.test_features_limits()` ("Set of common features that most internal tests require for compute and readback"), `.enable_noop()` ("Enable testing against the noop backend and miri"), plus the two expectation hooks:

- `.expect_fail(when: FailureCase)` — "Mark the test as always failing, but not to be skipped."
- `.skip(when: FailureCase)` — "Mark the test as always failing, and needing to be skipped."

**The load-bearing behavior:** if a feature/limit/downlevel precondition is not met by the current adapter, the test is **skipped, not failed**. This is how a single test body runs cleanly across hardware with different capabilities — the harness silently drops tests the adapter can't support rather than reporting red.

## `FailureCase` — per-backend / adapter / driver expectations

From [`tests/src/expectations.rs`](https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/expectations.rs). This is the heart of the determinism model. The matcher constructors:

- `always()`, `never()`
- `backend(Backends)` — "Tests running on any of the given backends."
- `adapter(&str)` — "Tests running on `adapter`" (**substring match** on the adapter name).
- `backend_adapter(backends, adapter)`
- `webgl2()`, `molten_vk()` ("the MoltenVK Vulkan driver on macOS"), `kosmic_krisp()`, `mac_vulkan(..)` ("either Vulkan driver on macOS")
- `validation_error(msg)` and `panic(msg)` — substring-match the expected error/panic text.
- `unexpected_error(msg)`

A case is refined with reason filters and `.with_message(..)` (case-insensitive substring matching on the validation-error/panic message), and `.flaky()` — "Test is flaky with the given configuration. Do not assert failure."

The behavior enum is **`FailureBehavior`** with two variants:

- **`AssertFailure`** — "Assert that the test fails for the given reason. If the test passes, the test harness will panic." This is the strict default for `.expect_fail`.
- **`Ignore`** — "Ignore the matching failure. This is useful for tests that flake in a very specific way."

### The unexpected-pass-panics rule (the key insight)

Under `AssertFailure`, a known-broken case **must keep failing in exactly the matched way**. If a backend *starts passing* — e.g. a driver bug gets fixed upstream — the harness **panics**, forcing whoever made it pass to delete the now-stale expectation. A backend cannot silently start passing; the expectation list stays honest as the renderer matures. This is far stronger than `#[ignore]`, which would silently keep the test disabled forever.

The directly transplantable insight for Buiy: expectations are **scoped to `backend × adapter-substring × driver`**, so a single test asserts a *different correct outcome per GPU* rather than being globally disabled. `FailureReason`/`FailureResult` do case-insensitive substring matching on the message.

> **Note (verified):** the older `Skip`/`device`/`environment` `FailureCase` variants implied by some external write-ups are **not present** in current source — verified against trunk. Don't lift them.

## Backend / adapter sweep

All test/example infra reads standardized env vars: `WGPU_BACKEND` (comma list of `vulkan`, `metal`, `dx12`, `gl`), `WGPU_ADAPTER_NAME` (adapter-name substring), and `WGPU_DX12_COMPILER`. The sweep tool is **`wgpu-info`**: "when wgpu-info is called with any amount of arguments, it will interpret all of the arguments as a command to run. It will run this command N different times, one for every combination of adapter and backend on the system," setting `WGPU_ADAPTER_NAME` and `WGPU_BACKEND` per run ([lib.rs/crates/wgpu-info](https://lib.rs/crates/wgpu-info)). Canonical invocation:

```
cargo run --bin wgpu-info -- cargo nextest run --no-fail-fast
```

## Runner: nextest + xtask

Tests **must** run under `cargo-nextest`, which gives **process-per-test isolation** — important because a GPU device crash or validation abort would poison a shared process. Per [`docs/testing.md`](https://github.com/gfx-rs/wgpu/blob/trunk/docs/testing.md): "you require you run the tests with cargo-nextest. This is what our xtask calls."

The repo-root entry point is `cargo xtask test`:
- `cargo xtask test --test wgpu-gpu` — the GPU tests.
- `cargo xtask test --bin wgpu-examples` — the image-comparison example tests.
- `cargo xtask cts` — runs CTS (the conformance suite, separate from these integration tests).
- Default-device run: `cargo nextest run --no-fail-fast`; single test: `cargo nextest run -p wgpu -- <name>`.

## Implications for Buiy

Buiy's reftest tier should adopt the `FailureCase` model **verbatim**, including the unexpected-pass-panics rule: a Buiy reftest that's known-broken on, say, the Vulkan backend records `expect_fail(backend(VULKAN))`, and if a Buiy renderer fix makes it pass, the harness forces the expectation's removal. Skip-on-unmet-precondition maps to Buiy's optional GPU features. Process-per-test isolation via nextest is already idiomatic in Rust and avoids one crashed `wgpu::Device` poisoning a whole test run — Buiy should mandate nextest for any test that creates a device. See [lessons.md](lessons.md) for the full Borrow list, and [open-problems.md](open-problems.md) for the substring-brittleness wart this model carries.

## Sources

- `wgpu_test` docs: https://wgpu.rs/doc/wgpu_test/index.html
- `tests/src/params.rs`: https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/params.rs
- `tests/src/expectations.rs`: https://github.com/gfx-rs/wgpu/blob/trunk/tests/src/expectations.rs
- `docs/testing.md`: https://github.com/gfx-rs/wgpu/blob/trunk/docs/testing.md
- wgpu README (env vars): https://github.com/gfx-rs/wgpu/blob/trunk/README.md
- `wgpu-info`: https://lib.rs/crates/wgpu-info
- wgpu crate version (crates.io API): https://crates.io/api/v1/crates/wgpu
- Sibling files: [determinism-rasterizer.md](determinism-rasterizer.md), [image-compare.md](image-compare.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md), [glossary.md](glossary.md)
