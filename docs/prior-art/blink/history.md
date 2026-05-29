**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — chronological timeline from the 2013 WebKit fork through LayoutNG, RenderingNG, CSS Containment / content-visibility, container queries, the Popover API, and CSS anchor positioning, with the Chrome version and ship date for each

# Blink history

A dated timeline of the Blink milestones that matter to Buiy's layout work. The
*how* of each item lives in the sibling files — [architecture.md](architecture.md),
[layout.md](layout.md), [stacking-and-paint.md](stacking-and-paint.md),
[containment-and-queries.md](containment-and-queries.md), [style.md](style.md) —
and the *who/how-it-ships* lives in [governance.md](governance.md). Every Chrome
version and date below was verified against chromestatus.com /
developer.chrome.com / web.dev / Wikipedia (see Sources).

## Timeline

| When | Milestone | Notes |
|---|---|---|
| **2013-04-03** | **WebKit fork → Blink announced** | Fork of WebKit `WebCore`; see [governance.md](governance.md). |
| **2018–2019** | RenderingNG property trees ship | "Property trees part 1" (2018), "part 2" (2019) — prerequisite for CompositeAfterPaint. |
| **2019 (Chrome 77)** | **LayoutNG** ships block + inline | First slice: block containers, inline, floats, out-of-flow positioning. No flex/grid/table/fragmentation yet. |
| **2020-01-15 (Edge 79)** | Edge adopts Chromium | See [governance.md](governance.md). |
| **2020 (Chrome 85, stable 2020-08-25)** | **`content-visibility`** ships | Render-skipping for off-screen subtrees. (web.dev article 2020-08-05.) |
| **~2020–2021** | RenderingNG documented | developer.chrome.com series; doc last-updated 2021-06-22. |
| **2022 (Chrome 103, 2022-06-21)** | Flex + grid block-fragmentation | LayoutNG fragmentation extended to flex and grid. |
| **2022 (Chrome 105, 2022-08-30)** | **Container queries** + `:has()` | Size container queries and container units. |
| **2022 (Chrome 106)** | Table block-fragmentation | LayoutNG table fragmentation. |
| **2023 (Chrome 114, 2023-05-31)** | **Popover API** ships | Declarative top-layer popovers, light dismiss, ESC handling. |
| **2024-05 (Chrome 125)** | **CSS anchor positioning** ships | Tether absolutely-positioned elements to anchors, no JS. |

## LayoutNG (2019, Chrome 77)

LayoutNG is the rewrite of Chromium's layout engine around an **immutable
fragment tree**: each box produces an `NGPhysicalFragment`, parents collect child
fragments, and painting and hit-testing traverse the *fragment* tree rather than
the `LayoutObject` tree (verified against [LayoutNG | Chrome for Developers](https://developer.chrome.com/docs/chromium/layoutng)
and [chromium.org/blink/layoutng](https://www.chromium.org/blink/layoutng/)). The
first shipment in **Chrome 77 (2019)** covered block container layout, inline
layout, floats, and out-of-flow positioning — but **not** flex, grid, tables, or
block fragmentation, which fell back to legacy layout. Flex and grid
fragmentation shipped in **Chrome 103**, table fragmentation in **Chrome 106**
(per the [block-fragmentation deep-dive](https://developer.chrome.com/docs/chromium/renderingng-fragmentation)).

*Hedge / correction.* The preamble's "flex ~Chrome 87, grid later" could not be
pinned to an exact ship version from public release notes during verification —
the documented public dates concern *fragmentation* support (Chrome 103/106), not
the initial NG flex/grid algorithm enable. Treat the precise per-primitive
NG-enable versions as unverified; what *is* verified is the phased order (block +
inline first in 77, the rest "in subsequent releases," fragmentation in 103/106).
Buiy does not need the exact versions — it relies on Taffy for layout — but the
*pattern* (ship the engine in slices, fall back to legacy per primitive until each
slice lands) is the relevant lesson.

This fragment-tree-as-render-handoff is the closest Blink analogue to Buiy's
layout/render contract: layout produces an immutable artifact and render reads it
without recomputing. Buiy's pipeline (`RemovedNodesGc → WritingModeInherit →
SyncStyles → CqActivate → TaffyCompute → CqFlipCheck → CqFlipReRun →
PostTaffyOverrides → WriteResolvedLayout`) writes resolved layout once; render
never recomputes stacking or paint order. See [layout.md](layout.md) and the Buiy
[`architecture.md`](../../specs/2026-05-08-buiy-layout-design/architecture.md) spec.

## RenderingNG (the pipeline, ~2021 documentation)

RenderingNG is the umbrella name for the modern Chromium rendering architecture,
documented on developer.chrome.com around 2021 (the overview doc is last-updated
2021-06-22). The **document lifecycle stages**, verified against the
[RenderingNG architecture doc](https://developer.chrome.com/docs/chromium/renderingng-architecture),
are: **animate → style → layout → pre-paint → scroll → paint → commit → layerize
→ raster/decode/paint-worklets → activate → aggregate → draw**. The pre-paint
stage computes the four **property trees** (transform, clip, effect, scroll) that
let the compositor thread (`cc`) re-composite without re-running layout — the
mechanism behind smooth scrolling and compositor-driven animation. See
[architecture.md](architecture.md) and [stacking-and-paint.md](stacking-and-paint.md).

## CSS Containment + `content-visibility` (Chrome 85, 2020)

`content-visibility: auto` lets the engine skip rendering work for off-screen
subtrees until they scroll into view (verified via the [Chrome 85 release notes](https://github.com/GoogleChrome/developer.chrome.com/blob/main/site/en/blog/new-in-chrome-85/index.md)
and [web.dev content-visibility](https://web.dev/articles/content-visibility)).
It builds on the `contain` property (layout / paint / size / style containment).
Buiy implements `contain` flags plus a `content-visibility` **stub** and
`will-change` as **stored-only** — landed in Phase 8 alongside transforms. See
[containment-and-queries.md](containment-and-queries.md) and the Buiy
[`transforms-and-containment.md`](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md)
spec.

## Container queries (Chrome 105, 2022-08-30)

One of the longest-requested CSS features. **Chrome 105 shipped 2022-08-30** with
size container queries and container units (`cqw`, `cqh`, etc.), landing
alongside `:has()` (verified against [@container and :has() landing in Chromium 105](https://developer.chrome.com/en/blog/has-with-cq-m105/)).
Container queries require a *layout-then-restyle* dependency that ordinary CSS
avoids — an element's matched rules depend on its container's resolved size. Buiy
models this with explicit pipeline passes: `CqActivate` before `TaffyCompute`,
then `CqFlipCheck` / `CqFlipReRun` after, so a query-driven style flip can force a
re-layout deterministically. See [containment-and-queries.md](containment-and-queries.md)
and the Buiy [`container-queries-and-writing-modes.md`](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md)
spec.

## Popover API (Chrome 114, 2023)

The Popover API (`popover` attribute / `popovertarget`) shipped in **Chromium
114 (2023)** as a declarative way to promote elements to the **top layer**, with
light-dismiss, accessible keyboard bindings, and ESC-to-close (verified via
[Introducing the popover API](https://developer.chrome.com/blog/introducing-popover-api)).
The top layer is the same render layer used by `dialog.showModal()` and
Fullscreen — content that escapes ancestor `overflow` clipping and paints above
everything else. Buiy's Phase 9 (NEXT, not yet built) introduces a `TopLayer`
enum (`None | Modal | Popover | Tooltip | Fullscreen`) ordered Fullscreen <
Tooltip < Popover < Modal, with a `TopLayerActivation` `VecDeque` resource and
per-window top layer. See [stacking-and-paint.md](stacking-and-paint.md) and the
Buiy [`stacking-and-top-layer.md`](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
spec.

## CSS anchor positioning (Chrome 125, May 2024)

Anchor positioning shipped in **Chrome 125, stable in May 2024** (verified via
[The CSS anchor positioning API](https://developer.chrome.com/docs/css-ui/anchor-positioning-api)
and [web.dev May 2024 platform notes](https://web.dev/blog/web-platform-05-2024)).
It lets an absolutely-positioned element tether to one or more anchor elements
declaratively, pairing naturally with top-layer popovers. As of writing it remains
Chromium-led — other engines were still catching up — a concrete example of the
monoculture dynamic discussed in [governance.md](governance.md). Buiy adds anchor
positioning as sub-pass **6d** of `PostTaffyOverrides`, *above* Taffy. See the Buiy
[`display-and-positioning.md`](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md)
spec.

## Cross-links

- How the pipeline and property trees work: [architecture.md](architecture.md).
- Fragment tree and layout passes: [layout.md](layout.md).
- Stacking contexts and the top layer: [stacking-and-paint.md](stacking-and-paint.md).
- Containment and container queries: [containment-and-queries.md](containment-and-queries.md).
- `ComputedStyle` and the megastruct critique: [style.md](style.md).
- Fork, stewardship, launch process, license: [governance.md](governance.md).

## Sources

- https://techcrunch.com/2013/04/03/google-forks-webkit-and-launches-blink-its-own-rendering-engine-that-will-soon-power-chrome-and-chromeos/
- https://en.wikipedia.org/wiki/Blink_(browser_engine)
- https://developer.chrome.com/docs/chromium/layoutng
- https://www.chromium.org/blink/layoutng/
- https://developer.chrome.com/docs/chromium/renderingng-fragmentation
- https://developer.chrome.com/docs/chromium/renderingng-architecture
- https://github.com/GoogleChrome/developer.chrome.com/blob/main/site/en/blog/new-in-chrome-85/index.md
- https://web.dev/articles/content-visibility
- https://developer.chrome.com/en/blog/has-with-cq-m105/
- https://developer.chrome.com/blog/introducing-popover-api
- https://developer.chrome.com/docs/css-ui/anchor-positioning-api
- https://web.dev/blog/web-platform-05-2024
- [layout.md](layout.md)
- [architecture.md](architecture.md)
- [stacking-and-paint.md](stacking-and-paint.md)
- [containment-and-queries.md](containment-and-queries.md)
- [governance.md](governance.md)
- [../../specs/2026-05-08-buiy-layout-design/architecture.md](../../specs/2026-05-08-buiy-layout-design/architecture.md)
- [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
- [../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md)
- [../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md)
- [../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md)
