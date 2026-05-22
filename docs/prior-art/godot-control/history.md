**Date:** 2026-05-22
**Status:** active
**Subject:** Godot — UI evolution from 1.0 (2014) through 4.6 (2026); founders, major UI inflection points, the Godot Foundation formation

# History

## Origin

Godot's roots reach back to **2001**, when [Juan Linietsky](https://github.com/reduz) and [Ariel Manzur](https://github.com/punto-) began building a custom engine in Argentina for in-house and contract game work. The engine was called variously *Larvotor*, *IT* (an internal name), and other monikers; it was used at Linietsky's studio OKAM Studio for commercial titles in Latin America for over a decade before public release. The name **Godot** (after Beckett's *Waiting for Godot*) was chosen for the public release.

## Public release: Godot 1.0 (2014-01-14)

Godot 1.0 launched on **January 14, 2014** under the **MIT license**, with the source published to GitHub. The 1.0 UI system shipped with **the Control + CanvasItem + scene-tree foundation we have today** — the architecture is fundamentally stable across the engine's entire public life. Anchors, offsets (then called margins), the Theme resource, BBCode in RichTextLabel, GDScript as the primary scripting language all existed at 1.0.

What didn't exist at 1.0: no BiDi, no complex-script text support, no AccessKit, no Vulkan renderer, no C# scripting, no GDExtension. These all accumulated over the next decade.

## Godot 2.x (2016 – 2018): GDScript polish, editor maturation

Godot 2.0 (February 2016) and 2.1 (August 2016) focused on editor UX, GDScript usability, and the asset pipeline. The Control system received incremental polish (FlowContainer arrived in this era, the Theme editor was rewritten) but no architectural changes. The community grew but remained niche.

## Godot 3.0 (2018-01-29): PBR + C# + Bullet physics

Godot 3.0 was the first "major" Godot in community consciousness:

- **PBR rendering** brought it closer to feature parity with commercial engines.
- **C# / .NET scripting** opened it to developers from Unity backgrounds. (C# support has been somewhat stop-start since — see [`distribution-and-governance.md`](distribution-and-governance.md).)
- **Bullet physics** replaced the in-house physics engine.
- **GDNative** — the ABI ancestor of GDExtension — landed, allowing C++ plugins.

The UI system was largely unchanged but inherited the broader engine improvements. Theme system + Control hierarchy + BBCode were stable.

## The 4.x rewrite

In **2019**, the team announced that 4.x would be a near-complete engine rewrite, with development split into two parallel teams: the 3.x maintenance line continued (3.1, 3.2, 3.3, 3.4, 3.5 over 2019–2022), while the 4.x team rebuilt the renderer (Vulkan), the text system (TextServer), and the audio pipeline. This was a multi-year effort.

The 4.x development cycle was contentious internally; some long-time contributors left. The team published [open development snapshots](https://godotengine.org/article/) throughout, and the contributor base expanded significantly via the Godot Foundation's funding (next section).

## Godot 4.0 (2023-03-01): Vulkan, TextServer, BiDi, complex scripts

Godot 4.0 shipped after **four years** of 4.x development. UI-relevant landmarks:

- **TextServer abstraction** (TextServerAdvanced + TextServerFallback). Pāvels Nadtočajevs (`@bruvzg`) led the work. **BiDi, complex graphemes, ligatures, multi-level font fallback, color emoji, variable fonts** all arrived at once.
- **Decoupled Font / FontSize** in the Theme system. Fonts no longer carry their size.
- **`LayoutMode` enum** on Control — editor metadata to make anchor+offset less intimidating.
- **`offset_*` properties** renamed from `margin_*` (3.x) to disambiguate from CSS-margin connotation.
- **Default theme refresh** — cleaner look, embedded-image-free.
- **Vulkan renderer** (Forward+ for desktop, Mobile for mobile, Compatibility for legacy via OpenGL ES). UI rendering benefited from the new renderer architecture but the Control hierarchy itself was unchanged.

The 4.0 release was the largest UI inflection point in Godot's history — the 9-year-late BiDi support transformed the engine's international viability.

## Godot 4.1 – 4.4 (2023 – 2025): incremental polish

- **4.1** (July 2023) — Animation improvements, particle system.
- **4.2** (November 2023) — Typed dictionaries, Movie maker mode.
- **4.3** (August 2024) — Editor theme refactor (more flexible presets, reduced default spacing), Wayland support, D3D12 backend on Windows.
- **4.4** (March 2025) — Physics interpolation, animation upgrades, no major UI changes.

Across this window, AccessKit work was in progress in pull requests but had not yet landed. The 4.x release cadence settled into ~8-month majors.

## Godot 4.5 (2025-09-XX): AccessKit lands

**Godot 4.5 introduced AccessKit-based screen-reader support** ([release notes](https://godotengine.org/releases/4.5/)), contributed by Pāvels Nadtočajevs. This is the second-largest UI inflection point in the engine's history (after 4.0's TextServer). Coverage as of 4.5: Project Manager + standard Control widgets complete; Inspector partial; full editor incomplete. Status explicitly marked **experimental**. See [`accessibility.md`](accessibility.md).

Other 4.5 features: stencil buffer, bent normal maps, shader baker (20× startup speedup on some platforms), abstract classes in GDScript, variadic arguments, internationalization live preview, visionOS export, WebAssembly SIMD, dedicated 2D navigation server.

## Godot 4.6 (2026-Q1): current as of 2026-05-22

Godot **4.6.2** released **April 1, 2026** ([Wikipedia: Godot game engine](https://en.wikipedia.org/wiki/Godot_(game_engine))). Adds standalone-library build mode (Godot embedded in other apps), modernized theme, AccessKit improvements toward editor coverage. Detailed 4.6 release notes are sparse in the public record as of writing.

## The Godot Foundation (2022-11)

The Godot project's legal home shifted from fiscal sponsorship under the Software Freedom Conservancy to its own **Stichting Godot Foundation** in **November 2022**. The Foundation is a Dutch non-profit (`Stichting` is the Netherlands' non-profit foundation legal form) and is the entity that:

- Holds the Godot trademark.
- Employs core Godot developers full-time (4–8 employees as of 2023, growing since).
- Receives donations and signs commercial partnerships.
- Coordinates roadmap planning.

The Foundation does **not** sell the engine (it remains MIT) but does receive grants (e.g., from Meta, Microsoft, Khronos) and partnership revenue (e.g., from W4 Games for console-port work). The structure deliberately separates the **engine project** (community-governed via PR review on GitHub) from the **commercial entity** (the Foundation as fiduciary). See [`distribution-and-governance.md`](distribution-and-governance.md).

Notable: the Foundation board / leadership has rotated; as of 2026 it includes the founders Juan Linietsky and Ariel Manzur in leadership roles, plus elected representatives from the contributor base.

## Forks and adjacencies

- **Redot Engine** — community fork of Godot 4.x launched in late 2024 in response to community frustrations with Foundation governance (specifically around developer DEI statements). Redot ships as a near-drop-in Godot fork; the UI / Control system is unchanged from upstream. Active but small.
- **W4 Games** — commercial company founded by ex-Godot core developers (Linietsky among them at one point, since stepped back from W4 to focus on the engine). Sells **console ports** (Switch, PS5, Xbox) and commercial support to Godot game studios. Funded by venture capital. Not a fork — collaborates with the Foundation.

## UI architecture stability

The remarkable observation across 12 years: **the Control + CanvasItem + Theme + Container architecture has not been rewritten.** Anchors + offsets, the C++-class-per-Container layout pattern, the Theme resource model, BBCode in RichTextLabel — these are all 1.0-era choices that have shipped through 4.6. The engineering has been *additive* (TextServer, AccessKit, type variations, LayoutMode) rather than *transformative*. This is unusual for a 12-year-old UI system and is a real strength of the original architecture.

The flip side: when a foundational choice is wrong (anchor+offset over CSS box model; BBCode over HTML; per-Container layout class over a generic layout engine), it persists for the engine's life. Godot has structurally chosen to live with its 2014 decisions rather than redesign.

## Implications for Buiy

- **The 4.x rewrite is the cautionary tale.** Godot needed a 4-year multi-team rebuild to add BiDi, decoupled FontSize, and a modern renderer. Designing those in from v1 is dramatically cheaper than retrofitting at year 9. Buiy's foundation commits to BiDi + AccessKit + CSS-grade layout from v1 partly because Godot's history demonstrates the retrofit cost.
- **MIT permissive posture works at scale.** Twelve years of MIT licensing did not prevent commercial uptake (Cassette Beasts, Dome Keeper, Brotato), commercial partnerships (W4 Games, Meta), or foundation viability. Validates Buiy's MIT-or-Apache stance.
- **Foundation governance scales.** The Godot Foundation (Stichting form, board-led, donation + partnership funded) is a working model for an open-source UI ecosystem that needs a legal entity for trademarks + employment + grants. Buiy doesn't need this structure today (we're a Bevy plugin) but the precedent is established.
- **Architectural stability is a feature.** Control hierarchy unchanged across 12 years means a Godot 1.0 plugin author can read modern Godot source and recognize the shape. Buiy's foundation aims at the same — small public-fielded components, clear sub-spec boundaries, no megacomponent-shaped reversals.

## Sources

- Godot Engine on Wikipedia — https://en.wikipedia.org/wiki/Godot_(game_engine)
- Godot 4.0 release announcement — https://godotengine.org/article/godot-4-0-sets-sail/
- Godot 4.5 release notes — https://godotengine.org/releases/4.5/
- Godot Foundation — https://godot.foundation/
- W4 Games — https://w4games.com/
- Software Freedom Conservancy — Godot's previous fiscal sponsor — https://sfconservancy.org/
- Redot Engine (community fork) — https://www.redotengine.org/
