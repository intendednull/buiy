**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — architecture: Control as base UI node, scene-tree integration, CanvasItem rendering, the GUI subsystem

# Architecture

## The inheritance chain

```
Object < Node < CanvasItem < Control
```

- **`Object`** — Godot's root reflection-aware type. Provides signals, properties, and the `_notification()` callback.
- **`Node`** — scene-tree participant. Parent / child / sibling, `_process()` / `_physics_process()` / `_ready()` lifecycle, group membership.
- **`CanvasItem`** — anything that draws into 2D space. Provides the `_draw()` virtual, `modulate` / `self_modulate` color multipliers, visibility, Z-index, the 2D canvas transform. **Sprite2D and TileMap also descend from CanvasItem** — Control is the GUI specialization, not the only 2D drawer.
- **`Control`** — adds the GUI half: anchors, offsets, size, focus, mouse filter, theme cascade, drag-and-drop, the `_gui_input()` callback.

This stack matters because **Control is not the only 2D thing in Godot** — it's the GUI-specialized branch of a broader 2D drawing system. The renderer doesn't know about UI vs sprite; both walk the same CanvasItem tree.

## The GUI subsystem

The actual UI orchestration lives in `Viewport`'s GUI manager (`scene/main/viewport.cpp` + `scene/gui/`). Each `Viewport` (which every window is) owns:

- A focus pointer (which Control currently has focus).
- A hover pointer (which Control the mouse is currently over).
- A drag-and-drop state machine.
- An input event router that walks the Control subtree top-down for `_gui_input()` handlers.
- A tooltip timer.
- A modal stack (popups, dialogs).

Control nodes register with their containing `Viewport` on `_enter_tree()`. The GUI manager translates raw `InputEvent`s from the OS layer into per-Control `_gui_input()` calls and `gui_input` signals, respecting the `mouse_filter` enum (`STOP` / `PASS` / `IGNORE`).

## Anchors, offsets, and the layout output

A Control's resolved geometry is computed from:

- `anchor_left`, `anchor_top`, `anchor_right`, `anchor_bottom` — floats in `[0.0, 1.0]` that pick a reference point in the parent's rect.
- `offset_left`, `offset_top`, `offset_right`, `offset_bottom` — pixel distances from those anchor points. (These were named `margin_*` in Godot 3.x and renamed to `offset_*` in 4.x to disambiguate from the CSS-margin connotation.)
- `custom_minimum_size` — a `Vector2` minimum that auto-layout containers respect.
- `size_flags_horizontal`, `size_flags_vertical` — bitflags consulted by containers (`FILL`, `EXPAND`, `SHRINK_CENTER`, `SHRINK_END`).
- `grow_horizontal`, `grow_vertical` — which direction the rect grows when the anchor edges resolve inside-out.

When a Control's parent is **also a Control but not a Container**, anchors + offsets directly determine the rect. When the parent is a Container (HBoxContainer / VBoxContainer / GridContainer / etc.), the container overwrites anchors + offsets each layout pass — the child's authored values are ignored and the container's algorithm wins. See [`layout-anchors-margins.md`](layout-anchors-margins.md).

## Drawing: the `_draw()` callback

Control rendering happens through CanvasItem's `_draw()` virtual. Built-in controls implement their visual via this hook — `Button::_draw()` calls `draw_style_box()`, `draw_string()`, `draw_texture_rect()` against the theme-resolved StyleBox / font / icon. Custom controls override `_draw()` and call the same Canvas drawing primitives (`draw_rect`, `draw_line`, `draw_polygon`, `draw_texture_rect`, `draw_string`, `draw_set_transform`, `draw_arc`, etc.).

The drawing is **immediate inside `_draw()`, retained at the Canvas level.** A Control queues a redraw via `queue_redraw()`; the canvas server caches the resulting draw commands and replays them each frame until something marks the Control dirty again.

This is functionally similar to bevy_ui's render-extract pipeline but expressed through a synchronous callback rather than ECS systems.

## Theme cascade

A Control resolves theme items through this lookup chain (per [`theme-and-styling.md`](theme-and-styling.md)):

1. Local override on the Control itself (`add_theme_color_override`, `add_theme_stylebox_override`, etc.).
2. Custom `Theme` resource on the Control or any ancestor Control, walking up.
3. Project-wide default theme (`Project Settings > GUI > Theme > Custom`).
4. Built-in fallback theme compiled into the engine.

The lookup happens lazily during `_draw()` via `get_theme_color()` / `get_theme_stylebox()` / `get_theme_font()` / `get_theme_font_size()` / `get_theme_icon()` / `get_theme_constant()`. Each call walks the chain on miss; cache invalidation on theme change is driven by signals.

## Input flow

OS event → `Viewport` → GUI manager:

1. The manager walks the Control subtree in reverse Z-order, top-down hit-testing using each Control's rect + `_clips_input()` callback for non-rectangular hit shapes.
2. The first Control whose `mouse_filter != IGNORE` and whose rect contains the event point becomes the candidate.
3. `gui_input` signal + `_gui_input()` virtual fire on the candidate.
4. If the candidate's `mouse_filter == PASS`, the event bubbles to the parent Control; `STOP` halts propagation.
5. Focus changes (Tab / Shift+Tab / arrow keys per `focus_neighbor_*` properties) re-target keyboard events.

Keyboard navigation is **manually configured** by default — each Control has `focus_neighbor_left/top/right/bottom` properties that you can wire in the editor or set in code. Godot 4.x added auto-neighbor inference algorithms but the manual wires remain the canonical mechanism.

## Custom controls

Subclassing `Control` is the supported extension mechanism:

```gdscript
extends Control

func _draw():
    draw_rect(Rect2(Vector2.ZERO, size), Color.RED)

func _gui_input(event):
    if event is InputEventMouseButton and event.pressed:
        # handle click
        pass

func _get_minimum_size():
    return Vector2(100, 50)
```

The same applies in C# (`public partial class MyControl : Control`) and GDExtension (Rust via `gdext`). The contract is small: implement `_draw()`, override `_gui_input()` for input, override `_get_minimum_size()` for container cooperation, emit signals for state changes.

## What Control *does not* own

Godot's UI architecture deliberately keeps a few things outside the Control hierarchy:

- **No layout cache** like Taffy. Each Container subclass implements its own layout algorithm directly in C++. Adding a new layout (e.g., true CSS Grid) means writing a new Container subclass — not extending a generic layout solver.
- **No CSS-style stylesheet selectors.** Theme items are looked up by `(type_name, item_name)` tuples — e.g., `("Button", "normal")` returns the normal StyleBox for buttons. There is no `.btn.primary:hover` selector syntax.
- **No reactive data binding.** Properties are imperatively set; signals fire on change. Two-way bindings are user-built on top.
- **No accessibility tree** before Godot 4.5. The 4.5 AccessKit integration is the first attempt — see [`accessibility.md`](accessibility.md).

## Implications for Buiy

- Buiy's foundation [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to owning the render pipeline; Godot has owned its renderer for 12 years and ships across desktop / mobile / web / consoles. Validates the build-it-ourselves bet at the engine scale.
- Godot's `_draw()` callback is the inversion of Buiy's "components describe; ECS systems render" pattern. Bevy's ECS gives Buiy declarative-rendering for free; Godot pays the imperative-rendering tax in every custom control. Validates the ECS substrate choice.
- Godot Container subclasses (HBoxContainer / VBoxContainer / GridContainer) implement layout in C++, one class per algorithm. Taffy + Buiy components compose CSS Flex + Grid + Block on one `Style` builder. Buiy's design is more expressive at the cost of taking Taffy as a dependency — see [`layout-anchors-margins.md`](layout-anchors-margins.md).
- Godot's *no layout cache* + *no stylesheet* + *no reactivity* are all "we keep it simple, you build the rest" choices. Buiy commits to all three (Taffy cache, token tokens with hot reload, observers + change detection) as first-class. Different goals; the divergence is intentional.

## Sources

- Control class reference — https://docs.godotengine.org/en/stable/classes/class_control.html
- CanvasItem class reference — https://docs.godotengine.org/en/stable/classes/class_canvasitem.html
- Viewport class reference (GUI manager lives here) — https://docs.godotengine.org/en/stable/classes/class_viewport.html
- Custom drawing in 2D tutorial — https://docs.godotengine.org/en/stable/tutorials/2d/custom_drawing_in_2d.html
- `scene/gui/control.h` — https://github.com/godotengine/godot/blob/master/scene/gui/control.h
- `scene/main/viewport.cpp` GUI manager — https://github.com/godotengine/godot/blob/master/scene/main/viewport.cpp
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
