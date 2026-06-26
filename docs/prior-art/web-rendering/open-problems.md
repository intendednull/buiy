**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — what this path structurally does NOT solve

The Bevy/wgpu/winit browser path reliably gets pixels onto a `<canvas>` ([bevy-bootstrap](bevy-bootstrap.md), [wgpu-backends](wgpu-backends.md), and the [ecosystem](ecosystem.md) precedents). What follows is the inverse: the load-bearing gaps that are *structural* to a canvas-only Rust app in mid-2026 — not bugs to be fixed in a release, but properties of the platform/stack that any consumer (Buiy included) must design around. One paragraph each, cited. Buiy-specific takes are in [lessons](lessons.md); these statements stay neutral.

## In-browser accessibility: no AccessKit web adapter

AccessKit — the accessibility layer Bevy and egui (and Buiy) use — ships five platform adapters: Windows (UI Automation), macOS (NSAccessibility), Unix/Linux (AT-SPI), Android, and iOS. A **web/canvas adapter is listed only under "Planned adapters"** and has not shipped, with no published timeline. Because a canvas paints its own pixels and exposes no DOM semantics, a screen reader sees nothing unless something bridges the accessibility tree into ARIA/hidden DOM. On web today an app can build a fully correct AccessKit `TreeUpdate` and push it into `accesskit_winit`'s null adapter, reaching **zero** assistive technology, silently. Closing this is an XL effort (a hidden-DOM/ARIA overlay driven off the existing tree) or a wait on the upstream adapter. (Source: accesskit.dev roadmap / github.com/AccessKit/accesskit.)

## IME composition: winit emits nothing on web

winit's web backend does not deliver `Ime::Preedit`/`Ime::Commit`: a `<canvas>` element cannot receive the browser `CompositionEvent`, so IME input is documented as **Unsupported** on Web/Orbital (winit#4424; tracking winit#1497). The practical consequence is that CJK, dead-key, and accented composition do not work through the standard winit event path; only direct Latin keystrokes survive. The known workarounds both live *outside* winit: egui's approach of overlaying a hidden `<input>` to capture composition and forward it (called "quite hacky" by the maintainers), or the newer `EditContext` API — cleaner, but Chromium-only as of mid-2026, so premature to depend on. Any robust web text-editing story requires the consumer to own this shim. (Source: rust-windowing/winit#4424.)

## Mobile soft-keyboard: a canvas raises no on-screen keyboard

Related to but distinct from IME: on phones/tablets, a focused WebGPU/WebGL2 `<canvas>` does **not** trigger the on-screen keyboard. The OS keyboard appears only when a real DOM input (or an `EditContext`) holds focus — which a canvas-only app has none of. So on mobile, text *entry* may be impossible (no keyboard ever appears), independent of whether composition would work. This is the same hidden-input/`EditContext` gap as IME (winit#4424) viewed from the mobile side; without that shim, desktop hardware-keyboard input is the realistic ceiling for any pure-canvas app on this stack. (Source: rust-windowing/winit#4424 and its `EditContext`/hidden-input discussion.)

## No single binary for WebGPU + WebGL2: you ship two artifacts

Bevy's `webgpu` and `webgl` cargo features are mutually exclusive in one wasm build — `webgpu` overrides `webgl`, and the WebGL2 path additionally forces `downlevel_webgl2_defaults` limits at compile time ([wgpu-backends](wgpu-backends.md)). There is no runtime "detect `navigator.gpu`, else fall back" within a single `.wasm`. The upstream request to fix this, bevy#13168 "Support WebGL2 and WebGPU in the same WASM file," is **open** (labelled Ready-For-Implementation, not done). Until it lands, broad browser reach means building **two** artifacts and selecting between them in JS via a `navigator.gpu` feature-detect loader — doubling build, CI, and download-path complexity. (Source: bevyengine/bevy#13168.)

## Binary size and load time

Bevy wasm bundles are large: optimized builds are reported "upwards of 30 MB," reducible to roughly 15 MB with `wasm-opt`, and even a minimal Bevy example lands around 3.5 MB after size flags. This directly hurts first-load latency, so the cheatbook recommends deferring/streaming the wasm load behind a loading screen. The mitigation stack is the standard wasm size pipeline — `opt-level="z"`, `lto="fat"`, `codegen-units=1`, `strip`, `wasm-opt -Oz`, brotli/gzip transfer compression — applied *after* `wasm-bindgen`, plus delayed loading. None of it makes a Bevy-class bundle small in absolute terms; Makepad's few-hundred-KB bundles ([ecosystem](ecosystem.md)) show the floor is a property of how much engine you link, not of wasm itself. (Source: Bevy Cheat Book size-opt + webpage pages; bevy#14864.)

## Single-thread-only scheduler on the web

The path runs single-threaded in the browser by default. wasm multithreading needs `SharedArrayBuffer`, which the browser gates behind **cross-origin isolation** (the `Cross-Origin-Opener-Policy: same-origin` + `Cross-Origin-Embedder-Policy: require-corp` header pair), plus a threads-enabled toolchain (atomics + bulk-memory, `wasm-bindgen-rayon`-style plumbing). Bevy therefore runs its scheduler single-threaded on web unless an app opts into that whole stack and can serve the isolating headers — which also restricts cross-origin embedding. The simplest correct posture is to *stay* single-threaded on web and avoid the COOP/COEP/atomics rabbit hole entirely; the cost is no parallelism for systems or asset work. (Sources: web-platform COOP/COEP + `SharedArrayBuffer` requirement, developer.mozilla.org/en-US/docs/Web/API/SharedArrayBuffer; Bevy single-threaded-on-web, bevy-cheatbook.github.io/platforms/wasm.html.)

## sRGB / gamma on the negotiated web surface

Color correctness is not free on the web surface. A WebGPU canvas is configured with a non-sRGB storage format and gets sRGB-encode-on-write only via `viewFormats` (an `*-srgb` *view* over the canvas), rather than a directly-configurable `*-srgb` canvas format; a WebGL2 surface may be a plain `rgba8unorm` with no automatic encode at all. Any renderer that pre-linearizes colors on the CPU and *relies on the target to sRGB-encode on write* (as Buiy does, feasibility report §6) will render everything too dark or too bright if the engine hands it a non-encoding view. This is the highest-risk *correctness* item on the path because it fails silently (no error, just wrong colors) and must be checked empirically in-browser, possibly fixed with a final-pass encode shader. (Sources: WebGPU canvas `viewFormats`/sRGB behavior, developer.mozilla.org/en-US/docs/Web/API/GPUCanvasContext/configure; feasibility report §6.)

## High-DPR × logical size can exceed `max_texture_dimension_2d` at startup

A browser surface is sized `logical_size × devicePixelRatio` in physical pixels. On high-DPR displays (phones at DPR 3+, large hi-dpi monitors) that product, or any full-window offscreen/compositor target derived from it, can exceed the adapter's `max_texture_dimension_2d` — commonly **4096** on WebGL2/downlevel and on conservative mobile WebGPU adapters — which makes texture/surface allocation fail **at launch**, before anything renders. The mitigation is to clamp offscreen and compositor target sizes to the live adapter limit and/or cap the effective scale factor, rather than trusting the raw `devicePixelRatio`. This is a startup-time crash class specific to the unbounded-physical-size nature of web canvases, not present when you control the window size natively. (Sources: wgpu downlevel `max_texture_dimension_2d` limit, wgpu-types `limits.rs`; feasibility report §6.)

## MSAA on the main pass needs validation on WebGL2

A bare `Camera2d` defaults to `Msaa::Sample4`, so the window pass is 4× multisampled and any format/sample-specialized pipeline is built at that sample count. WebGL2 implements MSAA via multisampled renderbuffers plus an explicit resolve, and a multisampled float (`Rgba16Float`) target compounds the `EXT_color_buffer_float` dependency ([wgpu-backends](wgpu-backends.md)). That 4×-main-pass-plus-resolve path is supported by wgpu but is not exercised by a typical 2D app, so it must be validated per-backend rather than assumed. The low-risk posture for a 2D UI is to evaluate `Msaa::Off` on web (coverage-AA in the shader where edges need it) instead of relying on hardware MSAA. (Sources: wgpu GLES MSAA/resolve support, `wgpu-hal` GLES backend; feasibility report §6.)

## Sources

- AccessKit adapters + planned web adapter: <https://accesskit.dev/> · <https://github.com/AccessKit/accesskit>
- winit web IME unsupported (CompositionEvent, hidden-input, EditContext): <https://github.com/rust-windowing/winit/issues/4424> · <https://github.com/rust-windowing/winit/issues/1497>
- Single WASM binary WebGPU+WebGL2 (open): <https://github.com/bevyengine/bevy/issues/13168>
- Bevy wasm size + load: <https://bevy-cheatbook.github.io/platforms/wasm/size-opt.html> · <https://bevy-cheatbook.github.io/platforms/wasm/webpage.html> · <https://github.com/bevyengine/bevy/discussions/14864>
- Single-threaded web + cross-origin isolation: <https://bevy-cheatbook.github.io/platforms/wasm.html> · <https://developer.mozilla.org/en-US/docs/Web/API/SharedArrayBuffer>
- WebGPU canvas sRGB/viewFormats: <https://developer.mozilla.org/en-US/docs/Web/API/GPUCanvasContext/configure>
- Buiy feasibility report (§2, §6; sRGB, DPR, MSAA, max_texture_dimension): ../../reports/2026-06-25-wasm-browser-support-feasibility.md
