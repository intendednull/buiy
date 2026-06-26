# Buiy WASM / browser-rendering support — design

**Date:** 2026-06-25
**Status:** draft

Graduates the **web** target out of the [foundation roadmap](2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) into its own design spec, and resolves the web portion of the foundation's **§5 open question "Platform support staging"** ([README.md § 5](2026-05-07-buiy-foundation/README.md#5-open-questions)). Aligns with — does not contradict — [architecture.md § 2.9 "Platform support — staged"](2026-05-07-buiy-foundation/architecture.md), which already classifies web as a deferred, manual-release-gate platform; this spec gives that classification a concrete target architecture and entry conditions.

Research substrate (read these first): the [WASM/browser feasibility report](../reports/2026-06-25-wasm-browser-support-feasibility.md) (the file:line-verified findings) and the [`web-rendering` prior-art folder](../prior-art/web-rendering/) (how the Bevy + wgpu + winit stack renders in a browser; `lessons.md` is the decision file).

## Purpose

Define the **target shape** of running Buiy in a web browser: a Buiy app, compiled to `wasm32-unknown-unknown`, rendering its custom Bevy render-graph pipeline into an HTML `<canvas>`. This spec says *what* we build and *why*, names the load-bearing decisions with their rejected alternatives, and fixes the entry conditions under which web graduates from "deferred / manual-release-gate" toward CI coverage. The step-by-step migration (the P0–P4 sequence) belongs in a later `docs/plans/` entry, not here.

This is a **render-and-reach** target, not a feature-parity target: the goal is that Buiy *paints and is interactive* in a browser, with the platform-service gaps (browser a11y, IME, mobile keyboard, cross-app clipboard) **named and staged**, not silently shipped as working.

## 1. Scope

**In scope (target state):**

- A `wasm32-unknown-unknown` build of Buiy that compiles, links, and renders a widget into a `<canvas>`.
- Two render backends: **WebGPU** (primary) and **WebGL2** (reach fallback), shipped as **two wasm artifacts** selected by a JS feature-detect loader.
- The dependency-gating, build-tooling, and render edits required to make Buiy's *own* pipeline fit the chosen backend's limits.
- A staged a11y / IME / clipboard posture with each gap behind a named seam and explicitly disclosed.

**Out of scope (this spec):**

- WASM **without** Bevy, and SSR — already a foundation non-goal ([foundation README § 5 non-goals](2026-05-07-buiy-foundation/README.md)). This spec is WASM *with* Bevy only.
- A browser **screen-reader** bridge (no AccessKit web adapter exists; see § 2 D7).
- Mobile-first / touch-first UX. Desktop-browser hardware-keyboard input is the v1-web ceiling.
- The implementation plan and its task breakdown (separate `docs/plans/` doc).
- Changing Buiy's native targets or behavior in any way. The web target is **purely additive** — every decision below is gated on `cfg(target_arch = "wasm32")` or a new wasm-only build target, and native builds must be byte-for-byte unaffected.

## 2. Decisions

Each decision names the choice, the reason, and the rejected alternative(s). All claims are verified in the [feasibility report](../reports/2026-06-25-wasm-browser-support-feasibility.md) (§ refs inline) against the tree at `fdb8dda`.

### D1 — WebGPU-first, WebGL2-fallback, as two artifacts

**Decision.** Target **WebGPU first** (the MVP and primary build); ship a **WebGL2** second artifact for reach; select between them at load time with a JS `navigator.gpu` feature-detect. Bevy's `webgpu` and `webgl` cargo features do not coexist in one binary (`webgpu` takes precedence over `webgl` when both are set — no runtime fallback), and the single-binary dual-backend upstream request (bevy#13168) is open — so broad reach *is* two artifacts.

**Why.** Buiy's renderer uses no compute, no storage buffers, no MRT, no depth/stencil (feasibility § 2), so it sits inside the WebGL2 downlevel subset *by construction* — WebGL2 is viable, making this a reach/effort trade, not a capability gate. WebGPU-first is the fastest path to pixels (the renderer needs no changes for a Chrome/Dawn MVP) and makes the float-target issue (D5) vanish.

**Rejected.** *WebGPU-only* — excludes Firefox-default-off platforms and older Safari/Android; insufficient reach for an app UI lib. *WebGL2-only* — forfeits WebGPU's cleaner HDR-effect path and performance, and still needs D4. *Wait for single-binary dual-backend* — blocks on upstream bevy#13168 with no committed timeline.

### D2 — Inherit Bevy's browser bootstrap; add zero bootstrap code

**Decision.** Buiy writes **no** browser-bootstrap code. It rides Bevy's `DefaultPlugins` for the entire path: the non-blocking `spawn_app` runner, canvas-by-CSS-selector binding, surface-from-`RawWindowHandle::Web`, and the async adapter/device init resolved through `RenderPlugin::finish()`. See [`prior-art/web-rendering/bevy-bootstrap.md`](../prior-art/web-rendering/bevy-bootstrap.md).

**Why.** Buiy already constructs no `Window`, sets no `WgpuSettings`/`Backends`, and never requests an adapter/device — it only *reads* `Res<RenderDevice>` in its own `finish()` and prepare systems, which is exactly where Bevy guarantees the async-initialized device exists (feasibility § 2–§ 3). The only Buiy-side bootstrap surface is *configuration*: a web example sets the `Window.canvas` selector and the backend cargo feature.

**Rejected.** Any Buiy-owned surface/adapter/event-loop management — would duplicate Bevy and reintroduce the synchronous-adapter blocker Buiy currently avoids.

### D3 — Dependency gating: arboard off wasm, MemClipboard default, x11/wayland under cfg(unix)

**Decision.** Move `arboard` under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, cfg-gate `ArboardClipboard` + its re-exports, and default the `Clipboard` resource to the existing pure-Rust `MemClipboard` on wasm. Move the bevy `x11`/`wayland` features under `[target.'cfg(not(target_arch = "wasm32"))']` as well (they pull Linux-only deps irrelevant on wasm; gating on `not(wasm32)` rather than `unix` keeps the native feature set — Windows/macOS included — byte-for-byte unchanged, preserving the additive property). Add a web example crate.

**Why.** `arboard` 3.6.1 has **no wasm backend** and is an unconditional dep of `buiy_core` (`Cargo.toml:29`), instantiated as the default `Clipboard` (`text/mod.rs:301`) — it fails to compile for wasm before any render code is reached (feasibility § 4 B1, reproduced via `cargo check --target wasm32`). The `ClipboardProvider` trait facade makes the swap a one-construction-site change.

**Rejected.** A web clipboard backend *in v1* (D8 defers it — the sync trait can't wrap the async browser clipboard). Vendoring/patching arboard — unnecessary given the facade.

### D4 — Pack the border/outline band pipeline to ≤16 vertex attributes

**Decision.** Reduce `BorderBandInstance` + `band.wgsl` from **17 vertex attributes** (`@location 0..=16`) to **≤16** (fold the two clip `vec2`s into one `vec4`, the affine into one `vec4`, and/or move per-corner radii into an instance-indexed UBO). The existing `BORDER_BAND_INSTANCE_STRIDE_BYTES` assert guards instance **byte-stride** drift (it does not count attributes); the attribute *count* is enforced by pipeline creation against `max_vertex_attributes` — the change must satisfy both.

**Why.** `wgpu`'s `downlevel_webgl2_defaults()` caps `max_vertex_attributes = 16` (verified, `wgpu-types limits.rs:574`); a 17th attribute fails pipeline creation on WebGL2 → no borders/outlines/focus rings. The **WebGPU spec baseline is also 16**, so this is also a portability risk on conformant WebGPU adapters — it only works on desktop Dawn (~30) today (feasibility § 4 B2). **Hard** requirement for the WebGL2 build; **advisable** for cross-browser WebGPU reach.

**Rejected.** Requiring `max_vertex_attributes ≥ 17` at device creation — fails outright on WebGL2 and on baseline WebGPU; only a stopgap. Disabling the band pipeline on web — drops focus-visible rings, a WCAG 2.4.7 regression.

### D5 — WebGL2 float-target fallback for the effect compositor

**Decision.** On the WebGL2 build, gate the `Rgba16Float` effect-compositor targets on `EXT_color_buffer_float`, or substitute `Rgba8Unorm` group targets when absent. No change on the WebGPU build (core there).

**Why.** The effect-group compositor renders to / alpha-blends on / samples `Rgba16Float` (`compositor.rs:437`); on WebGL2 that needs `EXT_color_buffer_float`. Per the `wgpu-hal-29` GLES capability model (verified in source), that one extension makes rgba16f both renderable **and** blendable, `EXT_float_blend`/`OES_texture_float_linear` are the 32-bit-only variants not consulted for rgba16f, and wgpu marks rgba16f filterable unconditionally. That is the capability model, not an end-to-end browser run — confirm on a real WebGL2 adapter (§ 6). Only the EffectGroup path (opacity < 1, isolation, shadow/outline groups) hits this; flat widgets never touch a float buffer (feasibility § 5 W1).

**Rejected.** `Rgba8Unorm` everywhere on web — loses linear-space precision (banding on translucent overlap) even where the extension is present.

### D6 — Single-threaded on web

**Decision.** Run Bevy's single-threaded scheduler on web (the current default — `multi_threaded` is off). Do **not** pursue wasm threads (atomics + COOP/COEP cross-origin isolation + `wasm-bindgen-rayon` + nightly/`build-std`) for v1-web.

**Why.** Bevy 0.19 does not run its multithreaded scheduler on web regardless, and a latency-bound UI lib gains little from parallelism; the cross-origin-isolation requirement also restricts embedding (prior-art `threading.md`). Zero change required — Buiy is already single-threaded.

**Rejected.** wasm threads in v1 — high cost, low payoff, deployment-restricting.

### D7 — Accessibility: tree is web-ready, sink is staged and disclosed

**Decision.** On web, Buiy builds its AccessKit `TreeUpdate` as usual (the tree builder `a11y/translate.rs` is winit-free pure data) but reaches **no browser assistive technology** — AccessKit ships no web/canvas adapter (verified: not on crates.io; only web is unshipped among AccessKit's five adapters). This is **staged and must be disclosed** in any web milestone, matching foundation § 2.9 (web = manual-release-gate). A future web a11y sink (a hidden-DOM/ARIA overlay driven off the *same* `build_tree_update` output, or an upstream `accesskit_web`) swaps only the sink.

**Why.** The data/sink split already exists (`translate.rs` builds, `adapter.rs` sinks); the in-process driver already consumes the tree for tests + the agent interface with no winit adapter, proving the data layer is web-ready. Only the platform sink is missing.

**Rejected.** Claiming web a11y "works" because the tree builds (it reaches nothing — a silent WCAG failure). Blocking all web work on a DOM sink (an XL effort) — the render/reach target is independently valuable and the gap is honestly disclosable.

### D8 — IME, mobile keyboard, cross-app clipboard: deferred behind named seams

**Decision.** v1-web supports **desktop hardware-keyboard** Latin input only. IME composition (winit emits no `Ime::Preedit`/`Commit` on web — winit#4424), the mobile soft-keyboard (no on-screen keyboard without a focused DOM input/`EditContext`), and cross-app clipboard (async `navigator.clipboard` vs the sync `ClipboardProvider` trait) are **deferred**, each behind a named seam: a Buiy-owned hidden-`<input>`/`EditContext` shim (IME + mobile keyboard) and an async clipboard bridge (cross-app paste). In-app copy/paste works via `MemClipboard`.

**Why.** These are functional gaps, not compile blockers; they degrade safely (no crash). The shims live *outside* winit and are real, separable work.

**Rejected.** Owning the IME/keyboard shim in v1-web — scope the render/reach target first; the shim is its own follow-on.

### D9 — Fonts: embedded by default; assets via WebAssetPlugin or embed

**Decision.** Rely on Buiy's embedded default font (`include_bytes!` Fira Sans, `font_system.rs:22`) for the MVP; system-font scanning stays opt-in/off (a no-op on wasm regardless). Assets that must be fetched go through Bevy's `WebAssetPlugin` (added before `AssetPlugin`); non-Latin coverage is a font-*supply* task (embed or serve the faces).

**Why.** The browser exposes no system fonts; cosmic-text + swash + fontdb run on wasm with in-memory `Source::Binary`. Text renders on web with zero font-supply work for the MVP (feasibility § 2; prior-art `assets-and-fonts.md`).

**Rejected.** System-font scanning on web (no-op / dead path). woff2-in-engine (fontdb wants sfnt; woff2 decompression is a separate seam if needed).

## 3. Target architecture

The web target is a thin additive shell over the existing native architecture:

1. **Build axis.** A web build sets exactly one bevy backend feature — `webgpu` (primary) or `webgl` (fallback) — and is produced as a separate wasm artifact. Native builds add neither. (`webgl` is the cargo-feature name; it applies WebGL2 limits — prior-art `wgpu-backends.md`.)
2. **Dependency cfg-gating (D3).** `arboard` and `x11`/`wayland` move under target-cfg tables; `buiy_core` compiles for `wasm32-unknown-unknown`. getrandom needs **no** action today (verified absent from the wasm production graph; only enable `wasm_js` if a future dep re-activates it).
3. **Bootstrap inheritance (D2).** A web example crate configures `Window { canvas, fit_canvas_to_parent, prevent_default_event_handling }` + `console_error_panic_hook` + a console tracing layer; trunk / `wasm-server-runner` serves an `index.html` with the canvas. No Buiy bootstrap code.
4. **Two render edits, both behind single-source seams.** D4 (band ≤16 attrs — a coordinated `primitive.rs`/`instance.rs`/`band.wgsl` edit, byte-stride-asserted) and D5 (`Rgba16Float`→`Rgba8Unorm`/extension-gate — `compositor.rs` `group_target_descriptor`). D4 is hard for WebGL2 + advisable for WebGPU; D5 is WebGL2-only.
5. **Platform-service shims (D7/D8), staged.** Clipboard → `MemClipboard` (sync) now, async bridge later; IME/keyboard → deferred hidden-input shim; a11y → tree builds, sink deferred.
6. **Correctness wiring (verify-then-fix).** sRGB-encode on the negotiated surface, DPR × logical-size clamped to `max_texture_dimension_2d`, runtime (not `cfg!`) macOS-modifier detection, `Msaa::Off` evaluation, canvas `prevent_default` for wheel/touch. These are correctness items resolved empirically in-browser (§ 7).

## 4. Verification strategy (entry conditions for graduation)

Per foundation § 2.9, web starts as a **manual-release-gate** platform and graduates toward CI as harnesses allow:

- **Compile gate (CI, immediately addable).** A `cargo build --target wasm32-unknown-unknown` lane for the web build(s), plus `wasm32-unknown-unknown` added to `deny.toml` `graph.targets` so the web dependency graph is audited. This catches D3-class regressions cheaply and is the first thing to land.
- **Smoke gate (CI, near-term).** A headless-browser load of a web example that asserts the canvas paints (no panic; non-blank). `panic = abort` on wasm makes render-extract panics fatal (a historical Buiy footgun the headless gate never exercises), so this lane has real value.
- **Visual gate (manual / future).** Buiy's existing visual-bug harness (`buiy_verify`) is native render-to-texture + readback; **in-browser** visual verification needs a *different* harness (browser screenshot diffing) — out of scope here, named as the gap that keeps web at manual-release-gate for visuals.
- **A11y gate (blocked upstream).** Real browser-AT verification waits on a web a11y sink (D7). Until then web a11y is manual-release-gate and disclosed, exactly as § 2.9 states.

Graduation condition: web moves off "manual-release-gate" for a given dimension when that dimension has a CI-usable harness (compile and smoke are reachable now; visual and a11y are not).

## 5. Phasing (pointer)

The [feasibility report § 8](../reports/2026-06-25-wasm-browser-support-feasibility.md) sequences the work P0 (compile) → P1 (WebGPU MVP) → P2 (looks-right on WebGPU) → P3 (WebGL2 reach) → P4 (polish: clipboard/IME/a11y/size/CI). That sequence is the basis for the implementation **plan** (`docs/plans/`), authored separately when this spec is ratified. This spec commits to the *target* and the *decisions*; the plan commits to the *order*.

## 6. Risks & open questions

- **sRGB / gamma on the negotiated surface (highest correctness risk).** Buiy pre-linearizes on CPU and relies on sRGB-encode-on-write; if Bevy hands Buiy a non-encoding view, colors are silently wrong. Resolve empirically in-browser; possibly a final-pass encode shader. (feasibility § 6; open-problems.md.)
- **WebGL2 float-target extension (end-to-end).** The `wgpu-hal-29` capability model shows `Rgba16Float` needs only `EXT_color_buffer_float` (renderable + blendable) and is filterable unconditionally (D5) — but confirm real-adapter behavior end-to-end, and that the compositor source layout's `filterable: true` flag does not force a linear-filter extension despite the `Nearest` sampler. (feasibility W1 / OQ#2.)
- **naga WGSL→GLSL ES 3.0 translation.** "WebGL2 viable" is established at the feature-inventory level, not proven end-to-end for Buiy's five shaders or the Bevy core_pipeline/tonemapping passes (historically fragile, bevy#17869). The WebGL2 build must be validated end-to-end.
- **WebGPU adapter attribute count.** D4 removes the risk; until then a baseline adapter reporting 16 silently fails the band pipeline even on WebGPU.
- **Single-binary dual-backend (bevy#13168).** If it lands upstream, the two-artifact loader (D1) can collapse to one — revisit then.
- **Web a11y sink (D7).** Whether to author a Buiy DOM/ARIA overlay or wait on `accesskit_web` is deferred to a future a11y-on-web spec.

## 7. References

- Research: [WASM/browser feasibility report](../reports/2026-06-25-wasm-browser-support-feasibility.md); [`web-rendering` prior-art folder](../prior-art/web-rendering/) (esp. [`lessons.md`](../prior-art/web-rendering/lessons.md), [`bevy-bootstrap.md`](../prior-art/web-rendering/bevy-bootstrap.md), [`wgpu-backends.md`](../prior-art/web-rendering/wgpu-backends.md)).
- Foundation: [README § 5 open questions](2026-05-07-buiy-foundation/README.md#5-open-questions), [architecture § 2.9 platform support](2026-05-07-buiy-foundation/architecture.md).
- Code anchors (`fdb8dda`): `crates/buiy_core/Cargo.toml:29` (arboard), `crates/buiy_core/src/text/mod.rs:301` (default Clipboard), `crates/buiy_core/src/render/band.wgsl` + `instance.rs:233` (17-attr band), `crates/buiy_core/src/render/compositor.rs:437` (`Rgba16Float`), `crates/buiy_core/src/text/font_system.rs:22` (embedded font), `crates/buiy_core/src/a11y/translate.rs` + `adapter.rs` (a11y data/sink split).
