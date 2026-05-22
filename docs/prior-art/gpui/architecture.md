**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — architecture: the four-stage render pipeline, the element/view/entity model, the per-platform abstraction layer

# Architecture

GPUI's architecture is best understood at three levels, each with its own paradigm:

1. **Application state** lives in `Entity<T>` handles owned by a single global `App`. State updates flow through an effect queue with run-to-completion semantics. _Closest analogue: Erlang/Elixir actor model with a single supervisor; emphatically not React's hook-driven re-render._
2. **Views** (anything implementing the `Render` trait) declaratively rebuild their element tree when notified. _Closest analogue: React's `render()` method — but without virtual DOM diffing; the rebuilt element tree just flows into the imperative paint pass below._
3. **Elements** (anything implementing the `Element` trait) imperatively perform a four-stage paint: layout → prepaint → paint → GPU submission. _Closest analogue: a game engine's render command buffer assembly._

The hybrid is the point. Views give you declarative high-level UI; elements give you escape hatches when the declarative path can't express what you need (custom hit-testing, oddly-shaped clipping, virtualized lists with O(1) layout). _The README's "hybrid immediate and retained mode" phrasing is the load-bearing description; this is not pure retained-mode._

## The four-stage render pipeline

Per the [DeepWiki summary of GPUI](https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)), every frame walks every element through four phases, managed by `gpui::Window`:

### Stage 1: Layout

Each element's style is translated into Taffy layout instructions. Taffy resolves the entire tree's flex/block layout in one pass and returns resolved geometry per node. _Same crate Buiy uses. Same `=0.10.1` pin discipline (GPUI pins exactly; Buiy follows Bevy's pin)._

Specialized list elements bypass Taffy:

- **`UniformList`** — O(1) layout for identical-height items (used in Zed's file tree, command palette, completion popup). The layout phase computes `count * row_height` directly; only visible rows are paid for.
- **`List`** — O(log N) layout via a `SumTree` (Zed's persistent ordered-collection structure) for variable-height items.

The lesson here is structural: Taffy is the right default but it is not the right primitive for "10k-row file tree." Specialized layout elements that skip Taffy entirely earn their complexity at high item counts. Buiy's [foundation `architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md) ("drives Taffy ourselves; extends layout without waiting for upstream") leaves room for the same escape-hatch pattern.

### Stage 2: Prepaint

Elements receive stable identifiers and register their hitboxes with the `Interactivity` machinery. The hit-test tree is rebuilt from scratch each frame (effectively immediate-mode hit-testing on top of retained-mode painting). Focus targets, mouse listeners, and key-binding contexts are registered here.

This is the phase where the "hybrid" pays off: declarative views generate elements; elements imperatively register their input affordances. There's no separate "input system" — input wiring is a side effect of paint.

### Stage 3: Paint

Each element emits drawing primitives into a flat `Scene` — typed records of `Quad`, `Glyph`, `Shadow`, `Path`, `MonochromeIcon`, `PolychromeIcon`, `Underline`, `Surface`. The scene is sorted into draw layers and batched per primitive type.

The scene primitives are deliberately small: a `Quad` is bounds + corner radii + background + border. Borders, gradients, shadows, clips — everything is expressed as one of ~8 primitive types. _This is the analogue of GPU "draw calls = vertex instances" thinking applied to UI._

### Stage 4: GPU submission

The platform-specific renderer (Metal on macOS; wgpu on Linux post-migration; DirectX 11 on Windows) takes the sorted scene and issues GPU commands. Each primitive type has its own shader pipeline (see [`gpu-rendering.md`](gpu-rendering.md)):

- Quads, shadows, paths, glyphs render in their own batched instance passes
- A glyph atlas holds rasterized alpha masks; color is applied per-instance at draw time so the same atlas serves any tint
- An icon atlas works the same way for monochrome icons; polychrome icons get a separate full-color atlas

The four-stage decomposition is **deeply analogous to a game engine's frame loop** (update → cull → record commands → submit). Antonio Scandurra's _videogame_ blog post is the canonical statement of this design philosophy: treat the screen as a parallel-drawn scene of typed primitives, not as a DOM to be diffed.

## State, ownership, and reactivity

Per [_Ownership and data flow in GPUI_](https://zed.dev/blog/gpui-ownership):

- **`App`** owns all application state. There is one `App` per process.
- **`Entity<T>`** is a typed handle into `App`-owned storage. Cheap to clone; weak-reference-able via `WeakEntity<T>`. Holding an `Entity<T>` does not give you mutable access to the underlying `T`.
- **`AppContext`** is the trait that lets you actually touch state. To mutate an entity, you call `entity.update(cx, |state, cx| { ... })` which lifts the state out of the `App` onto the stack, runs your closure, and restores it. This sidesteps Rust's borrow checker for entity-to-entity update chains without `RefCell` runtime cost.
- **`Context` variants:** `ModelContext<T>` is passed to update closures for data-only entities; `ViewContext<T>` extends it with window-scoped access (`Window` handle, focus operations, key bindings) for renderable entities.
- **Effect queue:** `emit(event)` and `notify()` do not invoke listeners synchronously. They enqueue effects that drain at the end of the current `update` call. This gives "run-to-completion" semantics — your update closure sees a coherent state, and downstream subscribers always observe a quiescent state.

The effect queue is the **decisive architectural choice** that distinguishes GPUI from naïve React-style reactive UIs. Reentrancy bugs (handler triggers handler triggers handler) are structurally prevented; the cost is one extra trip through the queue per notification.

### Comparison to Bevy's ECS

Buiy is built on Bevy's ECS, which solves the same problem (typed handles to global state, structured update flow) with different primitives:

| GPUI | Bevy ECS |
|---|---|
| `App` (global owner) | `World` (global owner) |
| `Entity<T>` (typed handle) | `Entity` (untyped handle) + `Query<&T>` |
| `entity.update(cx, |state, cx| ...)` | `Query<&mut T>` in a system + `Commands` |
| `emit(event)` / `notify()` | `EventWriter<E>` / `Trigger`/observer / change detection |
| Effect queue | Stage barriers + command flush + event channel drain |
| `WeakEntity<T>` | `Entity` (always weak; dangle is handled by `Query::get`) |

The semantic match is closer than it looks. Both refuse to let arbitrary code mutate arbitrary state at arbitrary times; both fence updates into well-defined drainage points. The difference is composability: Bevy's ECS composes _systems_ (free functions tagged with `Query` parameters) while GPUI composes _closures over entities_. **Buiy doesn't need to invent GPUI's effect queue — Bevy's ECS already provides equivalent semantics.**

## Per-platform abstraction layer

GPUI does not use [`winit`](https://crates.io/crates/winit) for windowing or [`wgpu`](https://crates.io/crates/wgpu) uniformly for graphics. Instead, it has three platform code paths in `crates/gpui/src/platform/`:

- **`platform/mac/`** — Cocoa via `objc2`/`core-graphics`/`core-text`; Metal renderer via the `metal` crate; macOS-specific keyboard, IME, clipboard, menu integration.
- **`platform/linux/`** — Wayland + X11 windowing via the `wayland-client` + `x11rb` crate families; renderer historically Blade (Vulkan), migrating to wgpu per PR [#46758](https://github.com/zed-industries/zed/pull/46758); FreeType/HarfBuzz-adjacent text shaping.
- **`platform/windows/`** — Win32 via the `windows` crate (v0.61); DirectX 11 renderer; DirectWrite for text shaping and ClearType rendering ([Zed on Windows announcement](https://zed.dev/windows)).

There is no `gpui_wgpu` unified backend — that was an aspirational naming in some external write-ups but does not match the source tree. The Linux backend is the only one moving to wgpu; macOS and Windows remain on native APIs.

### Why three backends instead of one

The reasoning surfaces across Zed's blog posts and HN threads: native APIs give better text quality, lower input latency, and tighter OS integration (menu bars, dock, system color schemes, IME composition) than a cross-platform abstraction can deliver. wgpu was acceptable for Linux because:

1. The Blade backend was causing maintenance pain ("a mess and causes several issues" per the PR description)
2. wgpu's Linux Vulkan target is mature
3. Linux has no single canonical text renderer to compete with (unlike Core Text or DirectWrite)

**Buiy's commitment to Bevy's render graph + wgpu (foundation §2.2) is the opposite bet** — one backend, accepted quality compromises on each platform, in exchange for simplicity. GPUI's three-backend strategy is the empirical signal that this bet has real costs. If Buiy ever finds wgpu's text rendering inadequate on Windows (DirectWrite-quality ClearType is hard to match), GPUI's split-backend approach is the precedent for what a remediation would look like.

## What lives in `crates/gpui/src/`

Approximate top-level organization (subject to change between Zed releases):

- `app.rs`, `app/` — `App`, `Application`, `AppContext`, the effect queue.
- `entity.rs`, `subscription.rs` — `Entity<T>`, `WeakEntity<T>`, `observe`, `subscribe`.
- `window.rs`, `view.rs` — `Window`, `View<T>` (via `Render`), `ViewContext`.
- `elements/` — the element catalog: `Div`, `UniformList`, `List`, `Img`, `Svg`, custom-element traits.
- `interactive.rs`, `actions.rs`, `key_dispatch.rs` — input wiring, the action system, key contexts and bindings.
- `text_system/`, `text_system.rs` — `TextSystem`, `WindowTextSystem`, `LineLayout`, `ShapedLine`, `WrappedLine`.
- `platform/`, `platform/mac/`, `platform/linux/`, `platform/windows/` — platform abstraction.
- `scene.rs`, `geometry.rs`, `color.rs` — primitive types fed into the GPU pipeline.
- `style.rs`, `styled.rs` — the `Styled` trait that gives elements Tailwind-style fluent styling (`.bg(red()).rounded(px(8.0))`).
- `taffy.rs` — the Taffy integration layer.

The crate is monolithic — there is no `gpui_core` / `gpui_text` / `gpui_a11y` split. Everything ships together. Compile times are correspondingly noticeable; cargo-bloat-style breakdowns put the crate in the multi-MB range. **Buiy's foundation §2.8 commitment to a multi-crate workspace (buiy_core, buiy_text, buiy_widgets, etc.) is the explicit anti-pattern to GPUI's monolithic shape.**

## Sources

- DeepWiki GPUI section (four-stage pipeline): https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)
- _Leveraging Rust and the GPU to render user interfaces at 120 FPS_ (Scandurra 2023): https://zed.dev/blog/videogame
- _Ownership and data flow in GPUI_: https://zed.dev/blog/gpui-ownership
- GPUI README: https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
- GPUI source tree: https://github.com/zed-industries/zed/tree/main/crates/gpui/src
- GPUI `Cargo.toml`: https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml
- Blade→wgpu migration PR #46758: https://github.com/zed-industries/zed/pull/46758
- Zed on Windows: https://zed.dev/windows
- docs.rs/gpui: https://docs.rs/gpui/latest/gpui/
