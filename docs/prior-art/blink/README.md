**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium: RenderingNG + LayoutNG + the CSS reference implementation) — folder index and prior-art entry point

# Blink (Chromium)

Blink is the rendering engine of the Chromium browser project — the code path from a parsed DOM to pixels — forked from WebKit's `WebCore` and announced 2013-04-03; its modern architecture (**RenderingNG**, ~2021) is the canonical reference implementation of the CSS modules Buiy implements a typed-Rust subset of (Display 3, Positioned Layout, CSS Containment 3, Writing Modes 4, Anchor Positioning 1). It is not shippable as a focused library: embedding Blink means embedding Chromium, so Buiy draws on it for *semantics and reference behavior*, not source.

## For Buiy

Buiy is a retained-mode UI library for the Bevy game engine, built parallel to `bevy_ui` on a decomposed substrate (Taffy for layout, `cosmic-text` for text, AccessKit for a11y, `wgpu` via Bevy for render) and authored as ECS public-fielded components, never a megacomponent. Blink earns a folder because it is the implementation web authors actually test against — when the W3C module text is ambiguous, "what does the platform do?" almost always resolves to "what Blink does." That makes it the load-bearing *behavior* reference for Buiy's CSS-faithful subset, while the Rust lineage ([../servo-stylo/](../servo-stylo/), [../taffy/](../taffy/)) is the *technique* reference. The single most transferable idea is Blink's hard-won BlinkNG discipline — **each pipeline stage reads the finished, immutable output of the prior stage and never reaches back to mutate it** — which is exactly Buiy's "layout writes, render reads" contract. Blink is the precedent for the stacking + top-layer model Buiy's Phase 9 (the next layout sub-pass, 6f) is about to build, so the [stacking-and-paint.md](stacking-and-paint.md) and [lessons.md](lessons.md) files are written to be directly usable by the Phase 9 plan author.

## Honest assessment

- **It is the reference, but it is a monoculture.** With Edge's 2020 switch to Chromium, the only remaining independent major engines are Gecko (Firefox) and WebKit (Safari). Blink's implementation choices become the de-facto web standard *ahead of or in place of* the W3C spec text. Buiy must cite the W3C modules as the contract and treat Blink as one implementation of them — following the spec, not the bug, where they diverge.
- **Governance is Google-concentrated.** The Blink Intent process (Prototype / Experiment / Ship, three API-owner LGTMs to ship) is transparent and documented, but the API owners are predominantly Google employees, so "three LGTMs" is not cross-vendor consensus. There is no external body with veto over what Blink ships.
- **~70% of Chrome's high-severity security bugs are memory-safety bugs.** A multi-million-line C++ rendering engine cannot be made memory-safe by review alone; mitigations (`MiraclePtr`, heap scanning, `base::span`) are retrofits, and there is no credible plan to rewrite `third_party/blink` in Rust.
- **`ComputedStyle` is a "god object."** Every resolved CSS property lives in one large generated struct; adding a property touches generated base, diff logic, sharing/dedup logic, and field-group placement. The code-generation, rare-data groups, and copy-on-write sharing exist *because* hand-maintaining it became untenable.
- **LayoutNG took roughly a decade and is not visibly "done."** The correct architecture (immutable fragment tree) was understood early; retrofitting it into a live, web-compat-constrained browser — carrying two layout engines in-tree the whole time, with a temporary legacy/NG boundary bug class — is what consumed the years. There is no public "legacy deleted" milestone.
- **What it gets right is what Buiy copies.** The immutable-output discipline, the property-tree separation (animate transform/opacity without re-running layout), the typed *union* of stacking-context triggers, and the top-layer escape-hatch model are all sound and worth borrowing — see [lessons.md](lessons.md).

## Key facts (verified 2026-05-29)

| Fact | Value | Source |
|---|---|---|
| Engine | Blink — Chromium's rendering engine (DOM → pixels) | developer.chrome.com/docs/web-platform/blink |
| Forked from | WebKit's `WebCore`, announced **2013-04-03** | blog.chromium.org (2013), TechCrunch (2013-04-03) |
| Modern architecture | **RenderingNG** (umbrella name; docs ~2021, overview last-updated 2021-06-22) | developer.chrome.com/docs/chromium/renderingng-architecture |
| Document lifecycle | **12 stages**: animate, style, layout, pre-paint, scroll, paint, commit, layerize, raster/decode/paint-worklets, activate, aggregate, draw | RenderingNG architecture page |
| Layout engine | **LayoutNG** — immutable fragment tree; block + inline shipped **Chrome 77 (2019)** | chromium.org/blink/layoutng, developer.chrome.com/docs/chromium/layoutng |
| LayoutNG flex/grid/table | migrated "in subsequent releases" after 77; *exact* NG-enable versions **not pinned** in public sources (preamble's "flex ~Chrome 87" unverified) | history.md, layout.md |
| LayoutNG fragmentation | flex/grid Chrome 103; table Chrome 106 | developer.chrome.com/docs/chromium/renderingng-fragmentation, web.dev/blog/compat2021-midyear |
| Paint property trees | four: **transform / clip / effect / scroll**, computed in pre-paint | RenderingNG architecture page |
| `contain` | four flags layout/paint/size/style; shipped **Chrome 52 (June 2016)**; `content` = layout+paint+style (no size), `strict` = all four | developer.chrome.com/blog/css-containment |
| `content-visibility` | **Chromium 85 (stable 2020-08-25; web.dev article 2020-08-05)** | web.dev/articles/content-visibility |
| Container queries | **Chrome 105, 2022-08-30** (with `:has()`); require containment to avoid layout loops | developer.chrome.com/blog/has-with-cq-m105 |
| Popover API (top layer) | enabled by default **Chrome 114, 2023-05-31** | developer.chrome.com/blog/new-in-chrome-114, chromestatus 5463833265045504 |
| Anchor positioning | **Chrome 125 (rollout 2024-05-14)**; syntax later renamed (`inset-area`→`position-area`, `position-try-options`→`position-try-fallbacks`) in **Chrome 129** | developer.chrome.com/blog/anchor-positioning-api, /blog/anchor-syntax-changes |
| Edge → Chromium | Edge 79, stable **2020-01-15** | blogs.windows.com/msedgedev (2020-01-15) |
| Shared by | Chrome, Edge, Brave, Opera, Vivaldi, Samsung Internet | governance.md |
| License | predominantly **BSD-3-Clause** (Google copyright holder); WebKit-inherited LGPL-2.1/BSD/MIT/MPL per-file | chromium.googlesource.com/.../LICENSE |
| Governance | Google-led inside open-source Chromium; Blink Intent process, three API-owner LGTMs to ship | chromium.org/blink/launching-features |
| Memory-safety bugs | ~70% of Chrome's high-severity bugs | chromium.org/Home/chromium-security/memory-safety |

## Contents

| File | Subject |
|---|---|
| [README.md](README.md) | This index — opening facts, honest assessment, how to use the folder. |
| [lessons.md](lessons.md) | **The decision file.** Validates / Avoid / Borrow for Buiy, Phase-9-ready. Start here when designing. |
| [glossary.md](glossary.md) | Blink/RenderingNG/CSS terms used across the folder, one line each. |
| [architecture.md](architecture.md) | RenderingNG, the 12-stage lifecycle, the Blink/`cc` split, threading, and the "render reads finished data" principle. |
| [layout.md](layout.md) | LayoutNG: the layout input node / constraint space / immutable fragment tree, the cache key, per-mode algorithms, line breaking, the decade-long migration. |
| [stacking-and-paint.md](stacking-and-paint.md) | Stacking-context formation triggers, CSS Appendix-E paint order, the four paint property trees, the top layer. Phase-9-critical. |
| [containment-and-queries.md](containment-and-queries.md) | `contain`, `content-visibility`, container queries (built *on* containment), CSS anchor positioning. |
| [style.md](style.md) | The style engine: `ComputedStyle` megastruct, the cascade, style recalc, invalidation sets, custom properties. |
| [governance.md](governance.md) | The 2013 fork, Google stewardship, the Intent process / API owners, multi-vendor Chromium, BSD-3-Clause, the monoculture concern. |
| [history.md](history.md) | Dated timeline: fork 2013 → LayoutNG 2019 → content-visibility 2020 → container queries 2022 → Popover API 2023 → anchor positioning 2024. |
| [critiques.md](critiques.md) | Honest costs: the monoculture, the C++ monolith, the megastruct, legacy-migration debt, not-a-focused-library. |
| [open-problems.md](open-problems.md) | Forward-looking structural gaps: memory safety, the two-engine boundary generalized, invalidation, cq/layout coupling, stacking machinery, writing modes, governance concentration. |
| [comparisons.md](comparisons.md) | Blink vs Gecko (Stylo+WebRender) / WebKit / Servo / Buiy, plus a pipeline-stage mapping table. |

## How to use this prior-art doc

1. **Designing Phase 9 (stacking + top layer, sub-pass 6f).** Read [lessons.md](lessons.md) first (it is written for the Phase 9 plan author), then [stacking-and-paint.md](stacking-and-paint.md) for the trigger union, Appendix-E paint order, the property-tree split, and the top-layer escape-from-clip model. Cross-check against the Buiy spec [stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md).
2. **Deciding the layout/render contract or pipeline shape.** [architecture.md](architecture.md) (the BlinkNG immutable-output principle, the Blink/`cc` split) and [layout.md](layout.md) (the immutable fragment tree as render-handoff, the constraint-space cache key).
3. **Working on containment, container queries, or anchor positioning.** [containment-and-queries.md](containment-and-queries.md) — the SIZE-zeroing footgun, the containment-requirement loop, `content-visibility`'s intrinsic-size precondition, the anchor-positioning syntax churn. Specs: [transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md), [container-queries-and-writing-modes.md](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md), [display-and-positioning.md](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md).
4. **Defending the decomposed-component model.** [style.md](style.md) (the `ComputedStyle` megastruct and how Blink fights its own size) plus [critiques.md](critiques.md) — the Validates/Avoid contrast lives in [lessons.md § Validates / § Avoid](lessons.md).
5. **Writing a "why this lineage / which spec do we cite" justification.** [governance.md](governance.md) (monoculture, cite the W3C module not the bug) and [comparisons.md](comparisons.md) (Blink = behavior canon, Servo/Stylo = Rust technique reference).
6. **Scoping or risk-assessing a feature.** [open-problems.md](open-problems.md) (what is genuinely unsolved) and [history.md](history.md) (when it shipped, and the multi-year migration cost Buiy avoids by starting clean).
7. **Looking up a term.** [glossary.md](glossary.md).

## Framing disclosure

This corpus is written from Buiy's stated stance: a retained-mode Bevy UI library built **parallel to `bevy_ui`**, implementing a **CSS-faithful typed-Rust subset above Taffy** (Buiy adds anchor positioning, container queries, sticky, writing modes, stacking + top-layer, transforms + containment as passes *above* Taffy, never forking it), with an **AccessKit-first** accessibility posture (WCAG 2.2 AA floor) and an `MIT OR Apache-2.0` license. The "Implications for Buiy" sub-sections in every file reflect that bias by design — they read Blink's choices through Buiy's decomposed-component, layout-writes/render-reads, clean-start-on-Taffy lens, and treat Blink as the canonical *behavior* reference rather than a thing to replicate wholesale. Where this folder says "Buiy diverges," that is a documented design decision, not a claim that Blink is wrong.

## Sources

- What is Blink — https://developer.chrome.com/docs/web-platform/blink
- Blink launch announcement (2013-04-03) — https://blog.chromium.org/2013/04/blink-rendering-engine-for-chromium.html
- RenderingNG architecture (12-stage lifecycle, property trees, Blink/cc split) — https://developer.chrome.com/docs/chromium/renderingng-architecture
- LayoutNG (Chrome 77, fragment tree) — https://www.chromium.org/blink/layoutng/ ; https://developer.chrome.com/docs/chromium/layoutng
- LayoutNG fragmentation (flex/grid 103, table 106) — https://developer.chrome.com/docs/chromium/renderingng-fragmentation
- CSS Containment in Chrome 52 — https://developer.chrome.com/blog/css-containment
- content-visibility (Chromium 85) — https://web.dev/articles/content-visibility
- Container queries land in Chromium 105 — https://developer.chrome.com/blog/has-with-cq-m105
- Popover API enabled by default Chrome 114 — https://developer.chrome.com/blog/new-in-chrome-114/ ; https://chromestatus.com/feature/5463833265045504
- Anchor positioning Chrome 125; syntax changes Chrome 129 — https://developer.chrome.com/blog/anchor-positioning-api ; https://developer.chrome.com/blog/anchor-syntax-changes
- Edge 79 Chromium stable (2020-01-15) — https://blogs.windows.com/msedgedev/2020/01/15/upgrading-new-microsoft-edge-79-chromium/
- Blink launch process / Intents — https://www.chromium.org/blink/launching-features/
- Chromium memory safety (~70% high-severity bugs) — https://www.chromium.org/Home/chromium-security/memory-safety/
- Chromium LICENSE (BSD-3-Clause) — https://chromium.googlesource.com/chromium/src/+/main/LICENSE
- Sibling files: [lessons.md](lessons.md), [glossary.md](glossary.md), [architecture.md](architecture.md), [layout.md](layout.md), [stacking-and-paint.md](stacking-and-paint.md), [containment-and-queries.md](containment-and-queries.md), [style.md](style.md), [governance.md](governance.md), [history.md](history.md), [critiques.md](critiques.md), [open-problems.md](open-problems.md), [comparisons.md](comparisons.md)
- Sibling prior art: [../taffy/](../taffy/), [../servo-stylo/](../servo-stylo/), [../bevy-ui/](../bevy-ui/), [../bevy-flair/](../bevy-flair/), [../coherent-gameface/](../coherent-gameface/), [../rmlui/](../rmlui/), [../dioxus/](../dioxus/)
- Buiy specs: [stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md), [transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md), [architecture.md](../../specs/2026-05-08-buiy-layout-design/architecture.md), [container-queries-and-writing-modes.md](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md), [display-and-positioning.md](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md), [foundation README](../../specs/2026-05-07-buiy-foundation/README.md)
