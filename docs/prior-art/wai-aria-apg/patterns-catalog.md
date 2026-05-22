**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — full catalog of the 32 widget design patterns with keyboard contract, ARIA role + state + property emission, and accessible name/description sourcing per pattern

# APG patterns catalog

This is the **lookup reference**. For every Buiy widget that maps to an APG pattern, find its row here, then cross-reference [`keyboard-contracts.md`](keyboard-contracts.md), [`roles-states-properties.md`](roles-states-properties.md), and [`name-computation.md`](name-computation.md) for cross-cutting details. The 32 patterns are enumerated as published at <https://www.w3.org/WAI/ARIA/apg/patterns/>.

The "Buiy widget" column names the Buiy widget that implements the pattern (from [`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)); the **tier** column repeats the Buiy foundation tier (F = foundation, C = core, E = extended).

## Index of patterns

1. [Accordion](#1-accordion-sections-with-showhide-functionality) — F
2. [Alert](#2-alert) — F
3. [Alert Dialog](#3-alert-dialog) — F
4. [Breadcrumb](#4-breadcrumb) — C
5. [Button](#5-button) — F
6. [Carousel](#6-carousel-slide-show-or-image-rotator) — C
7. [Checkbox (Dual-State + Tri-State)](#7-checkbox) — F
8. [Combobox](#8-combobox) — F
9. [Dialog (Modal)](#9-dialog-modal) — F
10. [Disclosure (Show/Hide)](#10-disclosure-showhide) — F
11. [Feed](#11-feed) — C
12. [Grid (Interactive Tabular Data and Layout Containers)](#12-grid) — C
13. [Landmarks](#13-landmarks) — F
14. [Link](#14-link) — F
15. [Listbox](#15-listbox) — F
16. [Menu and Menubar](#16-menu-and-menubar) — F (Menu) / C (Menubar)
17. [Menu Button](#17-menu-button) — F
18. [Meter](#18-meter) — C
19. [Radio Group](#19-radio-group) — F
20. [Slider (Single-Thumb)](#20-slider-single-thumb) — F
21. [Slider (Multi-Thumb)](#21-slider-multi-thumb) — F
22. [Spinbutton](#22-spinbutton) — F
23. [Switch](#23-switch) — F
24. [Table](#24-table-non-interactive-semantic) — C
25. [Tabs](#25-tabs) — F
26. [Toolbar](#26-toolbar) — C
27. [Tooltip](#27-tooltip) — F
28. [Tree View](#28-tree-view) — C
29. [Treegrid](#29-treegrid) — C
30. [Window Splitter](#30-window-splitter) — C

(APG also documents the **Disclosure (Modal)** pattern as a sub-variant of Dialog and **Slider Multi-Thumb** as a sibling to Slider; APG's index lists 32 rows total. Where APG bundles two rows under one heading — e.g. "Menu and Menubar", "Slider", "Slider (Multi-Thumb)" — Buiy treats them as separate widget contracts but a single pattern row in this catalog.)

## 1. Accordion (Sections with Show/Hide Functionality)

| Field | Value |
|---|---|
| Buiy widget | `Accordion` (F) |
| Role(s) | Each header is a `button`; the panel it controls is the disclosure region |
| States | `aria-expanded` on the header button (`true` / `false`) |
| Properties | `aria-controls` (button → panel id) |
| Keyboard | Enter/Space toggles; **Down/Up** moves between headers; **Home/End** to first/last header; optional `aria-disabled` on locked panels |
| Name | Header text from content |
| Description | Optional; via `aria-describedby` if a help string exists |
| WCAG anchors | 2.1.1, 2.4.3, 4.1.2 |

## 2. Alert

| Field | Value |
|---|---|
| Buiy widget | `Alert` (F) |
| Role | `alert` (implicit `aria-live="assertive"`, `aria-atomic="true"`) |
| States | none required |
| Properties | none required |
| Keyboard | none — alerts are non-interactive and unfocusable by default |
| Name | from content (AT reads the visible text) |
| Description | none typically |
| WCAG anchors | 4.1.3 |

## 3. Alert Dialog

| Field | Value |
|---|---|
| Buiy widget | `AlertDialog` (F) |
| Role | `alertdialog` |
| States | `aria-modal="true"` for modal alert dialogs |
| Properties | `aria-labelledby` (title), `aria-describedby` (body); `aria-controls` on invoker |
| Keyboard | Tab cycles within dialog (focus trap); **Esc** dismisses; focus moves to dialog on open; focus restores to invoker on close |
| Name | from `aria-labelledby` referencing the heading element |
| Description | from `aria-describedby` referencing the body text |
| WCAG anchors | 2.1.2, 2.4.3, 2.4.11, 4.1.2 |

## 4. Breadcrumb

| Field | Value |
|---|---|
| Buiy widget | `Breadcrumb` (C) |
| Role(s) | `navigation` wrapping a list of links |
| States | `aria-current="page"` on the link representing the current location |
| Properties | `aria-label` on the navigation landmark ("Breadcrumb") |
| Keyboard | normal Tab; no special keys |
| Name | landmark name from `aria-label` |
| WCAG anchors | 2.4.8 (AAA), 4.1.2 |

## 5. Button

| Field | Value |
|---|---|
| Buiy widget | `Button` (F), incl. toggle via `aria-pressed` |
| Role | `button` |
| States | `aria-pressed` (`true` / `false` / `mixed`) for toggle buttons; `aria-disabled`; `aria-expanded` if it controls a disclosure |
| Properties | `aria-haspopup` (`menu` / `listbox` / `dialog` / `tree` / `grid`) if the button opens one of those popups; `aria-controls` referencing the popup |
| Keyboard | Enter and Space activate |
| Name | from content (`<button>` text), or `aria-label` / `aria-labelledby` if icon-only |
| Description | optional via `aria-describedby` |
| WCAG anchors | 2.1.1, 2.5.3, 2.5.8, 4.1.2 |

## 6. Carousel (Slide Show or Image Rotator)

| Field | Value |
|---|---|
| Buiy widget | `Carousel` (C) |
| Role(s) | `region` (the rotator container, labeled "Carousel"), tablist + tab + tabpanel variant OR previous/next button variant |
| States | `aria-current="true"` on the slide control representing the current slide; `aria-roledescription="slide"` on each slide |
| Properties | `aria-label` on the region, `aria-controls` on previous/next buttons, `aria-live="polite"` while paused on auto-rotation |
| Keyboard | Tab to controls; **Enter/Space** activates pause/play/prev/next; if tablist variant, arrow keys cycle slides |
| Name | region `aria-label` (e.g. "Featured items") |
| WCAG anchors | 2.2.2 (pause / stop / hide), 4.1.2, 4.1.3 |

## 7. Checkbox

| Field | Value |
|---|---|
| Buiy widget | `Checkbox` (F), binary + tri-state |
| Role | `checkbox` |
| States | `aria-checked` (`true` / `false` / `mixed`); `aria-disabled`; `aria-required`; `aria-invalid` |
| Properties | `aria-labelledby` if label is external; `aria-describedby` for help text |
| Keyboard | **Space** toggles (Enter does NOT activate per APG); native `<input type=checkbox>` mirrors this |
| Name | from associated label element, `aria-label`, or `aria-labelledby` |
| WCAG anchors | 2.1.1, 2.5.3, 4.1.2 |

## 8. Combobox

| Field | Value |
|---|---|
| Buiy widget | `Combobox` (F) |
| Role(s) | `combobox` (the input), `listbox` (the popup), `option` (each item) |
| States | `aria-expanded` (true when listbox is open), `aria-activedescendant` (id of focused option without moving DOM focus), `aria-autocomplete` (`none` / `inline` / `list` / `both`) |
| Properties | `aria-controls` on combobox referencing the listbox; `aria-haspopup` defaults to `listbox` but may be `grid` / `tree` / `dialog` |
| Keyboard | **Down** opens popup and focuses first/active option; **Up** opens popup and focuses last; **Esc** closes popup; **Enter** selects active option and closes; **Tab** selects and moves focus; **Home/End** in listbox; type-ahead in input filters list |
| Name | from associated `<label>` or `aria-labelledby` |
| WCAG anchors | 1.3.5, 2.1.1, 3.3.2, 4.1.2 |

## 9. Dialog (Modal)

| Field | Value |
|---|---|
| Buiy widget | `Dialog` (F), modal + non-modal |
| Role | `dialog` |
| States | `aria-modal="true"` for modal dialogs |
| Properties | `aria-labelledby` (title), `aria-describedby` (body summary) |
| Keyboard | Tab cycles within dialog (focus trap); Shift+Tab reverses; **Esc** dismisses (unless `closedby="none"`); focus moves to first focusable on open; restores to invoker on close |
| Name | from `aria-labelledby` |
| WCAG anchors | 2.1.2, 2.4.3, 2.4.11, 4.1.2 |

## 10. Disclosure (Show/Hide)

| Field | Value |
|---|---|
| Buiy widget | `Disclosure` (F) |
| Role | `button` (the trigger); the panel is just a `region` or content |
| States | `aria-expanded` on the button |
| Properties | `aria-controls` (button → panel id) |
| Keyboard | Enter/Space toggles |
| Name | trigger label from content |
| WCAG anchors | 2.1.1, 4.1.2 |

## 11. Feed

| Field | Value |
|---|---|
| Buiy widget | `Feed` (C) |
| Role(s) | `feed` (the scrollable container), `article` (each feed item) |
| States | `aria-busy` while loading new items |
| Properties | `aria-setsize` (or `-1` if unknown), `aria-posinset` on each article |
| Keyboard | **PageDown** moves to next article; **PageUp** moves to previous; **Ctrl+End** moves to last; **Ctrl+Home** moves to first |
| Name | each article named via `aria-labelledby` or heading |
| WCAG anchors | 2.2.2, 4.1.3 |

## 12. Grid

| Field | Value |
|---|---|
| Buiy widget | `Grid` (C) |
| Role(s) | `grid` (container), `row`, `gridcell` (or `columnheader` / `rowheader`); `rowgroup` for groups |
| States | `aria-selected` on cells when grid supports selection; `aria-sort` on column headers |
| Properties | `aria-colcount`, `aria-rowcount`, `aria-colindex`, `aria-rowindex`, `aria-colspan`, `aria-rowspan`, `aria-multiselectable` |
| Keyboard | **Right/Left** moves cell within row; **Down/Up** moves cell within column; **Home/End** to row start/end; **Ctrl+Home/End** to first/last cell; **PageDown/PageUp** scrolls a screenful; **F2** enters edit mode for editable cells; **Esc** exits edit; type-ahead optional |
| Name | grid `aria-label` or `aria-labelledby` |
| WCAG anchors | 1.3.1, 1.3.2, 2.1.1, 4.1.2 |

## 13. Landmarks

| Field | Value |
|---|---|
| Buiy widget | Landmark container components (F): `banner`, `navigation`, `main`, `complementary`, `contentinfo`, `search`, `region`, `form` |
| Role | matching ARIA landmark role |
| Properties | `aria-label` or `aria-labelledby` when more than one of the same landmark type is present |
| Keyboard | no special interaction; screen readers offer landmark navigation (e.g. NVDA `d` key) |
| Name | from label property or heading inside |
| WCAG anchors | 1.3.1, 2.4.1 |

## 14. Link

| Field | Value |
|---|---|
| Buiy widget | `Link` (F) |
| Role | `link` |
| States | `aria-disabled` (rare; links are typically not disabled) |
| Properties | `aria-current` (`page` / `step` / `location` / `true`) on links representing the current location |
| Keyboard | **Enter** activates (Space does NOT) |
| Name | from content; or `aria-label` / `aria-labelledby` if needed |
| WCAG anchors | 2.4.4, 2.4.9 (AAA), 4.1.2 |

## 15. Listbox

| Field | Value |
|---|---|
| Buiy widget | `Listbox` (F), single + multi-select |
| Role(s) | `listbox`, `option`, optional `group` |
| States | `aria-selected` on options; `aria-multiselectable` on listbox; `aria-disabled`; `aria-orientation` if vertical/horizontal |
| Properties | `aria-activedescendant` (for the focused-but-not-DOM-focused option pattern); `aria-setsize` / `aria-posinset` when set is virtual/partial |
| Keyboard | **Down/Up** moves focus; **Home/End** first/last; **PageDown/PageUp** chunk; **Space** toggles in multi-select; **Shift+Down/Up** extends selection; **Ctrl+A** select-all (multi-select); type-ahead |
| Name | `aria-label` / `aria-labelledby` |
| WCAG anchors | 2.1.1, 4.1.2 |

## 16. Menu and Menubar

| Field | Value |
|---|---|
| Buiy widget | `Menu` (F), `Menubar` (C) |
| Role(s) | `menubar` / `menu`, `menuitem`, `menuitemcheckbox`, `menuitemradio`, optional `separator` (focusable variant `none`) |
| States | `aria-checked` on `menuitemcheckbox` / `menuitemradio`; `aria-expanded` on items that open submenus; `aria-disabled` |
| Properties | `aria-haspopup="menu"` on items opening submenus; `aria-orientation` on menubar (`horizontal`) and menu (`vertical`) |
| Keyboard | menubar: **Right/Left** between top-level items, **Down** opens submenu; menu: **Down/Up** between items, **Right** opens submenu, **Left** closes submenu / moves to parent; **Enter** activates / opens submenu; **Esc** closes menu; **Home/End** first/last; **Space** activates `menuitemcheckbox` / `menuitemradio`; type-ahead first-letter |
| Name | menubar/menu via `aria-label`; items from content |
| WCAG anchors | 2.1.1, 2.1.4 (aria-keyshortcuts), 4.1.2 |

## 17. Menu Button

| Field | Value |
|---|---|
| Buiy widget | `MenuButton` (F) |
| Role | `button` with `aria-haspopup="menu"` |
| States | `aria-expanded` on button (`true` when menu is open) |
| Properties | `aria-controls` referencing menu id |
| Keyboard | **Enter** / **Space** / **Down** opens menu and moves focus to first item; **Up** opens menu and moves focus to last item |
| Name | button text from content (or `aria-label` for icon) |
| WCAG anchors | 2.1.1, 4.1.2 |

## 18. Meter

| Field | Value |
|---|---|
| Buiy widget | `Meter` (C) |
| Role | `meter` |
| States | — |
| Properties | `aria-valuenow`, `aria-valuemin`, `aria-valuemax`, `aria-valuetext` (e.g. "75% — high") |
| Keyboard | none — meter is read-only |
| Name | `aria-label` / `aria-labelledby` |
| WCAG anchors | 1.3.1, 4.1.2 |

## 19. Radio Group

| Field | Value |
|---|---|
| Buiy widget | `RadioGroup` (F) |
| Role(s) | `radiogroup`, `radio` |
| States | `aria-checked` on each radio (only one true); `aria-disabled`; `aria-required`; `aria-invalid` on group |
| Properties | `aria-labelledby` / `aria-label` on group; `aria-orientation` |
| Keyboard | **Tab** enters group on the checked radio (or first if none checked); **Down/Right** moves and **auto-selects** next radio; **Up/Left** moves and selects previous; **Space** also activates the focused radio |
| Name | group label from `aria-labelledby`; each radio label from content / associated label |
| WCAG anchors | 2.1.1, 2.5.3, 4.1.2 |

## 20. Slider (Single-Thumb)

| Field | Value |
|---|---|
| Buiy widget | `Slider` (F) |
| Role | `slider` |
| States | `aria-disabled`; `aria-orientation` |
| Properties | `aria-valuenow`, `aria-valuemin`, `aria-valuemax`, `aria-valuetext` |
| Keyboard | **Right/Up** increment; **Left/Down** decrement; **Home** to min; **End** to max; **PageUp/PageDown** larger step (commonly 10× the step) |
| Name | `aria-label` / `aria-labelledby` |
| WCAG anchors | 1.3.1, 2.1.1, 2.5.7, 4.1.2 |

## 21. Slider (Multi-Thumb)

| Field | Value |
|---|---|
| Buiy widget | `Slider` (F) with multiple thumbs |
| Role(s) | `slider` per thumb |
| States | each thumb `aria-valuenow` |
| Properties | per-thumb `aria-valuemin`, `aria-valuemax`, `aria-valuetext`, `aria-label` (e.g. "minimum price", "maximum price") |
| Keyboard | per thumb as Slider single; **Tab** moves between thumbs |
| Name | per-thumb `aria-label` distinguishes thumbs |
| WCAG anchors | 1.3.1, 2.1.1, 2.5.7, 4.1.2 |

## 22. Spinbutton

| Field | Value |
|---|---|
| Buiy widget | `Spinbutton` (F) — numeric stepper |
| Role | `spinbutton` |
| States | `aria-readonly`, `aria-required`, `aria-invalid` |
| Properties | `aria-valuenow`, `aria-valuemin`, `aria-valuemax`, `aria-valuetext` |
| Keyboard | **Up/Down** increment / decrement; **PageUp/PageDown** large step; **Home/End** to min/max; **typed digits** edit value directly |
| Name | `aria-label` / `aria-labelledby` |
| WCAG anchors | 1.3.5, 2.1.1, 4.1.2 |

## 23. Switch

| Field | Value |
|---|---|
| Buiy widget | `Switch` (F) |
| Role | `switch` |
| States | `aria-checked` (`true` / `false`); `aria-disabled`; `aria-readonly` |
| Properties | `aria-labelledby` if external label |
| Keyboard | **Space** toggles (Enter also accepted per APG) |
| Name | from associated label |
| WCAG anchors | 2.1.1, 2.5.3, 4.1.2 |

## 24. Table (Non-Interactive Semantic)

| Field | Value |
|---|---|
| Buiy widget | `Table` (C) |
| Role(s) | `table`, `row`, `cell`, `columnheader`, `rowheader`, `rowgroup`, `caption` |
| States | — |
| Properties | `aria-colcount`, `aria-rowcount`, `aria-rowindex`, `aria-colindex`, `aria-colspan`, `aria-rowspan`; `aria-sort` on sortable column headers |
| Keyboard | none (non-interactive); cells are not focusable. (Use Grid pattern if interactive.) |
| Name | `<caption>` element or `aria-labelledby` |
| WCAG anchors | 1.3.1, 1.3.2, 4.1.2 |

## 25. Tabs

| Field | Value |
|---|---|
| Buiy widget | `Tabs` (F), auto-activate + manual-activate variants |
| Role(s) | `tablist`, `tab`, `tabpanel` |
| States | `aria-selected` on the active tab; `aria-disabled`; `aria-orientation` on tablist |
| Properties | `aria-controls` on each tab referencing its tabpanel; `aria-labelledby` on each tabpanel referencing its tab |
| Keyboard | **Right/Left** (horizontal) or **Down/Up** (vertical) moves tab focus; **Home/End** to first/last; auto-activate: selection follows focus; manual-activate: **Enter/Space** activates the focused tab; **Tab** moves into the active tabpanel |
| Name | each tab from content; tabpanel from its tab |
| WCAG anchors | 2.1.1, 4.1.2 |

## 26. Toolbar

| Field | Value |
|---|---|
| Buiy widget | `Toolbar` (C) |
| Role | `toolbar` containing buttons / toggles / menu buttons / spinbuttons |
| States | per-item states |
| Properties | `aria-label` / `aria-labelledby`; `aria-orientation` |
| Keyboard | **Tab** enters toolbar at first item (or last focused); **Right/Left** between items; **Home/End** first/last; items operate per their own pattern; **Tab** exits the toolbar |
| Name | `aria-label` (e.g. "Editing tools") |
| WCAG anchors | 2.1.1, 4.1.2 |

## 27. Tooltip

| Field | Value |
|---|---|
| Buiy widget | `Tooltip` (F) |
| Role | `tooltip` |
| States | — |
| Properties | the trigger references the tooltip via `aria-describedby` |
| Keyboard | **focus on trigger** shows tooltip; **Esc** dismisses without losing focus; tooltips are non-interactive (no internal focus) |
| Name | from trigger; tooltip itself is descriptive text |
| Description | tooltip serves as `aria-describedby` target |
| WCAG anchors | 1.4.13 (dismissable / hoverable / persistent), 4.1.2 |

## 28. Tree View

| Field | Value |
|---|---|
| Buiy widget | `Tree` (C) |
| Role(s) | `tree`, `treeitem`, `group` (for child item containers) |
| States | `aria-expanded` on items with children (`true` / `false`); `aria-selected` (single / multi); `aria-level`, `aria-setsize`, `aria-posinset` |
| Properties | `aria-multiselectable` on tree; `aria-orientation` (default `vertical`) |
| Keyboard | **Down/Up** moves to next/previous visible item; **Right** expands collapsed or moves to first child; **Left** collapses expanded or moves to parent; **Home/End** first/last visible; type-ahead; **Enter** activates / toggles selection; **Space** selects; **Ctrl+Space** / **Shift+Down** for multi-select |
| Name | tree `aria-label`; items from content |
| WCAG anchors | 1.3.1, 2.1.1, 4.1.2 |

## 29. Treegrid

| Field | Value |
|---|---|
| Buiy widget | `Treegrid` (C) |
| Role(s) | `treegrid`, `row`, `gridcell`, `rowheader`, `columnheader` |
| States | per-cell states; per-row `aria-expanded`; `aria-level` |
| Properties | grid properties + tree properties combined; `aria-rowindex`, `aria-colindex`, `aria-setsize`, `aria-posinset` |
| Keyboard | grid navigation (cell-wise) + tree expand / collapse: **Right** on collapsed row expands; **Left** on expanded row collapses; otherwise grid arrow keys per Grid |
| Name | `aria-label` / `aria-labelledby` |
| WCAG anchors | 1.3.1, 1.3.2, 2.1.1, 4.1.2 |

## 30. Window Splitter

| Field | Value |
|---|---|
| Buiy widget | `WindowSplitter` (C) |
| Role | `separator` (with `tabindex="0"` to be focusable) |
| States | `aria-orientation`, `aria-valuemin`, `aria-valuemax`, `aria-valuenow` (the current position) |
| Properties | `aria-controls` referencing the affected pane(s) |
| Keyboard | **Right/Left** (vertical splitter) or **Up/Down** (horizontal splitter) moves splitter; **Home/End** to min/max; **Enter** optional toggle to collapse/restore |
| Name | `aria-label` (e.g. "Resize file pane") |
| WCAG anchors | 2.1.1, 2.5.7 (keyboard alternative to drag), 4.1.2 |

## Patterns Buiy does NOT directly map to APG rows but the Buiy catalog ships

The Buiy widget catalog ([`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) includes several widgets whose APG pattern is either implicit, derivative, or extended for game-engine use:

- **Card** — APG has no "card" pattern. Buiy emits `role="button"` or `role="link"` when the whole card is clickable; `role="group"` otherwise. Documented in the Buiy widget catalog as a composite pattern over the canonical roles.
- **Rating** — Buiy implements as a discrete-step `slider` per the APG Slider pattern; arrow keys increment / decrement, Home/End set min/max.
- **Toast / Snackbar** — extension of the APG Alert pattern with auto-dismiss + WCAG 2.2.3 pause / stop / extend controls.
- **Popover** — HTML Popover spec (not APG); uses `role="dialog"` or appropriate role based on contents. APG covers Tooltip and Dialog separately; Popover is a state-machine wrapper over them.
- **Progressbar** — implicit ARIA role `progressbar` with `aria-valuemin` / `aria-valuemax` / `aria-valuenow`; APG documents this in the ARIA 1.2 spec rather than as a standalone pattern page.
- **Log**, **Status**, **Timer**, **Marquee** — live-region roles. APG covers Alert; the others are role-only with no keyboard contract.
- **Searchbox** — APG references it under Combobox; Buiy ships a separate widget contract because not every searchbox has a popup.
- **Skip link** — primitive landmark-navigation aid, not an APG-pattern widget; covered under [WCAG 2.4.1](wcag-22-aa-mapping.md).

For each, the per-widget spec under `buiy-widget-catalog-design` is the canonical contract.

## Sources

- APG patterns library (32 patterns): <https://www.w3.org/WAI/ARIA/apg/patterns/>
- WAI-ARIA 1.2 — Roles, States, Properties: <https://www.w3.org/TR/wai-aria-1.2/>
- WCAG 2.2 Understanding docs: <https://www.w3.org/WAI/WCAG22/Understanding/>
- Buiy widget catalog (foundation): [`docs/specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy accessibility (foundation): [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Sibling files: [`keyboard-contracts.md`](keyboard-contracts.md), [`roles-states-properties.md`](roles-states-properties.md), [`name-computation.md`](name-computation.md), [`focus-management.md`](focus-management.md)
