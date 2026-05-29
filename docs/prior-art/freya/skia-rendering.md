**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — Skia rendering integration via `freya-skia-safe`

# Skia rendering in Freya

Freya paints every element with **Skia** — the same 2D graphics engine that powers Chrome, Android, Flutter, and Firefox's WebRender fallback path. Skia gives Freya a *massive* feature surface (rounded clipping, gradients, shadows, blur, color filters, blend modes, text, SVG, paths) essentially for free; the price is a heavy C++ build dependency that ships as part of every Freya binary.

This is the substrate that Buiy explicitly **chose not to use** (foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly) — Buiy uses Bevy's render graph on wgpu directly with custom shaders). The lessons here are about *which primitives Freya gets for free from Skia* that Buiy will have to implement in wgpu shaders.

## The `freya-skia-safe` crate

Freya does not depend on the canonical `skia-safe` crate directly. It depends on **`freya-skia-safe 0.96.1`**, which is a fork (or a vendored variant) of `rust-skia`'s `skia-safe`. Configured with these Cargo features in Freya's workspace:

- `textlayout` — Skia's full text layout engine (paragraph builder, line breaking, text styles). **This is the text path.** Not cosmic-text.
- `svg` — Skia's SVG parser + renderer. Used for inline SVG support in the `svg` element.
- `webp` — WebP image decoding.

The `freya-skia-safe` fork exists, per maintainer commentary, to (a) pin specific Skia milestone versions Freya is tested against and (b) patch build-system rough edges. Upstream `skia-safe` is not directly substitutable; downstream consumers must use Freya's fork.

## Primitives Freya gets from Skia

Each of the following is **a Skia API call** in Freya's painter, not Freya-implemented:

| Primitive | Skia surface | Freya prop / element |
|---|---|---|
| Solid fill | `Paint::set_color` | `background: "rgb(...)"` |
| Linear gradient | `Shader::linear_gradient` | `background: "linear-gradient(...)"` |
| Radial gradient | `Shader::radial_gradient` | `background: "radial-gradient(...)"` |
| Conic gradient | `Shader::sweep_gradient` | `background: "conic-gradient(...)"` |
| Rounded corners | `Path::add_rrect` / `Canvas::clip_rrect` | `corner_radius: "10"` |
| Drop shadow | `MaskFilter::blur` + offset paint | `shadow: "0 4 10 rgb(...)"` |
| Inner shadow | Inverse clip + blurred paint | `shadow: "inset 0 4 10 ..."` |
| Backdrop blur | `Canvas::save_layer_alpha` + `ImageFilter::blur` | `backdrop_blur: "10"` |
| Color blend | `Paint::set_blend_mode` | `blend_mode: "multiply"` |
| Rotation / scale / skew | `Canvas::concat(Matrix)` | `rotate / scale_x / scale_y` |
| Clipping (rounded) | `Canvas::clip_rrect` | inherited from `corner_radius` |
| Clipping (path) | `Canvas::clip_path` | (limited; no full CSS `clip-path`) |
| SVG | `SvgDom::render` | `<svg svg_content="..." />` |
| WebP / PNG / JPEG | `Codec::decode` | `<image src="..." />` |
| Text (paragraph) | `ParagraphBuilder::build` + `Paragraph::paint` | `<label>` / `<paragraph>` |

The CSS-platform surface Freya exposes (rounded corners, gradients, shadows, blur, backdrop-filter, blend modes) is essentially **one-to-one with what Skia hands to it**. Freya rarely composes Skia primitives into higher-level shapes; it exposes them.

## Text rendering — Skia textlayout, NOT cosmic-text

The pre-amble for this corpus initially said Freya uses cosmic-text. **It does not.** Verified facts:

- Freya's workspace `Cargo.toml` declares `freya-skia-safe = { version = "0.96.1", features = ["textlayout", "svg", "webp"] }`.
- The `textlayout` feature pulls in **Skia's own paragraph + line-break + shaping engine** (which Skia itself sources from HarfBuzz + ICU under the hood).
- There is no `cosmic-text` dependency anywhere in the Freya workspace.
- The community misattribution is documented in [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) line 35: *"Freya uses Skia (via `freya-skia-safe`); Floem uses Parley 0.7.0. Do not cite either as cosmic-text users."*

Skia textlayout gives Freya:

- BiDi (via Skia's ICU bindings).
- Complex script shaping (HarfBuzz inside Skia).
- Color emoji (via Skia's native font handling — gets the OS font cascade).
- Font fallback (Skia's `FontMgr` per-platform).
- Selection rectangle geometry (`Paragraph::get_rects_for_range`).
- Caret positioning (`Paragraph::get_glyph_position_at_coordinate`).

The cost: **Skia owns the entire text stack.** Freya cannot intervene per-glyph or per-shaping-run; the text pipeline is opaque from Freya's perspective. cosmic-text gives an embedder more reach (per-span control, per-run shape access, glyph atlas ownership) — Buiy's choice.

## Skia surface, GL/Metal/D3D context

Freya runs Skia in **GPU-accelerated mode**: a Skia `Surface` is bound to a winit window's GL context (Linux/Windows) or Metal context (macOS). The frame loop is:

1. winit emits a `RedrawRequested` event.
2. Freya walks the dirty subtrees of its scene.
3. For each dirty node: emit Skia draw calls into the surface canvas.
4. Skia flushes the canvas to the GPU context.
5. Winit swaps buffers.

There is no separate "Freya render graph" abstraction — Skia's canvas API *is* the render graph. Compared to Bevy's render graph (where Buiy lives), this is dramatically simpler but also dramatically less composable with non-UI rendering work.

## Why Buiy did not pick Skia

The Buiy foundation spec is explicit ([architecture § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly)) that Buiy uses **Bevy's render graph + wgpu** with custom shaders for clipping, gradients, borders, filters, blend modes, top layer. The tradeoffs vs Skia:

| Concern | Skia (Freya) | wgpu (Buiy) |
|---|---|---|
| Feature surface day one | Massive — every CSS-platform primitive | Per-shader; Buiy must implement each |
| Build complexity | C++ + CMake + Clang; long builds | Pure Rust; cargo-only |
| Binary size | +20–40MB Skia C++ | smaller (just shaders + wgpu) |
| Integration with non-UI Bevy rendering | Foreign (Skia owns its own surface) | Native (shares render graph with 3D scene) |
| Per-pixel control | Black-box Skia internals | Full shader access |
| Mobile / WASM | Skia available but heavyweight on both | wgpu's WebGPU/WebGL backend native |
| 3D-anchored UI ([foundation § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) | Impossible inside Skia | Free — same render graph as 3D |

Buiy's "3D-anchored / diegetic UI" goal (foundation [architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns)) is structurally incompatible with Skia. That alone settles the substrate choice for Bevy.

## What Buiy borrows from Freya's Skia use

Specifically not the renderer — but the **primitive set as a checklist**. The list of Skia surfaces Freya exposes (rounded clip, gradients, shadows, blur, blend modes, transforms) is exactly the wgpu-shader checklist for `buiy-render-pipeline-design`. If Buiy's render-pipeline spec can render every primitive in this file's table, Buiy's renderer has parity with Freya's.

See also: GPUI (Zed's editor) and Vello as alternate-Rust-substrate examples. Vello is the closest pure-Rust analog to Skia. See [`ecosystem.md`](ecosystem.md).

## Sources

- Freya workspace `Cargo.toml` — https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- `rust-skia` upstream (basis for `freya-skia-safe`) — https://github.com/rust-skia/rust-skia
- Skia textlayout — https://skia.org/docs/dev/modules/skparagraph/
- Cross-references: [`../cosmic-text/lessons.md` line 35](../cosmic-text/lessons.md), [`../cosmic-text/ecosystem.md`](../cosmic-text/ecosystem.md), [`architecture.md`](architecture.md), [`critiques.md`](critiques.md), [`lessons.md`](lessons.md).
- Buiy foundation — [`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly), [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns).
