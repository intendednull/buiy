**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — fine-grained signal/effect reactivity, lineage, comparison to coarse-rerender and to Dioxus signals

This file is the substantive deep-dive on the Floem feature most likely to influence Buiy if §2.7 of the foundation spec is ever reopened.

## What "fine-grained" means

In a **coarse** model (React, Iced, Elm) every state change triggers a re-render of a whole subtree, then a diff (virtual DOM or equivalent) reconciles the new tree against the previous one, then minimal DOM mutations are applied. The work is `O(tree_size)` for the diff, even if only one leaf changed.

In a **fine-grained** model (Solid.js, Leptos, SolidJS, Floem) state changes propagate through a dataflow graph of signals → derived values → effects. Each effect (including the effect that writes a node property) re-runs *only* when one of its specifically-tracked dependencies changes. There is no diff phase; there is no virtual DOM. The work is `O(changed_signals)` plus the effects fanout.

Floem implements the second model. `label(move || counter.get().to_string())` does not allocate a new label every frame — it allocates a label *once*, and the closure is wired into the reactive runtime so the label's text property re-evaluates and re-renders the run-of-glyphs only when `counter` changes.

## Floem's primitive set

From `floem_reactive`:

```rust
let (count, set_count) = create_signal(0);              // signal
let doubled = create_memo(move |_| count.get() * 2);    // derived (memoized)
create_effect(move |_| {                                 // effect
    println!("count = {}, doubled = {}", count.get(), doubled.get());
});
set_count.set(1);                                        // triggers effect once
batch(move || { set_count.set(2); set_count.set(3); }); // effect fires once at end
```

Also: `RwSignal<T>` — a single-handle variant that's `Copy` when `T: Copy`. `Scope` for ownership/disposal. `provide_context` / `use_context` for tree-scoped state injection.

## Lineage: Solid.js → leptos_reactive → Floem

The Floem README states the runtime is **"inspired by leptos_reactive"**. Leptos (the JS-style Rust web framework) was itself modeled on Solid.js. Solid.js's reactive system traces to S.js and to the Reactively library. The chain:

```
S.js / Reactively (JS)  →  Solid.js (JS)  →  leptos_reactive (Rust, web)  →  floem_reactive (Rust, native)
```

This matters for Buiy because it means Floem's reactivity is **not novel**. It is a careful Rust port of well-trodden JS primitives. The hard parts (effect dependency tracking, memoization invalidation, batch semantics, scope disposal) have years of bug-find in JS land. The Rust port has its own challenges (`'static` closures, lack of GC, Arc/Rc patterns), but the *semantics* are stable.

For a Buiy designer: if §2.7 is reopened, the reading list is not just Floem — it's the Solid.js docs first, then leptos_reactive, then Floem. Floem is the closest native-Rust example but the conceptual home is JS.

## Comparison: Dioxus signals (0.5+)

Dioxus 0.5 (released 2024-03) shifted from `use_state` hooks to `Signal<T>`. The Dioxus signals have the same shape as Floem's (read/write, copy-friendly, automatic dependency tracking) but live inside Dioxus's **virtual-DOM** model. Dioxus is fundamentally a coarse-render framework with a fine-grained-signal *update mechanism*; Floem is fully fine-grained from view to render.

In practice:

- **Dioxus**: `rsx! { ... }` re-renders on signal change → produces a new VDOM subtree → diff → patch. Signals reduce *which* subtrees re-render but the diff phase remains.
- **Floem**: signals are wired directly into specific view-node properties. No VDOM, no diff.

Buiy implication: if Buiy ever adds reactivity, the Dioxus model (signals over VDOM) is closer to the React/Iced lineage; the Floem model (signals into a persistent view graph) is closer to the Solid.js / SwiftUI lineage. The choice is real and has performance + ergonomics consequences.

See [`../dioxus/signals-and-state.md`](../dioxus/signals-and-state.md) for Dioxus's framing.

## Comparison: React's coarse re-render

React (and Iced, the Rust Elm-architecture peer) call `view(state)` whenever state changes. The output is a new `Element` tree; the reconciler diffs against the previous. React Hooks (`useState`, `useMemo`) batch state changes per render cycle but the diff still runs.

This is the model **Buiy's foundation §2.7 implicitly chose against** when it picked observers + change detection. Bevy ECS already runs system-graph re-evaluation on change detection; layering React-style coarse re-render on top would double-up. The Floem model — surgical updates from signals into a long-lived UI graph — composes better with ECS, *if* Buiy ever decides to add reactivity.

## Comparison: SwiftUI / Compose

SwiftUI's `@State` / `@Binding` / `@ObservedObject` and Jetpack Compose's `mutableStateOf` + `remember` are both fine-grained signal systems with implicit dependency tracking, dressed up in macro / DSL syntax. Floem's `RwSignal` + `create_effect` is the same shape with explicit `.get()` / `.set()` instead of property-wrapper magic.

The big difference: SwiftUI and Compose pair the signal system with a **structurally** memoized view rebuild (the view function reruns; identity-stable nodes are reused). Floem skips the view rebuild entirely — the view tree is constructed once.

## What works in Rust that doesn't work cleanly in JS

- **`Copy` signals.** `RwSignal<T>` when `T: Copy` is itself `Copy`. This lets signals propagate through closures without `clone()` ceremony — a real Rust-friendly ergonomics win that JS doesn't need.
- **Lifetime/scope disposal.** Floem uses `Scope` to bound signal lifetimes. When the owning view is dropped, the scope disposes its signals and effects. This is more explicit than JS WeakRef-style cleanup.

## What's hard in Rust that's easy in JS

- **Closure `'static` bounds.** Every signal closure must be `'static`. Capturing references is impossible; you `Copy` signals or `clone` `Rc`/`Arc`. This is a real ergonomic tax compared to JS closures.
- **Generic `T` constraints.** `RwSignal<T>` works best when `T: Copy`. Non-`Copy` `T` (e.g. `String`, `Vec<T>`) require `.with(|t| ...)` callbacks to read by reference. The ergonomic delta vs JS is noticeable.

## Performance characteristics

Floem's README claims "very high performance" and lists "fine-grained reactivity" first among features. Specific benchmarks against Iced / Dioxus / egui are not published in tree. The 0.2.0 release notes mention performance work; PR #1063 ("Faster style v2", 2026-04-11) was specifically a style-pipeline rewrite for performance.

A fair Buiy stance: trust the reactivity model's asymptotic story (it's the same as Solid.js, which has been benchmarked exhaustively on the web). The Rust-specific overhead (Arc churn, closure allocation, generic monomorphization) is the empirical question, and the answer requires running the relevant benchmark — not citing a README.

## Sources

- Floem README (reactivity claim) — https://github.com/lapce/floem/blob/main/README.md
- `floem_reactive` docs — https://docs.rs/floem_reactive/
- Leptos book on reactivity — https://book.leptos.dev/reactivity/index.html
- Leptos appendix on reactive graph — https://book.leptos.dev/appendix_reactive_graph.html
- Solid.js docs — https://www.solidjs.com/docs/latest
- PR #1063 "Faster style v2" — https://github.com/lapce/floem/pull/1063
- Dioxus signals announcement (0.5) — https://dioxuslabs.com/blog/release-050/
- Cross-link: [`../dioxus/signals-and-state.md`](../dioxus/signals-and-state.md)
