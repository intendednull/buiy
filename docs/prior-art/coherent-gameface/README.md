**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — proprietary commercial "full HTML5/CSS/JS in a game engine" middleware

# Coherent Gameface

Coherent Gameface is the **closest existing-art for the "full HTML5/CSS/JS in a game engine" thesis** that Buiy partly inherits when it commits to web-platform-parity (foundation [`README.md` § 1 goal 1](../../specs/2026-05-07-buiy-foundation/README.md#buiys-goals-the-product)). Where NoesisGUI bets on XAML and RmlUi bets on a CSS-flavored subset, Coherent bets on **standards-compliant HTML5 + CSS3 + JavaScript** as the game-UI authoring layer — with a **proprietary in-house web engine** (`Cohtml` + `Renoir`, succeeding the earlier "Hummingbird" engine) rather than embedded Chromium/CEF.

It is **proprietary, quote-based-pricing commercial middleware** sold by Coherent Labs (Sofia, Bulgaria, ~50 staff as of the most recent public interview). It ships in AAA: the public customer page names **Borderlands 4, Marvel's Spider-Man 2, Civilization 7, Alan Wake 2, Minecraft, World of Tanks, Sea of Thieves, PUBG**, and ~18 other titles. Treat these as named-by-Coherent — independent confirmation per game is a separate exercise.

Buiy is the **open-source MIT/Apache analog targeting Bevy specifically**, scoped to the same web-platform-parity ambition but without Coherent's proprietary HTML-engine maintenance burden (because Buiy renders BSN-shaped Bevy components directly through Taffy + cosmic-text + Bevy's render graph, not through a parser-driven DOM tree).

## Key facts

| Fact | Value | Source |
|---|---|---|
| Vendor | Coherent Labs (a.k.a. Coherent Labs AD) | https://coherent-labs.com/ |
| Founded | 2012 | About page, copyright header |
| Founders (verified) | George (Georgi) Petrov, Dimitar Trendafilov, Stoyan Nikolov, Nick (Nikola) Vasilev | The Recursive company profile; Stoyan Nikolov LinkedIn |
| Headquarters | Sofia, Bulgaria | LinkedIn, Dun & Bradstreet |
| Team size | ~50 (per the PocketGamer.biz interview, growth from 5 → ~50) | PocketGamer.biz |
| Current leadership | George Petrov (CEO), Dimitar Trendafilov (CTO), Nick Vasilev (R&D Director) | About page |
| Latest product version | Gameface 3.0.1 / 3.0.1.1 (current LTS + feature tracks) | Gameface docs changelog |
| License | Proprietary commercial; **quote-based**, no public price list as of 2026 | Pricing page; FAQ |
| Indie pricing | "Special pricing for indies on request" — no fixed indie tier publicly documented | Pricing page |
| Trial | 30-day evaluation with watermark | Pricing page |
| Authoring | Standard HTML5 + CSS3 + JavaScript; Coherent **Gameface** for code-first developers; Coherent **Prysm** for Adobe-Animate-driven artists | Product pages |
| Runtime | Native C++; `Cohtml` HTML engine + `Renoir` rendering library | Renoir announcement |
| Renderer | DX11/12, Vulkan, Metal, OpenGL/GLES, console-native | Renoir announcement; Features page |
| Engines | Unreal Engine (UE 4 + UE 5), Unity (2020.2+), custom C++ | Product page |
| Platforms | Windows, macOS, Linux, iOS, Android, UWP, Xbox One/Series, PS4/PS5, Switch/Switch 2, WebGL | Features page |
| Web framework support | React, Preact, SolidJS, jQuery, Webpack, anime.js, TypeScript, Tailwind CSS | Features page |
| JS engine | **V8** on platforms where binary distribution permits it; alternate VM elsewhere (`window.onerror` is documented as V8-only) | Differences-to-traditional-browsers docs |
| Accessibility | **TextToSpeech + ARIA plugin family** (`CohtmlARIAHoverReadPlugin`, `CohtmlARIAFocusChangePlugin`, `CohtmlARIALiveRegionsPlugin`) — speech-API-driven, **no AccessKit, no OS-AT bridge** | Accessibility docs |
| Predecessor products | Coherent UI (announced 2012-09-24, WebKit-backed, subscriptions ended 2017-12-05); Coherent GT (WebKit-backed); Hummingbird (in-house engine, mobile-first, rebranded as Gameface 1.0 on 2018-12-07) | Announcing-Coherent-UI post, Gameface 1.0 release post |

## Honest assessment

Coherent Gameface is the **strongest existing-art for "full HTML5/CSS/JS in a game engine"** — both as a feasibility proof and as a cost study. The feasibility proof: AAA studios ship Gameface in production at console scale across Unity, Unreal, and custom engines. The cost study: doing this required **a proprietary in-house HTML engine** (`Cohtml`) and a proprietary in-house GPU renderer (`Renoir`) — both maintained by Coherent Labs as commercial products, both kept up-to-date against an ever-moving web platform spec, both behind quote-based pricing that gate-keeps the technology away from indie + open-source ecosystems.

That cost is the central lesson. Coherent's "we are faster than Chromium / CEF" claim (10× main-thread improvement on heavy pages, ~1ms UI completion on PS4-era hardware) is the *product justification*: if you're shipping web-platform UI in a game, embedded Chromium is the wrong shape, and Coherent makes the case for an in-house engine. But sustaining an in-house HTML engine is a multi-decade commitment — Coherent has been at it since 2012 (Coherent UI, WebKit-based), and the current Cohtml engine is the third generation. The web platform keeps moving (container queries, anchor positioning, modern color, variable fonts, view transitions, scroll-driven animations) and Cohtml must track or fall behind. **The feasibility of "AAA HTML5 game UI" is empirically proven; the feasibility of doing it as an open-source MIT/Apache project without a paying customer base is the open question Buiy answers differently.**

Buiy's answer (foundation [`architecture.md` § 2.1](../../specs/2026-05-07-buiy-foundation/architecture.md#21-one-line-summary)): **don't ship an HTML engine.** Buiy renders Bevy components directly via Taffy + cosmic-text + Bevy's render graph; BSN (`.bsn` asset format) is the structurally-analogous-to-HTML/UXML authoring layer; CSS-equivalents are decomposed components rather than a CSS parser. The "subset of web platform optimized for game UI" cost is paid in *spec-and-implementation effort* (a couple of subsystems per quarter, foundation [§ 4 sub-spec roadmap](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) rather than in *parser-and-engine-tracking effort*. That trade-off is Buiy's central architectural bet, and Coherent Gameface is the canonical data point for what the alternative path costs.

The pre-amble claim about a **Hellbender / Streamline Group acquisition (~2021)** could **not be verified** in public sources as of 2026-05-22 — Crunchbase, the company's About page, and the search corpus return no parent-company information. Treat as unverified; see [`history.md`](history.md) and [`distribution-and-governance.md`](distribution-and-governance.md). The pre-amble claim that **The Witcher 3** uses Coherent could not be verified either (CD Projekt Red uses its in-house engine + Scaleform/Flash historically); **Cyberpunk 2077** also could not be verified. The verified-only customer list is in [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md).

## Table of contents

1. [`architecture.md`](architecture.md) — Cohtml HTML engine + Renoir rendering library, two-product split (Gameface / Prysm), engine-binding shape, threading + memory model.
2. [`html5-coverage.md`](html5-coverage.md) — HTML/CSS/JS feature coverage: standard-compliant HTML5, CSS3 transforms / animations / filters / blend modes, Flexbox (full), Grid (custom-element component, NOT native CSS Grid), V8 JS, supported frameworks, what's missing vs full Chromium.
3. [`engine-integration.md`](engine-integration.md) — Unreal binding (UE 4 + UE 5), Unity binding (2020.2+), custom C++ binding, render hooks, threading, data-binding.
4. [`hummingbird-vs-chromium.md`](hummingbird-vs-chromium.md) — The case for a custom HTML engine over embedded Chromium/CEF: performance claims (10× main-thread, sub-millisecond render), memory claims, IPC critique. Marketing claims labeled as such.
5. [`history.md`](history.md) — 2012 founding, Coherent UI announcement (2012-09-24), Coherent GT, Hummingbird, Gameface 1.0 rebrand (2018-12-07), Coherent UI EOL (2017-12-05), 3.x cadence, ownership unverified.
6. [`distribution-and-governance.md`](distribution-and-governance.md) — Quote-based pricing, no public tiers, parent-company unverified, ~50-staff team, long-term viability.
7. [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) — Verified AAA users; comparison vs NoesisGUI (XAML), RmlUi (HTML subset open source), Scaleform (Flash predecessor), Buiy (open MIT).
8. [`critiques-and-open-problems.md`](critiques-and-open-problems.md) — Proprietary lock-in; HTML5-engine maintenance burden; accessibility-via-TextToSpeech (not OS-AT); pricing opacity; missing modern CSS (Grid native, container queries, anchor positioning, modern color).
9. [`lessons.md`](lessons.md) — **THE DECISION FILE** — validates / avoid / borrow for Buiy.
10. [`glossary.md`](glossary.md) — Coherent Gameface, Coherent UI, Coherent GT, Coherent Prysm, Hummingbird, Cohtml, Renoir, Coherent Labs, etc.

## How to use

**Framing disclosure.** These docs are written from a Bevy-only / open-source / ECS-native / AccessKit-first Buiy stance. Most "Implications for Buiy" sub-sections frame Coherent's choices through that lens. Future readers auditing whether *those* Buiy commitments are themselves the right primitives should weigh the corpus accordingly. Coherent Gameface is the canonical "what proprietary commercial full-HTML5 game UI middleware looks like" data point; Buiy is taking a different shape on purpose.

Cross-reference with:

- [`noesisgui/`](../noesisgui/) — proprietary XAML cousin; same proprietary-commercial-middleware posture, different markup language.
- [`rmlui/`](../rmlui/) — open-source HTML/CSS-flavored alternative; same web-platform-flavoring, MIT-licensed, no in-house JS engine.
- [`unreal-slate-umg/`](../unreal-slate-umg/), [`unity-ui/`](../unity-ui/) — engine-native UI stacks Coherent competes with at AAA tier.
- [`bevy-ui/`](../bevy-ui/) — substrate Buiy parallels; complementary precedent for what bevy-ui doesn't ship.

## Sources

- Coherent Labs homepage — https://coherent-labs.com/
- Coherent Gameface product page — https://coherent-labs.com/products/coherent-gameface/
- Coherent Gameface all-features page — https://coherent-labs.com/all-features-gameface/
- About Coherent Labs — https://coherent-labs.com/about-us/
- Gameface documentation (C++) — https://docs.coherent-labs.com/cpp-gameface/
- Gameface documentation (Unreal) — https://docs.coherent-labs.com/unreal-gameface/
- Gameface documentation (Unity) — https://docs.coherent-labs.com/unity-gameface/
- Coherent Labs FAQ — https://coherent-labs.com/frequently-asked-questions/
- Coherent Labs pricing page — https://coherent-labs.com/pricing/
- Announcing Coherent UI (2012-09-24) — https://coherent-labs.com/posts/announcing-coherent-ui/
- Gameface 1.0 release (2018-12-07) — https://coherent-labs.com/blog/releases/gameface-1-0-released/
- Prysm 1.0 release (2018-12-07) — https://coherent-labs.com/blog/releases/prysm-1-0-released/
- Coherent UI subscription end (2017-12-05) — https://coherent-labs.com/posts/coherent-labs-will-no-longer-provide-subscriptions-for-coherent-ui/
- Renoir Graphics Library introduction — https://coherent-labs.com/posts/introducing-renoir-graphics-library/
- Vulkan-support announcement — https://coherent-labs.com/vulkan-support/
- CEF critique post — https://coherent-labs.com/posts/what-developers-should-consider-when-using-chromium-embedded-framework-cef-in-their-games/
- PocketGamer.biz interview ("new standard for game UI") — https://www.pocketgamer.biz/interview/67816/can-coherent-labs-rise-as-the-new-standard-for-game-ui/
- The Recursive company profile — https://therecursive.com/company/coherent-labs/
- Stoyan Nikolov public bio — https://meetingcpp.com/2018/Speaker/items/Stoyan_Nikolov.html
- Crunchbase profile — https://www.crunchbase.com/organization/coherent-labs
- SteamDB tech detection — https://steamdb.info/tech/SDK/Coherent_Gameface_OR_Prysm/
- GameUIComponents OSS repo — https://github.com/CoherentLabs/GameUIComponents
- Differences-to-traditional-browsers docs — https://docs.coherent-labs.com/cpp-gameface/what_is_gfp/htmlfeaturesupport/
- Gameface CSS Properties reference — https://docs.coherent-labs.com/cpp-gameface/content_development/supported_features_tables/cssproperties/
- TextToSpeech / ARIA docs — https://docs.coherent-labs.com/cpp-gameface/integration/optional_features/texttospeech/
