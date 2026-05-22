**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — system-specific terms used across this corpus

# Glossary

Definitions for Unity-UI-specific identifiers, type names, and ecosystem terms used throughout this corpus. Cross-link liberally; do not duplicate definitions in evidence files — point at this glossary instead.

## The two UI stacks

- **UGUI** — `com.unity.ugui`; "Unity UI"; the GameObject-based retained UI system shipped Unity 4.6 (Dec 2014). Canvas + RectTransform + CanvasRenderer + EventSystem + LayoutGroup family. Still officially recommended for runtime as of Unity 6.3 LTS.
- **UI Toolkit** — `com.unity.ui`; the web-platform-inspired UI system. UXML + USS + VisualElement + C# API + Yoga. Debuted as **UIElements** in Unity 2017→2019.1, renamed during Unity 2020.1 beta, runtime-stable from Unity 2021 LTS.
- **UIElements** — Original name for UI Toolkit pre-2020.1. Still appears in legacy documentation, code, and asset paths.
- **IMGUI** — `OnGUI()` immediate-mode code-driven UI. The legacy pre-UGUI system; now Editor-only-recommended per Unity 6.3.

## UGUI types

- **`Canvas`** — root of any UGUI hierarchy. Three render modes (Overlay / Camera / World Space).
- **`CanvasScaler`** — companion to `Canvas`. Resolves design vs physical resolution + scale mode.
- **`CanvasRenderer`** — internal per-element mesh + material manager. Generally not user-facing; `Graphic` writes into it.
- **`RectTransform`** — `Transform` specialization for 2D rect-based layout. Anchors + pivot + sizeDelta + anchoredPosition.
- **`Graphic`** — abstract base for visual elements (`Image`, `RawImage`, `Text`, `TextMeshProUGUI`).
- **`Selectable`** — abstract base for interactive elements (`Button`, `Toggle`, `Slider`, `Scrollbar`, `Dropdown`, `InputField`).
- **`GraphicRaycaster`** — per-Canvas hit-tester for `EventSystem`.
- **`EventSystem`** — singleton scene GameObject that dispatches pointer/keyboard/submit/cancel events through `IPointerHandler` / `ISelectHandler` etc. interfaces.
- **`LayoutGroup`** — abstract; concrete: `HorizontalLayoutGroup`, `VerticalLayoutGroup`, `GridLayoutGroup`. Lays out direct children inside the group's `RectTransform`.
- **`LayoutElement`** — opt-in component overriding min/preferred/flexible width+height for layout-group computation.
- **`ContentSizeFitter`** — opt-in component that resizes a layout group's own `RectTransform` to fit its computed children.
- **`Mask` / `RectMask2D`** — clipping primitives. `RectMask2D` is cheap rect-only; `Mask` is alpha-masked + more expensive.
- **`ColorBlock`** — struct on `Selectable` holding normal/highlighted/pressed/selected/disabled colors. The canonical "megacomponent struct" anti-pattern.

## UI Toolkit types

- **`VisualElement`** — base class for every UI Toolkit node. Carries `style`, `name`, `classList`, `children`, `parent`, `userData`, `tooltip`, `focusable`, `pickingMode`. Not a GameObject.
- **`UIDocument`** — MonoBehaviour that hosts a UI Toolkit panel in a Scene. References UXML + USS + PanelSettings assets.
- **`PanelSettings`** — ScriptableObject configuring a UI Toolkit panel's rendering (sort order, scale mode, depth, world-space settings).
- **`UXML`** — XML markup asset format for UI Toolkit hierarchies. Element names map to `VisualElement` subclasses via reflection / factory pattern.
- **`USS`** — Unity Style Sheets. CSS-syntax-identical text format; CSS-subset semantics.
- **`UQuery`** — LINQ-style query API. `element.Q<T>(name, className)` and `element.Query<T>().Where(...)`.
- **`UI Builder`** — WYSIWYG editor for UXML + USS. Ships in Unity 2021 LTS+.
- **`UI Toolkit Debugger`** — Editor window for live tree inspection (matched USS selectors, layout boxes, event log).
- **`<TextElement>` / `<Label>` / `<TextField>` / `<Button>`** — built-in UI Toolkit elements.
- **`<ListView>` / `<MultiColumnListView>` / `<TreeView>`** — virtualized list elements.
- **`PropertyField`** — Editor element binding to a `SerializedProperty` by path.
- **`-unity-*` USS properties** — Unity-specific CSS-property extensions. Examples: `-unity-font`, `-unity-font-definition`, `-unity-text-align`, `-unity-text-outline-*`, `-unity-background-scale-mode`, `-unity-slice-*`, `-unity-material`.
- **Yoga** — Facebook's open-source flexbox layout engine. UI Toolkit's layout substrate.

## Text

- **TextMesh Pro (TMP)** — SDF-based text renderer acquired from Stephan Bouchard in 2017. Default text engine for UGUI's `TextMeshProUGUI` and the underlying engine inside UI Toolkit's `<TextElement>`.
- **TextCore** — TMP-derived text engine inside UI Toolkit. Same font assets as TMP; same SDF rendering.
- **TMP font asset** — Unity-specific baked-from-`.ttf`/`.otf` asset containing SDF atlas + glyph metrics + kerning + fallback chain.
- **Dynamic SDF Atlas** — Unity 2020+ mode that rasterises new glyphs on demand at runtime, addressing the explode-asset-size CJK problem.
- **Sprite Asset** — Unity-specific asset for inline emoji / button-prompt glyphs in text runs.

## Accessibility

- **`UnityEngine.Accessibility`** — built-in module since Unity 2023.2 Tech Stream.
- **`AccessibilityHierarchy`** — per-game accessibility tree, similar in shape to a browser accessibility tree but without ARIA.
- **`AccessibilityNode`** — per-element node with role/label/value/state.
- **`AssistiveSupport`** — singleton brokering OS AT communication; provides `NotificationDispatcher.SendAnnouncement(...)`.
- **Accessibility Hierarchy Viewer** — Editor window showing the live accessibility tree in Play mode.
- **UAP (UI Accessibility Plugin)** — `mikrima/UnityAccessibilityPlugin`; community Asset Store screen-reader bridge predating the 2023.2 module.

## Ecosystem

- **DoozyUI** — commercial UGUI widget kit; visual flow editor (Nody), animation engine (Reactor), prefab manager (UI Menu), messaging (Signals).
- **NGUI** — pre-UGUI Asset Store UI system by Tasharen Entertainment; widely used 2011-2014; preceded and informed UGUI.
- **App UI** — `com.unity.dt.app-ui`; first-party Unity Package productivity-app widget kit on UI Toolkit. Material-Design-flavoured + accessibility primitives.
- **MoreMountains Feel** — UGUI animation / juice / feedback kit.
- **Unity UI Extensions** — open-source community-maintained UGUI control collection.

## Governance / business

- **Unity Technologies** — the company. NYSE: U. San Francisco HQ.
- **Bevy Foundation (for contrast)** — Bevy's 501(c)(3) public charity governance; Buiy and bevy_ui are downstream of this. See [`../bevy-ui/governance.md`](../bevy-ui/governance.md).
- **Runtime Fee (2023)** — per-install fee policy announced 2023-09-13; partial walk-back 2023-09-22; **fully cancelled 2024-09-12**.
- **Unity 6** — current LTS family. Unity 6.0 released 2024-10-17; Unity 6.3 LTS is current (December 2025). Re-branded LTS line distinct from prior 2019/2020/2021/2022/2023 naming.
- **LTS** — Long-Term Support release. Yearly cadence; biweekly fixes for two years.
- **Tech Stream** — non-LTS intermediate release. New features land here first.

## Cross-corpus cross-references

- **AccessKit** — Buiy's accessibility substrate (foundation §2.6). Bevy's substrate too. *No Unity equivalent.* See [`../accesskit/`](../accesskit/).
- **Taffy** — Buiy's layout substrate (foundation §2.2). bevy_ui's too. *Yoga is Unity's equivalent — Flexbox subset only; no Grid.* See [`../bevy-ui/layout.md`](../bevy-ui/layout.md).
- **cosmic-text** — Buiy's text substrate. *TextMesh Pro / TextCore is Unity's equivalent.* See [`../bevy-ui/text-and-input.md`](../bevy-ui/text-and-input.md).
- **BSN** — Bevy's reflection-driven asset format for declarative authoring (PR #20158, draft). *UXML is Unity's equivalent.* See [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md).
- **APG (ARIA Authoring Practices Guide)** — W3C set of widget keyboard contracts Buiy commits to (foundation media-and-widgets.md). *No Unity equivalent.* See https://www.w3.org/WAI/ARIA/apg/.

## Sources

- UGUI manual — https://docs.unity3d.com/Packages/com.unity.ugui@latest/
- UI Toolkit manual — https://docs.unity3d.com/Manual/UIToolkits.html
- USS supported properties — https://docs.unity3d.com/Manual/UIE-USS-SupportedProperties.html
- Unity Accessibility manual — https://docs.unity3d.com/6000.3/Documentation/Manual/accessibility.html
- UXML Elements Reference — https://docs.unity3d.com/Manual/UIE-uxml-element-VisualElement.html
