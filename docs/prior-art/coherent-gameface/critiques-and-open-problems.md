**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — critiques and open problems

# Critiques and open problems

Each section below is a structural or substantive critique of Coherent Gameface as it ships today. Some are intrinsic trade-offs of the commercial-proprietary-HTML-engine model; others are catch-up work where the web platform has moved faster than Cohtml.

## 1. Proprietary lock-in

Coherent ships its core engine (Cohtml + Renoir) **closed-source under a commercial license**. Customers receive:

- Compiled libraries for their target platforms.
- Source-code escrow language in higher-tier contracts (standard for AAA middleware).
- Quarterly updates and LTS support tied to the active license.
- Per-engine plugin source typically delivered, but not redistributable.

The lock-in implications for a shipping AAA title:

- **Engine version migration depends on Coherent's release schedule.** UE 5.5 → UE 5.6 means waiting for Coherent's UE 5.6-compatible plugin.
- **Console SDK migrations depend on Coherent's per-platform builds.** PS5 → PS5 Pro adds NDA'd platform work that only Coherent can ship.
- **Bug fixes flow through Coherent's support channel.** If a renderer bug blocks a release, the studio doesn't have the source to fix it themselves (in most license tiers).
- **Long-term viability is single-vendor.** Coherent EOL'd Coherent UI in 2017. The same could happen to Gameface. AAA studios shipping on Cohtml have a multi-year-asset exposure (UI assets in HTML+CSS+JS that target Cohtml's specific implementation).

**Buiy mitigation**: MIT-OR-Apache-2.0 dual permissive. Source is the norm. Bevy ecosystem prevents single-vendor risk by design. **Hard constraint, not negotiable.**

## 2. HTML5-engine maintenance burden

Coherent's central commercial value is "we maintain an in-house HTML engine so you don't have to." That value comes with **a permanent and growing engineering bill**:

- The web platform spec advances every year. Container queries (2022–2023), `:has()` (2023), CSS Nesting (2023), anchor positioning (2024), `oklch()` (2023), view transitions (2024), scroll-driven animations (2024). Each of these is a major Chromium / WebKit / Gecko effort; Cohtml must implement or fall behind.
- Cohtml currently does **not** support: container queries, anchor positioning, native CSS Grid (uses a custom JS element instead), modern color spaces (status unclear), variable fonts (status unclear), `:has()`, CSS Nesting, view transitions, scroll-driven animations.
- The gap between "what Cohtml ships" and "what Chromium ships" is several years. This gap is **structurally permanent** — Coherent ~50–100 engineers cannot outpace Chromium ~thousands of engineers on web-platform spec absorption.
- JavaScript engine work: V8 + alternate VM. V8 is open-source but redistribution on consoles requires platform-specific build infrastructure Coherent maintains. The alternate VM (for platforms where V8 cannot be shipped) has its own maintenance bill.

**Buiy mitigation**: don't ship an HTML engine. Reuse Taffy (Flexbox + Grid + Subgrid + Block + Float, maintained by DioxusLabs + Nico Burns), cosmic-text (harfrust + skrifa + unicode-bidi, maintained by System76), AccessKit (Pneuma Solutions), wgpu via Bevy. The maintenance bill stays on the upstream communities; Buiy owns the integration + widgets + a11y bridge + render pipeline. See foundation [`architecture.md` § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly).

## 3. Accessibility: in-process TTS, not OS-AT

Cohtml's accessibility story is the **`CohtmlARIA*` JS plugin family** + a JavaScript SpeechAPI library:

- `CohtmlARIAHoverReadPlugin` — speak hovered element's `aria-label`.
- `CohtmlARIAFocusChangePlugin` — speak focused element's content on focus change.
- `CohtmlARIALiveRegionsPlugin` — speak ARIA live region changes.
- A `CohtmlARIAManager` to coordinate plugins, observe a DOM subtree.

This is **in-process TTS via a JS-side speech library**, not an OS-AT bridge. The implications:

- **No NVDA / VoiceOver / JAWS / TalkBack integration.** A user with their preferred screen reader running gets *nothing* from a Coherent-rendered UI — Cohtml does not expose an accessibility tree to the OS accessibility APIs (UIA on Windows, AT-SPI on Linux, AXAPI on macOS, AccessibilityNodeInfo on Android, UI Accessibility on iOS).
- **The user's preferred voice, speed, verbosity, pronunciation dictionary don't apply.** Cohtml's TTS uses the system speech synthesizer the JS library reaches, with default settings.
- **No braille display support.** Braille flows through screen readers; Cohtml bypasses screen readers.
- **No magnifier integration, no high-contrast theme integration, no system-zoom integration.** All of these depend on the OS-AT model.

Coherent's positioning is "we let you build accessibility yourself by hooking the ARIA plugins." That offloads the substance to the studio. AAA studios have accessibility teams; mid-tier and indie typically don't.

**Buiy mitigation**: AccessKit-first (foundation [`architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)). Buiy builds AccessKit trees from decomposed components; AccessKit handles OS-AT bridges on Windows / macOS / Linux / Android / iOS. Every widget ships with APG keyboard contract + accessible name/role/value + focus management. **AccessKit-first is one of Buiy's largest differentiators against Coherent (and against NoesisGUI, which has no a11y story at all).**

## 4. Pricing opacity

Coherent's pricing is **quote-based with no public tiers**. Compare to NoesisGUI's explicit tiered pricing (Indie €195 / Pro €9K / Premium €18K, with revenue + budget thresholds documented publicly). For prospective customers:

- **You cannot budget without engaging sales.** Forecasting tools, comparison shopping, and ROI analysis all require sales engagement.
- **Indies / mid-tier / open-source projects are de facto excluded.** "Special pricing for indies on request" puts every indie through a sales motion they often can't justify the effort for.
- **Educational + research use is unclear.** No academic license tier publicly documented.
- **The "evaluate via 30-day trial" model gates extended evaluation behind continued license cost.** Bigger projects need more than 30 days to assess fit.

The trade-off Coherent makes is **high-touch sales for high-value customers**. The opportunity cost is the long-tail discoverability and adoption among indies / smaller studios / open-source contributors.

**Buiy mitigation**: foundation [`README.md` non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals) explicitly excludes commercial pricing. crates.io distribution. Zero license fee. The discoverability + frictionless-adoption upside is what offsets the lack of hand-holding.

## 5. Modern CSS gap

Specifically missing or status-unclear in Cohtml's current shipping surface (as of 2026):

| CSS feature | Cohtml status | Web-platform availability |
|---|---|---|
| **Native CSS Grid** | Not native (custom JS element) | Stable since 2017 in all browsers |
| **Subgrid** | Not supported | Stable in Firefox/Safari since 2019/2023; Chrome since 2023 |
| **Container queries** | Not supported | Stable in all browsers since 2023 |
| **Anchor positioning** | Not supported | Chrome stable 2024; behind flag in other browsers |
| **`:has()` selector** | Status unclear | Stable in all browsers since 2023 |
| **CSS Nesting** | Status unclear | Stable in all browsers since 2023 |
| **Modern color (`oklch`, `oklab`, `color-mix`, wide-gamut `color()`)** | Status unclear | Stable in all browsers since ~2023 |
| **View transitions API** | Status unclear | Chrome stable 2024 |
| **Scroll-driven animations** | Status unclear | Chrome stable 2024; behind flag elsewhere |
| **Logical properties** | Status unclear | Stable in all browsers since ~2019–2021 |
| **Variable fonts** | Status unclear | Stable in all browsers since ~2017 |
| **Color fonts (`COLR/CPAL`, emoji)** | Status unclear | Stable in all browsers |

The status-unclear items might be partially supported — Coherent docs are paywalled / gated for some pages and the public marketing doesn't enumerate them.

**Buiy mitigation**: foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) tier list. The web platform feature catalog is the master list; modern features (container queries, anchor positioning, modern color, variable fonts, etc.) are explicitly in scope at tier **F** or **C**. Layout's already-landed plan `2026-05-09-buiy-layout-grid.md` ships native Grid via Taffy.

## 6. Custom JS-element for Grid: divergence cost

`coherent-gameface-grid` and `coherent-gameface-automatic-grid` ship as **JavaScript-implemented custom HTML elements** (Web Components). Authors write:

```html
<gameface-grid columns="3">
  <gameface-grid-cell>...</gameface-grid-cell>
</gameface-grid>
```

rather than CSS Grid's:

```css
.container { display: grid; grid-template-columns: repeat(3, 1fr); }
```

The problems:

- **Standard tooling doesn't apply.** Stylelint, IDE CSS validators, MDN reference, browser devtools' grid inspector — none of these understand `<gameface-grid>` semantics. Authors get less editor support.
- **The "develop in Chrome" pitch breaks.** Coherent's marketing claims "develop your UI in Chrome, ship in Gameface." But `<gameface-grid>` is a Cohtml-specific element; testing Grid layouts in Chrome requires loading Coherent's JS library and depending on its behavior matching Cohtml's. Cross-engine portability is reduced.
- **Performance overhead.** A custom element implementation goes through JS lifecycle hooks, layout via the JS layer, observer-driven re-layout. Native CSS Grid is a C++ layout pass.
- **Permanent ecosystem divergence.** Game projects that adopt `<gameface-grid>` are tied to Coherent's specific element naming + attribute model; if the project later wants to switch to a native CSS Grid implementation (Buiy, RmlUi, Chromium), the HTML markup must be ported.

This is the same pitfall RmlUi commits with its `decorator:` syntax (replacing CSS `background-image`) — see [`rmlui/lessons.md`](../rmlui/lessons.md) Avoid row "Custom DSL deviation from web spec." **Both Coherent and RmlUi pay this cost; Buiy's commitment is to track CSS-spec property names + semantics rather than invent equivalents.**

## 7. The "develop in Chrome, ship in Gameface" pitch has caveats

Coherent's marketing repeatedly says "use Chrome devtools to develop, use standard web frameworks, write standard HTML/CSS/JS." The reality:

- Cohtml's Flexbox has documented divergences from spec (all elements default to flex-column-ordering per some docs).
- `calc()` is not supported inside `@keyframes`.
- Mixed-unit `calc()` (e.g., `50% - 20px`) doesn't work.
- The HTML element coverage is a subset (no `<iframe>`, no `<input type="file">`, no `<form action>` URL submission, etc.).
- The JS engine is V8 on tier-1 platforms, an alternate VM elsewhere — same code may not behave identically across platforms.
- The networking / storage APIs aren't there.
- The OS-AT bridge isn't there.

The pitch is **directionally true** (you can develop game UI with HTML/CSS/JS habits) but **not literally true** (it's not Chrome; spot-checking in Chrome doesn't guarantee Cohtml behavior). The friction shows up at integration time and in late-stage QA.

## 8. AAA-specific tooling, indie-friction tooling

The pre-sales-engagement model means **Coherent staff fly out to onboarding sessions** with AAA studios. That's good service for the customers it targets; it's a **non-starter for indies and mid-tier**.

By contrast:

- crates.io + `cargo add buiy` works for any developer who can install Rust.
- Open-source means anyone can read the source, file an issue, submit a PR.
- BSN authoring lives in any text editor.
- Bevy's example-driven docs lower the onboarding cost.

The "high-touch sales for AAA" model is a fundamentally different go-to-market than "open-source ecosystem for everyone." Coherent serves one segment well; Buiy targets a different segment.

## 9. Pricing inflation pressure under modern game-industry economics

Game-industry 2023–2025 saw a wave of layoffs. Studios are pressure-tested on costs. Middleware that costs a high four-figure or five-figure license per platform per project becomes scrutinized.

- AAA studios will pay if Coherent demonstrably saves more than its cost (faster iteration, animator workflow, time saved vs in-house build).
- Mid-tier studios will look harder at open-source alternatives.
- Indies look at $0 alternatives by default.

The pricing-opacity + AAA-targeting strategy is durable as long as the AAA market remains willing to pay. The risk: a high-quality MIT alternative reaches "good enough for game UI" and erodes the floor. **RmlUi is doing this on the C++ side at indie/AA scale; Buiy aims to do this on the Bevy side.**

## 10. No public roadmap of CSS-spec absorption

Coherent's public roadmap (per Zendesk's article "Public roadmap") tracks per-product version features but does not commit specifically to "container queries by version X, anchor positioning by version Y, modern color by version Z." Customers have to ask sales for spec absorption forecasts.

For a studio evaluating Cohtml *as the UI platform for a 3-year project*, this is consequential — they can't predict which 2026/2027 CSS features will be available in the engine they ship on in 2028.

**Buiy mitigation**: foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) tier list **is** the roadmap. Every web-platform feature is listed with **F/C/E/O** classification. Sub-spec roadmap in foundation [`README.md § 4`](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) maps features to their owning sub-spec. Public, auditable, dated.

## Open problems

The list of **structurally unresolved problems Coherent is grappling with** (or that the public docs surface as ongoing work):

1. **Container queries** — required by every modern responsive-UI pattern. Status: not in current docs as of survey.
2. **Anchor positioning** — the modern replacement for `position: absolute` + JS math for popovers, tooltips, dropdowns. Status: not in current docs.
3. **CSS Nesting** — author-ergonomic improvement that authors increasingly expect. Status: unclear.
4. **`:has()`** — author-ergonomic and feature-rich; lots of UIs depend on it now. Status: unclear.
5. **Modern color + wide-gamut** — required for HDR-aware UIs and accurate color in modern displays. Status: unclear.
6. **Variable fonts** — required for modern typography. Status: unclear.
7. **View transitions API** — game-UI menus and scene changes increasingly use this. Status: unclear.
8. **Scroll-driven animations** — useful for HUD/loading-screen scroll effects. Status: unclear.
9. **OS-AT bridge** — moving from in-process TTS to actual UIA / AT-SPI / AXAPI tree publication. Status: not on the public roadmap.
10. **Variable-cost-cohort sales** — opening up an indie / open-source / educational tier with predictable pricing. Status: not on the public roadmap.

## Implications for Buiy

Every critique above is a place where Buiy's design decisions are **directly informed by what Coherent does and doesn't do**:

- Coherent's proprietary-lock-in → Buiy's MIT/Apache.
- Coherent's HTML-engine-maintenance-burden → Buiy's "no HTML engine" (reuse Taffy + cosmic-text).
- Coherent's in-process TTS a11y → Buiy's AccessKit-first OS-AT-bridged a11y.
- Coherent's pricing opacity → Buiy's crates.io free distribution.
- Coherent's modern-CSS gap → Buiy's foundation-spec target of web-platform parity including modern features.
- Coherent's custom-JS-element divergence → Buiy's CSS-spec-name-and-semantic-fidelity commitment.
- Coherent's "Chrome dev, ship Cohtml" caveats → Buiy doesn't make that promise (BSN is BSN, not HTML).
- Coherent's AAA-only tooling → Buiy's discoverability via Bevy + cargo ecosystem.
- Coherent's no-public-spec-roadmap → Buiy's foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) tier list is the public, auditable roadmap.

The pattern: **Coherent's design choices and gaps are the negative space that Buiy's foundation spec fills positively.** [`lessons.md`](lessons.md) synthesizes this into the validates / avoid / borrow decision file.

## Sources

- Coherent Labs FAQ — https://coherent-labs.com/frequently-asked-questions/
- Differences-to-traditional-browsers — https://docs.coherent-labs.com/cpp-gameface/what_is_gfp/htmlfeaturesupport/
- Gameface CSS Properties reference — https://docs.coherent-labs.com/cpp-gameface/content_development/supported_features_tables/cssproperties/
- TextToSpeech / ARIA docs — https://docs.coherent-labs.com/cpp-gameface/integration/optional_features/texttospeech/
- Coherent CEF critique post — https://coherent-labs.com/posts/what-developers-should-consider-when-using-chromium-embedded-framework-cef-in-their-games/
- Coherent public roadmap article — https://coherentlabs.zendesk.com/hc/en-us/articles/7957306751261-Public-roadmap
- GameUIComponents OSS repo (custom-element grid implementations) — https://github.com/CoherentLabs/GameUIComponents
- Buiy foundation visuals (tier list / roadmap) — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Sibling prior-art: [`../rmlui/critiques-and-open-problems.md`](../rmlui/critiques-and-open-problems.md), [`../noesisgui/critiques-and-open-problems.md`](../noesisgui/critiques-and-open-problems.md)
