**Date:** 2026-05-29
**Status:** active
**Subject:** WebRender — Servo's GPU renderer (display lists + retained scene + batching + frame builder); upstreaming into Firefox; relationship to wgpu / Vello / Buiy's Bevy render-graph path; brief note on Servo's text stack

# WebRender: GPU rendering by display list

WebRender is the GPU renderer born in Servo and now shipping in Firefox. Its premise is the one Buiy's render layer also assumes: **don't rasterize a tree of layers on the CPU and composite them — feed the GPU a flat, retained description of what to draw, and let the GPU do the rasterization and compositing in as few passes as possible.** WebRender is the reference implementation that proved this works for full web content at 60fps, so it is the load-bearing prior art for any "layout writes a paint description, render reads it" design — which is exactly Buiy's contract (see [layout.md](layout.md) and the Phase 9 stacking design at [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)).

This file is render-focused. For the engine as a whole see [architecture.md](architecture.md); for the style system see [stylo.md](stylo.md); for box layout see [layout.md](layout.md).

## The pipeline: display list → scene → frame → batches

WebRender splits work into stages that map cleanly onto Buiy's "layout produces a handoff, render consumes it" split:

1. **Display list.** The content process (or, in Servo, layout) emits a *display list* — a flat, serialized list of primitives (rectangles, text runs, images, gradients, borders, box-shadows, clips, stacking contexts). It is *unrasterized*: no pixels yet, just "draw this primitive here with this clip and this transform." The Firefox 67 rollout post put it plainly: "the representation of the page in the compositor is no longer a set of rasterized layers, but now an unrasterized display list."
2. **Scene building.** WebRender ingests the display list into a **retained scene** — a persistent data structure it keeps across frames. Only the *changed* parts of the scene are rebuilt; an unchanged subtree is reused. This is the retained-mode half of the design.
3. **Frame building.** The **frame builder** walks the retained scene for a given viewport/scroll state and produces a *frame*: it resolves clips and transforms, culls off-screen primitives, and assigns each primitive to a render target.
4. **Batching.** The frame builder groups primitives that share GPU state (same shader, same texture/atlas, compatible blend) into **batches**, so a screen full of text, borders, and rectangles draws in a handful of instanced draw calls instead of thousands. Batching is where the "GPU like a game engine" framing earns its keep.
5. **Rasterization + compositing in one GPU step.** "The compositing and rasterization steps have been joined into a single GPU-powered rendering step." Primitives are drawn directly to the framebuffer (or to intermediate render targets for effects like blur/opacity), with anti-aliasing done in the shader.

The clean separation — content emits a display list, WebRender retains+batches+rasterizes — is what lets the renderer be a black box the layout side never reaches into.

## Why this is the model for Buiy's render handoff

Buiy's pipeline contract is **layout writes, render reads — render never recomputes stacking or paint order** ([layout.md](layout.md)). WebRender is the existence proof that this split scales:

- Buiy Phase 8 (landed) has sub-pass 6e write a private `ResolvedTransform` for render to consume — a transform matrix handed off, never re-derived by the renderer. That is WebRender's "transforms live on stacking contexts in the display list" pattern in miniature.
- Buiy Phase 9 (next) has sub-pass 6f compute a private `StackingContext { painters_z: Vec<Entity> }` giving render a **pre-sorted paint order**. WebRender's display list already encodes stacking contexts and z-order; the renderer paints in list order and never re-sorts. Buiy is reproducing exactly that boundary: the painter's-algorithm order is decided in layout, serialized, and the GPU layer just consumes it.
- WebRender's *retained scene* (rebuild only changed subtrees) is the same incrementality Buiy gets for free from Bevy's ECS change-detection: only entities whose components changed get re-processed.

The difference: WebRender owns its whole GPU stack down to the OpenGL/`glow` calls. Buiy does **not** own a renderer — it emits draw data into **Bevy's render graph over wgpu** (foundation [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)). So WebRender is a *design* reference for the display-list/scene/batch shape, not a dependency or an architecture to copy wholesale.

## Hit-test order is paint order, reversed

Once Buiy's sub-pass 6f produces a paint order (`StackingContext { painters_z }`), the natural next question for Buiy's eventual interaction layer is whether hit-testing can reuse that order. Servo settles it: **hit-test order is the exact reverse of paint order.** Servo's `display_list/stacking_context.rs` builds paint front-to-back-into-the-list (back-most first), and its `build_display_list` carries a comment that it is "the forward version of the reversed stacking context walk algorithm in `hit_test.rs`" — i.e. hit-testing walks the *same* stacking-context tree in the *reverse* order, so the topmost-painted thing is the first hit candidate. WebRender retains a separate out-of-band hit-test data structure built from the same display list and tested against the retained scene (also handling scroll/clip), but the *ordering relationship* is the clean rule: paint = back→front, hit-test = front→back over the same structure. For Buiy this means `painters_z` is the single source of truth for both: render walks it forward; an interaction pass walks it (or its per-context slices) in reverse and returns the first entity whose bounds contain the point. Buiy does not need a second sorted structure — hit-testing is the reverse iteration of the order 6f already computes. (Scroll-clip and `pointer-events` masking are separate concerns layered on top, exactly as WebRender keeps clip/scroll data beside the paint primitives.)

## How WebRender upstreamed into Firefox

WebRender is the second large Servo-to-Firefox technology transfer (Stylo was the first — see [history.md](history.md)). Worth being precise, because the story is often overstated:

- WebRender shipped to **release** users first in **Firefox 67 (2019-05)**, but only to roughly **4% of the desktop base**: Windows 10 machines with NVIDIA GPUs. It was disabled at the 67 launch (2019-05-21), then ramped 25% → 50% → 100% of *qualified* users over the following two weeks. Broader hardware (AMD on Windows in 68, then Intel, macOS, Linux, Android) followed release-by-release; it was not a single "Firefox 67 turned it on for everyone" event. WebRender reached the **entire release population around Firefox 92 (2021)**, with the software fallback (`swgl`) covering hardware where the GPU path was untrusted.
- The canonical source of WebRender today is **`gfx/wr` inside `mozilla-central`** (Mozilla's Mercurial monorepo). The `github.com/servo/webrender` repo is a **downstream mirror** with a periodic one-way sync from `mozilla-central`; the published `webrender` crate (v0.68.0 on crates.io) trails that. So the upstreaming was thorough enough that the *primary development moved into Firefox's tree*. Inside Firefox the project was also branded "Quantum Render."

The lesson for Buiy's prior-art lens (see [lessons.md](lessons.md)): a renderer designed around a serialized display list and a retained scene is portable across very different host integrations (Servo's own GL stack vs. Firefox's compositor). Buiy's equivalent portability target is the Bevy render graph — the draw-data handoff should be defined so it doesn't assume one specific wgpu pipeline layout.

## The known costs (honest tone)

WebRender was not free wins, and the trade-offs are instructive:

- **Driver and hardware sensitivity.** A GPU-everything pipeline exposes the long tail of buggy/old GL drivers. Mozilla's gradual, hardware-gated rollout (NVIDIA Windows 10 first) existed precisely because WebRender behaved differently across vendors. Mozilla eventually built a **software fallback** (`swgl` / "software WebRender") to run the same pipeline on the CPU where the GPU path was untrusted — a reminder that "just use the GPU" needs a fallback for production breadth. Buiy inherits wgpu/Bevy's backend portability, but the same "what happens on a bad driver" question applies.
- **GPU memory for atlases and intermediate targets.** Batching trades draw calls for texture-atlas and render-target memory (glyph atlases, blur/opacity intermediate targets). On low-VRAM hardware this is a real ceiling. Buiy's glyph-atlas design must budget for this directly.
- **Effects force off-screen passes.** Anything that can't be done in a single shader pass — `opacity < 1` over a group, `filter`, `mix-blend-mode`, clip to a non-rectangular path — forces an intermediate render target, breaking batching. This is the *same* set of triggers Buiy's Phase 9 uses to *form a stacking context* ([../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)): positioned + z-index, `isolation`, non-identity transform, `contain:paint`, `opacity`/`filter`/`mix-blend-mode`. The convergence is not a coincidence — stacking-context boundaries *are* the GPU's natural render-pass boundaries. Buiy deciding paint order and group boundaries in layout (sub-pass 6f) hands render exactly the grouping it needs to manage off-screen passes.

## Relationship to wgpu and Vello

- **wgpu.** WebRender predates wgpu and targets OpenGL ES 3 / WebGL via `glow`, not wgpu. Servo *also* ships a separate WebGPU implementation backed by wgpu, but that is the web-facing `GPUDevice` API, not WebRender's own backend. Buiy, by contrast, renders through wgpu (via Bevy) as its *only* GPU path. So WebRender and Buiy converge on "GPU-accelerated display list" but diverge on the GPU abstraction.
- **Vello** ([../xilem-masonry/text-and-rendering.md](../xilem-masonry/text-and-rendering.md)). Vello and WebRender solve the same problem — GPU 2D rasterization of a retained scene — with different strategies. WebRender is **batch/instanced-draw-call** based with per-primitive shaders; Vello is **compute-shader** based (a sort-middle pipeline that bins and rasterizes paths in compute). Both demonstrate the capability set Buiy's wgpu pipeline targets (anti-aliased fill, gradients, clips, blends) without a CPU triangulator. Buiy treats both as *feasibility witnesses*, not dependencies: Buiy ships its own wgpu render-graph node and does not adopt Vello or WebRender as a crate.
- **Blitz** ([../dioxus/](../dioxus/)). DioxusLabs' Blitz is the closest thing to "Buiy's substrate minus Bevy/ECS": it combines **Stylo (CSS) + Taffy (box layout) + Parley (text) + Vello (GPU render, via `blitz-renderer-vello`)** to render HTML/CSS. Note the correction to the common shorthand "Stylo + Taffy + Vello": Blitz's text layout is **Parley**, with Vello as the rasterizer beneath it. Blitz is the practical demonstration that Stylo and Taffy compose under a Vello-class renderer — the same composition Buiy makes, except Buiy adds Bevy's ECS and render graph and stays on cosmic-text rather than Parley.

## Brief note on Servo's text stack (render-relevant only)

Render needs glyphs rasterized into something WebRender can batch. Servo's text path:

- **Shaping:** Servo uses **Rust bindings to HarfBuzz** (the C++ shaper). This is a divergence from the Linebender/cosmic-text world, which is moving to `harfrust` (the pure-Rust HarfBuzz port). Servo also authored the **`unicode-bidi`** crate (UAX #9 bidi) that the broader Rust ecosystem reuses.
- **Glyph delivery to the GPU:** shaped glyphs become text-run primitives in the display list; WebRender rasterizes glyphs into a texture atlas and batches the resulting quads. This atlas-and-batch model is exactly what Buiy's wgpu pipeline must reimplement for cosmic-text output (foundation text note [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)).

Buiy's text substrate is **cosmic-text**, not Servo's HarfBuzz-bindings stack — see [../cosmic-text/](../cosmic-text/) and the Linebender comparison in [../xilem-masonry/](../xilem-masonry/). The render-side takeaway from Servo is the *glyph-atlas + batched-quad* delivery, which is shaper-agnostic.

## License divergence (call-out)

Servo, Stylo, and WebRender are **MPL-2.0** (Mozilla Public License 2.0). Buiy is **MIT OR Apache-2.0**. MPL-2.0 is a file-level copyleft: it is compatible with linking into a permissively-licensed binary, but any *modified MPL files* must stay MPL and be disclosed. This means WebRender's code is a **reading reference**, not a vendoring source for Buiy — lifting WebRender source into Buiy would pull MPL obligations into an otherwise MIT/Apache tree. (Vello and wgpu are Apache-2.0/MIT, which is why they are the practical building blocks.) See [governance.md](governance.md) for the full licensing story.

## Sources

- WebRender MVP ships in Firefox 67: https://mozillagfx.wordpress.com/2019/05/21/graphics-team-ships-webrender-mvp/
- Firefox 67 WebRender rollout coverage: https://www.neowin.net/news/mozilla-will-begin-to-roll-out-webrender-to-firefox-users-with-version-67/
- WebRender repo (downstream mirror) + README (`mozilla-central/gfx/wr` is canonical): https://github.com/servo/webrender and https://github.com/servo/webrender/blob/main/README.md
- WebRender in Firefox / Quantum Render: https://wiki.mozilla.org/Platform/GFX/Quantum_Render
- Software WebRender (`swgl`) CPU fallback + Firefox 92 full rollout: https://bugzilla.mozilla.org/show_bug.cgi?id=1689203 and http://wiki.mozilla.org/Platform/GFX/WebRender_Where
- `webrender` crate (v0.68.0): https://crates.io/crates/webrender
- Servo WebGPU (wgpu-backed) context: https://book.servo.org/architecture/
- Hit-test order = reversed paint order (`build_display_list` comment referencing `hit_test.rs`): https://github.com/servo/servo/blob/main/components/layout/display_list/stacking_context.rs ; https://github.com/servo/servo/blob/main/components/layout/display_list/hit_test.rs
- Vello (compute-shader 2D renderer): https://github.com/linebender/vello
- Blitz (Stylo + Taffy + Parley + Vello): https://github.com/DioxusLabs/blitz and https://github.com/DioxusLabs/blitz/blob/main/README.md
- Servo text stack / HarfBuzz Rust bindings, `unicode-bidi`: https://book.servo.org/architecture/ and State of Text Rendering 2024 https://behdad.org/text2024/
- Servo project license (MPL-2.0): https://github.com/servo/servo
- Buiy stacking + top-layer spec: [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
- Buiy foundation README: [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior-art: [architecture.md](architecture.md), [stylo.md](stylo.md), [layout.md](layout.md), [history.md](history.md), [governance.md](governance.md), [lessons.md](lessons.md); [../xilem-masonry/](../xilem-masonry/), [../dioxus/](../dioxus/), [../cosmic-text/](../cosmic-text/)
