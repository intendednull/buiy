**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — the anchor + offset positioning model; LayoutMode in Godot 4; size flags; comparison to CSS box model

# Layout: anchors, offsets, and containers

Godot's layout model is **not** the CSS box model. It's a fractional-anchor + pixel-offset model that pre-dates the Flexbox / Grid era and reflects Godot's origin as a game-engine UI. The model has shipped since Godot 1.0 (2014) and is well-loved by long-time Godot users, but it is consistently a source of friction for developers coming from CSS / web.

## The primitive: four anchors + four offsets

Every Control has eight layout properties:

- **`anchor_left`, `anchor_top`, `anchor_right`, `anchor_bottom`** — floats in `[0.0, 1.0]`. Each picks a reference point inside the parent's rect.
  - `0.0` = the parent's left (for `anchor_left/right`) or top (for `anchor_top/bottom`).
  - `1.0` = the parent's right or bottom.
  - `0.5` = the parent's center.
- **`offset_left`, `offset_top`, `offset_right`, `offset_bottom`** — pixel distances from the corresponding anchor point.

The Control's actual rect is computed:

```
rect.x = anchor_left * parent.width + offset_left
rect.y = anchor_top * parent.height + offset_top
rect.width = (anchor_right * parent.width + offset_right) - rect.x
rect.height = (anchor_bottom * parent.height + offset_bottom) - rect.y
```

So `anchor_left=0, anchor_right=1, offset_left=10, offset_right=-10` produces a Control that's 20px narrower than its parent and recenters with parent resize — the equivalent of CSS `left: 10px; right: 10px;`.

(In Godot 3.x these were named `margin_left/top/right/bottom`; the rename to `offset_*` in 4.x avoids the CSS-margin connotation, since they're offsets from anchors, not collapsing margins.)

## LayoutMode (Godot 4.x)

Godot 4.x added the `LayoutMode` enum on Control to make the anchor+offset model approachable:

- **`POSITION`** — manual position + size; the editor sets anchors to `(0,0,0,0)` and exposes `position` / `size` directly.
- **`ANCHORS`** — the editor exposes the four anchors + offsets directly.
- **`CONTAINER`** — the Control is inside a Container; anchors/offsets are ignored (the Container will overwrite them each frame). The editor shows `size_flags_*` and `custom_minimum_size` instead.
- **`UNCONTROLLED`** — for nodes that aren't part of the GUI layout (e.g., a Control used as a 2D billboard).

`LayoutMode` is editor metadata, not a runtime distinction — the underlying math is always anchors + offsets. The mode toggles which inspector properties are exposed and which the editor's drag-handles manipulate.

## Layout presets

Anchors are confusing when written as four floats; Godot exposes "presets" — common configurations like:

- `Top Left`, `Top Right`, `Bottom Left`, `Bottom Right` — corner-pinned.
- `Center Top`, `Center Bottom`, `Center Left`, `Center Right`, `Center` — edge-or-center-pinned.
- `Top Wide`, `Bottom Wide`, `Left Wide`, `Right Wide`, `VCenter Wide`, `HCenter Wide` — pin to a stripe.
- `Full Rect` — fills the parent.

Each preset is a tuple of (anchors, offsets). The editor sets all eight at once. Source-of-truth remains the eight raw values.

## Containers override anchors

When a Control's parent is a Container subclass (HBoxContainer, VBoxContainer, GridContainer, FlowContainer, MarginContainer, CenterContainer, AspectRatioContainer, ScrollContainer, PanelContainer, TabContainer, SplitContainer), the parent overwrites the child's anchors and offsets each layout pass. The child's authored values are ignored.

What containers consult instead:

- `custom_minimum_size: Vector2` — minimum width / height in pixels.
- `size_flags_horizontal`, `size_flags_vertical` — bitflags consulted by the container:
  - `FILL` (1) — fill available space on this axis.
  - `EXPAND` (2) — also claim leftover space (proportional to `size_flags_stretch_ratio`).
  - `SHRINK_CENTER` (4) — when not filling, center within the slot.
  - `SHRINK_END` (8) — when not filling, align to the end.
- `size_flags_stretch_ratio: float` — when multiple children EXPAND, this ratio distributes leftover.

A child with `size_flags_horizontal = FILL | EXPAND` and `stretch_ratio = 2.0` claims twice as much leftover space as a sibling with `FILL | EXPAND` and `stretch_ratio = 1.0`. This is Godot's analogue to `flex: 2` vs `flex: 1`.

## Each container is a C++ class with its own algorithm

There is no generic layout solver. Each Container subclass implements `_notification(NOTIFICATION_SORT_CHILDREN)` directly:

- **`HBoxContainer::_notification`** — measure children, compute leftover, distribute by stretch ratio, lay out left-to-right.
- **`GridContainer::_notification`** — sort children into rows based on `columns`, compute per-column max widths and per-row max heights, lay out.
- **`FlowContainer::_notification`** — flow children into rows until the row is full, then wrap.

Adding a new layout algorithm to Godot means adding a new Container subclass — there's no extension point on a generic layout engine. This is a deliberate "keep it simple, build what we need" choice; the trade is "no third-party layout extensions" (you cannot ship a Taffy-style algorithmic plugin).

## What Godot's layout lacks vs CSS / Buiy

- **No flexbox proper.** HBoxContainer / VBoxContainer cover the main-axis arrangement, but no `flex-direction: column-reverse`, no `flex-wrap` (FlowContainer covers wrap but not the full Flexbox model), no `justify-content` / `align-items` keyword set, no `gap` (there's `separation` constant, but not gap-as-spec).
- **No CSS Grid.** GridContainer is a fixed-column rows-flow grid — no row / column span, no `grid-template-areas`, no `fr` units, no auto-placement, no subgrid.
- **No anchor positioning** (the CSS Anchor Positioning spec). Godot's "anchor" is fractional-anchor-to-parent; CSS anchor positioning is anchor-to-arbitrary-element. Different concepts despite the shared name.
- **No `position: sticky`.** ScrollContainer is the only overflow primitive; sticky headers are user-built via `_process()` repositioning.
- **No container queries.** Layouts don't react to their own container's size beyond the anchor+offset math.
- **No logical properties.** No `inline-size` / `block-size` / `padding-inline-*`; everything is left/top/right/bottom. The `mirror_layout` flag flips a Control horizontally for RTL but doesn't change the property model.
- **No writing modes.** Vertical text is via per-Control rotation; vertical layout (top-to-bottom for CJK) is not a layout-engine concept.
- **No multi-column** (`column-count`, `column-width`).
- **No table layout.** Tables are RichTextLabel BBCode `[table]` or Tree with multi-column rows.
- **No min/max-content / fit-content / stretch sizing keywords.** Sizes are pixels or percentages of parent via anchor differences.
- **No aspect-ratio property** (there's AspectRatioContainer as a workaround).
- **No box-sizing toggle.** Sizes are content sizes; padding is via MarginContainer or per-Control theme constants.

## Implications for Buiy

- **Validates Buiy's CSS-via-Taffy bet.** Godot is the long-running counter-example of "what if we don't do CSS?" The answer is "you ship, but every developer coming from web does a double-take." Buiy's foundation [`visuals.md § 3.2`](../../specs/2026-05-07-buiy-foundation/visuals.md) commits to the full CSS box model + Flexbox + Grid; the cost is a Taffy dependency, the benefit is no double-take.
- **Anchor + offset is not a bad model — it's a different model.** For top-level layout pinning ("HUD anchored to top-right of screen"), anchor + offset is *more direct* than CSS `position: absolute; top: 10px; right: 10px;`. Buiy's [`buiy-layout-design`](../../specs/2026-05-08-buiy-layout-design/) commits to CSS-mode primary; consider whether a "screen-anchor" affordance for game HUDs (where Godot-style anchoring is genuinely ergonomic) is worth a thin convenience layer on top of `position: fixed` + percentage offsets.
- **Per-container-as-a-class is rigid.** Buiy uses one Style builder + Taffy; new layout features (subgrid, container queries) extend the existing layout pass, not new component types. Validates the Taffy-driven approach.
- **`size_flags_horizontal/vertical` is a clean enum-based child→parent contract.** Buiy's foundation `visuals.md § 3.2` doesn't currently spec a child→parent flag system at this granularity (it uses CSS values directly); worth considering whether Buiy should expose a similar high-level enum for game-developer ergonomics.
- **Avoid Godot 3.x's `margin_*` naming.** Godot 4.x renamed to `offset_*` precisely because the CSS-margin connotation misled. Don't repeat the trap.

## Sources

- Size and anchors tutorial — https://docs.godotengine.org/en/stable/tutorials/ui/size_and_anchors.html
- Containers tutorial — https://docs.godotengine.org/en/stable/tutorials/ui/gui_containers.html
- Control class — Layout properties — https://docs.godotengine.org/en/stable/classes/class_control.html
- Container class — https://docs.godotengine.org/en/stable/classes/class_container.html
- BoxContainer (HBoxContainer / VBoxContainer) — https://docs.godotengine.org/en/stable/classes/class_boxcontainer.html
- GridContainer — https://docs.godotengine.org/en/stable/classes/class_gridcontainer.html
- `scene/gui/box_container.cpp`, `grid_container.cpp`, `flow_container.cpp` — https://github.com/godotengine/godot/tree/master/scene/gui/
- Buiy layout design — [`../../specs/2026-05-08-buiy-layout-design/README.md`](../../specs/2026-05-08-buiy-layout-design/README.md)
