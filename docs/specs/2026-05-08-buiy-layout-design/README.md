# Buiy — layout design

**Date:** 2026-05-08
**Status:** active
**Parent:** [`2026-05-07-buiy-foundation`](../2026-05-07-buiy-foundation/README.md) — sub-spec graduated from [foundation/visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout) and the foundation roadmap row [`buiy-layout-design`](../2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap).

## Purpose

Define the target shape of Buiy's layout subsystem: types, components, system pipeline, and invariants that realize the layout feature inventory in [foundation/visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout).

This spec describes the **target state** — what code should look like once the layout subsystem is complete. Phase 0 ships a minimal subset (Taffy bridge, `FlexDirection::{Row, Column}`, fixed `width`/`height`); the migration from Phase 0 to target lives in plans, not here.

## Children

This is a multi-file spec. The catalog is split across the children below; the parent (this README) holds purpose, scope, sub-spec roadmap, and open questions.

- [architecture.md](architecture.md) — Taffy bridge, hybrid `Style` builder + decomposed components, system pipeline order, `LayoutTree` GC, topological invariant, error model, crate placement.
- [box-model.md](box-model.md) — Content/padding/border/margin boxes, `box_sizing`, intrinsic sizing keywords, `aspect_ratio`, logical-property aliases, units (px / % / em / rem / viewport / container / `fr`).
- [display-and-positioning.md](display-and-positioning.md) — `Display` enum (block, inline, flex, grid, table, list-item, ruby, contents, flow-root, none); `Position` enum (static, relative, absolute, fixed, sticky); containing-block resolution; `inset` + logical inset; **anchor positioning** as a post-Taffy overlay pass.
- [flex-and-grid.md](flex-and-grid.md) — Flexbox parameters, Grid parameters (incl. subgrid status), masonry status, multi-column.
- [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) — `@container` and container units; same-frame re-layout activation strategy; writing-mode (`horizontal-tb`, `vertical-rl`, `vertical-lr`, `sideways-rl`, `sideways-lr`); `direction` (ltr/rtl); `text-orientation`; `unicode-bidi`.
- [overflow-and-scrolling.md](overflow-and-scrolling.md) — Overflow modes (visible/hidden/clip/scroll/auto + axis variants + logical), `scroll-behavior`, `overscroll-behavior`, scroll snap, scrollbar styling.
- [stacking-and-top-layer.md](stacking-and-top-layer.md) — Stacking-context formation triggers, `z_index`, `isolation`, **top layer** (modals/popovers/dialogs/fullscreen escape from stacking).
- [transforms-and-containment.md](transforms-and-containment.md) — `transform` (2D + 3D), longhand `translate`/`rotate`/`scale`, `transform-origin`, `perspective`, `backface-visibility`; `contain`; `content_visibility`; `will_change`.

Reading order: architecture first (it sets the invariants every other file relies on), then any topic in any order.

## 1. Goals and non-goals

### Goals

1. **Cover [foundation visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout) end-to-end.** Every tier-F and tier-C item in §3.2 maps to a concrete component, builder method, or system in this spec. Tier-E items are named with a deferral marker; tier-O items appear nowhere.

2. **Buiy extends Taffy where Taffy doesn't yet cover something needed.** The bridge stays one-directional — Buiy translates *to* Taffy, never patches Taffy's algorithm in place. Features Taffy lacks (anchor positioning, container queries, writing-mode polish) are implemented as Buiy passes that wrap Taffy, not as Taffy forks.

3. **BSN-friendly decomposed components.** Per the project convention (foundation goal §1.3), every layout property exposed to authors is a small public-fielded `Component` deriving `Reflect + Default + Clone + Component`. The mega-`Style` of Phase 0 graduates to a `Style` *builder* that produces a Bundle of decomposed components on insert.

4. **Predictable system order.** Layout runs in a fixed pipeline whose order is testable. Container query activation re-runs the Taffy compute step at most once per frame; anchor positioning runs after Taffy. No silent ordering surprises.

5. **Topology and identity stable across frames.** `LayoutTree` (the `NonSendResource` holding the `TaffyTree` and the `Entity → TaffyNodeId` map) is reused across frames so Taffy's internal cache stays warm; despawned entities are GC'd by a `RemovedComponents<Node>` reader.

### Non-goals

- **Phase planning.** Plans (`docs/plans/`) decide what subset ships when. This spec is target-shape only.
- **CSS string parser.** Buiy components are typed Rust values, not CSS strings. A CSS-flavored stylesheet layer above tokens is a foundation open question (foundation §5).
- **Custom layout algorithms beyond Taffy + the listed Buiy passes.** Anchor positioning and container queries are explicitly Buiy-owned because Taffy doesn't ship them. Anything else stays in Taffy.
- **Animation of layout properties.** `buiy-animation-design` owns interpolation; this spec defines the static target geometry only.
- **Render-side concerns.** Stacking context formation triggers are listed because they're computed during layout, but compositing, clipping, and z-order draw scheduling live in `buiy-render-pipeline-design`.
- **Hit-testing.** `buiy-input-events-design` owns picking; layout produces the `ResolvedLayout` that picking reads from.

## 2. Architectural pillars (one-line summaries)

Each pillar is detailed in [architecture.md](architecture.md); this section is the index.

1. **Single-pipeline, fixed order.** `RemovedNodes → SyncStyles → ContainerQueryActivate → TaffyCompute → ContainerQueryFlipCheck → maybe-re-Taffy → AnchorResolution → WriteResolvedLayout`, gated behind `BuiySet::Layout`.
2. **Hybrid component API.** Public ergonomic `Style` (struct-literal *and* fluent methods over the same builder); on insert, expands to a Bundle of decomposed components. Decomposed components are canonical for ECS storage, BSN, reflection, and serialization.
3. **`LayoutTree` is the bridge.** A `NonSendResource` holding `TaffyTree<()>` + `HashMap<Entity, TaffyNodeId>`. Lifetime: app-long. GC: `RemovedComponents<Node>` reader.
4. **Container queries: same-frame re-layout, capped 2×.** Activate against this frame's Taffy output; if any query flipped, run Taffy a second time. No fixed-point iteration.
5. **Anchor positioning: post-Taffy overlay.** Anchored elements are laid out by Taffy first (using their author-declared dimensions), then a Buiy pass overrides their `ResolvedLayout.position` based on the anchor's resolved rect.
6. **Topological invariant.** Parents resolve before children. Document order = AccessKit tree order = default tab order. Any violation is a bug, not a tunable.
7. **Error model.** Layout failures (Taffy `Err`, missing parent, invalid constraint) `warn!` and leave the prior frame's `ResolvedLayout` in place. No sentinel writes; no panic.

## 3. Sub-spec roadmap

This spec is a leaf — it does not spawn further sub-specs. Per-feature depth lives in the children listed under [Children](#children). When `buiy-widget-catalog-design` graduates and per-widget specs become multi-file children of *that* catalog, they may reference sections of this spec; they don't create new layout sub-specs.

## 4. Coordination with sibling specs

| Sibling | Coordination point |
|---|---|
| [`buiy-render-pipeline-design`](../2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) | Stacking context formation, `z_index`, top-layer dispatch, `transform` and `filter` propagation. Layout *computes* stacking-context triggers; render *consumes* them. |
| `buiy-text-rendering-design` | `min-content`/`max-content`/`fit-content` intrinsic sizing for text needs to query the text shaper for shrink-to-fit widths. The query interface lives in this spec ([box-model.md](box-model.md)); the implementation lives there. |
| `buiy-text-editing-design` | Caret position and selection rectangles consume `ResolvedLayout`; no layout-side coordination beyond that. |
| `buiy-focus-model-design` | Document order for default tab order is the layout topological order. |
| `buiy-accessibility-design` | AccessKit tree order = layout topological order. |
| `buiy-animation-design` | Layout properties are interpolatable; this spec defines their typed-value shape, animation defines how they tween. |
| `buiy-input-events-design` | `bevy_picking` AABB hit-tests against `ResolvedLayout`. Stacking + top-layer determines hit-priority; layout owns the geometry, picking owns the test. |
| `buiy-i18n-design` | Writing-mode and direction are layout's; ICU, BiDi resolution algorithm, and locale-aware formatters are i18n's. The boundary is: layout knows *which* mode is active, i18n decides *what* the text looks like in that mode. |
| `buiy-window-and-surface-design` | Layout root sizing pulls from `bevy::window::Window`; multi-window and render-target sizing contracts live there. |
| `buiy-3d-anchored-ui-design` | Worldspace UI uses the same `ResolvedLayout` produced by this pipeline; the 3D-anchor spec defines how worldspace transforms feed back into layout root sizing. |

## 4.1 Migration

This spec is target-state. The Phase 0 → target migration (15-component decomposition, hybrid `Style` builder, 8-step pipeline, anchor positioning, container queries, sticky/table/multicol sub-passes, stacking-context detection, top-layer per-window, `LogicalBoxModel` insert-helper) lives in a series of follow-up plans, the first of which is [Phase 1 — Foundation](../../plans/2026-05-08-buiy-layout-foundation.md). The plans sequence the work into reviewable PRs; nothing in this spec depends on the plans landing.

## 5. Open questions

- **Crate placement.** Whether layout lives in `buiy_core` (Phase 0 location) long-term, or splits into `buiy_layout` per [foundation README § 5 — crate-split refinement](../2026-05-07-buiy-foundation/README.md#5-open-questions). Resolution waits on the foundation open question; this spec assumes either.
- **Anchor positioning fallback-chain depth cap.** v1 supports any chain length (resolved per [display-and-positioning.md § 3.1](display-and-positioning.md#31-anchor-component)). Open: whether to cap depth via a `position_try_max_depth` resource if profiling surfaces deeply-nested fallback hot paths.
- **Container query unit semantics in nested containers.** `cqi` / `cqb` resolve against the nearest *queried* ancestor, but the interaction with `container-type: inline-size` vs `size` is subtle when nested. v1 implements the common case (single query container per axis); complex nesting is deferred to a follow-up. [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) details.
- **Subgrid availability.** Tracks Taffy upstream — Buiy ships subgrid when Taffy ships it. v1 surface includes the API stubs but the implementation returns `Display::Grid` semantics until upstream lands.
- **Masonry availability.** Tracks Taffy and CSS-WG. Currently flux. v1 marks it tier-E and does not ship.
- **Stacking-context performance.** The set of triggers is large (positioned + non-`auto` z-index, opacity < 1, transform, filter, will-change, isolation, mix-blend-mode). Whether to detect lazily (during paint) or eagerly (during layout) is open. [stacking-and-top-layer.md](stacking-and-top-layer.md) discusses.
- **`writing-mode: sideways-*` Taffy support.** Taffy 0.10 has logical properties but doesn't fully model sideways modes. Whether to ship a Buiy-side rotation pass or wait on Taffy is open. [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) details.
- **Top-layer ordering across windows.** When a Buiy app has multiple windows each with its own modal, modal stacking is per-window. Cross-window top-layer (a modal that visually escapes its window) is out of scope; tracked in `buiy-window-and-surface-design`.

## References

- [Foundation/visuals.md § 3.2 — Layout](../2026-05-07-buiy-foundation/visuals.md#32-layout) — feature inventory.
- [Foundation/architecture.md](../2026-05-07-buiy-foundation/architecture.md) — parallel-stack rationale, primitives integrated directly.
- [Foundation/cross-cutting.md](../2026-05-07-buiy-foundation/cross-cutting.md) — i18n cross-cutting concerns informing writing-mode design.
- Taffy — https://github.com/DioxusLabs/taffy
- CSS Anchor Positioning Module Level 1 — https://www.w3.org/TR/css-anchor-position-1/
- CSS Containment Module Level 3 — https://www.w3.org/TR/css-contain-3/ (container queries)
- CSS Writing Modes Level 4 — https://www.w3.org/TR/css-writing-modes-4/
- CSS Display Module Level 3 — https://www.w3.org/TR/css-display-3/
