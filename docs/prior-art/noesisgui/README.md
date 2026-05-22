**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — commercial XAML-based UI middleware for game engines

# NoesisGUI

NoesisGUI is the closest existing-art to Buiy on one specific axis: **a comprehensive, cross-engine UI library that has shipped in AAA games**. It is XAML-based (a Microsoft-derived markup language) rather than ECS-native; it is closed-source commercial software, not MIT/Apache; and it integrates with Unity, Unreal, and custom C++ engines rather than tying itself to one engine. But on the surface-area question — "what does it take to deliver a complete UI library that AAA studios will adopt?" — NoesisGUI's answer is the most concrete data point available.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Vendor | Noesis Technologies S.L. | https://www.noesisengine.com/ |
| Founded | 2013 (company); 2009 per Crunchbase (founder activity) | Crunchbase, gust.com |
| Headquarters | Madrid, Spain | Crunchbase, gust.com |
| Team size | ~2-9 people (publicly reported figures vary) | Crunchbase, CB Insights |
| Latest version | 3.2.13 (released 2026-04-27) | NoesisGUI Changelog |
| License | Proprietary commercial; royalty-free perpetual; tiered | https://www.noesisengine.com/licensing.php |
| Indie tier | €195 per project (gross revenue < €100K, project budget < €250K) | Licensing forum, March 2024 update |
| Pro tier | €9,000 first platform + €3,600/extra platform (project budget < €1.5M) | Licensing page |
| Premium tier | €18,000 first platform + €7,200/extra (budget €1.5M–€12M) | Licensing page |
| Authoring | XAML (Microsoft's declarative UI markup), Blend-compatible | Docs index |
| Runtime | Native C++ library; bindings for C#, Unity, Unreal | Docs index |
| Rendering | GPU-accelerated vector graphics; tessellation pipeline; render-device abstraction | Rendering tutorial |
| Engines | Unity (2020.2+), Unreal Engine (UE 5.7 as of 3.2.11), custom C++ | Tutorials |
| Platforms | Windows, macOS, Linux, iOS, Android, UWP, Xbox One/Series, PS4/PS5, Switch, Switch 2, WebGL | Features page |
| Accessibility | None advertised; no screen-reader integration | Features page (omission) |

## Honest assessment

NoesisGUI is the strongest existing-art for "proprietary commercial cross-engine UI middleware shipped in AAA games." Named production users include **Baldur's Gate 3** (Larian, confirmed v3.1.6), **Hellblade 2** (Ninja Theory), **Age of Wonders 4** (Triumph), **TopSpin 2K25** (Hangar 13), **Hytale** (Hypixel), and around 100 other studios spanning AAA gaming, simulation, and industrial sectors. It is XAML's main game-industry beachhead.

That said, the corpus does not lift NoesisGUI as a model for Buiy to follow. Buiy is open-source, ECS-native, Bevy-only, AccessKit-first; NoesisGUI is proprietary, XAML-native, multi-engine, and has no accessibility story. The fit is a contrast — what does it take to ship a complete UI library at AAA scale (NoesisGUI's answer: a decade of focused engineering by a small team plus a permissive XAML lineage that gave them a working spec on day one); and what does Buiy choose to do differently (open ecosystem, Bevy-only, web-platform-parity including a11y).

The pre-amble claims to verify were **partly wrong**: Counter-Strike 2 and Apex Legends are **not** verified NoesisGUI customers in Noesis's public customer list; Microsoft Flight Simulator is **not** named on the customer page either. Microsoft *is* a named customer but no specific Microsoft game is identified. Bungie / Destiny are **not** on the public customer list. See [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) for the verified-only list.

## Table of contents

1. [`architecture.md`](architecture.md) — runtime layout, framework vs integration API, render-device abstraction, threading.
2. [`xaml-paradigm.md`](xaml-paradigm.md) — XAML as authoring layer; dependency properties; data binding; MVVM; comparison to BSN/HTML.
3. [`rendering-and-performance.md`](rendering-and-performance.md) — GPU tessellation, vector graphics, sub-millisecond claim.
4. [`engine-integration.md`](engine-integration.md) — Unity, Unreal, and custom-engine bindings.
5. [`history.md`](history.md) — Noesis Technologies founding through 2026 release cadence.
6. [`distribution-and-governance.md`](distribution-and-governance.md) — licensing tiers, indie threshold, commercial viability.
7. [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) — verified customer list; comparison vs UGUI / UI Toolkit / Slate / UMG / Coherent Gameface / Buiy.
8. [`critiques-and-open-problems.md`](critiques-and-open-problems.md) — proprietary lock-in, XAML age, a11y gap, per-engine binding overhead.
9. [`lessons.md`](lessons.md) — **the decision file** — validates / avoid / borrow for Buiy.
10. [`glossary.md`](glossary.md) — XAML, dependency property, attached property, data binding, MVVM, RenderDevice, etc.

## How to use

**Framing disclosure.** These docs are written from a Bevy-only / open-source / ECS-native / AccessKit-first Buiy stance — most "Implications for Buiy" sub-sections frame NoesisGUI's choices through that lens. Future readers auditing whether *those* Buiy commitments are themselves the right primitives should weigh the corpus accordingly: it's a learn-from-NoesisGUI-into-Buiy artifact, not a neutral catalog. NoesisGUI is the canonical "what proprietary commercial cross-engine UI middleware looks like when it's done well" data point; Buiy is taking a different shape on purpose.

Cross-reference with [`bevy-ui/`](../bevy-ui/), [`unreal-slate-umg/`](../unreal-slate-umg/), [`gpui/`](../gpui/) for the substrate / parallel-stack / custom-renderer comparisons.

## Sources

- NoesisGUI homepage — https://www.noesisengine.com/
- NoesisGUI technology page — https://www.noesisengine.com/noesisgui/
- NoesisGUI customers — https://www.noesisengine.com/customers.php
- NoesisGUI licensing — https://www.noesisengine.com/licensing.php
- NoesisGUI docs index (v3.2.13) — https://www.noesisengine.com/docs/Gui.Core.Index.html
- NoesisGUI 3.2 changelog — https://www.noesisengine.com/docs/Gui.Core.Changelog.html
- Noesis Technologies on Crunchbase — https://www.crunchbase.com/organization/noesis-technologies
- Noesis GitHub organisation — https://github.com/Noesis
- Updated licensing forum post — https://www.noesisengine.com/forums/viewtopic.php?t=3260
- GameFromScratch hands-on review — https://gamefromscratch.com/noesisgui-hands-on-with-the-game-user-interface-framework/
- Larian / Baldur's Gate 3 confirmation — https://www.gamedev.net/news/noesis-technologies-rolls-out-30-ui-tool-used-in-baldurs-gate-iii-r1347/
