**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — Slate's declarative C++ retained-mode architecture

# Slate architecture

Slate is Unreal's **declarative, retained-mode, C++ UI framework**. It is the load-bearing UI runtime for both the Unreal Editor (entirely written in Slate) and game-side UI (which is usually authored in UMG, which sits on top of Slate). Source lives in `Engine/Source/Runtime/Slate/` and `Engine/Source/Runtime/SlateCore/`.

## The `S`-prefix and the class hierarchy

Every Slate widget class is prefixed `S`. The hierarchy is rooted at `SWidget`:

```
SWidget                          // abstract base; defines layout + paint + input + accessibility
├── SLeafWidget                  // no children (STextBlock, SImage, SSpacer)
├── SCompoundWidget              // owns a single ChildSlot, composes other widgets
│   ├── SButton
│   ├── SBorder
│   ├── SBox
│   └── (most widgets you ever write)
├── SPanel                       // multi-child layout containers
│   ├── SHorizontalBox / SVerticalBox
│   ├── SOverlay
│   ├── SUniformGridPanel / SGridPanel
│   ├── SWrapBox
│   ├── SScrollBox
│   └── SConstraintCanvas
└── (other internal subclasses)
```

`SCompoundWidget` is the workhorse — almost every custom widget extends it. It has exactly one `ChildSlot` member; the constructor fills the slot with whatever widget tree the author declares.

## The `SNew` declarative DSL

Slate's most distinctive trait is the `SNew` macro chain. Constructing a widget tree looks like this:

```cpp
TSharedRef<SButton> Button = SNew(SButton)
    .ButtonStyle(FAppStyle::Get(), "PrimaryButton")
    .OnClicked_Raw(this, &FMyClass::HandleClick)
    [
        SNew(SHorizontalBox)
        + SHorizontalBox::Slot().AutoWidth().VAlign(VAlign_Center)
        [
            SNew(SImage).Image(FAppStyle::GetBrush("Icons.Save"))
        ]
        + SHorizontalBox::Slot().FillWidth(1.f).Padding(4, 0)
        [
            SNew(STextBlock).Text(LOCTEXT("Save", "Save"))
        ]
    ];
```

Three macros do the heavy lifting:

- **`SNew(WidgetType)`** — expands to `MakeShared<WidgetType>(...)` plus an `FArguments` proxy. The proxy is the thing on which `.ButtonStyle(...)`, `.OnClicked_Raw(...)`, etc. chain.
- **`[ ... ]`** — the **slot operator**. After the argument chain, `[]` accepts the child-widget expression that fills `ChildSlot` (or the named slot the argument chain ended on, like `.Content()[ ... ]`).
- **`+ Container::Slot()`** — for multi-slot containers, each child is added with `+ SHorizontalBox::Slot().Padding(...).FillWidth(...)[ child ]`. The slot itself carries layout rules.

A second flavor — `SAssignNew(MemberPtr, WidgetType)` — does the same but also assigns the constructed widget to a `TSharedPtr` member for later access.

## `FArguments`: the slot/attribute/event DSL

Each `S`-prefixed class declares its construction-time interface inside a `SLATE_BEGIN_ARGS` / `SLATE_END_ARGS` block. This generates a nested `FArguments` struct with chainable setters:

```cpp
class SSubMenuButton : public SCompoundWidget
{
public:
    SLATE_BEGIN_ARGS(SSubMenuButton)
        : _ShouldAppearHovered(false)
    {}
        SLATE_ATTRIBUTE(FText,    Label)
        SLATE_ATTRIBUTE(bool,     ShouldAppearHovered)
        SLATE_EVENT(FOnClicked,   OnClicked)
        SLATE_NAMED_SLOT(FArguments, Content)
    SLATE_END_ARGS()

    void Construct(const FArguments& InArgs);
};
```

Four macro flavors cover the surface:

| Macro | Use |
|---|---|
| `SLATE_ARGUMENT(Type, Name)` | Plain value, set once at construction. |
| `SLATE_ATTRIBUTE(Type, Name)` | Bindable value — can be a literal *or* a `TAttribute` that re-evaluates each frame (the "bound attribute" mechanism). |
| `SLATE_EVENT(DelegateType, Name)` | A delegate callback (`OnClicked`, `OnTextChanged`, etc.). |
| `SLATE_NAMED_SLOT(Args, Name)` | A named child slot (`Content`, `Header`, `Footer`); the consumer fills it with `.Name() [ child ]`. |

`SLATE_BEGIN_ARGS` + `SLATE_END_ARGS` literally expand to a `struct FArguments { ... }` definition; the field names get a leading underscore (`_Label`, `_OnClicked`) and the chainable setter is the un-underscored name.

The widget's `Construct(const FArguments& InArgs)` method runs once at construction, reading from `InArgs._Label`, `InArgs._OnClicked`, etc., and building the actual child widget tree. Because `Construct` bodies are notoriously expensive to compile (they're one giant statement of nested templated builders), Slate provides `BEGIN_SLATE_FUNCTION_BUILD_OPTIMIZATION` / `END_SLATE_FUNCTION_BUILD_OPTIMIZATION` macros that disable optimization for the function — a workaround for MSVC compile times.

## Retained-mode pipeline

Slate is retained-mode. Each frame:

1. **Invalidation** — widgets mark themselves dirty when their state changes (`SInvalidationPanel` walls off subtrees that rarely change so they re-paint lazily).
2. **Layout (Pre-pass + Arrange)** — Slate walks the tree, asks each widget for its `ComputeDesiredSize` (intrinsic), then arranges children in each slot per the slot's rules. Layout is its own algorithm (not Taffy, not Flexbox, not Grid) — each container's slot semantics are hard-coded.
3. **Paint** — widgets emit `FSlateDrawElement`s (rects, borders, text runs, lines, splines, custom mesh) into the draw-element buffer.
4. **Render** — `FSlateRenderer` (an RHI-backed renderer, currently D3D11/12 / Vulkan / Metal / OpenGL ES depending on platform) batches and submits the draw elements. The renderer owns its own atlas, glyph cache, and effects pipeline.
5. **Input** — `FSlateApplication` is the global singleton that handles OS input (mouse / keyboard / touch / gamepad) and routes events down the focus path and hit-tested widget chain.

`FSlateApplication::Get()` is the global Slate application object. It owns the focus, drag-drop, modal stack, tooltip system, throttle manager, and `FSlateRenderer` plumbing.

## Built-in widget catalog (Slate)

A non-exhaustive list of the `S`-prefixed widgets that ship in core Slate:

- **Leaf:** `STextBlock`, `SRichTextBlock`, `SImage`, `SSpacer`, `SProgressBar`, `SThrobber`, `SCircularThrobber`.
- **Compound / interactive:** `SButton`, `SCheckBox`, `SHyperlink`, `SEditableText`, `SEditableTextBox`, `SMultiLineEditableText`, `SMultiLineEditableTextBox`, `SSlider`, `SSpinBox`, `SComboButton`, `SComboBox`, `SSearchBox`.
- **Containers / layout:** `SBorder`, `SBox`, `SOverlay`, `SHorizontalBox`, `SVerticalBox`, `SUniformGridPanel`, `SGridPanel`, `SWrapBox`, `SScrollBox`, `SConstraintCanvas`, `SDPIScaler`.
- **Lists / trees / tables:** `SListView`, `STreeView`, `STableViewBase`, `STileView`, `SHeaderRow`.
- **Windows / docking / modal:** `SWindow`, `SDockTab`, `SDockingTabStack`, `SMenuAnchor`, `SToolTip`, `SNotificationList`.
- **Editor-specific:** hundreds of `S*Widget` classes power the editor (`SGraphPanel`, `SDetailsView`, `SAssetPicker`, etc.) — they live in `Editor/` modules and are not available to runtime game code.

## Render-pipeline integration

Slate owns its own renderer. There is no "Slate runs on Unreal's mesh pipeline" abstraction — instead, `FSlateRHIRenderer` translates `FSlateDrawElement`s directly into RHI command lists at the end of the frame. This means Slate has full control over:

- Glyph atlas + glyph cache (`FSlateFontCache`).
- Image atlas for brushes (`FSlateRHIResourceManager`).
- Custom shaders for effects (`SMaterialEditorViewport` etc. use this).
- Per-window draw buffers (`FSlateWindowElementList`).

The trade-off mirrors what Buiy is committing to: owning the renderer unlocks the feature set you need; the cost is owning the maintenance forever.

## Sources

- Slate UI Programming — https://dev.epicgames.com/documentation/en-us/unreal-engine/slate-ui-programming-in-unreal-engine
- Slate Overview — https://dev.epicgames.com/documentation/en-us/unreal-engine/slate-overview-for-unreal-engine
- Understanding the Slate UI Architecture — https://dev.epicgames.com/documentation/en-us/unreal-engine/understanding-the-slate-ui-architecture-in-unreal-engine
- Anatomy of a Widget (Codekitten) — https://codekittah.medium.com/anatomy-of-a-widget-c-unreal-engine-b479a100c7e3
- Slate, Hello (UE4 wiki mirror) — https://michaeljcole.github.io/wiki.unrealengine.com/Slate,_Hello/
- Custom widgets in Unreal (Snorri Sturluson) — https://snorristurluson.github.io/CustomSlateWidgets/
- The Slate UI Framework Part 1 (Gerke Max Preussner) — https://de45xmedrsdbp.cloudfront.net/Resources/files/slateTutorials_westcoast-1963123470.pdf
