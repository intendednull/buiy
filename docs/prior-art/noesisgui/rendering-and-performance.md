**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — GPU vector rendering, tessellation pipeline, performance posture

# Rendering & performance

NoesisGUI's rendering approach is "WPF's drawing model, rebuilt for game engines on the GPU." All visuals — controls, text glyphs, shapes, gradients — are vector primitives tessellated at runtime into indexed triangle batches submitted through the `RenderDevice` interface (see [`architecture.md`](architecture.md)). The vector-graphics-on-GPU posture is the differentiator from competitors like Coherent Gameface (which renders an HTML/CSS DOM tree via a modified WebKit) and Unity UGUI (which is rect-and-sprite-based with limited vector support).

## Vector graphics on GPU

The Noesis renderer is described in the [docs](https://www.noesisengine.com/docs/Gui.Core.RenderingTutorial.html) as:

> A resolution-independent, vector-based rendering engine built to leverage modern graphics hardware, delivering crisp, adaptive visuals at any size.

The pipeline:

1. **Geometry construction.** Each visual element produces a vector geometry (paths, rectangles, ellipses, text-as-glyph-outlines).
2. **Tessellation.** Geometries are converted to indexed triangles via a "GPU-assisted tessellation algorithm." The algorithm produces resolution-independent triangulations — the same vector input gives correctly anti-aliased output at any zoom.
3. **Batching.** Triangles are batched by shader and texture binding to minimise draw-call count.
4. **Submission.** Batches are submitted via `RenderDevice` calls on the host's render thread.

The pipeline is "paint every pixel per frame" (per the WPF-comparison docs: *"NoesisGUI is optimized for rendering dynamic interfaces"* and *"paints every pixel per frame like a game engine, unlike WPF's static interface optimization."*) This differs from WPF, where retained-mode caching tries to skip work when the visual tree is unchanged. Noesis assumes the visual tree changes frequently (game UIs are highly animated) and re-tessellates per frame.

## Performance claim: sub-millisecond

NoesisGUI's headline performance claim, from the [features page](https://www.noesisengine.com/noesisgui/):

> Render in sub-millisecond times across all platforms.

The claim is qualified — it is "render time" specifically, not full frame budget. The numbers are not contextualised with reference scenes (node count, primitive count, shader complexity), and there is **no published benchmark methodology**. Treat the claim as a marketing assertion that production AAA games (Baldur's Gate 3, Hellblade 2) have shipped on it — the implied performance bound is "fast enough for AAA," not a machine-verifiable number.

For Buiy this is a useful precedent (GPU-rendered vector UI ships in AAA at acceptable performance) without giving Buiy a specific numeric target to match. Buiy's foundation spec ([§ 5 open questions](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)) lists per-fixture performance budgets as a question for `buiy-verification-design`; Noesis does not constrain that work, only validates that GPU vector rendering is a viable architecture for game UI.

## What the renderer ships

From the feature catalogue and Unity / Unreal docs, Noesis's renderer handles:

- **Filled and stroked paths** with miter / bevel / round joins; even-odd and non-zero fill rules.
- **Gradients** (linear, radial) with multi-stop interpolation.
- **Effects:** drop shadows, blur (via `BackgroundEffect` which blurs everything behind a panel — comparable to CSS `backdrop-filter`).
- **Rounded corners + clipping** via geometry tessellation, so rounded clip rects are first-class (no shader-based pixel masking required).
- **Text glyphs** as tessellated outlines (with cached glyph atlases at common sizes for performance).
- **Variable fonts** (since 3.2; supports OpenType variable axes).
- **Stroke effects on text** (XAML extension Noesis added; not present in stock WPF).
- **Single-pass stereo rendering** for VR (added in 3.2).
- **World-space UI in 3D** (UI rendered into world space without render-to-texture).
- **Rive integration** (`RiveControl`, since 3.2.0) — Rive animation files embedded as UI elements.
- **Lottie integration** (After Effects animations via the `Lottie-Noesis` python tool that converts to XAML).

The render-feature surface is approximately CSS-compositing + part-of-the-way through CSS Filters. Notably, the documentation does **not** mention CSS `mix-blend-mode`-style compositing, `clip-path` polygon clipping (only rect / rounded-rect / vector-path), or color-space gradients (gradients are sRGB by default). Buiy's foundation spec commits to all of these ([architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns)), so Buiy aims wider than Noesis on the renderer.

## Comparison vs UGUI and Slate

| Capability | UGUI (Unity) | Slate / UMG (Unreal) | NoesisGUI | Buiy target |
|---|---|---|---|---|
| Rounded clipping | No (shader workaround) | Yes (limited) | Yes | Yes |
| Vector paths | No | Limited | Yes | Yes |
| Gradients | No | Yes | Yes | Yes (any color space) |
| Backdrop blur | No | No | Yes (BackgroundEffect) | Yes |
| Mix-blend-mode | No | No | No | Yes |
| Color-space gradients | No | No | No | Yes |
| True top layer | No | Limited (popovers) | Yes | Yes |
| Variable fonts | Partial | Limited | Yes | Yes (cosmic-text) |
| Color emoji | Partial | No | Yes | Yes (cosmic-text) |
| Complex script shaping | Limited (TextMeshPro) | Limited | Yes (per 3.2 docs) | Yes (cosmic-text + harfrust) |
| Single-pass stereo VR | Yes | Yes | Yes | Open question |

The pattern: Noesis is roughly on par with what AAA studios expect from a 2026 commercial UI library; Buiy aims slightly wider (web-platform parity), which is a deliberate scope expansion the foundation spec commits to.

## Memory & atlas management

Public documentation does not detail glyph-atlas management or texture-budget characteristics. The render pipeline is described as RAM-managed by the host engine (textures come from the host's asset system; the renderer owns only its own GPU-side caches). This is appropriate for a library-not-engine, but means there is no published characterisation of Noesis's memory footprint at scale.

For Buiy, this is a gap-not-counter-example: nobody publishes UI-library memory budgets in this category. Buiy's verification harness ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) treats memory as a productivity-app fixture concern; the lack of a Noesis benchmark to compare to is normal, not a Noesis defect.

## Implication for Buiy

GPU-tessellated vector rendering for game UI is a *production-proven* architecture — Baldur's Gate 3 ships on it. Buiy's foundation [§ 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns) commits to a custom Bevy render-graph pipeline; Noesis is the existence proof that this works at AAA scale. Specific patterns worth borrowing:

- **Per-frame tessellation, no retained caching at the geometry level.** Cache the *texture* (glyph atlas), not the *tessellation*. This matches Buiy's likely shape with wgpu render passes.
- **`RenderDevice`-style abstraction is unnecessary for Buiy** because Bevy provides one (the render graph + wgpu). Don't reinvent.
- **Per-frame render thread submission.** Bevy's ExtractSchedule + render world is the analog; Buiy fits cleanly into it.
- **Variable fonts + complex-script support are AAA-table-stakes.** Buiy's cosmic-text commitment covers this.

What Noesis *doesn't* show is whether the GPU-tessellation approach handles the higher-end web-platform features (`mix-blend-mode`, color-space gradients, polygon `clip-path`) cleanly. That's Buiy's frontier; Noesis is precedent for the substrate but not for the frontier.

## Sources

- NoesisGUI rendering tutorial — https://www.noesisengine.com/docs/Gui.Core.RenderingTutorial.html
- Technology and features — https://www.noesisengine.com/noesisgui/
- WPF / UWP comparison — https://www.noesisengine.com/docs/Gui.Core.WPFComparison.html
- 3.2 changelog — https://www.noesisengine.com/docs/Gui.Core.Changelog.html
- Buiy foundation architecture § 2.3 — ../../specs/2026-05-07-buiy-foundation/architecture.md
