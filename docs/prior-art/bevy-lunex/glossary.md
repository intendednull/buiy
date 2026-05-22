**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — system-specific terms used across this corpus

# Glossary

Definitions for bevy_lunex-specific identifiers, type names, and ecosystem terms used throughout this corpus. Cross-link liberally; do not duplicate definitions in evidence files — point at this glossary instead. For terms shared with `bevy_ui` (`bevy_picking`, `AccessKit`, `cosmic-text`, `Taffy`, etc.) see [`../bevy-ui/glossary.md`](../bevy-ui/glossary.md).

## Layout components

- **`UiLayoutRoot`** — marker component for the root of a screen-space UI tree. Spawning it begins layout for the subtree. There is no global `UiTree` resource; the entity hierarchy is the layout tree. See [`component-model.md`](component-model.md) and [`architecture.md`](architecture.md).
- **`UiRoot3d`** — marker component for the root of a worldspace-3D UI tree. The root's `Transform` is *not* fetched from a camera viewport; it is whatever a parent 3D entity sets, enabling the UI to anchor to 3D scene entities. The single load-bearing component for bevy_lunex's worldspace-UI differentiator. See [`3d-and-worldspace.md`](3d-and-worldspace.md).
- **`UiLayout`** — the workhorse layout component. Wraps a `UiLayoutType` plus per-state overrides (`base`, `hover`, `clicked`, `selected`, `intro`, `outro`). Read by the layout solver each frame; the solver interpolates between `base` and active-state variants using the state value as the lerp parameter.
- **`UiLayoutType`** — enum with three variants (`Window` / `Solid` / `Boundary`) describing how a node fits inside its parent. The central layout shape. See [`layout.md`](layout.md) § "Layout primitives" and [`component-model.md`](component-model.md) § "The `UiLayoutType` enum."
- **`Window`** — `UiLayoutType` variant. `pos: UiValue<Vec2>` + `size: UiValue<Vec2>` + `anchor: Anchor`. The classic "place a rect at an anchor relative to parent center." The default and most-used variant.
- **`Solid`** — `UiLayoutType` variant. Aspect-ratio-locked. `size` is interpreted as a ratio (e.g. `Vec2::new(16.0, 9.0)` for 16:9); the node is sized to fit inside its parent according to `scaling: Scaling` (one of `Fit` / `Fill` / `HorFill` / `VerFill`). `align_x: Align` / `align_y: Align` control placement of the smaller axis. The mechanism behind bevy_lunex's "any aspect ratio" claim.
- **`Boundary`** — `UiLayoutType` variant. Two corners: `pos1` (top-left) and `pos2` (bottom-right); size is the delta. Useful for stretchy regions defined by their bounds rather than dimensions. Conceptually CSS `inset: 0 0 0 0` with arbitrary corner offsets.
- **`Dimension`** — output component holding resolved `Vec2 { x: width, y: height }`. Written by the layout solver each frame. The closest bevy_lunex analog to bevy_ui's `ComputedNode.size`. The layout pass writes both this *and* the entity's `Transform.translation` — there is no `Node` / `ComputedNode` input/output split.
- **`UiFetchFromCamera<const INDEX>`** — binds the root rectangle of a `UiLayoutRoot` to the viewport of the camera with the given `INDEX`. The `const INDEX` generic supports multi-camera setups; `UiLunexIndexPlugin<INDEX>` instantiates a copy of the solver per camera index.
- **`UiSourceCamera<const INDEX>`** — marks a camera as the source for index `INDEX`.

## Unit system

- **`UiValue<T>`** — sum-typed container holding optional values across multiple unit systems (`Ab`, `Rl`, `Rw`, `Rh`, `Vp`, `Vw`, `Vh`, `Em`). Supports mixed-unit arithmetic, summed at resolve time. Analogous to CSS `calc(4px + 1em)` but more rigid: no `rem`, no `ch` / `ex` / `lh`, no container-query units, no `fr`. See [`layout.md`](layout.md) § "Units."
- **`Ab(f32)`** — absolute pixels. `1Ab = 1px` at default scale.
- **`Rl(f32)`** — relative to parent (% of parent dimension).
- **`Rw(f32)`** — relative to parent **width** (applied to either axis).
- **`Rh(f32)`** — relative to parent **height** (applied to either axis).
- **`Vp(f32)`** — relative to viewport.
- **`Vw(f32)`** / **`Vh(f32)`** — relative to viewport width / height.
- **`Em(f32)`** — relative to font size. `1em = 1 * font_size`.

## State and interaction components

- **`UiBase`** — default state marker, always present implicitly.
- **`UiHover`** — pointer-hover state. Carries `forward_speed`, `backward_speed`, `curve` (easing function), `instant: bool`, plus a private interpolated value. The most-developed state component.
- **`UiClicked`** — click state. **WIP** per source comments.
- **`UiSelected`** — selection state. **WIP**.
- **`UiIntro`** / **`UiOutro`** — entry/exit transition states. **WIP**.
- **`UiState`** — aggregates per-state interpolated values for a node; the solver reads this to pick the right layout variant.
- **`UiStateTrait`** — trait every state component implements; exposes a normalized `value() -> f32` consumed by the lerp in the layout solver.
- **`OnHoverSetCursor`** — requests a cursor icon while a node is hovered.
- **`NoLunexPicking`** — opts an entity out of the bevy_lunex `bevy_picking` backend.

## Visual components

- **`UiColor`** — the single styling component bevy_lunex ships. Color per state (`base` / `hover` / `clicked` / `selected` / `intro` / `outro`), interpolated by the same state value as `UiLayout`. Applies to whichever drawable is on the entity (sprite, mesh, text); there is no separate background/border/foreground concept.
- **`UiDepth`** — overrides Z-axis stacking. Two modes: absolute Z value or local-to-parent. The bevy_lunex analog of `ZIndex`.
- **`UiEmbedding`** — marks an entity as carrying a resizable texture embedding. The mechanism for render-to-texture surfaces (UI rendered into a texture, applied to a 3D mesh elsewhere).
- **`UiTextSize`** — scales text relative to other nodes; bridges text intrinsic size into the layout unit system. Added at 0.2.1 to fix text blurriness from the transform-based model's pixel-fidelity problem.
- **`UiImageSize`** — analogous to `UiTextSize` for images. Bridges an image's intrinsic size into layout units.
- **`UiMeshPlane2d`** / **`UiMeshPlane3d`** — markers requesting reconstruction of a quad mesh sized to the node's `Dimension`. The 3D variant is what enables worldspace UI panels with arbitrary Bevy materials.

## Cursor system

- **`SoftwareCursor`** — virtual cursor entity with priority + atlas mapping. The "software cursor" subsystem rendered by bevy_lunex itself rather than the OS cursor.
- **`GamepadCursor`** — drives a `SoftwareCursor` from gamepad input. Modes: free (analog stick → pointer position) and snap (gamepad navigates between focusable nodes). Added at 0.2.2.
- **`CursorIconQueue`** — resource arbitrating cursor-icon requests across pointers by priority.

## Plugins

- **`UiLunexPlugins`** — plugin-group entry point. Composes `UiLunexPlugin` + `UiLunexPickingPlugin` + `UiLunexIndexPlugin<INDEX>` + `UiLunexDebugPlugin`.
- **`UiLunexPlugin`** — core solver and state systems. Registers `UiSystems::PreCompute` / `Compute` / `PostCompute` in `PostUpdate`.
- **`UiLunexPickingPlugin`** — `bevy_picking` backend (2D and 3D in a single algorithm). Registers in `PreUpdate::PickingSystems::Backend`.
- **`UiLunexIndexPlugin<const INDEX>`** — per-camera-index plugin for multi-camera setups.
- **`UiLunexDebugPlugin`** — gizmo overlay for debugging. Separate gizmo groups for 2D and 3D: `LunexGizmoGroup2d`, `LunexGizmoGroup3d`.
- **`UiSystems::PreCompute` / `Compute` / `PostCompute`** — labeled `SystemSet`s in `PostUpdate` partitioning per-frame layout work. The bevy_lunex analog of Buiy's planned `BuiySet::Layout` → `BuiySet::Render` ordering.

## Documentation and ecosystem

- **The Lunex Book** — canonical long-form documentation at `https://bytestring-net.github.io/bevy_lunex/`. Mix of architecture overview, primitive reference, interactivity chapter. Currently targets v0.4+ — knowingly drifts from 0.6.0 due to maintainer bandwidth ("don't expect updates during the semester"). See [`critiques.md`](critiques.md) § "Documentation completeness."
- **Bevypunk** — `IDEDARY/Bevypunk`. The flagship demo, built by the bevy_lunex maintainer himself. Cyberpunk-themed UI showcase demonstrating the full feature set. 218 GitHub stars. A WASM build ships on itch.io with documented stutter caveats. **Not a shipped commercial product** despite the repo's "production ready example" framing.
- **`bevy_rich_text3d`** — optional dep (gated behind the `text3d` Cargo feature, default-on). Renders rich text directly in 3D worldspace. Same-author crate (IDEDARY ecosystem). The mechanism that makes worldspace text a first-class capability.
- **`bytestring-net`** — the GitHub organization (`https://github.com/bytestring-net`) owning bevy_lunex. Functionally a personal namespace for IDEDARY's work. No public members visible. Six repositories: `bevy_lunex`, `bevy_skybox_cli`, `blueprint`, `pathio`, `mathio`, `Utilities`.
- **`blueprint`** — `bytestring-net/blueprint`. Aspirational "ECS UI framework for applications built on top of Lunex" — the application-shaped layer above bevy_lunex. Very early (7 stars).
- **`IDEDARY`** — primary (and sole) maintainer. GitHub handle `1D3D4RY`. Czech Republic. Self-identified university student. Author of bevy_lunex, Bevypunk, blueprint. See [`governance.md`](governance.md).

## Architectural concepts

- **Transform-based positioning** — bevy_lunex's defining architectural bet. UI nodes are positioned by writing into Bevy's general-purpose `Transform` (not a bespoke `UiTransform`). The same coordinate system serves UI hit-testing, animation, audio positioning, and rendering. Trades flexbox/grid (which Taffy ships) for trivial 3D-anchored UI. **Contrasts with Taffy-based positioning** (bevy_ui's choice, Buiy's choice), where a separate layout engine produces resolved rectangles in a `ComputedNode`-style output component. See [`layout.md`](layout.md) and [`critiques.md`](critiques.md) § "Transform-based vs Taffy-based."
- **Taffy-based positioning** — the inverted choice. Feed layout fields to Taffy, get resolved rectangles back in a separate `ComputedNode`. Decouples authoring from layout solving. Buiy's commitment per foundation [architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md).
- **3D-anchored UI** — UI panels that live in a 3D scene, anchored to entities via `Transform`. bevy_lunex's biggest single differentiator. The pattern: `UiRoot3d` + `UiMeshPlane3d` markers, parent under any 3D entity, layout-and-render works the same as screen-space UI. See [`3d-and-worldspace.md`](3d-and-worldspace.md).
- **Worldspace UI** — synonym for 3D-anchored UI in bevy_lunex's vocabulary. The Lunex Book uses "worldspace" and "diegetic" interchangeably depending on context. Distinguished from screen-space UI (the default, where `UiFetchFromCamera` binds the root to a viewport).
- **Diegetic UI** — UI that lives "in" the game world — terminals, screens, holograms. A subset of worldspace UI where the in-world surface is itself part of the game's narrative reality. Supported via direct `UiRoot3d` + `UiMeshPlane3d` or via `UiEmbedding` (render-to-texture onto an arbitrary mesh).

## Sources

- bevy_lunex source — `crate/src/lib.rs`, `crate/src/layouts.rs`, `crate/src/states.rs`, `crate/src/cursor.rs`, `crate/src/units.rs`, `crate/src/picking.rs` (main branch, 2026-05-22)
- The Lunex Book — https://bytestring-net.github.io/bevy_lunex/
- docs.rs — https://docs.rs/bevy_lunex/0.6.0/bevy_lunex/
- bevy_lunex repo — https://github.com/bytestring-net/bevy-lunex
- Bevypunk — https://github.com/IDEDARY/Bevypunk
- bytestring-net org — https://github.com/bytestring-net
- bevy-ui glossary (shared terms) — [`../bevy-ui/glossary.md`](../bevy-ui/glossary.md)
- Sibling evidence files: [`architecture.md`](architecture.md), [`layout.md`](layout.md), [`component-model.md`](component-model.md), [`styling.md`](styling.md), [`3d-and-worldspace.md`](3d-and-worldspace.md), [`history.md`](history.md), [`distribution.md`](distribution.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`open-problems.md`](open-problems.md), [`comparisons.md`](comparisons.md)
