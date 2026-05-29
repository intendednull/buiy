**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — the element model: `Element`, `Render`, `Div`, `Styled`, `Interactivity`, and how the hybrid immediate/retained decomposition compares to React, Elm, and immediate-mode

# Element tree

GPUI deliberately does not pick a single UI paradigm. The README's "hybrid immediate and retained mode" is not marketing hedge — it's the architecture. Two trait families divide the labor:

- **`Render`** is the retained-mode entry point. A view implements `Render` and returns an element tree each time it's notified.
- **`Element`** is the immediate-mode escape hatch. An element directly participates in the four-stage paint (layout/prepaint/paint/GPU submit) and can do whatever it wants inside each stage.

`Div` is the swiss-army built-in element that implements `Element` and also exposes the `Styled` and `Interactivity` traits. Most user code never implements `Element` directly — they compose `Div`s in `Render::render`. But the escape hatch is always available, and Zed itself uses custom `Element` implementations for terminal rendering, code editing, and the file tree.

## The `Render` trait — retained-mode views

A view is any `Entity<T>` where `T: Render`. The trait is roughly:

```rust
pub trait Render: 'static + Sized {
    fn render(&mut self, window: &mut Window, cx: &mut ViewContext<Self>)
        -> impl IntoElement;
}
```

Each frame, `Window` walks the root view tree and calls `render` on each view that has been marked dirty by `cx.notify()`. The returned element tree feeds the four-stage paint.

**Crucially, there is no virtual DOM and no diff.** GPUI does not compare last-frame's elements to this-frame's elements to compute a minimal patch. Instead, the entire element tree is freshly constructed, laid out, prepainted, painted, and submitted every frame the view is dirty. The optimization is "don't re-render unchanged views," not "compute a diff over rendered output."

This is structurally cheaper than React's diff and more expensive than immediate-mode's "always rebuild everything." It's the **dirty-view-rebuild** model: views that didn't notify don't re-render; views that did notify rebuild fully.

### What notifies a view

- Direct: `cx.notify()` inside an update closure.
- Observation: `cx.observe(other_entity, |this, _other, cx| cx.notify())`.
- Event subscription: `cx.subscribe(other_entity, |this, _other, event, cx| { ... cx.notify() })`.
- Implicit through `Entity::update` if the entity is itself rendered.

The model is "view subscribes to data; data notifies on change; view rebuilds." Closer to MobX or Solid's reactive subscriptions than to React's hook-driven re-render. **No `useState`, no `useEffect`, no `useMemo`.** State lives in entities; views subscribe.

## The `Element` trait — imperative escape hatch

An element implements (approximately):

```rust
pub trait Element: 'static + IntoElement {
    type RequestLayoutState;
    type PrepaintState;
    fn request_layout(&mut self, /* ... */) -> (LayoutId, Self::RequestLayoutState);
    fn prepaint(&mut self, /* ... */) -> Self::PrepaintState;
    fn paint(&mut self, /* ... */);
}
```

Each stage gets a typed bag of state passed forward. Custom elements implement these directly when they need:

- Layout that Taffy doesn't express (Zed's code editor lays out lines manually, integrating soft-wrapping, gutters, line-height anomalies, inlay hints, scroll position — none of which fit Flexbox)
- Painting that doesn't decompose into standard primitives (terminal cell grids with their own batching)
- Hit-testing that needs custom logic (the editor's click-to-cursor-position math)

This is **literally immediate-mode UI** inside the otherwise-retained framework: at paint time, the element draws whatever it wants. The retained-mode part is "the view that owns this custom element only re-renders when dirty." The immediate-mode part is "inside the custom element's paint method, you can do anything."

## `Div` and the `Styled` trait

The vast majority of GPUI UI code is built from `Div`. It is `<div>` from HTML, adapted to Rust:

```rust
div()
    .flex()
    .flex_col()
    .gap_2()
    .p_4()
    .bg(rgb(0x1e1e2e))
    .rounded(px(8.0))
    .child(label("Hello"))
    .child(button("OK").on_click(...))
```

The fluent API comes from the `Styled` trait, which exposes Tailwind-style style methods on any element that implements it. `Div` is `Styled`, so the chain compiles to a sequence of style-setter calls.

**The styling model is inline.** There is no stylesheet, no theme tokens at the language level, no selector cascade. Theming is achieved by Rust convention: define functions that return pre-styled elements (`fn primary_button(label: &str) -> Div { ... }`), or pass theme structs explicitly. Zed has a substantial theme system layered on top, but it lives in user code, not in GPUI.

This is the **anti-CSS bet** — same shape as Tailwind in JavaScript-land, same shape as Iced's per-widget `Style` structs, same shape as Slint's inline properties. The escape from CSS is deliberate. The cost is "no stylesheet hot-reload" and "no contrast linting at the CSS layer." Buiy's foundation §2.5 makes the opposite bet: token-based theme assets, hot-reloadable, OS-pref-bound.

### `Interactivity` — input wiring on elements

The `Interactivity` struct attaches mouse/keyboard/focus behavior to elements:

```rust
div()
    .on_click(cx.listener(|this, _ev, cx| { /* handler with view access */ }))
    .on_hover(|hovering, cx| { ... })
    .focusable()
    .key_context("MyWidget")
```

Listeners use `cx.listener` to capture a weak handle to the owning view; they get `&mut View` and `&mut ViewContext` when invoked. This is the typed equivalent of React's `useCallback(handler, [deps])` — but the dependencies are tracked structurally (the view's `Entity<T>`), not by array.

`key_context` registers a string identifier on the element; keybindings (defined elsewhere in `keymap.json`-style configuration) dispatch by matching contexts up the focus path. See [`text-and-input.md`](text-and-input.md) for the action system that consumes this.

## Comparison to other paradigms

### vs React

| | React | GPUI |
|---|---|---|
| State | Local hooks + global stores | Global `App` + typed `Entity<T>` handles |
| Re-render trigger | `useState` setter or context change | `cx.notify()` on the owning entity |
| Render output | Virtual DOM tree | Element tree (no diff; rebuilt directly into paint) |
| Diffing | Yes — `react-reconciler` minimizes real-DOM ops | No — entire element tree rebuilds |
| Effect model | `useEffect` deps + cleanup | Observe/subscribe with effect queue |
| Escape hatch | `useRef` + imperative DOM | `Element` trait — custom paint |

The most consequential difference is **no diffing**. React invests in reconciliation because the DOM is expensive to mutate. GPUI's "DOM" is the GPU command list, which is cheap to rebuild every frame. _This is the immediate-mode insight applied to retained-mode views._

### vs Elm (and Iced)

| | Elm / Iced | GPUI |
|---|---|---|
| State | Single global `Model` | Many `Entity<T>` handles |
| Update | Pure `Model + Msg -> Model` | Mutable closures over entities |
| View | Pure `Model -> Html` | `Render::render(&mut self)` (mutable) |
| Side effects | `Cmd`/`Task` returned from update | Direct calls inside update closures + effect queue |

The Elm Architecture's purity is gone. GPUI is unapologetically mutable — closures take `&mut self`, listeners mutate views directly. **The discipline lives in the effect queue, not in functional purity.** This is a defensible trade for Rust: `&mut` references are already type-checked to be unique; purity buys nothing further; mutation is just easier.

### vs immediate-mode (egui)

| | egui | GPUI |
|---|---|---|
| Frame work | Rebuild widget tree every frame | Rebuild only dirty views' element trees |
| Idle cost | Continuous rebuild (often gated to 60 FPS) | Zero (no view dirty = no render call) |
| State | `Memory` map keyed by ID | Typed entities |
| Layout | Manual, called inline | Taffy, called by framework |

GPUI's win over egui is **idle cost**. A Zed window with no input does zero CPU work. An egui window must rebuild its tree every frame even if nothing changed (though eframe can throttle to "request_repaint_after"). For Zed's "editor at rest waiting for keystroke" use case, this is decisive.

GPUI's loss to egui is **simplicity**. egui's "everything is `ui.button("X")`" is dramatically easier to learn. GPUI requires understanding entities, views, contexts, render traits, element traits, the effect queue. The README's pre-1.0 disclaimer is partly an honesty signal about the learning curve.

### vs Bevy ECS UI (and Buiy)

This is the most architecturally relevant comparison.

| | Bevy ECS / Buiy | GPUI |
|---|---|---|
| State container | `World` with typed `Component`s | `App` with `Entity<T>` |
| Dirty propagation | `Changed<T>` queries + observers | `cx.notify()` + effect queue |
| Update flow | Systems scheduled by Bevy | Closures called by event dispatch |
| Layout | Buiy: Taffy via own integration | Taffy via own integration (same crate!) |
| Render | Buiy: Bevy render graph + custom passes | Platform-native: Metal / wgpu / DX11 |
| Authoring | ECS spawn + BSN | Rust `render()` method composing `Div`s |
| Reactivity | Observers + change detection | Effect queue with run-to-completion |

**The semantic match is uncanny.** Both are typed-handle-into-global-storage models with dirty-flag propagation and externalized layout. Buiy gets the same primitives "for free" from Bevy's ECS that GPUI hand-built. The big divergence is **authoring**: Buiy commits to BSN (declarative, hot-reloadable, asset-driven); GPUI commits to Rust code only (no asset, no hot-reload of layout — only theme/keymap).

Three things Buiy can learn from GPUI's element model:

1. **Provide an imperative escape hatch.** Buiy's component model is declarative-first; the equivalent of GPUI's custom `Element` is "implement your own render-graph node that consumes Buiy's resolved layout/style data." Foundation [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md) leaves the door open ("Render pipeline — custom Bevy render passes..."); make sure third-party widget authors can actually walk through it.
2. **Specialize for high-cardinality lists.** GPUI's `UniformList` and `List` skip Taffy entirely. Buiy's widget catalog will need the same — a 100k-row table or a virtualized feed cannot afford O(n) Taffy invocations. This is a sub-spec for `buiy_widgets`.
3. **Inline styling has real costs.** GPUI's "everything is fluent setters" is fast to type but resists hot-reload, contrast linting, and theme swap. Buiy's token-driven theme is the deliberate counter-bet; the lesson is to keep that bet enforced even when an inline-styling API would feel ergonomic.

## Sources

- DeepWiki GPUI section: https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)
- GPUI docs.rs (Render, Element, Div, Styled): https://docs.rs/gpui/latest/gpui/
- _Ownership and data flow in GPUI_: https://zed.dev/blog/gpui-ownership
- GPUI README: https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
- Cross-link: bevy_egui lessons (Zed-vs-egui comparison): [`/home/user/buiy/docs/prior-art/bevy-egui/lessons.md`](../bevy-egui/lessons.md)
- Cross-link: Iced Elm-architecture: [`/home/user/buiy/docs/prior-art/iced/elm-architecture.md`](../iced/elm-architecture.md)
