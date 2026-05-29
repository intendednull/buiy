**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — the GPU rendering story: SDF primitives, three-backend split (Metal / wgpu / DX11), the Blade→wgpu migration, batching, clipping, rounded corners, gradients

# GPU rendering

GPUI's rendering pipeline is the load-bearing existence proof for Buiy's foundation §2.2-§2.3 commitment ("Bevy's render graph + wgpu — our render passes live in Bevy's render graph. Custom shaders for clipping, gradients, borders, filters, blend modes, top layer"). GPUI demonstrates that a custom-shader 2D UI pipeline can ship a serious productivity app — Zed 1.0 renders entirely this way at high frame rates on macOS, Linux, and Windows.

Three architectural commitments define GPUI's renderer:

1. **A small fixed set of typed primitive shaders.** Not "draw arbitrary triangle meshes" — there are ~8 primitive types, each with its own shader pipeline. The 2D-UI design space is reduced to "what can be expressed as a rectangle with rounded corners, optionally with a gradient, shadow, or clip."
2. **Signed Distance Functions for shape evaluation.** Rounded corners, drop shadows, clip masks all evaluate SDFs in the fragment shader. No tessellation. No stencil tricks. The pixel knows its distance to the nearest shape edge and decides its own opacity.
3. **Per-platform native graphics where useful, wgpu where pragmatic.** Three backends (Metal, wgpu, DX11) — see [`architecture.md` § "Per-platform abstraction layer"](architecture.md).

## The primitive set (Scandurra's _Videogame_ post)

From [_Leveraging Rust and the GPU to render user interfaces at 120 FPS_](https://zed.dev/blog/videogame) (2023-03-07), confirmed against the current `crates/gpui/src/scene.rs` source:

- **`Quad`** — rectangle with bounds, four corner radii, background color (solid or gradient), border (width + colors per side, also rounded), drop shadow.
- **`Shadow`** — drop shadow rendered as a separate primitive ahead of its caster so it can be blurred independently.
- **`Glyph` / `MonochromeSprite`** — a single text glyph at a quad, indexing into a single-channel alpha atlas. Color is applied per-instance.
- **`PolychromeSprite`** — colored sprite (color emoji, polychrome icons) indexing into a full-color atlas.
- **`Underline`** — text underlines and strikethroughs as a separate primitive; lets them clip independently of the glyph.
- **`Path`** — filled path with SDF-based anti-aliasing for arbitrary 2D shapes that aren't quads.
- **`Surface`** — embedded subsurface (video, web view, etc. — used sparingly in Zed).

That's effectively all of UI. Buttons are quads with borders. Text is glyphs. Drop-down arrows are paths. Images are polychrome sprites. Tabs are quads layered on quads.

## Signed Distance Functions for rounded shapes

The rounded-rectangle shader is the canonical example. The fragment shader receives:

- Quad bounds (center, half-extent)
- Per-corner radii
- Pixel position in quad-local space

It computes the SDF distance from pixel to the rounded-quad boundary, then maps that distance through a smoothstep to produce a 0-1 coverage value. The fragment alpha is `coverage * input_alpha`. Anti-aliasing is automatic and pixel-perfect at any zoom.

The same shader handles the border: compute SDF distance to outer edge, compute SDF distance to inner edge (outer minus border thickness), the band between is the border, inside the inner is the background.

Drop shadows use a closed-form Gaussian-blurred-rectangle approximation based on the error function — also a fragment-shader SDF computation, no convolution pass, one draw call per shadow.

**This is identical to the technique Buiy will need.** Bevy's wgpu pipeline can run the same shaders on the same primitive set. The Buiy [`buiy-render-pipeline-design`](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) sub-spec can borrow the primitive list, the SDF math, and the four-stage pipeline directly. Apache-2.0 means shader code is referenceable; license-compatible reimplementation in Buiy's MIT/Apache-dual context is straightforward.

## Glyph atlas and the alpha-as-color trick

Text rendering is the most expensive part of any UI. GPUI's strategy:

1. Shape glyphs once per (font, size, glyph_id) using OS shaping APIs (Core Text on macOS, DirectWrite on Windows, FreeType/HarfBuzz-adjacent on Linux). Cache by `LineLayout` to amortize across frames.
2. Rasterize glyphs once per (font, size, glyph_id, subpixel_x_offset) into a **single-channel alpha atlas** stored as a GPU texture. Four x-axis subpixel offsets give a 4x quality bump without quadrupling atlas size.
3. At draw time, instance one `Glyph` primitive per visible glyph. Each instance includes the atlas UV rect plus the desired color. The fragment shader samples the alpha and multiplies by the color.

**The alpha-as-color trick is critical.** Storing colored glyphs in the atlas would mean N copies for N colors. Storing alpha means one copy serves any tint, and theme color changes don't require atlas regeneration. Same trick works for monochrome icons.

Polychrome content (color emoji, full-color icons) gets a separate full-color atlas that pays the storage cost in exchange for no recoloring.

## Three GPU backends

Per the Cargo.toml dependencies and the [Blade→wgpu PR](https://github.com/zed-industries/zed/pull/46758):

### macOS — Metal (direct, via `metal` crate)

- Native Metal API via the `metal` Rust binding.
- Renders to a `CAMetalLayer` attached to the `NSWindow`.
- Pipeline objects are compiled once at startup; primitive shaders live in `.metallib` files.
- Color management via Core Graphics color spaces (display-P3 etc.).
- No wgpu involvement at all on macOS.

Why direct Metal: maximum performance, Apple-specific features (variable rate shading hooks, ProMotion sync), zero abstraction tax. Apple's text-rendering Core Text gives subpixel-positioned glyphs with grayscale anti-aliasing matching system font rendering.

### Linux — Blade (Vulkan) → wgpu migration

Historically Blade ([kvark/blade](https://github.com/kvark/blade)) — a Rust Vulkan abstraction. PR [#46758](https://github.com/zed-industries/zed/pull/46758) reimplements the Linux backend on wgpu, with the stated motivation:

> The blade graphics library is a mess and causes several issues for both Zed users as well as other 3rd party apps using GPUI. The PR removes blade and implements the linux platform using wgpu which is the de-facto standard in the rust UI and graphics ecosystem.

This is a load-bearing signal for Buiy: **the broader Rust UI ecosystem has converged on wgpu, and even GPUI is migrating to it where native APIs don't dominate.** Buiy's wgpu commitment is on the winning side of the empirical trend.

Linux specifics:

- Wayland + X11 windowing via `wayland-client` / `x11rb`.
- WGSL shaders translated through wgpu to Vulkan on the host driver.
- NVIDIA PRIME workarounds for hybrid-GPU laptops (PR [#23438](https://github.com/zed-industries/zed/pull/23438)) — known sharp edge.
- Text rendering: OS shaping via FreeType / HarfBuzz adjacency; no `cosmic-text` integration.

### Windows — DirectX 11 + DirectWrite (direct)

Per the [Zed on Windows announcement](https://zed.dev/windows):

> The Windows release uses DirectX 11 for rendering and DirectWrite for text shaping and ClearType rendering to provide predictable, native quality and smoother integration with the Windows graphics stack.

DirectX 11 (not 12) is a deliberate choice — broader driver compatibility, lower minimum hardware spec. DirectWrite is the Windows native text shaping/rasterization API; matching it with cross-platform crates is hard (subpixel ClearType is Windows-specific). No wgpu on Windows.

### Mobile and web

None. Tracking issues [#43206 (iOS)](https://github.com/zed-industries/zed/issues/43206) and [#43207 (Android)](https://github.com/zed-industries/zed/issues/43207) exist with no commitments. Web target (WASM) is not on the roadmap; the platform abstraction is too deeply tied to native APIs.

## Clipping and the top layer

GPUI implements clipping via **bounds passed as a uniform/instance attribute on every primitive**, not via stencil buffers or scissor rects. The fragment shader discards pixels outside the active clip rectangle. Nested clips push/pop a clip stack at scene-assembly time; only the final composed clip rect reaches the GPU per primitive.

This is computationally cheap (no stencil pass, no extra draws) and supports arbitrary clip nesting. It does not yet support arbitrary clip shapes — only axis-aligned rounded rectangles. For Zed this is fine; for Buiy's foundation §2.3 commitment to `clip-path` shapes (polygon clips, mask-image), the clip primitive itself needs to be richer (likely SDF-evaluated or shader-supplied mask textures).

**The "top layer" problem** — dialogs that visually float above all other content and cannot be clipped by ancestor overflow — is solved in GPUI by **scene-level draw-order overrides**. Elements can declare themselves on a higher draw layer; the scene sorter places them after everything else regardless of element-tree position. Same answer Buiy will need for foundation §2.3 "true top-layer compositing."

## Batching and draw call count

Per the Scandurra blog post and DeepWiki analysis:

- Each primitive type has its own shader pipeline.
- The scene assembly phase groups primitives by type and draw layer.
- Each (type, layer) group renders as a single instanced draw call.
- Atlas-backed primitives (glyphs, icons) share atlas binds across all instances.

A full Zed editor frame typically issues **single-digit GPU draw calls** despite painting thousands of primitives. This is the structural performance win — not "highly optimized triangle rendering," but "the right primitive decomposition lets you batch maximally."

Buiy's render pipeline can achieve the same batching shape on Bevy's render graph by:

1. Defining a Buiy-specific batched primitive material per type (`BuiyQuadMaterial`, `BuiyGlyphMaterial`, etc.).
2. Letting the render graph node walk Buiy's resolved layout/style data once per frame, emitting instance buffers per primitive type.
3. Issuing one draw per (primitive type, draw layer) pair.

The shape is well within what Bevy's `Material2d` and custom render-graph nodes can express. The sub-spec is `buiy-render-pipeline-design`.

## What GPUI does **not** do (yet)

These are the gaps relative to Buiy's foundation §2.3 ambitions — features GPUI doesn't currently ship that Buiy needs:

- **`backdrop-filter`** (Gaussian blur of the content under a panel — the iOS frosted-glass effect). Not in GPUI's primitive set.
- **`mix-blend-mode` / `isolation`** beyond standard alpha. GPUI does straight-alpha compositing; no multiply, screen, overlay, or isolated blend groups.
- **Arbitrary `clip-path` shapes.** Rounded rectangles only.
- **Color-managed gradients** in non-sRGB color spaces. The primitive supports gradient backgrounds but the color space story is sRGB-default.
- **Border-image.** No SVG-9-slice. Borders are solid color only.
- **Filter effects** (`drop-shadow` outside the quad primitive, `blur`, `hue-rotate`, etc.). Drop shadow is built into the quad; other filters are absent.

These are the **specific render-pipeline-extension opportunities** for Buiy. Each is a fragment-shader extension or a new primitive type. None are blocked by GPUI's pipeline structure; they just aren't built.

## Sources

- _Leveraging Rust and the GPU to render user interfaces at 120 FPS_: https://zed.dev/blog/videogame
- DeepWiki GPUI section: https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)
- Blade→wgpu PR #46758: https://github.com/zed-industries/zed/pull/46758
- Zed on Windows: https://zed.dev/windows
- _Zed editor switching graphics lib from blade to wgpu_ HN: https://news.ycombinator.com/item?id=47002825
- _Zed Editor Switches Graphics Library from Blade to wgpu_: https://ubos.tech/news/zed-editor-switches-graphics-library-from-blade-to-wgpu-for-better-performance/
- GPUI `Cargo.toml`: https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml
- GPUI iOS tracking #43206: https://github.com/zed-industries/zed/issues/43206
- GPUI Android tracking #43207: https://github.com/zed-industries/zed/issues/43207
