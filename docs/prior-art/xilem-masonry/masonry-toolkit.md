**Date:** 2026-05-22
**Status:** active
**Subject:** Masonry — retained-mode widget toolkit underlying Xilem (Linebender's lower layer)

# Masonry toolkit

Masonry is the **lower layer of the Linebender UI stack**: a retained-mode widget toolkit, framework-paradigm-agnostic, designed so that any higher-level reactive layer (Xilem is one; the README says "immediate-mode, Elm, functional reactive" are all implementable on top) can sit on the same widget infrastructure. The decomposition matters: it's the same architectural move bevy_headless_widgets made for bevy_ui, and the same one Buiy makes by separating `buiy_core` from `buiy_widgets`.

## Crate split inside the workspace

```
masonry          — high-level: re-exports core + widgets, opinionated defaults
masonry_core     — the trait machinery, no widgets, no platform layer
masonry_winit    — winit integration (event loop, render harness)
masonry_testing  — test-tools: snapshot-rendering, synthesized input replay
masonry_imaging  — image-loading helpers
tree_arena       — separately versioned tree-storage crate (currently 0.2.0)
```

The `masonry_core` / `masonry_winit` split is a deliberate "no platform in the core" gesture — the same gesture Buiy's foundation makes ([`architecture.md § 2.8`](../../specs/2026-05-07-buiy-foundation/architecture.md)) by separating `buiy_core` from any windowing.

## The `Widget` trait

The central abstraction. A `Widget` implementation lives in the retained tree and gets called by Masonry for each lifecycle event:

- `on_pointer_event(&mut self, ctx, event)` — pointer input
- `on_text_event(&mut self, ctx, event)` — keyboard + IME input
- `on_access_event(&mut self, ctx, event)` — accessibility actions
- `update(&mut self, ctx, event)` — internal state-change notifications (e.g., focus changes)
- `layout(&mut self, ctx, bc)` — compute size given `BoxConstraints`
- `compose(&mut self, ctx)` — apply transforms / animations
- `paint(&mut self, ctx, scene)` — emit `vello::Scene` paint commands
- `accessibility(&mut self, ctx, props, node)` — populate an `accesskit::Node`
- `children_ids(&self) -> SmallVec<[WidgetId; 16]>` — declare children for the tree
- `accessibility_role(&self) -> Role` — declare the AccessKit role

Note the per-call division of concerns: layout is separate from paint, paint is separate from accessibility, accessibility is separate from event handling. Each widget implements exactly the methods that matter and inherits no-op defaults for the rest. **This decomposition is identical in spirit to Buiy's BSN-friendly small-component rule.**

## Retained tree, owned by Masonry

Masonry stores widgets in a `tree_arena::TreeArena`. Each widget gets a stable `WidgetId`. The tree is heterogeneous (different concrete widget types per node) via a `dyn Widget` boxed trait object. Internal handles (`WidgetPod`) own the widget plus its metadata (id, layout-rect, paint-flag, accessibility-flag).

The tree-arena is in a separately versioned crate because Masonry has explored both safe-only and unsafe-with-`UnsafeCell` versions of the storage; the crate exposes both, and Masonry picks the one that benchmarks better.

## Constraint-passing layout (Flutter / Druid style)

Masonry's layout is `BoxConstraints`-passing: parent gives child `(min_width, max_width, min_height, max_height)`; child returns its preferred `Size`. The parent then positions the child via `ctx.set_origin`. This is the Flutter model and the Druid model — **not Flexbox, not Grid, not CSS Block** in any direct sense. Masonry's `Flex` widget *implements* Flexbox-flavored algorithms, but the underlying primitive is constraint-passing.

**No Taffy.** This is a deliberate choice and an architectural divergence from the rest of the Bevy / Rust UI ecosystem (bevy_ui, woodpecker_ui, Iced, Dioxus all use Taffy). The cost: Masonry can't pick up new CSS layout features (subgrid, container queries, anchor positioning) for free as Taffy adds them. The benefit: Masonry's layout is dead-simple, and constraint-passing composes nicely with arbitrary widget hierarchies without a graph-solver. For Buiy this is the **single biggest architectural divergence** — Buiy commits to Taffy and inherits the CSS-feature pipeline; see [`lessons.md`](lessons.md) Avoid row "Constraint-passing layout instead of Taffy."

## Paint via Vello

Widget `paint` methods receive a mutable `vello::Scene` and emit drawing commands directly into it. Masonry composites at the window level by submitting the root scene to Vello, which renders into a wgpu surface. No render-graph; the paint pass is a single tree-walk.

This is a much simpler model than Bevy's render-graph (Buiy's substrate). It works because Masonry doesn't need to interleave 2D UI with 3D scene rendering, doesn't need multiple cameras, doesn't need extracted vs main-world separation. For an app-only toolkit it's a feature; for a game-engine UI it's a constraint that doesn't apply.

## Event flow

Events enter via `masonry_winit` (winit `WindowEvent` callbacks), Masonry routes them through the tree from root to focused widget (or hit-test target), then bubbles unhandled events back up. The accessibility action flow is the same shape: `accesskit_winit::Adapter` emits `ActionRequest`, Masonry routes it to the widget whose `WidgetId` matches the `accesskit::NodeId`. The id-stability comes from `WidgetId` being a `NonZeroU64` that Masonry assigns and persists across layout/paint cycles.

## Testing infrastructure

`masonry_testing` is one of the more interesting parts of the workspace. It ships:

- **Snapshot rendering** — render a Masonry tree to a deterministic image, snapshot-compare with `insta`. The rendering pipeline is set up so a widget's paint output is byte-deterministic.
- **Synthesized input** — push pointer / keyboard / accessibility-action events into a tree in tests, observe widget state, assert.
- **Tree assertions** — walk the retained tree, assert on widget types and properties at given paths.

This is *exactly* the shape of Buiy's `buiy_verify` crate (foundation [`architecture.md § 2.8`](../../specs/2026-05-07-buiy-foundation/architecture.md)). The two harnesses solve the same problem from the same direction. See [`lessons.md`](lessons.md) Borrow #5.

## What Masonry doesn't ship

- **No reactive layer** — that's Xilem's job. Masonry exposes raw widgets.
- **No theme system** — widget styling is per-instance.
- **No layout engine beyond constraint-passing** — Flex / Grid / etc. are individual widgets, not pluggable algorithms.
- **No multi-window primitive in core** — `masonry_winit` ships multi-window support; `masonry_core` knows nothing about windows.
- **No async** — events come from winit, paint goes to Vello, both synchronous. Async lives in Xilem's runner.
- **No animation system primitive** — individual widgets implement their own animations (text cursor blink).

## Comparison

| Layer | Masonry | bevy_ui | Druid (legacy) | Iced | Slint | egui |
|---|---|---|---|---|---|---|
| Paradigm | Retained, OOP-flavored | Retained (ECS) | Retained, OOP-flavored | Elm-style functional | Retained (`.slint` markup) | Immediate-mode |
| Tree storage | `tree_arena` | ECS hierarchy | `WidgetPod` recursion | Owned trait objects | Generated structs | Per-frame stack |
| Layout | BoxConstraints | Taffy | BoxConstraints | Custom (Taffy-ish) | Custom DSL | Top-down per frame |
| Paint | Vello | wgpu render pipeline | Piet (legacy) | wgpu | wgpu / sw renderer | epaint (custom) |
| Text | Parley | cosmic-text (→Parley in 0.19) | piet-text (legacy) | cosmic-text | own | own |
| A11y | AccessKit | AccessKit via bevy_a11y | AccessKit | AccessKit (recent) | AccessKit | None |
| Reactive layer above | Xilem | bevy_feathers (planned), Buiy | n/a | Iced runtime | Slint runtime | (none — IM) |

## Where Masonry is load-bearing for Buiy

- The **decomposition shape** (toolkit-vs-reactive split, `_core` no-platform crate, `_testing` separate crate, `_winit` separate adapter) is exactly what Buiy is doing with `buiy_core` / `buiy_widgets` / `buiy_verify` / `buiy_bsn`. Reading Masonry's workspace `Cargo.toml` is the closest published reference for a sane Rust-UI multi-crate split.
- The **`Widget::accessibility` method shape** — passing a mutable `accesskit::Node` to the widget, letting the widget populate it — is the same model bevy_a11y *should* have used per [`../accesskit/lessons.md`](../accesskit/lessons.md). Buiy ships the same pattern via decomposed `A11yRole` / `A11yLabel` / etc. components driving `TreeUpdate`s — different mechanism (ECS-flavored), same shape (per-widget population of the accessibility node).
- The **snapshot-rendering test harness** is a published-and-working reference for visual-regression-on-UI. See [`lessons.md`](lessons.md) Borrow #5.

## What Buiy explicitly does *not* borrow

- **Constraint-passing layout.** Buiy commits to Taffy. See [`lessons.md`](lessons.md) Avoid row "Constraint-passing layout instead of Taffy."
- **`tree_arena`-style retained tree.** Buiy's ECS World *is* the retained tree; widgets are entities with components; there is no separate tree-storage crate.
- **The `Widget` trait per se.** Buiy is BSN-component-shaped, not trait-shaped.

## Sources

- Masonry 0.4.0 docs.rs: https://docs.rs/masonry/0.4.0/masonry/
- `masonry_core` source: https://github.com/linebender/xilem/tree/main/masonry_core
- `tree_arena` source: https://github.com/linebender/xilem/tree/main/tree_arena
- `masonry_testing` source: https://github.com/linebender/xilem/tree/main/masonry_testing
- Sibling files: [`xilem-architecture.md`](xilem-architecture.md), [`linebender-stack.md`](linebender-stack.md), [`accessibility.md`](accessibility.md), [`lessons.md`](lessons.md)
