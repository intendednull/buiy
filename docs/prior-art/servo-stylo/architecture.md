**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo — engine architecture: the multi-process / multi-thread model (constellation, script thread, layout, compositor, WebRender), the embedding surface (servoshell, `libservo`, Verso / Tauri), and the layout-2013 → current-layout rewrite at the architectural level.

# Servo architecture: process model, the style / layout / paint split, and the embedding surface

Servo is an experimental web engine written in Rust. Mozilla Research began it in **2012**, co-evolving it with the Rust language itself — Servo was effectively the first large non-compiler Rust codebase, used to stress-test whether Rust was viable for systems work. Mozilla **laid off the Servo team in August 2020** (part of a company-wide ~250-person cut). Stewardship passed to the Linux Foundation; after a near-dormant 2021–2022, **Igalia** revived day-to-day development in early 2023, and the project **joined Linux Foundation Europe in September 2023**, with Igalia as the funded primary maintainer/steward. Servo and Stylo are licensed **MPL-2.0** (Mozilla Public License 2.0) — a copyleft-per-file license that **diverges from Buiy's MIT OR Apache-2.0**; this matters for any direct code lift (see [governance.md](governance.md)).

This file is the architectural overview. Style internals live in [stylo.md](stylo.md), layout in [layout.md](layout.md), paint in [rendering.md](rendering.md).

## The split that matters: style / layout / paint are separate subsystems

Servo's defining architectural choice is that the three big jobs of a rendering engine are cleanly decomposed and were built so they could be extracted and reused independently:

- **Stylo** (style) — parses CSS, runs selector matching and the cascade, and produces computed styles. It is a **standalone crate** (`stylo`, repo `github.com/servo/stylo`, current `0.17.0`) that *both* Servo and Firefox depend on. Style does not know about layout boxes or pixels.
- **Layout** (box / fragment trees) — consumes computed styles plus the DOM and produces sized, positioned fragments, then a display list. Servo **owns** its block / inline / table / float algorithms but **delegates flexbox and CSS grid to Taffy** via `components/layout/taffy/` and the `stylo_taffy` adapter (see [layout.md](layout.md) §1). It is not "Servo vs Taffy" — Servo *embeds* Taffy for two of its formatting contexts.
- **WebRender** (paint) — consumes display lists and rasterizes on the GPU. It knows nothing about CSS or the DOM; it sees retained display items, builds a scene, batches, and draws.

The contract between them is one-directional and data-shaped: **style hands computed values to layout; layout hands a display list to WebRender; WebRender paints.** Paint never re-derives layout or stacking order — that is decided upstream and serialized into the display list. This is the same "layout writes, render reads" discipline Buiy commits to (see [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)): the render pass walks a pre-decided order, it does not recompute it.

## The multi-process / multi-thread model

Servo is structured around a small set of long-lived actors that communicate by message-passing over typed channels (originally IPC channels to enable multi-process; threads within a process otherwise).

- **Constellation** — the central coordinator. One per Servo instance. It owns the set of *pipelines* (one pipeline per frame: the top-level document and each `<iframe>`), brokers navigation, manages the lifecycle of content processes, and routes messages between script, layout, compositor, and the embedder. It is the only component with a global view; everything else is local to a pipeline.
- **Script thread** — runs in a *content process*. Hosts the DOM, runs JavaScript (via SpiderMonkey), services the event loop, and drives layout. A single script thread can own multiple pipelines (e.g. same-origin iframes). Script is where the document model lives.
- **Layout** — historically a separate *layout thread*; in the current engine it is invoked from the script thread's pipeline and may fan out to a **rayon** worker pool for parallel sub-trees. It builds the box tree, runs the layout algorithm to a fragment tree, and emits a display list. (Stylo, invoked just before layout, also parallelizes the cascade across rayon.)
- **Compositor / renderer** — runs in the main process. It receives display lists, hands them to **WebRender**, manages the WebRender scene, handles scrolling and hit-testing against the retained scene, and presents frames. In Servo's lexicon "compositor" and "renderer" are used near-interchangeably for this component.
- **WebRender** — the GPU rasterizer (display-list → retained scene → batched draw calls → framebuffer). Shared verbatim with Firefox. See [rendering.md](rendering.md).

The design intent was full **site isolation**: multiple content processes, each sandboxed, coordinated by one constellation. In practice multi-process has come and gone as a default depending on platform and era; the *architecture* assumes it, the *deployment* is configurable. The honest framing: the actor decomposition is real and load-bearing, but "N sandboxed content processes" has been more aspiration than always-on reality.

### Relevance to Buiy

Buiy is single-process and runs inside Bevy's ECS schedule, so the constellation / script-thread / IPC machinery has no analogue. What transfers is the **subsystem decomposition and the unidirectional contract**, not the process topology. Buiy's layout pipeline (`RemovedNodesGc → WritingModeInherit → SyncStyles → CqActivate → TaffyCompute → CqFlipCheck → CqFlipReRun → PostTaffyOverrides → WriteResolvedLayout`) is Servo's "style → layout → display list" pipeline expressed as ECS systems. Both engines sit Taffy under flex/grid; the difference is layering — Servo calls Taffy *inside* a formatting context (block/inline/table are its own), whereas Buiy runs Taffy as the *whole* `TaffyCompute` pass and stacks its own passes above it (see [layout.md](layout.md) §1). The rayon-parallel cascade in Stylo is the conceptual cousin of running these as Bevy systems; Buiy gets parallelism from the ECS scheduler rather than an explicit rayon pool.

## The layout-2013 → current-layout rewrite (architectural view)

Servo has carried two layout engines, named for when they began:

- **Layout 2013** (the original) stored boxes and fragments in a single **flow tree**: internal `Flow` nodes corresponded roughly to block / inline formatting contexts, leaves were fragments. Box state and fragment state were intermixed and mutable.
- **Layout 2020** (begun ~2019) deliberately **separated the box tree from the fragment tree**, as the CSS specs define them, and made fragment nodes atomically-reference-counted and mostly immutable. Layout runs in three phases: **box-tree construction → fragment-tree construction → display-list construction**. Fragments are the result of splitting box-tree elements by line-breaking, columns, and pagination — modelling the spec's notion of a fragment directly rather than approximating it.

This was an architectural correction, not a rewrite-for-its-own-sake: the flow tree conflated two concepts the CSS spec keeps distinct, which made fragmentation, multicol, and pagination structurally awkward. Status (verified May 2026): under Igalia, layout-2020 became the default, **legacy layout-2013 was fully removed in 2025** (PR #35943), and once there was only one engine the name "2020" stopped meaning anything — the `layout_2020` and `layout_thread_2020` crates were merged into a crate now simply called **`layout`** (PR #36613). So the dual-engine era is over; "Servo layout" today means the formatting-context / fragment-tree engine.

### Relevance to Buiy

The 2013→2020 lesson is the one Buiy already internalized: **model the spec's data structures (formatting contexts, a box tree distinct from a fragment/result tree), not a convenient approximation.** Buiy gets the box-vs-result separation for free because Taffy owns the box-layout pass and Buiy's `PostTaffyOverrides` sub-passes (6a sticky, 6d anchor, 6e transform-composition, 6f stacking) write a *separate* resolved-layout layer that render consumes — the same "don't mutate the layout inputs in place; produce an immutable result" discipline that drove Servo's immutable fragment tree. Servo's three phases map onto Buiy's "compute (Taffy) → override (post-passes) → write resolved layout → render reads."

## The embedding surface

Servo's stated 2023+ mission is to be an *embeddable* engine, not a standalone browser. The relevant pieces:

- **servoshell** — the reference shell / demo browser in the Servo repo. It is the dogfooding consumer of the embedding API; recent work ported it onto the new `WebView` API and removed the old `EmbedderEvent` path.
- **`libservo`** — the Rust embedding API: a `WebView` plus `WebViewDelegate` / `ServoDelegate` model where the embedder owns the window and event loop and Servo calls back through delegates. Still early-stage (described as pre-alpha through 2024–2025) but is the supported integration point. Offscreen rendering lets the embedder draw Servo into a framebuffer and present it however it likes, including translucent / transparent backgrounds and webviews positioned anywhere in a native window.
- **Verso** — a separate browser built on Servo by a Servo TSC member, used to exercise the features Servo needs to back a real browser (and which wrote its own compositor layer on top of Servo).
- **Tauri experiment** — an effort (NLnet-funded) to let Tauri use Servo as an alternative embeddable webview to the OS webview, prototyping embedding, offscreen rendering, and multiple webviews.

### Relevance to Buiy

Buiy is itself an *embedding* of a web-ish layout / paint stack into a host (Bevy), much as `libservo` embeds Servo into a host event loop. The delegate-callback shape — embedder owns the loop, engine calls back — is the inverse of Buiy, where Bevy owns the schedule and Buiy is a plugin slotted into it. The transferable observation is that a clean embedding API forces the engine to *not* assume it owns the window, the GPU surface, or the frame cadence — a constraint Buiy satisfies by living in Bevy's render graph (`wgpu` via Bevy) rather than owning a surface. The offscreen-rendering requirement (draw to a framebuffer, let the host present) is exactly Buiy's relationship to Bevy's render graph.

## Where this sits in the corpus

Servo is one of the two load-bearing *implementation* references for Buiy's CSS-faithful subset (the other being Blink — the canonical implementation). Servo is the Rust one, and its substrate (Stylo for style; and downstream, the **Blitz** project from DioxusLabs combining Stylo + Taffy + Parley + Vello — see [stylo.md](stylo.md) and [../dioxus/](../dioxus/)) is nearly Buiy's substrate minus Bevy/ECS. Compare Buiy's actual layout substrate in [../taffy/](../taffy/), its render host in [../bevy-ui/](../bevy-ui/), and the broader Rust-UI landscape in [../xilem-masonry/](../xilem-masonry/). For game-engine UI prior art with a similar paint / layout split see [../rmlui/](../rmlui/) and [../coherent-gameface/](../coherent-gameface/). Project context: [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md).

## Sources

- Servo origin (2012), Rust co-evolution, MPL-2.0: https://en.wikipedia.org/wiki/Servo_(software) ; https://github.com/servo/servo ; https://servo.org/about/
- Mozilla August 2020 layoffs (~250, Servo team): https://www.ghacks.net/2020/08/11/mozilla-lays-off-250-employees-in-massive-company-reorganization/
- Igalia revival (Jan 2023) + Linux Foundation Europe (Sept 2023): https://www.igalia.com/2023/09/07/The-Servo-project-is-joining-Linux-Foundation-Europe.html ; https://linuxfoundation.eu/newsroom/servo-web-rendering-engine-joins-linux-foundation-europe ; https://blogs.igalia.com/mrego/servo-revival-2023-2024/
- Architecture (constellation / script thread / pipeline / compositor / WebRender): https://book.servo.org/architecture/overview.html ; https://book.servo.org/design-documentation/architecture.html
- `stylo` standalone crate (v0.17.0, repo servo/stylo): https://crates.io/crates/stylo ; https://github.com/servo/stylo
- Layout 2013 vs 2020 (flow tree vs box / fragment trees, three phases): https://servo.org/blog/2023/04/13/layout-2013-vs-2020/ ; https://github.com/servo/servo/wiki/Layout-2020 ; https://book.servo.org/architecture/layout.html
- Legacy layout removal (2025) + crate rename to `layout`: https://github.com/servo/servo/pull/35943 ; https://github.com/servo/servo/pull/36613 ; https://github.com/servo/servo/pull/34994
- Servo delegates flexbox + CSS grid to Taffy (`components/layout/taffy/`, `stylo_taffy` adapter; CSS-grid WPT 18.6%→38.3% in PR #32619): https://github.com/servo/servo/tree/main/components/layout/taffy ; https://github.com/servo/servo/pull/32619 — see [layout.md](layout.md) §1
- Embedding surface (`libservo` WebView API, servoshell, offscreen rendering): https://servo.org/blog/2024/01/19/embedding-update/ ; https://servo.org/blog/2024/09/11/building-browser/ ; https://github.com/servo/servo/pull/35183 ; https://github.com/servo/servo/pull/35196
- Verso + Tauri experiments: https://wusyong.github.io/posts/verso-compositor-part1/ ; https://nlnet.nl/project/Tauri-Servo/
- Blitz (Stylo + Taffy + Parley + Vello): https://github.com/DioxusLabs/blitz
- Buiy specs: [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md) ; [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior-art: [stylo.md](stylo.md) ; [layout.md](layout.md) ; [rendering.md](rendering.md) ; [governance.md](governance.md) ; [../taffy/](../taffy/) ; [../bevy-ui/](../bevy-ui/) ; [../dioxus/](../dioxus/) ; [../xilem-masonry/](../xilem-masonry/) ; [../rmlui/](../rmlui/) ; [../coherent-gameface/](../coherent-gameface/)
