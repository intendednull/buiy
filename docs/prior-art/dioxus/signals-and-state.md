**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — signals (0.5+), Stores (0.7+), comparison vs React useState / Solid / Sycamore

# Signals and state

Dioxus's reactivity primitive since version **0.5.0 (2024-03-21)** is the **signal**: a `Copy` cell whose reads are tracked by a subscription graph, so any component or effect that read the signal during its last execution is re-run when the signal is written. In 0.7 (2025-10-31), Dioxus added **Stores** — a derivable trait for nested reactive state where individual fields can be subscribed to independently.

## Signal API

```rust
let mut count = use_signal(|| 0);          // create
let n = count();                            // read (subscribes the current scope)
count.set(42);                              // write
count += 1;                                 // also write
let doubled = use_memo(move || count() * 2); // derived
use_effect(move || println!("count is {}", count()));
```

Three load-bearing properties:

1. **`Signal<T>: Copy` even when `T: !Copy`.** This is the defining property — it's what makes signals trivially movable into closures, async tasks, child components, and event handlers without `.clone()` / `Arc` ceremony. Implemented via a custom **generational-box** allocator that stores the value in a scope-owned arena and hands out `Copy` handles; the value is dropped when the owning scope unmounts. The generational-box crate (also DioxusLabs-stewarded) was carved out of `dioxus-signals` and is reusable by other projects.
2. **Implicit subscription.** Reading a signal inside a component body subscribes that component to writes. Reading inside an event handler or async task does **not** subscribe the enclosing component (handlers/tasks don't re-run on write). This "subscribe only in the render path" rule is identical to Solid and the inverse of React's render-twice / `useEffect` model.
3. **`Send + Sync` opt-in.** Signals are `Send + Sync` (since 0.5), which means background tasks can write to them. The scheduler still applies the resulting re-render on the main scheduler thread.

## Stores (0.7+)

```rust
#[derive(Store)]
struct TodoList {
    items: BTreeMap<u32, TodoItem>,
    filter: Filter,
}

#[derive(Store)]
struct TodoItem {
    text: String,
    done: bool,
}

let mut todos = use_store(|| TodoList { ... });
todos.items.write().insert(id, TodoItem { ... });
// Only components reading items[id] re-render, not those reading items[other_id].
```

The `Store` derive macro generates per-field signal-like accessors so each field of a struct is independently subscribable. For `BTreeMap` and other collections, individual keys are subscribable — you get fine-grained reactivity matching what Solid's nested stores or Vue's reactive proxies provide, without proxy magic (everything is compile-time-generated).

This is the highest-leverage 0.7 feature for application-scale state — Signals scale awkwardly for nested data (every write to a nested field re-renders every reader of the outer signal). Stores fix this without forcing users to manually split state into many small signals.

## Comparison: signals across frameworks

| Framework | Primitive | `Copy`? | Subscription | Nested-state story | Async-write |
|---|---|---|---|---|---|
| **Dioxus 0.5+** | `Signal<T>` | Yes | Implicit in render path | `Store` derive (0.7) | `Send + Sync` |
| **Solid.js** | `createSignal<T>()` | n/a (JS) | Implicit in tracked scope | `createStore` (proxy-based) | n/a |
| **Sycamore (Rust)** | `Signal<T>` (Rc-based) | No (`Rc<RefCell<T>>`) | Implicit via `Context` | Manual nested signals | n/a |
| **Leptos (Rust)** | `RwSignal<T>` (arena-stored) | Yes (since 0.6) | Implicit in tracked scope | `Store<T>` macro (since 0.7) | Yes |
| **React** | `useState<T>` | n/a (JS) | Component-level (no granular tracking) | None — re-render whole component | n/a (await + setState) |
| **Bevy ECS** (Buiy's substrate) | `Changed<T>` filter | n/a | Query-explicit | Component-per-property | System scheduler |

Two takeaways:

1. **Rust signal frameworks converged on the same pattern.** Leptos and Dioxus independently arrived at *generational-arena-stored, Copy-by-default* signals (Leptos: `RwSignal`/`StoredValue`; Dioxus: `Signal`/`generational-box`). Sycamore is the older Rc/RefCell shape and is harder to use as a result. The convergence is strong evidence that **the Copy-by-default signal is the right Rust UI primitive when the framework owns the scheduler** — which Dioxus and Leptos do, and Bevy (and therefore Buiy) does not.
2. **Solid is the conceptual model.** Both Dioxus signals and Leptos signals trace their semantics to Solid's `createSignal` + `createEffect` (Ryan Carniato, 2021–). Subscribe-on-read in tracked scopes, fine-grained re-render via topological dependency graph, escape hatch for batched/transitive updates.

## Comparison to Bevy/Buiy reactivity

Bevy has **no signal primitive**. Reactivity is via:
- `Changed<T>` query filters (a system runs only on entities whose component changed).
- Observers (PR-merged for 0.14+; per-entity event subscriptions).
- `Resource` change detection (global state mutation triggers).

The Buiy foundation spec ([foundation README § 1.3](../../specs/2026-05-07-buiy-foundation/README.md), non-goals): *"A reactive component model with signals/computed/effects in v1. Bevy's observers + change detection are the reactivity primitive. A signal-style layer is a follow-up sub-spec, not part of foundation."*

Dioxus's experience clarifies what such a follow-up would need:
- A generational-arena-based storage so signals are `Copy`.
- A subscription graph that tracks *which Bevy entity / system* read which signal.
- An effect-execution boundary that aligns with Bevy's system scheduler (signals written during a system commit at end-of-system).
- A Store-shape for ECS-component-shaped state (likely already covered by `Changed<T>` for top-level components — the hard case is "subscribe to one field on one component for one entity").

The Solid/Leptos/Dioxus convergence suggests that a Buiy signal layer **should not invent a new model**. The pattern is settled: generational-arena, Copy-by-default, subscribe-on-read, batched effect commit.

## Performance characteristics

- **Reads are cheap** — `Copy` value plus a subscription-set insert (HashSet).
- **Writes are O(subscribers)** — every subscribed scope is marked dirty.
- **Memory overhead is per-allocation** — each `use_signal` creates an arena slot; signal values that survive a scope's lifetime live in the scope's bump arena.
- **Nested updates** — pre-Stores (0.5–0.6), updating one field of a nested struct in a signal re-renders all readers. Stores (0.7+) fix this; pre-0.7 code that hadn't been migrated still pays the cost.

The known weakness is **diamond dependencies**: if A depends on B and C, and B and C both depend on D, then writing D currently re-runs A twice (once via B, once via C). Solid/Leptos handle this via topological-order batching; Dioxus 0.7 has improved but not fully solved it — see [`open-problems.md`](open-problems.md) § "Reactivity edge cases."

## Implications for Buiy

- **If Buiy ever adds signals, follow the Dioxus/Leptos convergence pattern, not the Sycamore/Rc pattern.** Generational-arena, `Copy`-by-default, subscribe-on-read. The pattern is settled.
- **The Store-derive macro is the application-scale unlock.** Without per-field subscription, signals don't scale to non-trivial app state. Any Buiy signal sub-spec must include a Store-shape primitive from day one — not as a follow-up — because the difference between "signals demo well" and "signals work at app scale" is exactly the Store layer.
- **Solid is the conceptual ancestor.** When designing a Buiy signal layer, read Ryan Carniato's signal-rendering writeups before reading the Dioxus or Leptos source. Dioxus + Leptos + Sycamore are Rust-shaped implementations of the same Solid model; understanding the model upstream is cheaper than reverse-engineering from one implementation.
- **Schedule-alignment is the open question.** Bevy's system scheduler is parallel and ordered by access patterns; Dioxus's signal scheduler is serial and ordered topologically. A Bevy-native signal layer needs to bridge these — the answer is not obvious. See [foundation README § 5 — "Reactivity layer"](../../specs/2026-05-07-buiy-foundation/README.md).

## Sources

- Dioxus 0.5 release notes (signal introduction, 2024-03-21): https://dioxuslabs.com/blog/release-050
- Dioxus 0.7 release notes (Stores): https://dioxuslabs.com/blog/release-070
- `dioxus-signals` crate: https://crates.io/crates/dioxus-signals
- `generational-box` crate: https://crates.io/crates/generational-box
- Leptos `Store` (cross-reference, similar shape): https://github.com/leptos-rs/leptos/discussions/2725
- Ryan Carniato — "A Hands-on Introduction to Fine-Grained Reactivity" (Solid model upstream): https://dev.to/this-is-learning/a-hands-on-introduction-to-fine-grained-reactivity-3ndf
- Buiy foundation non-goals: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
