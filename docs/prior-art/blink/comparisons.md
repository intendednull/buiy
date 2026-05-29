**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — head-to-head vs Gecko, WebKit, Servo, and Buiy

# Comparisons

This file places Blink next to the other browser engines and against Buiy. Blink is the *canonical* reference implementation of the CSS modules Buiy implements a subset of; Servo/Stylo is the *Rust* reference implementation; Buiy is the consumer that cites both. Each row is a short summary plus the one design difference that matters most for Buiy. Companion to [`critiques.md`](critiques.md) (Blink's costs) and [`open-problems.md`](open-problems.md) (its open structural gaps). Architecture detail is in [`architecture.md`](architecture.md), [`layout.md`](layout.md), and [`style.md`](style.md).

## vs Gecko (Firefox: Stylo + WebRender)

| Aspect | Blink | Gecko |
|---|---|---|
| Language | C++ (Chromium tree) | C++ + Rust (Stylo style engine, WebRender compositor) |
| Style engine | `ComputedStyle` + cascade + invalidation sets | **Stylo** — parallel CSS engine, originally from Servo, in Firefox since 2017 (Quantum CSS / Firefox 57) |
| Compositor | `cc` + Viz property-tree compositor | **WebRender** — GPU display-list compositor, from Servo, in Firefox by 2017+ |
| Layout | LayoutNG (immutable fragment tree) | Gecko's own layout (reflow-based), not the Servo layout engine |
| Stewardship | Google (Chromium) | Mozilla |
| Share | Dominant (Chrome + Edge + Brave + Opera + …) | Minority, declining |

Summary: Gecko is the most architecturally interesting comparison for Buiy because it is the engine that *successfully shipped Rust components into a production C++ browser*. Stylo (parallel style resolution) and WebRender (GPU compositor) were both built in Servo and upstreamed into Firefox; Gecko's *layout* remained its own reflow engine rather than adopting Servo's. **The key design difference: Gecko proves a Rust style/compositor substrate is production-viable inside a real browser.** That existence proof is part of why Buiy's lineage is Rust (Stylo's author, `selectors`/`cssparser`, and Taffy all sit in the same Rust-CSS ecosystem) even though Buiy checks *behavior* against Blink. See [`critiques.md`](critiques.md) on Blink's memory-safety surface.

## vs WebKit (Safari)

| Aspect | Blink | WebKit |
|---|---|---|
| Origin | Forked *from* WebKit's WebCore, 2013-04-03 | The engine Blink forked from |
| Language | C++ | C++ |
| Layout | LayoutNG (immutable fragments) | WebKit's own layout (incl. the in-progress LFC / "Layout Formatting Context" rewrite) |
| Stewardship | Google | Apple |
| Distribution | Chromium open source; embedded via Content API / CEF | WebKit open source; the only engine permitted on iOS pre-2024-EU-DMA |
| Web-compat role | Sets de-facto behavior | Often the *last* engine to ship a feature; the practical compatibility floor |

Summary: Blink *is* a WebKit fork, so the two share deep heritage (the LGPL/BSD licensing in Blink's history comes from WebCore). They have diverged for over a decade. WebKit is frequently the slowest of the three majors to ship CSS features, which makes it the practical "can I use this yet" floor for web authors. **The key design difference for Buiy: WebKit is the *lagging* witness, Blink the *leading* one.** When Buiy implements a CSS feature it can ship it the moment the spec is stable; it is not gated by WebKit's adoption the way a public website is. Buiy uses Blink as the "what does the platform do" reference and does not need WebKit-parity as a constraint.

## vs Servo (Stylo + WebRender, Rust)

| Aspect | Blink | Servo |
|---|---|---|
| Language | C++ | Rust |
| Status | Production, dominant | Research/embedding engine; Mozilla laid off the team in 2020, revived by Igalia under Linux Foundation Europe (Sept 2023) |
| Style | `ComputedStyle` + invalidation sets | **Stylo** (`style` crate) — parallel, the same engine Firefox uses |
| Layout | LayoutNG | Servo's own layout (rebuilt during the 2023+ revival) |
| Goal | Ship the whole web | Embeddable, parallel-first web engine |
| Relationship to Buiy | Behavior reference | *Implementation-technique* reference (Rust CSS lineage) |

Summary: Servo is the Rust reference implementation of the web platform. Its components are battle-tested *both* in Servo and (via Stylo/WebRender) in Firefox, so Servo is not a toy — it is the proof that a parallel, Rust-native CSS engine works. For Buiy this is the most directly relevant comparison after Taffy itself: Servo demonstrates the typed-Rust CSS-semantics approach end-to-end. **The key design difference: scope.** Servo aims to be a *whole* web engine (HTML parsing, scripting via SpiderMonkey, full CSS, networking); Buiy implements a typed-Rust *subset* of CSS layout/style semantics for a game-engine UI plug-in, with no HTML parser and no scripting. Buiy borrows the lineage's *technique* (Rust, parallel-friendly, typed values) without inheriting the whole-browser scope. Servo/Stylo is itself in-corpus prior art — see [../servo-stylo/](../servo-stylo/) for the dedicated folder treating it as the Rust *technique* reference Buiy cross-checks Blink behavior against.

## vs Buiy

| Aspect | Blink | Buiy |
|---|---|---|
| Kind | Whole browser rendering engine | Retained-mode UI plug-in for the Bevy game engine |
| Language / license | C++; Chromium BSD-3-Clause (+ LGPL/MIT/MPL components) | Rust; `MIT OR Apache-2.0` |
| CSS coverage | Essentially all of CSS, bug-for-bug | A typed-Rust **subset**, citing W3C modules (Display 3, Positioned Layout, Containment 3, Writing Modes 4, Anchor Positioning 1) |
| Layout engine | LayoutNG (owns everything) | Taffy (Flexbox + Grid + Block + Float); Buiy adds passes *above* Taffy, never forks it |
| Text | Blink's own text/shaping stack | `cosmic-text` |
| Render | `cc` + Viz (own compositor) | Bevy's `wgpu` render graph |
| A11y | Platform a11y trees computed in-engine | AccessKit-first (WCAG 2.2 AA floor) |
| Resolved style | `ComputedStyle` megastruct (one god object) | Decomposed public-fielded ECS components (NO megacomponents) + hybrid `Style` builder |
| Layout/render contract | Pipeline phases; property trees flow style → paint → composite | **Layout writes, render reads** — render never recomputes stacking/paint order |

Summary: Blink and Buiy are not competitors — Buiy *cites* Blink. Blink is the complete, C++, monolithic reference implementation of the whole web platform; Buiy is a Rust, decomposed, subset implementation of CSS layout/style for Bevy UI. **The key design difference: god object vs decomposition.** Blink concentrates all resolved style in `ComputedStyle` and all layout in LayoutNG; Buiy spreads resolved state across per-entity components (`UiTransform`, `Containment`, the planned `Stacking`) and computes it in an ordered pipeline (`RemovedNodesGc` → `WritingModeInherit` → `SyncStyles` → `CqActivate` → `TaffyCompute` → `CqFlipCheck` → `CqFlipReRun` → `PostTaffyOverrides` → `WriteResolvedLayout`). Where Blink hands the compositor mutable-over-time property trees, Buiy hands render write-once values: Phase 8's `ResolvedTransform` (`M = T·R·S·M_transform`) and Phase 9's planned private `StackingContext { painters_z: Vec<Entity> }`.

## How Buiy's pipeline maps onto Blink's

Blink's verified pipeline stages — Animate, Style, Layout, Pre-paint, Scroll, Paint, Commit, Layerize, Raster, Activate, Aggregate, Draw — collapse, for Buiy's scope, into the layout phase plus Bevy's render graph:

| Blink stage | Buiy equivalent |
|---|---|
| Style | `SyncStyles` (+ `WritingModeInherit` for logical→physical) |
| Layout | `TaffyCompute` + `PostTaffyOverrides` (`6a` sticky, `6b` table stub, `6c` multicol stub, `6d` anchor, `6e` transform-composition, `6f` stacking + top-layer) |
| Pre-paint (property trees: transform / clip / effect / scroll) | Sub-pass `6e` writes `ResolvedTransform`; sub-pass `6f` (Phase 9, next) writes the pre-sorted `StackingContext` paint order + `TopLayer` ordering |
| Paint / Commit / Layerize / Raster / Activate / Aggregate / Draw | Delegated to Bevy's `wgpu` render graph; Buiy only *hands off* resolved data |

The structural contrast: Blink's stages each *mutate* shared structures that later stages read and the compositor keeps re-reading over animation frames; Buiy's `PostTaffyOverrides` sub-passes *finalize* per-entity values that render consumes read-only. Buiy is implementing the *layout half* of Blink's pipeline as a CSS-faithful subset, and explicitly does **not** own the compositor — that is Bevy's. See [`open-problems.md`](open-problems.md) and the Buiy stacking spec [`../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md`](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md).

## Where Blink sits among the references

- **Whole-web, C++, dominant, behavior canon:** Blink.
- **Whole-web, C+++Rust hybrid, minority, proves Rust CSS ships:** Gecko (Stylo + WebRender).
- **Whole-web, C++, lagging, compatibility floor:** WebKit.
- **Whole-web, Rust, embeddable, revived:** Servo (Stylo + WebRender + own layout).
- **CSS-subset, Rust, game-engine plug-in, cites the above:** Buiy (on Taffy — see [`../taffy/`](../taffy/)).

Buiy treats Blink as the canonical *behavior* reference and the Rust lineage (Servo/Stylo, Taffy) as the *technique* reference. The other engine prior-art relevant to Buiy's component-model and integration decisions lives in sibling folders: [`../bevy-ui/`](../bevy-ui/), [`../rmlui/`](../rmlui/) (another C++ HTML/CSS UI engine), [`../coherent-gameface/`](../coherent-gameface/) (a commercial HTML/CSS game-UI engine), [`../dioxus/`](../dioxus/), and [`../xilem-masonry/`](../xilem-masonry/).

## Sources

- Blink forked from WebKit, 2013-04-03 — https://techcrunch.com/2013/04/03/google-forks-webkit-and-launches-blink-its-own-rendering-engine-that-will-soon-power-chrome-and-chromeos/
- Stylo / Quantum CSS in Firefox 57 (2017) — https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/
- WebRender (Servo → Firefox) — https://github.com/servo/webrender
- Servo revival (Igalia, Linux Foundation Europe, Sept 2023) — https://linuxfoundation.eu/newsroom/servo-web-rendering-engine-joins-linux-foundation-europe
- Servo revival 2023–2024 (Igalia) — https://blogs.igalia.com/mrego/servo-revival-2023-2024/
- Servo (software) — https://en.wikipedia.org/wiki/Servo_(software)
- RenderingNG architecture (pipeline stages) — https://developer.chrome.com/docs/chromium/renderingng-architecture
- Chromium `LICENSE` (top-level BSD-3-Clause, Google copyright holder; WebKit-inherited LGPL/MIT/MPL per-file headers) — https://chromium.googlesource.com/chromium/src/+/main/LICENSE
- Buiy layout: stacking + top layer spec — ../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md
- Buiy layout design (folder) — ../../specs/2026-05-08-buiy-layout-design/
- Buiy foundation README — ../../specs/2026-05-07-buiy-foundation/README.md
- Buiy Taffy prior art — ../taffy/
- Buiy Servo/Stylo prior art (the Rust reference implementation) — ../servo-stylo/
