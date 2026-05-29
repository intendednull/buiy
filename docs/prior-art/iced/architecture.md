**Date:** 2026-05-22
**Status:** active
**Subject:** Iced — runtime architecture, element tree, renderer stack, layout engine, text pipeline

# Iced architecture

The most-adopted **retained-mode** Rust GUI library, as a counterpoint to immediate-mode egui. Built around the Elm Architecture (TEA): `Model + Message + Update + View`. Single-author lineage (Héctor Ramón / `hecrj`), MIT-licensed, 1,885,134 lifetime downloads on crates.io as of 2026-05-22, latest stable `0.14.0` published 2025-12-07.

Sibling files: [`elm-architecture.md`](elm-architecture.md), [`widgets-and-styling.md`](widgets-and-styling.md), [`layout-engine.md`](layout-engine.md), [`text-and-cosmic.md`](text-and-cosmic.md).

## Runtime topology

The Iced runtime owns a single feedback loop:

```
initial state
   │
   ▼
view()  ─► Element<'_, Message> tree
   │
   ▼
event loop (winit) ─► Message
   │
   ▼
update(&mut state, Message) ─► Task<Message>
   │                              │
   └──────────► state mutated  ◄──┘ (async side effects fold back as Messages)
   │
   ▼
view() runs again
```

`view()` is called *every frame an event arrives*, building a fresh `Element` tree from scratch. Iced is retained-mode in the sense that *application state* is retained between frames; the *widget tree* is rebuilt every event tick. This is the inverse of dominant retained models (Qt, GTK, web DOM) which retain the tree and mutate it.

## The application surface

Two equivalent entry points as of `0.14`:

1. **Function-based** (`0.13+`, recommended): `iced::run(title, update, view)` — Iced infers `State` and `Message` from function signatures, no trait implementation required.
2. **`Program` trait** + builder (`0.13+`): `iced::application(state_fn, update, view).theme(...).subscription(...).run()`.

The pre-0.13 `Application` and `Sandbox` traits still exist as thin wrappers but the function-based form is what current Iced code looks like. The `Program` trait is the underlying abstraction — `Application` is a `Program` with extras (subscriptions, themes, async lifecycle).

Function signatures look like:

```rust
fn update(state: &mut State, message: Message) -> Task<Message>;
fn view(state: &State) -> Element<'_, Message>;
fn subscription(state: &State) -> Subscription<Message>;  // optional
fn theme(state: &State) -> Theme;                         // optional
```

`Task<Message>` was named `Command<Message>` prior to 0.13 (renamed in [PR #2463](https://github.com/iced-rs/iced/pull/2463), September 2024). `Task` is a richer abstraction — it composes via `Task::batch`, `Task::chain`, `Task::map`, and can wrap arbitrary `Stream<Item = Message>` work, not just one-shot futures.

## The Element tree

`Element<'a, Message, Theme = Theme, Renderer = iced::Renderer>` is a type-erased widget container. The tree is rebuilt every time `view()` runs; widgets are short-lived values, not entities. State that needs to outlive a frame (button-press counters, text-input cursor positions, scroll offsets) lives in a parallel **widget-state tree** (`Tree` in `iced_core::widget::tree`) keyed by tree position + widget type-id, reconciled across rebuilds.

This is the "stateless widgets" trade-off: every interaction re-runs `view()`, so `view()` must be cheap. For perf, Iced provides:

- `widget::Lazy` — caches a sub-tree against a hash of dependencies; skips rebuild when unchanged.
- `widget::Responsive` — defers child construction until layout limits are known.
- `widget::keyed::column` — provides per-child stable keys so widget-state reconciliation survives reordering.

Compare to Bevy / Buiy ECS: in ECS each UI node is an entity with components; entities persist across frames; system code reads / writes components rather than rebuilding a tree. Iced's view-every-frame is closer to React's reconciler than to ECS.

## Renderer stack

Iced ships two built-in renderers, selected via Cargo features:

- **`iced_wgpu`** — wgpu-based GPU renderer (default). Targets Vulkan / Metal / DX12 / WebGPU. Maintains its own atlas / quad / triangle / gradient / shader pipelines. The `wgpu` feature pulls this in.
- **`iced_tiny_skia`** — software fallback via `tiny-skia` (`0.11`). Used when no GPU is available (headless tests, exotic platforms). The `tiny-skia` feature pulls this in.

Iced does **not** depend on Bevy's render graph, on `vello`, or on `skia-safe`. Its renderer is its own.

Custom shader effects ship as `widget::Shader` (wgpu-only) — apps can write their own wgsl and the renderer composites it inline. This is Iced's analog to Bevy's `UiMaterial`. See [`widgets-and-styling.md`](widgets-and-styling.md) for the styling surface.

The runtime is renderer-agnostic in principle (the `Program` trait is generic over `Renderer`), but in practice 99% of applications use the bundled wgpu + tiny-skia pair through the meta-crate.

## Layout engine

**Iced has its own layout engine in `iced_core::layout`. It does NOT use Taffy.** Implementation is a four/five-pass flex algorithm `core/src/layout/flex.rs` — the source notes it is "heavily inspired by the druid codebase."

Layout primitives:

- `Node` — tree of resolved boxes (`Size`, `Vector` offset, children).
- `Limits { min, max, fill }` — propagated downward; each widget's `layout()` method returns a `Node`.
- `flex` — main-axis distribution with fill factors, spacing, cross-axis alignment.
- `next_to_each_other`, `contained`, `padded`, `positioned` — basic composition helpers.

No CSS Grid, no anchor positioning, no container queries, no float, no subgrid. Layout is one-pass-down (`Limits` flow), one-pass-up (`Size` result), with the flex distribution pass between. Layout runs every frame that triggers a redraw; results are not cached across frames at the engine level (individual widgets can opt into caching via `Lazy`).

See [`layout-engine.md`](layout-engine.md) for the algorithm and the Buiy comparison.

## Text pipeline

Iced uses **cosmic-text** for text shaping, layout, BiDi, and font fallback. Iced 0.14 pins `cosmic-text 0.15` ([`Cargo.toml`](https://github.com/iced-rs/iced/blob/0.14/Cargo.toml)).

**Brief correction:** the brief asserted Iced migrated to Parley + harfrust. **This is wrong as of 0.14.0.** Iced has not migrated to Parley. The 0.14 changelog tracks cosmic-text version bumps (`0.13` → `0.14` → `0.15`) but no Parley adoption. Bevy's [issue #21765](https://github.com/bevyengine/bevy/issues/21765) (cosmic-text → Parley) is bevy-specific and has not propagated to Iced; the convergence-on-Parley narrative is bevy-side only.

This means Iced and Buiy converge on cosmic-text as their text substrate, while bevy_ui post-0.19 diverges to Parley. Iced is the largest non-Bevy production consumer of cosmic-text — its continued use validates Buiy's commitment. Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Validates.

See [`text-and-cosmic.md`](text-and-cosmic.md) for the timeline and implications.

## System layer

- **Window management:** `winit 0.30` (same as Bevy 0.15+).
- **Async runtime:** `Task` wraps `futures` directly; Iced is runtime-agnostic — `tokio`, `async-std`, `smol` all work. The `multi-thread` / `thread-pool` features pick the executor.
- **Subscriptions** — `Subscription<Message>` wraps any `Stream<Item = Message>` for background event sources (timers, websockets, file watchers, OS events). Iced manages subscription lifecycle (subscribe / unsubscribe by hash) across rebuilds.
- **Clipboard:** owned by the runtime via `arboard` (or platform equivalents).
- **IME:** added in 0.14 via winit's IME events. Iced exposes `text_input::Id::focus` + IME state through the widget tree.

## What Iced does not have

- No accessibility tree. No AccessKit integration. (See [`open-problems.md`](open-problems.md) when written; this is the largest production gap.)
- No CSS-style cascade. Styling is per-widget function dispatch ([`widgets-and-styling.md`](widgets-and-styling.md)).
- No CSS Grid layout — Row/Column/Container only ([`layout-engine.md`](layout-engine.md)).
- No design-token system. Themes are `struct Theme` values; widget styles are functions on `&Theme`.
- No hot-reload of layouts (until 0.14's experimental hot-reload).
- No first-class focus model with `:focus-visible` / focus traps / spatial navigation (basic tab focus only).

## Implications for Buiy

Iced is the most-deployed retained-mode Rust GUI and demonstrates that the Elm-architecture pattern *works* at production scale (COSMIC desktop is the proof). The architectural choices that diverge from Buiy:

1. **Iced has its own layout engine; Buiy uses Taffy.** Iced's layout works for its widget set but caps growth at CSS Grid / subgrid / container queries. Buiy buying Taffy directly inherits all of CSS layout for free; Iced would have to rewrite to get there. See [`layout-engine.md`](layout-engine.md) and [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Validates → Taffy.
2. **Iced rebuilds the widget tree every frame; Buiy/Bevy retain ECS entities.** The rebuild model trades runtime allocation for simpler reasoning. ECS-keyed state survives without reconciliation. Cross-link: [`elm-architecture.md`](elm-architecture.md) § "Comparison to ECS."
3. **Iced has no AccessKit.** This is the structural blocker on Iced being a fit for productivity apps with screen-reader requirements. Buiy's AccessKit-first stance ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) is therefore *not* validated by Iced — Iced shipped without it, which is the gap Buiy needs to not repeat.
4. **Iced validates cosmic-text as the long-term text substrate.** With Bevy moving to Parley post-0.19, Iced becomes the most important non-Bevy consumer keeping cosmic-text alive — see [`text-and-cosmic.md`](text-and-cosmic.md).

## Sources

- crates.io API — https://crates.io/api/v1/crates/iced (downloads, version metadata, license, MSRV).
- Iced repository — https://github.com/iced-rs/iced
- Iced 0.14.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.14.0
- Iced 0.13.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.13.0
- Iced 0.14 Cargo.toml — https://github.com/iced-rs/iced/blob/0.14/Cargo.toml
- Iced core layout — https://github.com/iced-rs/iced/blob/0.14/core/src/layout.rs
- Iced flex impl — https://github.com/iced-rs/iced/blob/0.14/core/src/layout/flex.rs
- Iced book — https://book.iced.rs/
- docs.rs/iced/0.14.0 — https://docs.rs/iced/0.14.0/iced/
- iced.rs — https://iced.rs/
- libcosmic (uses iced) — https://github.com/pop-os/libcosmic
- PR #2463 (Task API, Command rename) — https://github.com/iced-rs/iced/pull/2463
