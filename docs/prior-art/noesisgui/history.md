**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — Noesis Technologies founding through 2026 release cadence

# History

NoesisGUI's history is the story of a tiny Spanish software company (Noesis Technologies S.L.) building a XAML-reimplementation from scratch over 13+ years, landing high-profile AAA games as customers, and converging on a stable mature commercial product. The release cadence is steady; the company is profitable but small; the technical roadmap continues to expand (Rive, Lottie, variable fonts, complex script shaping, world-space UI) while keeping XAML as the load-bearing authoring layer.

## Timeline

| Date | Event |
|---|---|
| ~2009 | Founders begin work on a XAML-on-C++ engine; Noesis Technologies S.L. incorporated in Madrid (per Crunchbase). |
| 2013 | NoesisGUI publicly launched and marketed (per the homepage's "since 2013" framing). |
| ~2014 | First public Unity Asset Store listing (NoesisGUI 2.x). |
| 2015 | Unity Asset Store presence active; community posts about Noesis appear on Unity forums (Tasharen.com, Unity Discussions). |
| 2018 | NoesisGUI 2.x mature; D3D11 / D3D12 / Metal / Vulkan / OpenGL render backends. |
| 2020-05 | NoesisGUI 3.0.0 released ([Tutorials repo](https://github.com/Noesis/Tutorials) latest release marker). |
| 2020-12 | Unity minimum version becomes 2020.2 (still the minimum for current 3.2.x). |
| ~2022 | NoesisGUI 3.1 — additional XAML features, MeshGeometry pre-tessellated paths. |
| 2022-12 | NoesisGUI 3.2.0 — complex script text shaping, Language Server Protocol, Rive support, stereo VR rendering, world-space UI. |
| 2023-08 | Baldur's Gate 3 ships (Larian) — uses NoesisGUI 3.1.6 per third-party docs. |
| 2024-03 | Licensing structure changed: Indie license price dropped from €395 → €195, project budget threshold raised from €100K → €250K, prices for other tiers made public. Console support included in all licenses. |
| ~2024 | Noesis Studio (next-generation visual editor) announced as "Coming 2024" in beta. |
| 2024-05 | Hellblade 2 (Ninja Theory / Xbox Game Studios) ships using NoesisGUI. |
| 2025-04 | TopSpin 2K25 (Hangar 13) ships using NoesisGUI. |
| 2025-06 | NoesisGUI 3.2.8 — UE 5.6, Unity 6.1, rendering perf improvements. |
| 2025-10 | NoesisGUI 3.2.9 — Nintendo Switch 2 support, VirtualizingWrapPanel. |
| 2025-10 | NoesisGUI 3.2.10 — bugfix release. |
| 2026-02 | NoesisGUI 3.2.11 — UE 5.7, UMG ViewModel support, template hot-reload with instance state preservation. |
| 2026-03 | NoesisGUI 3.2.12 — Visual Studio 2026, WPF-compliant TimeSpan formatting. |
| 2026-04-27 | NoesisGUI 3.2.13 (current latest) — Unity 6.3 / 6.4, Xcode 26, DirectComposition on Win32. |

## Founders & key people

Public information about Noesis Technologies' team is sparse — the company has historically chosen low public-relations visibility, emphasising customers (testimonials from Larian, Ninja Theory, etc.) over founder profiles. Known names:

- **Jesús de Santos García** — CTO; appears on Crunchbase and Gust profiles.
- **Sergio Fernández Vanaclocha** — co-founder; appears on Gust profile.

Company size figures conflict across sources (CB Insights / RocketReach show 2-9 employees, with both 2-employee and 9-employee figures appearing in different snapshots). The most defensible characterisation: **fewer than 10 people**, possibly fewer than 5, headquartered in Madrid with possible additional presence in Avilés (Asturias). This is *small* even by indie-game-tools standards. The team punches massively above its weight.

## Release cadence: ~4 patch releases per year on the 3.2 line

The 3.2.x patch releases since 2024 have averaged roughly one every 2-3 months. Each release cycle is dominated by:

1. Bumping support for the latest Unity / Unreal minor.
2. Bumping support for the latest Xcode / VS / platform SDK.
3. Bugfix backlog.
4. Small new features (Rive control, BackgroundEffect, VirtualizingWrapPanel, single-pass stereo, etc.).
5. Console support (Switch 2 in 3.2.9; PS5 NGGC, etc.).

The cadence is **a stable mature product on a long support tail**, not a high-velocity new-platform startup. Customers like Larian (who ship one game per ~5 years) are well-served by this cadence; smaller indies who want bleeding-edge features less so.

## What 3.2.0 changed (December 2022)

The foundational 3.2 release was a substantial expansion of NoesisGUI's scope:

- **Complex script text shaping.** Before 3.2, text rendering was limited to simple-script (Latin, basic CJK). 3.2 added shaping for complex scripts (Arabic, Devanagari, Thai, Khmer). This pulled NoesisGUI closer to web-platform text parity.
- **Language Server Protocol implementation.** Editors implementing LSP get XAML intelligent completion, refactoring, navigation. The [LanguageServer repo](https://github.com/Noesis/LanguageServer) is MIT-licensed open source even though the core runtime is proprietary.
- **Rive integration.** `RiveControl` embeds Rive animation files (a more game-native vector animation format than Lottie).
- **Single Pass Stereo VR rendering.** UI renders simultaneously to both eye buffers in VR.
- **World Space UI.** Render Noesis UI directly into 3D world space (without render-to-texture).
- **BackgroundEffect.** Apply blur to elements behind a panel (CSS `backdrop-filter` equivalent).

After 3.2.0 the engine entered its current long-term-support shape; 3.2.x patches have not changed the API surface, only platform / engine compatibility and bugfixes.

## Customer-acquisition timeline

The customer trajectory is approximately:

- **2013-2017** — Early adopters; indie / small-studio focus.
- **2018-2020** — Mid-tier studios (Crystal Dynamics, Iceflake, Hangar 13) start using Noesis for production projects.
- **2020-2023** — Major AAA wins: Larian (Baldur's Gate 3), Ninja Theory (Hellblade 2), Triumph (Age of Wonders 4).
- **2023-2026** — Established AAA presence; new customers continue (Hytale, TopSpin 2K25, iRacing, Cricket 24, etc.); industrial / simulation sector growth (CAE, Kratos Defense, Canon, Beyeonics Surgical).

The customer list also covers non-game industrial use — flight simulation (CAE), surgical robotics (Beyeonics), CAD (Maxon), engineering (FLUOR, ARUP), audio (PK Sound, Tazman Audio), Linden Lab (Second Life-era successor work). Noesis's reach into industrial sectors is a structural reason for the company's commercial viability — it diversifies beyond games' lumpy revenue.

## What history doesn't tell us

- **Revenue.** Noesis is private, has not disclosed revenue figures publicly. Crunchbase shows no funding rounds (the company is bootstrapped / customer-funded). The commercial model implies revenue is per-project licenses + maintenance / priority-support fees, lumpy by nature.
- **Headcount over time.** Different snapshots give different numbers (2-9 reported, both extremes have appeared). Either the company has stayed small consistently, or it has fluctuated; the public record doesn't distinguish.
- **Roadmap beyond 3.2.x.** No public 3.3 or 4.0 roadmap is visible. The release notes pattern suggests Noesis is in a "mature stable product, maintain forever" mode rather than a "major refactor coming" mode.

## Implication for Buiy

The Noesis history is a useful study in **how a tiny team ships a complete UI library at AAA scale**: 10+ years of focused work, slow steady cadence, mature stable API, customer testimonials over founder profiles, diversified into industrial sectors for revenue stability. Buiy's open-source model means a different shape (community contributions are the labor model, not paid employees), but the *time investment* required to deliver comprehensive UI coverage is on the order of a decade. Buiy's foundation spec sets its scope deliberately ambitious (web-platform parity + AccessKit + Bevy); the realistic implementation horizon is years, not months, and Noesis's history is the data point.

## Sources

- NoesisGUI changelog — https://www.noesisengine.com/docs/Gui.Core.Changelog.html
- Crunchbase Noesis Technologies — https://www.crunchbase.com/organization/noesis-technologies
- Gust company profile — https://gust.com/companies/noesis_technologies
- CB Insights — https://www.cbinsights.com/company/noesis-technologies-sl
- StartupXplore — https://startupxplore.com/en/startups/noesis-technologies
- Noesis GitHub organization — https://github.com/Noesis
- Updated licensing forum post — https://www.noesisengine.com/forums/viewtopic.php?t=3260
- GameDev.net Baldur's Gate 3 announcement — https://www.gamedev.net/news/noesis-technologies-rolls-out-30-ui-tool-used-in-baldurs-gate-iii-r1347/
- Tutorials repo releases — https://github.com/Noesis/Tutorials
