**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — layout primitives and styling system

# Layout and styling

Slate has its **own** layout engine — not Flexbox, not CSS Grid, not Taffy. Each layout container hard-codes its arrangement rules in C++ inside its `OnArrangeChildren` and `ComputeDesiredSize` virtuals. The container set is small and specialized; layout authors choose by composing containers, not by setting `display: flex` on a generic node.

UMG mirrors Slate's container set with `U`-prefixed wrappers; each panel-child relationship is a concrete `UPanelSlot` subclass.

## Layout primitives

### Boxes — `SHorizontalBox` / `SVerticalBox`

The bread-and-butter linear layouts. Each child is wrapped in a `Slot` with sizing rules:

- **`AutoWidth()` / `AutoHeight()`** — child gets its desired size; container doesn't allocate extra.
- **`FillWidth(weight)` / `FillHeight(weight)`** — child receives a proportional share of leftover space (analogous to CSS `flex-grow`).
- **`Padding(...)`** — per-slot padding.
- **`HAlign(EHorizontalAlignment)` / `VAlign(EVerticalAlignment)`** — alignment within the slot's allocated rect: `Left/Center/Right/Fill`, `Top/Center/Bottom/Fill`.
- **`MaxWidth()` / `MaxHeight()`** — clamp.

```cpp
SNew(SHorizontalBox)
+ SHorizontalBox::Slot().AutoWidth().Padding(4, 0).VAlign(VAlign_Center)
  [ SNew(SImage).Image(IconBrush) ]
+ SHorizontalBox::Slot().FillWidth(1.f).Padding(4, 0).VAlign(VAlign_Center)
  [ SNew(STextBlock).Text(Label) ]
+ SHorizontalBox::Slot().AutoWidth()
  [ SNew(SButton).Text(LOCTEXT("Go","Go")) ]
```

UMG's `UHorizontalBox` / `UVerticalBox` exposes the same fields via `UHorizontalBoxSlot` / `UVerticalBoxSlot` reflection in the Details panel.

### Overlay — `SOverlay` / `UOverlay`

Stack children at the same coordinate origin. Each child sits in an `Overlay::Slot` with `HAlign`/`VAlign`/`Padding`, no z-index — order in the slot list determines paint order.

### Free-positioned canvas — `SConstraintCanvas` / `UCanvasPanel`

The closest Slate analog to absolute positioning. Each child slot carries:

- **Anchors** (`FAnchors`) — a 2D rect (`Minimum`, `Maximum`) defining what fraction of the parent each corner is anchored to. `(0,0)-(0,0)` anchors to top-left; `(1,1)-(1,1)` anchors to bottom-right; `(0,0)-(1,1)` stretches.
- **Offset** (`FAnchorData`) — pixel offset relative to the anchor.
- **Alignment** — local pivot.
- **Z-order** — explicit integer; higher draws on top.
- **Size-To-Content** — boolean; ignores offset and uses desired size.

`UCanvasPanel` is the default root for new Widget Blueprints because designers expect the "drag a button to a screen position" workflow. Engineers tend to swap it for `UVerticalBox`/`UHorizontalBox` once layout matters more than absolute position.

### Grids — `SGridPanel`, `SUniformGridPanel`, `UGridPanel`, `UUniformGridPanel`

`SGridPanel` is a non-uniform grid: each slot specifies `.Row(r).Column(c).RowSpan(n).ColumnSpan(n)`; row/column sizes are derived from the largest child in each line. `SUniformGridPanel` forces every cell to the same size (the size of the largest single cell).

There is **no** CSS-Grid-style named lines, line-template syntax, or fr units. The grid is a coarse table layout, not a content layout system.

### Wrap-flow — `SWrapBox` / `UWrapBox`

Linear flow that wraps to a new line at a configurable inner-wrap width. Useful for tag clouds and toolbars.

### Scroll containers — `SScrollBox` / `UScrollBox`

A scrollable list of children (vertical or horizontal). Children render at their desired size; the box scrolls them. Includes a `SScrollBar` companion widget for the scrollbar UI. Scroll position is its own state.

### Constraint / size containers — `SBox` / `USizeBox`, `SScaleBox` / `UScaleBox`

`SBox` is a single-child wrapper that applies size constraints (`WidthOverride`, `HeightOverride`, `MinDesiredWidth`, `MaxDesiredWidth`, etc.) and padding. The Slate equivalent of `<div style="width: 200px; padding: 8px">`.

`SScaleBox` scales its content to fit (stretch / fit / fill-X / fill-Y / scale-to-fit). Used for resolution-independent UI on top of `SDPIScaler` (which scales the whole tree by an OS-DPI factor).

### Single-child decorated container — `SBorder` / `UBorder`

A single-child container that paints a brush (`FSlateBrush`) as background and applies padding. The Slate equivalent of `<div class="bordered">`.

## What's missing relative to CSS

Slate's layout model **does not** include:

- Flexbox `flex-basis` / `flex-shrink` semantics. `FillWidth(weight)` is `flex-grow` only; shrinking is implicit and limited.
- CSS Grid named lines, `repeat()`, `minmax()`, subgrid, fr units, `grid-template-areas`.
- Anchor positioning (CSS `anchor()`, `position-anchor`).
- Container queries (no `@container` analog — UI must re-author per breakpoint).
- Logical properties (`margin-inline-start` etc.; Slate is physical-direction-only).
- Aspect-ratio property (must compose with `SScaleBox`).
- Subpixel-accurate text-aware baselines.

For game UI these gaps rarely bite at runtime — UMG asset authors place widgets per-screen-resolution. For app UI (the Buiy "Game and app, both" goal), they would bite immediately.

## Styling: `FSlateBrush`, `FSlateStyleSet`, `FSlateStyleRegistry`

Slate's styling system is **asset-based but typed**.

### `FSlateBrush`

A brush describes how to draw a single Slate element. Variants:

- **`FSlateBoxBrush`** / **`FSlateVectorBoxBrush`** — 9-slice scaled box (margin defines the 9-slice insets). The most common UI panel/button background.
- **`FSlateBorderBrush`** / **`FSlateVectorBorderBrush`** — border-only 9-slice.
- **`FSlateImageBrush`** / **`FSlateVectorImageBrush`** — plain image (UMG `UImage` defaults to this).
- **`FSlateColorBrush`** — solid color fill.
- **`FSlateRoundedBoxBrush`** — rounded rectangle with per-corner radius, outline, outline width.
- **`FSlateDynamicImageBrush`** — runtime-loaded texture (UMG `SetBrushFromTexture`).
- **`FSlateMaterialBrush`** — full Unreal `UMaterialInterface` used as the fill (for shader effects: blurs, gradients, masks, custom).
- **`FSlateNoResource`** — explicit "draw nothing" brush.

A brush is just a value struct — it doesn't own a widget. Widgets reference brushes (e.g. `SImage::SetImage(const FSlateBrush*)`); usually via a style set.

### `FSlateStyleSet` and the registry

A **style set** is a named collection of named properties — brushes, fonts, colors, button styles, text styles — that the application registers once at startup. Access:

```cpp
const FSlateBrush* Brush = FAppStyle::Get().GetBrush("Icons.Save");
const FSlateFontInfo Font = FAppStyle::Get().GetFontStyle("HeadingMedium");
const FButtonStyle& BtnStyle = FAppStyle::Get().GetWidgetStyle<FButtonStyle>("PrimaryButton");
```

`FSlateStyleRegistry::RegisterSlateStyle(MyStyleSet)` makes a style available globally; `FSlateStyleRegistry::FindSlateStyle("MyStyle")` retrieves it. The editor itself registers `FAppStyle` (UE5; formerly `FEditorStyle` in UE4) at boot; that single style set drives the entire editor's look.

### UMG styling

UMG widgets expose the same brush/font/style data via reflected `UPROPERTY` fields. The Details panel for a `UButton`, for example, has a `Style` field of type `FButtonStyle` with `Normal`, `Hovered`, `Pressed`, `Disabled` brushes, plus text padding, sound cues, etc. Asset authors can either edit values inline or reference a shared `FButtonStyle` data asset.

The **CommonUI** plugin adds another layer: typed style data assets (`UCommonButtonStyle`, `UCommonTextStyle`) that are first-class `.uasset`s, swappable per-platform, and shared across many widgets.

## Asset-driven authoring is the Designer's superpower

The reason UMG won over Slate-only authoring is straightforward: every property of every widget is a reflected `UPROPERTY`, every layout slot is a reflected `UPanelSlot`, and every style is a reflected struct. That reflection layer is what makes the **Widget Designer** possible — a visual editor that needs to enumerate every property, type-check it, undo/redo it, and serialize it survives only if reflection is universal.

Buiy's `.bsn` asset story makes the same bet: every Buiy component is `Reflect + FromReflect + Default + Clone + Component`, registered with `app.register_type::<T>()` — exactly so the asset-loader path is identical to the code path. See [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.4.

## Sources

- Slate Overview — https://dev.epicgames.com/documentation/en-us/unreal-engine/slate-overview-for-unreal-engine
- UCanvasPanel API — https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/UMG/UCanvasPanel
- UGridPanel API — https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/UMG/Components/UGridPanel
- UScrollBox API — https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/UMG/UScrollBox
- FSlateBrush API — https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/SlateCore/Styling/FSlateBrush
- FSlateStyleSet (UE Community Wiki) — https://unrealcommunity.wiki/fslatestyleset-zz89r8ee
- How to Style Slate Widgets Using Style Sets — https://minifloppy.it/posts/2023/how-to-style-slate-widgets-using-stylesets/
- Common UI Plugin — https://dev.epicgames.com/documentation/unreal-engine/common-ui-plugin-for-advanced-user-interfaces-in-unreal-engine
