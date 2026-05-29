**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — glossary of Godot-specific terms used across this corpus

# Glossary

Godot-specific terms used across this corpus, in alphabetical order. Cross-reference for readers familiar with one game engine's vocabulary but not Godot's.

## A

- **AccessKit** — Cross-platform Rust accessibility bridge (Windows UIA / macOS NSAccessibility / Linux AT-SPI). Adopted by Godot in **4.5** (September 2025). See [`accessibility.md`](accessibility.md).
- **Anchor** — In Godot Control, a float in `[0.0, 1.0]` picking a reference point inside the parent's rect. Each Control has four: `anchor_left`, `anchor_top`, `anchor_right`, `anchor_bottom`. **Not the same as CSS anchor positioning.** See [`layout-anchors-margins.md`](layout-anchors-margins.md).
- **AspectRatioContainer** — Container that maintains an aspect ratio for its children.
- **AT-SPI** — Linux Assistive Technology Service Provider Interface. The Linux accessibility tree protocol AccessKit (and Godot 4.5+) targets on Linux.

## B

- **BBCode** — Bulletin-board-style markup syntax used by `RichTextLabel`. Forum-software heritage from the 1990s. `[b]bold[/b]`, `[color=red]text[/color]`, `[url=...]link[/url]`. See [`text-and-input.md`](text-and-input.md).
- **BoxContainer** — Abstract container with linear (main-axis) layout. Concrete subclasses: `HBoxContainer` (horizontal), `VBoxContainer` (vertical).
- **bruvzg** — GitHub handle for **Pāvels Nadtočajevs**, contributor of both the 4.0 TextServer overhaul and the 4.5 AccessKit integration.

## C

- **CanvasItem** — Base class for anything drawing into 2D space. Parent of both Control (GUI) and Node2D (sprites, etc.). Provides `_draw()`, `modulate`, Z-index, visibility.
- **CenterContainer** — Container that centers a single child without resizing.
- **CodeEdit** — Multi-line plain-text editor with syntax-highlighting hooks, code completion, line folding, multi-caret, gutters. Extends `TextEdit`.
- **Container** — Subclass of Control that arranges its Control children by overwriting their anchors + offsets each layout pass. Each layout algorithm (HBox, VBox, Grid, Flow, Margin, Center, AspectRatio, Scroll, Panel, Tab, Split, SubViewport) is a separate concrete subclass.
- **Control** — Base class for all GUI elements. Inherits from `CanvasItem`. Provides anchors + offsets + focus + theme cascade + mouse_filter + drag-and-drop API. See [`architecture.md`](architecture.md).

## F

- **FlowContainer** — Container with wrap behavior (Flexbox `flex-wrap: wrap` analogue). `HFlowContainer` horizontal, `VFlowContainer` vertical.
- **Font** (Godot 4+) — Resource representing a typeface, **without** a size. FontSize is a separate theme item.
- **FontSize** (Godot 4+) — Integer theme item. Decoupled from Font in 4.0.
- **focus_neighbor_left/top/right/bottom** — Per-Control properties wiring the focus-navigation graph. Manual; auto-inference exists but is opt-in.

## G

- **GDExtension** — Godot 4+'s C ABI for native plugins (C++, Rust via `gdext`, Swift, Zig, etc.). Replaces GDNative from 3.x. ABI not yet fully stable across minor versions.
- **GDScript** — Godot's primary scripting language. Python-like, dynamic, integrated into the editor. The default authoring language for Godot UI.
- **Godot Foundation** — Dutch Stichting (non-profit foundation) formed November 2022. Holds the Godot trademark, employs core developers, manages donations + partnerships. See [`distribution-and-governance.md`](distribution-and-governance.md).
- **GraphEdit / GraphNode / GraphFrame** — Node-graph canvas primitives. Used by the shader graph editor and third-party visual-scripting plugins.
- **GridContainer** — Container with fixed-column row-flowing grid layout. Single `columns` property is the only knob; **not** CSS Grid (no span, no fr units, no auto-placement).

## H

- **HarfBuzz** — OpenType text-shaping library Godot uses (via `TextServerAdvanced`) for ligatures, complex scripts, BiDi shaping.
- **HBoxContainer / VBoxContainer** — Concrete BoxContainer subclasses. Flexbox main-axis-only analogue.

## I

- **ICU** — International Components for Unicode. Used by `TextServerAdvanced` for BiDi (UAX #9) and break iteration.
- **ItemList** — Flat selectable-list widget. Supports text + icons, single/multi-select, drag-and-drop reorder, multi-column display.

## J

- **Juan Linietsky** — Godot co-founder, Argentine software developer. GitHub: `@reduz`.

## L

- **LayoutMode** — Godot 4+ Control enum: `POSITION`, `ANCHORS`, `CONTAINER`, `UNCONTROLLED`. Editor metadata that controls which inspector properties are exposed. Does not change the runtime layout math (which is always anchors + offsets).
- **LineEdit** — Single-line editable text input. IME-aware (4.0+), undo/redo, secret mode, placeholder.
- **Linietsky** — Juan Linietsky, see above.

## M

- **Manzur** — Ariel Manzur, Godot co-founder, Argentine software developer. GitHub: `@punto-`.
- **MarginContainer** — Container adding padding to a single child via theme constants `margin_left/top/right/bottom`.
- **Margin (3.x)** — In Godot 3.x, the property name for pixel offsets from anchors. Renamed to `offset_*` in 4.x to disambiguate from CSS-margin connotation. See [`layout-anchors-margins.md`](layout-anchors-margins.md).
- **mouse_filter** — Per-Control enum (`STOP` / `PASS` / `IGNORE`) controlling whether input events stop, bubble to parent, or are ignored.

## N

- **NinePatchRect** — 9-slice-scaled bitmap Control. Used for stretchy backgrounds when bitmap-art-driven.

## O

- **OffsetLeft / OffsetTop / OffsetRight / OffsetBottom** — Pixel distances from each anchor point on a Control. The Godot 4 names; previously `margin_left/top/right/bottom` in 3.x.
- **OptionButton** — Dropdown / combobox widget. Holds an internal `PopupMenu` for choices.
- **Orca** — Linux GNOME screen reader. Was the only adapter that could partially see Godot UI pre-4.5 (and even then, severely limited).

## P

- **Panel** — Control that fills itself with a StyleBox background. Compare to ColorRect (solid color only).
- **PanelContainer** — Container that wraps children in a Panel's StyleBox background.
- **Popup** — Abstract top-level transient Control with click-outside-to-close. Concrete subclasses: PopupPanel, PopupMenu.
- **PopupMenu** — Vertical menu of items (text + icon + shortcut + submenu). Used by MenuButton, OptionButton, context menus.
- **PopupPanel** — Popup wrapped in a Panel's StyleBox.
- **Project Manager** — Godot's first-run / project-selection window. The most a11y-complete editor surface as of Godot 4.5.

## R

- **Range** — Abstract Control base for any min / max / value widget. Concrete subclasses: ProgressBar, HSlider, VSlider, SpinBox, ScrollBar.
- **Redot Engine** — Community fork of Godot launched late 2024 over Foundation-governance disputes. Active but small.
- **Resource (`.tres` / `.res`)** — Godot's serialized-data primitive. Theme, Font, StyleBox, Texture, Mesh, etc. all are Resources. Hot-reloadable via the editor's resource system.
- **RichTextLabel** — Multi-paragraph rich text Control with BBCode markup support. Display-only (not editable).
- **RTL** — Right-to-left text direction. Godot 4.0+ supports BiDi text via `TextServerAdvanced`.

## S

- **`scene/gui/`** — Source-tree directory in `godotengine/godot` containing the Control + Container + widget implementations. The artifact this prior-art folder is the synthesis of.
- **ScrollContainer** — Container providing scroll behavior for content larger than its rect. Godot's only overflow primitive.
- **`SHRINK_CENTER` / `SHRINK_END`** — `size_flags_horizontal/vertical` bitflag values for "when not filling, align center/end."
- **size_flags_horizontal / size_flags_vertical** — Per-Control bitflags consulted by Container parents: `FILL`, `EXPAND`, `SHRINK_CENTER`, `SHRINK_END`.
- **size_flags_stretch_ratio** — Per-Control float controlling how much leftover space an EXPAND-flagged child claims relative to siblings.
- **SplitContainer** — Container with draggable splitter between two child Controls. Concrete: HSplitContainer, VSplitContainer.
- **Stichting** — Dutch non-profit foundation legal form. The Godot Foundation is a Stichting.
- **StyleBox** — Abstract polymorphic resource describing a panel/button background. Concrete subclasses: `StyleBoxEmpty`, `StyleBoxFlat`, `StyleBoxTexture`, `StyleBoxLine`, `StyleBoxLinearGradient` (4.6+).
- **SubViewport** — Render-to-texture node. Embedded into a Control via `SubViewportContainer`.

## T

- **TabContainer** — Container with tabs across the top; one Control child per tab. Tab labels derive from child node names.
- **TextEdit** — Multi-line plain-text editor Control. Caret, undo/redo, selection, word-wrap, gutters.
- **TextServer** — Godot 4+ abstraction over text shaping, layout, and rasterization. Two implementations: `TextServerAdvanced` (HarfBuzz + ICU + FreeType, full BiDi + complex scripts) and `TextServerFallback` (minimal, Latin-only).
- **Theme** — Godot's UI skinning resource. Typed map keyed by `(theme_type, item_name) → value` across six item kinds (Color, Constant, Font, FontSize, Icon, StyleBox). Hot-reloadable. See [`theme-and-styling.md`](theme-and-styling.md).
- **theme_type_variation** — Per-Control property that switches which Theme type the Control queries for items. Used for Button variants without subclassing.
- **Tree** — Hierarchical tree Control with multi-column rows. Used in editor Scene dock, Inspector, FileSystem dock list view.
- **TreeItem** — Row in a Tree. Holds per-cell text + icons + custom widgets (button / check / range).

## V

- **Verschelde** — Rémi Verschelde, long-standing Godot core maintainer. Foundation-involved.
- **Viewport** — Godot's window / render-target primitive. Owns the GUI manager. Every window is a Viewport; SubViewport is also a Viewport.

## W

- **W4 Games** — Commercial company founded by ex-Godot core developers. Sells console-port runtimes (Switch / PS5 / Xbox) under commercial license. Not a fork.

## Sources

- Godot class reference index — https://docs.godotengine.org/en/stable/classes/index.html
- `scene/gui/` source — https://github.com/godotengine/godot/tree/master/scene/gui/
