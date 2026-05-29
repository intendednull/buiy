**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — timeline from the UE3 editor rewrite (~2010) through UE5 (2026)

# History

## Pre-Slate UE3 editor (~2003-2010)

UnrealEd, the editor that shipped with Unreal Engine 3, was built in **wxWidgets** on Windows. It worked but was a maintenance burden:

- wxWidgets bound Unreal to a specific cross-platform UI toolkit with its own paradigms.
- The editor's UI couldn't share rendering code with the engine — wxWidgets was its own world.
- Custom editor widgets required wxWidgets expertise on top of Unreal expertise.

Around 2010, Epic engineer **Nick Atamas** prototyped a replacement: a C++-only declarative UI framework that would render through Unreal's own renderer (RHI), share Unreal's input pipeline, and not require any third-party UI toolkit. That prototype became Slate.

## Slate's UE3→UE4 transition (~2010-2014)

Per [Tim Sweeney's "Classic Tools Retrospective" (Game Developer, 2018)](https://www.gamedeveloper.com/design/classic-tools-retrospective-tim-sweeney-on-the-first-version-of-the-unreal-editor):

> "[Nick] was prototyping a new UI layer in C++ that eventually became Slate ... We rewrote 100 percent of the Unreal Engine user interface in this unified way."

The UE4 development cycle (~2010-2014) used Slate as the **sole** UI toolkit. By UE4's public launch in **March 2014**:

- The entire Unreal Editor (level editor, Blueprint editor, content browser, material editor, every modal dialog) ran on Slate.
- Game-side UI was *also* expected to be authored in Slate; this was the only path for the first year-and-a-half of UE4.
- Slate was open in source-available form to every UE4 licensee from day one.

## UMG arrives (UE 4.5, November 2014)

After several months of community feedback that Slate's C++/macro authoring was inaccessible to non-engineers, Epic added **Unreal Motion Graphics** in UE 4.5 (November 2014). The 4.5 release notes:

> "Creating user interfaces has never been easier now that Unreal Motion Graphics is ready to use ... [UMG] is enabled by default and ready for wide use ... getting started with UMG is as simple as creating a new Widget Blueprint and building out UI in the editor from there."

UMG shipped in 4.5 with:

- The Widget Blueprint Editor (Designer + Graph tabs).
- The full `U`-prefix mirror of common Slate widgets.
- Default DPI-scaling rules for resolution-independent UI.
- Animation support — multiple per-WBP animations controllable from the Graph at runtime.

UMG was **not** a replacement for Slate; it sat on top of Slate from day one. Every UMG widget called `TakeWidget()` to construct a backing Slate widget at runtime. The pattern hasn't changed in 12 years.

## UE4 era (2014-2022): consolidation

Over the UE4 cycle, the Slate+UMG stack matured rather than transformed:

- **UE 4.7-4.10** — UMG list views, scroll boxes, widget switcher, animation polish.
- **UE 4.13** — Slate Editor Style refactor; `FSlateStyleRegistry` becomes the canonical style-discovery API.
- **UE 4.20** — Compositional UMG (multi-named-slot user widgets); RichTextBlock arrives.
- **UE 4.22 (2019)** — **First screen-reader support** (Windows third-party screen readers, common widgets only). Long-time community feature request.
- **UE 4.23 (2019)** — Mobile a11y (iOS VoiceOver, Android TalkBack) added.
- **UE 4.24 (2019)** — HTML5 target deprecated (Unreal's web story is sunsetted; web a11y never materializes).
- **UE 4.26-4.27 (2020-2021)** — CommonUI plugin moves toward general availability; UMG MVVM plugin in development.

By the end of UE4, the stack was: Slate for tools + advanced custom widgets, UMG for game UI, CommonUI for cross-platform shipping concerns (input routing, controller icons, focus management), MVVM for view-model data binding. Four interacting layers; each shipping team picked the subset they needed.

## UMG MVVM (UE 5.1, 2022)

The official Model-View-ViewModel plugin lands in UE 5.1 (November 2022). It replaces the per-frame "bound attribute" anti-pattern with declarative change-driven data sources. The MVVM plugin is the third major add-on layer above UMG and the modern Epic recommendation for data binding.

## UE5 era (2022-2026): Slate stays, the layers above grow

- **UE 5.0 (April 2022)** — Slate is rebranded slightly (`FAppStyle` replaces `FEditorStyle` as the canonical editor style; legacy `FEditorStyle` still works as an alias). No fundamental Slate architecture changes.
- **UE 5.1-5.3 (2022-2023)** — UMG MVVM, UMG Viewmodel-based data binding, performance work for large widget hierarchies.
- **UE 5.4 (April 2024)** — Refinements; no Slate replacement on the horizon.
- **UE 5.5-5.6 (2024-2025)** — Refinements continue; Nanite hits production; Slate/UMG continues unchanged.
- **UE 5.7 (2026)** — Current docs version as of May 2026. Slate's architecture is essentially identical to 2014.

Slate's stability — fifteen years of essentially the same C++ macro DSL, the same widget hierarchy, the same paint pipeline — is itself notable. It is among the longest-stable retained-mode UI frameworks in the industry. The trade-off is that Slate's *ergonomics* (the `SNew` macro chains, `TSharedRef` lifetime management, raw-pointer event handler captures) are also frozen at 2014 C++ vintage. Epic hasn't modernized the syntax in 12 years.

## What's not happening

A few projects worth noting that didn't materialize:

- **No "Slate 2.0".** Despite occasional community calls for a cleaner reactive layer, Epic has not shipped (or, as of public information, prototyped) a Slate replacement.
- **No Slate-on-wgpu / Slate-via-Vello.** Slate's renderer is Unreal's RHI; there is no plan to swap renderers.
- **No Slate as a standalone library.** Slate is structurally inseparable from UE — `FSlateApplication` ties into Unreal's tick / threading / asset / config systems. There is no "Slate-only" build of Unreal that a third party could embed.

## Timeline summary

| Year | Event |
|---|---|
| ~2010 | Nick Atamas prototypes Slate in C++ during UE3. |
| 2014-03 | UE4 launches. Entire editor on Slate. Game UI = Slate-only. |
| 2014-11 | **UE 4.5** ships UMG. Widget Blueprints + Designer tab + animations. |
| 2016 | UE 4.13 — `FSlateStyleRegistry` matures. |
| 2019 | UE 4.22 — first screen-reader bridge (Windows). |
| 2019 | UE 4.23 — mobile a11y (iOS / Android). |
| 2019 | UE 4.24 — HTML5 target deprecated. |
| 2020-2021 | UE 4.26-4.27 — CommonUI plugin matures. |
| 2022-04 | **UE 5.0** — Slate carries through largely unchanged. |
| 2022-11 | UE 5.1 — Official MVVM plugin. |
| 2024 | UE 5.4. |
| 2026-05 | UE 5.7 — Slate architecturally identical to 2014. |

## Sources

- Classic Tools Retrospective: Tim Sweeney on the First Version of the Unreal Editor — https://www.gamedeveloper.com/design/classic-tools-retrospective-tim-sweeney-on-the-first-version-of-the-unreal-editor
- Unreal Engine 4.5 Release Notes — https://www.unrealengine.com/en-US/blog/unreal-engine-45-released
- Unreal Engine 5.4 Release Notes — https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5.4-release-notes
- Unreal Engine 4 — Wikipedia — https://en.wikipedia.org/wiki/Unreal_Engine_4
- The Slate UI Framework Part 1 (Gerke Max Preussner) — https://de45xmedrsdbp.cloudfront.net/Resources/files/slateTutorials_westcoast-1963123470.pdf
- Supporting Screen Readers in Unreal Engine — https://dev.epicgames.com/documentation/en-us/unreal-engine/supporting-screen-readers-in-unreal-engine
- Common UI Plugin — https://dev.epicgames.com/documentation/unreal-engine/common-ui-plugin-for-advanced-user-interfaces-in-unreal-engine
