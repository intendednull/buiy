**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — verified customers, AAA production deployments, comparison vs UGUI / UI Toolkit / Slate / UMG / Coherent Gameface / Buiy

# Ecosystem & comparisons

NoesisGUI's customer roster is the single strongest piece of evidence for "this works at AAA scale." This file documents the verified-only customer list, separates corrected facts from the pre-amble's mistakes, and compares NoesisGUI against the other production game-UI options.

## Verified production customers (from Noesis customers page, May 2026)

Direct quote of the publicly listed customers, with categorization. The list is on [noesisengine.com/customers.php](https://www.noesisengine.com/customers.php).

### AAA games

| Game | Studio / Publisher | Notes |
|---|---|---|
| **Baldur's Gate 3** | Larian Studios | Confirmed via Larian testimonial; uses Noesis 3.1.6 per third-party docs. |
| **Hellblade 2** (Senua's Saga) | Ninja Theory / Xbox Game Studios | Released May 2024. |
| **Age of Wonders 4** | Triumph Studios / Paradox Interactive | Released May 2023. |
| **TopSpin 2K25** | Hangar 13 / 2K Games | Released April 2025. |
| **Hytale** | Hypixel Studios / Riot Games | In development; not yet released. |
| **Cricket 24** | Big Ant Studios | Released. |
| **iRacing** | iRacing | Online sim platform; ongoing. |

### Major studio customers

Microsoft, Ninja Theory, THQ Nordic, Capcom, Sumo Digital, Riot Games, Larian, 2K Games, Hangar 13, Crystal Dynamics, Paradox Interactive, Triumph Studios, Iceflake Studios, Gameloft, Tencent, Bonfire Studios, Bluepoint, Dambuster Studios, Take-Two Interactive, Lightspeed & Quantum, Media Molecule, Bugbear Entertainment, Hypixel Studios.

### Long-tail studio customers

Flaming Fowl Studios, Neobards Entertainment, 4J Studios, Black Forest, Big Ant Studios, Amazon Games, Grip Studios, Zwift, Nano Games, Bright Star Studios, HiberWorld, 1C Game Studios, WarDucks, Wookey Technologies, Linden Lab, Blitworks, TNB Games, Deviation Games, Reforged Studios, Atomic Torch Studio, Foresight Sports, Rebound Capital Games, The Game Kitchen, A Heart Ful of Games, Firefly Studios, Strangely Interactive, Run 8 Studios, StatusPro, DungeonFog, iGPManager, Tension, Taleworlds Entertainment, Kingdom Games, Bungarra Software, Digits Crossed Interactive, Overhype Studios, Dreamloop Games, Rockodile Games, Nexon Korea, PearlAbyss, Novaquark, Effixa Games, Polycular, Polywick Studio, Wired Games, Fourth Quarter, LionFire Games, Frozen Walrus, Krizelkratz 3000, Shiny Egg Studios, Rigid-soft, Celebrus Studios, Heptamind.com.

### Simulation sector

Medhost, z-Space, Modern Value, HERE, Crimson Leaf, Xmark Labs, Metanous.be, Atomsk.nl, Strongly-Typed Solutions, Holopoint.com.au, CAE, Kratos Defense, Ziosoft, S-Plane, Onepoint, GlobalSim, Theoris, Canon, Beyeonics Surgical, Morton Buildings, Smart Simulations, SimLab Studios, Dynamic Software Solutions, SideEffects AG, Slidecrew, Enscape, System D Labs, E4D, PK Sound, Tazman Audio, Coach Vision, Spivi, Clario Group, Covirtua.

### Industrial sector

FLUOR, ARUP, Maxon, Avanade, Plain Concepts, Easylaser, Redgiant, iQmetrix, Voca, AVIRE e-motive Displays, Dimark IT, MGGT, Bweez, Oreka Group, Latent Strategies, gemelo GmbH, Net-Allied Systems, iD3i, Lightimage.nl, RT Information Technologies, Zeropass.io, Beijing Eryiju Technology, CompuWeigh, AG IT Support, xeno-bits, GE.

## Pre-amble corrections

The task pre-amble named "Counter-Strike 2, Apex Legends, Microsoft Flight Simulator, Destiny 2" as suspected customers. **These are not verified.**

- **Counter-Strike 2** — Not on the public customer list. Valve is not a Noesis customer per public sources. The pre-amble was wrong.
- **Apex Legends** — Not on the public customer list. Respawn is not a Noesis customer per public sources. The pre-amble was wrong.
- **Microsoft Flight Simulator** — Not on the public customer list. Microsoft is listed as a customer but no specific game (and FlightSim is not named). The "Microsoft Flight Simulator" claim is unsupported.
- **Destiny 2 / Bungie** — Not on the public customer list. The pre-amble was wrong.

The **verified AAA hits using NoesisGUI** are: Baldur's Gate 3 (Larian), Hellblade 2 (Ninja Theory), Age of Wonders 4 (Triumph), TopSpin 2K25 (Hangar 13), Hytale (Hypixel, unreleased), Cricket 24 (Big Ant), iRacing. This is still a strong AAA roster — Baldur's Gate 3 alone is one of the highest-profile RPG releases of the 2020s — but it does not include the FPS-shooter games the pre-amble claimed.

## Worked example: Baldur's Gate 3 (Larian)

The most detailed publicly-documented Noesis deployment. Larian's testimonial:

> *"As the studio grows, so do our complexities in development. It was important that we put in place a UI middleware solution that could cope with all demands that Baldur's Gate 3 required. The MVVM pattern that Noesis uses is extremely flexible. It allows us to build large and complex interfaces that are easy to maintain."*

What we know publicly:

- Baldur's Gate 3 shipped on **NoesisGUI 3.1.6**.
- The game's UI is implemented in XAML files loaded by Noesis at runtime; the [Baldur's Gate 3 modding documentation](https://docs.baldursgate3.game/index.php?title=UI) explicitly describes the XAML structure and how mods extend it.
- The choice was driven by **MVVM scalability** at the scale of a complex CRPG UI (party management, inventory, character creation, conversations, combat tactics, save/load, options).
- Larian's engine (Divinity 4.0) is a custom in-house C++ engine; the Noesis integration was direct C++ SDK, not via Unity / Unreal.

This is the most complete picture of Noesis-at-AAA-scale: a custom-engine C++ integration, MVVM-architected UI, mod-extensible XAML, six years of development. It is the existence proof for the comprehensive-UI-library-at-AAA-scale claim that anchors Buiy's foundation.

## Comparison vs other game UI options

| Axis | UGUI | UI Toolkit (Unity) | Slate | UMG | NoesisGUI | Coherent Gameface | Buiy (target) |
|---|---|---|---|---|---|---|---|
| **Markup** | Inspector-driven | UXML (HTML-like) + USS | C++ DSL | Inspector / Blueprint | XAML | HTML + CSS + JS | BSN |
| **Engine binding** | Unity only | Unity only | Unreal only | Unreal only | Unity, Unreal, custom | Multiple | Bevy only |
| **License** | Proprietary | Proprietary | Proprietary (Unreal EULA) | Proprietary | Proprietary commercial | Proprietary commercial | MIT / Apache |
| **Source available** | No | Source (Unity Sources) | Yes (Unreal source) | Yes | Optional paid | Source on Premium | Yes (open) |
| **Vector graphics** | No | Limited | Limited | Limited | Yes (full) | Yes (HTML / CSS) | Yes |
| **Rounded clipping** | Shader workaround | Yes | Limited | Limited | Yes | Yes | Yes |
| **Backdrop blur** | No | No | No | No | Yes | Yes | Yes |
| **Mix-blend-mode** | No | No | No | No | No | Yes | Yes |
| **A11y / screen readers** | No (third-party) | No (third-party) | Limited (Windows screen-reader) | Limited | No | Limited (HTML ARIA) | Yes (AccessKit) |
| **MVVM / data binding** | No | UI Toolkit bindings | C++ delegates | Blueprint bindings | Yes (XAML) | JS bindings | ECS observers + change detection |
| **Hot-reload** | No | UXML hot-reload | Limited | UMG runtime tweaks | Yes (templates, since 3.2.11) | Yes (web devtools) | Yes (`.bsn` assets) |
| **Console support** | Yes | Yes | Yes | Yes | Yes (all majors) | Yes | Open (Bevy-driven) |
| **AAA shipping** | Yes (many) | Some | Yes (many) | Yes (many) | Yes (BG3, Hellblade 2) | Yes (Civ7, WoT) | Not yet |

The NoesisGUI/Coherent comparison is the most direct: both are **commercial cross-engine UI middleware** with **comprehensive feature sets** shipped in AAA games. The choice between them is:

- **NoesisGUI**: XAML-native, MVVM-ready, vector graphics native, strong Unity / Unreal / custom-C++ presence.
- **Coherent Gameface**: HTML + CSS + JS, web-developer-friendly, browser-engine-derived (forked WebKit subset), strong Unreal presence, used by Civilization 7 and World of Tanks 2.0.

The XAML-vs-HTML axis is the central choice for a studio. Studios with WPF / .NET heritage tend toward NoesisGUI; studios with web-developer hires tend toward Gameface. Each has shipped at AAA.

## Buiy positioning

Against this comparison, Buiy is **none of the above**. Buiy is:

- **Open source** (none of the commercial middleware can match this).
- **ECS-native** (none of the others — UGUI / UMG are GameObject-based, Slate is C++ widget objects, UIToolkit / Noesis / Gameface are retained-tree-based).
- **Bevy-only** (sharp contrast to NoesisGUI / Gameface multi-engine; sharp contrast to UGUI/UI Toolkit/Slate/UMG single-engine; sharp contrast in a different direction — Buiy doesn't pretend to be portable).
- **AccessKit-first** (none of the others ship comprehensive a11y; this is Buiy's largest single product differentiator).
- **Web-platform-parity-targeting** (Gameface is web-derived but doesn't claim parity; Buiy explicitly aims at it).

This positioning is **a deliberate market gap**: there is no open-source, ECS-native, accessibility-first, comprehensive UI library for any game engine. Buiy is filling it for Bevy specifically.

## Implication for Buiy

The NoesisGUI ecosystem proves three things relevant to Buiy:

1. **AAA studios will adopt a non-engine-native UI library** if it gives them a comprehensive feature set + MVVM scalability + good tooling. Larian, Ninja Theory, Triumph, Hangar 13 are all examples. **For Buiy, this means: the ergonomic / featureful Bevy-side UI library is something studios will use even if it isn't bevy_ui-blessed.**
2. **The "shipped in AAA" claim is the highest-value validation point** for a UI library. Until Buiy ships in a flagship Bevy title, the comprehensive-coverage claim is theoretical. NoesisGUI has Baldur's Gate 3; Buiy needs an analog.
3. **The web-platform-parity claim is not yet reduced to practice anywhere.** Neither NoesisGUI (XAML-derived) nor Gameface (HTML-derived) claims modern container queries, anchor positioning, view transitions, or full WCAG conformance. Buiy aiming there is genuinely novel; the corpus does *not* point to a "done it before" model.

## Sources

- NoesisGUI customers page — https://www.noesisengine.com/customers.php
- Larian Baldur's Gate 3 modding docs — https://docs.baldursgate3.game/index.php?title=UI
- GameDev.net BG3 announcement — https://www.gamedev.net/news/noesis-technologies-rolls-out-30-ui-tool-used-in-baldurs-gate-iii-r1347/
- Coherent Gameface — https://coherent-labs.com/products/coherent-gameface/
- SaaSHub comparison — https://www.saashub.com/compare-coherent-gt-vs-noesisgui
- gamefromscratch UI roundup — https://gamefromscratch.com/game-user-interface-technologies-roundup/
- Buiy foundation goals — ../../specs/2026-05-07-buiy-foundation/README.md
- Related prior-art folders: [`bevy-ui/`](../bevy-ui/), [`unreal-slate-umg/`](../unreal-slate-umg/), [`gpui/`](../gpui/)
