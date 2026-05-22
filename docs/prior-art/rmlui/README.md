**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — open-source HTML/CSS-flavored UI library for C++ games and applications; closest open-source precedent for "HTML+CSS in a game engine"

# RmlUi

RmlUi (the **R**ocket **M**arkup **L**anguage UI library) is a C++ UI library that parses an HTML-flavored markup language (**RML**) and a CSS-flavored stylesheet language (**RCSS**) into a layout tree, then submits vertices + indices + draw commands back to an embedder-supplied render interface. It is a **fork** of the dormant `libRocket` project (2008–2014, CodePoint Ltd / Shift Technology Ltd), restarted by Michael Ragazzon (`mikke89`) in 2018; the first RmlUi-branded release was **2.0** on 2019-10-13. It is the **strongest open-source precedent** for the "HTML+CSS in a game engine" pattern Buiy occupies — proprietary cousins like NoesisGUI and Coherent Gameface fill the AAA niche, but RmlUi is the open-source data point for the same design space with ~15 years of cumulative shipping history (libRocket era + RmlUi era).

**Honest assessment.** RmlUi is small (single primary maintainer, ~10 named lifetime contributors of any volume, modest commercial-game adoption) and long-lived. It does not chase modern web-spec parity — the project README states: *"We do not aim to be fully compliant with CSS or HTML, in particular when it conflicts with lightness and performance."* RCSS is anchored to **CSS 2.1 + selective CSS 3 borrowings** (flexbox, transforms, animations, transitions, media queries) — no CSS Grid, no subgrid, no container queries, no anchor positioning, no `clip-path`, no `backdrop-filter` (filters were added in 6.0 but only blur/drop-shadow/brightness etc., not arbitrary `clip-path` shapes). Accessibility is **not a feature** — the README, docs, and CHANGELOG make no mention of screen-reader integration, AccessKit, ARIA roles, or focus management beyond spatial controller navigation. Text shaping defaults to FreeType-only; HarfBuzz is a **sample**, not a built-in, and BiDi / complex-script support is correspondingly thin. For Buiy, RmlUi is a feasibility study (HTML+CSS in a game engine *does* ship as open source) and a cautionary tale (single-maintainer bus factor, custom-DSL drift from web spec, accessibility-as-afterthought).

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Project | RmlUi (Rocket Markup Language UI) |
| Repo | https://github.com/mikke89/RmlUi |
| Docs | https://mikke89.github.io/RmlUiDoc/ |
| License | MIT |
| Language | C++ (C++14+ since 3.x; 2.0 was the last C++11-compatible release) |
| Latest stable | **6.2** (2026-01-11) |
| Previous releases | 6.1 (2025-04-20), 6.0 (2024-08-26), 5.1 (2023-04-07), 5.0 (2022-12-11) |
| First RmlUi release | **2.0** (2019-10-13) — earlier tags `release-1.2.0` (2014-08-25), `release-1.3.0.0`, `release-1.2.1` inherit from libRocket |
| Primary maintainer | **Michael Ragazzon** (GitHub: `mikke89`) — bus factor 1 |
| Predecessor | **libRocket** (2008–2014, CodePoint Ltd + Shift Technology Ltd, MIT, dormant) |
| Authoring | **RML** (HTML/XHTML 1.0–flavored markup) + **RCSS** (CSS 2.1 + selective CSS 3) |
| Layout | Own block + inline layout engine; **Flexbox** added 5.0 (2022-12-11); **NO CSS Grid**, no subgrid, no container queries |
| Text | Own pipeline on **FreeType**; HarfBuzz is a *sample plugin* (not built-in); BiDi / RTL not first-class |
| Render | Embedder implements `RenderInterface`; library emits vertices + indices + draw commands |
| Reference backends | OpenGL 2, OpenGL 3, Vulkan, DirectX 12, SDL renderer, SDL GPU (the latter added in 6.x) |
| Platform integrations | GLFW, SDL, SFML, Win32, X11 |
| Platforms shipped | Windows, Linux, macOS, Android, iOS, Nintendo Switch |
| Accessibility | **None** — no screen-reader integration, no ARIA, no AccessKit; spatial controller navigation only |
| Notable users | The Thing: Remastered (Nightdive), Killing Time: Resurrected (Nightdive), ROSE Online, Unvanquished, Alchemist (Cfx.re / Rockstar Games), TruckersMP, WOTInspector |
| Governance | Single-maintainer + community PRs; no foundation, no corporate steward, no RFC process |

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, ToC, framing disclosure. |
| [`lessons.md`](lessons.md) | **The consult-this-when-designing decision file.** Validates / avoid / borrow. |
| [`glossary.md`](glossary.md) | System-specific terms (RML, RCSS, embedder, interfaces, etc.). |
| [`architecture.md`](architecture.md) | RML parser + RCSS parser + layout engine + embedder interface (Render / System / File / FontEngine / TextInputHandler). |
| [`rml-rcss-coverage.md`](rml-rcss-coverage.md) | RML vs HTML mapping; RCSS vs CSS coverage; what is supported, what is altered, what is excluded. |
| [`layout-and-styling.md`](layout-and-styling.md) | Block / inline / flexbox; animations + transitions; decorators; theming approach. |
| [`text-and-input.md`](text-and-input.md) | FreeType-based text; HarfBuzz sample; BiDi / complex-script status; IME on Win32; input handling. |
| [`accessibility.md`](accessibility.md) | RmlUi's accessibility absence; contrast with Buiy's AccessKit-first commitment. |
| [`history.md`](history.md) | libRocket genesis (2008+); decline (~2014); RmlUi fork (2018+); release timeline. |
| [`distribution-and-governance.md`](distribution-and-governance.md) | MIT license; single-maintainer model; bus factor; build / package ecosystem; commercial usage terms. |
| [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) | Shipping games; vs NoesisGUI, Coherent Gameface, Unity UGUI / UI Toolkit, bevy_ui, Buiy. |
| [`critiques-and-open-problems.md`](critiques-and-open-problems.md) | Bus factor, C++ integration friction, CSS coverage gaps, accessibility absence, modern-web features missing. |

## How to use this corpus

1. **If you are designing a Buiy feature** with an open-source HTML+CSS-in-game-engine analogue, start at [`lessons.md`](lessons.md). Find the relevant `Validates` / `Avoid` / `Borrow` entry.
2. **If you are evaluating Buiy's RCSS-flavored-stylesheet open question** (foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5, *"CSS-flavored stylesheet: Never, or as a future layer above tokens?"*), start at [`rml-rcss-coverage.md`](rml-rcss-coverage.md) — RmlUi is the most directly-applicable empirical study of *what subset of CSS is practical to ship in a game UI*.
3. **If you are designing the embedder boundary** for a render / system / asset interface in any Buiy 3D-anchored or render-to-texture scenario, start at [`architecture.md`](architecture.md) § "Embedder interfaces."
4. **If you are auditing the cost of deferring accessibility**, start at [`accessibility.md`](accessibility.md) — RmlUi is the longest-running data point for *what an HTML/CSS-flavored UI library that never built an a11y story looks like 15+ years in*.
5. **If you are pressure-testing single-maintainer governance vs Bevy's foundation**, start at [`distribution-and-governance.md`](distribution-and-governance.md) and follow into [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Bus factor."

## Framing disclosure

These docs are written from a **Buiy = parallel-to-bevy_ui + web-platform-parity (web-spec semantics, not custom DSL) + WCAG 2.2 AA + AccessKit-first + Rust-native + Bevy-only** stance. Most "Implications for Buiy" lines frame RmlUi's choices through that lens. A RmlUi feature gap becomes a Buiy commitment ("RmlUi never built a11y, Buiy starts with it"); a RmlUi shipping decision becomes a Buiy contrast ("RmlUi diverges from CSS spec for lightness; Buiy chooses spec fidelity"). RmlUi is **not** an integration target for Buiy — it is C++, not Rust; embedder-pattern, not ECS-native; the lessons are architectural, not migration. Future readers auditing whether *web-spec-fidelity + AccessKit-first* is itself the right primitive should weigh the corpus accordingly. It is a learn-from-RmlUi-into-Buiy artifact, not a neutral catalog.

## Cross-document cross-links

- **bevy_ui** → [`../bevy-ui/`](../bevy-ui/) — Bevy's own UI crate; the system Buiy is parallel to.
- **NoesisGUI** → [`../noesisgui/`](../noesisgui/) — proprietary AAA-grade XAML cousin; the closed-source data point in the same design space.
- **bevy_flair** → [`../bevy-flair/`](../bevy-flair/) and **belly** → [`../belly/`](../belly/) — the existing-art for CSS-on-Bevy specifically (read together with RmlUi for the foundation §5 stylesheet open question).
- **Slint** → (pending) and **Makepad** → [`../makepad/`](../makepad/) — DSL-above-runtime precedents; RmlUi sits adjacent (markup-not-DSL, but same "custom syntax above a custom runtime" pattern).

## Sources

- RmlUi GitHub repository — https://github.com/mikke89/RmlUi
- RmlUi documentation — https://mikke89.github.io/RmlUiDoc/
- RmlUi changelog — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- RmlUi releases — https://github.com/mikke89/RmlUi/releases
- libRocket GitHub repository — https://github.com/libRocket/libRocket
- RmlUi 2.0 release (first post-libRocket-fork release) — https://github.com/mikke89/RmlUi/releases/tag/2.0
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- NoesisGUI prior-art (sibling) — [`../noesisgui/README.md`](../noesisgui/README.md)
- bevy_ui prior-art (sibling) — [`../bevy-ui/README.md`](../bevy-ui/README.md)
