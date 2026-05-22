**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — critiques and open problems

# Critiques and open problems

Slate + UMG is the second-most-shipped game-engine UI stack on Earth — battle-tested at extreme scale, structurally stable for 15 years, comprehensive in feature coverage. None of the critiques below contradict that record. They identify *where the structure pays interest* and *what a successor with the benefit of hindsight would do differently*. For Buiy's purposes, this is the canonical list of pitfalls the design must avoid.

## Critiques

### 1. The `SNew` C++ macro DSL is exhausting

Slate's declarative syntax is genuinely impressive — at the time (2010-2014), C++11 was barely a thing, and the macros + template metaprogramming pattern (later refined as "Expression Templates" in modern usage) gave Epic a working declarative UI in pure C++. But every Slate author hits the same friction:

- **Compile times.** The nested template expansion inside a `Construct` body is slow enough to compile that Epic ships `BEGIN_SLATE_FUNCTION_BUILD_OPTIMIZATION` / `END_SLATE_FUNCTION_BUILD_OPTIMIZATION` macros that disable optimization for the function specifically because the optimizer takes minutes.
- **Lifetime management.** Raw `this` pointer captures in event handlers (`.OnClicked_Raw(this, &Class::Handler)`) are unsafe by default; the correct path is `.OnClicked_SP(SharedThis(this), &Class::Handler)`, which requires the class to inherit from `TSharedFromThis<T>` — a foot-gun pattern that catches every new Slate author.
- **Macro discoverability.** `SLATE_ARGUMENT` vs `SLATE_ATTRIBUTE` vs `SLATE_EVENT` vs `SLATE_NAMED_SLOT` vs `SLATE_PRIVATE_ARGUMENT_TYPE` — the macro vocabulary is large and named inconsistently relative to what it generates.
- **No clean intermediate representation.** Slate's "tree" lives only as the result of running `SNew` — you can't easily inspect, transform, or serialize it.

A clean Rust UI library — Buiy — gets the declarative experience essentially for free via the type system (no macros required for `commands.spawn((Button, OnPress::new(submit), children![Text::new("Save")]))`). The asset-side gets it via BSN (also no macros at the consumer site). The Slate macros are a 2010-era C++ artifact; nothing in the *shape* requires them.

### 2. UMG's per-frame Blueprint tick is a performance trap

Epic publishes a [dedicated optimization guide](https://dev.epicgames.com/documentation/en-us/unreal-engine/optimization-guidelines-for-umg-in-unreal-engine) that, summarized, says: **don't use `Tick`, don't use bound attributes, drive UI via events.** The shape of the trap:

- A bound attribute (Designer's "Bind Function" path) compiles to a Blueprint function called every frame from `STextBlock::TickWidget`. Every bound attribute = one Blueprint call per frame per binding.
- Blueprint VM is slower than native C++. Per Epic's own benchmarks: 10-100x slower for arithmetic-heavy node graphs, comparable once execution enters a native node.
- Per-frame Blueprint code is *the* dominant UMG perf cost at scale. A complex menu with 50 bound text attributes can spend more frame budget on Blueprint VM than on the entire game's render pass on lower-end hardware.

The MVVM plugin (UE 5.1+) is Epic's recommended fix — change-driven view-model bindings. But MVVM is opt-in, requires C++ for the view-model classes, and isn't the default authoring path most asset designers learn.

Buiy avoids this trap by **never** offering a per-frame poll path. Bevy's change detection + observers are change-driven by construction; per-frame property re-evaluation is not a primitive the API exposes.

### 3. Two parallel stacks (Slate + UMG) is permanent friction

Every UMG widget has a corresponding Slate widget; every UMG property mirrors a Slate field; every UMG event re-routes a Slate event. The mirror is maintained by hand. Consequences:

- **Doc drift.** Slate's `SButton` and UMG's `UButton` have separate docs; features land in one and lag in the other. The community-maintained UMG-Slate-Compendium exists *because* the official docs don't bridge the two cleanly.
- **Feature lag.** New Slate widgets often take quarters or releases to get a UMG wrapper. New UMG features rarely propagate down to Slate.
- **The "which to use" question** — see [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md). After 12 years, this is still asked monthly on the Epic forums.
- **Editor extensions vs runtime UI** split: editor extensions almost always go Slate-only because UMG doesn't run in editor windows; runtime game UI almost always goes UMG. The decision has nothing to do with the UI's needs and everything to do with where the UI lives.

Buiy commits to **one stack**: ECS components authorable both from Rust (`commands.spawn(...)`) and from `.bsn` assets. The same `Button` component is used everywhere. No mirror to maintain.

### 4. Accessibility lag

See [`accessibility.md`](accessibility.md) for the full picture. The summary critique:

- Five common widgets ship with screen-reader support; everything else needs project-side a11y code.
- No Linux a11y at all.
- No `:focus-visible` distinction (the WCAG 2.2 §2.4.7 / §2.4.13 obligation falls on each shipped title individually).
- No live-region primitives (`aria-live` equivalents).
- No live a11y inspector built into the editor.
- The Unreal Editor itself is inaccessible — a non-trivial commercial issue (vision-impaired developers cannot use UE professionally).

The gap shows up in WCAG-conformance audits, in IGDA Game Accessibility Special Interest Group reports, and in the fact that shipping accessibility-grade game UI on UE requires building most of the a11y stack per-title. Buiy's AccessKit-first commitment is structurally a corrective.

### 5. No web-platform-aligned layout

Slate's layout primitives — boxes, overlays, grids, scroll boxes — are powerful but ad-hoc. Missing primitives that show up in web layout:

- **No CSS Grid `fr` units, no named lines, no subgrid, no `repeat()`.**
- **No container queries** — UI must be re-authored per breakpoint.
- **No anchor positioning** (CSS `anchor()` family).
- **No logical properties** — Slate is physical-direction-only; RTL layout requires `RenderTransform` hacks.
- **No aspect-ratio property** — compose with `SScaleBox`.
- **No baseline alignment** for cross-text aligned controls.
- **No CSS `position: sticky`** equivalent.

For shipped games this rarely bites — game UI authors design per-resolution and per-platform. For app UI (a Buiy goal), it bites immediately. Buiy commits to Taffy + custom anchor/container-queries/writing-modes work above Taffy.

### 6. Renderer is opaque and engine-coupled

Slate's `FSlateRHIRenderer` is hardwired into Unreal's RHI. Consequences:

- **Cannot run Slate outside Unreal.** No standalone Slate library.
- **Cannot swap renderers.** Custom backends (wgpu, Vello, Skia) are not possible — Unreal's RHI is the only path.
- **The render pipeline is hard to extend.** Custom render passes mean modifying engine source — practical only for AAA studios who fork.

Buiy is in the opposite position: Bevy's render-graph is the renderer, wgpu is the GPU layer, and Buiy registers its own render-graph node. The pipeline is extensible by every consumer.

### 7. Single corporate steward

Epic Games is the only entity that ships Slate. No foundation, no public RFC process, no community trunk. Implications:

- **Direction-setting is internal.** When the BSN-equivalent decision happens for Slate, the community finds out at release.
- **No fork-and-experiment.** EULA forbids public forks.
- **Risk concentration.** If Epic shifts focus (Fortnite Creative growth, Unreal-for-virtual-production pivot, post-Apple-vs-Epic strategy), Slate gets whatever resourcing falls out.

Buiy is OSS-stewarded; the trade-off is uneven attention and slower cycles, but the floor isn't "one company's strategic call."

## Open problems

Issues that aren't critiques (Slate works fine), but are absences a 2026 successor would address.

### A11y completeness

WCAG 2.2 AA conformance is per-title at best. There is no out-of-the-box "conformant default theme" and no automated conformance harness. A modern UI stack should ship a verifier.

### Container queries / anchor positioning

The CSS spec has shipped both (container queries since 2023, anchor positioning since 2024 in Chrome/Edge/Safari). Slate has neither. A 2026-vintage UI stack should plan for both as first-class.

### View transitions

CSS View Transitions (Chrome 111+, Safari 18, Firefox in progress) give a declarative cross-fade-with-shared-element transition between UI states. UMG animations are timeline-based and per-WBP; the cross-state shared-element transition pattern requires hand-rolled work. Buiy's animation spec ([foundation animation sub-spec](../../specs/2026-05-07-buiy-foundation/interaction.md)) lists this as in scope.

### True scroll-driven animations

CSS Scroll-driven Animations spec (Chrome 115+, Firefox 136+). No Slate equivalent. UMG approximates with manual tick-bound scroll listeners.

### A reactive component model

UMG's MVVM plugin is the closest thing — and it's a *plugin*, not the primary authoring path. A 2026 UI stack might commit to signals/computed/effects or to per-property change detection at the runtime layer. (Buiy defers this — observers + change detection only in v1 per foundation README open question.)

### Hot-reload of components, not just assets

UMG Widget Blueprints hot-reload as assets; Slate widgets hot-reload only via the full live-coding C++ recompile (slow and sometimes flaky). Buiy commits to `.bsn` hot-reload as part of asset reloads (see foundation README open question on hot-reload of components).

### Web target

UE's HTML5 target was deprecated in UE 4.24. Slate has no future as a web-target UI. A 2026 UI stack should at least *plan for* a wgpu-on-WASM future where the renderer carries over even if a11y waits for AccessKit's web adapter. Buiy plans for this (foundation README open question on Bevy WASM policy).

### A single focus tree

Slate's focus model + CommonUI's spatial nav + UMG's `IsFocusable` flag + project-side focus traps amount to four overlapping mechanisms with edge cases at the intersections. A successor (Buiy) commits to one focus tree.

## What Slate / UMG got right

To round out the critique, the patterns worth preserving — see [`lessons.md`](lessons.md) for the structured borrow list:

- **Asset-based UI authoring.** Widget Blueprints made UMG the recommended path the moment they shipped.
- **Reflection-driven property surface.** Every property exposed identically in C++, Designer, and serialized asset. Identical to Buiy's BSN bet.
- **Declarative widget instantiation.** `SNew(SButton).OnClicked(...)[ child ]` is dense but readable. The shape (named arguments + slot content) translates to Buiy.
- **Style sets as registered assets.** `FAppStyle::Get().GetBrush("Icons.Save")` is a clean lookup pattern.
- **Cross-platform input routing (CommonUI).** Input stack + activatable widgets are a model worth borrowing.
- **`FText`-everywhere localization discipline.** Forces every shipped UI string through a localization pipeline.

## Sources

- Optimization Guidelines for UMG — https://dev.epicgames.com/documentation/unreal-engine/optimization-guidelines-for-umg-in-unreal-engine
- UMG Best Practices — https://dev.epicgames.com/documentation/en-us/unreal-engine/umg-best-practices-in-unreal-engine
- Balancing Blueprint and C++ — https://dev.epicgames.com/documentation/en-us/unreal-engine/balancing-blueprint-and-cplusplus
- UE5 Blueprint vs C++ performance (Spongehammer) — https://www.spongehammer.com/unreal-engine-5-blueprint-vs-cpp-performance/
- Blueprint Performance (GameDev Pensieve) — https://www.gamedevpensieve.com/engines/unreal/unreal_programming/unreal_blueprint-performance
- WIP Guide to Slate / UMG + dealing with current issues — https://forums.unrealengine.com/t/wip-guide-to-slate-umg-dealing-with-current-issues/819422
- Supporting Screen Readers in Unreal Engine — https://dev.epicgames.com/documentation/en-us/unreal-engine/supporting-screen-readers-in-unreal-engine
- Anatomy of a Widget — https://codekittah.medium.com/anatomy-of-a-widget-c-unreal-engine-b479a100c7e3
