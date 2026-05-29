**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — system-specific terms used across this corpus

# Glossary

Definitions for bevy_egui- and egui-specific identifiers, type names, and ecosystem terms used throughout this corpus. Cross-link liberally; do not duplicate definitions in evidence files — point at this glossary instead.

## Paradigm terms

- **Immediate-mode** — UI architecture where the widget tree is rebuilt from imperative function calls every frame. There is no persistent widget tree between frames; widgets are emitted by `ui.button(...)` / `ui.label(...)` etc. and return a `Response` describing what happened to them this frame. Lineage: Casey Muratori's 2005 forum sketch, Omar Cornut's Dear ImGui (2014), Emil Ernerfeldt's egui (2018+, Rust-native). See [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md).
- **Retained-mode** — UI architecture where the widget tree is a persistent data structure that survives between frames. Widgets are entities / nodes / objects with stable identity; changes flow through diffing, change-detection, or observers. The web DOM, native OS toolkits, `bevy_ui`, and Buiy are all retained-mode. See [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md) § "When retained-mode wins."
- **Pure immediate mode** — Emil Ernerfeldt's term for egui's design: no callbacks, no event-driven model. The user writes the UI as straight-line code each frame; widget interactions are inspected via the returned `Response`. See [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md) § "egui's stated philosophy."

## Crates and projects

- **egui** — Upstream immediate-mode Rust GUI library by Emil Ernerfeldt. Started ~2018 as **Emigui** (portmanteau of Emil's initials + "GUI"), renamed **egui** in 2020. Pronunciation: "e-gooey." Sponsored by Rerun.io. License: MIT OR Apache-2.0. See [`history.md`](history.md) § "egui upstream genesis."
- **Emigui** — egui's original name pre-2020. Some old documentation, repositories, and forum posts still reference this name. See [`history.md`](history.md).
- **bevy_egui** — Third-party Bevy plugin (vladbat00) wrapping egui. Bridges Bevy input/render/ECS into egui's per-frame loop. License: MIT (not dual). Latest stable 0.39.1 (2026-02-06).
- **bevy-inspector-egui** — Jakob Hellermann's reflection-driven Bevy world inspector built on bevy_egui. The canonical downstream consumer of bevy_egui (1.2M lifetime downloads). Every Bevy tutorial that wants to show a debug overlay introduces it. See [`ecosystem.md`](ecosystem.md) § "Canonical consumer."
- **eframe** — egui's official native + WASM runtime shell. Hosts egui outside any game engine. Rerun's viewer runs on eframe. See [`ecosystem.md`](ecosystem.md) § "egui's own ecosystem (non-Bevy)."
- **egui_extras** — Companion crate from emilk with `Table`, `DatePickerButton`, async `Image::from_uri`, and other extensions that didn't fit upstream. See [`api-surface.md`](api-surface.md) § "Other API surfaces."
- **egui_plot** — 2D plotting (lines, points, bars). Originally in-tree, now separate. Production-quality, widely used. See [`api-surface.md`](api-surface.md).
- **egui_tiles** — Tiling layout engine published by `rerun-io`. Powers Rerun's multi-pane viewer. Not a default egui dependency.
- **Rerun** (Rerun.io) — Computer-vision / robotics-visualization SDK co-founded by Emil Ernerfeldt in 2022. Its viewer is built end-to-end on egui at production scale (multi-pane, streaming data, performance-sensitive). The primary commercial backer of egui development. The legitimate "egui at scale" counterexample, but its streaming-data workload amortizes immediate-mode cost in a way game UIs don't. See [`history.md`](history.md) § "Rerun.io stewardship," [`use-cases.md`](use-cases.md) § "Productivity apps."
- **GPUI** — Nathan Sobo's bespoke retained-mode Rust UI framework. **Zed (zed.dev) is on GPUI, NOT egui.** Common misconception. GPUI gets a separate prior-art folder. See [`use-cases.md`](use-cases.md).
- **`bevy_egui_kbgp`** — Third-party crate adding keyboard / gamepad navigation overlay for bevy_egui. Solves the focus-on-gamepad gap that vanilla egui handles poorly.
- **`catppuccin/egui`** — Third-party theme crate matching the Catppuccin color palette. Example of theming over egui's `Visuals` system (precomputes complete `Visuals` instances).
- **`egui_flex`** — Third-party Flexbox-style layout for egui. egui's native layout is much simpler than CSS Flexbox; this crate adds it.

## Plugin, contexts, render integration

- **`EguiPlugin`** — bevy_egui's user-facing entry point. Three configuration fields: `enable_multipass_for_primary_context: bool`, `ui_render_order: UiRenderOrder` (gated on `bevy_ui` feature), `bindless_mode_array_size: Option<NonZero<u32>>` (gated on `render` feature). See [`architecture.md`](architecture.md) § "The plugin struct."
- **`EguiContext`** — Per-camera component holding an `egui::Context`. As of 0.35.0 (2025-06-30), every active camera carries its own context (was per-window before). See [`architecture.md`](architecture.md) § "Per-window / per-camera contexts."
- **`PrimaryEguiContext`** — Marker component on the first camera spawned, identifying it as the primary egui context.
- **`EguiContexts`** — System parameter providing ergonomic access to one or more `EguiContext` components from a Bevy system. The typical user API: `let ctx = contexts.ctx_mut().unwrap()` to get the primary `&mut egui::Context`. See [`api-surface.md`](api-surface.md).
- **`EguiPrimaryContextPass`** — The schedule label for systems that emit UI into the primary context, in multi-pass mode. Added in 0.34.0 / 0.35.0. The pass-loop system in `PostUpdate` may run this schedule multiple times per frame if egui requests a relayout. See [`architecture.md`](architecture.md) § "Multi-pass vs single-pass."
- **`EguiMultipassSchedule`** — Component attached to a non-primary `EguiContext` camera, declaring which schedule label its UI systems live in. Each non-primary context must declare its own schedule; sharing one across contexts is not supported.
- **`EguiInput`** — Per-context component holding `egui::RawInput` events being accumulated for the next `begin_pass`. Written by the `EguiPreUpdateSet::ProcessInput` system set during `PreUpdate`.
- **`EguiRenderOutput`** — Per-context resource holding egui's tessellated `ClippedPrimitive` paint jobs after `EguiPostUpdateSet::ProcessOutput` runs. Extracted to the render world for drawing.
- **`EguiManagedTextures`** — Resource holding the bevy_egui-managed `egui::TextureId::Managed(...)` mapping to Bevy `Assets<Image>`. The font atlas lives here.
- **`EguiUserTextures`** — Resource for user-supplied textures (RenderTargets piped into egui images). `egui_user_textures.add_image(handle)` returns a `TextureId::User(...)` for use in `egui::Image::new(texture_id, size)`.
- **`EguiRenderToImage`** — Component on a camera that targets a `RenderTarget::Image` — used for diegetic / in-world UI surfaces. See [`integration.md`](integration.md) § "Render-to-texture surfaces."
- **`EguiPickingOrder`** — Resource (since 0.39.0) setting the priority of bevy_egui's `bevy_picking` backend. Replaces the deprecated `PICKING_ORDER` const. Default 0.6 (above bevy_ui's default backend) or 0.4 (below) depending on `ui_render_order`. See [`architecture.md`](architecture.md) § "Picking integration."
- **`EguiClipboard`** — Bevy resource bridging egui's clipboard requests to the OS clipboard via `arboard` (desktop) or `web-sys` (WASM). Gated on the `manage_clipboard` feature.
- **`NodeEgui::EguiPass`** — Render-graph node label for bevy_egui's render pass. Inserted into both `Core2d` and `Core3d` core graphs. Draws after the main pass and before upscaling. See [`architecture.md`](architecture.md) § "Render-graph integration."
- **`SubGraphEgui`** — Render-graph subgraph containing bevy_egui's render nodes. Gated on the `render` feature.
- **`UiRenderOrder`** — Enum (gated on `bevy_ui` feature) controlling where the egui pass sits relative to bevy_ui's pass: `AfterUi` (default, draw over bevy_ui) or `BeforeUi` (draw under bevy_ui). Configurable since 0.36.0 (2025-08-04). See [`integration.md`](integration.md) § "Coexistence with bevy_ui."

## egui core types

- **`egui::Context`** — The retained surface egui keeps between frames: font atlas, `Memory` map (per-widget state keyed by `Id`), previous frame's tessellated output. The only thing that survives frames in immediate-mode. See [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md) § "What gets erased."
- **`egui::Ui`** — Layout-and-emission cursor. Every widget call takes `&mut Ui` and returns a `Response`. Acquired inside a `Window`/`Area`/`Panel`/etc. closure. The closure runs eagerly; `.show()` *is* the build. See [`api-surface.md`](api-surface.md) § "Acquiring a `&mut egui::Ui`."
- **`egui::Response`** — Returned by every widget call. Reports interaction state: `.clicked()`, `.hovered()`, `.double_clicked()`, `.dragged()`, `.changed()`, etc. The immediate-mode equivalent of a retained-mode observer.
- **`egui::RawInput`** — Per-frame input bundle egui consumes via `Context::begin_pass(raw_input)`. bevy_egui populates this each `PreUpdate` from Bevy input events.
- **`egui::Id`** — Stable widget identity derived from the call-site context (parent UI's `Id` + layout cursor + widget label or explicit `Id::new(...)`). The key into `Memory`. Collides with identical-label widgets in dynamic loops — `ui.push_id(i, |ui| {...})` is the workaround. See [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md) § "The id system."
- **`egui::Memory`** — Map keyed by `Id` holding per-widget state across frames: text-edit cursor position, scroll offset, collapsing-header open state, window position, drag state. The retained piece inside an otherwise immediate-mode system.
- **`egui::Style`** — Composite styling struct on `Context`: holds `Visuals`, `Spacing`, `text_styles`, `interaction`, `animation_time`, etc. Mutated via `ctx.style_mut()`.
- **`egui::Visuals`** — Flat struct of colors / shapes / strokes / shadows / `WidgetVisuals` per state (inactive, hovered, active, open, noninteractive). Built-in `Visuals::dark()` and `Visuals::light()`. No tokens, no cascade, no specificity, no state-driven styles in the data model. See [`critiques.md`](critiques.md) § "Styling limitations."
- **`egui::Spacing`** — Item spacing, button padding, slider width, indent, scroll-bar width, window margin.
- **`egui::FullOutput`** — Per-frame output of `Context::end_pass()`: textures-delta, `ClippedPrimitive` list, `PlatformOutput`.
- **`egui::ClippedPrimitive`** — Tessellated triangle list with a per-primitive clip rect and texture ID. egui's render output that bevy_egui submits to the render graph.
- **`egui::PlatformOutput`** — Per-frame platform-side output: cursor icon, clipboard write requests, IME state, open-URL requests, copy-to-clipboard, virtual-keyboard hint. bevy_egui's `EguiPostUpdateSet::PostProcessOutput` applies these to Bevy.
- **`egui::PaintCallback`** — Custom render-pass injection point inside egui paint (since egui upstream 0.20-ish, bevy_egui 0.29 / 2024-08-18). Apps inject custom wgpu shaders inside egui's draw stream. Low-level — the analog of bevy_ui's `UiMaterial` but considerably more raw.
- **`epaint::Tessellator`** — egui's tessellator. Converts shape primitives into vertex/index buffers each frame. bevy_egui does *not* re-tessellate; it forwards `ClippedPrimitive` output directly.

## Rendering modes

- **Multi-pass mode** — Default since 0.35.0 (2025-06-30); only mode going forward. UI systems live in the `EguiPrimaryContextPass` schedule (or a custom `EguiMultipassSchedule`). The pass-loop system in `PostUpdate` calls `world.run_schedule(...)` once, checks `egui::Context::wants_to_repaint_immediately()`, and re-runs if egui needs another pass to settle layout. Required for auto-sizing tooltips, popovers that measure their content before placement, anchor-positioned overlays. See [`architecture.md`](architecture.md) § "Multi-pass vs single-pass."
- **Single-pass mode** — Legacy mode, **DEPRECATED** since 0.35.0. UI systems run in ordinary `Update`. Cannot iterate inside one frame. Set `enable_multipass_for_primary_context: false` to opt back in (not recommended).
- **Bindless texture mode** — Render path option since 0.37.0 (2025-10-01) for large texture sets (many user textures). Configured via `bindless_mode_array_size`.
- **Partial texture update** — Optimization since 0.38.0: egui can dirty a sub-rect of the font atlas (when a new glyph is shaped) and bevy_egui uploads only the changed region.

## Picking / input

- **`capture_pointer_input_system`** — bevy_egui system that suppresses `bevy_picking` events when a pointer is over an egui widget. Gated on the `picking` feature.
- **Mesh-picking** — Pattern (since 0.35.0) where a 3D mesh whose surface is an egui render-target receives pointer hits via `bevy_picking`, which are forwarded into that mesh's egui context as a virtual pointer position. See [`integration.md`](integration.md) § "Render-to-texture surfaces."
- **Diegetic UI** — UI rendered into the 3D world as a textured surface — an in-game computer terminal, holographic display, vehicle dashboard, etc. egui supports this via render-to-texture + mesh-picking (0.35.0). Buiy's `buiy_3d` subsystem targets the same use case (foundation [`cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) § 3.17).

## Accessibility

- **AccessKit** — Cross-platform accessibility tree library (https://accesskit.dev). Provides `TreeUpdate` diffs that platform adapters (`accesskit_windows`, `accesskit_macos`, `accesskit_unix`) push to OS accessibility APIs (UI Automation, NSAccessibility, AT-SPI). Both bevy_egui and Buiy integrate AccessKit.
- **AccessKit feature flag (`accesskit`)** — bevy_egui's opt-in Cargo feature for AccessKit support. **Not in `default-features`.** Pulls `bevy_a11y` and egui's `accesskit` feature. Timeline: scaffolded 0.34 → disabled 0.37 → re-enabled OPT-IN 0.38 (2025-10-13). See [`api-surface.md`](api-surface.md) § "Accessibility" and [`critiques.md`](critiques.md) § "Accessibility."
- **`bevy_a11y`** — Bevy crate that bridges Bevy entities to AccessKit. Pulled in by bevy_egui's `accesskit` feature. Buiy explicitly does *not* layer over `bevy_a11y` — it replaces it on any window where Buiy is present (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6).

## Project / people

- **vladbat00** — Vladyslav Batyrenko. Sole maintainer of bevy_egui from 2020-08-14 onward. Based in Mariupol, Ukraine. Maintains as a hobby project; no employer is associated. Personal Patreon linked from the bevy_egui README. Bus factor 1. See [`governance.md`](governance.md).
- **Emil Ernerfeldt (emilk)** — Original author of egui. CTO + co-founder of Rerun.io. Sponsors egui development through Rerun. Multi-engineer team works on egui upstream. See [`history.md`](history.md) § "egui upstream genesis."
- **Casey Muratori** — Author of the 2005 forum thread "Immediate-Mode Graphical User Interfaces" that sketched the IMGUI architecture. Conceptual progenitor of Dear ImGui + egui.
- **Omar Cornut (ocornut)** — Author of Dear ImGui (2014), the C++ IMGUI library that egui's API consciously echoes. Dear ImGui is the industry standard for dev tools (game engine editors, scientific software, AAA studio internal tooling).

## Sources

- bevy_egui repository — https://github.com/vladbat00/bevy_egui
- bevy_egui README @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/README.md
- bevy_egui CHANGELOG @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/CHANGELOG.md
- bevy_egui Cargo.toml @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/Cargo.toml
- egui repository — https://github.com/emilk/egui
- egui crate docs — https://docs.rs/egui/latest/egui/
- Dear ImGui — https://github.com/ocornut/imgui
- Casey Muratori — Immediate-Mode Graphical User Interfaces (2005) — https://caseymuratori.com/blog_0001
- AccessKit — https://accesskit.dev
- Rerun.io — https://www.rerun.io/
- bevy-inspector-egui — https://github.com/jakobhellermann/bevy-inspector-egui
- Sibling evidence files: [`architecture.md`](architecture.md), [`api-surface.md`](api-surface.md), [`comparisons.md`](comparisons.md), [`critiques.md`](critiques.md), [`distribution.md`](distribution.md), [`ecosystem.md`](ecosystem.md), [`governance.md`](governance.md), [`history.md`](history.md), [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md), [`integration.md`](integration.md), [`open-problems.md`](open-problems.md), [`use-cases.md`](use-cases.md)
