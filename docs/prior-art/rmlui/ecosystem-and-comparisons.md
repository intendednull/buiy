**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — shipping users; head-to-head vs NoesisGUI, Coherent Gameface, Unity UGUI / UI Toolkit, bevy_ui, Buiy

# Ecosystem and comparisons

RmlUi occupies a specific niche: **open-source, MIT-licensed, C++-embeddable HTML/CSS-flavored UI** with a portable embedder-interface. The shipping-user list is small but consistent — primarily indie + AA studios, modding communities, and game-tooling sectors where the proprietary cousins' licensing costs do not pencil out and the in-engine UI tools are inadequate.

## Shipping users (verified via RmlUi README)

| User | Type | Studio / org | Notes |
|---|---|---|---|
| **The Thing: Remastered** | Game | Nightdive Studios | Released; remake of the 2002 horror game. Nightdive uses RmlUi in its **KEX** engine for HUD + menus. |
| **Killing Time: Resurrected** | Game | Nightdive Studios | Released. Same KEX-engine + RmlUi pipeline. |
| **ROSE Online** | Game | OSROSE / community | Free-to-play MMORPG. |
| **Unvanquished** | Game | Unvanquished Development | Open-source Tremulous-derived FPS. **Adopted libRocket in the early 2010s**, migrated to RmlUi after the fork. The single longest-running shipping user spanning both eras. |
| **Alchemist** | Tool | Cfx.re / Rockstar Games | Asset-converter tool. **Cfx.re is the team behind FiveM (the Grand Theft Auto V multiplayer mod ecosystem)**, and Rockstar Games acquired Cfx.re in 2023. The most commercially significant user lineage. |
| **TruckersMP** | Game / mod | TruckersMP project | Multiplayer mod for *Euro Truck Simulator 2* / *American Truck Simulator*. |
| **WOTInspector** | Tool | Community | World of Tanks analysis tool. |

### Reading the user list honestly

- **No AAA flagship.** No Baldur's Gate 3, no Hellblade 2, no major Activision / Ubisoft / EA / Take-Two-developed-internally title. The proprietary cousins (NoesisGUI in particular) own that segment.
- **No flagship Bevy or Rust title.** RmlUi is C++; the Bevy / Rust ecosystem has its own UI stack (bevy_ui, Buiy, Slint, Freya, Makepad).
- **The Cfx.re / Rockstar lineage is the commercially significant outlier.** Cfx.re Alchemist as a Rockstar-owned tool puts RmlUi in the *production tooling* tier at a AAA publisher — but in a tool, not in a shipped game's player-facing UI.
- **Nightdive's KEX engine ships RmlUi** in multiple commercial remasters (The Thing, Killing Time). KEX is a meaningful AA-tier remaster engine; this is the strongest "shipped game UI" data point.
- **Unvanquished** is the longest-running data point spanning both libRocket and RmlUi eras — a ~15-year-and-counting deployment validating long-term project stability.

### Users *not* on the list

Some commonly-rumored RmlUi adopters that **do not appear** in the verified README list:

- **Hytale** (Hypixel) — has been associated with RmlUi historically; not currently named in the README's user list as of 2026-05-22.
- **Larian Studios games** (Divinity Original Sin / Baldur's Gate 3) — these are NoesisGUI users (BG3 is the canonical NoesisGUI showcase).
- **Cyberpunk 2077 / The Witcher 3 (CD Projekt)** — internal Scaleform replacement; not RmlUi.

The verified user list is what the README publishes; absent contradicting evidence, treat unverified rumors as unverified.

## Comparisons

### vs NoesisGUI (closest design-space neighbor)

| Axis | RmlUi | NoesisGUI |
|---|---|---|
| License | MIT (free, no royalties) | Proprietary commercial (€195 indie / €9K–€18K pro/premium) |
| Authoring | RML (HTML-flavored) + RCSS (CSS-flavored) | XAML (Microsoft / WPF-derived) |
| Render | Embedder implements `RenderInterface` | RenderDevice abstraction (similar concept) |
| Maintainer | Single (`mikke89`) | Small company (Noesis Technologies, Madrid) |
| AAA users | None | Baldur's Gate 3, Hellblade 2, Age of Wonders 4, TopSpin 2K25 |
| AA / indie users | Nightdive KEX, Cfx.re, ROSE, etc. | ~100 studios across industries |
| Platforms | Windows, Linux, macOS, Android, iOS, Switch | Adds UWP, PS4/5, Xbox One/Series, Switch 2, WebGL |
| Accessibility | None | None |
| Active dev | Slowing (~1 release/year recent) | Active (3.2.13 released 2026-04-27) |
| Cost to ship | Free | €195–€18K+ depending on tier |
| Bus factor | 1 | Small team (small but >1) |

**Verdict**: NoesisGUI dominates AAA via XAML + funded engineering team + commercial licensing flexibility; RmlUi dominates open-source + commercially-unrestricted + indie/AA. They occupy the same design space (HTML/XAML-flavored cross-engine UI middleware) on opposite sides of the open-source / proprietary boundary.

### vs Coherent Gameface (the other proprietary cousin)

| Axis | RmlUi | Coherent Gameface |
|---|---|---|
| Authoring | RML + RCSS | **Actual HTML5 + CSS3 + JavaScript** (Chromium-derived rendering) |
| Render | Embedder's renderer | GPU-accelerated, Chromium-engine-derived |
| Spec fidelity | CSS 2.1 + select CSS 3 | Modern HTML5 / CSS3 (close to web parity) |
| Vendor | Open source (mikke89) | Coherent Labs (Bulgaria; acquired by Sumo Group 2017) |
| License | MIT | Proprietary commercial |
| AAA users | None | Halo Infinite, Crysis Remastered Trilogy, Quake Remastered |

**Verdict**: Gameface offers full Chromium-grade HTML/CSS/JS at the cost of running a full browser-derived stack in-process; RmlUi offers a *fraction* of HTML/CSS for a fraction of the runtime / memory footprint. The proprietary stack wins on web-spec fidelity, the open-source stack wins on dependency surface + license cost.

### vs Unity UGUI / UI Toolkit (in-engine cousins)

| Axis | RmlUi | Unity UGUI | Unity UI Toolkit |
|---|---|---|---|
| Authoring | RML + RCSS | C# components in scene | UXML + USS (HTML/CSS-flavored) |
| Lineage | libRocket 2008 → RmlUi 2018 | Unity 4.6, 2014 | Unity 2019.1, opened to runtime 2021.2 |
| Spec target | XHTML 1.0 + CSS 2.1 + select CSS 3 | None | HTML5-flavored + CSS3-flavored subset |
| Engine integration | Embedder | First-party Unity | First-party Unity |
| Performance posture | "Lightweight" | Heavy on draw calls historically | Lighter than UGUI |

**Verdict**: RmlUi and UI Toolkit are conceptually similar (HTML/CSS-flavored data-driven UI in a game engine). UI Toolkit is Unity-only and First-party; RmlUi is engine-portable. The fact that Unity built UI Toolkit (effectively reinventing the libRocket / RmlUi pattern in-engine, with their own UXML + USS variant) is the strongest validation that the pattern is the right answer for in-engine UI.

### vs Unreal Slate / UMG

| Axis | RmlUi | Slate | UMG |
|---|---|---|---|
| Lineage | libRocket 2008 → RmlUi | UE4 era; SDK-only | UE3-4-5; user-facing wrapper on Slate |
| Authoring | RML + RCSS files | C++ Slate widgets | Blueprint UMG editor |
| Spec target | HTML / CSS family | None — Unreal-native | None — Unreal-native |
| Cross-engine | Yes | No | No |

**Verdict**: Slate / UMG are in-engine Unreal-only, with no HTML/CSS lineage. The Unreal ecosystem typically pairs Slate/UMG with NoesisGUI (paid AAA) or RmlUi (community / modding) when an HTML/CSS-flavored authoring experience is required.

### vs bevy_ui (the Buiy parallel)

| Axis | RmlUi | bevy_ui |
|---|---|---|
| Language | C++ | Rust |
| Architecture | Embedder-interface + own runtime | ECS-native (Bevy plugin) |
| Authoring | RML + RCSS files | Rust spawn-syntax + (future) BSN |
| Layout | Own engine (no Grid) | Taffy substrate (Flexbox + Grid + Block + Float) |
| Text | FreeType default; HarfBuzz sample | cosmic-text (≤ 0.18); parley + swash (≥ 0.19) |
| Accessibility | None | AccessKit (since 0.10, March 2023) |
| License | MIT | MIT OR Apache-2.0 |
| Engine target | Any C++ embedder | Bevy only |
| AAA flagship | None (Cfx.re is the closest) | None |

**Verdict**: They occupy **non-overlapping niches**. RmlUi is C++-portable; bevy_ui is Bevy-only. The architectural patterns differ (embedder-interface vs ECS-native). The most interesting comparison is **what they prioritize**: bevy_ui (and Buiy by extension) has accessibility + Bevy-integration; RmlUi has cross-engine portability + an open-source heritage.

### vs Buiy

| Axis | RmlUi | Buiy (foundation target) |
|---|---|---|
| Language | C++ | Rust |
| Engine integration | Any C++ embedder | Bevy plugin (parallel to bevy_ui) |
| Authoring | RML + RCSS files | ECS spawn + BSN-friendly components |
| Layout | Own engine, no Grid, no container queries, no anchor positioning | Taffy substrate; Grid + container queries + anchor positioning all in scope |
| Text | FreeType + HarfBuzz sample; no BiDi paragraph algorithm | cosmic-text (harfrust + skrifa + unicode-bidi); BiDi + complex scripts first-class |
| Accessibility | None | AccessKit-first; WCAG 2.2 AA floor |
| Theme tokens | None (CSS-only; custom-properties since 6.0) | Token-based design system (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5) |
| Stylesheet | RCSS file format | Open question (foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5); tokens primary |
| Top layer / dialog | Not supported | Tier **F** |
| Container queries | Not supported | Tier **C** ([`../../plans/2026-05-21-buiy-layout-container-queries.md`](../../plans/2026-05-21-buiy-layout-container-queries.md) landed) |
| OS preferences (reduced-motion, color-scheme, contrast) | Not supported | Automatic via `UserPreferences` resource |
| Render-pipeline scope (filters, masks, top layer, mix-blend-mode) | Filters + masks added 6.0; no `backdrop-filter` / `mix-blend-mode` / top layer | All four committed for v1 |

**Verdict**: Buiy's foundation goals are **substantially more ambitious** than RmlUi's 18-year cumulative feature set, particularly on a11y + container queries + anchor positioning + modern CSS effects. The reasonable engineering scope is reflected in the foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) tier list — `F` and `C` features are what RmlUi struggled with most; `E` and `O` are explicitly cuttable.

## What RmlUi does that other game UI libraries do NOT

A small but worth-naming list:

- **Free, no royalties, MIT-licensed.** The single largest advantage over the proprietary cousins.
- **15+ years of cumulative shipping history.** libRocket + RmlUi is the longest-running open-source HTML/CSS-flavored game UI library, period.
- **Embedder-interface portability.** Works in C++ engines from id Tech derivatives to Cfx.re's GTA-derived runtime to Nightdive's KEX to Unreal plugins.
- **Spatial controller navigation** as a built-in feature (since libRocket era).
- **Decorators** as the visual-customization escape hatch — a flexible extension point that the embedder can grow.

## What RmlUi does NOT do that other game UI libraries DO

- **AAA flagship game**: NoesisGUI / Coherent Gameface dominate.
- **Modern CSS coverage (Grid, container queries, anchor positioning, modern color)**: every other library on this list has more (UI Toolkit / Gameface for Grid, Buiy for all four).
- **Accessibility**: bevy_ui (AccessKit-via-`bevy_a11y` since 0.10), Buiy (AccessKit-first), and web browsers have it; RmlUi has none.
- **Visual editor** (XAML Blend / Unity UI Builder / Unreal UMG): RmlUi has no first-party visual designer; RML / RCSS authors hand-write text files.
- **Integrated devtools**: visual debugging exists (since libRocket) but is far thinner than browser DevTools / Unity Profiler / Unreal's Insights / Buiy's planned `buiy_devtools`.

## Implications for Buiy

- **Buiy occupies a distinct niche** from RmlUi: Bevy-only + Rust-only + AccessKit-first + web-spec-fidelity. The overlap is the *concept* (HTML/CSS-flavored UI in a game engine), not the *implementation* (Rust + ECS + AccessKit vs C++ + embedder + no-a11y).
- **The proprietary cousins (NoesisGUI, Gameface) own AAA**, and they will continue to. Buiy is not trying to displace them in their niche.
- **RmlUi's user list shows the open-source niche is real and durable** — Nightdive, Cfx.re, Unvanquished, ROSE Online — but small. Buiy's market is similarly the open-source game / app niche, plus the Bevy-specific community. Realistic positioning.
- **Unity UI Toolkit's existence** (Unity reinventing the HTML/CSS-flavored UI pattern in-engine with their own UXML + USS) is the strongest validation that the pattern is right for in-engine UI. Buiy's foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1.1 web-platform-parity goal is consistent with the Unity-side data: the industry is converging on HTML/CSS-flavored game UI from multiple directions.
- **No AAA flagship for RmlUi after 15 years** is a sobering data point. Buiy's foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) verification commitment (CI gates + manual release gates, productivity-app fixtures + game scenarios) is the substitute — Buiy will not depend on a flagship-game adoption for proof; it commits to verification harness instead.

## Sources

- RmlUi GitHub README (users list) — https://github.com/mikke89/RmlUi
- NoesisGUI prior-art — [`../noesisgui/README.md`](../noesisgui/README.md), [`../noesisgui/ecosystem-and-comparisons.md`](../noesisgui/ecosystem-and-comparisons.md)
- bevy_ui prior-art — [`../bevy-ui/README.md`](../bevy-ui/README.md), [`../bevy-ui/ecosystem.md`](../bevy-ui/ecosystem.md), [`../bevy-ui/comparisons.md`](../bevy-ui/comparisons.md)
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Cfx.re acquisition by Rockstar (Aug 2023) — public news coverage
- Nightdive Studios KEX engine — Nightdive corporate page
- Coherent Labs Gameface — https://coherent-labs.com/products/coherent-gameface/
- Unity UI Toolkit — https://docs.unity3d.com/Manual/UIElements.html
