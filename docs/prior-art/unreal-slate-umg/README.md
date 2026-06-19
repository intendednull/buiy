**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — Epic Games' dual-layer UI stack (declarative C++ Slate + designer-asset UMG); the canonical AAA-scale game-engine UI

# Unreal Slate + UMG

Unreal Engine ships **two sibling UI stacks** that together cover Epic's full UI surface:

- **Slate** (introduced during the UE3→UE4 editor rewrite, ~2010; stable in UE4 2014). Declarative C++ retained-mode framework. Prefix `S`. The whole Unreal Editor is written in it.
- **UMG** (Unreal Motion Graphics, introduced in UE 4.5, November 2014). Designer-friendly Blueprint-driven layer built **on top of** Slate. Prefix `U`. The thing game UI authors actually touch.

Both ship with every Unreal Engine install. Slate is the load-bearing programmer framework; UMG is the asset-and-graph layer above it that runs the Widget Blueprint Editor. Game UI in UE4/UE5 is almost universally authored in UMG; Slate is reserved for engine plumbing, editor extensions, and the few cases where UMG can't reach low enough.

This folder treats Slate and UMG as **one stack with two layers** because they share a runtime (every UMG widget is backed by a Slate widget) and because Buiy's two analogous concerns — the programmer-facing component model and the asset-based authoring layer — map cleanly onto this split.

## Two-layer distinction at a glance

| | Slate | UMG |
|---|---|---|
| Audience | Engine / tools programmers | Game UI designers, Blueprint authors |
| Authoring | C++ with `SNew` macro chains | Widget Blueprint Editor (Designer + Graph) |
| Asset format | None — code-only | `.uasset` Widget Blueprints (WBPs) |
| Class prefix | `S` (e.g. `SButton`, `STextBlock`) | `U` (e.g. `UButton`, `UTextBlock`) |
| Base widget | `SWidget` / `SCompoundWidget` | `UWidget` / `UPanelWidget` / `UUserWidget` |
| Layout containers | `SHorizontalBox`, `SVerticalBox`, `SOverlay`, `SBox` | `UCanvasPanel`, `UHorizontalBox`, `UVerticalBox`, `UGridPanel`, `UScrollBox` |
| Layout slots | `FArguments` + `SLATE_*` macros | `UPanelSlot` subclasses, edited in Details panel |
| Runtime | Direct Slate Application + own renderer | UMG widgets wrap Slate widgets 1:1 |
| Who uses it | Unreal Editor, engine code, plugins | Virtually every shipped Unreal game |

Every UMG widget owns a corresponding Slate widget under the hood (`UButton` → `SButton`, `UTextBlock` → `STextBlock`). UMG is not a separate framework — it is a `UObject` + Blueprint-reflection wrapper that exposes Slate to the Editor and the Blueprint VM.

## Key verified facts

- **Slate origin.** Nick Atamas prototyped a new C++ UI layer during UE3 (~2010); the team rewrote 100% of the Unreal Editor UI on it for UE4. See [`history.md`](history.md).
- **UMG origin.** Shipped in **UE 4.5 (November 2014)**: "Unreal Motion Graphics is enabled by default and ready for wide use" per the 4.5 release notes. UMG is a Blueprint-friendly designer layer **on Slate**, not a replacement for it.
- **License.** Unreal Engine is **source-available under the Unreal Engine EULA** (proprietary, not OSI). Royalty model: **5% of gross revenue above $1M lifetime** per product; Epic Games Store sales are royalty-free; "Launch Everywhere with Epic" cuts the rate to 3.5%. Source on GitHub requires linking a verified Epic account. See [`distribution-and-governance.md`](distribution-and-governance.md).
- **Renderer.** Slate owns its own renderer (currently driving Unreal's RHI: D3D11/12, Vulkan, Metal, OpenGL ES). Not wgpu. Not retained-mode-on-engine-meshes.
- **Layout.** Slate has its own layout pass — not Flexbox, not CSS Grid. Containers (`SHorizontalBox`, `SVerticalBox`, `SOverlay`, `SBox`, `SUniformGridPanel`, `SWrapBox`, `SScrollBox`, `SConstraintCanvas`) own their own slot rules. UMG mirrors the set with its own `U`-prefixed panels.
- **Text.** Slate uses its own text shaping based on HarfBuzz (built into the engine), ICU for line breaking + BiDi, and `FreeType` for glyph rasterization. CJK, IME, BiDi, complex scripts are all supported. See [`text-rendering.md`](text-rendering.md).
- **Accessibility.** Unreal has *limited* accessibility. iOS VoiceOver + Android TalkBack bridges ship for mobile; Windows third-party screen readers (NVDA, JAWS) are supported on common UMG widgets (Text Block, Editable Text Box, Slider, Button, Checkbox). **No AccessKit, no Linux AT-SPI, no first-class focus model abstraction.** See [`accessibility.md`](accessibility.md).
- **CommonUI plugin.** Layered on UMG since UE 4.27 / UE5: cross-platform input routing, controller-icon swapping, focus management. The de-facto modern UMG add-on for shipping games. Referenced in [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md).

## Table of contents

- [`slate-architecture.md`](slate-architecture.md) — Slate's declarative C++ paradigm; `SNew` macro chain; `SCompoundWidget::Construct(FArguments)`; `SLATE_BEGIN_ARGS`/`SLATE_ATTRIBUTE`/`SLATE_EVENT`/`SLATE_NAMED_SLOT`; the `S`-prefixed widget hierarchy; render pipeline.
- [`umg-architecture.md`](umg-architecture.md) — `UWidget` / `UPanelWidget` / `UUserWidget` hierarchy; Widget Blueprints; the Widget Blueprint Editor (Designer / Graph / Animation tabs); how UMG wraps Slate; Blueprint data binding.
- [`widget-vocabulary.md`](widget-vocabulary.md) — Side-by-side widget enumeration: `SButton`↔`UButton`, `STextBlock`↔`UTextBlock`, `SImage`↔`UImage`, etc.
- [`layout-and-styling.md`](layout-and-styling.md) — Slate's box-and-overlay layout primitives, slot rules; UMG panel set; `FSlateBrush` + `FSlateStyleSet` + `FSlateStyleRegistry` styling.
- [`text-rendering.md`](text-rendering.md) — `FSlateFontInfo`, HarfBuzz + ICU, FreeType rasterization, BiDi, CJK, font fallback, IME composition.
- [`accessibility.md`](accessibility.md) — Where Unreal's a11y story is in 2026, what it covers, what it doesn't, and how the gap explains Buiy's AccessKit-first commitment.
- [`history.md`](history.md) — Slate prototype (~2010), UE4 editor rewrite, UMG in 4.5 (2014), CommonUI in 4.27+, UE5 era.
- [`distribution-and-governance.md`](distribution-and-governance.md) — Epic Games stewardship, the Unreal EULA, the 5%/3.5% royalty model, GitHub source access, governance.
- [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) — Production use (Fortnite, Gears, virtually every UE5 title), CommonUI, comparison vs Unity UGUI/UI Toolkit, Godot Control, Bevy UI, Buiy.
- [`critiques-and-open-problems.md`](critiques-and-open-problems.md) — Slate macro overhead, UMG Blueprint tick perf, two-stack friction, weak accessibility, no web-parity features (anchor positioning, container queries, true CSS layout).
- [`lessons.md`](lessons.md) — **The decision file.** Validates / Avoid / Borrow for Buiy.
- [`glossary.md`](glossary.md) — Slate, UMG, `SNew`, `FArguments`, `UWidget`, WBP, `FSlateBrush`, CommonUI, etc.

## Honest assessment

Slate + UMG is the **second-most successful game-engine UI stack in production** after Unity's UI systems. Fortnite — one of the most-played games on Earth — ships its entire UI on UMG + CommonUI. Every UE-shipped AAA title (Gears, Senua's Saga, Black Myth: Wukong, Final Fantasy VII Rebirth's PC port, Stalker 2) ships UI through this stack. The combination has been battle-tested at extreme scale across console, PC, mobile, VR.

What it is **not**: web-platform-aligned, accessible by default, ergonomically modern, or licensed in a way an OSS-first Rust project can borrow code from. Slate's `SNew(SButton).OnClicked_Raw(this, &MyClass::OnClicked)[ SNew(STextBlock).Text(...) ]` declarative DSL is widely cited as both impressive (it works) and exhausting (macros, raw pointer captures, build-time TArguments expansion). UMG's Blueprint authoring is approachable but performance-tax-laden — every UMG `Tick` is more expensive than a Slate `Tick`, and Epic publishes a dedicated "UMG Best Practices" guide on avoiding it. Accessibility lags every web platform by years.

For Buiy, the relevant takeaways are not the implementation. They are the **shape**:

1. A two-layer stack — fast programmer framework underneath, asset-friendly authoring on top — is exactly what Buiy commits to with ECS components underneath and `.bsn` assets on top. The split *works* at AAA scale. Where Buiy diverges is **one unified runtime** (Buiy components are directly authorable in BSN; there is no wrapping `U`-layer over an `S`-layer).
2. Declarative widget instantiation with named slots (`FArguments`) is borrowable as a *shape*; the C++ macro implementation is **not** borrowable — Buiy is Rust + BSN, and Rust's `derive` + reflection-driven asset format obviates the need for macro chains.
3. Designer-friendly asset-based UI authoring (Widget Blueprints) is the single biggest reason UMG won over Slate-only authoring for game UI. Buiy's `.bsn` asset story carries the same bet.
4. **Accessibility as an afterthought** is the cost of that bet. UE only added screen-reader support to UMG in 4.22 (2019), only added mobile a11y in 4.23, and still has no Linux story. Buiy makes the opposite commitment: AccessKit-first, WCAG 2.2 AA at the floor, from day one.
5. **Two parallel stacks (Slate + UMG)** create real friction: divergent class hierarchies, double-documentation, Blueprint-only features lagging C++ ones, and the perpetual "use Slate or UMG?" community question. Buiy commits to **one ECS + one BSN authoring layer**, no parallel stack.

See [`lessons.md`](lessons.md) for the structured Validates / Avoid / Borrow decision list.

## Cross-links

- Bevy comparison: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../bevy-ui/comparisons.md`](../bevy-ui/comparisons.md).
- Unity comparison: [`../unity-ui/lessons.md`](../unity-ui/lessons.md) (sibling folder).
- DSL-above-runtime comparison: [`../makepad/README.md`](../makepad/README.md), [`../slint/`](../slint/).
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md), [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md).

## Sources

- Slate UI Programming overview — https://dev.epicgames.com/documentation/en-us/unreal-engine/slate-ui-programming-in-unreal-engine
- Slate Overview — https://dev.epicgames.com/documentation/en-us/unreal-engine/slate-overview-for-unreal-engine
- Understanding the Slate UI Architecture — https://dev.epicgames.com/documentation/en-us/unreal-engine/understanding-the-slate-ui-architecture-in-unreal-engine
- UMG UI Designer — https://dev.epicgames.com/documentation/en-us/unreal-engine/umg-ui-designer-in-unreal-engine
- Widget Blueprints in UMG — https://dev.epicgames.com/documentation/en-us/unreal-engine/widget-blueprints-in-umg-for-unreal-engine
- Common UI Plugin — https://dev.epicgames.com/documentation/unreal-engine/common-ui-plugin-for-advanced-user-interfaces-in-unreal-engine
- Supporting Screen Readers in Unreal Engine — https://dev.epicgames.com/documentation/en-us/unreal-engine/supporting-screen-readers-in-unreal-engine
- Unreal Engine 4.5 Release Notes — https://www.unrealengine.com/en-US/blog/unreal-engine-45-released
- Unreal Engine EULA — https://www.unrealengine.com/eula/unreal
- Unreal Engine license options — https://www.unrealengine.com/license
- Tim Sweeney: Classic Tools Retrospective (the first Unreal Editor) — https://www.gamedeveloper.com/design/classic-tools-retrospective-tim-sweeney-on-the-first-version-of-the-unreal-editor
- The Slate UI Framework Part 1 (Gerke Max Preussner, Epic) — https://de45xmedrsdbp.cloudfront.net/Resources/files/slateTutorials_westcoast-1963123470.pdf
- UMG-Slate-Compendium — https://github.com/YawLighthouse/UMG-Slate-Compendium
