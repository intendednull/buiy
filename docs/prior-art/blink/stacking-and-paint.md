**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium / RenderingNG) — stacking-context formation, paint order, paint property trees, the top layer; mapped to Buiy Phase 9 (`StackingContext.painters_z`, `TopLayer` / `TopLayerActivation`).

# Blink: stacking, paint order, and the top layer

Blink is the rendering engine of the Chromium project — forked from WebKit's WebCore and announced 2013-04-03 ([Chromium blog](https://blog.chromium.org/2013/04/blink-rendering-engine-for-chromium.html)). It is the canonical reference implementation of the CSS stacking and paint model, so its choices are load-bearing prior art for Buiy's **Phase 9** stacking + top-layer sub-pass (6f). It is shared by Chrome, Edge (Chromium-based since Edge 79, 2020-01-15), Brave, Opera, Vivaldi, and Samsung Internet, which makes its quirks de-facto interop requirements.

This file is the Phase-9-critical reference. It maps Blink's stacking-context formation, paint order, paint property trees, and top-layer implementation onto Buiy's spec at [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md). For the surrounding engine architecture see [architecture.md](architecture.md); for `ComputedStyle` see [style.md](style.md); for containment triggers see [containment-and-queries.md](containment-and-queries.md).

## 1. Stacking-context formation triggers

CSS does not state one rule for "what forms a stacking context"; it states a *union* of triggers scattered across modules. Blink implements that union in `PaintLayer` / `LayoutObject::StyleDidChange`. The practically observable set (per the [MDN stacking-context enumeration](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_positioned_layout/Stacking_context)):

- the root element;
- `position: absolute` / `relative` with a `z-index` other than `auto`;
- `position: fixed` / `sticky` (forms one unconditionally);
- `opacity` < 1;
- `transform`, `filter`, `backdrop-filter`, `perspective`, `clip-path`, `mask` other than `none`;
- `mix-blend-mode` other than `normal`;
- `isolation: isolate`;
- `contain: layout`, `paint`, `strict`, or `content`;
- `will-change` naming any property that would otherwise form one;
- top-layer elements and their `::backdrop`.

Buiy's [stacking-and-top-layer.md § 2](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#2-stacking-context-formation) deliberately mirrors this as a typed union: positioned + `ZIndex::Layer(_)`, `Isolation::Isolate`, non-identity `Transform`, `Containment::contain` ⊇ `Paint`/`Strict`, render-side `opacity < 1` / `filter` / `mix_blend_mode`, and the root. The lesson Buiy took: enumerate the triggers in one place and treat *any* one as sufficient, rather than scattering the checks. Blink's own scattering across `LayoutObject` is a recurring source of "why did this element suddenly composite?" confusion — Buiy's single sub-pass-6f detection is the deliberate counter-design.

One CSS quirk Buiy keeps verbatim: a `z-index` on a `position: static` element forms no stacking context and is ignored. Blink enforces this; Buiy asserts it in its `PositionKind::Static` test ([§ 6](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#6-test-surface)).

### 1.1 Two triggers the 6f author must not conflate: `isolation` vs `contain: paint`, and `will-change`

Two entries in that union are easy to treat as interchangeable "just form a stacking context" flags, and getting them wrong produces a clipping bug rather than a paint-order bug:

- **`isolation: isolate` forms a stacking context but does *not* clip.** It exists to isolate `mix-blend-mode` (descendants blend among themselves, not with content behind the isolated root) — a pure stacking/effect operation. Per MDN, `isolation: isolate` "creates a new stacking context" with no containing-block or clip side effect.
- **`contain: paint` forms a stacking context *and* clips descendants to the border box.** MDN: "If a descendant overflows the containing element's bounds, then that descendant will be clipped to the containing element's border-box," and `layout`/`paint`/`strict`/`content` each create "a new stacking context" *plus* a new containing block and block formatting context.

So both are SC triggers in the 6f union, but only `contain: paint`/`strict` should also feed the clip side of the render hand-off. A 6f author who treats them identically (both "just SC triggers") will either over-clip `isolation: isolate` subtrees or under-clip `contain: paint` ones. Buiy already separates these — `Isolation::Isolate` is a `Stacking` field, `Containment::contain ⊇ Paint` is a `Containment` field — so keep the SC-trigger union and the clip behaviour as two distinct outputs of 6f. See [containment-and-queries.md § 5](containment-and-queries.md).

A second subtlety: **`will-change: transform` forms a stacking context even when `transform` is currently `none`.** It is a *pre-emptive* promotion hint — MDN says an element with a `will-change` value "specifying any property that would create a stacking context on non-initial value" forms one, i.e. based on the property's *potential*, not its current value. Buiy's Phase 8 stores `will-change` **stored-only** (it changes no layout/render output yet — see [containment-and-queries.md](containment-and-queries.md), [history.md](history.md)). The 6f decision is therefore explicit: if Buiy's union is to match Blink, `will-change` naming a would-be trigger must *itself* count as a trigger; if 6f instead only inspects the *resolved* `Transform`/`opacity`/etc., it will under-form stacking contexts relative to Blink. This is a documented divergence to make on purpose, not by omission.

## 2. Paint order within a stacking context

CSS 2.2 Appendix E ([painting order](https://www.w3.org/TR/CSS22/zindex.html)) defines a fixed seven-step order that Blink's paint walk in `PaintLayerPainter` follows:

1. the element's own background and borders;
2. negative-`z-index` child stacking contexts (most negative first);
3. in-flow, non-positioned, non-inline-level descendants (block boxes);
4. non-positioned floats;
5. in-flow, inline-level, non-positioned descendants;
6. positioned descendants with `z-index: auto` or `0`, in tree order;
7. positive-`z-index` child stacking contexts (least positive first).

Buiy compresses this into `StackingContext.painters_z: Vec<Entity>` — a *pre-sorted* paint order computed once at layout time ([§ 2.1](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#21-stackingcontext-private-component)). Buiy's order is: negative `z_index`, in-flow non-positioned, floats (always empty — floats are tier-O, deferred), in-flow positioned `z: auto`, positive `z_index`. Steps 1, 3, and 5 above (own box, blocks, inlines) collapse in Buiy because it has no separate inline-vs-block box generation at this layer — text is a leaf via `cosmic-text`, and box layout is delegated to Taffy ([../taffy/](../taffy/)). Buiy paints whole entities.

The contract is the headline difference: **Blink re-walks the layer tree at paint time**, whereas Buiy's spec mandates "layout writes, render reads — render never recomputes paint order" ([§ 5](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#5-mapping-to-render)). Blink mitigates its re-walk with the paint property trees (next section) and display-list caching; Buiy front-loads the sort into sub-pass 6f. The open question Buiy flagged — detect eagerly in 6f vs. lazily at paint — is exactly the tradeoff Blink resolved toward caching rather than naive recomputation. Buiy chose eager because its render side is a thin consumer, not a second rendering engine.

### 2.1 The tie-break the 6f sort must define: equal `z-index` paints in tree order

Appendix E orders by `z-index`, but *within* one `z-index` value (and for the `z-index: auto`/`0` step 6) the spec resolves ties by **tree order** — "in tree order" / "in the order in which they appear in the source document." `painters_z` is therefore a **stable sort** on `z_index`, and the tiebreak is document order. Blink gets that order for free: the DOM is an ordered tree, so a stable walk yields a deterministic tiebreak.

Buiy does *not* get it for free. Entities come from the ECS and have **no inherent document order** — the order a `World` iterates archetypes is not stable across spawns/despawns and is explicitly not a Buiy ordering guarantee. So the 6f sort must establish its own stable "tree order" to feed the stable sort, and that decision is load-bearing for determinism (two runs of the same scene must produce the same `painters_z`). The candidate keys, in rough order of robustness:

1. **Hierarchy walk + sibling index.** Walk Bevy's `Children` in stored child order, depth-first; a child's position in its parent's `Children` list *is* the authored "source order." This is the closest analogue to the DOM and the one that matches author intent. It requires `Children` ordering to be treated as significant (it already is for layout flow).
2. **A spawn-order / monotonic-id component.** Simpler, but it diverges from author intent the moment children are reordered without respawning, and it leaks entity-allocation order into paint order.

The spec's Appendix-E mapping does not yet pin which key 6f uses; this is the gap to close before implementing the sort. The recommendation that matches CSS is key (1) — tiebreak by the depth-first `Children`-order index — because it reproduces "source order" semantics and stays stable under the ECS's non-deterministic archetype iteration. Record the chosen key in the Phase 9 plan; a `painters_z` whose tiebreak depends on archetype iteration order is a latent nondeterminism bug. See Buiy [stacking-and-top-layer.md § 2.1](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#21-stackingcontext-private-component).

## 3. Paint property trees (transform / clip / effect / scroll)

The decisive RenderingNG redesign for stacking was moving visual effects off the layer tree and into four **paint property trees**, the outcome of the multi-year "Slimming Paint" project ([chromium.org/blink](https://www.chromium.org/blink/)):

- **transform tree** — accumulated `transform` and scroll translations;
- **clip tree** — overflow clips, `clip-path`, the viewport clip;
- **effect tree** — `opacity`, `filter`, `mix-blend-mode`, masks, isolation;
- **scroll tree** — scrollable areas and their offsets.

Each `LayoutObject` references one node in each tree (its "property tree state"). During the **pre-paint** lifecycle stage these trees are computed; **paint** produces a display list keyed against them; **commit** copies the trees and the display list to the compositor thread (`cc`), which can then re-composite — animate transforms, change opacity, scroll — *without re-running layout, style, or paint on the main thread* ([RenderingNG architecture](https://developer.chrome.com/docs/chromium/renderingng-architecture)). The full document lifecycle is 12 stages: animate, style, layout, pre-paint, scroll, paint, commit, layerize, raster/decode, activate, aggregate, draw (verified against the RenderingNG architecture page — note it is finer-grained than the common nine-stage summary).

This is the most important structural lesson for Buiy. Buiy does not build paint property trees today, but the *separation of concern* is identical: layout resolves the stable facts (membership, order, composed transform), render/compositor consumes them. Buiy's Phase 8 already produces a private `ResolvedTransform` render hand-off ([transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md)), which is the seed of a transform tree; sub-pass 6f's `painters_z` is the seed of the layerization input. Whether Buiy ever needs distinct clip/effect/scroll trees is an [open-problems.md](open-problems.md) question — for a retained-mode Bevy UI compositing via the [../bevy-ui/](../bevy-ui/) / wgpu render graph, the tradeoff differs from a 60fps scrolling web page.

A caveat to record honestly: the property-tree split was hard-won. Blink shipped it over years (Slimming Paint v1 → v2 → "CompositeAfterPaint", default ~2021), and the migration was a long-running source of paint-invalidation bugs. Buiy should not copy the trees speculatively — only when a concrete compositing requirement (e.g. animating opacity without re-laying-out) forces it.

## 4. The top layer

The **top layer** is a per-document parallel rendering layer that paints above everything else and *escapes ancestor `overflow` clipping and containing-block stacking entirely*. An element promoted to the top layer leaves its parent stacking context for paint purposes while keeping its layout-tree position. Blink populates the top layer from three sources:

- `dialog.showModal()` — the element plus a `::backdrop`;
- the **Popover API** (`popover` attribute), shipped enabled-by-default in **Chrome 114**, stable **2023-05-31** ([New in Chrome 114](https://developer.chrome.com/blog/new-in-chrome-114/), [chromestatus 5463833265045504](https://chromestatus.com/feature/5463833265045504));
- the **Fullscreen API** (`requestFullscreen`).

Blink maintains the top layer as a single ordered list — a **last-in / first-out (LIFO) stack** — across *all three* sources; the element added most recently paints on top, and the list bypasses the normal stacking machinery. Closing a popover or dialog removes it from the list.

#### `::backdrop` — where the backdrop paints within the top layer

Each top-layer element gets its own `::backdrop` pseudo-element, "a box the size of the viewport, which is rendered immediately beneath" that element (MDN). So within the top layer the paint order around one entry is: everything below the top layer → that entry's `::backdrop` → that entry. With multiple entries, each element/backdrop pair stacks above the previous in LIFO order, which is how a modal dialog opened over a popover correctly dims the popover too. **Buiy's Phase 9 `TopLayer` model has Modal/Popover/Tooltip/Fullscreen tiers but no backdrop entity**, so the open sub-question for 6f is whether Buiy models a backdrop at all: the minimal faithful model is a per-top-layer-entry backdrop slot that sorts *immediately below its owner* in the top-layer paint order (not a sibling in normal stacking). If Buiy skips backdrops entirely, modal dimming must be expressed some other way (e.g. a full-window scrim entity the app spawns) — a divergence to state, not assume. Buiy's `TopLayerActivation` `VecDeque` is already the LIFO ordering Blink uses; a backdrop is "owner's index − ε," not a new ordering concept.

#### Nested / paired top-layer semantics

Borrow item 3 in [lessons.md](lessons.md) tells the Phase 9 author to "study how Blink handles nested popovers"; the concrete parts a real implementation hits:

- **Nested popovers stack in open order** because they are pushed onto the single LIFO list in the order they open; closing the outer one light-dismisses (and closes) the inner ones it contains. Blink's *one ordered list* is what resolves this — there is no separate per-tier list whose interleaving has to be reconciled.
- **Light-dismiss ordering.** A click outside the topmost auto popover closes it (and any popovers nested *inside* it), top-down; `Esc` closes the topmost. The ordering that decides "topmost" is the same LIFO list.
- **Dialog-inside-popover / popover-inside-dialog** is exactly the case Buiy's *tiered* model complicates: Blink has no `Tooltip`/`Modal` *kind* ordering — it is purely recency, so a popover opened after a modal paints above it. Buiy fixes `Fullscreen < Tooltip < Popover < Modal` as a kind ordering *then* recency within a tier, so a popover opened after a modal stays *below* the modal. That is a deliberate divergence from Blink's single-list recency model and is the one place Buiy's "single ordered list" is actually two-level (tier, then `VecDeque` recency). Decide and document whether a popover spawned by a modal should escape the modal's tier — Blink would let it; Buiy's tier rule would not. See [comparisons.md](comparisons.md) for how [../coherent-gameface/](../coherent-gameface/) and [../rmlui/](../rmlui/) handle the same nesting.

Buiy's `TopLayer { None | Modal | Popover | Tooltip | Fullscreen }` enum ([§ 4](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#4-top-layer)) is the direct typed analogue, with three deliberate divergences from Blink:

1. **Explicit tier ordering.** Buiy fixes `Fullscreen < Tooltip < Popover < Modal` as a *kind* ordering, then orders within a tier by activation recency via the `TopLayerActivation` (`VecDeque<Entity>`) resource. Blink has no separate `Tooltip` tier (tooltips are popovers/hints there); Buiy adds it because game-UI tooltips are common and should sit below modals. This is a Buiy choice, not a CSS fact — flagged for [critiques.md](critiques.md).
2. **Per-window scope.** Buiy gives each window its own top layer ([§ 4.4](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#44-per-window-scope)); Blink's top layer is per-document. Multi-window is a Bevy-native concern with no clean CSS precedent.
3. **Single composite winner for fullscreen.** Buiy's spec makes one fullscreen entity win and the rest fall back to normal stacking ([§ 4.2](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#42-top-layer-ordering)); Blink's fullscreen stack is similarly last-wins but still a list.

The escape-from-clip behaviour is identical in intent: Buiy's [§ 4.3](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#43-escape-from-clip) says top-layer entities are not clipped by an ancestor `OverflowMode::Hidden`/`Clip`; their effective clip is the window viewport. In Blink this falls out of the top-layer element attaching to the viewport's property-tree state rather than its DOM ancestor's clip node — a concrete demonstration of why the property-tree separation (§ 3) matters: clip escape is a re-parenting in the clip tree, not a special case in the paint walk.

## 5. Hit-testing: the inverse of paint order

The folder covers paint order thoroughly; a Phase 9 author also needs its inverse, because Buiy integrates `bevy_picking` for pointer input ([../bevy-picking/](../bevy-picking/)) and stacking decides which entity an input event hits.

- **Hit-testing traverses *reverse* paint order.** The element painted last (topmost) is hit-tested first; the browser checks the topmost element under the pointer and walks downward until it finds a hit. So the same `painters_z` order that drives paint, read back-to-front, is the hit-test order. A pre-sorted `painters_z` is therefore *also* the pre-sorted hit-test list — front-loading the sort in 6f pays off twice (paint front-to-back, pick back-to-front).
- **The top layer intercepts input first.** Because top-layer entries paint above everything, they hit-test first; a modal dialog's backdrop captures clicks that would otherwise reach content beneath it (this is *why* modal means modal). Buiy's `TopLayer` ordering is thus an input-routing decision as much as a paint one.
- **`pointer-events` interacts with stacking, not layout.** `pointer-events: none` makes an element transparent to hit-testing only — "if that element has `pointer-events: none`, it is skipped and the browser checks the element underneath" (MDN); it changes neither paint, visibility, nor stacking. So in Buiy a "is this entity pickable?" flag must be consulted *during* the back-to-front `painters_z` walk, letting picking fall through to the next entity, rather than being a layout or stacking concern.

The Buiy-adjacent decision left open: `bevy_picking` has its own backend ordering/`Pickable` model ([../bevy-picking/architecture.md](../bevy-picking/architecture.md)), so 6f must decide whether stacking *feeds* the picking order (the CSS-faithful choice: hit-test = reverse `painters_z`, honouring `pointer-events`) or whether picking stays on `bevy_picking`'s own z/`render layer` ordering, which would let paint order and pick order disagree — the exact "why did the click go there?" confusion that mirrors Blink's "why is this on top?" When stacking and hit-test order diverge, CSS faithfulness is lost; flag the chosen coupling in the Phase 9 plan.

## 6. Performance: what the eager-sort premise rests on (and what is unmeasured)

The "pre-sort eagerly in 6f, don't re-walk at paint" choice (and the Avoid row in [lessons.md](lessons.md)) rests on a stated premise: **most entities form no stacking context**, so `painters_z` vectors are short and re-sorting is rare. That is the *cost-side* assumption — and it is asserted, not measured. The honest gap to record:

- Blink publishes no order-of-magnitude figure for "how often the layer tree is re-sorted per frame" or "typical layer count," and Buiy has no benchmark yet for how many entities form an SC in a representative UI, how large `painters_z` gets, or how often a frame dirties the sort. So the **eager-vs-lazy tradeoff this folder repeatedly raises cannot actually be adjudicated from the evidence here** — the eager choice is defensible on the *thin-render-side* argument (render is a consumer, not a second engine) but not yet on data.
- What *is* documented from Blink is qualitative: the property-tree split exists specifically so transform/opacity/scroll animation re-composites *without* re-running paint, i.e. Blink's own design treats per-frame paint-order recomputation as expensive enough to engineer around. That supports "don't recompute paint order every frame" but says nothing about absolute `painters_z` sizes.
- The actionable item for the Phase 9 plan: state the "most entities form no SC" assumption explicitly as a *premise to validate*, and add a counter/histogram (entities-with-SC per frame, max `painters_z` length, re-sorts per frame) before claiming eager-sort is the right call. Until then it is a reasoned default, not an evidenced one.

## 7. Implications for Buiy

- **Keep the trigger union in one place.** Sub-pass 6f detecting all SC triggers centrally is the right counter to Blink's scattered `LayoutObject` checks. Verify it runs *after* 6e so the composed `Transform` is available for trigger 3. Keep the SC-trigger union and the *clip* output separate (§ 1.1): `isolation: isolate` forms an SC without clipping; `contain: paint` forms an SC *and* clips.
- **Pre-sort, don't re-walk.** `painters_z` computed once honours "render reads, layout writes." Blink's per-frame re-walk + caching is the alternative Buiy explicitly rejected; record the perf assumption (most entities form no SC) in the spec *as a premise to measure* (§ 6). The same sorted vector, read back-to-front, is the hit-test order (§ 5).
- **Pin the equal-`z` tiebreak.** Appendix E breaks `z-index` ties by tree order; the ECS has none, so 6f must define a stable key (depth-first `Children`-order index, per § 2.1) or `painters_z` is nondeterministic.
- **Defer the property trees.** Adopt the layout/composite *separation* now; build distinct transform/clip/effect/scroll trees only when a real compositing requirement appears. Blink's multi-year Slimming Paint migration is the cautionary tale.
- **Top-layer clip escape is a re-parent, not a hack.** Model it as "effective clip = window viewport," matching Blink's clip-tree re-parenting, rather than a per-entity skip-clip flag.
- **Name the divergences.** The `Tooltip` tier and per-window scope are Buiy additions, not CSS — keep them visible so faithfulness claims stay honest. See [comparisons.md](comparisons.md) for how [../coherent-gameface/](../coherent-gameface/) and [../rmlui/](../rmlui/) handle the same problem with HTML/CSS-derived stacking.

## Sources

- Blink fork announcement (2013-04-03): https://blog.chromium.org/2013/04/blink-rendering-engine-for-chromium.html
- What is Blink (Chrome for Developers): https://developer.chrome.com/docs/web-platform/blink
- Chromium Blink overview (Slimming Paint, repaint): https://www.chromium.org/blink/
- RenderingNG architecture + 12-stage lifecycle + property trees: https://developer.chrome.com/docs/chromium/renderingng-architecture
- RenderingNG deep-dive: LayoutNG (immutable fragment tree, Chrome 77): https://developer.chrome.com/docs/chromium/layoutng
- CSS 2.2 Appendix E painting order (z-index order + "in tree order" tiebreak): https://www.w3.org/TR/CSS22/zindex.html
- MDN stacking-context formation enumeration (incl. `will-change` forms an SC on potential value; `isolation: isolate`): https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_positioned_layout/Stacking_context
- MDN `contain` (paint containment clips descendants to the border-box + forms a stacking context): https://developer.mozilla.org/en-US/docs/Web/CSS/contain
- MDN `::backdrop` (rendered immediately beneath the top-layer element; LIFO top-layer stack): https://developer.mozilla.org/en-US/docs/Web/CSS/::backdrop
- MDN `pointer-events` (hit-test only; `none` skips to the element underneath): https://developer.mozilla.org/en-US/docs/Web/CSS/pointer-events
- New in Chrome 114 (Popover API enabled by default, 2023-05-31): https://developer.chrome.com/blog/new-in-chrome-114/
- chromestatus: The Popover API: https://chromestatus.com/feature/5463833265045504
- Edge 79 Chromium stable (2020-01-15): https://blogs.windows.com/msedgedev/2020/01/15/upgrading-new-microsoft-edge-79-chromium/
- Buiy spec — stacking and top layer: ../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md
- Buiy spec — transforms and containment (Phase 8): ../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md
- Buiy foundation: ../../specs/2026-05-07-buiy-foundation/README.md
- Sibling prior-art: [architecture.md](architecture.md), [layout.md](layout.md), [containment-and-queries.md](containment-and-queries.md), [style.md](style.md), [critiques.md](critiques.md), [open-problems.md](open-problems.md), [comparisons.md](comparisons.md)
- Cross-engine prior art: [../taffy/](../taffy/), [../bevy-ui/](../bevy-ui/), [../servo-stylo/](../servo-stylo/), [../coherent-gameface/](../coherent-gameface/), [../rmlui/](../rmlui/)
- Buiy hit-testing substrate: [../bevy-picking/](../bevy-picking/), [../bevy-picking/architecture.md](../bevy-picking/architecture.md)
