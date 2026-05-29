**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — reactivity model (Dioxus signals + components + `rsx!`)

# Reactive model

Freya **does not have its own reactivity layer**. It is structurally a renderer for **Dioxus**'s reactivity core. Components, hooks, signals, scopes, and the `rsx!` macro are all Dioxus primitives consumed as-is. The Freya-specific layer is the *element schema* (rect/label/paragraph instead of div/span/p) and the *renderer + scheduler bridge* that turns Dioxus VDOM mutations into Skia draw calls.

This file documents what Freya inherits from Dioxus, what it overlays, and how the reactive model interacts with the Skia + Torin + AccessKit stack. For the Dioxus primitives themselves, the canonical reference is [`../dioxus/signals-and-state.md`](../dioxus/signals-and-state.md) and [`../dioxus/rsx-macro.md`](../dioxus/rsx-macro.md).

## What Freya inherits from Dioxus

| Primitive | Source | Behavior in Freya |
|---|---|---|
| `Element` return type | `dioxus-core` | A Dioxus `VNode`. Freya's renderer reconciles it into the Skia scene. |
| `Signal<T>` | `dioxus-signals` (via `generational-box`) | Same Copy-handle-on-arena shape as Dioxus core. |
| `Store<T>` (0.7+) | `dioxus-signals` | Per-field signal accessors. Works in Freya unchanged. |
| `use_signal()` | `dioxus-hooks` | Identical to Dioxus core. |
| `use_effect()` / `use_memo()` | `dioxus-hooks` | Identical. |
| `use_context()` / `use_context_provider()` | `dioxus-core` | Context propagation through scope tree. |
| `rsx!` macro | `dioxus-rsx` | Same syntax, Freya-specific elements. |
| Component function pattern | `dioxus-core` | `fn MyButton(props: Props) -> Element { rsx!(...) }`. |
| `EventHandler<T>` | `dioxus-core` | Event-handler prop type for `on*` callbacks. |
| Scheduler | `dioxus-core` | Dioxus's single-threaded scope-rerender scheduler. |
| VDOM diff | `dioxus-core` | Same algorithm Dioxus uses everywhere. |

The Dioxus version pin in Freya's main branch is `^0.6.3` (workspace `Cargo.toml`). This means **Freya is married to Dioxus 0.6.x's API surface**. Every Dioxus 0.x→0.y migration is a Freya migration. See [`history.md`](history.md) and [`critiques.md § Dioxus coupling`](critiques.md).

## Freya-specific hooks

On top of Dioxus's generic hooks, `freya-hooks` ships UI-aware hooks tied to Freya's renderer / scene:

| Hook | Purpose |
|---|---|
| `use_focus()` | Returns a `FocusManager` for the current node. Wires into Freya's focus tree (which feeds AccessKit + keyboard nav). |
| `use_animation()` | Spring + tween animation engine. Animations drive signals; signals drive re-render. |
| `use_canvas()` | Custom-paint hook — gives the component a Skia `Canvas` to draw arbitrary content into. The escape hatch from the declarative model. |
| `use_node()` / `use_node_ref()` | Reference to the laid-out scene node — size, position, hover state. |
| `use_theme()` | Active theme value (a Dioxus context). |
| `use_init_theme()` | Provide theme at the root. |
| `use_platform()` | Access to platform-info (DPI, window size, etc.). |

These are *Freya-renderer-specific* — they would not work in a Dioxus-web app because they reference Freya's scene tree. Conceptually similar to React Native's `useWindowDimensions` — generic hook shape, platform-specific implementation.

## How signals drive Skia repaint

The reactive loop is:

1. User mutates a signal: `count.set(count() + 1)`.
2. Dioxus's scheduler marks every scope that subscribed to `count` as dirty.
3. Dioxus re-runs the affected component functions; new VDOM nodes are produced.
4. Dioxus emits a *mutation stream* (Insert, Update, Replace, Remove) representing the VDOM diff.
5. `freya-core` consumes mutations and applies them to its retained scene tree.
6. Torin re-lays-out the affected subtree.
7. Skia repaints the dirty area.

Step 5 is where Freya's bridge lives. Dioxus's mutation stream is a `dioxus-core` primitive that other renderers also consume — `dioxus-web` writes DOM mutations, `dioxus-native` writes to Blitz's DOM-like tree, Freya writes to its own scene tree. This is the **renderer-abstraction surface** that Buiy *cannot* borrow directly because Buiy's substrate is Bevy ECS, not a Dioxus scope tree.

## Differences from Dioxus core / web / native

| Concern | Dioxus-web | Dioxus-native (Blitz) | Freya |
|---|---|---|---|
| Render | Browser DOM | WGPU + Stylo | Skia (C++) |
| Layout | Browser CSS | Taffy | Torin |
| Text | Browser | Parley + skrifa | Skia textlayout |
| A11y | DOM (for free) | None as of 0.7.9 ([`../dioxus/open-problems.md`](../dioxus/open-problems.md)) | AccessKit |
| Hot-reload | Subsecond + RSX hot-reload | Subsecond | Subsecond (in 0.6+) |
| Mobile | via WebView | Planned | No |
| WASM | Native | No | No |
| Authoring | `rsx!` w/ HTML elements | `rsx!` w/ HTML elements | `rsx!` w/ Freya elements (`rect`, `label`) |

The Freya divergence is the **element schema** — `rsx!` parses the same tokens, but the Freya schema does not accept `div`/`span`/`p`/`button`. This is enforced by the `rsx!` macro looking up element types in a Freya-provided element registry.

## What Buiy can and cannot borrow

**Buiy CAN borrow:**

- **The Dioxus signal shape itself** — `Signal<T>: Copy` via `generational-box` arena. This is the lesson [`../dioxus/lessons.md`](../dioxus/lessons.md) § Borrow #1 already captured. Freya is the *existence proof* that this shape works in a desktop-native context, not just web.
- **The hook-as-context-binding pattern** (`use_focus`, `use_theme`, `use_animation`). If Buiy ever ships a signal layer (foundation [open question](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)), the renderer-specific-hook pattern is a clean way to expose Buiy-renderer state (focus, theme, layout, animation) to signal-aware components.
- **The mutation-stream renderer-abstraction surface** as a *conceptual* model — but not the implementation, because Buiy's substrate is Bevy ECS, not a Dioxus scope tree.

**Buiy CANNOT borrow:**

- **The whole reactivity stack as a dependency.** Freya is structurally married to Dioxus 0.6.x. Buiy would be married to Dioxus version cadence, which is independent of Bevy. Foundation [non-goal § 1.3 / open question § Reactivity layer](../../specs/2026-05-07-buiy-foundation/README.md) is the right policy. The signal layer if Buiy ever adds one should be built directly on Bevy ECS, with the Dioxus *shape* as inspiration but its own implementation.
- **The `rsx!` macro as-is.** `rsx!` is a Dioxus proc-macro that resolves elements through Dioxus's component-type system. BSN's component-spawn model is structurally different — BSN nodes are Bevy components (`Reflect`-able structs), not Dioxus scope nodes.

## Sources

- Freya workspace `Cargo.toml` (`dioxus ^0.6.3`) — https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- Freya docs.rs (modules: `hooks`, `core`, `launch`) — https://docs.rs/freya/latest/freya/
- Cross-references: [`../dioxus/signals-and-state.md`](../dioxus/signals-and-state.md), [`../dioxus/rsx-macro.md`](../dioxus/rsx-macro.md), [`../dioxus/architecture.md`](../dioxus/architecture.md), [`architecture.md`](architecture.md), [`lessons.md`](lessons.md).
- Buiy foundation — [`README.md § 5 Open questions — Reactivity layer`](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions), [`architecture.md § 2.7 Reactivity`](../../specs/2026-05-07-buiy-foundation/architecture.md#27-reactivity).
