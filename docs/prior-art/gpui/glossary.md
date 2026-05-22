**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — glossary of key types and concepts

# Glossary

| Term | Definition |
|---|---|
| `App` | Process-global state owner. There is one `App` per running GPUI process. All entities live here. |
| `Application` | The entry point — `Application::new().run(...)`. Constructs the `App`, opens the initial window, drives the main event loop. |
| `AppContext` | Trait giving access to the `App`. Implemented by `App` itself, `ModelContext<T>`, `ViewContext<T>`. Lets generic code touch state regardless of which context it's running in. |
| `Window` | A single OS window. Owns its own GPU device handle, focus state, text-system instance. Lifetime managed by the platform layer. |
| `Entity<T>` | Strongly-typed handle to state of type `T` owned by `App`. Cheap to clone. Holding the handle does not grant mutable access — you call `entity.update(cx, closure)` to mutate. |
| `WeakEntity<T>` | A non-owning reference to an `Entity<T>`. Used to hold references without preventing drop. `upgrade()` returns `Option<Entity<T>>`. |
| `Model<T>` / "Model" | An `Entity<T>` where `T` is data-only (no `Render`). Used for pure state with no rendering responsibility. |
| `View<T>` / "View" | An `Entity<T>` where `T: Render`. Has a `render` method called when notified. |
| `ModelContext<T>` | Context passed to `entity.update` closures for a model entity. Gives access to the `App`, the entity itself, and observation/subscription/emit APIs. |
| `ViewContext<T>` | Context passed to `entity.update` closures for a view entity. Extends `ModelContext<T>` with window-scoped APIs (focus, key bindings, the `Window` handle). |
| `Render` | Trait implemented by view backing types. `fn render(&mut self, window: &mut Window, cx: &mut ViewContext<Self>) -> impl IntoElement;`. Called whenever the view is notified. |
| `Element` | Trait implemented by paintable elements. Defines `request_layout`, `prepaint`, `paint` — the imperative escape hatch from the otherwise-retained view model. |
| `IntoElement` | Trait for converting any element-like value (functions, `Div`, custom elements) into an `Element` for use in render output. |
| `Div` | Built-in element analogous to HTML `<div>`. Implements `Styled` and exposes `Interactivity`. The default building block — most user code composes `Div`s. |
| `Styled` | Trait giving Tailwind-style fluent setters: `.bg(color)`, `.p_4()`, `.flex()`, `.rounded(px(8.0))`, etc. Implemented by `Div` and other styleable elements. |
| `Interactivity` | Struct managing mouse, keyboard, focus, drag-and-drop wiring on an element. Accessed via methods like `.on_click(...)`, `.on_hover(...)`, `.focusable()`, `.key_context("...")`. |
| `Action` | Typed message representing a user-triggered command. Generated via the `actions!(namespace, [Name1, Name2])` macro. Dispatched via key bindings or programmatic `cx.dispatch_action(action)`. |
| `KeyContext` | String identifier attached to an element via `.key_context("MyWidget")`. Keymap entries match against context stacks up the focus path. |
| `FocusHandle` | Typed handle to a focusable element. Can be stored, passed around, used for programmatic focus via `handle.focus(cx)`. |
| `TextSystem` | Application-scoped text shaping and font management. Owns font collections and the line-layout cache. |
| `WindowTextSystem` | Window-scoped wrapper over `TextSystem`. Adds DPI scaling and the window's glyph atlas (tied to the window's GPU device). |
| `LineLayout` | Result of shaping a single visual line: glyph IDs, x positions, ascender/descender metrics, run boundaries. |
| `ShapedLine` | A `LineLayout` annotated with style information (color, decoration), ready to paint. |
| `WrappedLine` | A `ShapedLine` broken into multiple visual lines by soft-wrap. |
| `LineWrapper` | Utility that wraps text at a target width, using cached char widths for fast ASCII paths. |
| `UniformList` | Specialized layout element for lists of identical-height items. O(1) layout cost; only visible rows are paid for. Used for editor file trees, command palettes, completion popups. |
| `List` | Specialized layout element for lists of variable-height items, using a `SumTree` for O(log N) range queries. Used for editor multi-buffer view, search results. |
| `SumTree` | Zed's persistent ordered-collection data structure with cumulative summaries. Lives in a separate Zed crate; used by `List` and by Zed's text buffer. |
| `Scene` | The paint output — a flat list of typed primitives (`Quad`, `Glyph`, `Shadow`, `Path`, etc.) sorted by draw layer and ready for GPU submission. |
| `Quad` | Scene primitive: rectangle with bounds, corner radii, background, border, shadow. The most common UI primitive. |
| `Glyph` | Scene primitive: a single text glyph indexing into the alpha glyph atlas, with a per-instance color. |
| `MonochromeSprite` | Scene primitive: monochrome icon indexing the icon atlas, per-instance color. |
| `PolychromeSprite` | Scene primitive: full-color sprite (color emoji, polychrome icons), no per-instance tint. |
| `Shadow` | Scene primitive for drop shadows, rendered as a Gaussian-blurred rounded rectangle via closed-form math. |
| `Underline` | Scene primitive for text underlines/strikethroughs, painted independently of glyphs so they clip separately. |
| `Path` | Scene primitive for arbitrary filled paths (used for non-rectangular shapes like dropdown arrows). |
| `notify` | Method on `ModelContext` / `ViewContext` that marks the current entity dirty. Triggers re-render for views and re-evaluation for observers. Effect-queued — does not invoke listeners synchronously. |
| `emit` | Method on `ModelContext` / `ViewContext` that fires a typed event from the current entity. Effect-queued. |
| `observe` | Method on contexts that registers a callback to fire when another entity's `notify` is called. |
| `subscribe` | Method on contexts that registers a callback for typed events emitted by another entity. |
| Effect queue | The internal queue of pending `emit` / `notify` / observation / subscription callbacks that drains at the end of each update cycle. Provides run-to-completion semantics. |
| `gpui-ce` | Community Edition fork of GPUI ([github.com/gpui-ce/gpui-ce](https://github.com/gpui-ce/gpui-ce)) maintained outside Zed Industries after the Feb-2026 community-deprioritization announcement. |
| `gpui-component` | Third-party widget library ([longbridge/gpui-component](https://github.com/longbridge/gpui-component)) providing 60+ UI components GPUI mainline doesn't ship. Required dependency for any non-editor application of GPUI. |

## Sources

- GPUI docs.rs: https://docs.rs/gpui/latest/gpui/
- DeepWiki GPUI section: https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)
- _Ownership and data flow in GPUI_: https://zed.dev/blog/gpui-ownership
- GPUI source tree: https://github.com/zed-industries/zed/tree/main/crates/gpui/src
