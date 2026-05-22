**Date:** 2026-05-22
**Status:** active
**Subject:** egui — internal architecture, the Context/Ui/FullOutput pipeline, and backend abstraction

# Architecture

egui is the upstream Rust immediate-mode UI framework by Emil Ernerfeldt. This file describes its internal architecture: how a frame is constructed, what state persists, what gets emitted to the host, and how host backends consume that output. The bridge between Bevy and egui is documented separately at [`../bevy-egui/`](../bevy-egui/) — this file is about egui *itself*, the layer below the Bevy bridge.

## Crate facts (verified 2026-05-22)

| Field | Value |
|---|---|
| Crate | `egui` |
| Latest stable | **0.34.2** (2026-05-04) |
| Recent versions | 0.34.1 (2026-03-27), 0.34.0 (2026-03-26), 0.33.3 (2025-12-11), 0.33.2 (2025-11-13), 0.33.0 (2025-10-09) |
| Total downloads | 16,963,701 |
| Recent downloads (90 d) | 3,721,205 |
| Versions published | 61 |
| First release | 2020-05-30 (egui — predecessor crate Emigui starting ~2018, renamed to egui in 2020) |
| License | **MIT OR Apache-2.0** (dual — note bevy_egui chose MIT-only) |
| Repo | https://github.com/emilk/egui |
| Author / lead | Emil Ernerfeldt |
| Steward | Rerun.io (Emil's company; commercial driver) |
| Paradigm | Immediate-mode (rebuilt every frame from procedural code) |

The official tagline is **"An easy-to-use immediate mode GUI that runs on both web and native."**

## The central type: `egui::Context`

Everything flows through `egui::Context`. The user creates one `Context` per UI surface (typically per window or per camera in bevy_egui) and threads it through the application loop. The `Context` holds:

- **The font atlas / texture cache** (`epaint::TextureManager`), shared across frames.
- **`egui::Memory`** — a typed map keyed by `egui::Id`, holding cross-frame widget state: text-edit cursor position, scroll offset, collapsing-header open state, window position, drag state, focus state, animation state.
- **Style + Visuals** — the current theme (`egui::Style`, `egui::Visuals`) for the entire context.
- **Last frame's tessellated output** — the `Vec<ClippedPrimitive>` from the previous frame, kept for input hit-testing.
- **Plugin slots** — the `egui::Plugin` trait registered via `ctx.add_plugin(...)` (introduced in 0.33.0; replaces the older callback-based `on_begin_pass` / `on_end_pass` hook API).
- **Pass-loop state** — the `request_discard()` flag plus a `max_passes` budget (default 2) for multi-pass relayout.

`Context` is internally reference-counted (it's `Clone` and cheap to clone — under the hood it's an `Arc<RwLock<ContextImpl>>`), so multiple systems can hold handles to the same context without explicit lifetime gymnastics.

## Per-frame run: `begin_pass` → `Ui::add(...)` → `end_pass`

A frame is:

```rust
let raw_input: egui::RawInput = collect_input();
ctx.begin_pass(raw_input);
{
    // user code: emit widgets via Window / Area / Panel / Ui APIs
    egui::CentralPanel::default().show(&ctx, |ui| {
        if ui.button("Save").clicked() { /* ... */ }
    });
}
let full_output: egui::FullOutput = ctx.end_pass();
```

The higher-level helper `Context::run(raw_input, |ctx| { ... })` wraps `begin_pass` / `end_pass` and handles the multi-pass loop automatically (re-running the closure up to `max_passes` times if any widget calls `ctx.request_discard()` to request a relayout).

`begin_pass` clears the per-frame state (the in-flight `Shapes` list, the per-frame `widget_response` map) but leaves `Memory` intact. `end_pass` runs the layout finalization, tessellates the shapes, and packages the result.

0.34.0 introduced a higher-level entrypoint `Context::run_ui` that exposes a single whole-app `Ui` as the primary entry, with `Ui` deref-ing to `Context` — eliminating the `ui.ctx()` indirection. This shift sits on top of the underlying `begin_pass` / `end_pass` pair; it doesn't replace it.

## The `Ui` struct and layout

`Ui` is the layout cursor. When user code calls `ui.button("Save")`, the `Ui` allocates a rectangle for the button at the current cursor position, paints into it, registers the widget for hit-testing, and advances the cursor. There is no separate layout pass that "lays out a tree of widgets" — layout happens immediately, in sequence, as the user code walks the layout primitives.

Layout containers compose by closure:

```rust
ui.horizontal(|ui| {
    ui.label("Name:");
    ui.text_edit_singleline(&mut name);
});
```

Inside the closure, `ui` is a new `Ui` with a horizontal layout direction and bounded extent. When the closure returns, the parent `Ui` learns the child's used rect and advances its own cursor accordingly.

This is fundamentally different from Bevy's retained tree + Taffy layout solver. egui's layout is single-pass, top-to-bottom, no caching across frames — every frame recomputes every rectangle from scratch. Emil documents this as a "1-2 ms overhead per frame" claim in the rustdoc; bevy_egui's bridge inherits the same cost model.

For widgets whose size depends on a value computed during the same frame (auto-sizing tooltips, popovers that need content size before placement), egui's **multi-pass mode** lets a widget call `ctx.request_discard()`; the pass loop then re-runs the entire user closure with the previous pass's output available. `Options::max_passes` (default 2) caps this. The `request_discard` machinery is the mechanism behind bevy_egui's multi-pass schedule.

## Output: `egui::FullOutput`

`end_pass` returns:

```rust
pub struct FullOutput {
    pub platform_output: PlatformOutput,
    pub textures_delta: epaint::textures::TexturesDelta,
    pub shapes: Vec<epaint::ClippedShape>,
    pub pixels_per_point: f32,
    pub viewport_output: ViewportIdMap<ViewportOutput>,
}
```

- **`platform_output`** — host-applied side effects: cursor icon to set, clipboard text to write, IME state, open-URL request, virtual keyboard hint. The host backend (eframe / bevy_egui / a custom integration) applies these.
- **`textures_delta`** — `set` entries for new / updated atlas regions (the partial-update mechanism introduced in 0.38.x of bevy_egui mirrors a corresponding egui API), and `free` entries for textures the host should drop.
- **`shapes`** — the tessellated draw list. Each `ClippedShape` carries a clip rectangle plus an `epaint::Shape` (mesh, path, text, etc.). Calling `ctx.tessellate(shapes, pixels_per_point)` converts these to `Vec<ClippedPrimitive>` — flat triangle lists ready for a wgpu / glow draw call.
- **`pixels_per_point`** — current DPI scale.
- **`viewport_output`** — per-viewport (multi-window) state.

The host backend's only job is to apply `platform_output`, upload `textures_delta`, and issue draw calls for the tessellated `shapes`. This is intentionally tiny — egui is designed to be embedded into any rendering pipeline that can do textured triangles with scissor clipping.

## Memory model: `egui::Memory`

`Memory` is egui's retained surface — the *only* thing that persists between frames besides the texture atlas and the previous frame's shapes. It holds:

- **`Memory::data`** — a `IdTypeMap` (typed by `TypeId` per key) for user-keyed state. Read with `ctx.memory_mut(|m| m.data.get_temp::<T>(id))`.
- **Focus state** — which widget has keyboard focus, the focus-navigation order.
- **Drag state** — what's being dragged, drag delta accumulators.
- **Animation state** — `AnimationManager` tracking per-`Id` progress values for `ui.animate_bool(...)` / `ui.animate_value_with_time(...)`.
- **Per-widget retained state** — `CollapsingHeader` open/closed, `ScrollArea` scroll offset, `TextEdit` cursor position, `Window` rect.
- **`prev_pass_id`** map — what was hovered / clicked / focused last frame (so this frame's hit-tests can compare).

Every entry in `Memory` is keyed by `egui::Id`. If two widgets compute the same `Id`, they share state — and conflict. The `Id` collision problem is the immediate-mode tax; see [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) for the full story.

## The Id system: stable identity from call-stack

Because widgets don't persist as entities, egui has to derive a stable identity for "this button" across frames. `Id` is constructed by:

- `Id::new(source)` — hash a single value (e.g. a string label).
- `Id::with(child)` — extend a parent `Id` by hashing in a child seed.

The default convention: when user code calls `ui.button("Save")`, egui combines `ui.id()` (the parent UI's `Id`, itself derived from the enclosing `Window` / `Panel` / `horizontal` / etc.) with the button's label, producing a stable `Id` so long as the call site is stable across frames.

This works for static layouts. It fails for dynamic content — two list items with identical labels produce identical `Id`s, and the second inherits the first's `Memory` state (cursor position, scroll offset, drag state). The workaround is `ui.push_id(i, |ui| { ... })` per item, mixing an explicit per-item seed into the parent `Id`. Every long-running egui project develops local muscle memory for which loops need a `push_id`. The rustdoc on `Id` explicitly notes "the `Id`s must be unique" and "a collapsing region needs to remember whether or not it is open even if the layout next frame is different" — that "even if the layout next frame is different" is doing real work in the spec.

## Backend abstraction: how `FullOutput` is consumed

egui ships several official backends (each a separate crate in the `emilk/egui` workspace, except `egui_plot` which lives in its own repo):

- **`eframe`** — the official "batteries-included" runtime. Wraps `egui-winit` + (`egui-wgpu` or `egui_glow`) + a main loop. Runs on web (WASM + WebGL), Linux, macOS, Windows, Android. The default choice for standalone egui apps.
- **`egui-winit`** — winit input forwarding (translates winit events to `egui::RawInput`). Used by eframe and by hand-rolled native backends.
- **`egui-wgpu`** — wgpu renderer for `FullOutput`. Used by eframe on most targets and by bevy_egui internally (bevy_egui re-tessellates not, it uses egui's own `epaint::Tessellator` and submits the resulting triangles through Bevy's render graph).
- **`egui_glow`** — OpenGL/WebGL renderer via the `glow` crate. Smaller binary footprint on web than wgpu.
- **`egui_extras`** — image loaders (PNG/JPEG/SVG), `Table` widget, `DatePickerButton`. Officially adjacent but not part of `egui` core.
- **`egui_demo_lib`** — the demo app code, available as a library so other tools can embed the demo.

Outside the official set:

- **`bevy_egui`** — the Bevy bridge ([`../bevy-egui/architecture.md`](../bevy-egui/architecture.md)). Replaces eframe's main loop with Bevy's scheduling, uses Bevy's render graph instead of egui-wgpu's standalone path.
- **`egui_plot`** — immediate-mode 2D plotting, in its own repo (`emilk/egui_plot`); 6,765,876 total downloads. Lives outside `egui` core because plotting is heavy and not every app needs it.
- **Many community integrations** — SDL2, miniquad, Raylib, Termion (text-mode egui), each consuming `FullOutput` in its own way.

There is **no Skia backend** in the official set. egui's renderer is `epaint`, which is a small custom tessellator targeting textured triangles with scissor clipping. Skia would be possible but adds a heavyweight C++ dependency egui has deliberately avoided. (The pre-amble for this folder mentioned Skia as a backend variant — that's a misattribution; correcting here.)

## 0.34.0 text-rendering shift: `ab_glyph` → `skrifa` + `vello_cpu`

A notable architectural shift in 0.34.0 (2026-03-26): egui's text rendering swapped backends. The old stack — `ab_glyph` for font parsing + a custom rasterizer — was replaced by `skrifa` (Google's Rust port of FreeType's font-parsing primitives, the same crate that powers the Vello renderer) plus `vello_cpu` for glyph rasterization. The user-visible result: font hinting and variable font support (both absent under `ab_glyph`), noticeably sharper text. See [`text-rendering.md`](text-rendering.md) for the implications.

## The plugin trait (0.33.0)

0.33.0 introduced a trait-based plugin API replacing the older callback-based hooks:

```rust
pub trait Plugin {
    fn on_begin_pass(&mut self, ctx: &Context) { ... }
    fn input_hook(&mut self, raw_input: &mut RawInput) { ... }
    fn output_hook(&mut self, full_output: &mut FullOutput) { ... }
    fn on_end_pass(&mut self, ctx: &Context) { ... }
    fn on_widget_under_pointer(&mut self, ...) { ... }  // added 0.33.2
}
```

Plugins are owned by the `Context` and run at fixed lifecycle points. State lives on the plugin struct directly rather than in `Memory`, which is cleaner for inspector / instrumentation use cases. The `on_widget_under_pointer` hook (0.33.2) is the official entry point for building widget inspectors / picker overlays.

## See also

- [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) — the conceptual hinge: why this architecture is shaped the way it is.
- [`api-surface.md`](api-surface.md) — the widget vocabulary and authoring patterns.
- [`text-rendering.md`](text-rendering.md) — the post-0.34.0 text pipeline.
- [`../bevy-egui/architecture.md`](../bevy-egui/architecture.md) — how the Bevy bridge consumes the architecture described here.

## Sources

- egui repo — https://github.com/emilk/egui
- egui README @ master — https://raw.githubusercontent.com/emilk/egui/master/README.md
- egui CHANGELOG @ master — https://raw.githubusercontent.com/emilk/egui/master/CHANGELOG.md
- crates.io API (egui) — https://crates.io/api/v1/crates/egui
- crates.io API (egui_plot) — https://crates.io/api/v1/crates/egui_plot
- `egui::Context` rustdoc — https://docs.rs/egui/latest/egui/struct.Context.html
- `egui::Id` rustdoc — https://docs.rs/egui/latest/egui/struct.Id.html
- `egui::FontDefinitions` rustdoc — https://docs.rs/egui/latest/egui/struct.FontDefinitions.html
- Rerun.io — https://rerun.io
