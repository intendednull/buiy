**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — Architecture: transform-based layout, ECS-native retained model, render path

# Architecture

bevy_lunex is the most prominent third-party UI library for the Bevy game engine that runs **parallel to bevy_ui** rather than on top of it. It is the closest design-space neighbor to Buiy, and the most useful one to compare against because it makes very different design choices on almost every axis.

Latest stable: **0.6.0** (published 2026-01-22 by maintainer IDEDARY, bytestring-net org). License MIT OR Apache-2.0. Crate name `bevy_lunex` (underscored); GitHub path `bytestring-net/bevy-lunex` (hyphenated). Verified via crates.io API + GitHub.

## One-line summary

bevy_lunex is a **retained layout engine that positions Bevy entities by writing their `Transform`s**. There is no Taffy. There is no flexbox. There is no CSS-style box model. There is no separate render pipeline — UI is drawn by `bevy_sprite`, `bevy_text`, `bevy_pbr`, and (optionally) `bevy_rich_text3d`, all of which already render `Transform`-positioned entities.

## The transform-based layout model

Where bevy_ui (and Buiy) feeds layout fields to Taffy and gets back rectangles in a separate `ComputedNode`, bevy_lunex computes positions inline and writes them directly into Bevy's general-purpose `Transform` (and a sibling `Dimension` component for size).

The computation is one `UiLayoutType::compute()` call per node, taking:

- The parent rectangle (recursive — root is filled by `UiFetchFromCamera`).
- An absolute scale factor.
- Viewport size.
- Font size.

Returns a `(position, size)` pair that is splatted into the node's `Transform.translation` and `Dimension`. Verified in `crate/src/layouts.rs`.

There is no separate "computed" component. The layout solver is the placement, not a precursor to it. This is the central design bet, and it has direct consequences for the rest of the system — see [`component-model.md`](component-model.md) for the input/output collapse, [`layout.md`](layout.md) for the solver pass, [`3d-and-worldspace.md`](3d-and-worldspace.md) for why this choice is what makes worldspace UI cheap.

**Comparison to Buiy.** Buiy's foundation ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) commits to Taffy directly with a `ComputedNode`-style output split (borrowed from bevy_ui — see [bevy-ui lessons.md Borrow #2](../bevy-ui/lessons.md)). bevy_lunex's choice trades flexbox/grid (which Taffy ships) for direct `Transform` integration and trivial 3D-anchored UI. Buiy's bet is "we need the web-platform layout features more than we need cheap worldspace UI"; bevy_lunex's bet is the opposite.

## The "UI tree" pattern: `UiLayoutRoot` + children

There is no top-level resource analogous to `UiTree`. A bevy_lunex UI is an entity hierarchy rooted in an entity carrying `UiLayoutRoot` (plus typically `UiFetchFromCamera` to bind the root rectangle to a camera's viewport). Each descendant carries a `UiLayout` component describing its position/size relative to its parent.

This is conceptually a recursive containing-block resolution, but implemented as a single recursive system that walks the hierarchy each frame. There is no caching beyond Bevy's change detection on individual components. See [`layout.md`](layout.md) § "Solver pass" for the cadence.

## Solver pass: system ordering and schedule

bevy_lunex partitions per-frame work into three labeled SystemSets, ordered:

```
UiSystems::PreCompute  →  UiSystems::Compute  →  UiSystems::PostCompute
```

These live in the `PostUpdate` schedule (verified in `lib.rs`). State transitions (e.g. `UiHover` lerp) run in `Update`. Picking lives in `PreUpdate` and is registered with `bevy_picking`'s `PickingSystems::Backend` set (verified in `crate/src/picking.rs`).

**`UiLunexPlugins`** is the plugin-group entry point. It composes:

- `UiLunexPlugin` — core solver + state systems.
- `UiLunexPickingPlugin` — bevy_picking backend (2D + 3D).
- `UiLunexIndexPlugin<const INDEX>` — per-camera-index plugin for multi-camera setups.
- `UiLunexDebugPlugin` — gizmo overlay (separate gizmo groups for 2D / 3D: `LunexGizmoGroup2d`, `LunexGizmoGroup3d`).

## Render path: there is no render pipeline

This is the load-bearing simplification. bevy_lunex itself contains **no rendering code**. After the layout pass writes `Transform`s, Bevy's existing renderers do the drawing:

- Sprites are drawn by `bevy_sprite`.
- Text is drawn by `bevy_text`.
- 3D meshes (for in-world UI surfaces) are drawn by `bevy_pbr`.
- Optional 3D rich text is drawn by `bevy_rich_text3d` (gated behind the `text3d` feature flag).

bevy_lunex's `Cargo.toml` confirms this: it depends on `bevy_sprite`, `bevy_sprite_render`, `bevy_text`, `bevy_pbr`, `bevy_render`, `bevy_mesh`, `bevy_image`, `bevy_camera` — and **does not depend on `bevy_ui`**. Verified at `crate/Cargo.toml` (main, 2026-01-22).

The implication is dramatic: bevy_lunex sidesteps every renderer limitation Buiy cites against bevy_ui (non-rectangular clipping, `backdrop-filter`, `mix-blend-mode`, true top layer — see [bevy-ui lessons.md Avoid #2](../bevy-ui/lessons.md)) by **not solving any of them**. There is no rounded-corner clipping because clipping happens (or doesn't) in `bevy_sprite`. There is no top layer because z-ordering is `Transform.translation.z` and `UiDepth`. There is no `backdrop-filter` because there is no compositor.

This is internally consistent and a real design choice, not a gap. It means bevy_lunex is good at the things sprite/mesh rendering is good at and structurally incapable of the things they don't do.

## 3D-anchored UI as a first-class feature

Because layout writes `Transform`, a `Transform` is a `Transform` is a `Transform` — there is no extra work needed to anchor a UI node to a 3D entity in a 3D scene. Mark a root with `UiRoot3d` instead of (or in addition to) `UiLayoutRoot`, attach `UiMeshPlane3d` to the node, parent it under a 3D entity, and the same layout solver drives it.

This is bevy_lunex's biggest differentiator. The `sprite3d` and `text3d` examples (verified in `examples/`) demonstrate the pattern. See [`3d-and-worldspace.md`](3d-and-worldspace.md) for the full mechanism and the comparison with Buiy's planned `buiy_3d` sub-spec ([foundation cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

## In-world UI: panels can live in 3D scenes

Two patterns are supported:

1. **Worldspace UI rooted in 3D** — a `UiLayoutRoot` directly in 3D space, no projection back to screen. The UI panel exists in the world at a fixed world-position, optionally billboarded.
2. **UI projected from a 3D entity to a screen overlay** — e.g. damage numbers tracking a 3D character. Realized by sourcing the root's transform from a 3D entity while rendering in screen space.

Both use the same `UiLayout` component. There is no `WorldspaceUiLayout` vs `ScreenspaceUiLayout` split. See [`3d-and-worldspace.md`](3d-and-worldspace.md) for details.

## What bevy_lunex does NOT own

For honest comparison: bevy_lunex does not ship its own focus model, accessibility tree, theme tokens, animation library beyond per-state lerps, IME / complex-text editing, form state machine, devtools beyond a debug gizmo plugin, or a CSS-flavored stylesheet. It is a **layout positioning library plus a small interaction surface**, not a full UI framework. This is the right framing when reading the rest of these files.

## Sources

- bevy_lunex repo — https://github.com/bytestring-net/bevy-lunex
- crates.io listing — https://crates.io/crates/bevy_lunex
- docs.rs API — https://docs.rs/bevy_lunex/0.6.0/bevy_lunex/
- The Lunex Book — https://bytestring-net.github.io/bevy_lunex/
- `crate/src/lib.rs`, `crate/src/layouts.rs`, `crate/src/picking.rs`, `crate/Cargo.toml` (main branch, accessed 2026-05-22)
- Buiy foundation — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- bevy-ui prior-art — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
