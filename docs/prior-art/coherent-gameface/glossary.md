**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — glossary of products, technologies, and concepts

# Glossary

## Products

**Coherent UI** — the original Coherent Labs product (announced 2012-09-24). A WebKit-based embedded browser for game UI. Native-to-JS binding, full HTML5/CSS3/JS, GPU-accelerated. **Subscriptions ended 2017-12-05.** Open-sourced (mobile variant) under MIT at `CoherentLabs/CoherentUIMobileOpenSource`. Superseded by Coherent GT and Hummingbird.

**Coherent GT** — second-generation Coherent product (~2014). Also WebKit-based, but heavily optimized for game-UI use (less general-purpose-browser). The product line that shipped in **PUBG (PlayerUnknown's Battlegrounds)**. Superseded by Coherent Gameface (the Cohtml+Renoir rebrand).

**Hummingbird** — Coherent's first **in-house HTML engine** (~2016). Mobile-first, designed for 60-fps UI on phones / embedded. Replaced the WebKit dependency. Rebranded as **Coherent Gameface 1.0** on 2018-12-07.

**Coherent Gameface** — current code-first developer-facing UI middleware (released 2018-12-07). Runs HTML5 + CSS3 + JS via Cohtml + Renoir. **Latest version 3.0.1 / 3.0.1.1** (current LTS + Feature tracks). The product Buiy is most directly compared to.

**Coherent Prysm** — current artist-first UI middleware (released 2018-12-07, same day as Gameface 1.0). Same Cohtml + Renoir runtime; different authoring tool (Adobe Animate plugin instead of standard web tooling). Coherent's deliberate Scaleform-succession product for animator-driven UI pipelines.

## Technologies

**Cohtml** — Coherent's **in-house HTML5 + CSS3 + JavaScript engine**. C++ runtime. Parses HTML, resolves CSS, runs JS (V8 where licensing permits, alternate VM elsewhere), owns the DOM, dispatches events. Not based on Blink, WebKit, Gecko, or Servo. The product surface of "Gameface" + "Prysm" sits on top of Cohtml.

**Renoir** — Coherent's **in-house GPU rendering library**. C++ runtime. Consumes Cohtml's rendering command stream and translates to the target graphics API (DX11, DX12, Vulkan, Metal, OpenGL/GLES, console-native). Multi-threaded command generation, data-oriented design. Coherent claims **15-70% rendering improvement** vs the legacy backend.

**Coherent Editor** — Adobe Animate plugin distributed by Coherent Labs for Prysm authoring. Lets artists create animated UI in Animate; exports to the Cohtml runtime.

**GameUIComponents** — Coherent's open-source (MIT-licensed) Web Components library at `github.com/CoherentLabs/GameUIComponents`. Custom HTML elements (`<gameface-grid>`, `<gameface-virtual-list>`, etc.) usable both inside Cohtml and in Chrome for development.

**coherent-guic-cli** — Coherent's open-source CLI tool for scaffolding GameUIComponents custom elements.

## Concepts

**Cohtml view** — one fullscreen game UI screen (or one in-world UI overlay). The HTML5 page that Cohtml parses and renders. Equivalent to a top-level browser tab in browser-terminology.

**Data binding (Cohtml)** — Coherent's declarative C++ ↔ JS model-binding system. Bind a C++ data model to a DOM subtree; mutations on either side propagate to the other. Sits below React-style frameworks.

**`FileSystemReader`** — Cohtml's abstraction for asset loading. Embedders supply a `FileSystemReader` that maps HTML/CSS/JS/image asset references to the host engine's asset pipeline.

**`RenoirGPUMemoryInfo`** — Cohtml/Renoir API exposing per-UI-subsystem GPU memory usage (textures, buffers, atlases) for profiler attribution.

**Coherent Inspector** — Coherent's bundled Chrome-DevTools-equivalent inspector for running Cohtml views. Inspect DOM, view computed CSS, modify styles live, see layout boxes.

**CEF (Chromium Embedded Framework)** — the open-source library that lets an application embed Chromium as a browser. The primary alternative to Cohtml for HTML-driven game UI. Coherent's marketing positions Cohtml as the lighter, in-process, game-tuned alternative to CEF.

**Scaleform** — the Adobe Flash-based game UI middleware (Autodesk, 2003–2017). Discontinued by Autodesk in 2017. Coherent positioned Gameface + Prysm as the de-facto Scaleform successor for AAA studios with animator-driven UI workflows.

## Coherent Labs corporate

**Coherent Labs** — the Bulgarian game-UI middleware company founded 2012. Sofia, Bulgaria. ~50–100 staff as of 2026. Privately-held; **acquisition / parent-company information could not be verified** as of 2026-05-22.

**Coherent Labs AD** — the registered Bulgarian corporate entity (per Dun & Bradstreet). "AD" denotes "акционерно дружество" — joint-stock company under Bulgarian commercial law.

**Coherent, Inc. / Coherent Corp.** — **a different company.** The laser/photonics manufacturer acquired by II-VI Incorporated in July 2022 (renamed Coherent Corp). Frequently confused with Coherent Labs because of the shared "Coherent" name; **do not conflate.**

## Founders

**George Petrov** (also rendered as **Georgi Petrov**) — Coherent Labs co-founder, CEO (current). One of the four original founders; remains in named senior leadership 2012–present.

**Dimitar Trendafilov** — Coherent Labs co-founder, CTO (current). One of the four original founders; remains in named senior leadership 2012–present.

**Stoyan Nikolov** (handle `stoyannk`) — Coherent Labs co-founder, ex-Chief Software Architect (March 2012 – May 2019). Departed ~2019. Public speaker at meetingcpp; visible on LinkedIn / X. Subsequently joined Google.

**Nick Vasilev** (also rendered as **Nikola Vasilev**) — Coherent Labs co-founder, R&D Director (current). One of the four original founders; remains in named senior leadership.

## Other current leadership

**Aleksandra Ivanov** — Customer Success Director (per current About page).

**Annie Atanasova** — Director of Operations and Finance (per current About page).

## Comparative naming (Buiy-side)

For readers crosswalking from this corpus into Buiy's foundation spec:

| Coherent / Cohtml term | Buiy equivalent |
|---|---|
| HTML markup | BSN (`.bsn` asset format) |
| CSS stylesheet | Buiy decomposed components + theme tokens (no CSS parser in v1; foundation [`README.md` § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) open question for future CSS-flavored stylesheet) |
| Cohtml runtime | `buiy_core` + Taffy + cosmic-text + Bevy ECS |
| Renoir runtime | `buiy_core` render pipeline + wgpu via Bevy's render graph |
| Cohtml view | A Bevy entity hierarchy rooted on a Buiy root component, attached to a window or render target |
| `FileSystemReader` | Bevy `AssetServer` + asset reflection |
| Data binding (Cohtml) | Bevy observers + change detection (signals layer is foundation [`README.md` § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) open question) |
| `CohtmlARIA*` plugins | AccessKit + decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` components (foundation [`architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)) |
| Coherent Inspector | Buiy devtools sub-spec (foundation [`README.md` § 4 buiy-devtools-design](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) |
| `<gameface-grid>` custom element | `Display::Grid` on a Buiy component, mapped to Taffy's Grid layout (plan `2026-05-09-buiy-layout-grid.md`) |
| Cohtml hot-reload | BSN hot-reload via Bevy's asset system (foundation [`README.md` § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) open question) |

## Sources

- Coherent Labs glossary references — https://coherent-labs.com/about-us/
- Renoir Graphics Library introduction — https://coherent-labs.com/posts/introducing-renoir-graphics-library/
- Cohtml documentation index — https://docs.coherent-labs.com/cpp-gameface/
- The Recursive company profile (founders) — https://therecursive.com/company/coherent-labs/
- Stoyan Nikolov LinkedIn — https://bg.linkedin.com/in/stoyannikolov
- Coherent UI Mobile open-source repo — https://github.com/CoherentLabs/CoherentUIMobileOpenSource
- GameUIComponents repo — https://github.com/CoherentLabs/GameUIComponents
- Coherent Corp. (the LASER company — DIFFERENT COMPANY) — https://en.wikipedia.org/wiki/Coherent_Corp.
