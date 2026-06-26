# WASM / Browser-Rendering Support — Feasibility Research

**Date:** 2026-06-25
**Method:** 23-agent research workflow (6 codebase-subsystem analyzers + 6 cited web-research briefs → synthesis → adversarial per-claim verification + completeness critic), then **direct re-anchoring of every load-bearing fact against the working tree** by the coordinator.
**Tree:** `origin/main` @ `fdb8dda` (#80). Versions: **bevy 0.19.0, wgpu 29.0.3, naga 29, winit 0.30.13, cosmic-text 0.19, swash 0.2.9, fontdb 0.23, accesskit 0.24.1, accesskit_winit 0.32.2, arboard 3.6.1**. wasm-layer deps already resolved in `Cargo.lock`: wasm-bindgen 0.2.125, web-sys 0.3.102, web-time 1.1.0, getrandom 0.3.4 + 0.4.3.
**Status:** `[active]` — research input. No wasm work has been attempted; this scopes it.

> **Provenance & a methodology caveat worth recording.** The multi-agent *verification* phase produced two confidently-stated but **false** refutations (it claimed the 17-attribute band pipeline and `scroll.rs` wheel handling don't exist, and "corrected" several file:line citations). The cause: those agents searched a **stale tree** (the pre-#80 checkout, where the F-tier band work and wheel wiring hadn't landed), not the `fdb8dda` worktree. Every load-bearing claim below was therefore **re-verified directly against `fdb8dda`** (`grep`/`sed`/`cargo tree` runs cited inline). Where the synthesis and the verifier disagreed, the tiebreaker is the coordinator's own check, noted as such. Lesson for future workflows: pin agents to an absolute worktree path and have them echo a tree-fingerprint (a file length or `git rev-parse`) before trusting their citations.

---

## 1. Verdict (lead with this)

**Feasible at moderate effort, with no architectural rewrite.** Buiy is unusually well-positioned for the browser because its custom render pipeline is, by construction, inside the WebGL2 downlevel subset: **zero compute shaders, zero storage buffers, no multiple-render-targets, no depth/stencil**, fragment-discard clipping (not scissor/stencil), small std140 UBOs + instanced vertex buffers, and pipelines that already specialize on the *live* view format + sample count rather than any hardcoded surface format. The wasm-layer dependencies (wasm-bindgen, web-sys, web-time) are already in the lock via the bevy/wgpu/winit graph, the scheduler is already single-threaded, the default font is embedded, and the clipboard sits behind a swappable facade.

The work is overwhelmingly **build-wiring plus a few bounded edits behind seams that already exist**, not renderer surgery. There are exactly **two genuine hard requirements** before anything renders broadly:

1. **`arboard` is an unconditional dependency with no wasm backend** — a hard *compile* blocker today (the crate fails before any render code is reached).
2. **The border/outline "band" pipeline declares 17 vertex attributes** (`@location(0..=16)`), exceeding WebGL2's 16-attribute cap and the WebGPU spec baseline — a hard *render* blocker for the WebGL2 build (and a portability risk on conformant WebGPU adapters).

Everything else is correctness/reach/polish: WebGPU-first gets a widget on screen fast; a WebGL2 fallback build buys broad reach; production quality (web clipboard, in-browser a11y, IME, binary size) is incremental.

**Recommended first milestone:** a **WebGPU-only** build of `hello_button` painting + clicking in a Chrome/Edge canvas. The renderer needs *zero* changes for that (the 17-attribute band works on Dawn, which reports ~30 attributes) — it proves the custom render-graph pipeline paints to a browser canvas and de-risks everything after.

---

## 2. Why Buiy is unusually wasm-ready (the load-bearing architecture)

All verified against `fdb8dda`:

- **No compute / storage / MRT / depth-stencil.** `grep -niE "@compute|var<storage|ComputePipeline|BufferBindingType::Storage|StorageTextureAccess|dispatch_workgroups"` over `crates/buiy_core/src/render` → empty. This is the single fact that keeps a WebGL2 backend on the table at all (wgpu's `downlevel_webgl2_defaults` sets `max_storage_buffers_per_shader_stage = 0` and WebGL2 has no compute). Every render pipeline has one color target, `depth_stencil: None`.
- **Format/sample-count-specialized pipelines.** `BuiyPrimitiveKey` / band key / composite key carry `{format, samples}` read from the live `ViewTarget` (`crates/buiy_core/src/render/primitive.rs` — `pub format: TextureFormat`). A web canvas's negotiated surface format drops in with no render change.
- **Buiy never creates an adapter/device/surface** and sets no `WgpuSettings`/`Backends` override (grep across crates+examples = 0 hits). Bevy owns the async wasm device/canvas-surface init; Buiy only *reads* `Res<RenderDevice>` (in `finish()` and in its per-frame prepare systems) — never a blocking adapter request. So Buiy introduces no synchronous-adapter blocker.
- **Fragment-discard clipping**, not scissor/stencil (`render/clip.rs`; `set_scissor_rect` never called), so there is no backend-specific clip state and it sidesteps WebGL2's "can't sample a multisampled texture" trap for the single-sampled group/composite targets.
- **Default font is embedded.** `crates/buiy_core/src/text/font_system.rs:22` — `include_bytes!("../../assets/fonts/FiraSans-Regular-latin.ttf")` under the default-on `default_font` feature, pinned to all generic families. Text renders on wasm with no filesystem/fetch. System-font scanning is opt-in and off by default (`text/system_scan.rs` — `BuiyTextPlugin { system_fonts: true }`), and `load_system_fonts()` is a no-op on wasm anyway.
- **Clipboard behind a facade.** `ClipboardProvider` trait + `Clipboard(Box<dyn …>)` resource, with a pure-Rust `MemClipboard` that compiles everywhere (`text/edit/clipboard.rs`); arboard is injected at exactly one site (`text/mod.rs:301`).
- **a11y separates data from sink.** The semantic-tree builder (`a11y/translate.rs`) is winit-free pure data; only `a11y/adapter.rs` pushes into the platform sink. It compiles unchanged on wasm via accesskit_winit's null adapter.
- **Single-threaded scheduler** (bevy `default-features=false`, `multi_threaded` not enabled) matches Bevy's single-threaded-on-web constraint with no change — avoiding the atomics + COOP/COEP cross-origin-isolation rabbit hole entirely for the MVP.
- **Wheel scrolling is wired** through `bevy_picking`: `scroll.rs` `on_scroll` consumes `On<Pointer<Scroll>>` (registered observer, line 381) and writes `ScrollOffset`. Works on web through the standard pipeline (with the canvas `prevent_default` caveat in §6).

---

## 3. How Bevy renders in the browser — the substrate Buiy inherits

Buiy builds on Bevy's `DefaultPlugins`, so it inherits Bevy's **entire** wasm bootstrap and writes none of it. The browser-rendering question is therefore mostly "what does Bevy already do, and where does Buiy's own pipeline have to fit?" Verified against the Bevy **0.19.0** source on disk (the version the lock pins):

1. **Non-blocking event loop.** `bevy_winit/src/state.rs:895-903` — on `target_arch = "wasm32"` the runner calls winit's `event_loop.spawn_app(runner_state)` (the `EventLoopExtWebSys` web extension) instead of the native `event_loop.run_app(...)`. `spawn_app` hands control back to the browser rather than blocking the main thread, which is why `App::run()` is last and still returns on web. Buiy's app shape (`App::new().add_plugins(DefaultPlugins).add_plugins(BuiyPlugin)…run()`) is already correct — nothing to change.
2. **Canvas binding by CSS selector.** `bevy_winit/src/winit_windows.rs:282-299` — on wasm, Bevy reads the `Window` component's `canvas: Option<String>` selector, queries `document` for the matching `HtmlCanvasElement`, and attaches the winit window via `WindowAttributesExtWebSys::with_canvas` + `with_prevent_default`; `fit_canvas_to_parent` is applied in `system.rs:103`. You point Bevy at an existing `<canvas id="…">` in `index.html`; you never construct a window (Buiy already constructs none).
3. **Surface from a raw handle — same path as native.** `bevy_render/src/view/window/mod.rs:362-386` — `create_surfaces` builds `SurfaceTargetUnsafe::RawHandle` from the window's `RawHandleWrapper` and calls `instance.create_surface_unsafe(...)`. On web the handle is `RawWindowHandle::Web` (the canvas), but it flows through the **identical** code path; the canvas is just another raw handle.
4. **Async adapter/device init.** `bevy_render/src/settings.rs:259-301` + `lib.rs:357/452/501` — requesting a GPU adapter+device is async on the web. `RenderPlugin::build` kicks off `create_render`, which builds an `async_renderer` future (`initialize_renderer(...).await`) and, on wasm, `IoTaskPool::get().spawn_local(async_renderer).detach()`s it (fire-and-forget on the browser main thread, since you cannot block it) versus `bevy_tasks::block_on(...)` on native. The resolved resources land in a `FutureRenderResources` cell that `RenderPlugin::finish()` later `take()`s. **This is exactly why Buiy adds no synchronous-adapter blocker:** Buiy reads `RenderDevice` only in its own `finish()` and prepare systems, and `finish()` is where Bevy guarantees the async-initialized device exists.
5. **Backend + limits are one compile-time switch.** `bevy_render/src/settings.rs:71-97` — `WgpuSettings::default()` resolves `Backends::GL` when the bevy **`webgl`** feature is set (and `webgpu` is not), `Backends::BROWSER_WEBGPU` when **`webgpu`** is set, else `Backends::all()` (native auto-select); `WGPU_BACKEND` env-overrides it. The `webgl`-on-wasm path *also* forces `wgpu::Limits::downlevel_webgl2_defaults()` — the constrained set (storage-buffers-per-stage = 0, the 16-attribute caps, smaller UBO bindings) that **B2** and **W1** below must satisfy. (Note the cargo feature is named `webgl`, mapping to `wgpu/webgl`; it applies WebGL2 limits.)
6. **Bevy's own WebGL2 litmus test is "no storage buffers".** `bevy_render/src/lib.rs` `storage_buffers_are_unsupported()` returns `max_storage_buffers_per_shader_stage == 0`. Bevy uses zero-storage-buffers as the signal that it is on WebGL2; Buiy's pipeline is zero-storage-buffer by construction (§2), so it clears the very bar Bevy itself checks.

**Net:** the browser bootstrap — event loop, canvas attach, surface creation, async device, backend selection — is **100% Bevy's**, gated on one cargo feature (`webgl` or `webgpu`). Bevy ships and CI-builds browser demos of this exact path (`examples/` → `wasm-bindgen` + trunk / `wasm-server-runner`), so the substrate is proven in the field. Buiy's wasm work is consequently *not* "make it render in a browser" (Bevy does that) but the narrow pair: make Buiy's crate **compile** for wasm (B1: arboard) and keep Buiy's **own** pipeline inside the chosen backend's limits (B2: band attributes, W1: float targets).

---

## 4. Hard blockers (must fix before broad rendering)

### B1 — `arboard` has no wasm backend (hard COMPILE blocker) · Effort S
`arboard = { workspace = true }` is an ungated `[dependencies]` entry of `buiy_core` (`crates/buiy_core/Cargo.toml:29`; workspace pin at root `Cargo.toml:76`), and `ArboardClipboard::new()` is inserted unconditionally as the default `Clipboard` (`crates/buiy_core/src/text/mod.rs:301-302`). arboard 3.6.1 compiles a platform module only for linux/windows/macos; on `wasm32-unknown-unknown` its `platform::{Clipboard,Get,Set,Clear}` are undefined → `E0433`/`E0425` *inside arboard*, before `bevy_render` is even reached (an isolated `cargo check --target wasm32` reproduced this during research). The wasm/web support (1Password/arboard PR #160) is unmerged as of mid-2026.

**Fix:** move the arboard edge under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, cfg-gate `ArboardClipboard` + its re-exports, and default the `Clipboard` resource to the existing `MemClipboard` on wasm. The `ClipboardProvider` facade makes this a one-construction-site change. Keep the `clipboard-image` feature (`arboard/image-data`) non-wasm.

### B2 — Band pipeline declares 17 vertex attributes (hard RENDER blocker for WebGL2) · Effort M
The border/outline/focus-ring "band" pipeline rides its own `band.wgsl` shader and `BorderBandInstance` record (17 `#[repr(C)]` fields: `rect_pos/size`, 4 per-side colors, `width`, `outer_radius[8]`, `inner_radius[8]`, `clip_min/max`, `affine`). Its vertex layout (`crates/buiy_core/src/render/primitive.rs` `band_vertex_buffers`, ~lines 283-420) binds **`shader_location 0..=16`** — VBO0 carries 0–1, VBO1 carries 2–16 — and `band.wgsl` declares `@location(0)`…`@location(16)`. That is **17 attributes, max location 16**, so it needs `max_vertex_attributes ≥ 17`.

- **WebGL2:** `GL_MAX_VERTEX_ATTRIBS` minimum is **16** and is commonly *exactly* 16 (ANGLE/D3D11). The band pipeline **fails to create** → no per-side borders, outlines, or focus rings. It validates on native only because dev adapters (RADV/llvmpipe) report 32.
- **WebGPU:** the spec baseline `maxVertexAttributes` is **16**. It works on desktop Chrome/Firefox (Dawn reports ~30) **today**, but a conformant adapter reporting 16 would silently fail to create the pipeline.

**Fix:** pack the band record to ≤16 attributes — fold the two clip `vec2`s into one `vec4`, the affine into one `vec4`, or move the 8-float per-corner radii into an instance-indexed UBO. Coordinated byte-offset edit across `primitive.rs` `band_vertex_buffers`, `instance.rs` `BorderBandInstance` + the `pack_border`/`pack_outline` packers, and `band.wgsl` `@location`s, guarded by the existing `BORDER_BAND_INSTANCE_STRIDE_BYTES` assert. **Hard requirement for any WebGL2 build; strongly advised for WebGPU spec-safety.** (Interim defensive option: request `max_vertex_attributes ≥ 17` at device creation, or feature-detect and disable the band pipeline.)

> This is the standout finding. The synthesis caught it; the automated verification wrongly "refuted" it off a stale tree; direct inspection of `band.wgsl` (`@location(0..16)`) and `primitive.rs` confirms it is real.

---

## 5. WebGPU vs WebGL2 strategy

**Target WebGPU first; add a WebGL2 fallback build second for reach.** Because the renderer uses no compute/storage/MRT, WebGL2 is *architecturally* viable — so this is a reach/effort tradeoff, not a capability gate.

- **WebGPU** makes both WebGL2-specific concerns vanish: `Rgba16Float` is core-renderable/blendable/filterable (no extension dance), and the 17-attribute band works on current Chrome/Edge/Firefox-Dawn. Cost: reach — Firefox is default-off on most desktop platforms in mid-2026, and older Safari/iOS and pre-121 Android lack it.
- **WebGL2** is the broad-reach backend but needs **both** B2 (band ≤16 attrs) and W1 (float-target fallback) resolved.
- The two backends are **mutually exclusive in one wasm binary** (bevy's `webgpu` feature overrides `webgl`; upstream single-binary dual-backend issue still open). Broad reach = **two artifacts + a JS `navigator.gpu` feature-detect** picking which to load.

### W1 — `Rgba16Float` effect-compositor targets on WebGL2 · Effort M (WebGL2 only)
The effect-group compositor renders to, alpha-blends on, and samples **`Rgba16Float`** off-screen targets (`crates/buiy_core/src/render/compositor.rs:437`, `RENDER_ATTACHMENT|TEXTURE_BINDING`). Only the **EffectGroup** path (opacity < 1, isolation, shadow/outline groups) hits this; flat widgets render straight into the SDR window view and never touch a float buffer.

- **WebGPU:** core; nothing to do.
- **WebGL2:** needs `EXT_color_buffer_float` to make `rgba16f` color-renderable **and** blendable. (Per wgpu-hal 29's GLES adapter, `Rgba16Float` is marked filterable unconditionally and renderable+blendable under one `COLOR_BUFFER_HALF_FLOAT` cap, so `EXT_float_blend` and `OES_texture_float_linear` — the 32-bit variants — are likely *not* required. **Verify** whether the source layout's `filterable: true` flag forces a linear-filter extension even though the composite sampler is `Nearest`.) Widely available on desktop, spottier on mobile.

**Fix (WebGL2 build):** gate on `EXT_color_buffer_float`, or substitute `Rgba8Unorm` group targets when absent (costs linear-space precision → banding on translucent overlapping content, but keeps the feature working). Single format source: `compositor.rs` `group_target_descriptor`.

---

## 6. Correctness, interaction & reach gaps on web (not blockers, but real)

These don't stop a build; they're the difference between "paints" and "usable/production". Most are confirmed against `fdb8dda`.

- **sRGB / gamma (highest-risk correctness item).** Buiy pre-linearizes colors on the CPU and relies on the target doing sRGB-encode-on-write. Web surface formats are constrained (WebGPU canvas does sRGB via `viewFormats`, not a directly-configurable `*-srgb` canvas; a WebGL2 surface may be plain `rgba8unorm`). If Bevy hands Buiy a non-sRGB-encoding view, **everything renders too dark/bright**. Needs an empirical in-browser check and possibly a final-pass shader encode.
- **MSAA on the main pass.** A bare `Camera2d` defaults to `Msaa::Sample4`, so the **window pass is 4× multisampled** (`render/mod.rs:296`, `node.rs:87`, `pipeline.rs:335`; `examples/capture` uses `Sample4`, golden capture uses `Msaa::Off`). The intermediate group/composite targets are single-sampled, but the main pass is not. WebGL2 supports MSAA via multisampled renderbuffers + resolve (cost), but this exact path is untested here. **Recommend evaluating `Msaa::Off` for web** and validating the 4× main pass on the WebGL2 backend.
- **High-DPR startup crash.** `logical_size × devicePixelRatio` can exceed `max_texture_dimension_2d` (often 4096) on high-DPR mobile and fail at launch. Clamp offscreen/compositor target sizes to the adapter limit and/or override scale factor.
- **macOS-in-browser modifiers.** Command-vs-Control selection uses `cfg!(target_os = "macos")` at `text/edit/keymap.rs:128` and `text/edit/input.rs:515,539`. On `wasm32` `target_os` is `"unknown"`, so these compile to `false` → a Mac user in a browser silently gets Ctrl-not-Cmd shortcuts. Needs **runtime** platform detection.
- **IME composition is inert on web.** winit-web emits no `Ime::Preedit`/`Ime::Commit` (winit issue #4424 open), so Buiy's E5 composition path is dead — **CJK / dead-key / accented input won't compose**; only direct Latin typing survives. Degrades safely (no crash). Real fix is a Buiy-owned hidden-input / `EditContext` shim outside winit.
- **Mobile soft-keyboard.** A WebGPU/WebGL2 canvas does **not** raise the on-screen keyboard without a focused DOM input/`EditContext` — so on phones, text editing may be impossible (no keyboard appears), not merely "CJK won't compose". Desktop-keyboard input is the realistic ceiling until the hidden-input shim exists.
- **Accessibility reaches no screen reader on web.** The a11y subsystem compiles and builds a correct `TreeUpdate` on wasm, then pushes it into accesskit_winit's **null adapter** — reaching **zero** browser AT, silently (no panic/warning). AccessKit has no shipped web platform adapter as of 2026. A web build can look "a11y-complete" while exposing nothing. **This must be disclosed in any milestone.** Real web a11y is an XL hidden-DOM/ARIA-overlay effort (driven by the existing `build_tree_update` output) or waits on an upstream accesskit web adapter.
- **Clipboard is in-app-only for v1.** The synchronous `ClipboardProvider` trait cannot satisfy the async, permission-/gesture-gated browser clipboard. `MemClipboard` gives honest in-app copy/paste; real cross-app paste needs a trait rework or a cached-read bridge over `navigator.clipboard`.
- **Touch / momentum scroll & canvas event capture.** Wheel scroll is wired (§2), but the canvas must `prevent_default` wheel/touch over itself or the **page** scrolls instead; touch-drag momentum scrolling is unhandled.
- **naga WGSL→GLSL-ES translation (WebGL2) is unvalidated end-to-end.** "WebGL2 viable" is established at the feature-inventory level only; it is **not** yet confirmed that naga 29 translates *these* shaders (`shader.wgsl`, `shadow.wgsl`, `coverage.wgsl`, `composite.wgsl`, `band.wgsl`) to GLSL ES 3.0 and that the full Bevy `core_pipeline`/tonemapping passes Buiy composites through also fit the downlevel envelope (historically fragile, bevy#17869). Treat as "viable pending end-to-end validation," not proven.
- **Inter-stage varyings** were checked and are within the WebGL2/WebGPU baseline of 16 (the shaders' `VertexOut` tops out ~7) — not a concern.

---

## 7. Latent / non-issues (verified, do **not** pre-emptively "fix")

- **getrandom is NOT in the wasm production graph.** `cargo tree -p buiy --target wasm32-unknown-unknown -e no-dev -i getrandom@0.3.4` (and `@0.4.3`) → "nothing to print". On web, winit drops `ahash` (removing 0.3.4) and `uuid` pulls no getrandom (removing 0.4.3); the only getrandom on wasm is via `proptest` under `buiy_verify`, a **dev-dependency that never ships**. So a wasm runtime build does **not** hit a getrandom hard-error and does **not** currently need the `wasm_js` feature. It becomes relevant only if future code re-activates getrandom on web (e.g. enabling `uuid`'s `js`/`v4` feature, ahash runtime-rng, or `rand`'s OS RNG) — at which point enable `wasm_js` for the relevant major (and note: as of getrandom 0.3.4 the **feature alone** selects the backend; the `--cfg getrandom_backend="wasm_js"` RUSTFLAG is a no-op). getrandom also needs a secure context (https/localhost) at runtime.
- **`x11`/`wayland` bevy features** are inert on wasm but should move under `[target.'cfg(unix)']` for cleanliness.
- **Time** already flows through Bevy's `Time` (web-time-backed on wasm); no `std::time::Instant`/`SystemTime` in runtime src.
- **`buiy_verify` harness, golden/capture, GPU `#[ignore]` lane** are all test/dev-only and never ship to wasm — but note that **in-browser visual verification would need a new harness**.

---

## 8. Phased roadmap

| Phase | Goal | Key tasks | Effort |
|---|---|---|---|
| **P0 — Compiles on wasm** | `cargo build --target wasm32-unknown-unknown -p buiy` succeeds | B1 arboard target-gate + `MemClipboard` default; move `x11`/`wayland` under `cfg(unix)`; cfg-gate golden/capture harness off wasm; re-run `cargo check --target wasm32` for the full bevy_render+wgpu29+naga+winit feature set (never reached before arboard failed) | M |
| **P1 — MVP: a widget renders in-browser (WebGPU)** | Smallest end-to-end proof | add bevy `webgpu` feature; web example crate (`Window{ canvas, fit_canvas_to_parent, prevent_default_event_handling }` + `console_error_panic_hook` + console tracing); trunk/`wasm-server-runner` + `index.html` canvas; pin wasm-bindgen CLI to 0.2.125; load in Chrome/Edge 113+. *Renderer needs no changes — band works on Dawn.* | M |
| **P2 — Looks/behaves right (WebGPU)** | Gallery correct on WebGPU | verify/repair sRGB-gamma on the negotiated surface; clamp target sizes to `max_texture_dimension_2d`; **runtime** macOS-modifier detection; `sys-locale` `js` feature; canvas `prevent_default` for wheel/touch; run `buiy_gallery` in-browser | L |
| **P3 — Browser reach (WebGL2 build)** | Second artifact for Firefox-default / older Safari / older Android | **B2 trim band to ≤16 attrs** (hard requirement here); **W1** `Rgba16Float`→`Rgba8Unorm`/extension-gated fallback; build with the bevy `webgl` feature; `navigator.gpu` feature-detect loader; validate the full Bevy 0.19 webgl2 path end-to-end (incl. naga GLSL-ES translation of all 5 shaders) | L |
| **P4 — Production polish** | Close the honest gaps | async `navigator.clipboard` provider (needs async/cached-read bridge); **web a11y story** (hidden-DOM/ARIA sink over `build_tree_update`, or wait on upstream) — **XL**; IME hidden-input/`EditContext` shim; binary-size pipeline (`opt-level=z`, `lto=fat`, `codegen-units=1`, `strip`, `wasm-opt -Oz`, brotli, loading screen — expect ~15 MB+); wasm CI lane (`cargo build --target wasm32` + headless-browser smoke) + add `wasm32-unknown-unknown` to `deny.toml` `graph.targets`; optional woff2 seam in `BuiyFontLoader` | XL |

**Critical path to a working browser render is P0+P1 (M+M)** — no XL or blocked work on it. The XL items (in-browser a11y especially) are genuinely optional for "renders in the browser."

---

## 9. Open questions to settle empirically

1. **sRGB encode:** does Bevy hand Buiy an sRGB-encoding view on the negotiated web surface, or is a final-pass encode needed? (P2 gate.)
2. **WebGL2 float extension:** does the compositor source layout's `filterable: true` force `OES_texture_float_linear`, or does `EXT_color_buffer_float` alone suffice given the `Nearest` sampler? (W1.)
3. **naga + full webgl2 path:** do all 5 WGSL shaders translate to GLSL ES 3.0, and does the Bevy core_pipeline/tonemapping chain fit the downlevel envelope on a real WebGL2 adapter? (P3 gate.)
4. **WebGPU adapter attribute count:** do all target WebGPU adapters report `maxVertexAttributes ≥ 17`, or should B2 land before the WebGPU MVP ships beyond Chrome/Edge desktop?
5. **MSAA:** `Msaa::Off` for web vs validating the 4× main pass on WebGL2.

---

## 10. Net

Buiy's render architecture is wasm-ready *by construction*, the dependency stack already carries the wasm layer, and the seams that matter (clipboard, font, a11y data/sink split, format-specialized pipelines) already exist. The heavy lifting is done. The honest scope is: **two hard fixes (arboard gate, band ≤16 attrs), a WebGL2 float fallback, and a tail of web-correctness/polish** — with in-browser accessibility the one genuinely large, optional, partly-upstream-gated piece.
