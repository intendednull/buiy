**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — UI Toolkit (modern) architecture: UXML, USS, VisualElement, Yoga, data binding, runtime + Editor unified

# UI Toolkit architecture

UI Toolkit (`com.unity.ui`; originally **UIElements**, renamed during the Unity 2020.1 beta cycle, runtime-stable from Unity 2021 LTS) is Unity's web-platform-inspired UI stack. Three asset/code surfaces compose: **UXML** (XML hierarchy markup, HTML analog), **USS** (Unity Style Sheets, CSS subset), and **C#** (the `VisualElement` API). It runs the same code in the **Editor** (where it has been progressively replacing IMGUI since Unity 2019.1) and at **runtime** (where it competes with UGUI for new project UIs). It is the direct precedent for what Buiy is building.

## The three asset surfaces

### UXML — the markup
- XML file with the `<UXML>` root and a tree of `<VisualElement>` and subclasses (`<Button>`, `<TextField>`, `<Label>`, `<ScrollView>`, `<ListView>`, `<TreeView>`, `<Foldout>`, `<Toggle>`, `<Slider>`, `<DropdownField>`, `<Tab>`, `<TabView>`, `<RadioButtonGroup>`, etc.). The full element set is documented in the [UXML Elements Reference](https://docs.unity3d.com/Manual/UIE-uxml-element-VisualElement.html).
- Custom elements registered via `UxmlElement`/`UxmlFactory` (older Unity versions) or attribute-decorated `VisualElement` subclasses (Unity 2023.2+).
- Reused via `<Instance template="..."/>` for composition.
- Edited textually or via the **UI Builder** WYSIWYG editor (shipped in Unity 2021 LTS+).

### USS — the styles
- Plain-text stylesheet with **CSS-syntax-identical** rules. Per Unity: *"USS syntax is the same as CSS syntax, but USS includes overrides and customizations to work better with Unity."*
- **Selectors:** type (`Button`), class (`.my-class`), name (`#my-id`), pseudo (`:hover`, `:active`, `:focus`, `:disabled`, `:checked`, `:root`), descendant + child combinators.
- **Properties supported (verified):** flex-* family, align-*, justify-content; width/height/min-/max-; padding/margin/border-*; background-color, background-image, color, font-size, opacity; position (relative/absolute), top/right/bottom/left; transform (translate/rotate/scale); transition-* family; visibility, display (only `flex` or `none`); cursor; overflow.
- **Unity-prefixed properties (`-unity-*`)** cover features without CSS analog: `-unity-font`, `-unity-font-definition`, `-unity-font-style`, `-unity-text-align`, `-unity-text-outline-*`, `-unity-background-scale-mode`, `-unity-background-image-tint-color`, `-unity-slice-*` (9-slice), `-unity-material`, `-unity-paragraph-spacing`.
- **Custom properties (CSS variables):** supported (`--my-token: ...; ... var(--my-token)`).
- **What is NOT in USS:** CSS Grid, container queries, anchor positioning, `calc()`, media queries, keyframe animations (`@keyframes`), `mix-blend-mode`, `backdrop-filter`, `filter`, `clip-path` (only rectangular clipping via overflow + masking effects + post-processing filters via separate APIs), elliptical border-radius, multiple cursor fallbacks. See [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md) for the full audit.

### C# — the API
- `VisualElement` is the base type for every UI node. Properties: `style` (writable inline style — same surface as USS), `name`, `classList`, `parent`, `children`, `userData`, `tooltip`, `focusable`, `pickingMode`, etc.
- **UQuery** (`element.Query<T>()`, `element.Q<T>(name, className)`) is the LINQ-style selector API — comparable to `document.querySelector` + iteration.
- Event model: `RegisterCallback<MouseDownEvent>(handler)`, `RegisterCallback<KeyDownEvent>(handler)`, etc. — strongly-typed event classes with bubbling + trickle-down phases.
- `schedule.Execute(...).Every(ms)` for scheduled callbacks (the timer/animation hook before USS transitions).

## VisualElement tree — the retained-mode data structure

- Single tree per **panel**. A panel is owned by a `UIDocument` MonoBehaviour (runtime) or `EditorWindow` (Editor). Multiple panels per scene/Editor; each panel renders to its own mesh.
- VisualElements are *not* GameObjects. There is one `UIDocument` GameObject; everything under it is plain managed objects. This is the central perf win over UGUI — a 10,000-element list is one GameObject, not 10,000.
- Each `VisualElement` carries a `style` block (resolved cascaded USS) and a Yoga layout node. Yoga (Facebook's flexbox engine) is the layout solver.
- Rendering is **single batched mesh per panel** where possible — atlased textures, instanced vertex data, no per-element draw call.

## Yoga — the layout engine

- Facebook's [Yoga](https://yogalayout.com/) is a C++ flexbox implementation; Unity ships a C# binding. Yoga is also used by React Native, Litho, and other cross-platform UI runtimes.
- Implements a **subset of Flexbox**: `flex-direction`, `flex-wrap`, `align-items`, `align-content`, `justify-content`, `align-self`, `flex-grow`, `flex-shrink`, `flex-basis`, `position` (relative/absolute), insets.
- **Does NOT implement CSS Grid, subgrid, container queries, anchor positioning.** Unity inherits these gaps from Yoga.
- Layout is computed lazily; dirty propagation is per-VisualElement.

## Data binding — SerializedObject + custom bindings

Two binding flavours, both Unity-2023.2+:

- **SerializedObject binding** — `PropertyField` element binds to a serialized property on a `UnityEngine.Object` (the same surface IMGUI's `SerializedProperty` exposed). Editor-first; the standard way to build an Inspector.
- **Runtime data binding** — `dataSource` + `dataBindings` on a VisualElement; supports binding paths into POCO data, two-way for `INotifyBindablePropertyChanged`. Newer (Unity 2023.2+ stable); the closest analog to React/Vue/Svelte reactive binding inside Unity.
- **Custom bindings** — `CustomBinding` subclass for binding USS classes / arbitrary state to VisualElement properties. The "bind a USS class to a model field" pattern lives here.

## Editor + runtime unified

UI Toolkit is the **same code path** in the Editor (where it backs Inspector windows, the Asset Browser, settings panels, etc.) and at runtime (where `UIDocument` projects a panel onto a Canvas-like screen layer). This is a deliberate win over IMGUI (Editor-only) and UGUI (runtime-only): a custom control is portable.

In practice, the Editor uses USS asset chains for theming (Unity's dark + light Editor themes are USS) and the runtime carries the project's own USS. Editor migration from IMGUI is ongoing (see [`editor-ui-migration.md`](editor-ui-migration.md)).

## World-space UI

- Added Unity 6.2 (December 2025) per Unity's release notes and the official world-space UI tutorial.
- A `UIDocument` can be set to **World Space** render mode; the panel renders to a quad in 3D space.
- `Panel Settings` exposes `Pixels Per Unit` (default 100 panel pixels per world unit) and a `Collider Update Mode` for the auto-generated collider.
- This closes a years-old gap — UGUI had world-space Canvas since 2014.

## The render pipeline

- Per-panel batched mesh: VisualElement vertex data is built into a single mesh (or a small number) at draw time. Texture atlases batch image fills.
- "Textureless UI rendering" is a documented feature — rectangular fills + borders + text use vertex-attribute encoding rather than per-element textures, which UI Toolkit can do more aggressively than UGUI's per-element `CanvasRenderer`.
- Custom shaders attach via `-unity-material` USS property (panel-level) or VisualElement `style.unityBackgroundImageTintColor` + custom shader on the material.
- Limitations carried into 2026: **no mix-blend-mode, no backdrop-filter, no true CSS top layer** (the `:popover` / `<dialog>` analog is Unity-specific via panel layering, not a CSS top-layer-style escape from stacking contexts). Rounded clipping is supported via `border-radius` + `overflow: hidden`.

## Comparison to web DOM/CSS

| Web | UI Toolkit |
|---|---|
| HTML | UXML |
| DOM | VisualElement tree |
| CSS | USS (subset + Unity prefixes) |
| CSS Flexbox | Yoga (subset) |
| CSS Grid | Not supported |
| Container queries | Not supported |
| Anchor positioning | Not supported |
| `calc()` | Not supported |
| Custom properties / `var()` | Supported |
| `@keyframes` | Not supported (USS transitions only) |
| `:hover` / `:focus` / `:active` | Supported (+ `:checked`, `:disabled`, `:root`) |
| `mix-blend-mode` | Not supported |
| `backdrop-filter` | Not supported |
| ARIA | Not supported (no role/state model) |
| `document.querySelector` | `UQuery` (`Q<T>()`, `Query<T>()`) |
| Event bubbling / capture | Supported (`TrickleDown` / bubble phases) |
| DevTools (Inspect element) | **UI Toolkit Debugger** (Editor window) |
| Data binding | `SerializedObject` (Editor) / `dataSource` (runtime) |

## What UI Toolkit does well

- **Performance at element count.** Single batched mesh + no GameObject overhead beats UGUI cleanly at 1000+ elements (per community comparison; not first-party-published benches).
- **Web-developer onboarding.** UXML/USS is familiar enough that a web dev can read it on day one. The mental model transfers.
- **Editor + runtime unified.** Custom widgets are portable between Inspector and gameplay UI.
- **UI Builder.** The WYSIWYG editor for UXML/USS shipped Unity 2021 LTS+; the equivalent of a browser DevTools "elements" panel + visual style editor combined.
- **UI Toolkit Debugger.** Live inspection of the VisualElement tree, matched USS selectors, layout boxes — a genuine DevTools-grade tool.

## What UI Toolkit does badly

- **Runtime maturity.** Per Unity's own comparison page (Unity 6.3 manual): UI Toolkit `"is in active development and releases new features frequently. uGUI and IMGUI are established and production-proven UI systems that are updated infrequently."` Translation: UI Toolkit is officially the *less-stable* runtime UI as of 2026, even though it is the future.
- **No keyframed animation.** USS transitions only. Sequence animation requires C# `schedule` loops or Animator integration on a `UIDocument` GameObject. Unity's own comparison table flags "Keyframed animations: ❌ No" for UI Toolkit.
- **No in-Scene WYSIWYG.** UI Builder edits the asset, not the running scene. Iteration is asset-save + Play → reload, not drag-in-Scene.
- **Missing CSS surface area.** No Grid, no container queries, no anchor positioning, no `calc()`, no media queries. The web parity is **partial**, not complete.
- **Accessibility gap.** UI Toolkit ships no ARIA model; the Accessibility module (2023.2+) is a separate API stack the UXML/USS authoring does not feed into automatically. See [`accessibility.md`](accessibility.md).
- **World-space lag.** Unity 6.2 (late 2025) finally added world-space UI Toolkit — UGUI has had it since 2014.

## Implications for Buiy

- **The VisualElement tree pattern validates Buiy's parallel-stack bet** (foundation §2.1). A retained-mode tree with declarative styling outside Bevy's ECS would be wrong for Buiy, but the *shape* — one tree per panel, declarative authoring, single-batched mesh — is the right shape; Buiy adapts it onto ECS entities.
- **Yoga's missing-Grid problem is Taffy's solved problem.** Taffy (Buiy's substrate) has CSS Grid since 0.3 and is adding subgrid; Buiy inherits Grid for free, where UI Toolkit cannot.
- **USS-vs-CSS divergences are an onboarding tax.** Buiy's foundation §5 ("CSS-flavored stylesheet — never, or future layer?") should weigh this: USS deviates in small ways (no `calc()`, no `@keyframes`, `display` only `flex|none`) that web devs hit repeatedly. If Buiy ever ships a stylesheet layer, **commit to true CSS semantics where possible**; let the deviations be additions (Buiy-only tokens), not subtractions.
- **The UI Toolkit Debugger is the spec for Buiy devtools** (foundation §2.3). Match its scope: tree inspection, matched USS selectors, layout box visualization, focus order, accessibility tree viewer.
- **No-keyframes is the lesson.** UI Toolkit's transitions-only choice is now a known regret; Buiy's `buiy-animation-design` sub-spec should not repeat it.

## Sources

- UI Toolkit manual landing — https://docs.unity3d.com/Manual/UIToolkits.html
- UXML VisualElement reference — https://docs.unity3d.com/Manual/UIE-uxml-element-VisualElement.html
- USS supported properties — https://docs.unity3d.com/Manual/UIE-USS-SupportedProperties.html
- USS landing page — https://docs.unity3d.com/Manual/UIE-USS.html
- UQuery — https://docs.unity3d.com/Manual/UQuery.html
- SerializedObject data binding — https://docs.unity3d.com/Manual/UIE-Binding.html
- World Space UI Toolkit tutorial — https://unity.com/resources/how-to-create-world-space-ui-toolkit
- UI system comparison (Unity 6.3) — https://docs.unity3d.com/6000.3/Documentation/Manual/UI-system-compare.html
- Renaming UIElements to UI Toolkit — https://discussions.unity.com/t/renaming-uielements-to-ui-toolkit/782459
- Yoga layout engine — https://yogalayout.com/
- Performance considerations for runtime UI — https://docs.unity3d.com/Manual/UIE-performance-consideration-runtime.html
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
