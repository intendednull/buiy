**Date:** 2026-05-22
**Status:** active
**Subject:** Xilem — reactive view-tree-diffing UI architecture on top of Masonry

# Xilem architecture

Xilem is a **reactive UI library** in the React / SwiftUI / Elm family, adapted to Rust's ownership rules. Raph Levien's 2022-05-07 essay ["Xilem: an architecture for UI in Rust"](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html) is the design document; the crate is the implementation. As of 0.4.0 (2025-10-29), the implementation has caught up with most of the paper's claims, though still experimental.

## The single sentence

> The app state is an arbitrary `'static` Rust value. A pure function maps state to a view tree. Successive view trees are diffed; the diff drives mutations into Masonry's retained widget tree; user events propagate back as messages that mutate the state, triggering re-render.

That's it. The rest is mechanism.

## Lifecycle of a view

Every Xilem `View<State, Action, Element>` participates in a three-phase lifecycle the framework orchestrates:

1. **`build`** — called the first time the view appears in the tree. Returns `(Element, ViewState)`. The `Element` is the Masonry widget; the `ViewState` is the view's bookkeeping (e.g., previous-value memos).
2. **`rebuild`** — called every subsequent frame the view is present. Diffs `&self` against the previous frame's `&self`, mutates the Masonry widget through a `Pod<Element>` handle, updates the `ViewState`.
3. **`teardown`** — called when the view is removed. Cleans up the Masonry widget.

Plus a fourth method:

4. **`message`** — called when a user-action message bubbles up to this view. Routes the message to a callback that mutates `&mut State`. Returns `MessageResult` telling the runtime whether to re-render.

The pattern is the same as Elm's `update` / `view` / message-passing, but with Rust mutable references rather than Elm's immutable command-returning model.

## ID paths, not single IDs

A key Xilem invention (from the paper): **message routing uses id paths**, not single id-per-element values. Every view in the tree has an `Id` at its level; the route from root to leaf is `[Id, Id, Id, …]`. When Masonry generates a `WidgetEvent` from user input, Xilem routes the event up the path: each ancestor view's `message` method gets a chance to consume or transform it, with mutable access to the application state at that level (via lensing/adaptation — see Adapt nodes below).

The benefit over React-style "everything bubbles to the root reducer": components compose without forcing the root state shape to know about the leaf's data. The cost: every view has to participate in routing.

## Adapt nodes (lensing, descendant of Druid)

Druid introduced `Lens` for accessing a subfield of parent state. Xilem's `Adapt` view does the same job: an `Adapt` wraps a child view, accepts a parent `State`, and projects it to a child `ChildState` (via closure). When messages bubble back up, the closure runs in reverse to apply the child's mutation to the parent's slice. This makes components composable across state-type boundaries without the parent knowing the child's concrete state type.

`Adapt` is the reason Xilem doesn't need React-style context providers, Redux selectors, or SwiftUI `@Binding`. It's pure-functional state projection, written in Rust.

## View sequences

`ViewSequence<State, Action>` is a trait implemented by tuples, vectors, and option types of views. It lets a parent view contain N children of varying types and arities. `flex((button1, button2, conditional.then(|| button3)))` works because each tuple position implements `View`, the tuple itself implements `ViewSequence`, and `flex` accepts the sequence. This is the Rust-flavored answer to React's `children: ReactNode`.

## Memoization

`Memoize<Data, F: Fn(&Data) -> View>` skips the inner view's `build`/`rebuild` when `Data` is `PartialEq`-equal to the previous frame's. Xilem applications memo subtrees that don't depend on changed state. The pattern matches React's `React.memo` / SwiftUI's `EquatableView`, but Rust's `PartialEq` makes it cheap because the equality check is monomorphized.

## How `'static` types coexist with state mutation

The hardest design constraint: views must be `'static` (Masonry stores them), but they need to capture state references. Xilem's answer:

- Views capture `State` only through callbacks that the framework invokes with `&mut State`.
- The runtime owns the `State` and passes it through `ViewCtx` to each view's `message` method.
- View functions return `impl View<State, Action> + use<>` — the `+ use<>` precise-capturing syntax (stable Rust as of recent editions) tells the compiler "this return type captures nothing implicit", letting the views be `'static`.

This is the design choice the paper builds toward. Pre-`+ use<>`, the workaround was the `xilem_core` view-builder boilerplate; post-stabilization, the syntax is one Rust feature carrying most of the ergonomic weight.

## Comparison map

| Concept | Xilem | React | Elm | SwiftUI | Solid | Dioxus |
|---|---|---|---|---|---|---|
| State source | One owned `State` | Hooks/context | Single Model | `@State`/`@StateObject` | Signals | Hooks |
| Re-render trigger | Message routes to root state mutation | setState/dispatch | Update returns new Model | `@Published` change | Signal write | setState |
| View identity | id path | key prop | virtual DOM key | structural id + `@id` | structural | key prop |
| Granularity | View-tree diff (Masonry mutates widget tree) | VDOM diff (renderer mutates DOM) | VDOM diff | Apple-internal diff | Signal-fine-grained | VDOM diff |
| Composition primitive | `Adapt` + view functions | Component functions | Component records | View structs | Components | Component functions |
| Async | `tokio` (built into runtime) | Suspense + effects | Cmd/Task | `async`/`await` | Resources | use_future |
| Reactivity layer | View-diffing (coarse) | VDOM diff (coarse) | VDOM diff (coarse) | Apple's "graph" (intermediate) | Signals (fine) | VDOM diff (coarse) |

Xilem is **coarse-grained reactive** (view-diff) rather than **fine-grained reactive** (signals à la Solid / Leptos). Raph's reactive-UI series of blog posts argues this is a deliberate choice: signals introduce a graph that must be tracked, fine-grained diff has higher constant overhead, and coarse-grained diff with good memoization wins for most UI workloads. The tradeoff is debatable — see [`critiques-and-open-problems.md`](critiques-and-open-problems.md).

## Async integration

Xilem ships with a `tokio` runtime baked into the runner. Async tasks complete and emit messages; the runtime routes them to the right view path. This is Xilem's answer to Bevy's "async tasks via `Task<T>` + polling system" pattern, and to React's `useEffect`.

## What Xilem doesn't do

- **No CSS / stylesheet.** Styling is per-view setter chains. The 0.4.0 release added "styling properties" as a first-class concept on widgets, but there's no separation between content and style.
- **No theming / token system** beyond what Masonry exposes. There is no light/dark variant binding, no OS-preference subscription.
- **No layout engine beyond Masonry's Flutter-style constraint layout.** No Flexbox, no Grid as named CSS concepts (`flex` is a widget name, not the CSS algorithm).
- **No localization / i18n primitives.**
- **No animation system** (yet) beyond what individual widgets implement (cursor blink, etc.).

These omissions are characteristic of pre-1.0 reactive-paradigm research. Buiy explicitly does include theming, i18n, and animation — see [`lessons.md`](lessons.md).

## Where the architecture is load-bearing for Buiy

Buiy's foundation [`architecture.md § 2.7`](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to "observers + change detection only" — i.e., no signal/computed/effect layer in v1. The Xilem paradigm is the closest existing-art reference if Buiy ever picks up a follow-on reactivity sub-spec. Specifically:

- The view-as-function-of-state shape adapts cleanly onto Bevy ECS by treating the `World` as `State` and a system as the view function.
- `Adapt`-style state projection maps onto Bevy queries (a query *is* a state lens).
- ID-path message routing maps onto Bevy entities + observers, where the entity is the id-path-equivalent.

See [`lessons.md`](lessons.md) Borrow #1 for the full shape.

## Sources

- Xilem paper (Raph Levien, 2022-05-07): https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html
- Xilem 0.4.0 docs.rs: https://docs.rs/xilem/0.4.0/xilem/
- `xilem_core`: https://docs.rs/xilem_core/latest/xilem_core/
- "This Month in Xilem" monthly posts (Linebender blog, 2024 series): https://linebender.org/blog/
- Sibling files: [`masonry-toolkit.md`](masonry-toolkit.md), [`linebender-stack.md`](linebender-stack.md), [`history.md`](history.md), [`lessons.md`](lessons.md)
