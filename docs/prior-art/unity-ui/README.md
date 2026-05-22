**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — the two-stack UI surface (UGUI + UI Toolkit) of the Unity game engine; the strongest existing-art for "web-platform parity inside a game engine"

# Unity UI

Unity Technologies ships **two parallel UI stacks** inside the Unity game engine: **UGUI** (legacy `com.unity.ugui`, retained GameObject-based Canvas tree, shipped 2014 with Unity 4.6 — still the *officially recommended* runtime UI as of Unity 6.3) and **UI Toolkit** (modern, web-inspired UXML/USS/C# stack, introduced as **UIElements** in Unity 2017–2019, renamed to UI Toolkit during the Unity 2020.1 beta cycle, available for runtime from Unity 2021 LTS, world-space runtime UI added in Unity 6.2). Both ship with every Unity installation; both are written by the same team; both target the same renderer. Unity is the **closest game-engine UI predecessor with web-platform-parity ambitions** — UI Toolkit's UXML (HTML analog) + USS (CSS subset) is the direct precedent for Buiy's foundation §2.2 ("comprehensive web platform UI surface inside a game engine") and §2.5 (token-style theming vs USS stylesheets).

This corpus is the prior-art folder for Buiy. It documents BOTH stacks as sister systems within Unity, the institutional history (UGUI → UIElements → UI Toolkit → Editor-UI migration), and the accumulated pain points (Editor-migration cost, USS-vs-CSS divergence, accessibility lag, the 2023 runtime-fee saga). Unity is the most production-proven game-engine UI on Earth — every Unity game uses one or both of these stacks — and its experience is the highest-quality validation we have that "web-platform-parity UI inside a game engine" is achievable. It is also a case study in the cost of that achievement: the IMGUI→UI Toolkit Editor migration is in its **eighth year** and incomplete; UGUI is still officially recommended for runtime; USS diverges from CSS in ways that surprise web developers; the Accessibility Module was a 2023.2 addition that only the runtime-UI side participates in.

**Honest assessment.** Unity is proprietary, closed-source, and commercial. This corpus borrows **lessons**, not code or designs — and treats Unity Technologies as an institutional actor whose 2023 Runtime Fee announcement (later cancelled September 2024) demonstrated that proprietary substrate carries governance risk that an open-source Bevy + Buiy stack does not. The borrowable signal: UXML/USS/VisualElement is **the** worked example of declarative-web-flavored authoring shipping inside a production game engine, and the Editor migration story is a 5+-year cautionary tale that Buiy's BSN authoring story must learn from.

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Vendor | Unity Technologies (San Francisco; NYSE: U; merged with IronSource 2022) |
| License | Proprietary (Personal free + Pro/Enterprise paid tiers) |
| Engine release | **Unity 6.3 LTS** (December 2025) is the current LTS; Unity 6 family launched 2024-10-17 |
| Predecessor LTS | Unity 2022.3 LTS, Unity 2021.3 LTS, Unity 2019.4 LTS (historical) |
| **UGUI** package | `com.unity.ugui` v2.6 (built-in since Unity 2019.2); originally shipped Unity 4.6 (Dec 2014) |
| **UI Toolkit** package | `com.unity.ui` (built-in since Unity 2021 LTS); UIElements debut Unity 2017→2019.1 |
| **IMGUI** | Legacy `OnGUI` immediate-mode system; Editor-only by official recommendation as of Unity 6 |
| Runtime UI Toolkit | Available Unity 2021 LTS+; Editor adoption began Unity 2019.1; World-space UI Toolkit Unity 6.2 |
| Editor migration | IMGUI → UI Toolkit, **ongoing since Unity 2019.1** (~7 years); many built-in windows still IMGUI shells as of Unity 6.3 |
| Layout engine (UI Toolkit) | **Yoga** (Facebook's open-source flexbox engine) — implements a subset of CSS Flexbox |
| Layout engine (UGUI) | LayoutGroup + LayoutElement + ContentSizeFitter (proprietary, GameObject-driven) |
| Text engine | **TextMeshPro** (acquired from Stephan Bouchard 2017; integrated 2017+; SDF-based); UI Toolkit `<TextElement>` uses TextCore (TMP-derived) |
| Accessibility | `UnityEngine.Accessibility` module since Unity 2023.2 Tech Stream; TalkBack/VoiceOver mobile launch 2023.2; Windows Narrator + macOS VoiceOver expansion later; **NO ARIA model**; no WCAG conformance claim |
| Third-party a11y | `mikrima/UnityAccessibilityPlugin` (UAP) — community plugin for Windows/Android/iOS/Mac/WebGL screen reader bridge |
| Scripting | C# (Mono / IL2CPP), reflection-rich, edit-time + runtime |
| Notable widget kits | DoozyUI (UGUI-based, commercial Asset Store); App UI package (`com.unity.dt.app-ui`); MoreMountains Feel; many third-party kits — none with ARIA |
| Runtime Fee saga | Announced 2023-09-13, partial walk-back 2023-09-22, **fully cancelled 2024-09-12**; Marc Whitten (Unity Create CPO) resigned 2024-06-01 |

## Two-stack distinction (important for the rest of this corpus)

| Axis | UGUI (legacy) | UI Toolkit (modern) |
|---|---|---|
| Authoring | GameObjects in Scene + Canvas | UXML asset (HTML-like) + USS stylesheet (CSS-like) + C# `VisualElement` API |
| Hierarchy | `GameObject` tree under `Canvas` | `VisualElement` tree (no GameObjects) |
| Layout | `RectTransform` + LayoutGroup + ContentSizeFitter | Yoga (subset of Flexbox) |
| Styling | `Image`, `Text`, `Outline`, etc. per-GameObject components | USS classes + selectors + transitions |
| Rendering | `CanvasRenderer` per element | Single batched mesh per panel (texture-less when possible) |
| Editor use | No (UGUI is runtime-only) | Yes — Editor UI Toolkit migration since Unity 2019.1 |
| World-space UI | Yes (since 2014 via `Canvas` worldspace render mode) | Yes — added Unity 6.2 (December 2025) |
| Animation | Animator/Animation timeline keyframes | USS transitions only (no keyframes per Unity's compare table) |
| Production status | Mature, **officially recommended for runtime** | Production-ready 2021 LTS+; under active development; "not yet stable and mature" per community |

Throughout this corpus, **"UGUI"** and **"UI Toolkit"** are kept distinct. Where Unity's own documentation uses "Unity UI" ambiguously, this corpus disambiguates.

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, two-stack distinction, key facts, ToC. |
| [`lessons.md`](lessons.md) | **The consult-this-when-designing decision file.** Validates / Avoid / Borrow. |
| [`glossary.md`](glossary.md) | Unity-UI-specific terms used across the corpus. |
| [`ugui-architecture.md`](ugui-architecture.md) | UGUI: Canvas + RectTransform + CanvasRenderer + EventSystem; LayoutGroup family; render pipeline; pros/cons in 2026. |
| [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md) | UI Toolkit: UXML/USS/VisualElement; runtime + Editor unified; data binding; comparison to web DOM/CSS. |
| [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md) | UXML vs HTML mapping; USS vs CSS coverage (flex, grid, transitions, animations, variables, calc(), media queries, container queries, anchor positioning — each verified). |
| [`text-rendering.md`](text-rendering.md) | TextMeshPro lineage, TextCore in UI Toolkit, BiDi, complex shaping, IME, CJK/Arabic/Indic. |
| [`accessibility.md`](accessibility.md) | Unity Accessibility module (since 2023.2), AccessibilityHierarchy, AT integration (TalkBack/VoiceOver/Narrator), WCAG posture, gaps. |
| [`editor-ui-migration.md`](editor-ui-migration.md) | The 7+-year Unity Editor migration from IMGUI to UI Toolkit; what worked, what didn't; lessons for Buiy's BSN authoring story. |
| [`history.md`](history.md) | UGUI introduction (Unity 4.6, Dec 2014); UIElements debut (2017→2019); rename to UI Toolkit (2020.1 beta); runtime 2021 LTS; current state. |
| [`distribution-and-governance.md`](distribution-and-governance.md) | Unity Technologies, proprietary license, Runtime Fee 2023 + reversal 2024, Unity 6 LTS cadence, technical leadership. |
| [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) | Production usage, third-party widget kits (DoozyUI, App UI, UAP, Feel); compared to Unreal UMG/Slate, Godot Control, Bevy UI, Buiy. |
| [`critiques-and-open-problems.md`](critiques-and-open-problems.md) | UGUI perf at 1000+ elements, UI Toolkit runtime maturity, Editor migration pain, USS-vs-CSS divergence, accessibility lag, proprietary lock-in, Runtime Fee aftermath; open problems on WCAG/container queries/anchor positioning/APG widgets/BiDi editing. |

## How to use this corpus

1. **If you are designing a Buiy feature** that has a web-platform analog — start at [`lessons.md`](lessons.md), then dive into [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md) and [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md). UI Toolkit is the worked example for what shipping web-flavored UI inside a game engine looks like at scale.
2. **If you are designing BSN authoring** (foundation §2.4) — start at [`editor-ui-migration.md`](editor-ui-migration.md). The IMGUI→UI Toolkit migration is a 7-year cautionary tale Buiy must learn from.
3. **If you are auditing accessibility** — start at [`accessibility.md`](accessibility.md), then [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Accessibility lag." Unity's a11y module is recent (2023.2), partial, and has no ARIA model.
4. **If you are scoping the USS / stylesheet open question** (foundation README §5) — [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md) catalogs exactly which CSS features Unity ships, which it deviates on, and which it does not have. Pair with [`/home/user/buiy/docs/prior-art/bevy-flair/`](../bevy-flair/).
5. **If you are evaluating proprietary-substrate governance risk** — [`distribution-and-governance.md`](distribution-and-governance.md) covers the Runtime Fee 2023 saga.

## Framing disclosure

This corpus is written from a **Buiy-parallel-stack + web-platform-parity + WCAG 2.2 AA + BSN-friendly + open-source-substrate** stance. Unity is the closest existing-art for "web-platform parity in a game engine" — the corpus has an incentive to over-validate UI Toolkit's design bets. Where the corpus says "validates Buiy's choice," pressure-test whether the validation is genuine or motivated. Conversely, Unity is proprietary and a competing platform; the corpus has an incentive to over-emphasise Unity's pain points. Both pressures are present; the reader is the auditor.

A secondary disclosure: this corpus is built from public documentation, blog posts, community discussion, and verified release notes. It does not include access to Unity's internal source (Unity is closed-source) or roadmap. Where dates or claims are not independently verifiable, the corpus marks them as such.

## Cross-document inconsistencies surfaced

- **UI Toolkit rename year.** Sources disagree slightly: Unity 2020.1 beta announcement (March 2020) per Unity Discussions thread; pre-amble said "Unity 2021." The 2020.1 beta blog post is the load-bearing source. [`history.md`](history.md) reports 2020.
- **UGUI as "retained mode" vs UI Toolkit as "retained mode."** Both are technically retained — UGUI retains via GameObjects, UI Toolkit retains via VisualElement tree. Some community sources frame UI Toolkit as the "retained-mode" system in contrast to UGUI's "GameObject mode," which conflates two different design axes. [`ugui-architecture.md`](ugui-architecture.md) and [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md) disambiguate.
- **UI Toolkit availability for runtime.** Unity 2021 LTS is the universally-cited runtime stability mark; some sources cite Unity 2022 LTS as the "production-ready" mark. Both are right for different definitions: 2021 LTS for "you can ship a screen-overlay UI with it," 2022 LTS for "the package API stopped shifting." [`history.md`](history.md) reports both.
- **UI Toolkit accessibility.** Unity's Accessibility module (2023.2) covers semantic announcements + AssistiveSupport on mobile; whether it covers UI Toolkit specifically vs UGUI specifically is muddy in Unity docs — the API is documented as GUI-system-agnostic. [`accessibility.md`](accessibility.md) reports this verbatim.

## Sources

- Unity UI Toolkits overview — https://docs.unity3d.com/Manual/UIToolkits.html
- Unity UI Systems comparison — https://docs.unity3d.com/Manual/UI-system-compare.html
- UGUI manual (com.unity.ugui) — https://docs.unity3d.com/Packages/com.unity.ugui@latest/
- USS supported properties — https://docs.unity3d.com/Manual/UIE-USS-SupportedProperties.html
- USS overview — https://docs.unity3d.com/Manual/UIE-USS.html
- UXML VisualElement reference — https://docs.unity3d.com/Manual/UIE-uxml-element-VisualElement.html
- UQuery — https://docs.unity3d.com/Manual/UQuery.html
- TextMesh Pro Joins Unity (Unity Blog, 2017-03-20) — https://blog.unity.com/games/textmesh-pro-joins-unity
- Mobile screen reader support (Unity Blog) — https://unity.com/blog/engine-platform/mobile-screen-reader-support-in-unity
- Unity Accessibility manual — https://docs.unity3d.com/6000.3/Documentation/Manual/accessibility.html
- Unity 6 release announcement — https://unity.com/blog/unity-6-features-announcement
- Unity is Canceling the Runtime Fee (2024-09-12) — https://unity.com/blog/unity-is-canceling-the-runtime-fee
- Marc Whitten resignation — https://mobilegamer.biz/marc-whitten-quits-unity/
- UIElements renaming announcement (Unity Discussions) — https://discussions.unity.com/t/renaming-uielements-to-ui-toolkit/782459
- Migrate from UGUI to UI Toolkit — https://docs.unity3d.com/6000.3/Documentation/Manual/UIE-Transitioning-From-UGUI.html
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_ui prior-art (cross-link) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
