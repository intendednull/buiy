**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — runtime architecture: VirtualDom, components, hooks, renderer abstraction

# Architecture

Dioxus is a **React-shaped framework runtime**: it owns a scheduler, a virtual DOM, a hook context system, and a renderer-trait surface that backends implement. Authoring is component-functions-plus-`rsx!`-macro. The same component code targets web/desktop/mobile because the renderer is pluggable; the VDOM is the cross-target invariant.

## VirtualDom: the runtime core

The central type is `dioxus_core::VirtualDom`. It owns:

- A bump-allocated arena of `VNode`s (the current render output).
- A queue of dirty scopes (component instances) that need re-rendering.
- A scheduler that drains the queue, runs each dirty component's function, allocates a new VNode tree, and diffs the new tree against the previous to produce a list of **mutations** (`AppendChildren`, `CreateElement`, `SetText`, `SetAttribute`, `RemoveEventListener`, etc.).
- A **context** map that hooks read/write (state lives in `ScopeState`).

A renderer holds the `VirtualDom`, repeatedly calls `vdom.render_immediate()` to drain pending updates, and applies the returned mutation stream to the host (DOM element nodes for web, native widgets for `dioxus-desktop`'s webview, Blitz `blitz-dom` nodes for the WGPU path).

## Components are functions

```rust
fn Counter(props: CounterProps) -> Element {
    let mut count = use_signal(|| 0);
    rsx! {
        button { onclick: move |_| count += 1, "{count}" }
    }
}
```

Component invocation creates a **scope** (an indexed slot in `VirtualDom`'s scope arena). The scope owns the hook state (signals, effects, contexts created inside) and the most recent rendered `Element`. Subsequent invocations of the same component reuse the same scope by position-in-tree, so hook state persists across renders — the same positional-hook pattern React uses.

The runtime is **single-threaded per `VirtualDom`** (Dioxus 0.5 made signals `Send + Sync`-capable for cross-thread reads, but the scheduler itself is not parallel). Multi-window desktop apps spawn multiple `VirtualDom`s.

## Hooks and signals

Dioxus carries a typical hooks vocabulary:

- `use_signal(init)` — the dominant state primitive (since 0.5). Returns `Signal<T>: Copy`.
- `use_effect(closure)` — re-runs when read signals change.
- `use_memo(closure)` — cached derived value, re-computes on signal change.
- `use_resource(future)` — async data fetch with suspense.
- `use_context::<T>()` / `use_context_provider(init)` — provide/consume context.
- `use_hook(init)` — escape hatch for custom hooks.
- `use_future`, `use_coroutine`, `use_callback`, `use_drop`, etc.

The legacy `use_state` / `use_ref` from 0.4 still exist but are de-emphasized. Most documentation and templates default to signals. See [`signals-and-state.md`](signals-and-state.md).

## RequiredComponents-equivalent? No.

Unlike Bevy's `#[require(...)]` pattern (see [`prior-art/bevy-ui/`](../bevy-ui/) and the borrowed primitives in [`prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md)), Dioxus has no required-companion-component mechanism — there are no components-in-the-ECS-sense to attach companions to. Component composition happens via JSX-shape: a `<Counter />` rendering inside another component's `rsx!` is the only composition primitive. There is no analogue to ECS's data-oriented decomposition.

## Renderer abstraction

The renderer trait surface is **not a single trait** — Dioxus uses backend-specific crates that consume the `Mutation` stream from `VirtualDom`. The major backends:

- **dioxus-web** — compiles to WASM; mutations applied to the browser DOM via `wasm-bindgen`.
- **dioxus-desktop** — wraps a Webview (wry/tao) and runs WASM inside it; native-window chrome but DOM rendering still happens in webview.
- **dioxus-native** — runs WGPU + Blitz directly, no webview. Mutations applied to Blitz's `blitz-dom`. **Experimental** in 0.7.
- **dioxus-mobile** — variant of desktop for iOS/Android.
- **dioxus-ssr** — renders to a string for server-side rendering.
- **dioxus-fullstack** — combines SSR + client hydration + server functions on Axum.

The diversity of backends is the core architectural commitment ("one codebase, every platform"). It is also the source of most of Dioxus's quality-per-target variance. See [`targets.md`](targets.md) and [`open-problems.md`](open-problems.md) § "Multi-target fragmentation."

## Scheduler / runtime loop

Per frame (or per event):
1. Event fires (DOM event, custom event, async task wakeup, signal write from background task).
2. The scheduler marks the affected scope(s) dirty.
3. `vdom.render_immediate()` drains the dirty queue — for each scope, run the component function, build a new VNode tree, diff against the previous, append mutations to the output buffer.
4. Renderer drains mutations and applies them to the host.

Async work is supported via `tokio` (server / native targets) or `wasm-bindgen-futures` (web). `Suspense` boundaries collect pending futures and re-render when they resolve.

## Compared to Bevy's frame loop

| Concern | Dioxus | Bevy (Buiy substrate) |
|---|---|---|
| Update unit | Dirty scope (component instance) | System (ECS query) |
| State storage | Hook arena per scope; signals in generational-box | ECS components on entities |
| Change detection | Signal-subscription graph | `Changed<T>` query filters |
| Rendering | VDOM-diff → mutation stream → backend | ECS extract → render-graph passes |
| Threading | Single-threaded scheduler per `VirtualDom` | Parallel system scheduler |
| Authoring | Function components + `rsx!` | Spawn entities + components (+ BSN draft) |

This table is the cleanest summary of why Dioxus is not a Buiy substrate: the **unit of work** is fundamentally different (component instance vs ECS query), and Buiy's substrate (Bevy's ECS scheduler, change detection, render graph) is already committed.

## Implications for Buiy

- **The runtime decouples authoring from rendering** — `rsx!` produces VNodes, backends consume mutations. Buiy's analogous decoupling is `BSN` (authoring) → ECS spawn (`buiy_core` widgets) → render-graph passes (`buiy_render`). Different layer, same intent. See [foundation architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md).
- **Scheduler is not parallel.** Validates Buiy's choice to lean on Bevy's parallel system scheduler instead. A signal-based runtime built *on top of* Bevy would still be subject to whatever ordering constraints the signal graph imposes; Dioxus's experience is that "one mutation queue, applied serially" is the load-bearing simplification, but it caps multi-core scaling.
- **Hot-reload (Subsecond) is genuinely novel.** Dioxus 0.7's binary-patching hot-reload across WASM/desktop/mobile is the most aggressive runtime-modification story in Rust UI. The technique (incremental linking + `subsecond::call()` integration points) is portable in principle; whether Buiy's BSN-asset-reload story benefits from any of it is open. See [`history.md`](history.md) § "0.7 — Subsecond."

## Sources

- Dioxus repo README: https://github.com/DioxusLabs/dioxus/
- `dioxus_core` docs: https://docs.rs/dioxus-core/
- Dioxus 0.7 release notes: https://dioxuslabs.com/blog/release-070
- Dioxus 0.5 release notes (signals): https://dioxuslabs.com/blog/release-050
- Sibling: [`rsx-macro.md`](rsx-macro.md), [`signals-and-state.md`](signals-and-state.md), [`targets.md`](targets.md)
