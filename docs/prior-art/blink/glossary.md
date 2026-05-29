**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium / RenderingNG) — system-specific terminology used across this folder

# Blink — glossary

Short definitions for Blink / RenderingNG / CSS-reference terms used across this prior-art folder. Each entry points to the file where the full discussion lives.

## Engine, architecture, pipeline

- **Blink** — Chromium's rendering engine: the code path from a parsed DOM to pixels. Forked from WebKit's `WebCore`, announced 2013-04-03. Lives at `third_party/blink/` in the Chromium tree. See [architecture.md](architecture.md), [governance.md](governance.md).
- **Chromium** — the open-source project Blink lives inside; Google-stewarded. Shared by Chrome, Edge, Brave, Opera, Vivaldi, Samsung Internet. See [governance.md](governance.md).
- **`cc` (Chromium compositor)** — the compositing half of the render process. Consumes Blink's committed property trees + display list, breaks them into layers, rasters tiles, assembles a compositor frame. Runs on the compositor thread. See [architecture.md § 3](architecture.md).
- **Viz** — the display compositor in a separate GPU process; aggregates compositor frames from every visible surface and draws. See [architecture.md § 3](architecture.md).
- **RenderingNG** — the umbrella name for the modern Chromium rendering architecture (~2021); the cleaned-up shape of Blink + `cc` + Viz after BlinkNG / LayoutNG / CompositeAfterPaint / property-tree work landed. Not a separate engine. See [architecture.md § 2](architecture.md), [history.md](history.md).
- **BlinkNG** — the rewrite that gave RenderingNG its clean phase boundaries; built on principles including *uniform point of entry*, *functional stages*, *constant inputs*, and *immutable outputs* ("once a stage has finished, its outputs should be immutable"). See [architecture.md § 5](architecture.md).
- **Document lifecycle** — the ordered set of stages turning a mutated DOM into a composited frame. The verified 12 stages: **animate, style, layout, pre-paint, scroll, paint, commit, layerize, raster/decode/paint-worklets, activate, aggregate, draw**. See [architecture.md § 2](architecture.md).
- **Layerize** — the lifecycle stage that breaks the display list into a composited layer list. (Earlier informal summaries called this "tiling.") See [architecture.md § 2](architecture.md).
- **Commit** — the lifecycle hand-off stage: Blink's finished property trees + display list are *copied* to `cc`'s thread so the two can run concurrently. See [architecture.md § 3](architecture.md).
- **Pre-paint** — the lifecycle stage that computes the four paint property trees and invalidates stale display lists. See [architecture.md § 2](architecture.md), [stacking-and-paint.md § 3](stacking-and-paint.md).
- **DocumentLifecycle** — the Blink class that enforces phase state (e.g. `ComputedStyle` may only be mutated in `kInStyleRecalc`; nothing dirty remains at `kStyleClean`). See [style.md § 1](style.md).
- **Slimming Paint / CompositeAfterPaint** — the multi-year project (v1 → v2 → CompositeAfterPaint, default ~2021) that moved visual effects off the layer tree into property trees. See [stacking-and-paint.md § 3](stacking-and-paint.md).

## Layout (LayoutNG)

- **LayoutNG** — Chromium's modern layout engine; replaced the legacy in-place `LayoutObject` engine. Block + inline shipped Chrome 77 (2019). See [layout.md](layout.md).
- **`LayoutObject`** (formerly `RenderObject`) — the legacy node type that held *both* input style-derived state and mutable output geometry and recomputed in place; the coupling LayoutNG removed. See [layout.md § 1](layout.md).
- **Layout input node (`LayoutInputNode` / `NGLayoutInputNode`)** — a thin read-only view over the `LayoutObject` tree + its `ComputedStyle`; the immutable *input identity* to a layout pass. Subclasses `BlockNode` / `InlineNode` dispatch to the algorithm for the box's formatting context. See [layout.md § 2](layout.md).
- **Constraint space (`ConstraintSpace` / `NGConstraintSpace`)** — the immutable input *parameters* to layout: available inline/block size, percentage-resolution sizes, fragmentation context, float exclusions, writing mode/direction, cache flags. See [layout.md § 2](layout.md).
- **Fragment tree / physical fragment (`PhysicalBoxFragment` / `NGPhysicalFragment`)** — LayoutNG's immutable *output*: the geometry a box produced for one constraint space. One input node can produce several fragments (when split across columns/pages), which is why the output is a separate tree. The primary input to paint, hit-test, and compositing. See [layout.md § 2](layout.md).
- **Cache key** — each fragment is stored alongside the `ConstraintSpace` that produced it; an equal constraint space on the next layout reuses the cached fragment without descending. The mechanism behind "predictably linear" O(n) layout. See [layout.md § 2](layout.md).
- **FlexNG / GridNG / TablesNG** — the per-formatting-context NG algorithm migrations (Flexbox, CSS Grid, tables). GridNG (Microsoft Edge team) fixed the Chrome 92→93 exponential-layout / hysteresis bugs; TablesNG (Chrome/Edge 93) fixed sticky table headers and closed 72 bugs. Exact NG-enable versions for flex/grid are not pinned in public sources. See [layout.md § 3](layout.md), [history.md](history.md).
- **`NGLineBreaker` (inline layout)** — the inline formatting-context algorithm: collect an items list (text runs, atomic inlines, open/close tags), shape via HarfBuzz, run a greedy line breaker honoring `white-space`, hyphenation, bidi, `vertical-align`, float intrusion. Output is line-box fragments. Buiy delegates the equivalent to `cosmic-text`. See [layout.md § 3](layout.md).
- **Block fragmentation** — splitting content across columns/pages/regions; the feature that forces the multi-fragment output tree. Landed last (block ~Chrome 102; flex/grid 103; table 106). See [layout.md § 3–4](layout.md).

## Style (ComputedStyle and the cascade)

- **`ComputedStyle`** — the fully-resolved, post-cascade value of every CSS property an element supports; one per element; read by layout, paint, hit-test, compositor input, and the a11y tree. Immutable after the Style phase (post-BlinkNG). The "megastruct" / "god object." See [style.md § 1–2](style.md).
- **`ComputedStyleBase`** — the generated base class of `ComputedStyle`, produced by `make_computed_style_base.py` from `css_properties.json5`; emits fields, getters, setters, diff, and packing. See [style.md § 2](style.md).
- **`css_properties.json5`** — the property database driving `ComputedStyleBase` generation; adding a property is editing JSON5, not the struct. See [style.md § 2](style.md).
- **Rare-data groups** (`StyleRareNonInheritedData`, `StyleRareInheritedData`, …) — heap-allocated, reference-counted, copy-on-write field groups for uncommon properties, so a common element pays only for common fields. The structural cost Buiy avoids via per-component presence/absence. See [style.md § 2](style.md).
- **Cascade** (`StyleResolver` / `StyleCascade`) — the algorithm producing `ComputedStyle`: collect applicable declarations, resolve by origin/importance, cascade layers (`@layer`), specificity, source order; resolve `var()`, `calc()`, `revert`, `!important`. Buiy has *no* cascade. See [style.md § 3](style.md).
- **Style recalc** — the per-frame work bringing `ComputedStyle` up to date after DOM/stylesheet changes; post-BlinkNG a bounded phase over dirty elements. See [style.md § 4](style.md).
- **Invalidation sets / `RuleFeatureSet`** — the mechanism mapping a class/attribute/state change to the minimal set of elements to recalc, without re-matching every selector. Deliberately *over-invalidates* ("err on the side of correctness"). Buiy's analogue is Bevy ECS `Changed<T>`. See [style.md § 5](style.md).
- **Registered custom property (`@property`)** — a custom property with a `syntax`, `inherits` flag, and optional `initial-value`; validated when computed, not when parsed; `inherits: false` narrows recalc scope. See [style.md § 6](style.md).

## Stacking, paint, top layer

- **Stacking context** — a subtree painted as a unit relative to siblings. Formed by a *union* of triggers (root; positioned + non-`auto` `z-index`; `opacity < 1`; `transform`/`filter`/etc.; `isolation: isolate`; `contain: paint`/`strict`; `will-change`; top-layer). See [stacking-and-paint.md § 1](stacking-and-paint.md).
- **`PaintLayer`** — the Blink object where stacking-context formation is decided (in `LayoutObject::StyleDidChange`). See [stacking-and-paint.md § 1](stacking-and-paint.md).
- **Paint order (CSS 2.2 Appendix E)** — the fixed seven-step order within a stacking context (own background/borders → negative-`z` SCs → in-flow non-positioned blocks → non-positioned floats → in-flow inlines → positioned `z: auto`/`0` → positive-`z` SCs); followed by `PaintLayerPainter`. Equal-`z-index` ties break by **tree order** (source-document order) — a stable-sort tiebreak Buiy must reconstruct since the ECS has no inherent document order. See [stacking-and-paint.md § 2, § 2.1](stacking-and-paint.md).
- **`isolation: isolate`** — forms a stacking context to isolate `mix-blend-mode` (descendants blend among themselves, not with content behind), with **no clip** side effect — unlike `contain: paint`, which forms an SC *and* clips. Both are SC triggers in the Phase 9 union; only the latter feeds the clip output. See [stacking-and-paint.md § 1.1](stacking-and-paint.md), [containment-and-queries.md § 5](containment-and-queries.md).
- **`will-change`** — a promotion hint; `will-change: transform` (or any value naming a would-be SC trigger) forms a stacking context **pre-emptively, even when the property is currently `none`/initial** — based on the property's *potential*, not its current value. Buiy stores it **stored-only** in Phase 8. See [stacking-and-paint.md § 1.1](stacking-and-paint.md).
- **Hit-testing** — pointer/input picking; traverses **reverse paint order** (topmost-painted is tested first), so the back-to-front read of `painters_z` is the hit-test order. The top layer intercepts input first. See [stacking-and-paint.md § 5](stacking-and-paint.md).
- **`pointer-events`** — controls hit-testing only (not paint, visibility, or stacking); `pointer-events: none` makes the browser skip an element and test the one underneath. Buiy consults a pickability flag during the back-to-front `painters_z` walk. See [stacking-and-paint.md § 5](stacking-and-paint.md), [../bevy-picking/](../bevy-picking/).
- **Paint property trees** — the four trees computed in pre-paint and consumed by `cc`: **transform** (transforms + scroll translations), **clip** (overflow clips, `clip-path`, viewport clip), **effect** (`opacity`, `filter`, `mix-blend-mode`, masks, isolation), **scroll** (scrollable areas + offsets). Each `LayoutObject` references one node in each ("property tree state"). See [stacking-and-paint.md § 3](stacking-and-paint.md).
- **Top layer** — a per-document parallel rendering layer that paints above everything and escapes ancestor `overflow` clipping and containing-block stacking. Populated by `dialog.showModal()` (+ `::backdrop`), the Popover API, and Fullscreen; maintained as an ordered list (most-recent paints on top). See [stacking-and-paint.md § 4](stacking-and-paint.md).
- **Popover API** — the `popover` attribute / `popovertarget` declarative way to promote an element to the top layer, with light-dismiss and ESC handling; enabled by default Chrome 114 (2023-05-31). See [stacking-and-paint.md § 4](stacking-and-paint.md), [history.md](history.md).
- **`::backdrop`** — a viewport-size pseudo-element rendered **immediately beneath** its owning top-layer element (modal dialog / popover / fullscreen); each top-layer entry has its own. The top layer is a LIFO stack, so each element/backdrop pair stacks above the previous. Buiy's Phase 9 `TopLayer` model has no backdrop entity yet — an open 6f sub-question. See [stacking-and-paint.md § 4](stacking-and-paint.md).

## Containment and queries

- **`contain`** — the CSS Containment property (Chrome 52, 2016) with flags `layout` / `paint` / `size` / `style`, plus shorthands `content` (= layout+paint+style, *no* size) and `strict` (= all four). A performance opt-in: proves work inside a subtree cannot escape. See [containment-and-queries.md § 1](containment-and-queries.md).
- **SIZE-zeroing** — `contain: size` (or `inline-size`) with no declared size resolves the contained axis to zero (the box collapses), because the engine may not examine descendants for an intrinsic size. Blink does this silently; Buiy adds a `warn!`. See [containment-and-queries.md § 1.1](containment-and-queries.md).
- **`content-visibility`** — Chromium 85 (2020); `visible` (no effect) / `hidden` (skip rendering, retain state, off the a11y tree) / `auto` (gain layout+style+paint containment always, plus size containment + skip layout/paint/hit-test when off-screen and not user-relevant). See [containment-and-queries.md § 2](containment-and-queries.md).
- **`contain-intrinsic-size`** — a declared fallback size so a size-contained or `content-visibility: auto` box does not collapse to zero / cause scroll jank when its contents are skipped. The `auto` keyword remembers the last-rendered size. See [containment-and-queries.md § 1.1, § 2](containment-and-queries.md).
- **Container query (`@container`)** — Chrome 105 (2022-08-30); styles an element by an ancestor *container's* resolved size. Requires containment to avoid the loop where a child sizes its parent. See [containment-and-queries.md § 3](containment-and-queries.md).
- **`container-type`** — establishes a query container: `size` applies layout+style+size containment (both axes queryable); `inline-size` applies layout+style+inline-size (inline axis only — the common case); `normal` applies no containment (style queries only). See [containment-and-queries.md § 3](containment-and-queries.md).
- **Container units (`cqw` / `cqi` / `cqh` / …)** — length units resolved against the query container's size. See [containment-and-queries.md § 3](containment-and-queries.md).
- **CSS anchor positioning** — Chrome 125 (rollout 2024-05-14); tethers an absolutely-positioned element to anchor element(s) declaratively. `anchor-name` registers an anchor, `anchor()` references its edges in `inset` properties, `position-try-fallbacks` declares fallbacks on overflow. Chrome-125 names `inset-area` / `position-try-options` were renamed `position-area` / `position-try-fallbacks` in Chrome 129. See [containment-and-queries.md § 4](containment-and-queries.md), [history.md](history.md).

## Governance and process

- **Blink Intent process** — the public, `blink-dev`-mailing-list workflow for web-exposed feature changes: **Intent to Prototype** (notification, behind a flag), **Intent to Experiment** (origin trial; one API-owner LGTM), **Intent to Ship** (enable by default; three API-owner LGTMs). See [governance.md](governance.md).
- **API owners** — a small, named group of senior Chromium contributors who grant the LGTMs gating ship; predominantly Google employees, so three LGTMs is not cross-vendor consensus. See [governance.md](governance.md).
- **ChromeStatus (chromestatus.com)** — the public tracker of each web feature's Intent stage and ship version. See [governance.md](governance.md), [history.md](history.md).
- **Engine monoculture** — the concern that, post-Edge-79, Blink's choices become the de-facto web standard ahead of the W3C spec text, since only Gecko and WebKit remain independent. The reason Buiy cites the W3C modules as the contract and treats Blink as one implementation. See [governance.md](governance.md), [critiques.md](critiques.md).
- **MiraclePtr / BackupRefPtr** — Chromium's use-after-free heap-hardening mitigations; part of why ~70%-memory-safety-bug C++ is "mitigated, not solved." See [open-problems.md § Memory safety](open-problems.md).

## Sources

- RenderingNG architecture / BlinkNG / LayoutNG — https://developer.chrome.com/docs/chromium/renderingng-architecture ; https://developer.chrome.com/docs/chromium/blinkng ; https://developer.chrome.com/docs/chromium/layoutng
- LayoutNG (chromium.org) — https://www.chromium.org/blink/layoutng/
- CSS 2.2 Appendix E painting order (z-index order + tree-order tiebreak) — https://www.w3.org/TR/CSS22/zindex.html
- MDN stacking-context enumeration (`will-change` on potential value; `isolation: isolate`) — https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_positioned_layout/Stacking_context
- MDN `contain` (paint containment clips + forms an SC) — https://developer.mozilla.org/en-US/docs/Web/CSS/contain
- MDN `::backdrop` (immediately beneath the top-layer element; LIFO) — https://developer.mozilla.org/en-US/docs/Web/CSS/::backdrop
- MDN `pointer-events` (hit-test only; `none` falls through) — https://developer.mozilla.org/en-US/docs/Web/CSS/pointer-events
- CSS Containment in Chrome 52 — https://developer.chrome.com/blog/css-containment
- content-visibility (Chromium 85) — https://web.dev/articles/content-visibility
- Container queries (Chrome 105) — https://developer.chrome.com/blog/has-with-cq-m105
- Popover API (Chrome 114) — https://developer.chrome.com/blog/new-in-chrome-114/
- Anchor positioning (Chrome 125) + syntax changes (Chrome 129) — https://developer.chrome.com/blog/anchor-positioning-api ; https://developer.chrome.com/blog/anchor-syntax-changes
- Blink launch process / Intents — https://www.chromium.org/blink/launching-features/
- Chromium memory safety — https://www.chromium.org/Home/chromium-security/memory-safety/
- Sibling files: [README.md](README.md), [lessons.md](lessons.md), [architecture.md](architecture.md), [layout.md](layout.md), [stacking-and-paint.md](stacking-and-paint.md), [containment-and-queries.md](containment-and-queries.md), [style.md](style.md), [governance.md](governance.md), [history.md](history.md), [critiques.md](critiques.md), [open-problems.md](open-problems.md), [comparisons.md](comparisons.md)
- Buiy hit-testing substrate: [../bevy-picking/](../bevy-picking/)
