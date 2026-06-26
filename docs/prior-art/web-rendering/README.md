**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — folder overview

This folder documents **how a Bevy app — and therefore Buiy — renders in a web browser**: the end-to-end wasm path through Bevy 0.19, wgpu 29, and winit 0.30, from canvas bind to GPU surface, plus the toolchain, threading model, asset/font supply, accessibility reach, ecosystem precedents, and the structural gaps that path does *not* close. It is the reference corpus for any future spec that commits Buiy to a browser target. Evidence files stay neutral; Buiy-specific reads live only in clearly-marked "Implications for Buiy" sub-sections and in [lessons](lessons.md).

## Key facts

| | |
|---|---|
| **Subject** | Web/WASM rendering path of the Bevy + wgpu + winit stack |
| **Stack + verified versions** | bevy 0.19.0 · wgpu/wgpu-hal/naga 29.0.3 · winit 0.30.13 · raw-window-handle 0.6.2 · cosmic-text 0.19.0 · swash 0.2.9 · fontdb 0.23.0 · accesskit 0.24.1 · accesskit_winit 0.32.2 · wasm-bindgen 0.2.125 · web-sys 0.3.102 · web-time 1.1.0 |
| **The two backends** | **WebGPU** (`Backends::BROWSER_WEBGPU`, cargo `webgpu`) — native `navigator.gpu`, no shader translation, compute/storage/MRT core. **WebGL2** (`Backends::GL`, cargo `webgl`) — wgpu `gles` HAL over OpenGL ES 3.0, naga translates WGSL→GLSL ES 3.0, `downlevel_webgl2_defaults` limits forced |
| **The one-feature switch** | Per Bevy wasm artifact you pick **one** backend at compile time from the `webgl`/`webgpu` cargo features (`bevy_render/src/settings.rs:71-97`). The feature is named **`webgl`**, not `webgl2`. Bevy 0.19 has no single-binary runtime fallback (bevy#13168 open) |
| **Broad reach = two artifacts** | WebGPU ≈ 82–85% reach (mid-2026); WebGL2 ≈ near-universal. Full coverage = **two wasm builds + a JS `navigator.gpu` feature-detect loader** |
| **Buiy's two hard fixes** | **B1** `arboard` is an ungated dep with no wasm backend = compile blocker. **B2** the border/outline "band" pipeline declares 17 vertex attributes (`band.wgsl` `@location 0..16`) > the WebGL2 (and baseline-WebGPU) cap of 16 |

## Canonical reading order

1. [bevy-bootstrap](bevy-bootstrap.md) — how Bevy boots a wasm app end to end (event loop, canvas, async device, backend switch). Start here.
2. [wgpu-backends](wgpu-backends.md) — the WebGPU-vs-WebGL2 split, the downlevel limit table, the 16-attribute cap, `Rgba16Float` on WebGL2.
3. [browser-support](browser-support.md) — what browsers ship which backend (mid-2026), and the two-artifact reach strategy.
4. [winit-web](winit-web.md) — winit 0.30 web backend: non-blocking loop, canvas binding, input, the IME gap, clipboard.
5. [toolchain](toolchain.md) — wasm-bindgen, trunk/wasm-server-runner, getrandom, web-time, panic=abort, binary size.
6. [threading](threading.md) — why the web path is single-threaded and why that is correct for a UI lib.
7. [assets-and-fonts](assets-and-fonts.md) — HTTP fetch vs embed, `WebAssetPlugin`, cosmic-text/swash on wasm, woff2.
8. [accessibility](accessibility.md) — the data/sink split, and why a11y reaches no browser AT on web.
9. [ecosystem](ecosystem.md) — who actually ships this class of app to the browser (egui, Makepad, Vello, Bevy).
10. [open-problems](open-problems.md) — the structural gaps the path does not solve.
11. [lessons](lessons.md) — **the decision file**: Validates / Avoid / Borrow, tied to the feasibility report's B1/B2/W1.
12. [glossary](glossary.md) — one-line definitions of the system-specific terms.

## Table of contents

- [bevy-bootstrap.md](bevy-bootstrap.md) — Bevy's end-to-end wasm bootstrap
- [wgpu-backends.md](wgpu-backends.md) — wgpu 29's two browser backends + limit envelope
- [browser-support.md](browser-support.md) — WebGPU vs WebGL2 browser availability (mid-2026)
- [winit-web.md](winit-web.md) — winit 0.30 web backend (loop, canvas, input, IME)
- [toolchain.md](toolchain.md) — wasm build + serve toolchain
- [threading.md](threading.md) — threading model on wasm
- [assets-and-fonts.md](assets-and-fonts.md) — asset and font loading in the browser
- [accessibility.md](accessibility.md) — accessibility on the web
- [ecosystem.md](ecosystem.md) — who ships this path
- [open-problems.md](open-problems.md) — structural gaps
- [lessons.md](lessons.md) — Validates / Avoid / Borrow (decision file)
- [glossary.md](glossary.md) — system-specific terms

## Glossary

System-specific terms (WebGPU, WebGL2, `Backends::GL`/`BROWSER_WEBGPU`, downlevel limits, `SurfaceTargetUnsafe`, `EventLoopExtWebSys`/`spawn_app`, DPR, COOP/COEP, `EXT_color_buffer_float`, getrandom `wasm_js`, web-time, …) are defined one line each in [glossary.md](glossary.md).

## How to use

Read in the canonical order above for the full path, or jump by subsystem from the table of contents. If you are deciding *whether and how* Buiy adopts a web target, start at [lessons](lessons.md) (the Validates / Avoid / Borrow decision file) and the feasibility report it ties to, `docs/reports/2026-06-25-wasm-browser-support-feasibility.md`. Each evidence file is independently skimmable, cites file:line or authoritative URLs in its own `## Sources`, and cross-links its siblings.

**Framing disclosure.** These docs are written from Buiy's stance — a Rust UI library that runs its OWN custom Bevy render-graph pipeline (not bevy_ui), aiming at web-platform-parity semantics with AccessKit-first a11y. The 'Implications for Buiy' sub-sections read the Bevy/wgpu/winit web path through that lens. A future reader auditing whether that stance is itself right should weigh the corpus accordingly: it is a learn-from-the-stack-into-Buiy artifact, not a neutral catalog of every Rust-on-web option.

## Sources

- `bevy_render-0.19.0/src/settings.rs:71-97` — backend + forced-limit selection
- `wgpu-types-29.0.3/src/limits.rs:574` — `downlevel_webgl2_defaults`
- `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` — Buiy feasibility verdict (B1, B2, W1, phased roadmap)
- Sibling files in this folder (see table of contents)
