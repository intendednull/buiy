**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — critiques, open problems, structural limitations

# Critiques & open problems

NoesisGUI is a successful commercial product, but it carries genuine structural limitations that matter for any open-ecosystem alternative (like Buiy) thinking about how *not* to repeat the same shape. This file separates **critiques** (problems with NoesisGUI as a product) from **open problems** (things that NoesisGUI structurally does not solve and probably will not, in its current form).

## Critiques

### 1. Proprietary lock-in

Noesis is closed-source. Customers who haven't paid for source-code access (which is most INDIE-tier customers, and many PRO customers) are entirely dependent on Noesis Technologies S.L. for:

- Engine version compatibility (Unity / Unreal / Xcode / VS bumps).
- Console SDK updates.
- Bug fixes for issues blocking shipping.
- Long-term platform support past 2030.

The mitigation is buying the source-code add-on. This costs extra, and the indie tier doesn't offer it at any price. **A studio shipping on Noesis without source-code access has accepted a structural single-vendor risk** that has no equivalent in open-source alternatives.

For Buiy, this is the strongest "do the opposite" lesson: Buiy is MIT/Apache open source, and that commitment is non-negotiable.

### 2. XAML's age and learning curve

XAML was designed for WPF in 2006. The language is 20 years old in 2026; the design idioms (dependency properties, attached properties, markup extensions, styles + templates + resources + triggers) carry the weight of a decade of pre-React UI thinking. Forum users describe the learning curve as steep:

> *"NoesisGUI has a steep learning curve, and XAML has intricacies that will massively bite you in the butt unless you really know what you're doing."*
>
> — from various forum / community posts ([gamefromscratch](https://gamefromscratch.com/noesisgui-hands-on-with-the-game-user-interface-framework/), Noesis forum threads).

The standard workflow involves **Microsoft Blend for Visual Studio** as the WYSIWYG editor — and Blend is **Windows-only**. macOS-based developers face workflow friction: either VM Windows + Blend, or use the Noesis Studio beta. **The standard tooling chain is not cross-platform**, despite the runtime being cross-platform.

For Buiy, the lesson is the inverse: **the authoring tooling must match the runtime's platform reach**. BSN files are text; any editor can author them. Hot-reload works without specialised editor software. Noesis Studio's WYSIWYG path is a feature Buiy would benefit from eventually but is not load-bearing on day one.

### 3. Accessibility is absent

Searches for "NoesisGUI accessibility" / "NoesisGUI screen reader" return essentially zero relevant results. The features page does not mention accessibility. There is no AccessKit integration, no UIA bridge, no NVDA / VoiceOver / TalkBack / Narrator hookup. The framework does not produce an accessibility tree.

This is consistent with the broader game-UI middleware market — Coherent Gameface, UGUI, Slate, UMG all lag behind productivity-app accessibility. But it is **structurally different from Buiy's commitment**: Buiy treats AccessKit as foundation-tier (foundation [§ 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)), not as a future add-on.

The absence of a11y in Noesis is the single largest gap between Noesis and what Buiy aims to be. The market evidence (Noesis ships in AAA games without a11y) suggests **a11y is not a hard customer requirement for AAA games today**. Buiy is making a bet that this changes, driven by:

- The European Accessibility Act (effective June 2025).
- WCAG 2.2 as the floor.
- Game industry maturity — accessibility scoring has appeared in mainstream press (Wired, Polygon's annual accessibility reviews).
- Productivity-app use of Buiy (Buiy aims at both games and apps).

NoesisGUI's a11y gap is **the canonical case study for Buiy's "AccessKit-first or not at all" stance**.

### 4. Per-engine binding maintenance burden

The cross-engine value proposition has a structural cost: every Unity / Unreal / platform-SDK update requires Noesis to ship a corresponding plugin update. The 3.2.x patch release rate (one every 2-3 months across 2024-2026) is dominated by:

- Unity minor support bumps (2020.2, 2021.x, 2022.x, 2023.x, 6.0, 6.1, 6.3, 6.4).
- Unreal minor support bumps (4.27, 5.0 through 5.7).
- Xcode bumps (every WWDC).
- Visual Studio bumps (every spring / fall).
- Per-console SDK bumps (each platform holder cadence).

This is the **tax of multi-engine portability**, and Noesis Technologies pays it on their customers' behalf. **For Buiy, by committing to Bevy-only, Buiy explicitly does not pay this tax** — but also explicitly cannot serve customers who want engine portability.

### 5. Asset Store version drift

The Unity Asset Store version of NoesisGUI is reportedly outdated; users have to download the newest version from the official site as an independent package. This is a recurring complaint:

> *"The version in the Unity Asset Store is outdated, requiring users to download the latest version from the official website and install it as an independent package, which is still inconvenient for Unity users."*

This is a workflow / distribution issue specific to Unity's Asset Store rather than Noesis's runtime. For Buiy, the lesson is: **distribute via the ecosystem's primary package manager (crates.io for Buiy) and keep it up to date**. Bevy doesn't have an asset-store equivalent; this concrete issue won't repeat. But the abstraction — keep your primary distribution channel current — is worth carrying forward.

### 6. WYSIWYG ergonomics still maturing

Noesis Studio (the next-gen visual editor) has been "Coming 2024" in beta for an extended period. The mature tooling story for Noesis remains "use Blend for Visual Studio on Windows + write XAML by hand." For a 13-year-old product, the WYSIWYG tooling is less mature than competitors (UI Toolkit Editor in Unity; UMG editor in Unreal; web devtools for Gameface).

Buiy's tooling story is currently *worse* than Noesis (Buiy is younger; no Studio-equivalent yet). The foundation spec lists `buiy-devtools-design` as a sub-spec, with a Studio-like authoring tool implied but not committed. The Noesis precedent — that even a 13-year-old product struggles to ship Studio-quality WYSIWYG — should set realistic expectations for Buiy's tooling timeline.

## Open problems

### A. Modern web-platform features

XAML predates and structurally cannot easily acquire modern web-platform features:

- **Container queries** (CSS, 2023): a XAML element's size is *known* by its layout container, but there is no XAML idiom for "style this element based on its parent's resolved size." Buiy commits to container queries (foundation [§ 3.x cross-cutting](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).
- **Anchor positioning** (CSS, 2024): position an element relative to an anchor element somewhere else in the tree. XAML has popups that anchor to other elements, but no general "position B relative to A" primitive across the tree.
- **View transitions** (CSS, 2024+): animate layout changes between two states of the DOM. XAML's animation model is property-by-property; there is no analog for view transitions.
- **Scroll-driven animations** (CSS, 2024+): animations driven by scroll position rather than time. XAML has scroll events; integrating them with animations is manual.

NoesisGUI may absorb these features over time, but the XAML grammar is a structural constraint — extending XAML to express container-query selectors or anchor-positioning relationships is a non-trivial language extension. Buiy starts from a clean slate and can express these naturally in component shapes.

### B. AccessKit-equivalent accessibility tree

There is no AccessKit-equivalent in Noesis. To add one would mean:

- Producing an accessibility tree alongside the visual tree.
- Mapping XAML control types to ARIA roles / states / properties (XAML has its own UI Automation framework, but it's Windows-only).
- Integrating with platform AT bridges (UIA on Windows, AX on macOS, AT-SPI on Linux, UIAccessibility on iOS, AccessibilityNodeInfo on Android).
- Wiring keyboard contracts to APG patterns.

This is feasible work but Noesis has not announced any roadmap toward it. For Buiy, **the absence of a Noesis-equivalent a11y story is the open problem the foundation spec exists to solve**.

### C. WCAG conformance

No public WCAG 2.x conformance claim is made by Noesis. Conformance would require:

- WCAG 2.2 AA pass for the default theme (contrast ratios, focus visibility, focus order, error identification, etc.).
- APG keyboard contracts on every widget.
- Documented conformance reporting (VPAT / ACR).

Noesis ships widgets but does not document them as WCAG-conformant. Buiy commits to WCAG 2.2 AA as the floor (foundation goal 2).

### D. Pricing transparency at enterprise tier

Premium tier prices are public; Enterprise tier requires contact. The €12M+ project budget tier is opaque. This is normal for B2B sales but is a Noesis-specific friction point — small Premium-eligible studios get clean pricing; ambitious Enterprise-eligible projects face a sales conversation. Buiy has no analog (open source).

### E. Multi-window / multi-display app patterns

Noesis is designed primarily for one fullscreen game UI. Multi-window productivity-app patterns (DAW-style detachable panels, multi-display IDE windows, OS-window-per-document) are not the framework's focus. The integration model (one `NoesisView` per UMG widget / one `NoesisView` per Unity camera) implies a one-render-target-per-view shape that doesn't naturally extend to many-windows-of-the-same-app.

For Buiy, which aims at "Game and app, both" (foundation goal 6), multi-window is in scope (foundation [§ window-and-surface sub-spec](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)). Noesis doesn't offer a model here.

### F. Web target

Noesis supports WebGL as a render backend. The XAML runtime itself runs in the browser. But "web target" in the modern sense — DOM accessibility, browser-native form controls, browser devtools, browser-native screen-reader bridge — is not Noesis's path. It's a WebGL canvas with all the implications (no native a11y, no native text input integration, no browser-zoom respect, no native clipboard).

For Buiy this matches Buiy's WASM-target open question (foundation [§ 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)): a Bevy WASM target works for visuals but lacks the browser-native a11y bridge until AccessKit ships its web adapter.

### G. The "AI prompting" forum thread

A 2024-2025 forum thread (linked in critiques search results) discusses using AI prompting to generate XAML. The framing — *"AI-generated XAML code can end up having little bugs or inefficiencies that will cost more time fixing than learning how to do it yourself"* — surfaces a related tension: the maturity of AI tooling for XAML is shallow compared to AI tooling for HTML/CSS/JS, simply because XAML's training-data presence in LLMs is much smaller than the web platform's. **An open ecosystem with millions of HTML examples online benefits from LLM-assisted authoring in a way that XAML structurally does not.** Buiy's BSN format faces the same risk (small training-data corpus), but BSN's similarity to ECS Rust spawning patterns means Buiy benefits from Rust + Bevy LLM-trainability.

## Implication for Buiy

The Noesis critique catalogue maps fairly cleanly onto Buiy's foundation commitments — most of the things Noesis *doesn't* solve are things Buiy explicitly commits to solving:

| Noesis critique | Buiy commitment |
|---|---|
| Proprietary lock-in | MIT/Apache dual license |
| XAML learning curve | BSN matches ECS spawning idiom; familiar to Bevy users |
| Windows-only WYSIWYG (Blend) | BSN is text; any editor works; devtools sub-spec |
| No accessibility | AccessKit-first ([§ 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)) |
| Per-engine binding overhead | Bevy-only; structurally avoids |
| No container queries / anchor positioning | Foundation cross-cutting spec includes |
| No WCAG conformance claim | Foundation [§ goal 2](../../specs/2026-05-07-buiy-foundation/README.md) commits to WCAG 2.2 AA |
| No multi-window app patterns | Foundation window-and-surface sub-spec |
| No browser-native a11y | AccessKit web-adapter dependency (foundation [§ 5 open](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)) |

The lesson: **NoesisGUI is the existing-art for what AAA studios will accept today**; Buiy is positioning itself for what AAA studios will need by 2028-2030. The gap between those two product surfaces is what justifies Buiy's existence in the open-source / Bevy-only / a11y-first niche.

## Sources

- NoesisGUI features page — https://www.noesisengine.com/noesisgui/
- WPF / UWP comparison — https://www.noesisengine.com/docs/Gui.Core.WPFComparison.html
- GameFromScratch hands-on review — https://gamefromscratch.com/noesisgui-hands-on-with-the-game-user-interface-framework/
- Noesis forum (XAML / AI prompting threads) — https://www.noesisengine.com/forums/
- Unity Asset Store reviews — https://assetstore.unity.com/packages/tools/gui/noesisgui-2-2-9282/reviews
- Buiy foundation architecture § 2.6 (AccessKit) — ../../specs/2026-05-07-buiy-foundation/architecture.md
- Buiy foundation goals — ../../specs/2026-05-07-buiy-foundation/README.md
- European Accessibility Act (EN 301 549) — https://www.etsi.org/standards-search#search=EN%20301%20549
