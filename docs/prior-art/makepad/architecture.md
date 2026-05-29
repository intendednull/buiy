**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — runtime + Live-DSL + own-renderer architecture

# Architecture

Makepad is a **standalone Rust UI framework**, not a Bevy-shaped or game-engine-shaped library. It owns the full vertical stack: a custom DSL parser/compiler (the Live language), a runtime that materializes DSL nodes into widget trees, a custom GPU renderer with direct platform backends, and a cross-platform event loop / windowing layer. No wgpu. No winit at the top level (Makepad has its own per-platform event-loop integration). No ECS.

## The DSL-compiler-as-source-of-truth model

A Makepad app is a hybrid of two languages:

- **Rust** — the imperative / business-logic / event-handling code. Defines `struct` types with `#[derive(Live, Widget)]` and impl blocks for behaviour.
- **Live** (`.live` syntax) — the declarative UI / styling / shader / animation layer. Lives **embedded in Rust** inside `live_design! { ... }` macro blocks, or in external `.live` files loaded at runtime.

The Live compiler (`makepad-live-compiler` crate, 1.0.0, 0% documented on docs.rs) parses both sources into a unified `LiveRegistry`. The registry holds a flat array of `LiveNode`s — typed values, property bindings, component instantiations, shader sources, animation timelines, all interleaved. The runtime resolves `#[derive(Live)]` Rust structs against the registry by walking the node array and binding properties.

Pipeline:

```
.rs source ──┐
             ├──► live_design! macro
.live files ─┘     │
                   ▼
             Live tokenizer → Live parser → Live expander → LiveRegistry (Vec<LiveNode>)
                                                                  │
                                                                  ▼
                                                  Rust runtime (#[derive(Live)] structs)
                                                                  │
                                                                  ▼
                                                       Widget tree (Vec<Box<dyn Widget>>)
                                                                  │
                                                                  ▼
                                                       Draw pass → GPU backend (Metal / DX11 / OpenGL / WebGL)
```

The crate dependency graph mirrors this: `makepad-live-tokenizer` → `makepad-live-compiler` (parser + expander + registry) → `makepad-platform` (runtime + windowing + GPU backend) → `makepad-derive-live` (the derive macros) → `makepad-widgets` (the public API + widget catalog).

Implications:

- **Live is the source of truth for layout, styling, animation, and shader code.** Rust glue defines event-handling and state-mutation behaviour.
- **The DSL compilation runs at every `cargo build`** via the `live_design!` proc-macro. There is no separate `slint-build`-style `build.rs` codegen step required (though `cargo-makepad` exists for cross-target tooling, not for DSL compilation).
- **Hot-reload reuses the live compiler at runtime** to re-parse `.live` source and produce a new `LiveRegistry`, which the runtime swaps in over the live widget tree. See [`live-language.md`](live-language.md) on the hot-reload mechanism.

## The runtime + widget model

Makepad's widget model is **trait-object based**, not ECS-based. A widget is a Rust struct that:

- Derives `Live` (binds properties from the LiveRegistry), `Widget` (registers as a `Box<dyn Widget>`-compatible component), and optionally `LiveHook` (for lifecycle callbacks).
- Implements the `Widget` trait, with `draw_walk`, `handle_event`, and related methods.
- Holds `#[live]` fields for properties bound from Live syntax and `#[rust]` fields for internal state.

Example shape (paraphrased from `examples/hello_world/`):

```rust
#[derive(Live, Widget)]
struct MyButton {
    #[deref] view: View,           // inherits View's drawing
    #[live] text: String,          // bound from Live: `text: "Click me"`
    #[rust] click_count: u32,      // pure Rust state
}

impl Widget for MyButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) { ... }
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep { ... }
}
```

The `Cx` (context) is the per-frame god-object: holds the draw state, event queue, asset registry, animation timeline, GPU backend handle, and the LiveRegistry reference. It's threaded through every widget method by reference. Equivalent role to a "world" in ECS terms but **mutable-borrow-not-archetype-driven** — closer to a retained scene-graph context than to a Bevy ECS world.

## Event loop & windowing

Makepad does **not** use winit (vs Slint, Iced, egui, Dioxus desktop — all winit consumers). Instead, `makepad-platform` ships per-platform event-loop implementations:

- **macOS / iOS** — NSApplication / UIKit-backed event loop, Metal CALayer for the surface.
- **Windows** — Win32 message loop, DirectX 11 swapchain.
- **Linux** — direct X11 / Wayland event handling, OpenGL via EGL or GLX.
- **Web** — `requestAnimationFrame` + WebGL canvas, with full WASM compilation.
- **Android** — JNI bridge + Android NativeActivity, OpenGL ES.

This is a deliberate choice: skipping winit lets Makepad target platforms winit doesn't (OpenHarmony, tvOS) and avoid the winit-version-pin upgrade treadmill. It also means **Makepad does not benefit from winit's AccessKit integration** ([`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)) — the `accesskit_winit` adapter is unreachable. See [`open-problems.md`](open-problems.md).

## Asset, animation, theming

- **Asset model.** Live syntax declares assets inline: `dep("crate://self/icons/foo.svg")` resolves an asset path through the LiveRegistry's dependency resolver. Fonts, shaders, images, and `.live` sub-files use the same `dep()` path syntax. No separate asset server.
- **Animation.** Live syntax expresses animations declaratively: `animator = { default = on, on = { from: {all: Forward {duration: 0.2}}, ...}}`. The runtime's `Animator` advances animation state per frame against `Cx`'s time. No separate animation crate; baked into Live + platform.
- **Theming.** Live's `inherit` semantics + the `THEME_*` global constants form the de facto theming primitive. Makepad ships `theme_desktop_dark`, `theme_desktop_light`, `theme_mobile_dark` etc. as bundled `.live` files. No OS-preference binding (no `prefers-color-scheme` / `forced-colors` / `prefers-reduced-motion` automatic wiring).

## How `.live` and Rust compose

Three composition modes (from the examples):

1. **All-in-Rust via `live_design!` macro.** Most idiomatic. The macro takes `.live` syntax inline, parses at compile time, embeds the LiveRegistry data into the binary. Cross-language refactoring is restricted to one file but still spans two languages.

   ```rust
   live_design! {
       MyButton = {{MyButton}} {
           text: "Click me",
           layout: { padding: 8.0 },
           draw_bg: { color: #f00 },
       }
   }
   ```

2. **External `.live` files loaded at startup.** Used by Makepad Studio and the `hotload_ui` example. Files referenced via `live_register_widget!` / dep paths; loaded at startup and hot-reloaded on change.

3. **Runtime construction via `LiveValue` / `LiveNode` APIs.** Programmatic UI construction is *possible* (the LiveRegistry is mutable), but the supported authoring path is one of the first two. No documented "build a widget tree from runtime data" reference equivalent to BSN's `spawn`.

## Implications for Buiy

- **DSL above runtime is shippable: confirmed (with caveats).** Live's `.live` + Rust glue model is the second production proof (after Slint's `.slint` + Rust glue) that a Rust UI toolkit can ship a DSL-above-runtime authoring layer at 1.0. Buiy's BSN authoring layer ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)) inherits this validation. See [`lessons.md`](lessons.md).
- **Skipping wgpu has real cost.** Maintaining Metal + DX11 + OpenGL + WebGL backends in-house is four backend implementations to keep current. Buiy explicitly stays on wgpu (via Bevy's render graph) — the corpus reads Makepad's choice as costly, not visionary. See [`gpu-rendering.md`](gpu-rendering.md).
- **Skipping winit forecloses AccessKit.** The `accesskit_winit` adapter is the canonical producer-side integration path; skipping winit means re-implementing platform AT bridges from scratch. Makepad has not, so it has none. Buiy stays on winit (via Bevy) to keep AccessKit reachable.
- **Trait-object widgets versus ECS components.** Makepad's `Box<dyn Widget>` model is closer to retained-scene-graph than to BSN's "small, public-fielded, observable, decomposed" component shape. Buiy's BSN-friendly constraint ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)) is **not** what Makepad does. The Live DSL composes with trait-object widgets; Buiy's BSN composes with ECS entities + decomposed components.

## Sources

- Makepad repo: https://github.com/makepad/makepad
- `makepad-widgets` docs.rs: https://docs.rs/makepad-widgets/1.0.0/makepad_widgets/
- `makepad-live-compiler` docs.rs: https://docs.rs/makepad-live-compiler/latest/makepad_live_compiler/ (0% documented, module structure only)
- Examples: https://github.com/makepad/makepad/tree/dev/examples (`hello_world`, `hotload_ui`, `uizoo`, `slides`, `splash`)
- Sibling files: [`README.md`](README.md), [`live-language.md`](live-language.md), [`gpu-rendering.md`](gpu-rendering.md), [`open-problems.md`](open-problems.md)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
