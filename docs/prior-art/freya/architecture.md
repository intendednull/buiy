**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — architectural composition (Skia + Dioxus + Torin + AccessKit)

# Architecture

Freya is best understood as **a custom renderer for Dioxus's reactivity core, painted with Skia, laid out with Torin, and accessibility-bridged with AccessKit**. It is *not* a from-scratch GUI framework — every layer is a composition of existing crates. Marc Espín's contribution is the *integration* and the layout engine (Torin).

## Layered composition

```
+--------------------------------------------------+
|                  Application code                |
|   fn App() -> Element { rsx!( rect { ... } ) }   |
+--------------------------------------------------+
|       freya (meta-crate, re-exports + launch)    |
+--------------------------------------------------+
| freya-components | freya-hooks | freya-elements  |
+--------------------------------------------------+
|       freya-core (renderer + scheduler)          |
+--------------------------------------------------+
|    freya-engine  (Skia abstraction)              |
+--------------------------------------------------+
| dioxus-core | torin    | accesskit  | winit      |
| (VDOM +     | (layout) | (a11y tree)| (windowing)|
|  scheduler) |          |            |            |
+--------------------------------------------------+
|       freya-skia-safe (Skia C++ bindings)        |
+--------------------------------------------------+
|            Skia (C++) | system winit | OS a11y   |
+--------------------------------------------------+
```

The internal Freya crates (workspace members, all version-locked together):

| Crate | Purpose |
|---|---|
| `freya` | Meta-crate. Re-exports + `launch()` / `launch_cfg()` entry points. |
| `freya-core` | The renderer. Walks the Dioxus VDOM, drives Torin layout, calls into `freya-engine` for paint, builds the AccessKit tree. |
| `freya-engine` | Abstraction layer over Skia (and theoretically over a mock backend for tests). |
| `freya-elements` | Primitive element types: `rect`, `label`, `image`, `paragraph`, `text`, `svg`, etc. These are the "HTML-ish" tags exposed in `rsx!`. |
| `freya-components` | Pre-built widgets: `Button`, `Slider`, `VirtualScrollView`, `Calendar`, etc. |
| `freya-hooks` | Reactive hooks: `use_focus`, `use_animation`, `use_canvas`, `use_node`, theming hooks. |
| `freya-winit` | winit integration (event loop, IME, clipboard). |
| `torin` | Freya's own layout engine (also in `crates/torin/`). |

## Reactivity → render flow per frame

1. **Dioxus VDOM** (in `dioxus-core`) holds the component tree and re-renders subtrees when subscribed signals change.
2. **`freya-core`** walks the rendered VDOM and reconciles it against its own retained scene tree of `Node`s.
3. **Torin** lays out the scene tree (size + position pass; see [`layout-and-styling.md`](layout-and-styling.md)).
4. **`freya-engine` + `freya-skia-safe`** paint each laid-out node into a Skia canvas: backgrounds, gradients, shadows, borders, text (via Skia textlayout), images, SVG, clipping. See [`skia-rendering.md`](skia-rendering.md).
5. **AccessKit `TreeUpdate`** is pushed to the platform adapter when accessibility state changes.
6. **winit** presents the Skia surface (GL / Metal / D3D context, depending on platform).

The architecture is *retained-mode-on-top-of-VDOM*: Dioxus's reconciler emits mutations into a Freya scene-tree resource, and Freya re-lays-out + repaints only the affected subtrees. This is functionally similar to React-Native's bridge-to-native-views pattern but with Skia primitives instead of native widgets.

## The `rsx!` macro shape (in Freya context)

Freya reuses Dioxus's `rsx!` macro directly (it is a `dioxus-core`/`dioxus-rsx` re-export). Elements are *Freya-specific* (`rect`, `label`, `paragraph`) rather than HTML tags (`div`, `span`, `p`):

```rust
fn app() -> Element {
    let mut count = use_signal(|| 0);

    rsx!(
        rect {
            width: "100%",
            height: "100%",
            background: "rgb(20, 20, 20)",
            padding: "20",
            direction: "vertical",
            cross_align: "center",
            main_align: "center",

            label {
                font_size: "60",
                color: "white",
                "{count}"
            }

            Button {
                onpress: move |_| count += 1,
                label { "Increment" }
            }
        }
    )
}
```

Key observations:

- **Attributes are stringly-typed.** `width: "100%"`, `background: "rgb(...)"`, `direction: "vertical"`. This is the CSS-styled-props pattern — values parse at runtime, errors surface at runtime. Buiy's BSN-typed component approach is the *opposite* design choice.
- **Format-string interpolation** (`"{count}"`) works as in Dioxus — see [`../dioxus/rsx-macro.md`](../dioxus/rsx-macro.md) for the underlying mechanism.
- **No `bevy_ecs::Entity` notion.** Each `rect`/`label`/`Button` is a Dioxus VDOM node, not an ECS entity. Component instances are scoped by Dioxus's scope hierarchy, not by Bevy's `Entity` IDs.

## Element / Component split

- **Elements** (`rect`, `label`, `paragraph`, `image`, `svg`) are the *primitives*. They map to Freya scene-tree nodes with style attributes. Defined in `freya-elements`.
- **Components** (`Button`, `Slider`, `Calendar`, etc.) are *Dioxus function components* that compose elements. Defined in `freya-components`. Stateful via hooks (`use_signal`, `use_focus`, etc.).

This split mirrors HTML elements vs React components. The element layer is fixed (defined by Freya); the component layer is open (third-party can ship components).

## How Freya differs from a "real" Dioxus renderer

Dioxus core supports multiple renderers — `dioxus-web` (DOM), `dioxus-desktop` (webview-wrapped), `dioxus-native`/Blitz (WGPU + Stylo). Freya is structurally a **fourth Dioxus renderer**, but with these differences:

- **Not officially part of `DioxusLabs/`** — lives at `marc2332/freya`, not under the Dioxus organization. Marc is a member of `@dioxus-community` but not `@DioxusLabs`.
- **Skia, not browser-native or WGPU-via-Stylo.** Freya is the only first-party-feeling Dioxus renderer that paints with Skia directly.
- **Native-only, no web target.** Freya cannot compile to WASM. Dioxus's multi-target promise does not extend through Freya.
- **Own layout engine.** Blitz uses Taffy. Dioxus-desktop uses webview CSS. Freya uses **Torin**.

See [`../dioxus/targets.md`](../dioxus/targets.md) for the Dioxus renderer matrix; Freya sits outside that matrix as a community-maintained alternative.

## Why this is a useful comparison for Buiy

| Concern | Buiy | Freya |
|---|---|---|
| Render substrate | wgpu via Bevy render graph | Skia (C++ via `freya-skia-safe`) |
| Layout | Taffy (foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly)) | Torin (own) |
| Text | cosmic-text | Skia textlayout |
| A11y | AccessKit (foundation [§ 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)) | AccessKit |
| Reactivity | Bevy observers + change detection (signals: open question) | Dioxus signals + VDOM |
| Authoring | BSN (typed reflection) + ECS spawn | `rsx!` (stringly-typed attrs) |
| Substrate ECS | Bevy ECS | None — Dioxus scope hierarchy |
| Platforms | Bevy's matrix (desktop + WASM + mobile) | Desktop only |

Both pick **AccessKit** for a11y. Buiy and Freya are the two strongest existing-art proof-points that AccessKit-first integration works on the native-Rust track. Everything else diverges.

## Sources

- Freya workspace `Cargo.toml` — https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- Freya docs.rs (modules list) — https://docs.rs/freya/latest/freya/
- Freya `crates/torin/Cargo.toml` — https://raw.githubusercontent.com/marc2332/freya/main/crates/torin/Cargo.toml
- Freya website (overview, "Powered by Dioxus and Skia") — https://freyaui.dev/
- Cross-references: [`../dioxus/architecture.md`](../dioxus/architecture.md), [`../dioxus/rsx-macro.md`](../dioxus/rsx-macro.md), [`../dioxus/targets.md`](../dioxus/targets.md), [`../accesskit/lessons.md`](../accesskit/lessons.md).
- Buiy foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md).
