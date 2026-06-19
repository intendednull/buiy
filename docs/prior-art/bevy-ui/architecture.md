**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — plugin placement, system ordering, Node/ComputedNode decomposition, Taffy integration, render-pipeline shape and its current caps

## Where bevy_ui sits in the Bevy plugin graph

`bevy_ui` is a workspace crate inside the Bevy monorepo. Its plugin is `UiPlugin`, added to a Bevy `App` either by `DefaultPlugins` or manually. `UiPlugin::build` ([source](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/lib.rs)):

- Initializes `UiSurface`, `UiScale`, `UiStack`, and computed-target-camera/render-info propagation plugins.
- Registers the `ui_focus_system` in `PreUpdate`.
- Calls `build_text_interop()` to wire bevy_ui ↔ bevy_text.
- Conditionally adds the picking backend behind the `bevy_picking` feature.
- Adds the accessibility plugin and interaction-state observers.

bevy_ui depends on (Cargo.toml, [main HEAD](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/Cargo.toml)): `taffy 0.10`, `accesskit 0.24`, and optionally `bevy_picking 0.19.0-dev`. The text stack (parley 0.9.0 + swash 0.2.6) is reached *via* `bevy_text`, not by `bevy_ui` directly.

Crate workspace metadata: license `MIT OR Apache-2.0`, MSRV `1.95.0`, edition 2024 ([Cargo.toml](https://github.com/bevyengine/bevy/blob/main/Cargo.toml)).

## System sets

bevy_ui exports a `UiSystems` label enum whose variants partition per-frame UI work. `UiPlugin::build` chains them as ([lib.rs](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/lib.rs)):

```
UiSystems::Prepare → Propagate → Content → Layout → PostLayout → Stack
```

`Focus` runs separately in `PreUpdate` (the hit-test pass — see [text-and-input.md](text-and-input.md)).

The chain places computed/derived properties downstream of layout: content-size measurement seeds Taffy in `Content`, Taffy solves the tree in `Layout`, computed components and clipping resolve in `PostLayout`, and z-order stacking finalises in `Stack`. Render-app data extracts from the main world *after* `Stack` via Bevy's standard `ExtractSchedule`.

## Node / Style / ComputedNode decomposition

The current shape of UI components is the result of a three-release migration ([component-model.md](component-model.md) details the timeline):

- **Pre-0.15:** `NodeBundle` wrapped `Node`, `Style`, `BackgroundColor`, `BorderColor`, etc. Authoring required spawning the full bundle.
- **0.15 (Dec 2024):** Bundles deprecated. `Style`'s fields were merged into `Node` ("`The Style component fields have been moved into Node. Style was never a comprehensive 'style sheet', but rather just a collection of properties shared by all UI nodes`" — [Bevy 0.15 notes](https://bevy.org/news/bevy-0-15/)). Computed/layout-derived properties moved to a new `ComputedNode` component. `Node` itself became a required-components hub: spawning `Node` auto-inserts the standard companion components.
- **0.18 (Dec 2025):** Border-radius moved *back* into `Node` as a field ("`BorderRadius is no longer a component, instead a border_radius: BorderRadius field has been added to Node`" — [0.17→0.18 migration](https://bevy.org/learn/migration-guides/0-17-to-0-18/)). `LineHeight` was split out of `TextFont` into its own component.

So as of 0.18.1 / 0.19-rc.1, the canonical decomposition is:

- `Node` — authoring-side style + identity. Includes layout fields (`width`, `height`, `display`, `position_type`, `flex_*`, `grid_*`, etc.) and now `border_radius`.
- `ComputedNode` — laid-out geometry (resolved size, position, content size, scroll position).
- `BackgroundColor`, `BorderColor`, `Outline` — visual decoration as separate components.
- `BackgroundGradient`, `BorderGradient` — gradient variants (0.17+).
- `UiTransform` / `UiGlobalTransform` — 2D-only transforms ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)), replacing 3D `Transform`/`GlobalTransform` for UI.

The 0.15 split was the design that made required-components a real authoring story, but it left a hybrid: visual properties like `BackgroundColor` are decomposed, while layout properties remain bundled in `Node`. See [component-model.md](component-model.md) for the BSN implications and issue #17644's critique of the analogous `bevy_a11y` AccessibilityNode megacomponent.

## Taffy integration

bevy_ui drives Taffy through `UiSurface`, a resource that maintains the `taffy::TaffyTree`:

- `bevy_ui::layout::ui_surface` ([source](https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui/src/layout)) holds the tree handle plus an entity ↔ Taffy-node mapping.
- `bevy_ui::layout::convert` translates `Node` fields into `taffy::Style`.
- During `UiSystems::Content`, content-size measurement functions (e.g. text intrinsic size from parley) feed Taffy.
- During `UiSystems::Layout`, `taffy::compute_root_layout` runs against the viewport; results are copied back into `ComputedNode`.
- During `UiSystems::PostLayout`, scroll-position and clipping geometry are derived from the computed tree.

Taffy 0.10 supplies flex, grid, and block layout; bevy_ui enables all three via its `taffy` feature flags. Layout features that Taffy does not yet ship (anchor positioning, container queries, subgrid, masonry) are accordingly unavailable through bevy_ui — see [layout.md](layout.md).

## Render pipeline

bevy_ui's render lives in the `bevy_ui::render` module (extracted into the render sub-app). The pipeline:

1. **Extract** — `ExtractedUiNode`s are pulled from the main world. Each carries a `z_order: f32` (renamed from `stack_index: u32` in 0.18 to support finer-grained ordering and texture-slice fixes — [0.17→0.18 migration](https://bevy.org/learn/migration-guides/0-17-to-0-18/)).
2. **Prepare** — UI items are batched. Standard pipeline targets a single quad-mesh shader for solid nodes; texture-sliced/9-patch images go through a separate path; gradients and per-side border colors (0.17+) have their own shader specialisations.
3. **Queue / Render-graph node** — `bevy_ui::render` registers a render-graph node that runs after the main 3D/2D passes and before tonemapping (within the UI camera's view).
4. **Atlas** — text glyphs are rasterised via parley/swash into a glyph atlas; images participate in the standard `Image` asset pipeline.
5. **UI camera** — UI is associated with a camera via `ComputedNodeTarget`; multiple UI cameras and multi-window are supported.
6. **UiMaterial** — `UiMaterial` lets users plug custom shaders into the UI pipeline for materials beyond the built-in node shader. This is the documented extension point for visual effects bevy_ui doesn't ship natively.

## Renderer caps that Buiy treats as parallel-stack rationale

Verified as of 0.18.1 / 0.19-rc.1 against current source and a January 2026 issue from a core UI contributor:

- **Non-rectangular clipping is not supported.** From issue [#22345 (Unified Bevy User Interface, viridia, 2026-01-02)](https://github.com/bevyengine/bevy/issues/22345): "`We currently only support rectangular clipping regions (which are the easiest to implement and cheapest from a performance standpoint), but which are inadequate for the kinds of UIs we want to build.`" Rounded-rect clipping along arbitrary `BorderRadius` is approximated by the node shader during fragment shading, not by clip planes — so it cannot compose with arbitrary shapes (CSS `clip-path`). There is no `mask-image` equivalent.
- **No backdrop-filter.** Achieving the CSS `backdrop-filter` effect requires sampling the framebuffer behind a node, blurring it, and compositing — bevy_ui does not implement this in its render-graph node.
- **No mix-blend-mode or isolation.** UI nodes blend with the default alpha-over operator. Per-node blend-mode selection, isolation groups, or `mix-blend-mode` analogues are not exposed; `UiMaterial` lets users specialise blend state per material, but there is no first-class node-level CSS-mix-blend semantic.
- **No CSS `top layer`.** Modal/popup/tooltip elevation in bevy_ui is via `z_order` and the stacking pass — there is no out-of-flow top-layer compositor pass equivalent to the browser CSS top layer (which sits above all stacking contexts and traps focus).
- **Render-graph integration with arbitrary 2D/3D scenes is structurally separate.** Per #22345 again, the UI tree uses `UiGlobalTransform` (2D, introduced in 0.17) rather than the world's `GlobalTransform`, and the layout box-model "`is quite different from the way that 2d and 3d scenes are constructed.`" Mixing UI with diegetic in-world content remains a coordination problem, not a primitive.

These limitations are not bugs — they are the result of bevy_ui being a CSS-flavoured rectangular-stacking renderer optimised for the game-HUD case. Issue #22345 is the upstream proposal to lift several of them, but its status (as of 2026-05) is `S-Needs-Design-Doc` and the listed scope (clipping in particular) is described as "`require substantial architectural redesign`." See [open-problems.md](open-problems.md) (Agent B) for the current Bevy-side issue status.

For Buiy, the chosen response is parallel rather than patch: own the render-graph node, own the clip-path primitive, own backdrop-filter, own top-layer compositing. See `docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.3.

## Sources

- https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui
- https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui/src
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/Cargo.toml
- https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_ui/Cargo.toml
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/lib.rs
- https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui/src/layout
- https://github.com/bevyengine/bevy/blob/main/Cargo.toml
- https://github.com/bevyengine/bevy/issues/22345
- https://github.com/bevyengine/bevy/issues/17644
- https://bevy.org/news/bevy-0-15/
- https://bevy.org/news/bevy-0-17/
- https://bevy.org/learn/migration-guides/0-17-to-0-18/
