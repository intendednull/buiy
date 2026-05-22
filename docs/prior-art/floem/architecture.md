**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — runtime structure, view functions, reactive runtime, render pipeline

## The three layers

Floem partitions into three layers that match the Solid.js / Leptos lineage but compiled to native:

1. **Reactive runtime** (`floem_reactive`). Signals, effects, derived values, scopes, batching. Pure data; no UI knowledge. Re-exported from the crate root as `floem::reactive`.
2. **View tree.** Composable `View` impls. A view is a function `fn() -> impl View` that allocates `View` nodes; `View` nodes are stateful (they own their child views, their style, their attached event handlers). The reactive runtime drives view updates by re-running closures attached to specific view properties.
3. **Render + windowing.** A custom `winit` fork (`floem-winit`) drives the event loop; rendering goes through one of four backends (vger, vello, skia, tiny-skia) selected by Cargo feature.

This is **not** the classical Elm / Iced architecture (one big `view(model) -> Element` that re-runs every frame). It is the Solid.js architecture: views are constructed *once* and reactive primitives drive surgical updates into the long-lived view tree.

## View functions and the View trait

A Floem app entry point is shaped like:

```rust
fn app_view() -> impl View {
    let (counter, set_counter) = create_signal(0);
    h_stack((
        label(move || format!("count: {}", counter.get())),
        button("inc").on_click_stop(move |_| { set_counter.update(|n| *n += 1); }),
    ))
}

fn main() { floem::launch(app_view); }
```

Three things happen here that distinguish Floem from a coarse-re-render framework:

- `app_view` runs **once**, at app startup. The returned view tree is the persistent root.
- `label(move || ...)` captures a closure. Floem wires that closure into the reactive runtime so it re-runs whenever any signal it reads changes — but only the `label`'s text is re-evaluated, not the whole tree.
- Event handlers are wired by builder methods on view nodes (`.on_click_stop(...)`). The `_stop` suffix is Floem's convention for "consume the event."

`View` is a trait with required methods for `id`, `view_data`, `build` (initial render), `update` (when state changes), `layout` (Taffy integration), `paint` (renderer dispatch), and event hooks. Built-in views (`label`, `button`, `text_input`, `list`, `virtual_list`, `dyn_container`, `tab`, `scroll`, `stack`, `h_stack`, `v_stack`, `dyn_stack`, etc.) live in the `views` module.

## The reactive runtime

`floem_reactive` provides:

- `RwSignal<T>` — a read-write signal. Cheap (`Copy` if `T: Copy`). `.get()` reads; `.set(v)` writes; `.update(|t| ...)` mutates in place.
- `create_signal` — the older `(read, write)`-tuple flavor (Leptos-classic style).
- `create_effect` — register a closure that re-runs when any signal it reads changes.
- `create_memo` / `derive` — cached derived values; only recompute when dependencies change.
- `Scope` / `RwScope` — ownership boundary for signals. When a scope is disposed, its signals and effects are torn down.
- `batch(|| { ... })` — coalesce signal writes; effects fire once after the closure.

Details, comparisons, and the Solid.js → Leptos → Floem lineage are in [`fine-grained-reactivity.md`](fine-grained-reactivity.md).

## Layout

Layout is delegated to **Taffy 0.9.2** (with the `grid` feature). Floem builds a parallel Taffy tree from the view tree; the `Style` builder API maps directly to Taffy properties plus Floem-specific extensions (background, border, transitions). PR #1063 ("Faster style v2", merged 2026-04-11) restructured the style pipeline for performance.

See [`layout-and-styling.md`](layout-and-styling.md).

## Render pipeline

The render side is **multi-backend by Cargo feature**. The same view tree paints through:

- **`vger`** — Floem-team's GPU renderer on top of wgpu. Default for many configurations.
- **`vello`** — Linebender's GPU compute renderer (also wgpu-based).
- **Skia** — via `floem_skia_renderer` (AnyRender-backed; GPU).
- **`tiny-skia`** — CPU fallback when no GPU is available.

Each backend implements the same paint trait surface. Glyph runs from Parley/Swash are passed to the active renderer for rasterization + composition.

The renderer-per-feature design is rare in the Rust UI space. Most peers pick one (Iced→wgpu, egui→painter abstraction, Slint→Skia-or-software, Xilem→Vello). Floem's multi-backend stance is a hedge — but it triples the surface area for testing.

## Text

Text goes through **Parley 0.7.0** (Linebender) for shaping + layout, **Swash 0.2** for rasterization, **Fontique 0.7.0** for font discovery. This is the Linebender/Xilem stack. See [`text-and-parley.md`](text-and-parley.md).

## Windowing and event loop

Floem does **not** use upstream `winit` directly. The workspace dependency is:

```toml
winit = { git = "https://github.com/lapce/winit", rev = "133268de...", package = "floem-winit" }
```

A `lapce/winit` fork is held to absorb fixes Floem needs without waiting for winit upstream release cycles. This is a real pattern in Rust UI (Lapce's editor pressures the winit dependency in ways upstream doesn't always accept), but it has real costs: forks drift, security fixes lag, and external crates that also depend on winit cannot share Floem's event loop trivially. See [`critiques.md`](critiques.md).

## Module surface (top-level `floem` crate)

From docs.rs/floem 0.2.0:

- `prelude` — the curated import bundle.
- `views` — built-in views (label, button, list, stack, scroll, text_input, etc.).
- `style` — the style builder.
- `animate` — transitions and keyframe animations (added in 0.2.0).
- `event`, `keyboard`, `pointer` — input event types.
- `window` — window-level APIs.
- `context` — context propagation through the view tree.
- `responsive` — breakpoint / media-query analog.
- `reactive` (re-export of `floem_reactive`) — the signal runtime.

## Build/feature flags worth knowing

- `default` — pulls in `vger` renderer.
- `vello` — switch to the Vello renderer.
- `skia` / `tiny-skia` — Skia / CPU paths.
- `serde` — derive-serde on style types.

## Sources

- Floem repo on `main` — https://github.com/lapce/floem
- Cargo.toml workspace deps — https://github.com/lapce/floem/blob/main/Cargo.toml
- docs.rs/floem 0.2.0 module list — https://docs.rs/floem/latest/floem/
- 0.2.0 release notes — https://github.com/lapce/floem/releases
- PR #1063 "Faster style v2" — https://github.com/lapce/floem/pull/1063
- PR #1074 Wayland surface recovery — https://github.com/lapce/floem/pull/1074
