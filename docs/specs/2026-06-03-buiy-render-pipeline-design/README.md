# Buiy — render-pipeline design

**Date:** 2026-06-03
**Status:** draft
**Parent:** [`2026-05-07-buiy-foundation`](../2026-05-07-buiy-foundation/README.md) — sub-spec graduated from [foundation/visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling) and the foundation roadmap row [`buiy-render-pipeline-design`](../2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap).

## Purpose

Define the target shape of Buiy's render pipeline: the GPU paint and compositing layer that consumes the immutable geometry the layout subsystem produces and turns it into pixels. This covers the render/paint/compositing rows of [foundation/visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling) — per-element clipping, top-layer compositing, atlasing, box-shadow, opacity, borders, backgrounds, outlines — plus the Bevy render-graph integration and the render-side component model that the layout subsystem's stacking-context formation already reaches for.

This spec describes the **target state** — what the render subsystem looks like once complete. Phase 0 ships a minimal real subset (a single instanced SDF rounded-rect `ViewNode` in `Core2d`, extracting `(Visual.background_token, border_radius, ResolvedLayout.position/size)`); the migration from Phase 0 to target lives in plans, not here.

### The contract, in one sentence

**Layout writes immutable paint inputs; render reads them and never re-derives paint order, stacking, or geometry.** This is the BlinkNG "immutable outputs" principle ([prior-art/blink/lessons.md](../../prior-art/blink/README.md)) — Buiy enforces it from day one rather than retrofitting it over a decade. Every architectural decision below serves that contract.

## Children

This is a multi-file spec. The catalog is split across the children below; the parent (this README) holds purpose, scope, pillars, the canonical component contract, the sub-spec roadmap, and open questions.

- [architecture.md](architecture.md) — Bevy 0.18 render-graph integration (`RenderApp`, `ExtractSchedule`, `ViewNode`/`ViewNodeRunner`, `Core2d` node-group placement), the typed-primitive batched node + top-layer composite pass, per-window node-group ownership, the hybrid persistent-buffer/per-frame-instance handoff, system-set order and the render-prep stage, crate placement.
- [component-model.md](component-model.md) — the render-side components that replace the temporary `Visual`: `Background`, `Border`, `BoxShadow`, `Opacity`, `Outline`; the reserved-now effect components `Filter`, `MixBlendMode`, `BackdropFilter` (so layout sub-pass 6f can read SC trigger 5); the new `ClipRect` and `EffectGroup`; reflection/BSN conventions.
- [clip-and-transform.md](clip-and-transform.md) — the `WriteClipRects` render-prep pass (ancestor clip intersection, `Changed<ScrollOffset>` recompute), and the `Transform`/`GlobalTransform` bridge (layout composes `ResolvedLayout` + `ResolvedTransform` into a Bevy `Transform`; `TransformSystems::Propagate` owns `GlobalTransform`; render reads it), including `perspective` / `transform-style` / `backface-visibility` consumption.
- [paint-order-and-top-layer.md](paint-order-and-top-layer.md) — forward consumption of `StackingContext.painters_z`, the paint/hit-test ordering identity, top-layer compositing at the root, the `::backdrop` model, and `Display::None` / `ContentVisibility` skip rules.
- [effect-compositor.md](effect-compositor.md) — the off-screen effect-group compositor: render-target allocation/pooling per `EffectGroup`, the v1 effect set (group `opacity` + `isolation`), reserved seams for `filter` / `backdrop-filter` / `mix-blend-mode`, and the gate-#15 memory budget.
- [atlas-and-text-seam.md](atlas-and-text-seam.md) — the F-tier texture atlas (glyph / icon / gradient / mask): allocation, warmup, eviction, pooling; and the reserved glyph-alpha primitive + shared `TextureAtlas` resource that `buiy-text-rendering-design` plugs into without this spec owning glyph shaping.
- [color-and-forced-colors.md](color-and-forced-colors.md) — linear-light render, sRGB output pre-tonemapping, theme-token resolution against `Res<Theme>`, the forced-colors contract (gate #11), and the boundary against deferred C-tier color management.
- [verification.md](verification.md) — how render correctness is proven: the headless-no-GPU constraint, the gate mapping (#2 visual-regression, #5 layout-snapshot for `ClipRect`, #10 hit-target, #11 forced-colors, #14 render-time, #15 atlas/RSS), and the e2e golden-image harness.

Reading order: architecture first (it sets the integration seams and the handoff every other file relies on), then component-model, then any topic in any order.

## 1. Goals and non-goals

### Goals

1. **Cover the [foundation visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling) render rows end-to-end at their tier.** Every tier-F paint/compositing item maps to a concrete component, pass, or render-graph node in this spec. Tier-C items are named with a deferral marker and a reserved seam so the deferred work does not require re-architecting. Tier-E items are named only.
2. **Render is a thin read-only consumer of layout's immutable output.** Render never re-sorts `painters_z`, never re-derives stacking, never recomputes geometry. If render needs a derived value (clip rect, composed transform), layout (or a layout-adjacent render-prep pass that reads only layout output) computes it and writes it to a component render extracts.
3. **Integrate with Bevy 0.18's render stack, not bevy_ui's.** Buiy owns its render passes inside Bevy's render graph (`RenderApp` + `ExtractSchedule` + `Core2d` node group), reusing `bevy_render` primitives (`PipelineCache`, `RenderDevice`, `ViewNode`) but none of bevy_ui's renderer — exactly because bevy_ui caps non-rect clipping, backdrop-filter, mix-blend-mode, isolation, and true top-layer compositing.
4. **Per-window by construction.** Render-graph node ordering, instancing, and top-layer root attachment are owned by a per-window node group keyed by winit `WindowId`. No global render state assumes a single Buiy window.
5. **Unblock the five render-gated layout follow-ups.** Landing this spec's component model and passes lets the layout subsystem wire: `UiTransform` paint + `Containment` PAINT clip + perspective/backface; render-side SC formers (`opacity`/`filter`/`mix-blend-mode`); `will-change` layer-promotion + SC trigger; the `Transform`/`GlobalTransform` bridge; and non-px translate resolution.
6. **Verifiable headless and on GPU.** Pipeline/graph registration, the component model, and `ClipRect` geometry are unit/integration-testable headless (no wgpu adapter on CI runners). Pixel correctness is proven by a golden-image e2e harness on a canonical CI GPU (gate #2). The spec defines which layer proves which property.

### Non-goals

- **Phase planning.** Plans (`docs/plans/`) decide what subset ships when. This spec is target-shape only.
- **Glyph shaping and text layout.** `buiy-text-rendering-design` owns cosmic-text integration, shaping, font fallback, and BiDi. This spec owns *only* the shared texture atlas and the glyph-alpha primitive seam that text paint plugs into.
- **Layout geometry.** `buiy-layout-design` owns `ResolvedLayout`, `ResolvedTransform`, `StackingContext`, `Containment`, `Overflow`, `ScrollOffset`, and the stacking-context trigger union. This spec *consumes* them and *contributes* the render-side trigger-5 components layout reads back.
- **Animation.** `buiy-animation-design` owns interpolation and the property-tree question (foundation §5 #18). This spec ships **no property trees** in v1 (see pillar 7) and renders the current frame's resolved values.
- **Hit-testing mechanism.** `buiy-input-events-design` owns picking. This spec fixes the *ordering identity* (hit-test order = paint order reversed) so the two cannot diverge, but picking's backend lives there.
- **The C-tier effect shaders.** `filter`, `backdrop-filter`, `mix-blend-mode`, gradients, masks, and `clip-path` are deferred. This spec ships their component model and the off-screen compositor they will ride, but not their shaders.
- **Window and surface lifecycle.** `buiy-window-and-surface-design` owns window creation, per-window top-layer routing, and the render-to-texture surface contract. This spec reserves the per-window node-group structure and an RTT target seam but does not build window management.

## 2. Architectural pillars (one-line summaries)

Each pillar is detailed in [architecture.md](architecture.md) or the named child; this section is the index and the record of the decisions taken during brainstorming (2026-06-03).

1. **Immutable layout output, thin render consumer.** Layout writes; render reads. `painters_z` is walked forward for paint and backward for hit-test; never recomputed. The decisive principle ([blink/lessons.md](../../prior-art/blink/README.md)).
2. **Typed-primitive batched node + top-layer composite pass.** A small set of typed SDF primitives (quad / shadow / glyph / path) batched per primitive + layer, plus one ordered composite pass for the top layer, inside a per-window `BuiyRenderLabel` node group in `Core2d` after `Node2d::EndMainPass` (pre-tonemapping, so output participates in HDR/color management). The shape two production Rust GPU UIs converged on ([prior-art/gpui](../../prior-art/gpui/README.md), [prior-art/makepad](../../prior-art/makepad/README.md)). **Runner-up:** a single grow-in-place node (rejected — cannot express the intermediate targets top-layer/effect compositing need); a full slimming-paint sub-graph (rejected — over-built before a concrete C-tier need).
3. **Hybrid handoff.** Persistent GPU buffers + atlas + view-uniform (the Phase-0-closeout-named upgrade that retires per-frame allocation and the per-instance y-flip/radius hack), with the per-frame instance set rebuilt from a `Changed<T>`-gated `Extract<Query>`. Damage tracking is the ECS change-detection Buiy gets for free; **no screen-space damage-rects in v1**. **Runner-up:** full immediate rebuild (rejected — wastes the gate-#14 budget); full retained scene (rejected — premature invalidation complexity, the classic stale-paint bug source).
4. **`ClipRect` computed in a render-prep pass, not in render extract.** Clip inputs (overflow, scroll viewport, containment:paint boundary, the box) are all layout-owned, so a `WriteClipRects` pass reading only layout output computes per-entity clip rects and writes a `ClipRect` component — keeping render a thin consumer and making clip geometry testable under the layout-snapshot gate (#5). A separate `Changed<ScrollOffset>` recompute keeps scroll responsive without re-running full layout. **Runner-up:** ancestor-walk in render extract (rejected — pushes tree traversal across the render boundary where it can silently drift from layout semantics and is invisible to gate #5).
5. **Layout owns the transform tree; render reads `GlobalTransform`.** Layout composes `ResolvedLayout.position` + `ResolvedTransform.matrix` into a Bevy `Transform`; `TransformSystems::Propagate` owns `GlobalTransform`; render reads `GlobalTransform`. Reuses Bevy's battle-tested hierarchical propagation and hands picking and the future 3D-anchored-UI/RTT path `GlobalTransform` for free. **Runner-up:** render reads `ResolvedTransform` directly (rejected — re-implements propagation Bevy already provides and diverges from picking/3D expectations).
6. **Off-screen effect-group compositor, built now, opacity + isolation only in v1.** The same SC-trigger set that forms stacking contexts is the set that forces off-screen render targets, so the compositor machinery (a render target per `EffectGroup`, GPU composite) is built in v1 and carries group `opacity` and `isolation` correctly. `filter` / `backdrop-filter` / `mix-blend-mode` get their component model and a reserved boundary but no shader in v1 (C-tier fast-follow). **Runner-up:** v1 forward-pass approximation of group opacity (rejected by the user — group opacity over overlapping children is subtly wrong, and correctness from day one was preferred).
7. **No property trees in v1.** `ResolvedTransform` and `painters_z` are the seeds; distinct clip/effect/scroll property trees are deferred until a concrete compositing need forces them. **The trigger condition that would revisit this is named: animating opacity (or transform) without re-running layout.** Until then, render paints the current frame's resolved values and relies on ECS change-detection. ([blink/lessons.md](../../prior-art/blink/README.md) "avoid building property trees speculatively".)

## 3. The component contract

This is the canonical list of the components on the layout↔render boundary. Children elaborate fields; this table is the single source of truth for names and ownership so the child files stay consistent. "Owner" is the subsystem whose spec defines the type; "render reads" / "render writes" is this subsystem's relationship to it.

### 3.1 Layout-owned, render reads (already exist)

| Component | Owner | What render reads it for |
|---|---|---|
| `ResolvedLayout` { position, size } | layout | Box geometry (folded into `Transform` by the bridge; see pillar 5). |
| `ResolvedTransform` { matrix } | layout | Composed transform (present iff non-identity); folded into `Transform`. |
| `StackingContext` { painters_z: Vec<Entity> } | layout | Forward paint order; reversed hit-test order. Never re-sorted. |
| `Stacking` { z_index, isolation, top_layer } | layout | Top-layer membership + tier; isolation as an effect-group boundary. |
| `Containment` { contain: ContainFlags } | layout | PAINT flag → clip boundary (consumed via `ClipRect`); STYLE/LAYOUT inform.|
| `Overflow` { x, y } + `ScrollOffset` { x, y } | layout | Clip viewport + scroll translation (consumed via `ClipRect`). |
| `ContentVisibility` (Visible/Auto/Hidden) | layout | Hidden → skip subtree; Auto off-screen → skip paint (render half of Phase 11). |
| `Display::None` | layout | Skip entirely (paint, clip, stacking traversal). |

### 3.2 Render-owned, this spec introduces

| Component | Tier | Purpose |
|---|---|---|
| `Background` | F | Solid color token (v1); reserved layered/gradient fields (C). Replaces `Visual.background_token`. |
| `Border` | F | Per-side width/style/color longhands; elliptical per-corner radius. Replaces `Visual.border_radius`. |
| `BoxShadow` | F | Ordered list; multiple, inset, spread, blur, color. |
| `Opacity` | F | Group opacity; non-1 forms an `EffectGroup` and an SC trigger (layout reads it back via pillar 6). |
| `Outline` | F | Focus indicator: color/style/width/offset. Painted outside the border box; never clipped by the element's own clip. |
| `Filter` | C (reserved) | Filter function list. Component + SC-trigger participation ship v1; shaders deferred. |
| `BackdropFilter` | C (reserved) | Backdrop filter list. Component + boundary ship v1; backdrop-sampling shader deferred. |
| `MixBlendMode` | C (reserved) | Blend mode. Component + SC-trigger participation ship v1; blend shader deferred. |
| `ClipRect` | F | Per-entity computed clip (see pillar 4). Written by `WriteClipRects`, read by render and picking. |
| `EffectGroup` | F | Marker on entities that establish an off-screen compositing boundary (opacity<1 / isolation / reserved filter/blend). Read by the compositor to allocate a render target. |

> **Crate-placement constraint (foundation §5 #28).** The reserved effect components (`Opacity`, `Filter`, `BackdropFilter`, `MixBlendMode`) must live where **both** layout sub-pass 6f and render can read them. If the workspace later splits `buiy_render` out of `buiy_core`, this dependency edge points layout → these components → render and must not be inverted. [architecture.md](architecture.md) pins their crate home.

### 3.3 The `Visual` migration

`Visual` (Phase 0: `background_token`, `foreground_token`, `border_radius`) is a temporary carrier. It is replaced by `Background` + `Border`; `foreground_token` (reserved, unused) moves to `buiy-text-rendering-design`. The migration is a plan concern; this spec defines only the target (`Visual` gone).

## 4. What this spec unblocks

The five render-gated entries in [`docs/plans/follow-ups.md`](../../plans/follow-ups.md) resolve once the matching piece of this spec lands:

1. **`UiTransform` paint + `Containment` PAINT clip + perspective/backface** → pillar 5 (bridge consumes `perspective`/`transform-style`/`backface-visibility`) + pillar 4 (`ClipRect` enforces PAINT).
2. **Render-side SC formers (`opacity`/`filter`/`mix-blend-mode`)** → § 3.2 reserved components; layout 6f gains its trigger-5 clause reading them.
3. **`will-change` layer-promotion + SC trigger** → render-side `WillChange` promotion hint + layout 6f reading the SC-forming-property clause (one change, both sides).
4. **`Transform`/`GlobalTransform` bridge** → pillar 5.
5. **Non-px translate units** → resolved layout-side once the bridge fixes the coordinate contract; this spec pins the contract.

## 5. Open questions

Decisions deferred to a sibling spec or to implementation calibration. Each names its owner.

1. **Per-window top-layer routing.** v1 ships a single global `TopLayerActivation` resource reading the primary window (the layout Phase-9 D2 simplification). True per-window routing is owned by `buiy-window-and-surface-design`. This spec reserves the per-window node-group structure; the global-activation simplification is a tracked dependency, not a final decision.
2. **Render-to-texture surface contract.** The RTT target abstraction (C-tier, feeds `buiy_3d`) is foundation §5 #13. This spec reserves a render-target seam the `ViewNode` can target but does not commit the `buiy_3d` boundary.
3. **`::backdrop` modeling.** Whether modal/popover dimming is a render-synthesized backdrop (internal sort key `owner_index − ε` in top-layer order) or an app-spawned scrim entity. [paint-order-and-top-layer.md](paint-order-and-top-layer.md) proposes render-synthesized; confirm against `buiy-window-and-surface-design`.
4. **Per-fixture perf and leak numbers.** Gate #14 (render-time vs main baseline, ±10% default slack) and gate #15 (RSS slope < 1 MB/min, atlas entries return to baseline) mechanisms are committed; the concrete per-fixture numbers are owned by `buiy-verification-design` and calibrate over time. This spec defines atlas eviction/warmup and buffer pooling so the gates are *satisfiable*.
5. **Crate split.** Foundation §5 #1/#28 — whether `buiy_render` splits out of `buiy_core`. This spec's component-placement constraint (§ 3.2) holds under either outcome.
6. **Animation substrate.** Foundation §5 #18. v1 takes no dependency (no property trees); the revisit trigger is named in pillar 7.

---

*Brainstormed and approved 2026-06-03. Design forks recorded in pillars 2–6. This is the target state; the Phase-0 → target migration lives in `docs/plans/`.*
