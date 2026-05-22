**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — Validates / Avoid / Borrow decisions for the Buiy foundation

# Lessons for Buiy

This is the consult-this-when-designing decision file. The other files in this corpus are evidence; this file is the synthesis. Three sections — **Validates** (Buiy choices NoesisGUI's experience confirms), **Avoid** (pitfalls to mitigate), **Borrow** (primitives worth studying).

NoesisGUI is the canonical existing-art for **"proprietary commercial cross-engine UI middleware shipped in AAA games"** — directly relevant to Buiy's foundation [goal 1 (comprehensive UI library scope)](../../specs/2026-05-07-buiy-foundation/README.md#buiys-goals-the-product) and [goal 6 (game and app, both)](../../specs/2026-05-07-buiy-foundation/README.md#buiys-goals-the-product). The XAML lineage is also relevant — XAML predates web-platform parity, but the declarative pattern is conceptually similar to BSN.

## Validates

These Buiy design choices are confirmed by NoesisGUI's market evidence:

- **A custom-render-pipeline GPU vector UI works in AAA.** Baldur's Gate 3, Hellblade 2, TopSpin 2K25, Age of Wonders 4 all ship on NoesisGUI's GPU-tessellated vector renderer. Buiy's foundation [§ 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns) commits to a custom Bevy render-graph pipeline; Noesis is the production-scale existence proof. See [`rendering-and-performance.md`](rendering-and-performance.md).
- **A complete UI library + engine integration is a viable product surface.** Studios buy Noesis instead of building their own UI tooling. The implication for Buiy: a comprehensive open-source UI library for Bevy serves a real need, even though bevy_ui exists. The market evidence is that AAA studios specifically opt *into* third-party UI middleware over engine-native UI when it offers comprehensive features + tooling. See [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md).
- **Declarative reflection-driven markup above ECS works.** Noesis's XAML is reflection-driven (every property is a registered `DependencyProperty` queryable from markup). BSN ([PR #20158](https://github.com/bevyengine/bevy/pull/20158)) is reflection-driven (every component is `Reflect`-registered, queryable from `.bsn` assets). The pattern of "declarative tree, reflection-resolved property values, hot-reloadable" is proven at AAA scale.
- **MVVM separation pays off for complex UIs.** Larian's testimonial on Baldur's Gate 3 specifically called out MVVM as the reason they chose Noesis. Buiy's reactivity (foundation [§ 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md#27-reactivity), open for v1 signals layer) should preserve a clean ViewModel-equivalent separation — Bevy resources / observed components on the ViewModel side, render components on the View side.
- **Console support matters.** Noesis ships per-console SDK builds; AAA studios buy specifically for the console-platform support coverage. Buiy's foundation defers console support (see [§ 2.9 platform support staged](../../specs/2026-05-07-buiy-foundation/architecture.md#29-compatibility--policy)); the lesson is *this is real customer demand*, even though Buiy's open-source posture means it can't ship NDA'd console code itself.
- **Per-frame tessellation, no retained caching at geometry level.** Noesis re-tessellates per frame because game UIs change every frame. Bevy + Buiy will have the same pattern naturally — render passes extract per-frame, no special caching layer needed. See [`rendering-and-performance.md`](rendering-and-performance.md).
- **A `RenderDevice`-style boundary lets the runtime ship across many graphics APIs.** Noesis abstracts over D3D11/12, Metal, Vulkan, OpenGL. Buiy delegates this to wgpu (which is one layer up from Noesis's abstraction) — and so doesn't need its own `RenderDevice`. The lesson is that the *seam* is right; the *level* differs because Buiy has Bevy's wgpu integration available.

## Avoid

Pitfalls drawn from NoesisGUI's posture, with Buiy's mitigation:

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Proprietary lock-in** — NoesisGUI is closed-source; non-source-add-on customers face single-vendor risk on engine bumps, console SDKs, bug fixes, long-term viability. | [`distribution-and-governance.md`](distribution-and-governance.md) § "Long-term viability"; [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Proprietary lock-in." | Buiy is MIT/Apache dual-licensed. Source is the norm. The Bevy ecosystem prevents single-vendor risk by design. **Hard constraint, not negotiable.** |
| **No accessibility at all** — Noesis ships in AAA games without an accessibility tree, screen-reader integration, ARIA / WCAG, or keyboard contracts. Market evidence says AAA tolerates this today; trajectory suggests this changes by 2028-2030. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Accessibility is absent." | AccessKit-first ([foundation § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)). Every widget ships with APG keyboard contract + accessible name/role/value + focus management + AccessKit tree wiring. **Buiy's largest single product differentiator.** |
| **Per-engine binding tax** — every Unity / Unreal minor release means a Noesis 3.2.x patch. The 3.2.x cadence is dominated by engine-bump work, not core-framework work. | [`engine-integration.md`](engine-integration.md) § "Comparison: per-engine binding overhead." | Bevy-only ([foundation README non-goal "Non-Bevy frontends"](../../specs/2026-05-07-buiy-foundation/README.md#non-goals)). Each Bevy minor is a Buiy migration event — *one* migration event, not three (Unity + Unreal + Xcode + VS + each console). |
| **Dependency-property runtime cost** — every XAML property access is a dictionary lookup + value-precedence resolution; the system is general and uniform but pays per-access overhead. | [`xaml-paradigm.md`](xaml-paradigm.md) § "Dependency properties." | Bevy ECS components are typed and direct. **Don't recreate dependency properties for the sake of uniformity.** Property inheritance is opt-in (theme tokens already provide it for the common case). |
| **Megaclass + many-properties pattern (the XAML idiom)** — every XAML element type has many properties (a `Button` has ~50+); every property is registered separately but lives on the one class. | XAML / WPF pattern. | Bevy + Buiy: decomposed components ([foundation goal 3](../../specs/2026-05-07-buiy-foundation/README.md#buiys-goals-the-product), [§ 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)). Every Buiy component is small, public-fielded, observable, decomposed by concern. **No megacomponents.** This is reinforced by both NoesisGUI's pattern (avoid) and bevy_a11y issue #17644 (avoid). |
| **Windows-only authoring tooling (Blend for VS)** — the de-facto XAML authoring tool requires Windows + Visual Studio. macOS / Linux users face friction. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "XAML's age and learning curve." | Buiy authoring is text-based BSN; any editor works. Hot-reload through Bevy's asset system. Devtools sub-spec (foundation [§ 4](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) covers visual inspectors but isn't load-bearing for authoring. **Authoring is cross-platform by default.** |
| **WebGL target without browser-native a11y / form / DOM integration** — Noesis on WebGL is a canvas. No native screen reader, no form-control native behaviour, no browser zoom respect. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Web target." | Foundation [§ 5 WASM open question](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) acknowledges this. Web a11y waits for AccessKit's web adapter (not yet shipped). **Don't claim browser-native web parity until the AccessKit web adapter exists.** |
| **One-render-target-per-view design assumes single-window** — Noesis's `View` model implies one-fullscreen-game-UI, not multi-window app. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Multi-window / multi-display app patterns." | Foundation [window-and-surface sub-spec](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) commits to multi-window. AccessKit adapter ownership is per-window already ([§ 2.6 adapter ownership](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)). **Multi-window is foundation-tier for Buiy.** |
| **Proprietary distribution friction** — Noesis on Unity Asset Store version drift, Noesis Studio "Coming 2024" multi-year beta, Indie tier without source-code access. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Asset Store version drift" + "WYSIWYG ergonomics still maturing." | crates.io is Buiy's primary distribution; release discipline tracks Bevy minor cadence. Source is open by license. Devtools sub-spec is committed in the foundation roadmap. **No distribution-channel drift possible — there is one channel.** |
| **Markup-language age + LLM-training thinness** — XAML predates LLM training corpora; AI-assisted XAML authoring is reportedly less effective than AI-assisted HTML/CSS. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "AI prompting forum thread." | BSN is similar to ECS spawning idioms; Rust + Bevy LLM corpus is large and growing. **Pick markup that LLMs already understand.** |

## Borrow

Concrete primitives worth studying and adapting from NoesisGUI:

1. **Two-tier API: Framework vs Integration.** Noesis splits its API into a high-level engine-agnostic Framework API (Controls / Panels / DependencyProperty system / XAML loader) and a low-level engine-specific Integration API (`RenderDevice`, texture providers, font providers). Buiy doesn't need an Integration API at the C-level (Bevy provides it), but the *conceptual* split is right: **the public Buiy API is uniformly engine-agnostic-shaped (no Bevy-specific types in user code outside of plugin-installation)** even though the runtime depends on Bevy internally. See [`architecture.md`](architecture.md) § "Two-tier API."

2. **MVVM separation pattern.** Noesis's ViewModel is the C# object with `INotifyPropertyChanged` properties + `ICommand` methods; View is the XAML; Model is the underlying domain data. The pattern is what Larian called out as load-bearing for Baldur's Gate 3 UI scale. For Buiy: keep state in ECS resources / observed components (the ViewModel-equivalent), render against them via observers + change detection (the View-equivalent). When/if Buiy adds a signals layer (foundation [§ 5 open question](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)), preserve the MVVM-style separation. See [`xaml-paradigm.md`](xaml-paradigm.md) § "Data binding & MVVM."

3. **GPU-tessellated vector geometry pipeline.** Noesis re-tessellates per frame; texture caching is for glyph atlases only; gradients / rounded rects / vector paths are first-class. Buiy's render pipeline should use this shape: **tessellate per frame, cache atlases only, treat vector paths as first-class primitives.** See [`rendering-and-performance.md`](rendering-and-performance.md) § "Vector graphics on GPU."

4. **Single-pass stereo rendering for VR.** Noesis renders UI to both eye buffers in a single pass for VR. Buiy's 3D-anchored UI sub-spec (foundation [§ 3 cross-cutting](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) should treat single-pass stereo as a first-class concern, not a future retrofit. See [`rendering-and-performance.md`](rendering-and-performance.md) § "What the renderer ships."

5. **`BackgroundEffect` pattern for backdrop blur.** Noesis ships a `BackgroundEffect` element that blurs everything behind a panel — CSS `backdrop-filter` analogue. Buiy commits to `backdrop-filter` ([foundation § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns)); the implementation pattern of "panel + backdrop-effect references prior frame's render target" is the borrowable shape.

6. **Template hot-reload preserving instance state.** As of 3.2.11 (February 2026), Noesis preserves instance state across XAML template hot-reload. Buiy's BSN hot-reload (foundation [§ 5 hot-reload of components open question](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)) should commit to the same: preserve entity state where it doesn't conflict with the new template, surface clear errors where it does.

7. **VirtualizingWrapPanel-style virtualization.** Added in 3.2.9 (October 2025). For productivity-app patterns (long lists, grid views) virtualization is necessary; Bevy's `UniformList` / `List` (from [gpui/](../gpui/)) is the parallel. Buiy's widget catalog needs virtualization for any list/grid pattern at scale.

8. **Console support discipline.** Noesis ships per-console SDK builds, manages NDAs centrally, makes console support included-in-all-tiers (rather than tier-gated). This is the customer experience console-shipping studios value. Buiy can't replicate exactly (open-source can't carry NDA'd code), but the *pattern* — **console support is foundation-tier, not a future tier-gated feature** — is right. When Bevy adds first-class console support, Buiy should not gate UI features by console.

9. **Localization first-class via XAML markup extensions.** Noesis supports LTR and RTL layouts; localization is integral to the XAML model. Buiy's i18n sub-spec (foundation [§ 4 buiy-i18n-design](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) commits to BiDi + RTL + ICU + locale-aware formatters; the pattern of "localization expressed in markup, not retrofitted to widgets" is right.

10. **Rive / Lottie integration.** Noesis ships native support for Rive and Lottie as XAML-embedded animation primitives. Buiy's animation sub-spec (foundation [§ 4 buiy-animation-design](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) currently focuses on transitions + keyframes + springs; consider Rive / Lottie as first-class external animation formats that BSN can embed, not as a future add-on. The animator workflow benefit is significant for game studios.

## How to use this file

When designing a Buiy feature:

1. **Find the row in `Avoid`** that names a pitfall close to your design. Read the linked file for the source detail.
2. **Find the entry in `Borrow`** that names a primitive close to what you're designing. Read the linked file to understand the NoesisGUI shape, then adapt for Buiy's ECS / open-source / Bevy-only architecture.
3. **Promote any decision into a Buiy spec** under `docs/specs/` — this file is for capturing what we learn from NoesisGUI, not for encoding Buiy's own decisions.

## Sources

- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling evidence files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`xaml-paradigm.md`](xaml-paradigm.md), [`rendering-and-performance.md`](rendering-and-performance.md), [`engine-integration.md`](engine-integration.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md), [`glossary.md`](glossary.md)
- Related prior-art folders: [`bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`unreal-slate-umg/`](../unreal-slate-umg/), [`gpui/`](../gpui/)
- NoesisGUI features page — https://www.noesisengine.com/noesisgui/
- WPF / UWP comparison docs — https://www.noesisengine.com/docs/Gui.Core.WPFComparison.html
- Larian Baldur's Gate 3 testimonial — https://www.noesisengine.com/
- Noesis customers page — https://www.noesisengine.com/customers.php
