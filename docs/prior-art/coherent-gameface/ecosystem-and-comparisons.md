**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — verified AAA users, peer comparisons (NoesisGUI, RmlUi, Scaleform, Buiy)

# Ecosystem and comparisons

## Verified AAA customer list

Coherent Labs's product page displays an "18+ titles" customer reel. The list below is **what Coherent's public marketing names** — independent per-game verification (looking up SDK fingerprints, dev-blog credits, or studio confirmations) is a separate exercise and is **not done in this corpus**. Treat these as **named-by-Coherent**, not as independently-confirmed-by-third-party.

### Named on coherent-labs.com/products/coherent-gameface/

- **Borderlands 4** (Gearbox Software, 2024 — see also the Gearbox testimonial Coherent quotes)
- **Marvel's Spider-Man 2** (Insomniac Games, 2023)
- **Civilization 7** (Firaxis Games, 2025)
- **Alan Wake 2** (Remedy Entertainment, 2023)
- **Minecraft** (Mojang Studios / Microsoft — exact title or platform variant not specified)
- **PUBG** (PlayerUnknown's Battlegrounds — historically Coherent GT, predates Gameface rebrand; per the PocketGamer.biz interview)
- **World of Tanks** (Wargaming)
- **Sea of Thieves** (Rare / Microsoft)
- **Cricket 24** (per the noesisgui sister doc; appears in Coherent reel)
- **iRacing** (sim racing platform)
- **Hytale** (Hypixel Studios — also named on NoesisGUI's customer page, suggesting the Coherent attribution may be specific to a subsystem or earlier prototype; verify)

Additional named in surrounding marketing / interviews:

- **Gearbox Software** as a customer (explicit testimonial quoted on coherent-labs.com)
- **Bluehole Studio Inc.** (PUBG developer, CTO Soo Min Park is quoted)
- **Crayta** (per the Unit 2 Games / Crayta Developer Area tutorial which uses Coherent Gameface)

### Pre-amble verification

The pre-amble suggested the following AAA customer claims to verify:

| Pre-amble claim | Status |
|---|---|
| Cyberpunk 2077 | **Could not verify** — not on Coherent's public customer list; CD Projekt Red uses an in-house engine; no Coherent attribution in public sources. |
| The Witcher 3 | **Could not verify** — same CDPR engine context; no Coherent attribution in public sources. |
| "AAA studios — verify" generally | Confirmed. Multiple AAA studios are named (Insomniac, Firaxis, Gearbox, Remedy, Mojang, Rare, Wargaming). |

The "Cyberpunk 2077 / Witcher 3" claim should be **dropped** from the corpus unless a primary source surfaces. CDPR shipped Cyberpunk 2077 in 2020 with their REDengine; historically their UI was Flash-based (Scaleform). Coherent attribution would be notable if true; absence from Coherent's own marketing customer reel is informative.

## Comparison to peers

### Coherent Gameface vs NoesisGUI (XAML)

See [`noesisgui/`](../noesisgui/) for the full NoesisGUI corpus.

| Axis | Coherent Gameface | NoesisGUI |
|---|---|---|
| Authoring | HTML5 + CSS3 + JavaScript (React, Preact, jQuery, Tailwind) | XAML (Microsoft-derived markup) + C# / C++ codebehind |
| Engine | Cohtml + Renoir (in-house) | Noesis-native runtime (in-house) |
| Engines supported | UE 4/5, Unity 2020.2+, custom C++ | UE 5.7, Unity 2020.2+, custom C++ |
| Pricing | Quote-based, opaque | Tiered with public prices (Indie €195, Pro €9K, Premium €18K — March 2024 restructure) |
| Indie tier | Special pricing on request | Public €195 tier (rev < €100K) |
| Customers | Borderlands 4, Spider-Man 2, Civ 7, Alan Wake 2, Minecraft, Sea of Thieves, PUBG | Baldur's Gate 3, Hellblade 2, Age of Wonders 4, TopSpin 2K25, Hytale, Cricket 24, iRacing |
| Authoring tool | Standard web tooling (VSCode, Chrome devtools, Webpack); Adobe Animate for Prysm | Blend for Visual Studio (Windows-only); Noesis Studio (multi-year beta) |
| LLM-trained markup | HTML/CSS/JS has the largest training corpus | XAML predates the LLM corpora; reportedly less effective for AI-assisted authoring |
| Accessibility | TextToSpeech + JS-side ARIA plugins (no OS-AT bridge) | None advertised |
| Team size | ~50–100 (Sofia, Bulgaria) | ~2–9 publicly reported (Madrid, Spain) |
| Founded | 2012 | 2013 |

Headline comparison: **Coherent takes the HTML5 path; Noesis takes the XAML path. Both ship at AAA scale.** Coherent's surface is broader (HTML/CSS/JS is bigger than XAML/C#) and the tooling ecosystem is broader; Noesis's surface is narrower but tightly integrated with MVVM patterns. Coherent's team is significantly larger.

### Coherent Gameface vs RmlUi (open-source HTML/CSS)

See [`rmlui/`](../rmlui/) for the full RmlUi corpus.

| Axis | Coherent Gameface | RmlUi |
|---|---|---|
| License | Proprietary commercial | MIT |
| Authoring | HTML5 + CSS3 + JavaScript | RML (HTML-flavored) + RCSS (CSS-flavored, subset + extensions) |
| Spec fidelity | Standards-compliant HTML5 + CSS3 (with documented divergences — e.g., grid as custom element) | RCSS subset (CSS 2.1 + Flexbox + transitions/animations/transforms; no Grid, no container queries, no anchor positioning) |
| JS engine | V8 + alternate VM | None — bindings via `data-model` + C++ glue |
| Engine integration | UE 4/5, Unity, custom C++ — Coherent ships plugins | Five embedder interfaces (Render / System / File / FontEngine / TextInputHandler) — embedders build their own bindings |
| Accessibility | In-process TTS + JS ARIA plugins | None |
| Team size | ~50–100 | 1 primary maintainer (`mikke89`) |
| Cadence | Quarterly minors + LTS | ~1 release per year recent |
| Customers (verified) | Borderlands 4, Spider-Man 2, Civ 7, Alan Wake 2, Sea of Thieves, ... | Nightdive KEX engine, Cfx.re Alchemist, Unvanquished, ROSE Online, TruckersMP |

Headline comparison: **RmlUi is the open-source counterpart to Coherent on the HTML-flavored axis.** Coherent is what RmlUi would look like with commercial pricing and ~50× the engineering headcount. The substrate decision is structurally similar (custom in-house HTML/CSS engine, embedder-interface integration); the difference is scope (Coherent ships full standards-compliant HTML5+CSS3; RmlUi ships a curated subset) and the commercial-vs-open-source split.

### Coherent Gameface vs Scaleform (Flash predecessor)

| Axis | Coherent Gameface | Scaleform |
|---|---|---|
| Status | Active 2018–present | Discontinued by Autodesk 2017 |
| Authoring | HTML5 + CSS3 + JS | ActionScript 3 (Flash / AIR ecosystem) |
| Tooling | Standard web tooling + Adobe Animate (Prysm) | Adobe Flash Professional / Animate |
| Substrate | Cohtml + Renoir (in-house) | Scaleform GFx (in-house Flash runtime) |
| Authored by | Coherent Labs | Scaleform Corp → Autodesk (acquired 2011) |
| Lineage | Successor de-facto in the post-Flash era | Built on Adobe Flash's market ubiquity (~2003 onward) |

Headline comparison: **Coherent positioned Gameface as the Scaleform successor when Autodesk EOL'd Scaleform in 2017.** AAA studios needed a vector-UI-with-animator-workflow successor; Coherent Prysm (Adobe Animate plugin) was deliberately scoped for that handoff. Coherent's marketing and the PocketGamer.biz interview both reference this competitive positioning explicitly.

### Coherent Gameface vs Buiy

| Axis | Coherent Gameface | Buiy |
|---|---|---|
| License | Proprietary commercial, quote-based | MIT-OR-Apache-2.0 dual permissive |
| Authoring | HTML5 + CSS3 + JS | BSN (`.bsn` Bevy asset format), ECS spawn idioms |
| Substrate | In-house HTML engine (Cohtml) + GPU renderer (Renoir) | Taffy (layout) + cosmic-text (text) + AccessKit (a11y) + wgpu via Bevy's render graph |
| Engines supported | UE 4/5, Unity 2020.2+, custom C++ | **Bevy-only** — Bevy 0.18+ (latest stable, foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md#non-goals)) |
| JS engine | V8 + alternate VM | **None** — logic in Rust ECS systems / observers |
| Accessibility | TTS + JS ARIA plugins, in-process | **AccessKit-first, OS-AT-bridged**, WCAG 2.2 AA floor |
| Theme model | CSS custom properties | Token-based theming (semantic tokens + OS preferences) |
| Layout | Flexbox full, Grid via custom element (not native CSS Grid) | Taffy Flexbox + Grid (subgrid, container queries, anchor positioning all in roadmap) |
| Maintainer headcount | ~50–100 (Coherent Labs commercial team) | Buiy maintainer cluster + Bevy Foundation governance + upstream Taffy/cosmic-text/AccessKit/wgpu maintainer communities |
| Distribution | Direct sales | crates.io |
| Customers | Borderlands 4, Spider-Man 2, Civ 7, Alan Wake 2, PUBG, ... | None yet (pre-1.0) |
| Web platform parity | Standards-compliant HTML5 (with documented gaps) | Web-platform UI feature catalog as the master list (foundation [`README.md` goal 1](../../specs/2026-05-07-buiy-foundation/README.md#buiys-goals-the-product)) |
| Mobile / console | Yes (all major consoles, mobile via Hummingbird/Prysm lineage) | Bevy's platform support determines this (currently Windows/macOS/Linux first-class; Android/iOS in-progress; web via wgpu-WASM) |

Headline comparison: **Buiy and Coherent address overlapping problem space with structurally different solutions.** Coherent says "ship the web platform inside the engine via an in-house parser+VM+renderer." Buiy says "ship a feature-parallel UI library against curated Rust substrates, no parser / no VM." Coherent serves AAA on Unity + Unreal + custom-C++ at commercial pricing. Buiy serves Bevy at zero license cost. The two products do not directly compete in 2026 because the engine-target audiences don't overlap (no AAA Bevy titles yet). They will compete *conceptually* when Bevy reaches AAA-shipping readiness.

## Where Coherent slots in the broader middleware landscape

| Middleware | Substrate | License | AAA usage | Status |
|---|---|---|---|---|
| Scaleform | Flash / AS3 | Commercial (Autodesk) | Heavy 2007–2017 | EOL 2017 |
| Coherent UI | WebKit | Commercial | 2012–2017 | EOL 2017 |
| Coherent GT | WebKit | Commercial | 2014–2018 (PUBG era) | Subsumed by Gameface |
| Coherent Gameface | Cohtml + Renoir (in-house HTML) | Commercial | 2018–present, AAA | **Active** |
| Coherent Prysm | Cohtml + Renoir (Adobe Animate plugin) | Commercial | 2018–present, Scaleform-successor | **Active** |
| NoesisGUI | XAML / WPF dialect | Commercial tiered | 2013–present, AAA | **Active** |
| RmlUi (post-libRocket) | RML + RCSS, custom engine | MIT | 2018–present, indie + AA | **Active** |
| CEF (Chromium Embedded) | Chromium | BSD | Common in launcher/wiki/overlay, rare in core game UI | Active |
| Unreal UMG (engine-native) | Slate-wrapped | UE-EULA + royalty | Default for UE titles | Active |
| Unity UI Toolkit + UGUI (engine-native) | Yoga + Unity-native | Commercial | Default for Unity titles | Active |
| Godot Control (engine-native) | Godot-native | MIT | Indie / AA in Godot | Active |
| **Buiy** | Bevy + Taffy + cosmic-text + AccessKit | MIT/Apache | None yet | Pre-1.0 |

The middleware market is **two-axis-segmented**:

1. **Engine-native vs cross-engine middleware.** Engine-native = UMG, UI Toolkit, Godot Control, **Buiy** (Bevy-only). Cross-engine = Coherent, NoesisGUI, RmlUi, CEF.
2. **Web-platform-flavored vs not-web-platform.** Web-platform = Coherent, RmlUi, CEF, **Buiy** (web-feature parity goal). Not-web = NoesisGUI (XAML), Scaleform (Flash), Unreal Slate, UGUI.

Buiy uniquely occupies the **engine-native + web-platform-flavored + open-source** corner. The closest precedent on the web-platform-flavored axis is Coherent (proprietary, cross-engine) or RmlUi (MIT, cross-engine). The closest precedent on the engine-native axis is Godot Control (MIT, Godot-only, web-platform parity NOT a goal) or UI Toolkit (proprietary, Unity-only, web-platform parity *is* a goal).

## Implications for Buiy

- **The "web-platform-flavored in-engine UI" axis has commercial proof at AAA scale.** Coherent is the canonical existence proof. Buiy validates the same axis without paying the proprietary-license cost — by reusing Bevy's substrate rather than building an HTML engine.
- **The competitive landscape says Coherent is the most direct conceptual predecessor.** When framing Buiy's positioning to potential users, "Coherent Gameface, but Bevy-native and MIT-licensed, with AccessKit-first a11y" is a reasonable elevator pitch — provided the audience knows Coherent.
- **Avoid claiming Buiy "replaces Coherent."** Coherent serves AAA studios on Unity / Unreal / custom-C++; Buiy serves Bevy projects. The audiences don't overlap today. If Bevy reaches AAA shipping readiness, the comparison becomes direct — but that's a years-out scenario.
- **The customer list teaches a verification discipline.** Coherent's marketing names ~18 titles but doesn't enumerate which subsystem of each game uses Gameface (a HUD overlay? the full UI? a launcher? a level-loading screen?). When Buiy starts naming customers, **specify the scope of each adoption** so the claim is auditable.

## Sources

- Coherent Gameface product page customer reel — https://coherent-labs.com/products/coherent-gameface/
- Coherent Labs About page customer references — https://coherent-labs.com/about-us/
- PocketGamer.biz interview (PUBG, Bluehole, Scaleform succession) — https://www.pocketgamer.biz/interview/67816/can-coherent-labs-rise-as-the-new-standard-for-game-ui/
- SaaSHub Chromely vs Coherent GT comparison — https://www.saashub.com/compare-chromely-vs-coherent-gt
- SaaSHub NoesisGUI vs Coherent GT comparison — https://www.saashub.com/compare-noesisgui-vs-coherent-gt
- SteamDB tech detection (Coherent_Gameface_OR_Prysm) — https://steamdb.info/tech/SDK/Coherent_Gameface_OR_Prysm/
- Crayta Developer Area (Coherent Gameface tutorial) — https://tutorials.crayta.com/introduction-to-widgets-building-a-game-ui/
- Sibling prior-art: [`../noesisgui/`](../noesisgui/), [`../rmlui/`](../rmlui/), [`../unreal-slate-umg/`](../unreal-slate-umg/), [`../unity-ui/`](../unity-ui/), [`../godot-control/`](../godot-control/)
