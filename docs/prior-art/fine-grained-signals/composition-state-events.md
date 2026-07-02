**Date:** 2026-06-26
**Status:** active
**Subject:** Fine-grained reactive signals — Solid.js + Leptos (+ reactive_graph), and the ECS / Bevy Changed<T> bridge

# Composition · state · events — the core DX

Siblings: [README](README.md) · [architecture](architecture.md) ·
[styling-theming](styling-theming.md) · [open-problems](open-problems.md) · [lessons](lessons.md)

This is the file that matters for Buiy's D6-vs-D7 decision (friction **F7**): how
the signal lineage composes UI, models state, propagates change — then the
load-bearing question, *can a fine-grained signal runtime live inside a parallel
ECS scheduler at all?* (Answer: only as an ergonomic surface; not as the runtime.)

## 1. Composition — the component body runs ONCE

The single most transferable idea to a retained ECS. A Solid component is a plain
function returning JSX, but: *"a Solid component is only run once, when it is
first rendered … After that, the component is not re-run, even if the
application's state changes."* Updates flow through the graph the body wired up.
This maps exactly onto retained ECS: **the component body is a one-time
spawn/wiring step, not a per-frame render** — kill the tree-walks (**F7**).

Props are read-only and one-way, and **eager reads destroy reactivity**:

```tsx
function MyComponent(props) {
  const { name } = props;        // ❌ breaks reactivity (destructure = read)
  const name = props.name;       // ❌ breaks reactivity
  const name = () => props.name; // ✓ a getter stays live
}
```

Helpers exist *because* props are live: `mergeProps` (reactive defaults),
`splitProps` (reactive destructure), `children()` (memoize child resolution =
the slot mechanism).

### Dynamic lists — two primitives, no VDOM to paper over the choice

```tsx
// <For>: keyed by REFERENCE; index is a SIGNAL (rows move, don't re-render)
<For each={data()}>{(item, index) => <li>{item.name} {index()}</li>}</For>
// <Index>: keyed by POSITION; item is a SIGNAL (content at a slot changes)
<Index each={data()}>{(item, index) => <li>{item().name}</li>}</Index>
```

Leptos's keyed `<For each key let:item>` has the same trap: a value changing
without the key changing yields no update — forcing a deliberate state-granularity
choice (coarser key → re-render row; or wrap fields in `RwSignal`; or Stores).

## 2. State model

### Local

```rust
let (count, set_count) = signal(0);   // Leptos: (ReadSignal, WriteSignal)
```
```tsx
const [count, setCount] = createSignal(0); // Solid: [getter, setter]
```

The getter/setter split is a **capability** split: the getter is a read-cap, the
setter a write-cap — both are values you can pass around (basis of the event
story below).

### Nested — Stores (fine-grained over a tree)

Per-signal state doesn't scale to records; both ecosystems add a path-targeted
store with **leaf-granular** updates ("updating only the properties that change"):

```tsx
const [store, setStore] = createStore({ users: [/* records */] });
setStore("users", 0, "username", "felix");                 // path write
setStore(u => u.location === "Canada", "loggedIn", false); // predicate write
```

ECS already *is* a store of components; sub-component change granularity is the
gap (half of why D6 tempts at all — see [open-problems](open-problems.md)).

### The controlled `value` / typed-`onChange` convention (Buiy F2)

The canonical two-way idiom — and the direct contrast to Buiy's single untyped
`OnPress(Entity)`:

```rust
let (name, set_name) = signal("Controlled".to_string());
view! {
  <input type="text"
    on:input:target=move |ev| set_name.set(ev.target().value())  // typed onChange
    prop:value=name />                                            // controlled value
}
```

Leptos states the subtlety: the `value` **attribute** sets only the *initial*
value; the `value` **property** keeps updating. **Lesson for Buiy:** a controlled
widget is `value: Signal<T>` **in** + `on_change: impl Fn(T)` **out**, both
*typed to the widget's domain* (`String`/`bool`/`f32`). That is per-widget typed
change — the thing `OnPress(Entity)` lacks (**F2**).

## 3. Events / change propagation — a menu of typed-change shapes

Leptos's *Parent-Child Communication* enumerates the design space (read it as
"what a typed change story looks like"):

```rust
// (1) pass a WriteSignal — child mutates parent state directly
// (2) pass a Callback — mutation logic stays in the parent
// (3) on:click listener — when it maps 1:1 to an event, no prop at all
// (4) provide_context / use_context — escapes prop-drilling, LOSES type-safety
```

Stated tradeoffs (verbatim-ish): passing a `WriteSignal` "can make it hard to
reason about … not at all clear when or how it will change"; the callback "keeps
local state local, preventing spaghetti mutation" but forces logic up to the
parent; context "eliminates prop drilling" but "you don't have type-safety
anymore." Under the hood it is all the graph: a write notifies subscribers,
effects are the leaves that touch the world, and a `Memo` is equality-gated —
*"only affects the effects and computations that depend on its value, without
requiring any diffing,"* notifying dependents **only if the result actually
changes**. Dependency edges are tracked **at runtime** (dynamic) — exactly what
an ECS cannot do statically.

## 4. CRUCIAL — fine-grained signals on a parallel ECS scheduler

### 4.1 The structural conflict (#10212, viridia, 2023-10-21)

*"With an ECS system, you have to declare up front which slices of the world you
want to access — but reactive dependencies may reach outside of that slice."* The
parallelism is *bought* by static declared access; the fine-grainedness is
*earned* by dynamic runtime tracking. Consequences raised: **cascades cost
frames** (marker-component propagation, one schedule-step per layer → depth-N
chain takes N frames vs Solid's synchronous within-tick settle); reactions are
ad-hoc closures that "break ECS's homogeneous execution model." Counter-positions:
`Changed<T>`/`Added<T>` filters as coarse reactivity (nicopap); first-class memos
e.g. `GlobalTransform`-as-a-memo (ewmb7701); a dedicated **"Flow" schedule** run
multiple times/frame to drain cascades (jkb0o).

### 4.2 What the working prototypes actually do — they go SERIAL

**bevy_reactor** (viridia): the entire graph is **one exclusive system**.

```rust
app.add_systems(Update, run_reactions);   // the whole graph = ONE system
// Reaction::react(&mut self, owner, world: &mut World, tracking)  ← EXCLUSIVE &mut World
#[derive(Component)] pub struct ReactionCell(pub Arc<Mutex<dyn Reaction + ...>>);
```

Reactions are *components* carrying `Arc<Mutex<dyn Reaction>>`; each runs
**synchronously** with exclusive `&mut World`; deps tracked in a per-entity
`TrackingScope` keyed off `world.change_tick()`. The graph is a serial
sub-runtime nested in a single ECS node — parallelizes with nothing. Its
`Signal<T>` enum (`Mutable | Derived | Constant`), setup-runs-once, LCS keyed
diff for `foreach`, and despawn/respawn conditionals are deliberately
Solid-shaped, now wired into Bevy 0.19's `bsn!`/`Scene`/`template` — the same
authoring layer Buiy uses (the closest living reference).

**bevy_lazy_signals** (knutsoned): computeds recompute in `PreUpdate`,
propagation **deferred** (visible next eval cycle = frame-delayed) and **lossy**
(multiple sends/frame collapse to the final value). Open questions in its own
README: infinite-loop detection; whether change detection can replace marker
components. **Pinned to Bevy 0.14, 0.5.2-alpha, ~2yr stale** — design reference,
not a dependency.

**Floem** is the counter-example that proves the cost: a Solid-grade synchronous
graph *because it is not on an ECS* — persistent (long-lived `Scope`s, not
rebuilt per frame), single-threaded runtime. The ergonomics come bundled with a
**serial, persistent, single-owner** graph, every time.

### 4.3 Where Bevy itself is heading (#17917, Cart, 2025-02-17)

"In-place Reaction systems": reactions are components holding reactive systems
that **poll `System::is_changed()` each frame** (built on change detection),
order via an atomic `ReactionIndex`, and **run serially in index order** —
"prioritizing predictable ordering and change detection accuracy over
parallelism." Even Bevy's own plan trades the parallel scheduler away for the
reactive subset.

### 4.4 Conclusion for the composition/state/events layer

Scoped to *this* file (the full Validates/Avoid/Borrow + D6-vs-D7 verdict is in
[lessons](lessons.md), not re-derived here):

- **`Changed<T>` is already a coarse signal at the change-propagation layer** —
  per-component, per-frame, equality-free dirty bit, filterable in parallel. It
  gives "react to what changed" free + parallel, at **component granularity /
  one-frame latency**. It does *not* give sub-field granularity, equality-gated
  propagation (it fires on any `DerefMut`, a silent-overwork footgun, **F3**),
  automatic *derived* values, or synchronous multi-level cascades.
- **The authoring shapes are what port** — controlled `value`/typed-`onChange`
  (**F2**) and keyed lists without a VDOM (**F7**) — deliverable *on top of*
  `Changed<T>` (a `Derived<T>` recomputed in a normal system when its sources are
  `Changed`, writing an output component), no parallel-hostile graph imported. The
  global decision (prototype thin `Derived<T>`-over-`Changed<T>` before borrowing a
  runtime; record D6-vs-D7 as **open**) is argued in [lessons](lessons.md).

## Sources

- https://docs.solidjs.com/concepts/components/basics · https://docs.solidjs.com/concepts/components/props · https://docs.solidjs.com/concepts/control-flow/list-rendering · https://docs.solidjs.com/concepts/stores · https://docs.solidjs.com/concepts/signals
- https://book.leptos.dev/view/05_forms.html · https://book.leptos.dev/view/08_parent_child.html · https://book.leptos.dev/view/04b_iteration.html · https://book.leptos.dev/appendix_reactive_graph.html
- https://github.com/bevyengine/bevy/discussions/10212 · https://github.com/bevyengine/bevy/discussions/17917 · https://github.com/bevyengine/bevy/discussions/10978
- https://github.com/viridia/bevy_reactor (src/lib.rs, src/reaction.rs, src/signal.rs, Cargo.toml)
- https://github.com/knutsoned/bevy_lazy_signals · https://crates.io/crates/bevy_lazy_signals
- https://docs.rs/floem_reactive/latest/floem_reactive/ · https://github.com/lapce/floem
- https://docs.rs/leptos/latest/leptos/ · https://docs.rs/reactive_graph/latest/reactive_graph/
