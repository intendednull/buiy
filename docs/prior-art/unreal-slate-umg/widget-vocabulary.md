**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — side-by-side widget vocabulary

# Widget vocabulary

Every commonly-used UMG widget has a corresponding Slate widget that it wraps. The table below enumerates the pairs that show up most often in tutorial code, the editor itself, and shipped games. The mapping is not perfectly 1:1 — UMG omits some Slate widgets that are editor-only (the `S*View`/`S*Picker` constellation that powers the asset browser), and a handful of UMG widgets are pure-UMG niceties without a direct Slate twin (e.g. `UDynamicEntryBox` for list virtualization).

## Common interactive widgets

| Concept | Slate (`S`) | UMG (`U`) | Notes |
|---|---|---|---|
| Push button | `SButton` | `UButton` | UMG's `UButton` has a single content slot; Slate's `SButton` has `.Content()[ ... ]`. |
| Checkbox / toggle | `SCheckBox` | `UCheckBox` | Both support tri-state (`Checked` / `Unchecked` / `Undetermined`). |
| Radio button | `SCheckBox` (style variant) | `UCheckBox` (style variant) | Both stacks model radios as checkboxes with a `ESlateCheckBoxType::Radio` style + manual group management. There is no first-class `SRadioButton` / `URadioButton`. |
| Slider | `SSlider` | `USlider` | Horizontal. `SSpinBox<T>` is a templated numeric input. |
| Spinner / numeric stepper | `SSpinBox<T>` | `USpinBox` | Templated in Slate; UMG version is float-only. |
| Progress bar | `SProgressBar` | `UProgressBar` | Both support continuous + indeterminate marquee styles. |
| Hyperlink | `SHyperlink` | (no direct UMG twin; commonly `URichTextBlock` decorator) | |
| Throbber / spinner | `SThrobber`, `SCircularThrobber` | `UThrobber`, `UCircularThrobber` | |

## Text and editing

| Concept | Slate (`S`) | UMG (`U`) | Notes |
|---|---|---|---|
| Static text | `STextBlock` | `UTextBlock` | Single-style text run. |
| Rich text (multi-style runs, inline images, hyperlinks) | `SRichTextBlock` | `URichTextBlock` | Decorators (`URichTextBlockDecorator`) handle inline markup like `<bold>...</bold>`. |
| Single-line edit | `SEditableText`, `SEditableTextBox` | `UEditableText`, `UEditableTextBox` | "Box" variants add background/border styling. |
| Multi-line edit | `SMultiLineEditableText`, `SMultiLineEditableTextBox` | `UMultiLineEditableTextBox` | The text-editing surface for chat, notes, search results. |
| Search box | `SSearchBox` | `USearchBox` (limited) | Comes with a search icon + clear-button decoration. |
| Password field | `SEditableTextBox` with `IsPassword=true` | `UEditableTextBox` with `IsPassword=true` | |

## Media

| Concept | Slate (`S`) | UMG (`U`) | Notes |
|---|---|---|---|
| Image | `SImage` | `UImage` | Backing brush (`FSlateBrush`) carries the texture / material / tiling info. |
| Video / movie | (Engine-side `UMediaPlayer` + `UMaterialInstance` rendered into `SImage`) | `UWidget` wrapping `UMediaPlayer` | UMG game UI typically renders video via a media-texture-backed brush. |
| 3D scene / viewport | `SViewport`, `SLevelViewport` | `UViewport`-style custom widgets | Editor uses `SLevelViewport`; game UI uses `URetainerBox` + render-to-texture patterns. |

## Containers / layout

(See [`layout-and-styling.md`](layout-and-styling.md) for the layout-rule details. This table is just the vocabulary.)

| Concept | Slate (`S`) | UMG (`U`) |
|---|---|---|
| Horizontal flow | `SHorizontalBox` | `UHorizontalBox` |
| Vertical flow | `SVerticalBox` | `UVerticalBox` |
| Overlay (stack with absolute-ish anchoring) | `SOverlay` | `UOverlay` |
| Free-positioned anchored canvas | `SConstraintCanvas` | `UCanvasPanel` |
| Grid (cell-based) | `SGridPanel` | `UGridPanel` |
| Uniform grid (equal cells) | `SUniformGridPanel` | `UUniformGridPanel` |
| Wrap-flow | `SWrapBox` | `UWrapBox` |
| Scroll container | `SScrollBox` | `UScrollBox` |
| Size constraint | `SBox` | `USizeBox` |
| Border (single-child, decorated) | `SBorder` | `UBorder` |
| Padding (single-child) | `SBox` with `.Padding(...)` | `USizeBox`/`UBorder` with padding |
| Aspect-ratio container | `SDPIScaler`, `SScaleBox` | `UScaleBox` |
| Widget switcher (one-of-many) | `SWidgetSwitcher` | `UWidgetSwitcher` |

## Lists / trees

| Concept | Slate (`S`) | UMG (`U`) |
|---|---|---|
| List view (virtualized 1D) | `SListView<T>` | `UListView` |
| Tile view (virtualized 2D grid) | `STileView<T>` | `UTileView` |
| Tree view (virtualized hierarchical) | `STreeView<T>` | `UTreeView` |
| Dynamic entry container (CommonUI) | — | `UDynamicEntryBox` |
| Header row (column headers for list/tree) | `SHeaderRow` | (UMG handles via per-row entry widgets) |

The Slate list/tree views are **templated** on the item type (`SListView<TSharedPtr<FMyItem>>`); UMG list-views use a runtime `UObject*` item type plus an "entry widget class" Blueprint reference. The UMG path is dynamic and Blueprint-friendly; the Slate path is faster but C++-only.

## Windows / popovers / chrome

| Concept | Slate | UMG |
|---|---|---|
| OS window | `SWindow` | (no direct UMG twin; created from C++) |
| Tooltip | `SToolTip` | UMG widgets carry a `ToolTipText` property + a `ToolTipWidget` child |
| Menu anchor / popup | `SMenuAnchor`, `SComboButton` | `UComboBoxString`, `UMenuAnchor` |
| Dropdown / combobox | `SComboBox<T>`, `SComboButton` | `UComboBoxString`, `UComboBoxKey` |
| Modal notification | `SNotificationList` | (CommonUI plugin patterns) |
| Dock tab (editor-only) | `SDockTab`, `SDockingTabStack` | (editor-only) |

## Editor-only Slate widgets

A large fraction of `S*` widgets ship only with the editor and are unavailable to game UI:

- `SGraphPanel`, `SGraphNode`, `SGraphPin` — the Blueprint node-graph editor.
- `SDetailsView`, `SPropertyEditor*` — the universal property inspector.
- `SAssetPicker`, `SAssetView`, `SAssetSearchBox` — the Content Browser.
- `SColorPicker`, `SCurveEditor`, `STimecode*` — specialized editor widgets.

These illustrate the breadth of what Slate is asked to cover (essentially "all UI in a AAA-grade 3D editor") but aren't part of the runtime UMG vocabulary. They're worth study, not borrow.

## CommonUI additions

The CommonUI plugin (see [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md)) adds another layer of UMG widgets:

- `UCommonButtonBase`, `UCommonTextBlock`, `UCommonRichTextBlock`, `UCommonImage`.
- `UCommonActivatableWidget` — stack-based screen-state management.
- `UCommonInputActionDataBase` — controller-icon swap.
- `UCommonBoundActionBar` — auto-rendered action bar matching the current focus's input bindings.

These extend UMG without replacing it; every CommonUI widget still wraps a Slate widget under the hood.

## Sources

- Widget Blueprints in UMG — https://dev.epicgames.com/documentation/en-us/unreal-engine/widget-blueprints-in-umg-for-unreal-engine
- UPanelWidget API — https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/UMG/Components/UPanelWidget
- UCanvasPanel API — https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/UMG/UCanvasPanel
- UScrollBox API — https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/UMG/UScrollBox
- UGridPanel API — https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/UMG/Components/UGridPanel
- UMG-Slate-Compendium — https://github.com/YawLighthouse/UMG-Slate-Compendium
- Common UI Plugin overview — https://dev.epicgames.com/documentation/unreal-engine/common-ui-plugin-for-advanced-user-interfaces-in-unreal-engine
