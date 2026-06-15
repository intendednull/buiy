# Determinism stack

**Date:** 2026-06-15
**Status:** draft
**Spec:** specs/2026-06-15-buiy-verification-design/README.md

The job of this tier is narrow and load-bearing: **make every pixel test reproducible so the diff is a signal, not noise.** It engineers nondeterminism out at the source ("remove the nondeterminism, don't just tolerate it") so the perceptual metric's default fuzz budget can be `(0, 0)`. It extends the *already-built* flake triad (`GoldenConfig::deterministic()` — `fixed_clock`/`wait_for_fonts`/`warm_atlas`, `golden.rs:38`) with the missing knobs — Ahem font mode, DPR pin, MSAA/dither pinned-off, async-asset flush — exposes them through a `DeterministicApp` builder in `buiy_verify`, and pins the CI software rasterizer (lavapipe) below all of it. Reftests need this stack *less* than goldens (both halves render in one process, so residual drift cancels) but reuse the same builder.

## Contract deviations

Two deviations from the SHARED API CONTRACT, both forced by verified prior-art (`prior-art/wgpu-testing/determinism-rasterizer.md` § "The `LP_NUM_THREADS` myth"):

1. **`LP_NUM_THREADS=0` is NOT a determinism knob — dropped.** The contract and the report (§ Cross-cutting) list it as a determinism setting. wgpu's `install-mesa/action.yml` does **not** set it, and Mesa documents it only as a thread-count perf knob, never a determinism one; llvmpipe tiles per-thread so output is stable regardless of thread count. Determinism comes from the **pinned Mesa version**, not thread count. This spec does not export `LP_NUM_THREADS` and the plan author must not add it expecting FP determinism. (It may still be set to `1` as a *defensive belt-and-suspenders* with a comment that it is not the determinism source — optional, not asserted.)
2. **`VK_ICD_FILENAMES` → `VK_DRIVER_FILES`.** The contract names `VK_ICD_FILENAMES`. That variable is deprecated; the modern Vulkan-loader variable is `VK_DRIVER_FILES` (Mesa envvars; wgpu migrated). This spec uses `VK_DRIVER_FILES` (loader still honors the old name, but new CI wiring should not encode a deprecated path).

## Where the code lives

The crate split follows the contract: **app-coupled capture stays in `buiy_core::render`**, **pure config/builder lives in `buiy_verify::determinism`**.

- `buiy_core::render::golden` — extend `GoldenConfig` (below), define the **canonical `Dpr` type** (the single definition site every other tier imports — see § "Extending `GoldenConfig`"), and promote the capture entry point `capture_to_image(&mut App, &GoldenConfig) -> image::RgbaImage` out of `tests/support/mod.rs` into `golden.rs` src (the contract's shared seam; consumes the existing `gpu_render_app_scaled` / `wait_for_text_ready` / `readback_rgba` machinery, `tests/support/mod.rs:156`/`:266`/`:353`).
- `buiy_verify::determinism` — the `DeterministicApp` builder and the asserted-setup-step checklist. It *re-exports* the `FontMode`/`Dpr` config types from `buiy_core::render::golden` (their canonical home, since `GoldenConfig` carries them) rather than redefining them. Pure / app-independent: it *configures* an `App`, it does not own the GPU.
- CI rasterizer pin — a composite GitHub Action under `.github/actions/install-mesa/` + an env contract; not Rust, but specified here so the plan author wires it.

## Extending `GoldenConfig` (`buiy_core::render::golden`)

`GoldenConfig` keeps its three landed booleans and grows the four missing axes. New fields default to their deterministic value in `deterministic()`; the struct stays `Copy` (all fields are `Copy`).

```rust
/// Deterministic-capture configuration. Extends the landed flake triad
/// (fixed_clock / wait_for_fonts / warm_atlas) with the font, DPR, sampling,
/// and asset-flush axes that the determinism spec adds.
#[derive(Clone, Copy, Debug)]
pub struct GoldenConfig {
    // --- landed triad (unchanged) ---
    pub fixed_clock: bool,
    pub wait_for_fonts: bool,
    pub warm_atlas: bool,
    pub accept: bool,
    // --- determinism additions ---
    /// Collapse the font axis. `Real` rasterizes the fixture's actual fonts
    /// (the narrow fidelity suite); `Ahem` substitutes the em-box font so any
    /// text-bearing golden is byte-identical across hosts (§ Ahem mode).
    pub font_mode: FontMode,
    /// Device-pixel-ratio pin. A 1× vs 2× render is a *different rasterization*,
    /// not a tolerance — captured as a fixture axis, never fuzzed (§ DPR pin).
    pub dpr: Dpr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontMode { Real, Ahem }

/// **Canonical `Dpr` definition site.** Device-pixel-ratio as *integer
/// milliscale* (1000 = 1.0×, 2000 = 2.0×) so the type is `Eq + Hash + Ord`
/// without float pitfalls — it is a *fixture axis* that must key a golden /
/// coverage cell, never a tolerance. Defined ONCE here in
/// `buiy_core::render::golden`; `goldens.md` (`GoldenKey.dpr`) and `coverage.md`
/// (`Matrix.dprs` / `Cell.dpr` / `CoverageKey.dpr`) import this type, they do
/// **not** redefine it. The capture boundary converts the window's `f32`
/// `scale_factor` via `Dpr::from_f32` and back via `Dpr::as_f32` when sizing the
/// offscreen target.
///
/// Derives `serde::{Serialize, Deserialize}` so `goldens.md`'s `GoldenKey` /
/// `BlessLedger` can persist it in the bless ledger without re-wrapping. The
/// `serde` derive is feature-gated in `buiy_core` only if needed; `buiy_core`
/// already carries `serde` as a workspace dep.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord,
         serde::Serialize, serde::Deserialize)]
pub struct Dpr(pub u32);
impl Dpr {
    pub const X1: Self = Dpr(1000);
    pub const X2: Self = Dpr(2000);
    /// Round an `f32` scale factor to integer milliscale (e.g. `1.0 → Dpr(1000)`).
    pub fn from_f32(scale: f32) -> Self { Dpr((scale * 1000.0).round() as u32) }
    /// Back to the `f32` scale factor the window/extract path uses.
    pub fn as_f32(&self) -> f32 { self.0 as f32 / 1000.0 }
}

impl GoldenConfig {
    pub fn deterministic() -> Self {
        Self {
            fixed_clock: true, wait_for_fonts: true, warm_atlas: true, accept: false,
            font_mode: FontMode::Ahem,   // layout goldens collapse the font axis by default
            dpr: Dpr::X1,
        }
    }
    /// The real-glyph fidelity variant: Ahem off, everything else pinned.
    pub fn fidelity() -> Self { Self { font_mode: FontMode::Real, ..Self::deterministic() } }
}
```

**MSAA / dither are pinned as constants, not config.** They are *never* a per-fixture knob — a golden captured with MSAA on is non-comparable with one captured off. They live as module constants the capture path asserts:

```rust
/// Single-sampled: the 4× MSAA resolve antialiases edges nondeterministically
/// across drivers. Buiy's in-shader analytic AA is deterministic given identical
/// FP, so MSAA buys nothing here and costs determinism. Mirrors the existing
/// `spawn_capture_camera`'s `Msaa::Off` (tests/support/mod.rs:229).
pub const CAPTURE_MSAA: bevy::render::view::Msaa = bevy::render::view::Msaa::Off;
/// Deband dither perturbs the low bits of the tonemapped output. Capture cameras
/// pin it off; assert no `DebandDither::Enabled` on the capture camera.
pub const CAPTURE_DITHER_OFF: bool = true;
```

## DPR pin

`ExtractedNodes.scale_factor` already carries the ratio (filled from the primary window, `extract.rs:606`; default `1.0`, `extract.rs:156`), and `gpu_render_app_scaled(logical_w, logical_h, scale_factor)` already builds an app at an explicit `with_scale_factor_override` (`tests/support/mod.rs:156`–`:161`). The pin is therefore **plumbing that exists** — the determinism contribution is to make it an *asserted* capture invariant: `capture_to_image` sizes the offscreen target to `logical × dpr` physical pixels (the existing scaled-builder contract) and asserts `ExtractedNodes.scale_factor == cfg.dpr.as_f32()` before readback (the `f32`→`Dpr` conversion lives at this capture boundary). DPR is a *fixture axis* fed by `coverage::Matrix.dprs`, never a tolerance widening.

## Ahem font mode (collapse the font axis)

The bulk of text-bearing goldens test *boxes*, not glyphs; real glyph rasterization is the canonical per-platform flake source (Flutter's entire `matchesGoldenFile` Ahem trick, `prior-art/flutter-golden-testing/obscure-text-font.md`). `FontMode::Ahem` substitutes a bundled Ahem face (every glyph a solid em-square box) so any non-fidelity golden is byte-identical across hosts; the narrow fidelity suite runs `FontMode::Real`.

- **Asset:** a committed `Ahem.ttf` (MIT, the WPT/Web-Platform Ahem) under `crates/buiy_core/tests/fixtures/fonts/`, alongside the existing per-script subsets (`tests/fixtures/fonts/`). License file beside it, mirroring the `OFL-*.txt` precedent.
- **Wiring:** `DeterministicApp` registers it through the production bytes path — `FontRegistry::register_bytes("Ahem", ahem_bytes, FontFaceDescriptors::default())` (`registry.rs:165`) — under family name `"Ahem"`, and when `font_mode == Ahem` makes it the **sole resolvable family** for fixture text so fallback cannot reintroduce a platform font. Concretely: the deterministic app disables system-font loading (fixtures already run bundled-only; `fixture_font_bytes`/`register_fixture_font`, `tests/support/mod.rs:292`/`:306`) and the fixture's BSN sets `font-family: Ahem`. This is a *capture-time* substitution; the shaping `.snap` fixtures and the real-glyph fidelity suite are unaffected (they pin `FontMode::Real`).
- **Boundary (Open Q #7 in the report):** which goldens are Real vs Ahem is a per-fixture declaration on the fixture, not global. Default Ahem; opt into Real only for the fidelity suite (glyph hinting/subpixel, color-emoji, decorations).

## Async-asset flush to quiescence

`wait_for_fonts` covers fonts; the general invariant is **zero pending assets before readback** (a half-streamed image or shader flips the diff). `capture_to_image` drives `app.update()` until quiescence, asserting all four conditions, then captures:

```rust
/// All must hold before the readback frame, in `capture_to_image`:
///   1. asset_server pending loads == 0   (no in-flight Image/Shader/Font load)
///   2. AtlasWarmupQueue::is_empty()       (warm_atlas; golden.rs:87)
///   3. fonts_ready(atlas, warmup, &keys)  (wait_for_fonts; golden.rs:82)
///   4. PipelineCache has no Queued/Compiling Buiy pipeline (shaders ready)
/// Bounded by MAX_SETTLE_FRAMES; panic with which condition never held.
```

This generalizes the existing `wait_for_text_ready` poll (`tests/support/mod.rs:266`, conditions 2+3) by adding the asset-server (1) and pipeline-cache (4) gates. The fixed clock means the loop terminates deterministically: time is advanced by `Time::<Virtual>::advance_by` (the landed manual-clock mechanism, `tests/text_caret_selection.rs:178`), never `Instant::now()`, so `fixed_clock` is "drive `Time<Virtual>` at explicit virtual timestamps."

## `DeterministicApp` builder (`buiy_verify::determinism`)

The single public seam every GPU tier (reftest, golden) constructs its app through. It owns the *setup* (knob application + the asserted checklist); `capture_to_image` in `buiy_core` owns the *capture*.

```rust
pub struct DeterministicApp { cfg: GoldenConfig, logical: (u32, u32) }

impl DeterministicApp {
    /// Default-deterministic at a logical viewport size.
    pub fn new(logical_w: u32, logical_h: u32) -> Self;
    pub fn with(mut self, cfg: GoldenConfig) -> Self;     // override the config
    pub fn font_mode(self, m: FontMode) -> Self;
    pub fn dpr(self, dpr: Dpr) -> Self;

    /// Build a painting-capable headless App with every knob applied:
    ///   - `gpu_render_app_scaled(w, h, cfg.dpr.as_f32())`    (DPR pin)
    ///   - `TimeUpdateStrategy::ManualDuration(0)` + Time<Virtual> driven manually
    ///   - registers Ahem and makes it sole family when font_mode == Ahem
    ///   - capture camera spawned at CAPTURE_MSAA, dither off
    /// Returns an App ready for fixture spawn; NOT yet finished (caller finishes).
    pub fn build(self) -> bevy::app::App;

    /// `build` + spawn the fixture + `capture_to_image(&app, &cfg)`. The one-call
    /// path tiers use. Internally asserts the four quiescence conditions.
    pub fn capture(self, fixture: impl FnOnce(&mut App)) -> image::RgbaImage;
}
```

`build` is a thin, **single-bodied** wrapper over the landed `gpu_render_app_scaled` so it cannot drift from the canonical plugin stack (the same anti-drift discipline `gpu_render_app_with_resolution` already enforces, `tests/support/mod.rs:168`).

## CI software-rasterizer pin (lavapipe) vs. the local real-GPU lane

**The argument:** Buiy owns its renderer, so it pins **one** canonical software rasterizer and stores **one golden per cell** — no per-OS/per-GPU matrix. A rolling distro rasterizer is a moving reference image (wgpu abandoned `ppa:oibaf` for exactly this; `prior-art/wgpu-testing/determinism-rasterizer.md`).

- **Rasterizer:** Mesa **lavapipe** (`libvulkan_lvp.so`), consumed as a **version-pinned, self-built artifact** (reuse `gfx-rs/ci-build`'s prebuilt tarball directly — no need to build our own Mesa). Pin `MESA_VERSION` + `ci-binary-build` tag explicitly; bump deliberately in a tracked issue, regenerating affected goldens in the same PR.
- **Adapter selection (env contract):** a composite action writes its **own** ICD JSON (the upstream ICD path is build-host-absolute) and exports:
  - `VK_DRIVER_FILES=$PWD/icd.json` — loader sees *only* lavapipe; cannot pick a hardware GPU.
  - `WGPU_ADAPTER_NAME=llvmpipe` — case-insensitive substring nails the exact device (`initialize_adapter_from_env`).
- **NOT set:** `LP_NUM_THREADS` (see Contract deviation 1).
- **Local real-GPU lane (this host, AMD RX 6700 XT / RADV):** the `#[ignore]` GPU tests run on real hardware locally and in the separate GPU-verify campaign. **Division of labor (cemented):** CI goldens run on pinned lavapipe (the stored-baseline gate); real-hardware shader/AA/blend paths are covered by the GPU-verify campaign, *not* a CI gate. The local lane does **not** compare against the stored lavapipe baseline (cross-rasterizer pixels are non-comparable) — it runs the determinism / reftest checks, which are rasterizer-internal-invariant, not baseline.

**One canonical rasterizer ⇒ one golden per cell.** The Tier-5 key schema `(widget, state, theme, viewport, backend, dpr)` (golden tier) carries `backend` for forward-compat, but with a single pinned Vulkan/lavapipe rasterizer the `backend` axis is a constant today — collapsing the worst combinatorial multiplier. Cross-backend goldens are out of scope (the pinned-rasterizer guarantee holds within one backend only).

## Reftests need this LESS than goldens

A reftest renders **both** halves in one process (one device, driver, clock, atlas, font stack). Every platform-variance term is *shared* and therefore cancels in the diff — so reftests tolerate a residual the determinism stack has not yet eliminated, and their default fuzz budget can stay `(0,0)` even before the lavapipe pin lands. They still **reuse** `DeterministicApp` (same fixed clock, same Ahem option, same quiescence flush) for *intra-run* stability (e.g. atlas warmup must complete before *either* half captures). The CI rasterizer pin is a hard prerequisite for **stored goldens** (the baseline must be bit-reproducible across runs and machines) and only a *nice-to-have* for reftests. This is why the report builds reftests first and the lavapipe pin in the golden step.

## Dependencies

- **No new Rust crate** is required by this tier. `image = "0.25"` (workspace) supplies `RgbaImage`; `bevy = "0.18"` supplies `Time<Virtual>`, `TimeUpdateStrategy`, `Msaa`. The Ahem `.ttf` is a committed test fixture, not a dependency.
- **`insta`** and the **perceptual-metric crate** are added by the snapshot/metric tiers, not here. If a plan author adds either, run `cargo deny check` (config at repo-root `deny.toml`) before committing — the project gates new deps on it.
- **CI action** `gfx-rs/ci-build` is consumed as a *release artifact*, not a crate dep; it carries no `cargo deny` surface. The pinned Mesa version is recorded in the action YAML.

## Verification

How the determinism harness verifies *itself* (these are tests of the test infra, runnable in CI):

1. **Idempotent-capture (pure-CPU + GPU lanes).** `capture_to_image` of the same fixture twice in two fresh `DeterministicApp`s ⇒ `metric::compare(a, b, default)` passes at budget `(0, 0)`. This is the landed "re-capture IS the golden" check (`render_golden_harness.rs`) re-expressed against the unified metric and the new builder — the direct proof the knobs actually pin the output. GPU (`#[ignore]`).
2. **Knob-sensitivity (negative tests).** Flipping each knob *changes* the bytes: `dpr(X1)` vs `dpr(X2)` of the same fixture differ; `FontMode::Real` vs `FontMode::Ahem` differ for a text fixture. Proves those knobs are load-bearing, not no-ops. GPU (`#[ignore]`). **MSAA is the exception — verified inert.** A 4× MSAA capture of the same fixture is byte-identical to `CAPTURE_MSAA` (`Off`) for Buiy's pipeline, because the SDF AA is analytic in-shader and the quads are axis-aligned + pixel-covering, so a hardware resolve is identity. This *confirms* the MSAA-pin rationale (the pin costs nothing while removing the cross-driver resolve risk); the test therefore asserts the verified equality, not a difference. See § Landed.
3. **Quiescence assertions fire.** Inject a never-loading asset / an undrained warmup queue and assert `capture` panics naming the unmet condition (1–4 above) — proves the flush gate cannot be silently skipped (the wgpu "implicit golden bootstrapping" Avoid: fail loudly, never green on a missing precondition).
4. **Clock determinism.** Assert `capture` uses `Time<Virtual>` and never reads wall time: a fixture whose visual depends on time captures identically across two runs at the same virtual timestamp; a test grep/lint forbids `Instant::now()` in the capture path.
5. **CI-pin smoke (CI-only).** On the lavapipe leg, assert the selected adapter name contains `llvmpipe` (env wiring took effect) before any golden runs — a one-line guard that the rasterizer pin is active, not silently falling back to a hardware adapter.

## Landed (determinism stack, plan Phase 3 tasks 3.1–3.5, 3.10)

The determinism substrate is implemented and verified; the Tier-5 stored-golden
corpus (plan 3.6–3.9) remains future work. Status stays `draft` until the
Phase 4.7 docs flip closes the whole campaign.

- **`GoldenConfig` extension + `FontMode`** — `crates/buiy_core/src/render/golden.rs`: `FontMode { Real, Ahem }`, the `font_mode`/`dpr` fields, `deterministic()` (Ahem + `X1`), `fidelity()` (Real). Tests: `crates/buiy_core/tests/render_golden_config.rs`. (3.1)
- **Ahem box-font** — the canonical W3C/WPT public-domain `Ahem.ttf` (em-box font) committed at `crates/buiy_core/tests/fixtures/fonts/Ahem.ttf` (+ `LICENSE-Ahem.txt`). Registered through the production bytes path and made the **sole resolvable family** by `buiy_verify::determinism::{register_ahem, stage_ahem}`. The obscure-text rectangle fallback the spec allowed was **not** needed — the genuine em-box font was obtainable. Tests: `crates/buiy_verify/tests/determinism_ahem.rs` (headless). (3.2)
- **Quiescence flush + DPR-pin assertion** — `capture_to_image` drives `app.update()` to quiescence over the four conditions (pending assets via the new `PendingCaptureAssets` resource; atlas warmup drained; `fonts_ready`; no `Queued`/`Creating` pipeline), polling the device to `Wait`, then asserts `scale_factor == cfg.dpr`. Panics naming the unmet condition on budget exhaustion. Tests: `crates/buiy_core/tests/render_capture_quiescence.rs` (the `Instant::now` grep-lint headless; the never-loading-asset panic GPU `#[ignore]`). (3.3)
- **`DeterministicApp` builder** — `crates/buiy_verify/src/determinism.rs`: `new`/`with`/`font_mode`/`dpr`/`build`/`capture`, a single-bodied wrapper over `capture_app_scaled` that pins the DPR, the fixed virtual clock (`TimeUpdateStrategy::ManualDuration(0)`), and the Ahem sole-family. Re-points `support::reftest_app` (the one-line 1b seam swap; the five 1b reftest `#[ignore]` cases re-run green through it). Tests: `crates/buiy_verify/tests/determinism_build.rs` (headless). (3.4)
- **GPU determinism self-tests** — `crates/buiy_verify/tests/determinism_capture.rs` (`#[ignore]`): idempotent capture passes at `(0,0)` for a rect AND an Ahem-text fixture; Ahem text is font-availability-invariant; `dpr`/`font_mode` knob-sensitivity negatives; the MSAA-inert finding above. **All six pass on the AMD RX 6700 XT.** (3.5)
- **CI lavapipe pin** — `.github/actions/install-mesa/action.yml` (consumes `gfx-rs/ci-build`'s prebuilt tarball, writes its own ICD JSON, exports `VK_DRIVER_FILES` + `WGPU_ADAPTER_NAME=llvmpipe`, **never** `LP_NUM_THREADS`) + the `gpu` job in `.github/workflows/ci.yml` (the `llvmpipe`-adapter smoke guard before the `#[ignore]` GPU lane). A **config/doc deliverable**: lavapipe is not installed locally, so this is validated on the real GPU here; the lavapipe leg is the CI stored-baseline gate. (3.10)

## Sources

Code: `crates/buiy_core/src/render/golden.rs:18`–`:88` (GoldenConfig, deterministic(), fonts_ready); `crates/buiy_core/tests/support/mod.rs:156` (gpu_render_app_scaled), `:161` (with_scale_factor_override), `:229`/`:237` (Msaa::Off capture camera), `:266` (wait_for_text_ready quiescence poll), `:292`/`:306` (bundled-font registration), `:353` (readback_rgba); `crates/buiy_core/src/render/extract.rs:156`/`:606` (scale_factor default + fill); `crates/buiy_core/src/text/registry.rs:165` (register_bytes); `crates/buiy_core/tests/text_caret_selection.rs:178` (Time<Virtual>::advance_by). Prior-art: `docs/prior-art/wgpu-testing/{lessons.md,determinism-rasterizer.md}` (lavapipe pin, VK_DRIVER_FILES, the LP_NUM_THREADS myth); `docs/prior-art/flutter-golden-testing/obscure-text-font.md` (Ahem). Report: `docs/reports/2026-06-14-visual-bug-detection-strategy.md` § Cross-cutting mechanisms ("Deterministic-rendering stack for wgpu CI").
