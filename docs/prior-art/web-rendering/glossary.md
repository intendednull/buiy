**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — glossary of system-specific terms

One concise line each. See the linked evidence files for detail; this is a lookup aid, not a tutorial.

- **WebGPU** — the W3C browser GPU API (`navigator.gpu`); wgpu's `webgpu` backend forwards WGSL to it directly with no shader translation and full compute/storage/MRT. ([wgpu-backends](wgpu-backends.md))
- **WebGL2** — OpenGL ES 3.0 exposed to a `<canvas>`; wgpu's `webgl` backend runs its `gles` HAL against it, with naga translating WGSL→GLSL ES 3.0. No compute stage. ([wgpu-backends](wgpu-backends.md))
- **`Backends::GL`** — the wgpu backend bitflag Bevy resolves for the `webgl` cargo feature on wasm; the WebGL2 path. ([bevy-bootstrap](bevy-bootstrap.md))
- **`Backends::BROWSER_WEBGPU`** — the wgpu backend bitflag Bevy resolves for the `webgpu` cargo feature on wasm; the native WebGPU path. ([bevy-bootstrap](bevy-bootstrap.md))
- **downlevel limits** — `wgpu::Limits::downlevel_webgl2_defaults()`, the reduced limit envelope Bevy forces on the `webgl` path (storage/compute = 0, 16 KiB UBO, 16 vertex attributes, 4 color attachments). ([wgpu-backends](wgpu-backends.md))
- **`wasm32-unknown-unknown`** — the Rust compile target for the browser; no OS, no filesystem, no ambient RNG/clock — runtime services come from JS shims. ([toolchain](toolchain.md))
- **wasm-bindgen** — post-processes the raw `.wasm` into a browser-importable `*_bg.wasm` + JS glue; the crate and `wasm-bindgen-cli` must be the same version (0.2.125 here) or the browser throws `LinkError`. ([toolchain](toolchain.md))
- **trunk** — the de-facto Bevy/Rust *app* bundler: runs cargo + wasm-bindgen + `wasm-opt`, copies assets, serves with live-reload from an `index.html`. ([toolchain](toolchain.md))
- **wasm-server-runner** — the de-facto dev-loop tool: register it as the wasm cargo runner and `cargo run --target wasm32-unknown-unknown` builds, bindgens, and serves in one step (no size optimization). ([toolchain](toolchain.md))
- **naga** — wgpu's shader translator; on the WebGL backend it converts every WGSL shader to GLSL ES 3.0 at pipeline creation (the historically fragile part of the WebGL path). ([wgpu-backends](wgpu-backends.md))
- **`SurfaceTargetUnsafe`** — the wgpu surface-creation input Bevy builds from a `RawHandleWrapper`; `::RawHandle` is the same code path on web (canvas) and native. ([bevy-bootstrap](bevy-bootstrap.md))
- **`RawWindowHandle::Web`** — the `raw-window-handle` 0.6.2 variant for a canvas: a numeric id matching the canvas's `data-raw-handle` attribute, which wgpu uses to locate it. ([winit-web](winit-web.md))
- **`EventLoopExtWebSys` / `spawn_app`** — winit's web extension; `spawn_app` registers the app as browser callbacks and returns immediately instead of blocking like `run_app`, so the browser main thread is never blocked. ([bevy-bootstrap](bevy-bootstrap.md), [winit-web](winit-web.md))
- **`WindowAttributesExtWebSys`** — winit's web builder methods (`with_canvas`, `with_prevent_default`, `with_focusable`, `with_append`) Bevy uses to bind winit to an existing `<canvas>`. ([winit-web](winit-web.md))
- **device-pixel-ratio (DPR)** — `window.devicePixelRatio`; winit's web `scale_factor`. The canvas backing store is `logical_size × DPR`, which on high-DPR displays can overflow `max_texture_dimension_2d`. ([winit-web](winit-web.md), [open-problems](open-problems.md))
- **COOP/COEP / cross-origin isolation** — the `Cross-Origin-Opener-Policy: same-origin` + `Cross-Origin-Embedder-Policy: require-corp` header pair that gates `SharedArrayBuffer`, hence wasm threads; a Spectre mitigation, not a Bevy choice. ([threading](threading.md), [open-problems](open-problems.md))
- **`EXT_color_buffer_float`** — the WebGL2 extension that makes `Rgba16Float` color-renderable and blendable; on WebGPU that capability is core. ([wgpu-backends](wgpu-backends.md))
- **AccessKit web adapter** — the planned-but-unshipped adapter that would bridge an AccessKit `TreeUpdate` into browser ATs via a hidden ARIA/DOM mirror; until it ships, canvas UIs reach no web screen reader. ([accessibility](accessibility.md))
- **getrandom `wasm_js`** — the getrandom 0.3.4+ Cargo feature that selects the `crypto.getRandomValues` web backend (the old `--cfg getrandom_backend` rustflag is no longer required, though it still works as an explicit override); absent from Buiy's wasm production graph today. ([toolchain](toolchain.md))
- **web-time** — drop-in replacement for `std::time` on wasm (`Instant`→`performance.now()`, `SystemTime`→`Date.now()`); Bevy routes its `Time` through it via `bevy_platform`. ([toolchain](toolchain.md))
- **console_error_panic_hook** — startup hook that prints Rust panics with a readable message + stack to the browser console instead of a bare `RuntimeError: unreachable`. ([toolchain](toolchain.md))

## Sources

- The sibling evidence files in this folder (cross-linked per term)
- `wgpu-types-29.0.3/src/limits.rs:574`; `bevy_render-0.19.0/src/settings.rs:71-97`
