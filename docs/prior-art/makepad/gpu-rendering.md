**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — direct-backend GPU rendering (Metal / DX11 / OpenGL / WebGL); shader pipeline; the no-wgpu choice

# GPU rendering

Makepad's renderer is **directly written against each platform GPU API**, not abstracted through wgpu / WebGPU. Per the README and crate metadata: "compiles to wasm/webGL, osx/metal, windows/dx11 linux/opengl." Four backend implementations live in-tree under `makepad-platform`:

| Platform | GPU API | Surface |
|---|---|---|
| macOS / iOS | **Metal** (via metal-rs-style FFI) | CAMetalLayer |
| Windows | **DirectX 11** (D3D11 via Win32 + COM) | DXGI swapchain |
| Linux | **OpenGL** (via EGL / GLX) | X11 / Wayland window |
| Android | **OpenGL ES** | ANativeWindow |
| Web (WASM) | **WebGL** (via JS bindings) | HTMLCanvasElement |
| tvOS | Metal | shared with iOS |

OpenHarmony status is "builds but doesn't run yet" per Robrix README.

## The no-wgpu choice

Most contemporary Rust UI frameworks (Bevy, Iced, Dioxus desktop via Blitz, egui via wgpu backend) sit on wgpu — the cross-platform GPU abstraction crate. Makepad deliberately doesn't. The stated rationale (inferred from Rik Arends's posts and the dependency choices): keeping low-level control over per-platform peculiarities, shipping on platforms wgpu doesn't yet handle (e.g. tvOS, OpenHarmony), and avoiding the wgpu version-pin churn.

Costs of the choice:

- **Four backend implementations to maintain.** Metal / DX11 / OpenGL / WebGL all live in `makepad-platform`. Each new GPU feature (compute shaders, ray tracing, mesh shaders, descriptor indexing) has to be implemented four times — or supported only on the subset of platforms that have it.
- **No automatic wgpu improvements.** wgpu rolls a new release ~every six weeks with bug fixes, perf improvements, and feature catch-up; Makepad doesn't inherit them.
- **No Vulkan / DX12 backend.** Modern explicit-API backends (Vulkan on Linux / Android, DX12 on Windows, Metal Argument Buffers on Apple) are not in Makepad as of 1.0. wgpu has them.
- **Hardware acceleration disabled in some environments.** WebGL is the WASM target, not WebGPU; ANGLE / SwiftShader fallback decisions live in the platform code paths Makepad maintains.

## Shader pipeline

Shader code is **embedded in Live syntax** as inline GLSL-flavoured snippets:

```live
draw_bg: {
    instance hover: 0.0,
    fn pixel(self) -> vec4 {
        return mix(self.color, #4af, self.hover);
    }
}
```

The Live compiler parses these snippets, performs type inference against the surrounding Live properties (`instance` declares a per-instance attribute the GPU reads), and emits backend-specific shader source — MSL for Metal, HLSL for DX11, GLSL ES for OpenGL ES / WebGL, GLSL for desktop OpenGL. The crate is `makepad-shader-compiler` (also 0% documented on docs.rs).

This is genuinely novel — the only mainstream Rust UI library where **the application author writes shader code as a first-class part of the UI definition**. Buttons can declare their own `pixel` functions. Backgrounds can be procedural. Animation values flow into shaders as uniforms or instance attributes directly from Live property bindings. The cost is a learning curve that's part GLSL, part Live, part Makepad's particular shader semantic conventions.

## Drawing primitives

The `Cx2d` (2D drawing context, threaded through `draw_walk`) exposes a small set of primitives:

- `DrawQuad` — instanced rectangle / rounded-rect with custom pixel shader. The workhorse for backgrounds, fills, borders.
- `DrawText` — glyph runs via Makepad's font atlas. See below.
- `DrawShader` — generic shader-instanced draw call. Triangles, lines, points.
- `DrawCube` / 3D primitives — for the 3D examples (`splash`, `3D`, `xr`).

Every `Draw*` is itself a Live-bound widget: properties (colour, geometry, instance attributes) flow from Live syntax into uniforms / vertex attributes. Batching is automatic for adjacent same-shader draw calls. Atlas warmup is the application's responsibility (no auto-prewarm).

## Text rendering

Makepad ships **its own text shaping and font-atlas pipeline**. Not cosmic-text. Not HarfBuzz. The crate is `makepad-font` / part of `makepad-platform`. Capabilities (per examples + crate scanning):

- Glyph rasterization from TrueType / OpenType fonts.
- Distance-field text (SDF) for sharp scaling.
- Basic LTR shaping.

Limitations versus cosmic-text (Buiy's text engine):

- **No BiDi / Unicode UAX #9 shaping.** RTL scripts, mixed-direction paragraphs, are not first-class.
- **Limited complex-script shaping.** Indic / Arabic / Thai shaping is not equivalent to HarfBuzz-driven shaping.
- **No font fallback by Unicode block.** Emoji / CJK fallback is manual.
- **No IME composition surface.** Composed-input rendering for CJK / Korean is rudimentary.

Robrix as a Matrix client surfaces these limitations in practice — see [`open-problems.md`](open-problems.md) and [`critiques.md`](critiques.md).

## Clipping, blend modes, filters

- **Rectangular clipping** via scissor / viewport. Rounded-rect clipping via shader-based SDF (each draw evaluates the SDF in `pixel`).
- **Non-rectangular `clip-path`** — supported via SDF shapes if the application author writes the SDF in the pixel shader. No high-level `clip-path: circle(...)` declarative shape.
- **Mix-blend-mode / isolation / groups** — not modeled at the Live layer. Achievable via custom render passes if the developer writes them.
- **Backdrop-filter** — example `windows_blur` demonstrates host-window-level backdrop blur; per-element `backdrop-filter` is not a first-class Live property.
- **Top-layer / popup compositing** — popups render in-tree as `<PopupNotification>` / `<Modal>` widgets within the same window surface; no separate true-top-layer compositing equivalent to the web's `<dialog>` top layer.

## Implications for Buiy

- **Validates GPU-rendering for production app UI.** Robrix runs daily on macOS / Linux / Windows / Android / iOS at acceptable performance. The GPU-renderer choice is not science-fiction; it ships. Buiy's own render pipeline ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) is validated as a viable shape.
- **Borrow: shader-as-first-class authoring.** Live's inline `fn pixel(self) -> vec4` is a uniquely powerful primitive — application authors can write shaders alongside layout / styling. Buiy's render pipeline sub-spec ([`buiy-render-pipeline-design`](../../specs/2026-05-07-buiy-foundation/README.md)) should evaluate whether BSN could carry inline shader snippets the same way.
- **Avoid: skipping wgpu.** Buiy stays on Bevy's render graph + wgpu. The corpus treats Makepad's four-backend maintenance burden as a cost, not a feature. wgpu is the right substrate for a UI library that doesn't have a 6+ year backend-maintenance head start.
- **Avoid: skipping cosmic-text.** Makepad's text stack is below cosmic-text's capabilities (BiDi, complex shaping, emoji, font fallback). Buiy's cosmic-text commitment ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) is *exactly* the corrective. See also [`../cosmic-text/`](../cosmic-text/) if that folder exists; the lesson is clear.
- **Borrow: dirty-region rendering discipline (if Makepad ships it; partial).** Per the examples, Makepad's draw pipeline is not always full-clear-redraw — some demos use dirty-region updates. Buiy's perf budget thinking benefits from the same discipline.

## Sources

- README: https://github.com/makepad/makepad
- `makepad-platform` (per repo `platform/` folder structure)
- `makepad-shader-compiler` docs.rs: https://docs.rs/makepad-shader-compiler/latest/ (0% documented, module structure only)
- Examples `splash`, `shader`, `windows_blur`, `xr`: https://github.com/makepad/makepad/tree/dev/examples
- Sibling files: [`architecture.md`](architecture.md), [`live-language.md`](live-language.md), [`open-problems.md`](open-problems.md), [`lessons.md`](lessons.md)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.2, § 2.3
