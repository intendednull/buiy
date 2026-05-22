**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — UMG's designer-asset layer on top of Slate

# UMG architecture

**Unreal Motion Graphics** (UMG) is the designer-facing UI authoring layer that ships **on top of Slate**. It was introduced in **Unreal Engine 4.5 (November 2014)** and has been Epic's recommended layer for game UI ever since. UMG is *not* a replacement for Slate — it is a `UObject`-derived wrapper that:

1. Exposes Slate widgets to **Blueprint** (Unreal's visual-scripting VM and reflection system).
2. Stores UI as an **asset** (`.uasset` → "Widget Blueprint", file extension `.uasset` with `WBP_` prefix conventionally).
3. Provides a **visual editor** — the Widget Blueprint Editor, with Designer / Graph / Animation tabs.

## The `U`-prefix and the class hierarchy

UMG widgets are `UObject`s — they participate in garbage collection, reflection, Blueprint visibility, and serialization. The hierarchy is rooted at `UWidget`:

```
UWidget                          // UObject base; owns a TakeWidget() factory for the backing Slate widget
├── UPanelWidget                 // multi-child layout container
│   ├── UCanvasPanel             // free-positioned, anchored, z-ordered children
│   ├── UHorizontalBox / UVerticalBox
│   ├── UGridPanel
│   ├── UScrollBox / UWrapBox
│   ├── UOverlay
│   ├── UUniformGridPanel
│   ├── USizeBox / UBorder       // (single-child special cases also live under UPanelWidget)
│   └── UWidgetSwitcher
├── UTextBlock / URichTextBlock
├── UImage
├── UButton / UCheckBox / USlider / UProgressBar
├── UEditableText / UEditableTextBox / UMultiLineEditableTextBox
├── UComboBoxString
├── UListView / UTileView / UTreeView
└── UUserWidget                  // custom widget made of other widgets — the WBP backing class
```

Each UMG widget owns a `TakeWidget()` method that **constructs the backing Slate widget** lazily — `UButton::TakeWidget()` constructs an `SButton`, applies the Blueprint-set properties (style, click handler, content), and returns the `TSharedRef<SWidget>`. The Slate widget is what actually renders, lays out, and handles input. UMG is the **reflection + asset + Blueprint surface** on top.

## Widget Blueprints (WBPs)

A **Widget Blueprint** is an asset that defines a custom `UUserWidget` subclass:

- Stored as a `.uasset` file under `Content/`, typically named `WBP_MainMenu`, `WBP_HealthBar`, etc.
- Its **root** is a single panel widget (commonly `UCanvasPanel` by default, swappable to `UVerticalBox`/`UHorizontalBox`/`UOverlay`/etc.).
- Children are added by drag-and-drop in the Designer tab; each child appears in a **Hierarchy panel** alongside it.
- Each child widget has a `bIsVariable` flag — when true, the widget is exposed as a Blueprint-VM variable so the Graph tab can read/write its properties at runtime.

Widget Blueprints can extend other Widget Blueprints (inheritance). They can also be templated via the **Widget Templates** mechanism (UE5+) to share a layout shell with parameterized content.

## The Widget Blueprint Editor

Opening a `.uasset` Widget Blueprint launches the Widget Blueprint Editor with three primary tabs:

- **Designer** — visual canvas. Drag widgets from the **Palette** into the **Hierarchy**. Manipulate via the canvas (drag/resize/anchor) or the **Details** panel (every Slate-attribute equivalent appears here, plus UMG-specific properties: slot layout, accessibility config, render transform). The Designer renders a live Slate preview.
- **Graph** — Blueprint visual-scripting graph. Author event handlers (`OnClicked`, `OnTextChanged`, `OnConstruct`, `OnTick`), bind data, drive transitions. Variables for `bIsVariable=true` widgets appear in the My Blueprint panel.
- **Animation** — keyframe-driven UMG animations (introduced as "more robust animation support" in UE 4.5). Each WBP can carry many named `UWidgetAnimation` tracks targeting widget properties (opacity, transform, color, brush). Playable from the Graph at runtime.

Behind the scenes, the Widget Blueprint Editor is itself a giant Slate widget (`SWidgetDesigner`, `SWidgetTreeView`, etc.) — Unreal's editor eats its own dogfood.

## Slots: where UMG diverges from Slate

In Slate, the layout-slot for a child is an inline construct: `+ SHorizontalBox::Slot().FillWidth(1.f).Padding(8)[ child ]`. The slot itself isn't separately addressable.

In UMG, every panel-child relationship is a concrete `UPanelSlot` subclass:

| Panel | Slot type |
|---|---|
| `UCanvasPanel` | `UCanvasPanelSlot` (anchors, offsets, z-order, alignment) |
| `UHorizontalBox` | `UHorizontalBoxSlot` (padding, size: Auto/Fill, horizontal/vertical alignment) |
| `UVerticalBox` | `UVerticalBoxSlot` (same as HBox but vertical) |
| `UGridPanel` | `UGridSlot` (row, column, row-span, column-span, alignment) |
| `UScrollBox` | `UScrollBoxSlot` |
| `UOverlay` | `UOverlaySlot` |

These slot objects are first-class `UObject`s — they live on disk inside the WBP asset, are visible in the Details panel ("Slot" category), and can be navigated to from Blueprint via `UWidget::Slot`. This makes layout configuration **reflection-driven**, which is the single most important enabler for UMG's visual Designer.

## Data binding

UMG offers three binding mechanisms with increasing performance cost:

1. **Direct property set** — `MyTextBlock->SetText(NewText)` from the Graph tab on event. Cheapest. The Epic-recommended path.
2. **Function binding** (a.k.a. "bound attributes") — in the Details panel for a text/color/visibility property, click "Bind" → "Bind Function" → a new function appears in the Graph. The function runs **every frame** (the equivalent of Slate's `TAttribute<T>` lambda). Convenient but expensive — the [UMG optimization guidelines](https://dev.epicgames.com/documentation/unreal-engine/optimization-guidelines-for-umg-in-unreal-engine) call them "inefficient" and recommend avoiding at scale.
3. **MVVM / View Models** (UE 5.1+) — the official Model-View-ViewModel plugin. Declarative data-source-to-widget bindings, change-driven (not per-frame), reflection-registered.

## Lifecycle hooks on `UUserWidget`

A custom `UUserWidget` subclass (i.e. any WBP) exposes Blueprint-overridable hooks:

- `NativeConstruct` / `Construct` — fires after the widget tree is built and added to the viewport (analog to Slate's `Construct(FArguments)`).
- `NativeDestruct` / `Destruct` — fires when the widget is removed from the viewport / garbage collected.
- `NativeTick` / `Tick` — per-frame; the most expensive entry point. Best-practice: don't override it.
- `NativeOnFocusReceived` / `NativeOnKeyDown` / `NativeOnMouseButtonDown` / `NativeOnPaint` etc. — Slate event-routing reflected up to UMG.

## Adding a UMG widget to the viewport

Game code adds a Widget Blueprint to the screen with:

```cpp
UUserWidget* MainMenu = CreateWidget<UUserWidget>(GetWorld(), WBP_MainMenuClass);
MainMenu->AddToViewport();
```

`AddToViewport` calls `TakeWidget()` on the root, then inserts the resulting Slate widget into the viewport's overlay (`GameViewportClient`'s `SViewport` child). From that point on, the Slate runtime owns rendering and input — UMG is just the asset and the Blueprint-handler surface.

## Cross-link

For the `S`-prefix side of every widget, see [`slate-architecture.md`](slate-architecture.md). For side-by-side widget tables, see [`widget-vocabulary.md`](widget-vocabulary.md).

## Sources

- UMG UI Designer in Unreal Engine — https://dev.epicgames.com/documentation/en-us/unreal-engine/umg-ui-designer-in-unreal-engine
- Widget Blueprints in UMG — https://dev.epicgames.com/documentation/en-us/unreal-engine/widget-blueprints-in-umg-for-unreal-engine
- UMG UI Designer Quick Start Guide — https://dev.epicgames.com/documentation/unreal-engine/umg-ui-designer-quick-start-guide-in-unreal-engine
- UMG Best Practices — https://dev.epicgames.com/documentation/en-us/unreal-engine/umg-best-practices-in-unreal-engine
- Optimization Guidelines for UMG — https://dev.epicgames.com/documentation/unreal-engine/optimization-guidelines-for-umg-in-unreal-engine
- Unreal Engine 4.5 Release Notes — https://www.unrealengine.com/en-US/blog/unreal-engine-45-released
- UPanelWidget API — https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/UMG/Components/UPanelWidget
- UCanvasPanel API — https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/UMG/UCanvasPanel
