**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — the Control-subclass widget vocabulary: buttons, text, lists, dialogs, color pickers, graph editor, containers

# Control hierarchy

Godot's `scene/gui/` directory ships ~60 Control subclasses on `master`. They cluster into seven groups; each group is a useful precedent for Buiy's widget catalog (foundation [`media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)).

## Button family

`BaseButton` is the abstract base — toggle / pressed state, `disabled`, `focus_mode`, `shortcut`, the `pressed` / `toggled` / `button_down` / `button_up` signals.

- **`Button`** — the standard pushbutton. Supports text + icon, `expand_icon`, `flat` (chromeless), `alignment`.
- **`CheckBox`** — labeled checkbox. Square indicator + label, three states (checked / unchecked / indeterminate via shader).
- **`CheckButton`** — toggle-switch styled checkbox. Same semantics as CheckBox, different visual.
- **`LinkButton`** — text-only "hyperlink" button. No StyleBox; underline on hover.
- **`OptionButton`** — dropdown / combobox. Holds an internal `PopupMenu` for the choices.
- **`MenuButton`** — button that opens a `PopupMenu` on press. Used for menu bars and dropdown menus.
- **`ColorPickerButton`** — button whose face shows a color swatch; opens an embedded `ColorPicker` popup.
- **`TextureButton`** — bitmap-driven button. Different textures per state (normal / pressed / hover / disabled / focus). Used heavily by games where the button visual is a fully-baked sprite.

## Text display

- **`Label`** — single-line or multi-line text, no editing. Properties: `text`, `horizontal_alignment`, `vertical_alignment`, `autowrap_mode`, `text_overrun_behavior` (truncate / ellipsis / wrap), `uppercase`, `clip_text`. Theme items: `font`, `font_size`, `font_color`, `outline_color`, `outline_size`, `shadow_color`, `shadow_offset_x/y`, `shadow_outline_size`.
- **`RichTextLabel`** — multi-paragraph rich text with BBCode markup. Supports inline images, tables, lists, custom effects (wave, shake, rainbow, fade, tornado). The closest Godot has to an HTML body. **Not editable.** See [`text-and-input.md`](text-and-input.md).

## Text editing

- **`LineEdit`** — single-line editable text. Properties: `text`, `placeholder_text`, `secret` (password mask), `secret_character`, `max_length`, `editable`, `caret_blink`, `select_all_on_focus`, `right_icon`, `clear_button_enabled`. Signals: `text_changed`, `text_submitted`, `text_change_rejected`.
- **`TextEdit`** — multi-line plain-text editor. Properties: `text`, `wrap_mode`, `scroll_smooth`, `minimap_draw`, `gutters`, `caret_multiple`, `selecting_enabled`, `drag_and_drop_selection_enabled`, `virtual_keyboard_enabled`. Has its own caret management, undo / redo stack, search, replace.
- **`CodeEdit`** — extends `TextEdit` with code-editor affordances: syntax highlighting hooks (`SyntaxHighlighter` resource), code completion, line-folding, bookmarks, breakpoints, indent-on-enter. This is the editor's own code-editing surface — Godot eats its own dog food here.

## Selection lists and trees

- **`ItemList`** — flat list of selectable items (text + optional icon). Supports single-select / multi-select, drag-and-drop reorder, `same_column_width`, `max_columns`. Used in the editor's FileSystem dock thumbnails.
- **`Tree`** — hierarchical tree with multi-column rows. Each `TreeItem` can have text, icons, custom cells (button / check / range / cell-renderer). Used in the editor's Scene dock, Inspector, FileSystem dock list view.
- **`PopupMenu`** — vertical menu of items (text + icon + shortcut + submenu). Used everywhere — context menus, dropdown menu, MenuButton's contents, OptionButton's choices.

## Progress / range / sliders

- **`Range`** (abstract) — base for any min / max / value control. Properties: `min_value`, `max_value`, `step`, `value`, `page`, `allow_greater`, `allow_lesser`, `rounded`, `exp_edit`, `ratio`.
- **`ProgressBar`** — display-only Range. Properties: `show_percentage`, `fill_mode` (LTR / RTL / TTB / BTT / radial / clockwise / counter-clockwise).
- **`HSlider` / `VSlider`** — interactive sliders. Tick marks, dragging, scrubbing.
- **`SpinBox`** — numeric input with up / down arrows + free typing. Built on top of `LineEdit` + buttons.
- **`ScrollBar`** (abstract) + **`HScrollBar` / `VScrollBar`** — scrollbar primitives. Used internally by `ScrollContainer`, `TextEdit`, `Tree`, `ItemList`. Standalone use is unusual.

## Containers — the layout primitives

A `Container` is a Control that lays out its Control children by overriding their anchors + offsets each layout pass. Each algorithm is a separate C++ class.

- **`BoxContainer`** (abstract) + **`HBoxContainer` / `VBoxContainer`** — flexbox-like main-axis layout. Honors child `size_flags_horizontal/vertical` (`FILL` / `EXPAND` / `SHRINK_CENTER` / `SHRINK_END`).
- **`GridContainer`** — fixed-column grid; children flow row-by-row. `columns` property is the only knob; no row / column span, no fr units, no auto placement (you order children in scene tree). Very different from CSS Grid — see [`layout-anchors-margins.md`](layout-anchors-margins.md).
- **`FlowContainer`** + **`HFlowContainer` / `VFlowContainer`** — flexbox `flex-wrap: wrap` analogue. Wraps children to new rows when they exceed parent's main axis.
- **`MarginContainer`** — adds padding to a single child via theme constants `margin_left/top/right/bottom`.
- **`CenterContainer`** — centers the single child without resizing it.
- **`AspectRatioContainer`** — preserves aspect ratio of children. `stretch_mode`: `WIDTH_CONTROLS_HEIGHT`, `HEIGHT_CONTROLS_WIDTH`, `FIT`, `COVER`.
- **`ScrollContainer`** — content larger than the container scrolls. Provides `HScrollBar` + `VScrollBar` automatically. Doubles as Godot's only "overflow" primitive.
- **`PanelContainer`** — wraps children in a `Panel`'s StyleBox background.
- **`TabContainer`** — tabs across the top; one child Control per tab. **The tabs are derived from child node names** — there is no separate "tab definition" — which is ergonomic but couples tab labels to scene-tree identity.
- **`SplitContainer`** + **`HSplitContainer` / `VSplitContainer`** — draggable splitter between exactly two child Controls.
- **`SubViewportContainer`** — embeds a child `SubViewport` (render-to-texture pipeline) into the UI; the SubViewport's content is rasterized and displayed as a UI element. Buiy's analogue is the [foundation §2.3 "render-to-texture surfaces"](../../specs/2026-05-07-buiy-foundation/architecture.md) commitment.

## Dialogs and popups

- **`Popup`** (abstract) — top-level transient container with click-outside-to-close.
- **`PopupPanel`** — Popup wrapped in a Panel StyleBox.
- **`PopupMenu`** — already covered above.
- **`Window`** (descends from `Viewport`, not `Control`, but commonly grouped here) — top-level OS window or embedded sub-window. Holds Control children.
- **`AcceptDialog`** — modal dialog with OK button. Base for `ConfirmationDialog` (adds Cancel) and `FileDialog`.
- **`FileDialog`** — file picker with mode `OPEN_FILE` / `OPEN_FILES` / `OPEN_DIR` / `OPEN_ANY` / `SAVE_FILE`. Native or Godot-rendered.
- **`ColorPicker`** — full HSV / RGB / OKHSL / OKHSV picker with palette + recent colors. Used heavily in the editor.

## Specialized

- **`Panel`** — a Control whose only job is to fill itself with a StyleBox. Used as a flat "card" background.
- **`ColorRect`** — a Control filled with a solid color. Cheaper than Panel for plain fills.
- **`TextureRect`** — image display Control. Properties: `texture`, `stretch_mode` (KEEP / SCALE / TILE / KEEP_CENTERED / KEEP_ASPECT / KEEP_ASPECT_CENTERED / KEEP_ASPECT_COVERED).
- **`NinePatchRect`** — 9-slice scaled texture. Used heavily for stretchy button backgrounds when bitmap-driven.
- **`VideoStreamPlayer`** — Control-embedded video playback.
- **`ReferenceRect`** — debug-only outline. Used in editor for layout debugging.
- **`Separator`** + **`HSeparator` / `VSeparator`** — thin dividing line.

## Graph editor

- **`GraphEdit`** — node-graph canvas with pan / zoom / connections. The visual scripting editor in Godot 3.x was built on this; in 4.x, third-party plugins (`Orchestrator`, `Block Coding`) and the shader graph editor are the heavy users.
- **`GraphNode`** — a node inside a `GraphEdit`. Has named input + output slots; connections are drawn between slots.
- **`GraphFrame`** — visual grouping in a GraphEdit. (Godot 4.3+.)
- **`GraphElement`** — abstract base for GraphNode + GraphFrame.

## What is *not* in the catalog

- **No date picker, time picker, or color-name picker.** Color is hex / RGB / HSV. Dates are user-built on top of SpinBox / LineEdit.
- **No autocomplete combobox.** OptionButton is a closed dropdown; type-to-search is in the engine, but a true `<datalist>`-style suggestion list is user-built.
- **No drag handle / sortable list out-of-box.** ItemList supports drag-reorder via `allow_reselect` + manual `_can_drop_data()` / `_drop_data()`; sortable list with affordances is user-built.
- **No data-grid / table editor with column resize / sort.** Tree comes closest but is hierarchical-first.
- **No virtual list / virtual table** for million-row scenarios. ItemList and Tree render all items eagerly; large lists need user-built virtualization.

## Implications for Buiy

- Naming and shape — `HBoxContainer` / `VBoxContainer` / `GridContainer` / `MarginContainer` / `CenterContainer` / `AspectRatioContainer` / `ScrollContainer` / `PanelContainer` — are clean, learnable, and battle-tested. Buiy's foundation [`media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) covers analogues; the Godot naming is worth studying for label choices even where Buiy's implementation differs (one Style builder vs many Container classes).
- The `Range`-abstract-base for "anything with min / max / value" (ProgressBar, HSlider, VSlider, SpinBox, ScrollBar) is a sound decomposition. Buiy's foundation `interaction.md` should consider a similar shared abstract for any min/max/value-bearing widget.
- The `TabContainer`-derives-tabs-from-child-node-names ergonomic is **clever for scene-tree authoring but couples identity** (renaming a tab changes the scene-graph path). Buiy's BSN-friendly model decouples tab identity from child entity name explicitly.
- `CodeEdit` is the most ambitious built-in Godot widget — it's the Godot script editor's substrate. Buiy's foundation doesn't commit to a code-edit widget at v1 ([`text.md`](../../specs/2026-05-07-buiy-foundation/text.md) defers rich-text editing to `buiy-text-editing-design`); CodeEdit's feature set is a good catalog of what "code-edit-grade" means at the widget level.
- Gaps Buiy explicitly does not punt on: date / time picker (foundation `media-and-widgets.md` § APG patterns), autocomplete combobox (APG `combobox` with `aria-autocomplete=list/both`), sortable lists (drag-and-drop primitives + a11y replacement contract per `buiy-input-events-design`), virtualized lists (foundation §3.1 + `buiy-widget-catalog-design`). Borrow the names, fill the gaps.

## Sources

- `scene/gui/` directory listing — https://github.com/godotengine/godot/tree/master/scene/gui/
- Control node gallery — https://docs.godotengine.org/en/stable/tutorials/ui/control_node_gallery.html
- BaseButton, Button, CheckBox, CheckButton, OptionButton, MenuButton class refs — https://docs.godotengine.org/en/stable/classes/class_basebutton.html etc.
- Container, BoxContainer, GridContainer, FlowContainer class refs — https://docs.godotengine.org/en/stable/classes/class_container.html etc.
- Tree, ItemList, PopupMenu class refs — https://docs.godotengine.org/en/stable/classes/class_tree.html etc.
- TextEdit, CodeEdit, RichTextLabel class refs — https://docs.godotengine.org/en/stable/classes/class_textedit.html etc.
- GraphEdit, GraphNode — https://docs.godotengine.org/en/stable/classes/class_graphedit.html
