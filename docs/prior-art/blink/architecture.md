**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — RenderingNG architecture, the document lifecycle, the Blink/cc split, and the "render reads finished data" separation

# Blink (Chromium) — architecture

Blink is the rendering engine of the Chromium browser project: the code path from a parsed DOM to pixels on the screen. It is the canonical implementation of the CSS specifications Buiy implements a typed-Rust subset of — Display 3, Positioned Layout, Containment 3, Writing Modes 4, Anchor Positioning 1 — so its architecture is load-bearing prior art even though Buiy is a retained-mode Bevy UI library, not a browser. This file covers the *shape* of the engine: the modern **RenderingNG** rendering architecture, the document-lifecycle stages, the split between Blink and the compositor (`cc`), the high-level threading model, and the design principle most directly relevant to Buiy's layout pipeline — that each pipeline stage reads the **finished, immutable** output of the prior stage and never reaches back to mutate it.

Layout internals are in [layout.md](layout.md); stacking and paint order in [stacking-and-paint.md](stacking-and-paint.md); containment and queries in [containment-and-queries.md](containment-and-queries.md); the `ComputedStyle` "megastruct" critique in [style.md](style.md). Governance and history are in [governance.md](governance.md) / [history.md](history.md); the unflattering view in [critiques.md](critiques.md) / [open-problems.md](open-problems.md).

## 1. Origin and stewardship

Blink was announced 2013-04-03 as a fork of WebKit's WebCore, by Google, in the open-source Chromium project. The stated motivation was that Chromium's multi-process architecture and WebKit's support for many divergent embedders had created mutual complexity that slowed both projects. Blink is now shared by every major Chromium-based browser: Microsoft Edge (since Edge 79 shipped stable on 2020-01-15, replacing the EdgeHTML engine), Brave, Opera, Vivaldi, and Samsung Internet. The project as a whole is governed by a top-level **BSD-3-Clause** `LICENSE` with Google LLC as the named copyright holder; because Blink descends from WebKit, individual WebKit-inherited files still carry their own LGPL-2.1 / BSD / MIT / MPL per-file headers (see [governance.md](governance.md) § License).

The practical consequence for prior-art purposes: when a CSS module says "user agents must…", Blink is the implementation most authors test against, and its bugs and deviations become de-facto interop reality. Servo/Stylo (the Rust reference implementation — see [../servo-stylo/](../servo-stylo/)) is the other half of that picture.

## 2. RenderingNG and the document lifecycle

**RenderingNG** is the name Google gives to the modern rendering architecture documented on `developer.chrome.com` (~2021), the result of a multi-year rewrite (BlinkNG, LayoutNG, CompositeAfterPaint, the property-tree work). It is not a separate engine — it is the cleaned-up shape of Blink + `cc` + Viz after those projects landed.

The **document lifecycle** is the ordered set of stages that turn a mutated DOM into a composited frame. Per the RenderingNG architecture article, the stages are:

1. **Animate** — change computed styles and mutate property trees over time.
2. **Style** — apply CSS to the DOM, producing `ComputedStyle` per element.
3. **Layout** — determine size and position; LayoutNG produces the immutable fragment tree.
4. **Pre-paint** — compute the property trees (transform / clip / effect / scroll) and invalidate stale display lists.
5. **Scroll** — update scroll offsets (can run without re-running layout/paint).
6. **Paint** — compute a *display list* describing how to raster GPU texture tiles.
7. **Commit** — copy property trees and the display list to the compositor thread.
8. **Layerize** — break the display list into a composited layer list.
9. **Raster, decode, and paint worklets** — turn display lists, encoded images, and worklet code into GPU texture tiles.
10. **Activate** — build a *compositor frame* describing how to draw and position those tiles.
11. **Aggregate** — combine compositor frames from all visible surfaces (in the Viz process).
12. **Draw** — execute the aggregated frame on the GPU to produce pixels.

The pre-amble's stage list (animate, style, layout, pre-paint, paint, commit, tiling, raster/decode, activate, draw) is accurate; the official article spells "tiling" as **layerize**, and inserts **scroll** and **aggregate** as named stages. Crucially, **animations of visual effects and scrolling can skip layout, pre-paint, and paint** — they mutate property trees and re-run only commit-onward, often entirely on the compositor thread without waking the main thread. This is the architectural payoff of the property-tree split.

## 3. The Blink / cc split

Blink and the compositor (`cc`, the "Chromium compositor") are two halves of one render process:

- **Blink** owns the *content*: DOM, style resolution, layout (LayoutNG), pre-paint property-tree generation, and paint (display-list recording). Its job ends at producing two immutable artifacts — the **property trees** and the **display list**.
- **`cc`** owns *compositing*: it takes Blink's committed property trees + display list, breaks them into layers, rasters tiles (on worker threads), and assembles a compositor frame. `cc` can independently re-run scroll and compositor-driven animations by mutating its *copy* of the property trees, without asking Blink to redo anything.

The **commit** stage is the hand-off seam: Blink's finished output is *copied* to `cc`'s thread so the two can run concurrently on the next frame. The frame `cc` produces is then submitted to the **Viz** display compositor (a separate GPU process) which aggregates frames from every visible surface and draws.

This is the same shape as Buiy's substrate boundary, one layer up: in Buiy, *layout writes, render reads* (the [transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md) "render-handoff" contract), and rendering goes through `wgpu` via Bevy's render graph rather than `cc` + Viz. Blink's split is the precedent for cleanly separating "compute the scene" from "draw the scene" so the draw side can run on its own schedule.

## 4. Threading model (high level)

Three thread roles matter at the architectural level:

- **Main thread** (one per render process) — runs JavaScript, the rendering event loop, parsing, style, layout, pre-paint, paint, and hit testing. This is where the document lifecycle steps 1–7 run.
- **Compositor thread** (one per render process) — runs `cc`. Processes input, performs scrolling and compositor-driven animations, and runs layerize/commit-side work. By living off the main thread it keeps scroll and transform/opacity animations smooth even when the main thread is busy in JavaScript or layout.
- **Raster / worker threads** — a pool that turns the layer list into GPU tiles (raster) and decodes images. Off the critical path of both other threads.

The design intent, per the RenderingNG docs, is **pipelined parallelization**: stages for successive frames overlap across threads rather than running strictly one-after-another. The point Buiy should take is not the thread count (Buiy is a Bevy ECS schedule, not a multi-process browser) but the *discipline that enables threading*: a stage may only run off-thread if its inputs are immutable and its outputs are a clean copy. Buiy's `LayoutTree` is a `!Send` `NonSendResource` and layout is a sequential pass (see [../taffy/](../taffy/) architecture § 8), so Buiy does not parallelize layout — but it does enforce the same write/read split that lets the render side consume layout's output without coordination.

## 5. "Render reads finished data" — the BlinkNG separation principle

BlinkNG (the rewrite that produced RenderingNG's clean phase boundaries) is built on six stated principles, the most relevant being:

- **Uniform point of entry** — always enter at the pipeline's start; "the reentrant code paths have been removed… it is no longer possible to enter the pipeline starting at an intermediate phase."
- **Functional stages** — each stage is a deterministic function of its inputs.
- **Constant inputs** — a stage's inputs do not change while it runs.
- **Immutable outputs** — "once a stage has finished, its outputs should be immutable for the remainder of the rendering update."
- **Checkpoint consistency** and **deduplication of work**.

LayoutNG embodies this directly: layout's primary output is the **immutable, read-only fragment tree**, which is "the primary input to subsequent rendering phases." Pre-paint, paint, and compositing read that fragment tree; they never mutate it, and "accessing the previous state isn't allowed," which the docs note prevents hysteresis bugs (where this frame's result depends on a stale prior result). The legacy engine, by contrast, had a single mutable render tree that every stage poked at, with re-entrancy and order-dependent bugs.

This is the single most transferable idea for Buiy. Buiy's layout pipeline is an ordered sequence — `RemovedNodesGc, WritingModeInherit, SyncStyles, CqActivate, TaffyCompute, CqFlipCheck, CqFlipReRun, PostTaffyOverrides, WriteResolvedLayout` — and `PostTaffyOverrides` chains sub-passes 6a sticky → 6b table → 6c multicol → 6d anchor → 6e transform-composition → 6f stacking+top-layer (Phase 9, next). The Phase 8 work that just landed writes a **private** `ResolvedTransform` render-handoff; Phase 9 will compute a **private** `StackingContext { painters_z: Vec<Entity> }` that hands the renderer a *pre-sorted* paint order. In both cases the rule is Blink's rule: the renderer **reads** a finished, owned artifact and **never recomputes** stacking or paint order itself. See [stacking-and-paint.md](stacking-and-paint.md) and Buiy's [stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md).

## 6. Implications for Buiy

- **Adopt the immutable-output discipline, not the thread topology.** Buiy can't (and shouldn't) replicate Blink's main/compositor/raster split — it has one ECS schedule. What it should copy is the contract that each pipeline stage produces a self-consistent, immutable artifact the next stage only reads. Buiy already does this with `ResolvedTransform` and plans to with `StackingContext.painters_z`. The lesson Blink paid for in years of rewrite is: do not let the render side reach back into layout.
- **The "skip layout for transform/opacity animation" optimization is the property-tree dividend.** Blink can animate transform and opacity without re-running layout because those live in property trees, not in the box geometry. Buiy's transform composition (`M = T·R·S·M_transform`) and its containment work already separate transform from layout geometry, which is the same separation that would later let a Buiy animation system mutate `ResolvedTransform` without re-running Taffy. Keep that boundary clean.
- **The fragment-tree / `painters_z` analogy is exact and worth naming.** Blink's fragment tree is the canonical example of "layout's output is a read-only thing the rest of the engine consumes." Buiy's per-entity resolved-layout writes plus the private stacking order are the same idea at a coarser grain. Cite Blink as the precedent in the Phase 9 spec.
- **Where Buiy diverges deliberately:** Blink carries a single `ComputedStyle` megastruct (see [style.md](style.md)); Buiy uses decomposed public-fielded components with no megacomponent. Blink layers stacking/containment/transform *inside* a monolithic layout+paint engine; Buiy layers them as passes *above* Taffy, never forking Taffy ([transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md)). The architecture is borrowed; the data model is not.

## Sources

- Blink launch announcement (2013-04-03) — https://blog.chromium.org/2013/04/blink-rendering-engine-for-chromium.html
- "What is Blink?" — https://developer.chrome.com/docs/web-platform/blink
- RenderingNG architecture (lifecycle stages, threading, Blink/cc split) — https://developer.chrome.com/docs/chromium/renderingng-architecture
- RenderingNG deep-dive: BlinkNG (the six principles, immutable outputs, no re-entrancy, fragment tree) — https://developer.chrome.com/docs/chromium/blinkng
- RenderingNG deep-dive: LayoutNG (immutable fragment tree) — https://developer.chrome.com/docs/chromium/layoutng
- LayoutNG (Chromium project page; block/inline shipped Chrome 77, 2019) — https://www.chromium.org/blink/layoutng/
- Edge 79 Chromium stable release (2020-01-15) — https://blogs.windows.com/msedgedev/2020/01/15/upgrading-new-microsoft-edge-79-chromium/
- Chromium `LICENSE` (top-level BSD-3-Clause, Google copyright holder; WebKit-inherited LGPL/MIT/MPL per-file headers) — https://chromium.googlesource.com/chromium/src/+/main/LICENSE
- Buiy stacking + top-layer design (Phase 9) — [`docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md`](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
- Buiy transforms + containment design (Phase 8, render-handoff contract) — [`docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md`](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md)
- Buiy layout architecture (pipeline order, layout-writes/render-reads) — [`docs/specs/2026-05-08-buiy-layout-design/architecture.md`](../../specs/2026-05-08-buiy-layout-design/architecture.md)
- Buiy foundation overview — [`docs/specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior art: [layout.md](layout.md), [stacking-and-paint.md](stacking-and-paint.md), [containment-and-queries.md](containment-and-queries.md), [style.md](style.md), [governance.md](governance.md), [history.md](history.md), [critiques.md](critiques.md), [open-problems.md](open-problems.md), [comparisons.md](comparisons.md); cross-engine: [../taffy/](../taffy/), [../servo-stylo/](../servo-stylo/) (the Rust reference implementation)
