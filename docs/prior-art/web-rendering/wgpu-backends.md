**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — wgpu 29's two browser backends (WebGPU and WebGL)

wgpu 29 reaches the browser through **two mutually-distinct backends**, selected by
cargo feature. A Bevy app (and therefore Buiy) renders through whichever one the
build enables. They are not API-compatible at the limit level, so the choice is
load-bearing for what a renderer can do.

## The two backends

**WebGPU** — `Backends::BROWSER_WEBGPU`, cargo feature `webgpu`. Talks to the
browser's native `navigator.gpu` (the W3C WebGPU API) via `web-sys`. wgpu compiles
a thin `ContextWebGpu` wrapper (`wgpu-29.0.3/src/backend/webgpu.rs`, gated
`#[cfg(webgpu)]` in `src/backend/mod.rs:1`) that forwards directly to the browser
implementation — there is **no** `wgpu-core`/`wgpu-hal` layer underneath and **no**
shader translation (WGSL is handed to the browser as-is). The `webgpu` feature is
still marked unstable: building it requires `RUSTFLAGS=--cfg=web_sys_unstable_apis`
"because webgpu is unstable and unstable APIs don't follow web_sys semver" (wgpu
wiki). Compute, storage buffers, MRT, `Rgba16Float` render/blend — all core.

**WebGL** — `Backends::GL`, cargo feature `webgl`. Runs wgpu's `gles` HAL
(`wgpu-hal-29.0.3/src/gles/`) against a **WebGL2** context, i.e. OpenGL **ES 3.0**
semantics. Goes through the full `wgpu-core` → `wgpu-hal` stack, and **naga
translates every WGSL shader to GLSL ES 3.0** at pipeline creation. WebGL2 has no
compute stage at all (ES 3.0, not 3.1), so the limit set is severely cut (below).

## Backend selection: compile-time feature, with one runtime nuance

The `webgpu`/`webgl` cfgs are independent (`wgpu-29.0.3/build.rs:13-14`:
`webgpu = feature "webgpu"`, `webgl = feature "webgl"`). They are **not** strictly
mutually exclusive at the wgpu level: with **both** features on, `Instance::new`
probes `navigator.gpu` at runtime and picks WebGPU when present, else falls back to
the GL context (`wgpu-29.0.3/src/api/instance.rs:71-91` — `requested_webgpu &&
support_webgpu` ⇒ `ContextWebGpu`, otherwise `ContextWgpuCore`). So a single wasm
binary *can* host both and auto-select.

**Bevy 0.19 does not expose that.** `WgpuSettings::default()` resolves a single
`Backends` value at compile time from the two features — `Backends::GL` for
`webgl` (and not `webgpu`), `Backends::BROWSER_WEBGPU` for `webgpu`, else native
auto-select (`bevy_render/src/settings.rs:71-84`); `WGPU_BACKEND` env-overrides
it. On the `webgl` path Bevy also forces `wgpu::Limits::downlevel_webgl2_defaults()`
(`settings.rs:90-97`). Net: per Bevy artifact you get **one** backend. Broad
browser reach therefore means two wasm builds plus a JS `navigator.gpu`
feature-detect loader — see [browser-support](browser-support.md).

## `downlevel_webgl2_defaults` — the WebGL2 limit envelope

From `wgpu-types-29.0.3/src/limits.rs:574` `downlevel_webgl2_defaults()` (the set
Bevy forces on the `webgl` path) — the storage/compute zeros are set there; the
`max_vertex_attributes` / UBO / VBO / color-attachment rows are inherited via
`..downlevel_defaults()`:

| Limit | Value | Bites a 2D UI renderer? |
|---|---|---|
| `max_storage_buffers_per_shader_stage` | **0** | No SSBOs at all; no compute |
| `max_storage_textures_per_shader_stage` | **0** | No storage images |
| `max_storage_buffer_binding_size` | **0** | — |
| all `max_compute_*` | **0** | No compute stage on WebGL2 |
| `max_uniform_buffer_binding_size` | **16 KiB** | Caps a single UBO binding |
| `max_vertex_buffers` | **8** | Roomy for instanced 2D |
| `max_vertex_attributes` | **16** | **The one that bites** (see below) |
| `max_color_attachments` | **4** | Fine for single-target 2D |

For a 2D UI renderer the storage/compute zeros are usually non-issues (you don't
use them), and 16 KiB UBOs / 8 VBOs are comfortable. The limit that actually bites
is **`max_vertex_attributes = 16`**: any pipeline that wants a 17th vertex
attribute (`@location(16)`) fails to create. WebGPU's own spec baseline is *also*
16, so an over-16 attribute layout is a portability risk on conformant WebGPU
adapters too — it only "works" on desktop Dawn because Dawn reports ~30.

### Implications for Buiy
Buiy's pipeline is inside this envelope by construction (no compute/storage/MRT/
depth-stencil), **except** the border/outline "band" pipeline, which declares **17**
vertex attributes (`band.wgsl` `@location(0..=16)`) and so exceeds the 16 cap on
WebGL2 *and* on a baseline WebGPU adapter. That is the renderer's single hard
limit-violation — packing it to ≤16 attributes is a hard requirement for any
WebGL2 build and spec-safety insurance for WebGPU. See
[lessons](lessons.md) and `docs/reports/2026-06-25-wasm-browser-support-feasibility.md`
(B2).

## Float render targets on WebGL2 (`Rgba16Float`)

Verified against `wgpu-hal-29.0.3/src/gles/adapter.rs`. `Rgba16Float`'s texture
capabilities (`adapter.rs:1190`) are `filterable | storage | half_float_renderable`,
which decomposes as:

- **`filterable`** (`adapter.rs:1102`) = `COPY_SRC|COPY_DST|SAMPLED|SAMPLED_LINEAR`,
  added **unconditionally** for `Rgba16Float`. So linear-filtered *sampling* of an
  rgba16f texture needs **no** extension on the GLES backend — a source binding
  marked `filterable: true` does **not** force `OES_texture_float_linear`. (That
  feature, `FLOAT32_FILTERABLE` at `adapter.rs:578-583`, only governs the **32-bit**
  formats `R32/Rg32/Rgba32Float` via `texture_float_linear`; it never touches
  rgba16f.)
- **`half_float_renderable`** (`adapter.rs:1130-1136`) = `COLOR_ATTACHMENT |
  COLOR_ATTACHMENT_BLEND | sample_count | MULTISAMPLE_RESOLVE`, gated on the
  private cap `COLOR_BUFFER_HALF_FLOAT`. That cap is set when the GL extension list
  contains `GL_EXT_color_buffer_half_float`, `GL_ARB_half_float_pixel`, **or** any
  `color_buffer_float` (`GL_EXT_color_buffer_float` / `GL_ARB_color_buffer_float` /
  `EXT_color_buffer_float`) — `adapter.rs:621-629`.

**Conclusion:** on WebGL2, the single extension **`EXT_color_buffer_float`** is
sufficient to make `Rgba16Float` color-renderable **and** blendable (it satisfies
`COLOR_BUFFER_HALF_FLOAT`); linear sampling is already unconditional. The 32-bit
variants **`EXT_float_blend`** and **`OES_texture_float_linear`** are **not**
required for rgba16f, per wgpu-hal 29's capability model. (Caveat: this is wgpu's
*model* of what the backend permits — the actual browser/driver must honor it;
confirm empirically. Without `EXT_color_buffer_float`, wgpu won't expose rgba16f as
a render target at all, so a fallback to `Rgba8Unorm` is the alternative.) On
WebGPU, `Rgba16Float` renderable+blendable+filterable is **core** — none of this
applies.

## WGSL → GLSL ES 3.0 via naga (WebGL backend only)

The `webgl` backend depends on naga translating each WGSL shader to GLSL ES 3.0 at
pipeline creation (same naga version as native; `naga-29.0.3`). This is
historically the fragile part of the WebGL path: naga can emit GLSL that the
browser's GLSL compiler rejects in edge cases (e.g. unsized arrays, or constructs
with no ES 3.0 expression). Anything requiring the WebGL2 envelope above
(storage, compute, MRT beyond 4, >16 attributes) has no ES 3.0 translation by
definition. "WGSL compiles on WebGPU" therefore does **not** imply "the same WGSL
translates on WebGL2" — the webgl path must be validated shader-by-shader,
end-to-end, on a real WebGL2 adapter.

## Async adapter/device on the web

On wasm the adapter/device request is awaited, not blocked on — and Bevy owns the
whole handoff, so a consumer that only reads `Res<RenderDevice>` inherits it for
free and adds no synchronous-adapter blocker. The mechanism (the
`spawn_local`-detach vs `block_on` split and the `FutureRenderResources` →
`RenderPlugin::finish()` handoff) is documented once in
[bevy-bootstrap §4](bevy-bootstrap.md); see [threading](threading.md) for the
`web_task` cancellable-future angle.

## Sources

- wgpu wiki, "Running on the Web with WebGPU and WebGL": https://github.com/gfx-rs/wgpu/wiki/Running-on-the-Web-with-WebGPU-and-WebGL
- `wgpu-29.0.3/build.rs:13-14` (backend cfg aliases); `src/backend/mod.rs:1`; `src/api/instance.rs:71-130` (runtime WebGPU-vs-GL selection); `Cargo.toml:108-131` (`web`/`webgl`/`webgpu` features)
- `wgpu-types-29.0.3/src/limits.rs:574` (`downlevel_webgl2_defaults`)
- `wgpu-hal-29.0.3/src/gles/adapter.rs:578-583, 621-633, 1102, 1130-1146, 1190` (float-format capabilities)
- `bevy_render-0.19.0/src/settings.rs:71-97, 259-301`; `src/lib.rs:452-458, 501` (backend selection, forced webgl2 limits, async device init)
- naga WGSL→GLSL fragility: https://sotrh.github.io/learn-wgpu/beginner/tutorial3-pipeline/ ; original WebGL2 backend PR https://github.com/gfx-rs/wgpu/pull/1686
- `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` (B2, W1, §3, §5)
