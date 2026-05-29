**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — UGUI (legacy) architecture: Canvas + RectTransform + CanvasRenderer + EventSystem + LayoutGroup family

# UGUI architecture

UGUI (`com.unity.ugui`, originally shipped Unity 4.6 December 2014, internal codename "Unity UI") is Unity's **GameObject-based** UI system. Every UI element is a `GameObject` in the Scene; every UI element carries a `RectTransform` (a GameObject `Transform` specialised for 2D rect-based layout); rendering goes through a per-element `CanvasRenderer`; input goes through the `EventSystem` singleton + per-element `IPointerHandler` and friends. Despite being twelve years old and officially superseded by UI Toolkit for new code, **UGUI is still Unity's officially-recommended runtime UI** as of Unity 6.3 LTS (per Unity's own "Comparison of UI systems" page — `"uGUI is the recommended solution for ..."`).

## The core pieces

- **`Canvas`** — root of any UGUI hierarchy. Three render modes: **Screen Space — Overlay** (drawn after everything, no camera), **Screen Space — Camera** (drawn into a camera's near plane), **World Space** (a 3D quad — the original world-space UI primitive, available since 2014). A scene can have multiple Canvases; each becomes a draw-call batch root.
- **`CanvasScaler`** — companion component on the same GameObject as `Canvas`. Resolves "design resolution" + "physical resolution" + "scale mode" (constant pixel size / scale-with-screen-size / constant physical size). The DPI / multi-resolution story is solved here.
- **`RectTransform`** — every UI element's transform. Extends `Transform` with `anchorMin`, `anchorMax`, `anchoredPosition`, `sizeDelta`, `pivot`. Anchors define the **rect** in the parent's coordinate space; pivot is the rotation/scale origin. The anchor system is UGUI's responsive-layout primitive — anchors at `(0,0)` / `(1,1)` make a child stretch with its parent; anchors collapsed to `(0.5, 0.5)` make a fixed-size child.
- **`GraphicRaycaster`** — sits on the same GameObject as `Canvas`. Performs UI hit-testing for the `EventSystem`. Each Canvas has its own raycaster; raycasters are ordered by Canvas sort order.
- **`EventSystem`** — singleton scene GameObject. Dispatches pointer/touch/keyboard/submit/cancel events through `IPointerHandler` / `ISelectHandler` / `ISubmitHandler` / `IDragHandler` etc. interfaces on UI-element MonoBehaviours. One `EventSystem` per scene.
- **`CanvasRenderer`** — internal component on every visual element. Manages the per-element mesh + material that goes to the GPU. Most UGUI developers never touch this directly; `Image` / `RawImage` / `Text` write into it.
- **`Graphic` (base class)** — `Image`, `RawImage`, `Text`, `TextMeshProUGUI` all derive from `Graphic`. `Graphic` knows how to fill its `RectTransform` rect into the parent `CanvasRenderer`'s mesh.
- **`Selectable` (base class)** — `Button`, `Toggle`, `Slider`, `Scrollbar`, `Dropdown`, `InputField` all derive from `Selectable`. Provides the focused/hovered/pressed/disabled visual states + navigation-via-EventSystem hookup.

## The layout system

UGUI's layout is **anchor-driven by default + opt-in computed layout via the LayoutGroup family**. Three concepts compose:

- **`LayoutGroup`** (abstract). Concrete: `HorizontalLayoutGroup`, `VerticalLayoutGroup`, `GridLayoutGroup`. Each lays out direct children inside the group's `RectTransform` rect. Children's `RectTransform` is *overwritten* each frame the layout group runs.
- **`LayoutElement`** — opt-in component on a child to override min/preferred/flexible width+height that the group uses when computing children's sizes.
- **`ContentSizeFitter`** — opt-in component on a layout group's GameObject that resizes the group's own `RectTransform` to fit its computed children. The composition of `LayoutGroup` (resize children) + `ContentSizeFitter` (resize self) + nested anchors gives UGUI a flex-like vocabulary, though much less expressive than Flexbox.

**Layout rebuild** is dirty-tracked at the Canvas level. `LayoutRebuilder.MarkLayoutForRebuild()` queues a rebuild for the next `Canvas.willRenderCanvases` callback. A single Canvas's rebuilds batch; this is the canonical UGUI perf rule — "split your UI across multiple Canvases so each Canvas's dirty set is small."

## The render pipeline

- Each `Canvas` batches its descendant `CanvasRenderer` meshes into draw calls grouped by material + texture.
- Sort order within a Canvas follows the GameObject hierarchy depth-first; `Canvas.sortingOrder` orders between Canvases.
- A change to any descendant's mesh (text re-shape, image swap, color change) **invalidates the Canvas's batched mesh** — the whole Canvas re-batches. This is the source of UGUI's "many elements changing per frame" perf cliff and the standard remedy (split into static vs dynamic Canvases).
- Custom shader effects are per-`Graphic` via `material` field on `Image` / `RawImage` / `Text`. No global stylesheet, no `mix-blend-mode`, no `backdrop-filter`, no true top layer, no `clip-path`.
- Clipping is `RectMask2D` (rect-only) or `Mask` (alpha-masked, more expensive). Rounded clipping is *not* native — projects either ship a custom mask shader or use a `Mask` with a rounded-rect texture.

## What UGUI does well

- **In-Scene WYSIWYG authoring.** A UI artist drags components in the Scene view; the Game view updates live. UI Toolkit cannot match this.
- **World-space UI is first-class.** A `Canvas` in World Space render mode is just a quad in 3D space. Health bars, name plates, diegetic UI all work without ceremony.
- **Animation timeline integration.** Unity's Animator/Animation system retargets `RectTransform` and `Graphic` properties directly. UI Toolkit only has USS transitions; no keyframed animation per Unity's own comparison table.
- **Custom shaders per element.** Drop a custom material on an `Image` and you have a shader-driven UI element. UI Toolkit's custom-shader story is centralised (panel-level mesh shaders) and less ergonomic for one-off effects.
- **Twelve years of community polish.** DoozyUI, NGUI patterns, Asset Store widget kits, well-trod perf practice ("split Canvases", "disable Raycast Target where you don't need it", "pool list items"). The UGUI knowledge corpus is enormous.

## What UGUI does badly

- **No global stylesheet.** Theming is per-Graphic field-tweak. The `Selectable.colors` struct is the only built-in "theme" surface — four colors (normal/hover/pressed/disabled) per Selectable. Any consistent visual language must be implemented by the project (per-component-prefab patterns are typical).
- **Performance at scale.** A Canvas with 1000+ active `CanvasRenderer`s churning text every frame is a known perf cliff. Unity's own perf docs lead with "split into multiple Canvases" as the first remedy.
- **Layout group cost.** `HorizontalLayoutGroup` / `VerticalLayoutGroup` rebuild their children's `RectTransform`s every dirty frame; for medium-large lists this dominates. Unity's optimization guidelines `"advise limiting its use."` (Source: Angry Shark Studio comparison.)
- **Limited responsive-layout vocabulary.** Anchors + LayoutGroups + ContentSizeFitter are powerful but ad-hoc; nothing like Flexbox's `flex-grow` / `flex-shrink` axis cleanly maps. Multi-axis layouts with `min`/`max` constraints commonly require custom MonoBehaviours.
- **No accessibility hook.** UGUI ships zero accessibility integration. The `UnityEngine.Accessibility` module (2023.2+) can describe a UGUI scene but requires per-element manual annotation; there is no automatic role inference. Third-party `mikrima/UnityAccessibilityPlugin` (UAP) is the de facto bridge for screen-reader support.

## Pros/cons in 2026

| Axis | UGUI 2026 verdict |
|---|---|
| Maturity | **Highly mature** — twelve years of production use across millions of titles |
| Documentation | Extensive (Unity Manual + community + DoozyUI patterns) |
| Performance ceiling | Moderate — known cliffs at 1000+ active CanvasRenderers; mitigation patterns well-known |
| Web-platform parity | None (anchors + LayoutGroup, not Flexbox; no stylesheet; no `clip-path`/`backdrop-filter`) |
| Accessibility | Nothing built-in; Unity 2023.2 Accessibility module is opt-in per element |
| Animation | Best-in-class (Animator/Animation timeline) |
| World-space UI | First-class since 2014 |
| Official status | "Recommended for runtime" per Unity's own comparison page |

## Implications for Buiy

UGUI is the **anti-pattern** Buiy's foundation §2.4 (BSN-native components) and §3.1 (web-flavored authoring) is designed against:

- **Field-tweak theming is the megacomponent problem at scale.** Every UGUI `Selectable` carries a `ColorBlock` struct with 4 colors; that's a megacomponent. Buiy's decomposed-components rule (foundation §2.4) is the direct corrective.
- **Layout via per-element transforms vs declarative tree.** UGUI's anchor model is per-element imperative; UI Toolkit + Buiy use declarative Flexbox via Yoga / Taffy. Buiy follows the declarative path.
- **World-space UI is first-class even in 2014 retained-mode systems.** This validates Buiy's foundation `buiy_3d` sub-spec scope (architecture.md §2.3: "3D-anchored / diegetic UI — first-class achievable").
- **Animation integration is genuinely better than UI Toolkit's.** UI Toolkit's no-keyframe-animation gap (per Unity's own table) is a lesson — Buiy's animation sub-spec (`buiy-animation-design`) should commit to keyframes day-one, not transitions-only.
- **Per-Canvas batch invalidation is a render-pipeline anti-pattern.** Buiy's render pipeline (foundation architecture.md §2.3) avoids the single-batch-per-tree topology in favour of dirty-region tracking.

## Sources

- UGUI manual — https://docs.unity3d.com/Packages/com.unity.ugui@latest/
- Canvas component reference — https://docs.unity3d.com/Packages/com.unity.ugui@2.6/manual/class-Canvas.html
- Comparison of UI systems in Unity — https://docs.unity3d.com/6000.3/Documentation/Manual/UI-system-compare.html
- Unity UI optimization tips — https://unity.com/how-to/unity-ui-optimization-tips
- Angry Shark Studio — UI Toolkit vs UGUI 2025 — https://www.angry-shark-studio.com/blog/unity-ui-toolkit-vs-ugui-2025-guide/
- Optimizing LayoutElement and LayoutGroup — https://llmagicll.medium.com/optimizing-ui-performance-in-unity-deep-dive-into-layoutelement-and-layoutgroup-components-b6a575187ee4
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
