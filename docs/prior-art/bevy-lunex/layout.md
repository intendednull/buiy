**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — Layout: transform-based solver, layout primitives, units, comparison to Taffy/CSS

# Layout

This file covers how bevy_lunex's layout algorithm actually computes positions, and where it sits in the broader design space relative to Taffy and CSS.

## The transform-based layout algorithm

bevy_lunex's layout is a single recursive system that walks the UI hierarchy in `PostUpdate` (set `UiSystems::Compute`, bracketed by `PreCompute` and `PostCompute`).

For each node carrying `UiLayout`, the solver:

1. Looks up the parent's resolved rectangle (the parent's `Transform.translation` + `Dimension`).
2. Resolves the node's `UiLayout` for the **current active state** — by interpolating between `UiLayout::base` and any state variants (e.g. `UiLayout::hover`) using `UiState::value()` as the lerp parameter.
3. Calls `UiLayoutType::compute(parent_rect, abs_scale, viewport_size, font_size)`.
4. Writes the result into `Transform.translation` (position) and `Dimension` (size).
5. Recurses into children.

There is no separate "build a layout tree, hand to a solver, read back computed boxes" round-trip. The layout *is* the placement. Verified in `crate/src/layouts.rs`.

This works because the layout primitives are *self-contained per node* — each node's position depends only on its parent's rectangle, not on its siblings. There is no flex-line solving, no grid-track sizing, no margin collapse — because none of those exist in bevy_lunex.

## Layout primitives

Three variants of `UiLayoutType` cover the whole surface. See [`component-model.md`](component-model.md) § "The `UiLayoutType` enum" for fields; this section focuses on layout *semantics*.

### `Window`

The default. `pos` + `size` + `anchor`. The anchor is which point of the node lands at `pos` (top-left, center, bottom-right, etc.). Conceptually: `position: absolute; left: pos.x; top: pos.y; transform: translate(-anchor.x*100%, -anchor.y*100%);`.

This is the workhorse — most production bevy_lunex layouts are stacks of Windows nested under a root.

### `Solid`

Aspect-ratio-locked. `size` is interpreted as a ratio (e.g. `Vec2::new(16.0, 9.0)` for 16:9), and the node is sized to fit inside its parent according to the `scaling` mode:

- `Fit` (default): scale down until both axes fit; honors `align_x` / `align_y` for placement of the smaller axis.
- `Fill`: scale up until both axes fill; one axis overflows.
- `HorFill` / `VerFill`: scale to match one axis exactly.

`Align` is a wrapped `f32` with constants `START = -1.0`, `CENTER = 0.0`, `END = 1.0`. The multiplier-style API is more cryptic than CSS `align-items` keywords but trivially mapped.

Solid is bevy_lunex's answer to the "any aspect ratio" claim — a UI built with Solid roots keeps its visual proportions across all window sizes, with letterboxing controlled by `align_x` / `align_y`.

### `Boundary`

Two corners: `pos1` (top-left) and `pos2` (bottom-right). Size = `pos2 - pos1`. Useful for stretchy regions defined by their bounds rather than their dimensions.

Conceptually identical to CSS `inset: 0 0 0 0;` with arbitrary corner offsets.

## Anchor / origin / pivot

Bevy's `bevy_sprite::Anchor` type is reused — Bottom, BottomLeft, BottomRight, Center, CenterLeft, CenterRight, TopCenter, TopLeft, TopRight, plus `Custom(Vec2)`.

There is no separate `transform-origin` analog for rotations; rotations are not part of bevy_lunex's layout model. To rotate UI you set `Transform.rotation` directly and accept that layout computation doesn't account for rotated bounding boxes.

## Relative vs absolute positioning

All bevy_lunex layout is **always relative to the immediate parent**. There is no `position: absolute` analog that escapes to the nearest positioned ancestor — every node is positioned in its parent's coordinate space. To escape, you reparent.

This is much more rigid than CSS positioning but maps directly to Bevy's `Parent` / `Children` hierarchy. The layout solver does not have to track containing blocks, stacking contexts, or `position`-affected ancestor chains.

## Units

bevy_lunex's `UiValue<T>` is a sum-typed container holding optional values across multiple unit systems. Verified in `crate/src/units.rs`:

| Unit | Meaning |
|---|---|
| `Ab(f32)` | Absolute pixels (`1Ab = 1px` at default scale). |
| `Rl(f32)` | Relative to parent (% of parent dimension). |
| `Rw(f32)` | Relative to parent **width** specifically (% of width, applied to either axis). |
| `Rh(f32)` | Relative to parent **height** specifically (% of height, applied to either axis). |
| `Vp(f32)` | Relative to viewport. |
| `Vw(f32)` | Relative to viewport width. |
| `Vh(f32)` | Relative to viewport height. |
| `Em(f32)` | Relative to font size (`1em = 1 * font_size`). |

`UiValue<T>` supports `Ab(4.0) + Em(1.0)` — mixed-unit arithmetic, summed at resolve time. This is a clean shape and analogous to CSS `calc(4px + 1em)`.

What's missing vs CSS: `rem` (root em — bevy_lunex has no document-root font-size analog), `ch` / `ex` / `lh` / `cap` / `ic`, container-query units (`cqw` / `cqh`), and `fr` (grid fractions — moot, no grid).

## Scaling and DPI

DPI handling routes through Bevy's existing scale-factor plumbing — the root rectangle from `UiFetchFromCamera` is in physical pixels, and `Ab` is interpreted as physical pixels post-scale. There is **no explicit DPI awareness in bevy_lunex itself**; it inherits whatever Bevy + winit report.

For UI that should remain visually consistent across DPI changes, the Solid + aspect-ratio approach is the idiomatic recipe. Mix-and-match `Ab` with `Rl` / `Vp` units in the same UI and you'll get DPI-dependent layouts.

## Animations and transitions

bevy_lunex's animation surface is narrow but coherent. The state system (`UiHover`, etc. — see [`component-model.md`](component-model.md) § "State components") drives smooth transitions between per-state layout variants.

Mechanism:

1. `UiLayout` carries `base: UiLayoutType` plus optional per-state overrides (`hover: Option<UiLayoutType>`, etc.).
2. State component (e.g. `UiHover`) holds an interpolated value 0.0..1.0 with configurable `forward_speed`, `backward_speed`, and `curve` (easing function).
3. The `system_state_hover_update` system in `Update` advances the value each frame.
4. The layout solver in `PostUpdate` `Compute` lerps between base and hover layouts using that value.

This is per-property transition (only layout properties — bevy_lunex doesn't ship transition support for arbitrary components). `UiColor` carries the same per-state structure and lerps colors.

What's not here: keyframe animations, layout transitions in the FLIP sense, spring physics, animation timelines, scroll-driven animations, view transitions. Reduced-motion gating is not built in — the application must check the OS preference itself.

## Comparison to Taffy / CSS box model

| Feature | CSS / Taffy | bevy_lunex |
|---|---|---|
| Flexbox | Full | **None** |
| CSS Grid | Full | **None** |
| Block flow, margin collapse | Yes | **None** |
| Box model (content/padding/border/margin) | Yes | **None** — there is `size` only |
| `aspect-ratio` | Yes | Yes (via `Solid`) |
| `min/max-width/height` | Yes | **None** |
| `position: absolute` containing-block resolution | Yes | N/A (always parent-relative) |
| Anchor positioning | CSS draft, Taffy roadmap | **None** |
| Container queries | CSS, Taffy roadmap | **None** |
| Logical properties (`inline-size`, `block-size`) | Yes | **None** — axes are physical x/y only |
| Writing modes | Yes | **None** |
| Scroll containers, overflow, scroll-snap | Yes | **None** built in |
| Per-state property transitions | Via CSS `:hover` + transition | **Yes** (per state component) |
| Aspect-ratio-locked container | `aspect-ratio` + `object-fit` | **First-class** (`UiLayoutTypeSolid`) |

The Lunex Book is explicit about this — quoting the introduction: "provides you with only capability to position entities" and explicitly noting flexbox is not present. This is design, not gap.

The most charitable framing: bevy_lunex is a **game-HUD layout primitive** (which is exactly what early 2D-game UIs need — place a healthbar at top-left, an inventory grid in the middle, a minimap top-right) plus a 3D-anchored extension. It is **not** a CSS-replacement. Pulling it toward CSS-parity would require essentially rewriting the layout solver.

**Implication for Buiy.** Buiy's foundation commits to Taffy + container queries + anchor positioning + logical properties ([visuals.md § 3.2](../../specs/2026-05-07-buiy-foundation/visuals.md)). The bevy_lunex comparison validates that those features are *not* a foregone conclusion at the engine-UI layer — a real shipping alternative explicitly rejects them. Buiy's bet is that web-platform-parity is what users will want; bevy_lunex's bet is that simple aspect-ratio-aware positioning is what game-UI authors want. Both bets have evidence — see [bevy-ui lessons.md](../bevy-ui/lessons.md) § "Game and app, both" framing.

## Sources

- `crate/src/layouts.rs`, `crate/src/units.rs`, `crate/src/states.rs` (main branch, 2026-05-22)
- docs.rs — https://docs.rs/bevy_lunex/0.6.0/bevy_lunex/
- The Lunex Book — https://bytestring-net.github.io/bevy_lunex/
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- bevy-ui lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Taffy — https://github.com/DioxusLabs/taffy
