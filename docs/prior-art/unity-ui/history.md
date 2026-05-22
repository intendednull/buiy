**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — chronological history of UGUI, UIElements, UI Toolkit; key release marks

# History

Two UI stacks, one institutional timeline. UGUI debuted in 2014 and is twelve years old; UIElements debuted in 2017, renamed to UI Toolkit in 2020, became runtime-stable in 2021 LTS, gained world-space UI in 2025. Both still ship and are both maintained as of Unity 6.3 LTS (December 2025).

## Pre-UGUI (≤2014)

- **OnGUI / IMGUI** — Unity's original UI: an immediate-mode `OnGUI()` callback in every MonoBehaviour. Code-driven, no Scene-view authoring, "the ancient OnGUI system" per community accounts. Still ships and is Editor-only-recommended in Unity 6.3.
- **NGUI** — Tasharen Entertainment's Asset Store UGUI predecessor (created by Michael Lyashenko). Widely used 2011-2014; Unity hired Lyashenko in late 2013, and many NGUI patterns informed UGUI.

## 2014 — UGUI introduction

- **Unity 4.6 (December 2014)** — UGUI ships as the new built-in UI. GameObject-based; Canvas + RectTransform + CanvasRenderer + EventSystem; LayoutGroup family; Selectable hierarchy (Button/Toggle/Slider/Scrollbar/Dropdown/InputField). Replaces OnGUI for runtime UI.
- **Unity 5 era (2015-2016)** — UGUI matures; Asset Store widget ecosystem (NGUI patterns ported; DoozyUI begins).
- **2017 acquisition of TextMesh Pro** (March 2017) — Stephan Bouchard's TMP joins Unity; planned integration into Unity 2017+.

## 2017-2019 — UIElements debut (Editor first)

- **2017** — UIElements announced; roadmap published early 2018.
- **Unity 2019.1 (April 2019)** — UIElements ships as a Unity-built-in for **Editor** UI authoring (UXML/USS files for custom Editor windows and Inspectors). Runtime use not yet officially supported.
- **Unity 2019.2-2019.3** — UIElements API maturation; first Editor windows ship in UIElements.
- **Unity 2019.4 LTS (June 2020)** — UIElements stable for Editor; long-term-support release. (Many older "stuck on 2019.4 LTS" projects still use this.)

## 2020-2021 — Rename and runtime

- **March 2020 (Unity 2020.1 beta)** — UIElements **renamed to UI Toolkit** (announcement on Unity Discussions). The umbrella term covers `VisualElement` C# API + UXML + USS + UI Builder + UI Toolkit Debugger + Event Debugger.
- **Unity 2020.x** — UI Toolkit Editor adoption broadens; runtime UI Toolkit experimental.
- **Unity 2021 LTS (June 2021)** — UI Toolkit becomes **built-in** (not separable package); **runtime UI Toolkit shipped as production-supported** for screen-overlay UI. UI Builder matures. This is the canonical "you can ship UI Toolkit at runtime" mark.
- **2021.x** — TextMesh Pro continues integration; TextCore (TMP-derived) used inside UI Toolkit's text element.

## 2022-2023 — Production maturity

- **Unity 2022 LTS (June 2022)** — UI Toolkit declared production-ready for runtime; surface stable; biweekly fix support for two years. New Editor windows author predominantly in UI Toolkit.
- **Unity 2023.1 Tech Stream** — UI Toolkit data binding APIs (runtime binding via `dataSource`; SerializedObject binding stabilises).
- **Unity 2023.2 Tech Stream (October 2023)** — **Accessibility module** ships: `UnityEngine.Accessibility`, mobile screen reader support (TalkBack/VoiceOver), `AccessibilityHierarchy` Editor viewer. This is the first first-party Unity accessibility primitive in the engine's history.
- **September 2023** — Unity announces the **Runtime Fee policy**. Massive developer backlash. CEO John Riccitiello departs October 2023. (See [`distribution-and-governance.md`](distribution-and-governance.md).)
- **Unity 2023.3 Tech Stream** — Screen reader support extended to Windows (Narrator) and macOS (VoiceOver).

## 2024 — Unity 6 family

- **June 2024** — Marc Whitten (Unity Create CPO who fronted the Runtime Fee response) resigns.
- **September 12, 2024** — Unity **cancels the Runtime Fee** entirely. Revenue model returns to seat-based subscriptions (with price increases).
- **October 17, 2024 — Unity 6 (a.k.a. Unity 6.0)** releases. Re-branded LTS line — Unity 6 is the new LTS family, distinct from the 2019/2020/2021/2022/2023 naming. UI Toolkit improvements; Vector Graphics package fully integrated (SVG import for UI without separate package).

## 2025-2026 — Unity 6 line

- **2025 — Unity 6.2** — World Space UI Toolkit shipped. UI Toolkit gains world-space rendering after eleven years of UGUI having it. Mesh LOD support, performance improvements.
- **December 2025 — Unity 6.3 LTS** — Current LTS. UI Toolkit continued improvements; UGUI still officially recommended for runtime per Unity's own UI system comparison page; many Editor windows still IMGUI-shelled.

## Net institutional posture (2026)

- **UGUI** — Mature, twelve years old, officially recommended for runtime. Asset-Store widget ecosystem (DoozyUI, MoreMountains, etc.) primarily targets UGUI. Performance well-understood. Accessibility is opt-in via 2023.2+ module or third-party UAP.
- **UI Toolkit** — Production-ready since 2021 LTS for runtime, since 2019.1 for Editor. Still "in active development" per Unity's own framing. Editor migration ongoing 7+ years. World-space UI just added. Strongest existing-art for "web-platform-inspired UI in a game engine."
- **IMGUI** — Legacy, Editor-only-recommended; not going away.

## Key dates table

| Date | Event |
|---|---|
| December 2014 | UGUI ships (Unity 4.6) |
| 2017 | UIElements announced; TextMesh Pro acquisition |
| April 2019 | UIElements ships in Unity 2019.1 for Editor |
| June 2020 | Unity 2019.4 LTS — UIElements stable for Editor |
| March 2020 | UIElements renamed to **UI Toolkit** (Unity 2020.1 beta) |
| June 2021 | Unity 2021 LTS — UI Toolkit built-in; **runtime UI Toolkit ships** |
| June 2022 | Unity 2022 LTS — UI Toolkit production-ready |
| October 2023 | Unity 2023.2 — Accessibility module ships |
| September 13, 2023 | Runtime Fee announced |
| October 2023 | CEO John Riccitiello departs |
| September 12, 2024 | **Runtime Fee cancelled** |
| June 2024 | Marc Whitten resigns |
| October 17, 2024 | **Unity 6** releases (new LTS family) |
| 2025 | Unity 6.2 — World Space UI Toolkit |
| December 2025 | **Unity 6.3 LTS** (current) |

## Implications for Buiy

1. **Twelve-year lifecycles are normal.** UGUI 2014 → still recommended 2026. Buiy is committing to a comparable lifetime; foundation §2.9's "rolling latest-stable Bevy" + per-minor migration approach must be sustainable for that horizon.
2. **Rename mid-flight is risky but survivable.** UIElements → UI Toolkit (2020) is a precedent for naming a system at debut and renaming once. Buiy committed to "Buiy" early; this is fine.
3. **Production-readiness is a multi-year graduation.** UI Toolkit: 2019 ships → 2021 LTS runtime stable → 2022 LTS production-ready → 2024 actually-used-by-default-recommended (still not the case). Three+ years from "ships" to "the recommended default." Buiy's planning should expect the same arc.
4. **Governance shocks happen.** The 2023 Runtime Fee saga demonstrated that proprietary substrate carries governance risk. Foundation §2.9 commits to open-source Bevy + open-source Buiy; this is the precedent.
5. **A11y delays compound.** Unity 2014 → 2023.2 = nine years of a11y-free UGUI shipping. Buiy commits a11y day-one (foundation §2.6); the cost of "later" is in this history.

## Sources

- Unity 4.6 release notes (community archives) — Unity 4 release archive
- Renaming UIElements to UI Toolkit — https://discussions.unity.com/t/renaming-uielements-to-ui-toolkit/782459
- UI Toolkit roadmap (Unity Roadmap) — https://unity.com/roadmap/263-ui-toolkit-available-for-runtime-2021-lts-
- Unity 2022 LTS overview — https://unity.com/releases/2022-lts
- Mobile screen reader support — https://unity.com/blog/engine-platform/mobile-screen-reader-support-in-unity
- Unity 6 announcement — https://unity.com/blog/unity-6-features-announcement
- Unity 6 download — https://unity.com/releases/unity-6
- Unity is Canceling the Runtime Fee — https://unity.com/blog/unity-is-canceling-the-runtime-fee
- TextMesh Pro joins Unity — https://blog.unity.com/games/textmesh-pro-joins-unity
- Marc Whitten quits Unity — https://mobilegamer.biz/marc-whitten-quits-unity/
- State of UI in Unity (One Wheel Studio) — https://onewheelstudio.com/blog/2022/10/28/state-of-ui-tool-kit
