**Date:** 2026-05-22
**Status:** active
**Subject:** egui — terms used across the corpus

# Glossary

Terms specific to egui (and the immediate-mode paradigm it embodies) used across this prior-art folder. Cross-link to the evidence file where each term is load-bearing.

## Core types and APIs

- **`egui`** — the crate; current name since 2020-08-10. Pronounced "e-gooey" per Emil. See [`history.md`](history.md).
- **`Emigui`** — the 2018–2020 project name ("Emil's Immediate-mode GUI"). Renamed to `egui` on 2020-08-10. See [`history.md`](history.md).
- **`epaint`** — the egui-internal 2D graphics + tessellation crate. Owns the font atlas, glyph rasterization, `Galley`, the tessellator, and the `ClippedShape` / `ClippedPrimitive` types. The "renderer" everyone is asked about is actually `epaint`. See [`architecture.md`](architecture.md), [`text-rendering.md`](text-rendering.md).
- **`Context`** (`egui::Context`) — the per-UI-surface root object. Holds the font atlas, `Memory`, current `Style` + `Visuals`, last frame's tessellated output, plugin slots, and pass-loop state. Internally `Arc<RwLock<ContextImpl>>` — cheap to clone. See [`architecture.md`](architecture.md) § "The central type."
- **`Ui`** (`egui::Ui`) — the layout cursor passed to widget code inside `Panel`/`Window`/closure scopes. Allocates rectangles, paints, advances the cursor, and returns `Response`s. Since 0.34.0, `Ui` derefs to `Context` (eliminating `ui.ctx()` indirection). See [`architecture.md`](architecture.md) § "The `Ui` struct."
- **`Response`** (`egui::Response`) — the return value of every widget call. Carries `clicked()`, `double_clicked()`, `secondary_clicked()`, `hovered()`, `dragged()`, `drag_delta()`, `has_focus()`, `interact_rect`, pointer coordinates. The property-shape contract of egui widgets. See [`api-surface.md`](api-surface.md).
- **`Sense`** (`egui::Sense`) — the interactivity declaration for a widget: `Sense::hover()`, `Sense::click()`, `Sense::drag()`, `Sense::focusable_noninteractive()`. Composed via bitwise-OR. See [`api-surface.md`](api-surface.md).
- **`RawInput`** (`egui::RawInput`) — the per-frame input bundle the host backend passes into `begin_pass(...)`: events (keyboard, pointer, IME), modifiers, time, viewport state. See [`architecture.md`](architecture.md) § "Per-frame run."
- **`FullOutput`** (`egui::FullOutput`) — the per-frame output bundle from `end_pass()`: `platform_output` (cursor/clipboard/IME), `textures_delta` (atlas updates), `shapes` (tessellated draw list), `pixels_per_point`, `viewport_output`. The host backend's only job is to consume this. See [`architecture.md`](architecture.md) § "Output."
- **`ClippedShape`** / **`ClippedPrimitive`** (`epaint`) — pre- and post-tessellation entries in the draw list. Each carries a clip rectangle plus a shape (mesh, path, text). See [`architecture.md`](architecture.md).
- **`Id`** (`egui::Id`) — a hash-derived stable identity for a widget across frames. Constructed from `Id::new(source)` or `Id::with(child)`. The `Id`-collision-in-loops pitfall is the single most common community-Discord question. See [`architecture.md`](architecture.md) § "The Id system" and [`critiques.md`](critiques.md) § "`Id` system pitfalls."
- **`Memory`** (`egui::Memory`) — the typed map keyed by `Id` that holds cross-frame widget state: text-edit cursor, scroll offset, collapsing-header open state, window position, drag state, focus state, animation state. The *only* retained surface egui has. See [`architecture.md`](architecture.md) § "Memory model" and [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md).
- **`push_id`** (`Ui::push_id`) — the canonical workaround for `Id` collisions in loops: `ui.push_id(i, |ui| { ... })` mixes an explicit per-item seed into the parent `Id`. See [`architecture.md`](architecture.md), [`critiques.md`](critiques.md).
- **`request_discard`** (`Context::request_discard`) — the multi-pass termination request. A widget calls this to ask the pass loop to re-run the user closure with current pass's output available. Capped by `Options::max_passes` (default 2). See [`architecture.md`](architecture.md) § "Per-frame run."
- **Multi-pass rendering** — the mode where a single frame may run the user closure multiple times to settle layout (auto-sizing tooltips, popovers needing content size before placement). Introduced in 0.29 (2024-09); default in upstream egui from 0.34+ via `Context::run_ui` and via `bevy_egui` defaults since 0.35.0. See [`architecture.md`](architecture.md), [`history.md`](history.md).

## Style, theming, and animation

- **`Style`** (`egui::Style`) — top-level styling container on `Context`. Holds `Visuals` plus `spacing`, `interaction`, `text_styles`, `wrap_mode`, `animation_time`, debug overlays. See [`styling-and-theming.md`](styling-and-theming.md).
- **`Visuals`** (`egui::Visuals`) — the color/treatment subset of `Style`. Holds the dark/light split, button backgrounds, text colors, hyperlink colors, selection colors, hover/active states, window frame styling. `Visuals::dark()` and `Visuals::light()` are the two presets. See [`styling-and-theming.md`](styling-and-theming.md).
- **`TextStyle`** (`egui::TextStyle`) — named text style key (`Body`, `Heading`, `Monospace`, `Button`, `Small`, `Name(custom)`). Maps to a `FontId { size, family }` via `Style::text_styles`. See [`text-rendering.md`](text-rendering.md), [`api-surface.md`](api-surface.md).
- **`Spacing`** (`egui::Spacing`) — the non-color sub-struct of `Style` holding item spacing, indent width, slider grab radius, button padding, etc. See [`styling-and-theming.md`](styling-and-theming.md).
- **`animate_value`** / **`animate_bool`** (`Context::animate_value`, `Context::animate_bool`) — egui's two animation primitives. Linear eased interpolation of an `f32` (or bool-cast-to-float) over a duration keyed by `Id`. No keyframes, no springs, no transition system. See [`critiques.md`](critiques.md) § "Animation / transition primitives are weak."

## Lifecycle and API patterns

- **`begin_pass`** / **`end_pass`** (`Context::begin_pass`, `Context::end_pass`) — the low-level frame lifecycle. `begin_pass(raw_input)` clears per-frame state; `end_pass()` runs layout finalization, tessellates shapes, and returns `FullOutput`. See [`architecture.md`](architecture.md).
- **`Context::run`** — the higher-level wrapper around `begin_pass` / `end_pass` that handles the multi-pass loop automatically.
- **`Context::run_ui`** (0.34.0+) — a higher-level entrypoint exposing a single whole-app `Ui` as the primary entry. `Ui` derefs to `Context` here, eliminating `ui.ctx()` indirection. See [`architecture.md`](architecture.md).
- **Plugin trait API** (`egui::Plugin`, 0.33.0+) — trait-based plugin system replacing older callback hooks. Hooks: `on_begin_pass`, `input_hook`, `output_hook`, `on_end_pass`, `on_widget_under_pointer` (0.33.2). Plugins are owned by `Context` and run at fixed lifecycle points; state lives on the plugin struct. See [`architecture.md`](architecture.md) § "The plugin trait."
- **Panel API** (`egui::Panel`, 0.34.0+) — unified panel primitive consolidating `SidePanel::left` / `SidePanel::right` / `TopBottomPanel::top` / `TopBottomPanel::bottom` / `CentralPanel::default` into one type with directional config. Legacy types remain as aliases. See [`api-surface.md`](api-surface.md) § "Container widgets."

## Container and layout primitives

- **`Window`** — floating, draggable, resizable container with chrome. Position/size persisted in `Memory`.
- **`Area`** — bare positioned region without window chrome. Building block for tooltips, popups, drag overlays.
- **`Panel`** (since 0.34.0) — unified resizable panel docked to a side or center (see Panel API above).
- **`SidePanel`** / **`TopBottomPanel`** / **`CentralPanel`** — legacy panel aliases pre-0.34.0; still present in 0.34.x.
- **`ScrollArea`** — clipped scrollable region. `ScrollArea::vertical()`, `::horizontal()`, `::both()`. Supports virtualization via `.show_rows(...)`.
- **`CollapsingHeader`** — disclosure-triangle + collapsible content. Open state persisted in `Memory`.
- **`Grid`** — labeled-grid layout primitive (typically used for forms).
- **`Frame`** — styled container box with optional border, fill, rounding, shadow.

## Text rendering

- **`Galley`** (`epaint::text::Galley`) — the unit of cached text layout. Once a `(text, font, size, wrap)` tuple is shaped, the resulting glyph positions + atlas references are cached and reused. See [`text-rendering.md`](text-rendering.md).
- **`FontDefinitions`** (`egui::FontDefinitions`) — the font-registration API. Registers font files (TTF/OTF) and assigns them to families (`FontFamily::Proportional`, `FontFamily::Monospace`, custom).
- **`ab_glyph`** — Alex Butler's pure-Rust font parser. The pre-0.34.0 text-rendering stack (no shaping, no hinting, no variable fonts). Replaced in 0.34.0. See [`text-rendering.md`](text-rendering.md).
- **`skrifa`** — Linebender's Rust port of FreeType's font-parsing primitives. The same crate used by the Vello renderer. egui's font parser since 0.34.0 (2026-03-26). See [`text-rendering.md`](text-rendering.md), [`open-problems.md`](open-problems.md) § "egui's relationship to the Linebender stack."
- **`vello_cpu`** — Linebender's CPU-only Vello variant for glyph rasterization. egui's rasterizer since 0.34.0. See [`text-rendering.md`](text-rendering.md).

## Backends and sub-crates

- **`eframe`** — the official "batteries-included" runtime crate. Wraps `egui-winit` + (`egui-wgpu` or `egui_glow`) + a main loop. Runs on web, Linux, macOS, Windows, Android. The default choice for standalone egui apps. See [`distribution.md`](distribution.md), [`architecture.md`](architecture.md) § "Backend abstraction."
- **`egui-winit`** — winit input forwarding crate. Translates winit events to `egui::RawInput`. Used by eframe and by hand-rolled native backends.
- **`egui-wgpu`** — wgpu renderer for `FullOutput`. Used by eframe on most targets.
- **`egui_glow`** — OpenGL/WebGL renderer via the `glow` crate. Smaller binary footprint on web than wgpu.
- **`egui_extras`** — second-party crate with image loaders (PNG/JPEG/SVG), `Table` widget, `DatePickerButton`. Officially adjacent but not in `egui` core. See [`distribution.md`](distribution.md).
- **`egui_demo_lib`** — the demo app code, shipped as a library so other tools can embed the demo.
- **`egui_plot`** — immediate-mode 2D plotting, in its own repository (`emilk/egui_plot`). 6,765,876 lifetime downloads, latest 0.35.0. The de facto standard for "I need a graph in my Rust app." See [`ecosystem.md`](ecosystem.md), [`api-surface.md`](api-surface.md).
- **No Skia backend.** egui's renderer is `epaint`, a custom tessellator targeting textured triangles with scissor clipping. Skia would be possible but adds a heavyweight C++ dep egui has deliberately avoided. See [`architecture.md`](architecture.md) § "Backend abstraction."

## Accessibility

- **AccessKit** — the cross-platform Rust crate that wraps UIA (Windows), AT-SPI (Linux), and NSAccessibility (macOS) into a single tree-update API. egui's a11y bridge. **Always-on in egui since 0.34.0** (2026-03-26); opt-in from 0.20.0 (2022-12-08) onward. See [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) § "Stable accessibility tree," [`history.md`](history.md), [`critiques.md`](critiques.md).
- **APG** — WAI-ARIA Authoring Practices Guide. Specifies 30+ widget patterns (combobox, listbox, menu, tabs, treeview, etc.) each with a defined keyboard contract. egui's combobox/menu/tabs implementations are approximate, not APG-compliant. See [`open-problems.md`](open-problems.md) § "Full ARIA APG / WCAG 2.2 AA conformance."
- **ACCNAME 1.2** — W3C accessible-name computation spec. Defines the precedence chain (aria-labelledby > aria-label > text content > title). egui resolves a single string per widget, not the full chain. See [`open-problems.md`](open-problems.md).
- **WCAG 2.2 AA** — Web Content Accessibility Guidelines, Level AA. The conformance floor Buiy targets. egui has no CI gates or runtime constraints enforcing WCAG SCs. See [`open-problems.md`](open-problems.md).

## People and projects

- **Emil Ernerfeldt** (`@emilk`) — egui's architect and lead maintainer. Started Emigui as a hobby project on a train on 2018-11-04. Now co-founder + employee at Rerun.io. See [`history.md`](history.md), [`governance.md`](governance.md).
- **Casey Muratori** — Then at Insomniac Games (best known for the Ratchet & Clank tools pipeline). Published *Immediate-Mode Graphical User Interfaces* in 2005 — the forum-thread + video that originated the immediate-mode paradigm. See [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) § "History."
- **Omar Cornut** — Media Molecule (now ex-Tequila Works). Published Dear ImGui in 2014 as the C++ single-header library implementing Muratori's idea. Within five years it became the de facto debug-UI library for the entire commercial game industry. egui's closest sibling. See [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md).
- **Dear ImGui** — Omar Cornut's C++ immediate-mode GUI library. The industry-standard debug-UI library inside Unity, Unreal, Frostbite, Blizzard, EA, and every game studio with internal tools. Not the same as egui; sibling architectures targeting different language ecosystems. See [`comparisons.md`](comparisons.md).
- **Rerun** (Rerun.io) — streaming-data visualizer for ML / robotics teams. Emil's company since 2022; commercial steward of egui. The Rerun Viewer is the flagship production egui app. The "egui at scale" counterexample to "egui doesn't scale" — but Rerun's workload (streaming data, mostly-3D viewports with egui chrome) amortizes the immediate-mode cost in a way game UIs don't. See [`ecosystem.md`](ecosystem.md), [`governance.md`](governance.md).
- **Embark Studios** — Emil's former employer (2019–2022). Internal dogfooding context for egui's maturation; uses egui for world editors, asset browsers, debug overlays, dev dashboards. See [`history.md`](history.md), [`ecosystem.md`](ecosystem.md).
- **bevy-inspector-egui** — Jakob Hellermann's reflection-driven Bevy world inspector. 1.22M downloads; the canonical bevy_egui consumer. Walks the ECS via Bevy reflection and emits egui widgets per field. See [`ecosystem.md`](ecosystem.md) and the [`../bevy-egui/`](../bevy-egui/) folder.
- **Linebender** — the umbrella organization for Druid → Xilem, Parley, Vello, skrifa. Maintains the Rust stack egui has begun absorbing (skrifa + vello_cpu since 0.34.0; parley mentioned in roadmap hints). See [`text-rendering.md`](text-rendering.md), [`open-problems.md`](open-problems.md) § "egui's relationship to the Linebender stack."

## Sources

- egui repository — https://github.com/emilk/egui
- egui CHANGELOG @ master — https://raw.githubusercontent.com/emilk/egui/master/CHANGELOG.md
- egui crates.io — https://crates.io/crates/egui
- egui rustdoc — https://docs.rs/egui/latest/egui/
- Rerun.io — https://www.rerun.io/
- AccessKit — https://accesskit.dev
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- ACCNAME 1.2 — https://www.w3.org/TR/accname-1.2/
- Linebender — https://linebender.org
- Casey Muratori — https://caseymuratori.com/blog_0001
- Dear ImGui — https://github.com/ocornut/imgui
- Sibling evidence files: [`architecture.md`](architecture.md), [`api-surface.md`](api-surface.md), [`comparisons.md`](comparisons.md), [`critiques.md`](critiques.md), [`distribution.md`](distribution.md), [`ecosystem.md`](ecosystem.md), [`governance.md`](governance.md), [`history.md`](history.md), [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md), [`open-problems.md`](open-problems.md), [`styling-and-theming.md`](styling-and-theming.md), [`text-rendering.md`](text-rendering.md)
- bevy_egui glossary (cross-corpus) — [`../bevy-egui/glossary.md`](../bevy-egui/glossary.md)
