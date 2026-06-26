# Buiy WebGPU/browser v1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Spec:** [`specs/2026-06-25-buiy-wasm-browser-support-design.md`](../specs/2026-06-25-buiy-wasm-browser-support-design.md) (v1 = WebGPU-only).
**Research substrate:** the throwaway prototype (branch `wasm-proto`, `PROTOTYPE-JOURNAL.md` + retrospective) validated this end-to-end; the KEEP commits port here.

**Goal:** Buiy compiles to `wasm32-unknown-unknown` and renders a widget into an HTML `<canvas>` on the **WebGPU** backend, with a headless-browser CI smoke lane that catches the shader-conformance class.

**Architecture:** Purely additive over native. One shared-code change — the WGSL uniformity fix (behavior-identical on native, asserted by a GPU test). Everything else is `cfg(target_arch="wasm32")`-gated or new wasm-only files. WebGL2 reach is a deferred milestone (not in this plan).

**Tech Stack:** Rust, Bevy 0.19 / wgpu 29 (WebGPU backend), trunk + wasm-bindgen, Playwright + headless Chrome for the smoke gate.

**Build/verify commands** (from `CLAUDE.md`): native gate `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked`; GPU lane `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`. cargo network is sandboxed — lock/deny steps need an unsandboxed shell.

---

### Task 1: WGSL uniformity fix (D2) + native-equivalence GPU test

The hard render prerequisite. Native naga is lenient; Chrome's Tint rejects `fwidth`/`textureSample` after an early-`return` clip-discard. Compute the derivative unconditionally + mask the clip into alpha. Behavior-identical on native (clipped → alpha 0 either way). Validated in prototype `8c017e1`.

**Files:**
- Modify: `crates/buiy_core/src/render/shader.wgsl` (Quad fragment, ~L82-90)
- Modify: `crates/buiy_core/src/render/coverage.wgsl` (Glyph fragment, ~L80-88)
- Modify: `crates/buiy_core/src/render/band.wgsl` (band fragment, ~L107-140)
- Test: `crates/buiy_core/tests/render/render_clip_uniformity.rs` (new, `#[ignore]` GPU lane)

- [ ] **Step 1: Write the failing GPU test** (native, `#[ignore]`) — render a quad with a clip AABB that excludes its right half, read back, assert the excluded half is fully transparent (alpha 0) AND the included half has ink. This pins the masked-clip behavior == the old early-return behavior. Build on `crates/buiy_core/tests/support/mod.rs` (`gpu_render_app`/`render_to_image`/`readback_rgba`).

```rust
// crates/buiy_core/tests/render/render_clip_uniformity.rs
//! D2: the masked-clip shaders (uniformity fix) must clip identically to the
//! old early-return — clipped fragments output alpha 0. #[ignore] GPU lane.
#[path = "../support/mod.rs"]
mod support;
use support::{gpu_render_app, readback_rgba, render_to_image};
// ... spawn a Quad-painting node with a ClipRect excluding x > center;
// render_to_image; readback_rgba; assert right-half alpha == 0, left-half alpha > 0.
```

- [ ] **Step 2: Run it to verify it fails** — `cargo test -p buiy_core -j 2 --test render_clip_uniformity -- --ignored --test-threads=1`. Expected: FAIL (test infra references not yet wired) — fix compile, then it should PASS on the *current* (early-return) shaders too, since clipping already works. (This test guards the *change*; it must pass before AND after.) Confirm it PASSES on current shaders first.

- [ ] **Step 3: Apply the shader fix** — in each of the three shaders, replace the early-return clip block with a `clipped` boolean + a final `* mask`:

```wgsl
// shader.wgsl (Quad): replace the `if (clip) { return 0; } ... fwidth` block with:
    let clipped = any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max);
    let d = sdf_rounded_rect(in.local_uv * in.half_size, in.half_size, in.radius);
    let aa = fwidth(d);
    let alpha = 1.0 - smoothstep(-aa, aa, d);
    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(in.color.rgb, in.color.a * alpha * mask);
```
```wgsl
// coverage.wgsl (Glyph):
    let clipped = any(in.frag_pos < in.clip_min) || any(in.frag_pos > in.clip_max);
    let coverage = textureSample(atlas, atlas_samp, in.atlas_uv).r;
    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(in.color.rgb, in.color.a * coverage * mask);
```
```wgsl
// band.wgsl: hoist `clipped` above the SDF/fwidth, and at the end:
    let mask = select(1.0, 0.0, clipped);
    return vec4<f32>(col.rgb, col.a * band * mask);
```
Each gets a comment: `// WebGPU/Tint requires derivative builtins in uniform control flow; native naga is lenient. Mask the clip instead of early-returning. Behavior-identical on native.`

- [ ] **Step 4: Re-run the GPU test** — same command. Expected: PASS (clipping unchanged).
- [ ] **Step 5: Run the full GPU lane** — `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` and `cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1`. Expected: PASS (no golden/reftest regression — masked output is pixel-identical).
- [ ] **Step 6: Commit** — `git commit -m "fix(render): WGSL uniformity — mask clip instead of early-return before fwidth/textureSample"`.

---

### Task 2: arboard compile-gate (D4)

Sole core compile blocker. Validated in prototype `5ad4803`.

**Files:**
- Modify: `crates/buiy_core/Cargo.toml` (move `arboard` to a `cfg(not(wasm32))` target table; ~L29)
- Modify: `crates/buiy_core/src/text/edit/clipboard.rs` (cfg-gate `ArboardClipboard` struct + 2 impls)
- Modify: `crates/buiy_core/src/text/edit/mod.rs` (split the re-export)
- Modify: `crates/buiy_core/src/text/mod.rs` (split re-export ~L55; insert `MemClipboard` on wasm ~L301)

- [ ] **Step 1: Gate the dep** — move `arboard = { workspace = true }` out of `[dependencies]` into `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`. Leave `clipboard-image = ["arboard/image-data"]` (only resolves when enabled).
- [ ] **Step 2: cfg-gate `ArboardClipboard`** — prefix `#[cfg(not(target_arch = "wasm32"))]` on the struct (`#[derive(Default)] pub struct ArboardClipboard`), `impl ArboardClipboard`, and `impl ClipboardProvider for ArboardClipboard`.
- [ ] **Step 3: Split the re-exports** — in `text/edit/mod.rs` and `text/mod.rs`, move `ArboardClipboard` out of the shared `pub use {...}` group into a `#[cfg(not(target_arch = "wasm32"))] pub use ...ArboardClipboard;`.
- [ ] **Step 4: Default `Clipboard` per target** — in `text/mod.rs` wrap the existing insert with `#[cfg(not(target_arch = "wasm32"))]` and add a `#[cfg(target_arch = "wasm32")]` arm inserting `Clipboard(Box::new(MemClipboard::default()))`.
- [ ] **Step 5: Verify wasm compiles** — `cargo check -p buiy_core --target wasm32-unknown-unknown`. Expected: PASS (no arboard error). (Cold ~2 min.)
- [ ] **Step 6: Verify native unaffected** — `cargo check -p buiy_core` (and the clipboard-image feature: `cargo check -p buiy_core --features clipboard-image`). Expected: PASS.
- [ ] **Step 7: Commit** — `git commit -m "feat(wasm): cfg-gate arboard off wasm32, default to MemClipboard"`.

---

### Task 3: Lean WebGPU feature + web example crate (D5)

**Files:**
- Create: `examples/buiy_web/Cargo.toml`, `examples/buiy_web/src/main.rs`, `examples/buiy_web/index.html`
- Modify: root `Cargo.toml` (`[workspace] members` += `examples/buiy_web`; release size profile)

- [ ] **Step 1: Create the web example** — mirror the prototype (`web_proto`): `DefaultPlugins.set(WindowPlugin{ primary_window: Some(Window{ canvas: Some("#buiy".into()), fit_canvas_to_parent: true, prevent_default_event_handling: true, ..default() }), ..default() })` + `BuiyPlugin` + `Camera2d` + `Button::new("Save")` + `#[cfg(target_arch="wasm32")] console_error_panic_hook::set_once()`. `Cargo.toml`: `bevy = { workspace = true, features = ["webgpu"] }` (the bevy **meta** feature; investigate trimming in Step 3), `buiy = { path = ... }`, `[target.'cfg(target_arch="wasm32")'.dependencies] console_error_panic_hook = "0.1"`. `index.html`: a `<canvas id="buiy">` + `<link data-trunk rel="rust" data-wasm-opt="0"/>`.
- [ ] **Step 2: Add to workspace + build** — add `"examples/buiy_web"` to members; `trunk build examples/buiy_web/index.html` (needs lock from Task 4 + unsandboxed shell). Expected: builds; dist has js+wasm.
- [ ] **Step 3: Trim to the lean feature set** — measure the wasm size with the full `webgpu` meta-feature, then try a curated feature list that excludes the 3D crates (`bevy_pbr`/`bevy_mikktspace`). Acceptance: the example still renders the button (verified by Task 5's smoke) with the smaller artifact; if trimming breaks the WebGPU backend or the build, keep the meta-feature and record why in the spec's §2-deferred. Document the before/after size in the commit body.
- [ ] **Step 4: Commit** — `git commit -m "feat(wasm): buiy_web WebGPU example + lean feature set"`.

---

### Task 4: Cargo.lock + cargo deny / MSRV (D6)

The WebGPU feature adds backend crates (`bevy_anti_alias`, `bevy_dev_tools`, `bevy_post_process`, `bevy_feathers`, `bevy_pbr`, `bevy_mikktspace`, …) the `--locked` lock never had. **Needs an unsandboxed shell** (cargo index refresh).

**Files:** `Cargo.lock` (regenerated), `deny.toml` (add wasm32 target)

- [ ] **Step 1: Refresh the lock** — in an unsandboxed shell, run a resolve that adds the backend deps (`cargo update -p bevy_anti_alias --precise <ver>` won't work pre-lock; instead `rm -rf ~/.cargo/registry/index/*/.cache` then `cargo metadata >/dev/null` to add them at 0.19.0). Confirm `grep bevy_anti_alias Cargo.lock`. Keep the diff to the new crates only (cargo is minimal).
- [ ] **Step 2: Add wasm32 to deny.toml** — add `"wasm32-unknown-unknown"` to `[graph] targets` (~L26) so the web graph is audited.
- [ ] **Step 3: cargo deny** — `cargo deny check`. Expected: PASS (or surface a real license/advisory on a new crate to resolve).
- [ ] **Step 4: MSRV** — confirm the new crates build on MSRV 1.95 (`cargo +1.95 check -p buiy_web --target wasm32-unknown-unknown`, or rely on the CI MSRV job).
- [ ] **Step 5: Commit** — `git commit -m "build(wasm): lock the WebGPU backend deps + audit wasm32 graph"` (lock + deny in one deliberate commit).

---

### Task 5: Headless-browser WebGPU CI smoke lane

The only gate that catches the D2 class (real Tint). Productionizes the prototype's Playwright runner.

**Files:**
- Create: `tools/web-smoke/run.mjs` (Playwright runner), `tools/web-smoke/package.json`
- Create: `crates/buiy_core/tests/.../` — N/A (this is a JS+CI harness, not a Rust test)
- Modify: `.github/workflows/ci.yml` (new `web-smoke` job)

- [ ] **Step 1: Write the runner** — `tools/web-smoke/run.mjs`: launch headless Chrome (`--enable-unsafe-swiftshader --ignore-gpu-blocklist`, software WebGPU for GPU-less CI runners), hook `createShaderModule.getCompilationInfo()` to collect Tint errors, navigate to the served example, wait for load, assert **(a)** zero shader-compilation errors, **(b)** zero `create_render_pipeline` validation errors in console, **(c)** the `#buiy` canvas is non-blank (screenshot pixel variance > threshold). Exit non-zero on any failure. (Port from prototype `run-proto.mjs`/`run-proto2.mjs`.)
- [ ] **Step 2: Local dry-run** — `trunk build` the example, serve it, `node run.mjs`. Expected: PASS (0 errors, non-blank) — reproduces the prototype's validated result.
- [ ] **Step 3: Add the CI job** — a `web-smoke` job in `ci.yml`: install rust + wasm32 target + trunk + wasm-bindgen-cli (pinned to the lock's version) + `npx playwright install chromium`; `trunk build examples/buiy_web/index.html`; serve + `node tools/web-smoke/run.mjs`. Gate on it. (No GPU on hosted runners → swiftshader WebGPU.)
- [ ] **Step 4: Commit** — `git commit -m "test(wasm): headless-browser WebGPU smoke lane (catches the Tint shader class)"`.

---

### Task 6: Size profile + measure

**Files:** root `Cargo.toml` (`[profile.release]`/a `wasm-release` profile), the example build.

- [ ] **Step 1: Add a size profile** — `wasm-opt -Oz` (via trunk's `data-wasm-opt`), `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"` for the release wasm build. (Keep the workspace `[profile.dev] debug=0` audit constraint intact.)
- [ ] **Step 2: Measure** — `trunk build --release` the example; record raw + brotli wasm size.
- [ ] **Step 3: Document** — note the shippable size in the spec §6 (Risks/size) and the commit body. (Loading-screen/streaming deferred.)
- [ ] **Step 4: Commit** — `git commit -m "build(wasm): release size profile + measured size"`.

---

## Final gate (before PR)
- [ ] Native gate green: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked`.
- [ ] GPU lane green: `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` + `cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1`.
- [ ] wasm builds: `cargo check -p buiy_core --target wasm32-unknown-unknown` + `trunk build examples/buiy_web/index.html`.
- [ ] Web smoke green locally.
- [ ] `cargo deny check` green.
- [ ] Open PR; **stop at merge gate** (CI green → human go).

## Spec coverage check
D1 (WebGPU-only) = scope of this plan. D2=Task 1. D3 (inherit bootstrap) = Task 3 (no Buiy bootstrap code). D4=Task 2. D5=Tasks 3+6. D6=Task 4. D7 (single-threaded)=no change. D8 (a11y staged), D9 (IME/clipboard deferred), D10 (fonts)=no v1 code (disclosed). Verification §4 = Tasks 1+5. Deferred (§2-deferred WebGL2) = not in this plan.
