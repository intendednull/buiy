**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — production usage, CommonUI, comparisons with other game-engine UI stacks

# Ecosystem and comparisons

## Production usage

Slate + UMG sits in the second-most-shipped game-engine UI stack in production, behind Unity's UGUI/UI Toolkit combined. The list of titles whose UI ships on Slate + UMG is long; a representative sample:

- **Fortnite** (Epic's flagship, hundreds of millions of MAUs) — the entire game UI, including the front-end menus, item shop, Battle Royale HUD, Creative UI, Fortnite Festival music UI, ships on UMG + CommonUI. CommonUI was originally extracted from Fortnite's UI codebase before becoming a public plugin.
- **Gears of War** series (Gears 5 onward) — UMG-based campaign + multiplayer UI.
- **Senua's Saga: Hellblade II** (Ninja Theory, 2024) — UMG.
- **Black Myth: Wukong** (Game Science, 2024) — UE5 UMG, the largest 2024 Chinese AAA hit.
- **Final Fantasy VII Rebirth** (Square Enix, 2024) and the prior **Remake** (2020) — UMG + custom UE plugins. Square Enix has used UE for HUD/menu while keeping engine cinematics in proprietary tech.
- **Stalker 2: Heart of Chornobyl** (GSC Game World, 2024) — UE5 UMG.
- **Bluepoint's Demon's Souls remake** (2020), **Returnal** (Housemarque, 2021), **Layers of Fear (2023)**, **Robocop: Rogue City (2023)** — all UMG.
- **Indie at scale:** thousands of UE-shipped titles use UMG for at least their menu UI; many use it for HUD as well.

Outside games:

- **Unreal Editor itself** is the largest single Slate UI in production — millions of widgets across hundreds of editor modes.
- **Twinmotion** (Epic's archviz product) — Slate-driven.
- **MetaHuman Animator** — Slate UI.
- Film/VFX pipelines (LED-volume real-time virtual production rigs) routinely use Slate-extended editor surfaces.

## CommonUI: the de-facto modern UMG add-on

CommonUI (originally Fortnite-internal, plugin-extracted around UE 4.27, polished through UE5) is the layer that takes UMG from "works for a single-platform game" to "works for a shipping cross-platform AAA title." It adds:

- **Input Routing System** — a stack of active widgets receives input first; widgets not on the active stack don't intercept. Solves the "multiple overlapping menus + which one consumes Escape" problem.
- **Cardinal Navigation** — gamepad / arrow-key spatial nav. The directional-navigation algorithm finds the best candidate in the chosen `CompassOctant` direction.
- **Activatable Widgets** — `UCommonActivatableWidget` carries a stacked lifecycle (`Activate` / `Deactivate`); pushes onto a stack, automatic input routing on top of stack, automatic restoration of previous focus when popped.
- **Platform-Aware Controller Icons** — `UCommonInputActionDataBase` swaps the icon for "Confirm" based on the active input device (Xbox A button vs PlayStation Cross vs Switch B vs keyboard Enter).
- **Common Bound Action Bar** — auto-renders the action bar at the bottom of the screen ("A — Confirm", "B — Back") based on the active widget's bindings.
- **Style Data Assets** — `UCommonButtonStyle`, `UCommonTextStyle` — shared styled `.uasset`s, swappable per-platform.

CommonUI is the layer where Buiy's "borrow the shape, leave the code" thesis most clearly applies. Buiy's focus model spec ([`foundation focus-model sub-spec`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.3) already commits to roving-tabindex + spatial gamepad nav + traps + restoration — exactly the CommonUI feature set.

## UMG MVVM plugin

The official UE 5.1+ MVVM plugin replaces "Bind Function" (per-frame poll) with a change-driven view-model binding. Authors define `UMVVMViewModelBase` subclasses with `FieldNotify`-marked properties; widget bindings re-evaluate only when the field changes. The plugin is the right answer to UMG's per-frame-binding tax — and a useful template for what Buiy's reactivity layer (open question: signals/computed/effects) should look like if it ever ships.

## Comparison vs other game-engine UI stacks

### vs Unity UGUI (2014–)

| | Slate + UMG | Unity UGUI |
|---|---|---|
| Runtime | C++ (Slate), Blueprint VM + C++ (UMG) | C# (Mono → IL2CPP) |
| Authoring | UMG Widget Blueprint Editor | Unity Inspector + RectTransform layout |
| Layout | Slate boxes/overlay/grid | RectTransform + LayoutGroup components |
| Text | HarfBuzz + ICU + FreeType | TextMesh Pro (SDF text rendering) |
| A11y | Limited (UE 4.22+) | Limited (third-party UAP plugin) |
| Asset format | `.uasset` Widget Blueprints | `.prefab` GameObject hierarchies |
| Shipping at scale | Fortnite, AAA UE titles | Most non-UE shipped games |

UGUI's `RectTransform` + `LayoutGroup` model is closer to CSS Flexbox than Slate's specialized boxes, but both stacks reach the same place: visual designer + property inspector + runtime data binding.

### vs Unity UI Toolkit (2019–)

Unity's newer UIToolkit (the UXML + USS + C# layer that replaces UGUI in the editor and is positioned to replace it in-game) is structurally **far closer to web** than Slate:

- **UXML** — XML-flavored markup (vs UMG's reflection-driven `.uasset`).
- **USS** — Unity Style Sheets, CSS-like.
- **VisualElement tree** — closer to DOM than Slate's `SWidget` tree.
- **Flexbox layout** via Yoga (Facebook's library, similar to Taffy).
- **PanelSettings + UI Document component** for in-game instantiation.

UI Toolkit is the existing-art closest to what Buiy aspires to (declarative + CSS-flavored + Flexbox). See cross-link: `../unity-ui/` (when written).

### vs Godot Control (2014–)

Godot's `Control` is the closest **OSS** sibling to UMG: GDScript-friendly, has its own visual editor, ships with the engine. Major differences from Slate:

- Single-stack — there is no programmer-only-C++ layer underneath; `Control` is the only UI vocabulary.
- GDScript scripting is the primary authoring language; C# / C++ are secondary.
- Layout is `Container`-based (similar shape: `VBoxContainer`, `HBoxContainer`, `GridContainer`); no Flexbox semantics.
- A11y story is also limited but improving — Godot added AT-SPI integration (PR #76829) for Linux a11y around 2023, putting it ahead of Unreal on the open-platform a11y axis.
- MIT-licensed and OSS-stewarded — adoption is broader in OSS, smaller in AAA.

### vs Bevy UI / Buiy

| | Slate + UMG | Bevy UI | Buiy |
|---|---|---|---|
| Runtime model | Two layers: C++ retained-mode Slate + UObject-wrapped UMG | ECS components (single layer) | ECS components (single layer) |
| Layout | Slate's own | Taffy (Flexbox + Grid + Block) | Taffy (Flexbox + Grid + Block) |
| Text | HarfBuzz + ICU + FreeType | cosmic-text (until 0.19) → parley + swash (0.19+) | cosmic-text (committed) |
| A11y | Limited platform bridges | AccessKit via `bevy_a11y` (megacomponent issue) | AccessKit-first, decomposed |
| Asset authoring | Widget Blueprints (`.uasset`) | (planned: BSN) | `.bsn` assets (committed) |
| License | Unreal EULA (5% royalty) | MIT-or-Apache | MIT-or-Apache (planned) |
| Steward | Epic Games | Bevy Foundation | TBD |
| Shipping | AAA at scale | Tiny Glade (not on bevy_ui), Foresight Spar | Pre-1.0 |

The takeaway: Slate proves the *shape* (declarative widgets + asset-based designer layer + custom renderer) ships at AAA. Buiy commits to the same shape with three changes — a single unified runtime instead of a parallel `S`/`U` stack, AccessKit-first instead of a11y-as-afterthought, and permissive OSS license instead of EULA.

### vs Coherent Gameface (HTML5-in-engine)

Coherent Gameface is a commercial **HTML/CSS-in-game** UI middleware widely used by AAA studios that prefer web tech over engine-native UI:

- **Atomic Heart**, **Star Citizen**, **Conan Exiles** — Gameface UI.
- Renders HTML/CSS via a Chromium-derived layout/render core.

Gameface exists because Slate's authoring is engineer-heavy and UMG's Blueprint VM has perf taxes. It's the "buy your way out of the engine UI question" option. Buiy is positioned to compete with this niche by being **web-aligned by default** — anchor positioning, container queries, full Flexbox + Grid, CSS-equivalent layout — without needing a Chromium dependency.

## Distinctive friction: the perpetual "Slate or UMG?" question

A recurring community pattern is the "should I use Slate or UMG?" question, surfacing in Epic forums roughly monthly across UE4 and UE5's combined twelve-year history. The community settles on roughly:

- **UMG by default.** Designer-friendly, Blueprint-friendly, asset-based, hot-reload via WBP recompile.
- **Slate for editor extensions.** UMG can't render editor windows or hook into editor UI infrastructure.
- **Slate for runtime cases UMG cannot reach.** Performance-critical HUDs that can't tolerate per-frame Blueprint cost, custom-rendered widgets, deep input customization.

The fact that this question still gets asked in 2026 — after twelve years of dual-stack life — is itself the strongest argument for Buiy's single-stack commitment.

## Sources

- Common UI Plugin — https://dev.epicgames.com/documentation/unreal-engine/common-ui-plugin-for-advanced-user-interfaces-in-unreal-engine
- Common UI Quickstart — https://dev.epicgames.com/documentation/en-us/unreal-engine/common-ui-quickstart-guide-for-unreal-engine
- Common UI: Action Bar — https://unrealist.org/commonui-actionbar/
- Common UI: Switchers and Tabs — https://unrealist.org/commonui-switchers-and-tabs/
- Unlocking Cross-Platform UI with Unreal's Common UI — https://pixelantgames.com/blog/unlocking-cross-platform-ui-with-unreals-commonui/
- UMG vs HUD vs Slate (UE forums) — https://forums.unrealengine.com/t/umg-vs-hud-vs-slate-what-is-better-for-developing/349732
- UMG or Slate? (UE forums) — https://forums.unrealengine.com/t/umg-or-slate/304385
- Optimization Guidelines for UMG — https://dev.epicgames.com/documentation/unreal-engine/optimization-guidelines-for-umg-in-unreal-engine
- Cross-link: [`../bevy-ui/comparisons.md`](../bevy-ui/comparisons.md), [`../bevy-ui/ecosystem.md`](../bevy-ui/ecosystem.md)
