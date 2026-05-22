**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — production users + partners + cross-toolkit comparisons (Qt/QML, egui, Iced, Dioxus, Druid/Xilem, Buiy)

# Ecosystem and comparisons

This file combines two related lenses: who's actually shipping Slint (ecosystem) and how Slint compares against the other GUI toolkits a Buiy designer might be weighing it against (Qt/QML, egui, Iced, Dioxus, Druid/Xilem, plus the Buiy non-comparison).

## Production users (verified)

- **OTIV** — rail automation. Industrial customer named explicitly in the Rust Foundation member spotlight; Slint provides "safe, reliable, and intuitive user interfaces" for assisted / remote / autonomous rail-driving systems. Safety-critical embedded HMI use case. The single most-cited Slint production deployment.
- **KDAB consulting clients** — KDAB (110+ employees, offices in Germany / France / UK / USA / Sweden) lists Slint as a service-partner offering. Multiple KDAB-integrated Slint deployments are referenced in KDAB and Slint marketing, generally industrial / automotive / medical without named-customer disclosure.
- **LibrePCB 2.0** — open-source PCB design software that completed its Qt → Slint migration in LibrePCB 2.0 (released 2025; 2.0.1 in Feb 2026). Listed in the "Making Slint Desktop-Ready" blog post as a flagship migration partner driving Slint's desktop-readiness work.
- **Internal Slint demos**: `slint-viewer`, the Live Preview, the Online Editor, and the WASM demos themselves are all production Slint apps maintained by SixtyFPS GmbH.

Industries cited (per Rust Foundation spotlight) in the commercial customer base: industrial automation, medical devices, automotive, aerospace/defense.

## Partners (per https://slint.dev/partners)

**Service partners** (integration consulting):
- **KDAB** (Germany / France / UK / USA / Sweden; embedded + desktop + mobile)
- **Crossware** (cross-platform UI / IoT / multimedia for Linux + MCUs + Android)
- **Cynetis Embedded** (embedded software dev tools, RTOS, hardware)
- **Extenly** (embedded UI + cross-platform app dev)
- **Felgo** (mobile / desktop / embedded / web)
- **Spyrosoft** (1000+-employee software group; automotive / healthcare / industrial)
- **tQCS** (industrial / medical / automotive embedded systems)

**Silicon and hardware partners**:
- **Espressif** (ESP32 family; the ESP-IDF target)
- **STMicroelectronics** (STM32; first-class Slint target since 1.8)
- **Raspberry Pi**
- **Toradex** (Arm-based embedded hardware + software)

**Operating systems**:
- **QNX** (safety-critical RTOS; 255M+ deployed vehicles per QNX marketing)
- **Yocto Project** (custom Linux for embedded)
- **Zephyr Project** (Linux Foundation RTOS)

The partner ecosystem is structurally an embedded / industrial story; the desktop ecosystem is less partner-mediated.

## Honest-scale framing

Slint is a **mid-sized open-source product** by Rust GUI standards:

- 22.7k GitHub stars (folder-write time).
- 1.1M lifetime crate downloads on `slint` (237k in the recent 90d).
- 55+ crate releases since 0.2.0 (2022-02).
- Crate categories: `gui`, `rendering::engine`, `no-std`.

By comparison: egui ~25k stars; Iced ~25k stars; Bevy ~37k stars; Dioxus ~22k stars. Slint sits in the comparable band of "established Rust GUI projects with several thousand commits and a working 1.0 product."

**Production-deployment scale is harder to quantify.** OTIV is named; KDAB's clients aren't all named; LibrePCB is in-progress. There is no published "X apps shipping Slint" count. The embedded / safety-critical focus means many deployments are internal industrial systems that don't advertise their UI toolkit publicly.

## Comparisons

The comparison set is the alternatives a Buiy designer would weigh against Slint.

### vs Qt / QML (the parent)

The most important comparison because Slint is QML-flavored by design and the founders shipped QML at The Qt Company.

| Dimension | Qt / QML | Slint |
|---|---|---|
| **Implementation language** | C++ | Rust |
| **DSL** | QML (JavaScript-flavored, dynamic typing inside expressions) | `.slint` (statically typed, pure bindings enforced) |
| **License** | LGPL / commercial (similar dual-license shape) | GPL / royalty-free / commercial (similar triple-license shape) |
| **Ecosystem age** | 30+ years; massive | 6 years; growing |
| **Platform breadth** | Desktop + mobile + embedded + automotive + every BSD | Desktop + mobile + WASM + MCU + QNX (via partner) |
| **Accessibility** | Mature platform-native (Qt Accessibility) | AccessKit-backed; pinned a few versions behind upstream |
| **Authoring** | DSL + C++ + QML JavaScript expressions | DSL + Rust / C++ / JS / Python; pure expressions only |

The "Slint is the Rust-native QML" framing is what the founders own. The structural differences: Slint's DSL is statically typed (QML is dynamic in expressions); Slint enforces pure bindings (QML doesn't); Slint targets MCUs more aggressively than mainstream Qt does (Qt for MCU exists but is a separate product line).

### vs egui (immediate-mode peer)

| Dimension | egui | Slint |
|---|---|---|
| **Mode** | Immediate-mode | Retained-mode |
| **Authoring** | Pure Rust API | `.slint` DSL + Rust |
| **Accessibility** | AccessKit (enabled by default in eframe) | AccessKit (manual wiring) |
| **Target** | Game tools, debugging UIs, simple apps | Embedded HMI + desktop apps |
| **Maturity** | 1.x; broadly adopted | 1.x; commercial product |
| **License** | MIT OR Apache-2.0 | GPL OR royalty-free OR commercial |

Different design points entirely. egui is great for game-tool UI and debugging; Slint is great for retained-mode app UI. Buiy is retained-mode like Slint, but with Bevy ECS as authoring (not DSL).

### vs Iced

| Dimension | Iced | Slint |
|---|---|---|
| **Architecture** | Elm-architecture-inspired; message-passing | DSL + reactive bindings; QML-inspired |
| **Authoring** | Pure Rust API | `.slint` DSL + Rust |
| **Accessibility** | Draft only (PR open since 2025-11) | AccessKit since 2023-06 (production) |
| **License** | MIT | GPL OR royalty-free OR commercial |
| **Renderer** | wgpu | Skia / FemtoVG / FemtoVG-WGPU / software / Qt |

Iced is Buiy's nearest stylistic neighbor in the Rust UI ecosystem in *some* ways (pure-Rust authoring, no DSL), but with a fundamentally different message-passing architecture (vs Buiy's ECS observers). The license is what makes Iced the candidate Bevy-ecosystem developers actually consider; Slint's license keeps it out of that conversation.

### vs Dioxus

| Dimension | Dioxus | Slint |
|---|---|---|
| **Architecture** | React-like; virtual DOM; signals | DSL + reactive bindings |
| **Authoring** | RSX macros in Rust | `.slint` DSL |
| **Renderer** | Web (DOM) + native (custom) + WGPU experimental | Skia / FemtoVG / software / Qt |
| **Target** | Web-first; cross-platform via native | Embedded + desktop + WASM + MCU |
| **License** | MIT OR Apache-2.0 | GPL OR royalty-free OR commercial |

Different niches. Dioxus is "React for Rust"; Slint is "QML for Rust." Both ship; both have growing ecosystems. Buiy is neither — Buiy is "Bevy ECS for UI."

### vs Druid / Xilem (Linebender lineage)

| Dimension | Xilem (current Linebender direction) | Slint |
|---|---|---|
| **Architecture** | View tree with reactive state; new generation succeeding Druid | DSL + reactive bindings |
| **Authoring** | Pure Rust API | `.slint` DSL |
| **Accessibility** | AccessKit (Linebender are AccessKit co-stewards) | AccessKit (pinned behind) |
| **Renderer** | Vello (Linebender's GPU 2D renderer) | Skia / FemtoVG / etc. |
| **License** | MIT OR Apache-2.0 | GPL OR royalty-free OR commercial |
| **Target** | Desktop apps (the Druid niche); experimental | Embedded + desktop |

Xilem is the most architecturally adjacent to "what Buiy is doing" in some respects — declarative view trees with reactive state in Rust. Linebender owns the AccessKit producer integration story alongside Slint and egui. Different ecosystem (no Bevy ECS); same MIT/Apache license tier.

### vs Buiy (the non-comparison)

Buiy is a UI library for the Bevy game engine targeting full web-platform-parity for games and apps with WCAG 2.2 AA accessibility. Slint is a standalone commercial GUI product targeting embedded + desktop with AccessKit-based accessibility. The structural differences:

| Dimension | Slint | Buiy |
|---|---|---|
| **Host engine** | Standalone (Qt-replacement positioning) | Bevy plugin |
| **Authoring** | `.slint` DSL | ECS + BSN |
| **Layout** | Custom layout solver | Taffy (shared with bevy_ui) |
| **Text shaper** | Parley + Fontique (since 1.14) | cosmic-text |
| **Accessibility** | AccessKit (pinned behind upstream) | AccessKit (pin-cadence policy) |
| **License** | GPL OR royalty-free OR commercial | MIT OR Apache-2.0 |
| **Target** | Embedded + desktop + WASM + MCU + mobile | Bevy desktop + mobile + (WASM TBD) |
| **Feature scope** | What an HMI / desktop app needs | Full web-platform UI parity |

Slint and Buiy are not direct competitors and don't substitute for each other. A user evaluating "what UI library for my Bevy game?" would not consider Slint; a user evaluating "what UI library for my STM32 industrial controller?" would not consider Buiy. The corpus exists because Slint is a relevant *prior art* — patterns to borrow (AccessKit producer wiring; property qualifiers; embedded-allocation discipline) — not a competitor.

## Implications for Buiy

- **Qt/QML is the spiritual ancestor of both Slint and any future Bevy-side DSL.** If Buiy ever adds a DSL, study QML (and Slint as its Rust-native cousin) first — these are the load-bearing reference designs.
- **AccessKit-adopter cohort is small and worth keeping aligned with.** Slint + egui + Freya + Xilem/Masonry + Bevy. Each has production AccessKit producer code; each can be a check on whether Buiy's producer-side decisions are reasonable.
- **License is what keeps Slint out of the Buiy decision tree.** Even if Slint's technical fit were perfect (it's not — Bevy ECS + Slint runtime can't easily share a window), the GPL+commercial gate would. Buiy's MIT OR Apache-2.0 is non-negotiable for the Bevy ecosystem.
- **Embedded toolkit revenue model doesn't translate to game-UI library.** Slint's commercial-license + partner-ecosystem revenue stream depends on industrial / medical / automotive customers paying for safety-critical HMI software. Game studios don't pay for UI libraries the same way; the business model would have to be entirely different (e.g., engine-level sponsorship, foundation grants, individual-maintainer GitHub Sponsors). Buiy's foundation-style governance doesn't try.
- **Production-deployment proof is patchy across the Rust UI ecosystem.** Slint has OTIV + KDAB clients + LibrePCB-in-progress; egui has a long tail of small apps; Iced has Cosmic Desktop; Dioxus has internal Cloudflare deployment + early adopters. None of them have a flagship game studio deployment. Buiy aiming for "Bevy ecosystem UI" puts it in a niche no Rust GUI toolkit has flagship-shipped to yet.

## Sources

- Rust Foundation member spotlight (OTIV): https://rustfoundation.org/media/member-spotlight-slint/
- Slint partners page: https://slint.dev/partners
- Slint "Making Slint Desktop-Ready" (LibrePCB 2.0): https://slint.dev/blog/making-slint-desktop-ready
- KDAB Slint partner page: https://www.kdab.com/software-technologies/slint/
- Slint crate stats: https://crates.io/crates/slint
- egui repo: https://github.com/emilk/egui
- Iced repo: https://github.com/iced-rs/iced
- Dioxus repo: https://github.com/DioxusLabs/dioxus
- Xilem repo: https://github.com/linebender/xilem
- Qt Company: https://www.qt.io/
- Sibling files: [`accessibility.md`](accessibility.md), [`governance-and-distribution.md`](governance-and-distribution.md), [`open-problems.md`](open-problems.md)
- Sibling prior-art: [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md), [`../bevy-ui/comparisons.md`](../bevy-ui/comparisons.md), [`../egui/`](../egui/), [`../iced/`](../iced/)
