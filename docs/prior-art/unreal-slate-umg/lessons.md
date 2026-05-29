**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — Validates / Avoid / Borrow decisions for the Buiy foundation

# Lessons for Buiy

The consult-this-when-designing decision file. Other files in this folder are evidence; this file is the synthesis. Three sections — **Validates** (Buiy choices that Slate/UMG confirms), **Avoid** (pitfalls Buiy must mitigate), **Borrow** (primitives worth adapting).

## Validates

Buiy design commitments that Unreal Slate/UMG's 15-year production record confirms.

- **Dual-layer authoring (programmer code + designer-friendly assets).** UMG shipped on top of Slate in 2014 and immediately became the recommended path for game UI. Twelve years later, virtually every shipped UE title uses UMG, not Slate-only. The asset-based + visual-editor + Blueprint-handler path *wins* over code-only authoring at scale. Buiy's commitment to **ECS code + `.bsn` assets, both first-class** (foundation [architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)) is validated. Where Buiy differs: **one runtime, one component model** — not a `U`-wrapper over an `S`-base; the same `Button` component is what BSN spawns and what ECS code spawns.

- **Declarative widget instantiation is the right shape.** Slate proves at AAA scale that declarative widget trees + named-slot children + bindable attributes is a workable authoring paradigm. Buiy's `commands.spawn((Button, OnPress::new(submit), children![Text::new("Save")]))` is the same shape, sans macros. Validates [architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md).

- **Owning the renderer pays for itself.** Slate has owned its render pipeline since 2010; the gap between Slate's feature set (rounded clipping, alpha blending, materials-as-brushes, custom shaders per widget) and a hypothetical "ride the engine renderer" path is the difference between *being able* to ship the Unreal Editor and *not*. Buiy's parallel-stack decision ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md): "Render pipeline — custom Bevy render passes that walk Buiy hierarchies") is validated. Owning the renderer unlocks rounded clipping, `clip-path`, `backdrop-filter`, `mix-blend-mode`, true top-layer compositing — capabilities bevy_ui can't deliver without major rework.

- **Reflection-driven component surface unlocks visual authoring.** Every UMG property is a reflected `UPROPERTY`, every slot a reflected `UPanelSlot` — that universality is what makes the Widget Blueprint Designer possible at all. Buiy's commitment that every component derives `Reflect + FromReflect + Default + Clone + Component` and is `register_type<T>`'d ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)) is validated; without it, no `.bsn` editor is buildable.

- **Cross-platform input routing belongs in the foundation.** CommonUI is so essential to AAA UE shipping that Epic re-extracted it from Fortnite as a public plugin. Cardinal navigation, input stacks, activatable widgets, controller-icon swap — these are not afterthoughts. Buiy's commitment to **one focus tree with sequential nav + spatial gamepad nav + traps + restoration + roving tabindex** ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) puts CommonUI's shape in the foundation, not in a plugin.

- **A localization-by-default text API is enforceable.** Slate's `FText`-everywhere discipline means no UI in shipped Unreal titles accidentally hard-codes English strings — the API rejects raw strings at the boundary. Buiy can carry this contract via the type system + a CI lint that bans `&str` in widget property positions intended for user-visible text. (Add to verification harness fixtures.)

- **Styling as registered, named, typed assets.** `FSlateStyleSet` + `FSlateStyleRegistry` proves the "register-once, lookup-by-name, type-checked" pattern works for a UI of editor-scale complexity. Buiy's theme-as-asset + semantic-tokens commitment ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the same shape; the contrast linter at load and in CI is an explicit advance.

- **Single steward → architectural stability.** Slate has been architecturally stable for 15 years because Epic alone makes the decisions. This is a positive lesson for Buiy in the medium term: a small maintainer set with clear technical direction shipped a stack that has not needed to be replaced. Buiy's plan to ship a coherent foundation in `docs/specs/` before opening to community drift is informed by this.

## Avoid

Pitfalls drawn from Slate/UMG with Buiy's mitigation.

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **C++ macro DSL ergonomics.** `SLATE_BEGIN_ARGS`/`SLATE_END_ARGS`/`SLATE_ATTRIBUTE`/`SLATE_EVENT`/`SLATE_NAMED_SLOT` plus `SNew`/`SAssignNew`/`SP`/`Raw`-suffixed binders are a large macro vocabulary inherited from 2010-era C++. Compile-time pain so severe that Epic ships `BEGIN_SLATE_FUNCTION_BUILD_OPTIMIZATION` to disable the optimizer. | [`slate-architecture.md`](slate-architecture.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "The `SNew` C++ macro DSL". | Buiy is **Rust + BSN**. No author-facing macros for widget construction. ECS spawn syntax is plain function calls; BSN syntax is asset markup. Macro use restricted to internal `Reflect` derive (one macro, deterministic, fast). |
| **Two parallel stacks (Slate + UMG).** Twelve years of "should I use Slate or UMG?" community traffic; doc drift; feature lag between the layers; editor-vs-runtime split forced by stack choice, not UI needs. | [`umg-architecture.md`](umg-architecture.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) § "Distinctive friction", [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Two parallel stacks". | **One stack.** Buiy ECS components are the single authoring vocabulary; `.bsn` is asset notation for the same components, not a wrapper layer. No `U`-prefix mirror. |
| **Accessibility as an afterthought.** First screen-reader support arrived in UE 4.22 (2019), nine years after Slate started; covers five widgets; no Linux; no live-region primitive; no `:focus-visible`; no AccessKit equivalent. | [`accessibility.md`](accessibility.md). | **AccessKit-first** from foundation v1. Decomposed `A11yRole`/`A11yLabel`/`A11yDescription`/`A11yStates`/`A11yRelations` components. ACCNAME 1.2. WCAG 2.2 AA at the floor. Live-region announcer as a Buiy resource. Cross-link [foundation accessibility § 3.11](../../specs/2026-05-07-buiy-foundation/accessibility.md). |
| **Proprietary EULA + royalty.** Unreal Engine EULA forbids OSS reuse; 5% royalty above $1M lifetime; cannot combine with copyleft code. | [`distribution-and-governance.md`](distribution-and-governance.md). | **MIT OR Apache-2.0** dual permissive. Buiy can be embedded in any Rust project, including copyleft ones. No royalty, no tracking, no per-product reporting. |
| **Per-frame Blueprint tick / bound-attribute trap.** UMG's "Bind Function" path runs a Blueprint VM call per binding per frame. Epic ships a whole optimization guide that says "don't do this." | [`umg-architecture.md`](umg-architecture.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "UMG's per-frame Blueprint tick". | Buiy exposes **no per-frame poll path.** Change-driven only: observers + change detection (Bevy primitives). The MVVM-equivalent reactive layer is deferred (foundation README § 5 open question), but never as a per-frame poll. |
| **Layout primitives not web-aligned.** No CSS Grid `fr`/named lines/subgrid, no container queries, no anchor positioning, no logical properties, no aspect-ratio, no `position: sticky`. | [`layout-and-styling.md`](layout-and-styling.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "No web-platform-aligned layout". | Taffy gives Buiy Flexbox + Grid + Block + Float; Buiy adds anchor positioning, container queries, writing-modes, top-layer above Taffy. See [layout sub-spec](../../specs/2026-05-08-buiy-layout-design/README.md). |
| **No Linux platform parity.** Linux a11y missing entirely; Linux Editor build exists but is uncommon. | [`distribution-and-governance.md`](distribution-and-governance.md) § "Mac, Linux, PC platform parity", [`accessibility.md`](accessibility.md). | Buiy targets Linux equally with Windows/macOS for desktop v1. AccessKit's AT-SPI adapter for Linux is the substrate. |
| **No published RFC process.** Epic decides Slate's direction internally; community PRs are the exception. | [`distribution-and-governance.md`](distribution-and-governance.md) § "Steward". | Buiy's `docs/specs/` + `docs/plans/` + `docs/reports/` + `docs/prior-art/` is the canonical doc log. No design state in Discord. The `docs/README.md` master index is the only entry point (foundation `CLAUDE.md` cements this). |
| **`Tick`-based UMG bindings encourage author-side tech debt.** Authors learn Bind Function, ship it, then can't remove it without breaking the menu. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "UMG's per-frame Blueprint tick". | No `Tick` analog. Buiy components are change-detected by Bevy ECS by default. |
| **Renderer locked to engine RHI.** Slate cannot run outside Unreal. Custom render backends are not possible. | [`distribution-and-governance.md`](distribution-and-governance.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Renderer is opaque and engine-coupled". | Buiy's render pipeline is **Bevy render-graph nodes + wgpu**; extensible by every Bevy consumer; portable wherever Bevy runs. |

## Borrow

Concrete primitives worth studying and adapting (in Rust, in BSN, in Buiy ECS).

1. **The Widget-Blueprint asset shape.** A WBP is reflection-driven, hierarchy-explicit, slot-typed, hot-reloadable, and edited in a visual Designer. Buiy's `.bsn` assets are the analog. **Borrow:** the WBP-asset DX — a designer can open a `.bsn` in an editor, drag widgets in a hierarchy view, edit properties in a Details panel, and the asset persists with stable identity. **Skip:** the `.uasset` binary format (Buiy uses Bevy's asset system with `.bsn` text format), the inheritance model (Buiy uses ECS composition, not class inheritance).

2. **`SNew` + `FArguments` named-slot pattern → BSN slot syntax.** Slate's `SNew(SHorizontalBox) + Slot().FillWidth(1).Padding(...)[ child ]` cleanly separates layout-rule (the `Slot()` arguments) from content (the `[ child ]` block). BSN inherits exactly this separation: a `children!` macro takes the child list; per-child slot configuration is per-component. **Borrow:** the *shape* — slot-config and content are addressable separately. **Skip:** the `+` operator and the `[...]` operator overloading.

3. **`SLATE_NAMED_SLOT` for compound widgets with multiple insertion points.** A `Card` widget has a `Header`, `Body`, `Footer`; a `Dialog` has `Title`, `Content`, `Actions`. Slate's named slots make this declarative. **Borrow:** Buiy widget components that have multiple semantic child slots expose them as named relationship components (`HeaderOf`, `BodyOf`, `FooterOf`) or as a single `slot: Slot` component with a discriminator field. The shape carries.

4. **`FSlateBrush` family as a renderer-input checklist.** The variants (`FSlateBoxBrush` 9-slice, `FSlateBorderBrush`, `FSlateImageBrush`, `FSlateColorBrush`, `FSlateRoundedBoxBrush`, `FSlateDynamicImageBrush`, `FSlateMaterialBrush`) are a battle-tested enumeration of what a UI renderer needs to support. **Borrow:** treat this list as the v1 capability target for `buiy_core`'s render pipeline. Solid fill, 9-slice, image, rounded-rect with outline, material-driven custom — all explicitly callable shapes.

5. **`FSlateStyleSet` + `FSlateStyleRegistry` as a theme-asset model.** Named, registered, type-checked, lookup-by-string. Buiy's theme tokens are the analog. **Borrow:** the "single resource holds the registry, anyone can lookup by name" pattern. **Refine:** Buiy uses typed semantic tokens (`color.surface.primary`) instead of string names, so the compiler enforces correctness.

6. **CommonUI's input-routing + activatable-widget stack.** Push a widget onto a stack, it gets input focus, it gets controller-icon updates, it gets the action bar. Pop, prior focus restored. **Borrow:** Buiy's focus model + popover/modal stack should compose into a similar contract — push-pop activation, automatic input routing, automatic action-bar update. This is the model the foundation spec already commits to ([architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md) focus model + popover/modal stack).

7. **CommonUI's cardinal-direction spatial nav.** Gamepad / arrow-key spatial nav by `CompassOctant` direction, manual edges takes priority, fallback by best-candidate scoring with visibility + screen-bounds + camera filtering. **Borrow:** the algorithm shape directly — Buiy's `AutoDirectionalNavigation` analog (which Bevy 0.18+ also ships; see [`../bevy-ui/text-and-input.md`](../bevy-ui/text-and-input.md) § "Gamepad / spatial navigation") follows the same pattern.

8. **`SInvalidationPanel` for cordoning off rarely-changing subtrees.** A subtree marked invalidation-only re-paints only when explicitly invalidated; otherwise it reuses the cached draw elements from the prior frame. **Borrow:** Buiy's render pipeline ships an analog component — large static subtrees (item descriptions, lore text, achievements lists) can opt into cached-paint. This is a perf escape hatch worth designing in early.

9. **`FText`-everywhere localization discipline.** Slate's API takes `FText` (a localizable type), not `FString` (a raw string), for any user-visible text. **Borrow:** Buiy uses a localizable text type (`buiy::Text` wraps an id + namespace, resolves via Bevy's locale resource); the public widget API for user-visible text positions takes only the localizable type, never `&str`. Enforce in CI as a lint.

10. **`UMGViewModel` / MVVM as a deferred-but-planned reactive layer.** UMG's MVVM plugin (UE 5.1+) replaced per-frame poll with change-driven view-model binding. **Borrow:** the *shape* — a separate view-model entity carries data; widget bindings re-evaluate only on change. Buiy defers this to foundation README § 5's open question on a signals/computed/effects layer; when it ships, it should look more like UMG MVVM (change-driven, declarative bindings) than like Floem's signal reactivity or Dioxus's hooks model. The shape is closer to ECS-natural.

11. **`UDynamicEntryBox` + `UListView` virtualized list pattern.** UMG's list views virtualize so a 10,000-item list materializes only the visible widgets + a few off-screen. **Borrow:** Buiy's `buiy_widgets` list/grid/tree widgets virtualize by default. Build the virtualization primitive into the foundation, not into individual widget specs.

12. **The Widget Reflector live a11y/layout inspector.** UMG ships a Widget Reflector that inspects the live Slate tree, highlights bounds, dumps the accessibility tree. **Borrow:** Buiy devtools spec ([foundation cross-cutting](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) commits to an AccessKit tree viewer + layout overlay + contrast checker + focus visualizer. The Widget Reflector is the precedent.

## Cross-stack synthesis

When the Buiy team designs an authoring story, the comparison frame is:

| Concept | Slate/UMG | Bevy UI | Buiy commitment |
|---|---|---|---|
| Code authoring | C++ + `SNew` macros | `commands.spawn((..., children![...]))` | Same as Bevy |
| Asset authoring | Widget Blueprints | (BSN planned) | `.bsn` assets, first-class |
| Reflection | UPROPERTY-based | Bevy `Reflect` | Bevy `Reflect`, BSN-friendly by construction |
| Designer tool | UMG Widget Blueprint Editor | (none) | TBD (long-term) |
| Renderer | RHI-coupled custom | bevy_ui crate + wgpu (capped) | Custom Bevy render-graph passes + wgpu (full) |
| A11y | Limited platform bridges | AccessKit via bevy_a11y | AccessKit, decomposed, per-window |
| License | EULA + royalty | MIT-or-Apache | MIT-or-Apache |

## How to use this file

When designing a Buiy feature:

1. **Find the row in `Avoid`** that names a pitfall close to your design. Read the linked file for the original incident.
2. **Find the entry in `Borrow`** that names a primitive close to what you're designing. Read the linked file to understand the Slate/UMG shape, then adapt for Buiy's component model (decomposed, public-fielded, observable, reflection-registered, no per-frame poll).
3. **Promote any decision into a Buiy spec** under `docs/specs/`.

## Sources

- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling evidence files: [`README.md`](README.md), [`slate-architecture.md`](slate-architecture.md), [`umg-architecture.md`](umg-architecture.md), [`widget-vocabulary.md`](widget-vocabulary.md), [`layout-and-styling.md`](layout-and-styling.md), [`text-rendering.md`](text-rendering.md), [`accessibility.md`](accessibility.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md), [`glossary.md`](glossary.md)
- Cross-link: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) (Bevy parallel)
- Unreal documentation root — https://dev.epicgames.com/documentation/en-us/unreal-engine
- Common UI Plugin — https://dev.epicgames.com/documentation/unreal-engine/common-ui-plugin-for-advanced-user-interfaces-in-unreal-engine
- Unreal Engine EULA — https://www.unrealengine.com/eula/unreal
