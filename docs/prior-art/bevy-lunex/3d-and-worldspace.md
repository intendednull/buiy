**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — 3D-anchored UI and worldspace UI: the load-bearing differentiator

# 3D-anchored and worldspace UI

This is bevy_lunex's biggest single differentiator vs bevy_ui (and the most directly relevant comparison for Buiy's planned `buiy_3d` sub-spec). The mechanism is small, well-defined, and entirely a consequence of the transform-based layout choice covered in [`architecture.md`](architecture.md) and [`layout.md`](layout.md).

## The core idea

Because the layout solver writes results into Bevy's general-purpose `Transform`, and because Bevy already renders `Transform`-positioned entities in 3D scenes via `bevy_pbr` / `bevy_sprite_3d`, a UI panel in a 3D scene is **not a different kind of UI** — it's the same UI tree with a different root.

There is no projection step, no separate 3D-UI render pass, no "convert UI coords to 3D coords" math the user has to write. Layout produces world-space transforms; the existing 3D renderer draws them. Verified via `examples/sprite3d/` and `examples/text3d/` (in the main branch as of 2026-01-22).

## Mechanism: `UiRoot3d` + `UiMeshPlane3d`

Two markers compose the worldspace pattern:

- **`UiRoot3d`** on the root entity tells the solver to treat this tree as 3D-space-rooted. The root's `Transform` is *not* fetched from a camera viewport; instead it's whatever you (or a parent 3D entity) set.
- **`UiMeshPlane3d`** on any node in the tree reconstructs a quad mesh sized to that node's resolved `Dimension`. The quad lives at the node's `Transform`, in 3D space. Apply any standard Bevy 3D material (PBR, custom shader, masked image) to it.

The layout system doesn't know or care that the tree is in 3D — it does the same recursive `compute()` walk as it would for a screen-space root. The 3D renderer handles culling, depth, lighting (if you use a `StandardMaterial`), shadows, fog, etc.

## Worldspace UI: anchoring to a 3D entity

Pattern: parent the `UiRoot3d` entity under any 3D entity in the scene (a character, a vehicle, a building). The UI tracks the parent's `GlobalTransform` automatically. A floating health-bar above an enemy is:

1. Spawn enemy entity with `Transform`.
2. Spawn a child entity with `UiRoot3d`, `UiLayoutRoot`, and `Transform { translation: Vec3::Y * 2.0, .. }` (1.0 unit above the parent).
3. Add layout children — bar background, fill, text — each with `UiLayout` and (typically) `UiMeshPlane3d`.

When the enemy moves, the UI moves with it. When the enemy is occluded, the UI is occluded (because it's a real 3D mesh going through normal depth test). When the enemy is dead, despawn the parent and the UI goes with it.

## Billboards / always-face-camera

bevy_lunex itself does **not** ship a billboard component. To make a 3D-anchored UI face the camera, the application either:

1. Uses Bevy's general transform-tracking primitives (a system that rotates the UI root to face the active camera each frame), or
2. Integrates a third-party billboard crate (e.g. `bevy_mod_billboard`).

This is a notable gap given how often "damage numbers" / "name tags" are cited as worldspace-UI use cases. Buiy's `buiy_3d` sub-spec ([foundation cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) item: "UI panels as billboards in 3D space") commits to first-class billboard support as a tier-C item.

## Render-to-texture surfaces

bevy_lunex supports a render-to-texture pattern via the `UiEmbedding` component, which marks an entity as carrying a resizable texture embedding. The mechanism (verified via lib.rs exports + book references): a separate camera renders the UI tree into a texture, and that texture is applied to a 3D material on a mesh elsewhere. This is the recipe for in-world screens / terminals / billboards-as-UI-surfaces.

The pattern is supported but not as turn-key as the direct `UiRoot3d` + `UiMeshPlane3d` route. For most "UI in 3D space" needs, the direct approach is preferred because it skips the indirection through a texture (no resolution baking, no wgpu render target allocation, no second camera).

## Hit-testing of 3D-anchored UI

bevy_lunex's picking backend (`crate/src/picking.rs`) handles both 2D and 3D UI in a single 2D-style algorithm:

1. For each pickable UI node, convert the 3D camera ray into the node's local coordinate space.
2. Find where the ray intersects the node's Z=0 plane.
3. Test whether the intersection point is inside `Rect::from_center_size(Vec2::ZERO, dimension)`.
4. Z-sort hits by camera-space depth (radix sort on `Transform.translation.z`).
5. Honor `Pickable::should_block_lower` for opaque-UI semantics.

This works for both screen-space (the ray is perpendicular to the screen) and worldspace UI (the ray comes from wherever the camera + cursor point). The `UiLunexPickingPlugin` registers as a `bevy_picking` backend in `PreUpdate::PickingSystems::Backend`.

A 3D-anchored UI panel set up this way is fully clickable, hoverable, and integrated with the `UiHover` state machine — pointer events drive state transitions, which drive layout interpolation, which drives the visual response, all without screen-space-vs-world-space special cases.

`NoLunexPicking` opts an entity out of the backend (useful for decorative-only nodes).

## Game-UI use cases

| Use case | Pattern |
|---|---|
| HUD (health, minimap, ammo) | `UiLayoutRoot` + `UiFetchFromCamera` in screen space |
| Damage numbers tracking a 3D enemy | `UiRoot3d` parented to enemy; despawn after fade |
| Floating nameplate above 3D character | `UiRoot3d` + (app-provided) billboard system |
| In-world terminal / control panel | `UiRoot3d` + `UiMeshPlane3d` materials, or render-to-texture via `UiEmbedding` onto a mesh |
| Diegetic menu (the menu is *in* the world) | Direct `UiRoot3d` mesh; pickable via the picking backend |
| Inventory grid for a top-down game | `UiLayoutRoot` in screen space; same layout components as 3D variants |
| Multi-monitor / split-screen UI | `UiSourceCamera<INDEX>` + `UiFetchFromCamera<INDEX>` per camera; `UiLunexIndexPlugin<INDEX>` per index |

The pattern coverage is real and well-demonstrated in the `Bevypunk` flagship demo (a cyberpunk-themed UI showcase by the maintainer; cited from the README).

## Comparison to Buiy's planned `buiy_3d` sub-spec

Buiy's foundation commits to a separate `buiy_3d` crate ([foundation architecture.md § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md)) with scope enumerated in [cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md):

| Foundation item | bevy_lunex status |
|---|---|
| UI panels as billboards in 3D space (**C**) | App-side; not built in |
| UI panels on curved or arbitrary surfaces (**E**) | Not supported — `UiMeshPlane3d` is a flat quad only |
| Worldspace UI hit-testing through the 3D scene (**C**) | Fully supported via `UiLunexPickingPlugin` |
| Diegetic UI (UI that lives "in" the game world) (**C**) | Fully supported |
| Render-to-texture for UI applied to 3D meshes (**C**) | Supported via `UiEmbedding` |

bevy_lunex covers the **C-tier** items (worldspace hit-testing, diegetic UI, render-to-texture) and skips the **E-tier** item (curved surfaces). The billboard gap is real. Per-line, this is the most direct match between any existing Bevy UI library and Buiy's planned scope — bevy_lunex's 3D story is what `buiy_3d` will need to match or exceed.

The architectural difference Buiy carries forward: Buiy's components are renderer-owned (Buiy draws its own quads with its own materials), so `UiMeshPlane3d`-style "the user picks an arbitrary 3D material" composes differently. Buiy's render pipeline could ship its own first-class 3D-anchored material slot, but the choice of whether to let users drop in raw Bevy materials (bevy_lunex's model) or only Buiy-blessed materials (a more restricted model with consistency benefits) is an open question for `buiy_3d`.

The foundation spec ([cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) explicitly notes Buiy nodes use Bevy's general `Transform`, the same choice bevy_lunex made. This is the most important load-bearing decision bevy_lunex validates for Buiy: **3D-anchored UI is unblocked the moment your component model uses `Transform` instead of a bespoke `UiTransform`**.

## Sources

- `crate/src/lib.rs`, `crate/src/layouts.rs`, `crate/src/picking.rs` (main branch, 2026-05-22)
- `examples/sprite3d/`, `examples/text3d/` directory listings (main branch)
- The Lunex Book — https://bytestring-net.github.io/bevy_lunex/
- Bevypunk showcase — https://github.com/IDEDARY/Bevypunk
- Buiy foundation cross-cutting — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
