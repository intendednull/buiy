**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — accessibility: Unity Accessibility Module (since 2023.2), AccessibilityHierarchy, AT integration, WCAG posture, gaps

# Accessibility

Unity's first-party accessibility story arrived **late**. The `UnityEngine.Accessibility` module debuted in **Unity 2023.2 Tech Stream** (~October 2023) with mobile screen reader support (Android TalkBack, iOS VoiceOver); Windows Narrator + macOS VoiceOver expanded in Unity 2023.3 Tech Stream and follow-ups; the API was added to LTS in Unity 6.0+. For roughly **nine years** between UGUI's 2014 release and the 2023 module, Unity shipped no first-party accessibility — projects either built their own bridge or used `mikrima/UnityAccessibilityPlugin` (UAP), a community Asset Store package. This is the closest existing-art case study for "what 'add accessibility late' looks like at scale."

## The Unity Accessibility Module

- **Package:** `UnityEngine.Accessibility` — built-in module, enabled by default in new projects (Unity 2023.2+).
- **Core types:** `AccessibilityHierarchy` (per-game accessibility tree), `AccessibilityNode` (per-element node with role/label/value/state), `AssistiveSupport` (singleton that brokers OS AT communication).
- **Editor tool:** Accessibility Hierarchy Viewer — a Play-mode window showing the live accessibility tree, similar to a browser's accessibility devtools.
- **GUI-system-agnostic.** Unity's explicit design: the API works for UGUI, UI Toolkit, IMGUI, or custom rendering. The hierarchy is built by application code; there is no automatic role inference from UGUI `Button` → `role=button`.

## AT (assistive technology) bridges

| Platform | AT | Status |
|---|---|---|
| Android | TalkBack | ✅ supported since Unity 2023.2 |
| iOS | VoiceOver | ✅ supported since Unity 2023.2 |
| macOS | VoiceOver | ✅ supported (later 2023 / 2024 timeframe) |
| Windows | Narrator | ✅ supported (later 2023 / 2024 timeframe) |
| Linux | Orca (AT-SPI) | ❌ not supported |
| Web (WebGL) | ARIA | ❌ no bridge |
| Switch / PS / Xbox | Console-platform AT | ❌ not in scope; console-platform OS APIs vary |

The bridge is **OS-native announcement-and-focus**, not a full ARIA-style declarative role/state/property model. Application code constructs the `AccessibilityHierarchy` and calls `AssistiveSupport.NotificationDispatcher.SendAnnouncement(...)` for live announcements; the platform AT reads accordingly.

## How accessibility integrates with UI Toolkit and UGUI

- **UI Toolkit:** UXML elements have a `tabindex`-equivalent (`focusable` boolean + `tabIndex` int) but **no ARIA role attribute**. There is no `aria-label` analog in UXML. Accessibility is bridged via C# code that builds the `AccessibilityHierarchy` mirroring the VisualElement tree.
- **UGUI:** Same story — `Selectable` carries no role/label/description. Accessibility code reflects MonoBehaviours into `AccessibilityNode`s.
- **In both cases**, the application owns the bridge code. Some elements (e.g. `<TextField>`) get default treatment by the module's heuristics; most do not.

## Pre-2023 third-party: UI Accessibility Plugin (UAP)

- `mikrima/UnityAccessibilityPlugin` — Asset Store package by Metalpop Games.
- Targets Windows, Android, iOS, Mac, WebGL.
- Provides screen-reader bridge + a high-contrast mode + reading order configuration.
- Was the de facto accessibility solution from ~2017 through Unity 2023.2.
- Many shipping accessible Unity titles still use UAP rather than the Accessibility module because UAP predates and has more developer-friendly UGUI integration.

## WCAG conformance posture

- **Unity does not claim WCAG conformance** for engine output. The Accessibility module documentation does not enumerate WCAG SCs.
- **Game-side conformance is the developer's responsibility.** Unity provides primitives (announcements, focus, semantic labels); the game decides which SCs to meet.
- The Accessibility module addresses some Perceivable / Operable SCs (screen-reader bridge ≈ SC 4.1.2 Name, Role, Value; assistive support ≈ SC 4.1.3 Status Messages) but the engine ships no automated WCAG check, no contrast linter, no focus-order validator.
- **No reduced-motion / prefers-contrast / forced-colors integration in the module.** Reduced motion is application-managed (project reads OS preference itself); USS has no `@media (prefers-reduced-motion)` query.

## What Unity does well (accessibility, narrow)

- **GUI-system-agnostic API.** Not coupling to UGUI vs UI Toolkit was the right call — the same accessibility code works for both, and for custom UI.
- **Mobile-first AT integration.** TalkBack + VoiceOver bridges shipped first; this addresses the largest accessibility-impacted user base (mobile screen reader users).
- **Hierarchy Viewer in Editor.** A live tree-inspection tool ships with the module — devtools-grade.

## What Unity does badly (accessibility)

- **Nine-year gap.** UGUI 2014 → Accessibility module 2023.2. Unity titles released in that window are accessible only via UAP, custom code, or not at all.
- **No ARIA model.** There is no `role="button"` / `aria-label` / `aria-describedby` / `aria-expanded` declarative surface in UXML or UGUI. Every game re-implements role mapping. WAI-ARIA APG patterns have no engine support.
- **No automatic role inference.** A UI Toolkit `<Button>` does **not** automatically appear as `role=button` in the accessibility hierarchy — application code must build the hierarchy entry.
- **No live-region model.** Live regions exist via `SendAnnouncement(...)` API call only; there is no equivalent of `aria-live="polite"` / `aria-live="assertive"` on an element.
- **No focus model integration with USS.** `:focus-visible` does not exist in USS; styling focused elements differently for keyboard-only users requires application-side trickery.
- **WebGL has no AT bridge.** Unity-built web apps cannot expose accessibility to web ATs (NVDA, JAWS, VoiceOver-Safari) — there is no `aria-*` emission to the surrounding DOM.
- **Linux/Orca unsupported.** AT-SPI bridge does not exist.
- **No first-party WCAG conformance claim.** Engine output is not warranted to meet any conformance level.
- **Editor accessibility is open.** The Unity Editor itself (built on UI Toolkit) does not pass an accessibility audit; Unity Discussions hosts a long-running thread on Editor accessibility for blind developers.

## Implications for Buiy

This is the **single most important "Avoid" case** in this corpus and one of the strongest validations of Buiy's foundation design bets.

1. **AccessKit-first is correct.** Buiy's foundation §2.6 makes AccessKit the source of truth from day one; Unity's 9-year gap demonstrates the cost of deferring. *Adding accessibility late means most existing UI is forever inaccessible.* Buiy commits to never being in that position.
2. **ARIA role-first authoring is the right primitive.** UXML's no-ARIA-attributes choice forced every Unity game to re-implement role mapping. Buiy's `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` decomposed components (foundation §2.6) ship at the same authoring layer as everything else.
3. **Automatic role inference for built-in widgets.** Buiy widgets (`Button`, `Checkbox`, `Slider`, `Tabs`, etc., per the APG catalog in foundation media-and-widgets.md) ship their `A11yRole` as a required-component default. A Unity-style "you must build the hierarchy yourself" choice is the wrong default.
4. **Live-region model as first-class component.** Buiy commits to `aria-live` semantics via `LiveRegion` / `Announcer` (foundation accessibility.md §3.11). Unity's announcement-API-only approach is insufficient for productivity apps.
5. **OS-preference integration is non-negotiable.** Buiy's `UserPreferences` resource (foundation §2.5) reads `prefers-reduced-motion`, `prefers-contrast`, `forced-colors`, `prefers-color-scheme` and binds theme variants automatically. Unity's USS has *zero* of these; this is a clean lead.
6. **Web target (WASM) needs the AccessKit web adapter.** Buiy's WASM target story (foundation README §5 open question) is exactly the gap Unity-WebGL has. AccessKit's web adapter is the answer; Unity has no equivalent.
7. **Editor / authoring-tool accessibility matters.** Unity's Editor a11y is an open problem; Buiy's BSN authoring story (foundation §2.4) should commit to authoring-tool accessibility from the start, not as an afterthought. *(The BSN authoring tool is one of Buiy's open spec areas — see `buiy-bsn-integration-design`.)*

The borrowable signal from Unity is *narrow*: the **Accessibility Hierarchy Viewer** is a good devtools shape (live tree, role/label/state per node, Play-mode-only) that Buiy's `buiy_devtools` should match.

## Sources

- Unity Accessibility manual — https://docs.unity3d.com/6000.3/Documentation/Manual/accessibility.html
- Mobile screen reader support (Unity Blog) — https://unity.com/blog/engine-platform/mobile-screen-reader-support-in-unity
- Unity accessibility expanded (Can I Play That, Sep 2025) — https://caniplaythat.com/2025/09/05/unity-expands-native-screen-reader-support-and-accessibility-api/
- a11y-public-sample (Unity Technologies) — https://github.com/Unity-Technologies/a11y-public-sample
- UI Accessibility Plugin (UAP) — https://github.com/mikrima/UnityAccessibilityPlugin
- WWDC22 accessibility in Unity games — https://developer.apple.com/videos/play/wwdc2022/10151/
- Accessibility in the Unity Editor (Discussions thread) — https://discussions.unity.com/t/accessibility-in-the-unity-editor/947198
- UI Toolkit screen reader (Discussions thread) — https://discussions.unity.com/t/ui-toolkit-screen-reader/246795
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation architecture §2.6 — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy-a11y prior art — [`../bevy-a11y/`](../bevy-a11y/)
- accesskit prior art — [`../accesskit/`](../accesskit/)
