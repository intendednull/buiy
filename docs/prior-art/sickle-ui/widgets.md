**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — widget catalog: every widget shipped at 0.4.0, components, events, spawn extensions

# Widget catalog

sickle_ui ships a fixed catalog of ~30 widgets across three sub-modules of `sickle_ui::widgets::`: `inputs/`, `layout/`, `menus/`. Each widget exposes (a) a primary marker component, (b) sometimes a `Config` struct for construction parameters, (c) sometimes a state component (`PseudoStates`, slider value, dropdown selection), (d) zero or more event types, (e) a spawn extension trait `Ui<Name>Ext`. The pattern is uniform across the catalog — once you've seen the shape on one widget, every widget reads the same way.

This file is the verbatim inventory. For the spawn-API DSL pattern, see [`api.md`](api.md); for the underlying interaction state machines, see [`architecture.md` § FluxInteraction / DynamicStyle](architecture.md).

## Inputs (`sickle_ui::widgets::inputs::`)

Four widgets only.

### `checkbox`

- **Components:** `Checkbox`, `CheckboxPlugin`.
- **Events:** `CheckboxChanged` (fires when the toggle state flips).
- **Spawn extension:** `UiCheckboxExt::checkbox(label: Option<&str>, checked: bool) -> UiBuilder<Entity>`.
- **Pseudo-state contract:** `PseudoState::Checked` is added/removed when toggled. Themes key off it.
- **Keyboard:** not handled. There is no APG keyboard contract — pointer-only interaction. Space/Enter activation must be implemented by the app.
- **Tri-state:** **not supported.** No `aria-checked="mixed"` analog. Compare APG checkbox-tri-state pattern that Buiy commits to in foundation [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md).

### `slider`

- **Components:** `Slider`, `SliderBar`, `SliderDragHandle`, `SliderPlugin`.
- **Config:** `SliderConfig` (min, max, initial, step?, axis via `SliderAxis::Horizontal | Vertical`).
- **Events:** `SliderChanged`.
- **Spawn extension:** `UiSliderExt::slider(config: SliderConfig) -> UiBuilder<Entity>`.
- **Interaction:** drag-only via `drag_interaction` plumbing. No keyboard arrow-key increment/decrement, no Home/End to min/max, no Page-Up/Page-Down. Source span ~670 lines.
- **Multi-thumb:** not supported. Single thumb (`SliderDragHandle`) only.

### `radio_group`

- **Components:** `RadioGroup`, `RadioButton`, `RadioGroupPlugin`.
- **Events:** `RadioButtonChanged`.
- **Spawn extension:** `UiRadioGroupExt::radio_group(...) -> UiBuilder<Entity>`.
- **Keyboard:** none. APG radio-group arrow-key navigation (Up/Down/Left/Right cycling, with roving tabindex) is not implemented.

### `dropdown`

- **Components:** `Dropdown`, `DropdownOption`, `DropdownOptions`, `DropdownPanel`, `DropdownPanelPlacement`, `DropdownPlugin`.
- **Enums:** `DropdownPanelAnchor` (positioning hint for the popup).
- **Events:** `DropdownChanged`.
- **Spawn extension:** `UiDropdownExt::dropdown(...)`.
- **APG contract:** **not implemented.** APG `combobox` and `listbox` patterns require `aria-expanded`, `aria-activedescendant`, full keyboard navigation; none of these are provided. Source span ~890 lines but coverage is purely visual.

**Missing inputs vs APG / Buiy foundation:** no Switch (toggle), no Spinbutton (numeric stepper), no Searchbox, no Textbox / multi-line text input, no Combobox (the `Dropdown` is closer to a Bevy-native select-with-popup than to APG combobox), no Date picker, no Time picker, no Color picker (despite the `theme_colors` module shipping a palette, no widget surfaces it), no File picker.

## Layout (`sickle_ui::widgets::layout::`)

Thirteen widgets covering containers, panels, scroll surfaces, and dockable workspaces.

### `container`

- **Component:** `Container`.
- **Spawn extension:** `UiContainerExt::container(|ui| { ... }) -> UiBuilder<Entity>`.
- A generic styled `Node`. The minimal building block above raw `Node`.

### `column` / `row`

- **Components:** `Column`, `Row` (plus their plugins).
- **Spawn extensions:** `UiColumnExt::column(|ui| { ... })`, `UiRowExt::row(|ui| { ... })`.
- Flex containers preset to `FlexDirection::Column` / `Row`. The vast majority of sickle UI authoring is `column` and `row` calls nested via closures.

### `panel`

- **Components:** `Panel`, `PanelPlugin`.
- **Spawn extension:** `UiPanelExt::panel(...)`.
- A titled container — header bar + content area. The primary editor-style enclosure.

### `floating_panel`

- **Components:** `FloatingPanel`, `FloatingPanelConfig`, `FloatingPanelLayout`, `FloatingPanelDragHandle`, `FloatingPanelResizeHandle`, `FloatingPanelCloseButton`, `FloatingPanelTitle`, `FloatingPanelPlugin`.
- **Events / system markers:** `FloatingPanelUpdate`, `UpdateFloatingPanelPanelId`.
- **Spawn extension:** `UiFloatingPanelExt::floating_panel(...)`.
- The flagship editor widget: draggable, resizable, closable window inside the Bevy viewport. Not a true OS-level window — a Bevy entity rendered at floating coordinates with drag/resize plumbing.

### `foldable`

- **Components:** `Foldable`, plus the standard plugin.
- **Spawn extension:** `UiFoldableExt::foldable(label: &str, expanded: bool, |ui| { ... })`.
- A disclosure widget: header with chevron, content collapses/expands.
- **Pseudo-state contract:** `PseudoState::Open` / `PseudoState::Closed` toggle on click.
- **Keyboard:** none. APG disclosure pattern (Enter/Space to toggle) is not implemented.

### `scroll_view`

- **Components:** `ScrollView`, `ScrollViewViewport`, `ScrollViewContent`, `ScrollBar`, `ScrollBarHandle`, `ScrollViewPlugin`.
- **Spawn extension:** `UiScrollViewExt::scroll_view(scroll_axis: Option<ScrollAxis>, |ui| { ... })`.
- Drag-and-mouse-wheel scrolling. Keyboard scroll (Page-Up/Down, Home/End, arrows) is not implemented.

### `sized_zone`

- **Components:** `SizedZone`, `SizedZoneConfig`, `SizedZoneResizeHandle`, `SizedZoneResizeHandleContainer`, `SizedZonePreUpdate`, `SizedZonePlugin`.
- **Spawn extension:** `UiSizedZoneExt::sized_zone(config, |ui| { ... })`.
- A flex child with a draggable resize handle. The building block for splitter panes in editor layouts.

### `docking_zone`

- **Component:** `DockingZone` (+ plugin).
- **Spawn extension:** `UiDockingZoneExt::docking_zone(...)`.
- The drop target for `floating_panel` re-docking. Combined with `tab_container` and `sized_zone`, this is sickle's editor-workspace skeleton.

### `resize_handles`

- **Component:** generic resize-handle infrastructure consumed by `sized_zone` / `floating_panel`. Not user-spawned directly.

### `tab_container`

- **Components:** `Tab`, `TabContainer`, `TabBar`, `TabViewport`, `TabPlaceholder`, `CloseTabContextMenu`, `PopoutTabContextMenu`, `TabContainerPlugin`, `TabContainerUpdate`.
- **Spawn extensions:** `UiTabContainerExt::tab_container(...)`, `UiTabContainerSubExt`, `UiTabPlaceholderExt`.
- Tabbed interface with close-and-popout context menus on each tab.
- **APG contract:** **not implemented.** APG tabs pattern (auto-activate vs manual-activate variants, Home/End/arrow keyboard navigation, `aria-selected`, `aria-controls`) is absent.

### `label`

- **Components:** `Label`, `LabelConfig`, `LabelPlugin`.
- **Spawn extension:** `UiLabelExt::label(config: LabelConfig)`.
- A themed text node. Distinct from raw `Text` because it consumes the theme's typography tokens.

### `icon`

- **Component:** `Icon`, `IconPlugin`.
- **Spawn extension:** `UiIconExt::icon(...)`.
- Image-as-icon widget (sources from the theme's icon set).

## Menus (`sickle_ui::widgets::menus::`)

Nine widgets covering app menu bars, context menus, submenus, and toggle items.

### `menu`

- **Component:** `Menu` (+ plugin).
- **Spawn extension:** `UiMenuExt::menu(...)`.
- The root menu container.

### `menu_bar`

- **Component:** `MenuBar`.
- **Spawn extension:** `UiMenuBarExt::menu_bar(|ui| { ui.menu(...); ... })`.
- Horizontal bar of top-level menus (File / Edit / View style).

### `menu_item`

- **Component:** `MenuItem`.
- **Spawn extension:** `UiMenuItemExt::menu_item(label, shortcut?)`.
- A clickable row inside a menu.

### `toggle_menu_item`

- **Component:** `ToggleMenuItem`.
- **Spawn extension:** `UiToggleMenuItemExt::toggle_menu_item(...)`.
- Menu item with a checkbox-style toggle state.

### `submenu`

- **Component:** `Submenu`.
- **Spawn extension:** `UiSubmenuExt::submenu(label, |ui| { ... })`.
- Nested menu that opens a side popup on hover.

### `menu_separators`

- A purely visual divider widget for menu vertical separation.

### `context_menu`

- **Component:** `ContextMenu`.
- **Spawn extension:** `UiContextMenuExt::context_menu(...)`.
- Right-click-triggered menu. Position resolved on click.

### `shortcut`

- A keyboard-shortcut binding helper used by `menu_item`. Not a widget per se; provides the visual + binding plumbing.

### `extra_menu`

- An additional menu variant; the docs gloss is sparse. Likely the "more options" overflow menu for menu bars.

**Keyboard contracts in `menus/`:** sickle does provide the `shortcut` plumbing for binding keyboard shortcuts to menu items (Ctrl+S, etc.) and renders the shortcut hint in the item. But the **APG menubar pattern** — Tab to enter the menu bar, arrow-key navigation between top-level menus, Down to open, Esc to close, type-ahead — is not implemented. The menus are mouse-driven with bind-time keyboard shortcuts, not keyboard-navigable per APG.

## Widget coverage matrix vs Buiy foundation media-and-widgets

The Buiy foundation [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) catalogs the target widget set. Mapped against sickle:

| Buiy widget | Tier | sickle equivalent | Status |
|---|---|---|---|
| Button | F | (none — sickle uses raw styled containers + `FluxInteraction`) | **missing as named widget** |
| Link | F | — | missing |
| Text / Label | F | `label` | partial (no semantic levels) |
| Heading | F | — | missing |
| Image | F | `icon` (image-as-icon) | partial |
| Checkbox | F | `checkbox` | partial (no tri-state, no keyboard) |
| Switch | F | — | missing |
| Radio group | F | `radio_group` | partial (no keyboard) |
| Slider | F | `slider` | partial (no keyboard, single-thumb only) |
| Listbox | F | — | missing |
| Combobox | F | `dropdown` (close but not APG) | partial |
| Spinbutton | F | — | missing |
| Textbox | F | — | **missing** |
| Searchbox | F | — | missing |
| Menu / Menubar / Menu Button | F/C | `menu` / `menu_bar` / `menu_item` | partial (no APG keyboard) |
| Tabs | F | `tab_container` | partial (no APG keyboard) |
| Toolbar | C | — | missing |
| Dialog (modal/non-modal) | F | (apps build from `floating_panel`) | partial |
| Popover / Tooltip | F | (apps build from `floating_panel` + `flux`) | partial |
| Disclosure / Accordion | F | `foldable` | partial (no keyboard, no exclusive) |
| Window splitter | C | `sized_zone` | partial (no keyboard resize) |
| Progressbar / Meter | F/C | — | missing |
| Alert / Status / Toast | F | — | missing (no live-region plumbing at all) |
| Tree / Treegrid | C | — | missing |
| Table / Grid | C | — | missing |

The headline finding: sickle covers the **editor-tooling** subset (panels, tabs, docking, menus, sliders) competently and skips most of the **app-and-document** subset (text input, form controls, dialogs, alerts, live regions, tables). It is, accurately, a widget kit for "editors and utilities" — same scope claim `bevy_feathers` later inherited. See [`ecosystem.md` § "vs bevy_feathers"](ecosystem.md).

## Accessibility contract

**Zero AccessKit integration.** No `AccessibilityNode`, no role mapping, no accessible name source, no state propagation, no focus tree. Screen readers see sickle widgets as generic UI nodes. This is the largest single gap relative to Buiy's foundation goal 2 (`WCAG 2.2 AA is the floor`). See [`critiques.md` § "Accessibility absence"](critiques.md).

## Implications for Buiy

1. The widget catalog confirms: **third-party Bevy widget kits without official engine backing skew toward editor / tooling scope.** Same shape as `bevy_feathers`. App / form / document widgets are systematically underweighted because game developers who write widget kits don't ship text input or live regions. Buiy's foundation goal 6 (`Game and app, both`) deliberately fights this gravity.
2. Every sickle widget that *exists* has a **known APG-keyboard gap**. Each one would be a Buiy widget where the keyboard contract is the verifiable claim, not a TODO. Buiy's widget-catalog sub-spec should treat the APG contract as the load-bearing item, not as a polish layer.
3. The naming pattern `Ui<Name>Ext::<name>(...)` is the cost of the extension-trait DSL — see [`api.md`](api.md). Buiy's component-first authoring avoids this by making each widget a component (`Button`, `Slider`, etc.) and spawning via BSN / ECS, not via traits.
4. sickle's `tab_container` + `docking_zone` + `floating_panel` + `sized_zone` quartet is the **most ergonomic Rust expression of an editor docking layout in any Bevy library**. Buiy is unlikely to need this in v1 (Buiy targets game UI and app UI, not editor IDEs), but if Buiy ever ships editor primitives, the sickle composition pattern is worth re-reading. Note: there is no equivalent in `bevy_feathers`.

## Sources

- docs.rs catalog — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/index.html
- inputs/ — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/inputs/index.html
- layout/ — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/layout/index.html
- menus/ — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/menus/index.html
- checkbox — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/inputs/checkbox/index.html
- slider — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/inputs/slider/index.html
- dropdown — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/inputs/dropdown/index.html
- radio_group — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/inputs/radio_group/index.html
- scroll_view — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/layout/scroll_view/index.html
- floating_panel — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/layout/floating_panel/index.html
- tab_container — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/layout/tab_container/index.html
- sized_zone — https://docs.rs/sickle_ui/0.4.0/sickle_ui/widgets/layout/sized_zone/index.html
- Surviving fork README (capability summary) — https://github.com/UkoeHB/sickle_ui
- Buiy foundation widget catalog — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
