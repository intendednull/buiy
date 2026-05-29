**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — Component model: shipped components, layout enum, BSN-friendliness, comparison to bevy_ui

# Component model

This file enumerates the components bevy_lunex 0.6.0 ships and evaluates them against the BSN-friendliness criteria Buiy commits to (small, public-fielded, observable, decomposed — [foundation architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)).

## Shipped components (verified against `crate/src/lib.rs` exports)

### Layout components

- **`UiLayoutRoot`** — marker component for the root of a UI tree. Spawning it begins layout for the subtree.
- **`UiRoot3d`** — marker component for a worldspace-3D UI root.
- **`UiLayout`** — the workhorse. Wraps a `UiLayoutType` plus per-state overrides. Read by the layout solver each frame.
- **`Dimension`** — output component holding resolved `Vec2 { x: width, y: height }`. Written by the solver.

### State components

- **`UiBase`** — default state marker (always present implicitly).
- **`UiHover`** — pointer-hover state. Carries `forward_speed`, `backward_speed`, `curve` (easing function), `instant: bool`, plus a private interpolated value.
- **`UiClicked`** — click state. **WIP** per source comments.
- **`UiSelected`** — selection state. **WIP**.
- **`UiIntro`** / **`UiOutro`** — entry/exit transitions. **WIP**.
- **`UiState`** — aggregates state-transition values for a node.

Each state implements `UiStateTrait` exposing a normalized `value() -> f32`. The `UiLayout` component carries per-state property variants; the solver picks the right one based on the active state's interpolated value. Verified in `crate/src/states.rs`.

### Visual components

- **`UiColor`** — color per state. Supports state-driven color transitions (e.g. base → hover).
- **`UiDepth`** — overrides Z-axis stacking. Two modes per source: absolute Z value or local-to-parent.
- **`UiEmbedding`** — marks an entity as carrying a resizable texture embedding (used for render-to-texture surfaces).
- **`UiTextSize`** — scales text relative to other nodes; bridges text size into the layout unit system.
- **`UiImageSize`** — analogous for images.
- **`UiMeshPlane2d`** / **`UiMeshPlane3d`** — markers requesting reconstruction of a quad mesh sized to the node's `Dimension`. The 3D variant is what enables worldspace UI panels with materials.

### Camera / viewport components

- **`UiFetchFromCamera<const INDEX>`** — binds the root rectangle to the viewport of a camera with the given index. The `INDEX` generic supports multi-camera setups; the `UiLunexIndexPlugin<INDEX>` plugin instantiates a copy of the solver per camera index.
- **`UiSourceCamera<const INDEX>`** — marks a camera as a source for index `INDEX`.

### Interaction components

- **`OnHoverSetCursor`** — requests a cursor icon while a node is hovered (verified in `cursor.rs`).
- **`NoLunexPicking`** — opt-out per-entity from the bevy_picking backend.

### Cursor components (the "software cursor" system)

- **`SoftwareCursor`** — virtual cursor entity with priority + atlas mapping.
- **`GamepadCursor`** — drives a `SoftwareCursor` from gamepad input (free or snap modes).
- **`CursorIconQueue`** — resource that arbitrates cursor-icon requests across pointers by priority.

## The `UiLayoutType` enum

This is the central layout shape. Three variants, each a small struct:

```rust
// (paraphrased from crate/src/layouts.rs; field names verbatim)
enum UiLayoutType {
    Window(UiLayoutTypeWindow),     // pos + size + anchor
    Solid(UiLayoutTypeSolid),       // aspect-ratio-locked, with alignment
    Boundary(UiLayoutTypeBoundary), // two-corner positioning
}
```

- **Window**: `pos: UiValue<Vec2>`, `anchor: Anchor`, `size: UiValue<Vec2>`. The classic "place a rect at an anchor relative to parent center."
- **Solid**: `size: UiValue<Vec2>` (interpreted as an aspect ratio, e.g. 16:9), `align_x: Align`, `align_y: Align`, `scaling: Scaling`. Scaling is one of `HorFill`, `VerFill`, `Fit` (default), `Fill`. Solid is what makes "any aspect ratio" claims tractable — it preserves the ratio across window resizes.
- **Boundary**: `pos1` (top-left), `pos2` (bottom-right). Size is the delta.

`UiValue<T>` is a composite holding optional `Ab` / `Rl` / `Rw` / `Rh` / `Vp` / `Vw` / `Vh` / `Em` values per axis, summed at `compute()` time. See [`layout.md`](layout.md) § "Units" for the unit system.

This is **a much smaller and more rigid model than CSS-style flexbox/grid**. There is no flow, no gap, no grid track sizing, no flex-basis. A bevy_lunex UI built in only these three primitives is constructed by explicitly placing each node, often as relative percentages of a parent — closer to absolute positioning + manual layout math than to a layout *engine* in the CSS sense.

## BSN-compatibility status

**Verdict: structurally BSN-friendly, with one large reservation.**

What's good:

- All listed components derive `Component` and `Reflect` (verified by docs.rs).
- Fields are public (verified by reading `layouts.rs`, `states.rs`).
- Decomposition pattern is comparable to bevy_ui's: color, depth, mesh, layout are separate components on the same entity (not megacomponents). This is the BSN-friendly shape Buiy commits to and matches the post-#17644 direction in bevy_ui ([bevy-ui lessons.md Avoid #1](../bevy-ui/lessons.md)).
- `UiLayout` carries per-state variants (base / hover / clicked / selected) as separate fields, which a BSN template could patch independently.

What's not good:

- `UiHover` and other state components mix public configuration (`forward_speed`, `curve`) with a private interpolated value. The private field is fine — it's solver state — but it means the component is *not* purely declarative, which BSN templates assume.
- The `Anchor` and `Align` types use sentinel constants (e.g. `Align::CENTER = 0.0`) rather than named variants. Less BSN-ergonomic; would benefit from being an enum.
- BSN itself has not landed in Bevy yet ([PR #20158](https://github.com/bevyengine/bevy/pull/20158) is still draft per [bevy-ui lessons.md "Top of file"](../bevy-ui/lessons.md)) — so "BSN-compatibility" of any Bevy UI library is a forward-looking judgment, not a tested fact.

bevy_lunex 0.6.0 ships against Bevy 0.18, the same major version the BSN PR is being prepared against. If BSN lands in 0.19 or later, bevy_lunex's current shape will need only minor adjustments (the WIP state-component reorganization) to be BSN-authorable.

## Comparison to bevy_ui's component shape

| Axis | bevy_ui (0.18) | bevy_lunex (0.6.0) |
|---|---|---|
| Root component | `Node` (carries layout fields) | `UiLayoutRoot` (marker only) |
| Layout input | `Node` fields (display, flex, grid…) | `UiLayout::layout` enum |
| Layout output | `ComputedNode`, `UiTransform`, `UiGlobalTransform` | `Transform`, `GlobalTransform`, `Dimension` |
| Color/decoration | `BackgroundColor`, `BorderColor`, `BorderRadius`, `Outline`, gradients, shadows — separate components | `UiColor` (per-state) — single component |
| Z-stacking | `ZIndex`, `GlobalZIndex` | `UiDepth` |
| Visibility short-circuit | `Visibility` + `Display::None` | `Visibility` |

Key observations:

- bevy_lunex collapses the input/output split that bevy_ui exposes through `Node` / `ComputedNode`. bevy_lunex's `UiLayout` is read; `Transform` and `Dimension` are written. The two-component split bevy_ui uses (input vs output) — borrowed by Buiy ([bevy-ui lessons.md Borrow #2](../bevy-ui/lessons.md)) — is absent in bevy_lunex.
- bevy_lunex reuses Bevy's general `Transform` / `Visibility` instead of UI-specific `UiTransform` / `ViewVisibility`-specific clones. This is the load-bearing simplification that makes worldspace UI free.
- bevy_lunex has fewer decoration components (just `UiColor`) where bevy_ui has many. Less surface, less flexibility, more retreating to "use a sprite with a material."

For Buiy's purposes the lesson is: **bevy_lunex demonstrates that a UI-on-Bevy-Transform model is viable as long as you accept the consequences** (no compositor → no clip-path, no backdrop-filter; no flexbox → manual positioning). Buiy's foundation rejects those consequences for the web-platform-parity goal. The comparison validates the *parallel-stack* idea (you can build a UI library without inheriting bevy_ui's renderer) while diverging on the *what to build* question.

## Sources

- `crate/src/lib.rs`, `crate/src/layouts.rs`, `crate/src/states.rs`, `crate/src/cursor.rs`, `crate/src/units.rs` (main branch, 2026-05-22)
- docs.rs — https://docs.rs/bevy_lunex/0.6.0/bevy_lunex/
- Buiy foundation — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy-ui prior-art — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- BSN PR — https://github.com/bevyengine/bevy/pull/20158
- bevy_a11y BSN-hostility issue — https://github.com/bevyengine/bevy/issues/17644
