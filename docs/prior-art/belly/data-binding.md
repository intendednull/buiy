**Date:** 2026-05-22
**Status:** active
**Subject:** belly's data-binding system — `from!`, `to!`, `connect()` / `on()` / `handle()`, the `run!` macro, transformers

# Data binding

The bindings runtime is belly's reactivity layer. It is **independent of the cascade engine** — they share no state — and lives entirely inside `belly_core` plus the `belly_macro` procedural macros that produce binding values.

## The two primitives

belly distinguishes two flavors of binding:

1. **Value bindings** — `from!` / `to!`. Connect a source field on a component (or resource) to a target field. When the source changes, the target updates.
2. **Event connections** — `connect()` / `on()` / `handle()`. Route an event source (button press, hover, custom event) to a handler closure.

The first is reactive (continuous, declarative). The second is imperative (single-shot per event, callback-based). belly uses both heavily — the `counter-binds.rs` example threads them together.

## Value bindings

The minimal example from `counter-binds.rs`:

```rust
commands.add(
    from!(counter, Counter:count|fmt.c("Value: {c}")) >> to!(label, Label:value)
);
```

What this binds:

- **Source:** the `count` field of the `Counter` component on the entity bound to the local name `counter`.
- **Transformer:** `fmt.c("Value: {c}")` — a formatting transformer that produces a string. `{c}` is the placeholder for the source value.
- **Sink:** the `value` field of the `Label` component on the entity `label`.
- **Direction:** `>>` (one-way, from → to). The binding has no inverse declared.

`from!` and `to!` are procedural macros that emit code building a binding descriptor. The runtime walks descriptors each frame, checks change detection on the source component, and writes through the transformer to the sink. The pattern is comparable to MVVM `OneWay` bindings in XAML, or to Vue's `v-bind`.

**Two-way bindings** are expressed with two single-direction declarations, not a single `<->` operator. There is no built-in inverse-transformer inference — the developer writes the inverse explicitly.

### Transformers

A transformer is an in-place computation between source and sink. The most common are:

- `fmt.c("…{c}…")` — formatting (source → string, with `{c}` slot for source value).
- Identity (no transformer) — for direct field-to-field copy with matching types.

The transformer set is extensible — `belly_core` exposes a trait — but the documentation marks "bind transformers" as "Work in progress" at v0.5.0. The list of useful transformers ships small.

### Bindings in `eml!`

The `bind:` attribute embeds a value binding directly in markup:

```rust
eml! {
    <label bind:value=from!(counter, Counter:count|fmt.c("Value: {c}"))/>
}
```

This is equivalent to the separate `commands.add(from! … >> to! …)` form but ergonomically inline. The macro expansion produces the same binding descriptor.

## Event connections

The minimal pattern from `connections.rs`:

```rust
commands.connect()
    .entity(btn)
    .on(button_pressed)
    .handle(run!(|counter: &mut Counter| {
        counter.0 += 1;
    }));
```

Components:

- **`connect()`** — entry on `Commands` (and `&mut World` and observer contexts) that opens a builder.
- **`.entity(btn)`** — source entity. (Alternative: `.event(event_kind)` for world-scoped events not bound to an entity.)
- **`.on(signal)`** — the signal kind. `button_pressed`, `hover_enter`, `text_changed`, etc. are exposed as constants/functions.
- **`.handle(callback)`** — the handler. Wrapped in `run!` (see below) so the macro can wire system params.

A second form in the same example file:

```rust
ctx.connect()
    .entity(btn)
    .on(button_pressed)
    .handle(run!(|ctx, e: Entity| {
        ctx.commands().entity(*e).despawn_recursive();
    }));
```

Here the handler receives the connection context (`ctx`) and the firing entity. The `run!` macro is doing the work of generating a Bevy system from the closure — packaging system params, change-detection filters, and the closure body into a runnable callback.

### `run!` macro

`run!` is the bridge between an inline closure and a Bevy system. It rewrites the closure into a function with the right system-param signature so Bevy's scheduler can dispatch it. Without `run!`, the developer would need to spell out the full `SystemParam` types explicitly.

Common closure signatures observed in examples:

- `run!(|c: &mut Counter| c.0 += 1)` — exclusive mutable access to one component on the firing entity.
- `run!(for counter |c: &mut Counter| c.count += 1)` — exclusive mutable access to a captured entity (`counter`).
- `run!(|ctx, e: Entity| { … })` — connection context + firing entity for full-world manipulation.

### Events in `eml!`

The `on:` attribute embeds an event connection inline:

```rust
eml! {
    <button on:press=run!(for counter |c: &mut Counter| c.count += 1)>
        "Increment"
    </button>
}
```

This is the form used most often in the example apps.

## Implementation shape

The bindings runtime lives in `belly_core`. From observation:

- **Connection tables** — Bevy resources hold the active `from!` / `to!` connections and event subscriptions.
- **Change-detection systems** — each frame, systems walk the connection table, check source change-flags, run transformers, write sinks. The same system pattern applies to event handlers, dispatched on signal emission.
- **Signal emission** — widgets emit signals (`button_pressed`, etc.) via Bevy events the handler systems subscribe to.
- **System ordering** — the bindings runtime runs in `Update`, ahead of cascade application, so binding-written values participate in the cascade. (Cf. [`architecture.md`](architecture.md).)

No reactive graph optimization — every binding is checked every frame. At small scale (the example apps) this is fine; at 1000+ bindings the cost would dominate. No published benchmark.

## What belly's bindings are not

- **Not signals/computed/effects.** The reactivity model is dataflow-via-change-detection, not the SolidJS / Vue / Leptos signal graph model. There is no automatic dependency tracking — sources are declared at binding-creation time.
- **Not observable collections.** A `for!` loop in markup is one-shot at spawn; there is no observable `Vec<T>` whose changes re-render a list.
- **Not async/promise-aware.** Handlers are synchronous Bevy systems.

## Implications for Buiy

The Buiy foundation excluded a signal-style reactivity layer from v1 ([README.md § 1 non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals)) — observers + change detection are the v1 primitive. belly's experience supports this:

1. **Change-detection-as-reactivity works for the small-scale case.** belly's bindings runtime is a thin layer over Bevy's change detection; it's enough for the demo apps. Buiy can ship without a signal layer and still be authorable.

2. **Reactive-graph optimization matters at scale.** belly has no per-binding optimization beyond change-flag short-circuiting. If Buiy ever adds reactive collections / computed values / fine-grained signals (the deferred follow-up sub-spec), the dependency-graph design space is the place to invest — not in inventing a binding *macro*.

3. **Inline `on:` / `bind:` in markup is ergonomic, but coupled.** belly's `bind:` and `on:` attributes are coupled to `eml!` — they are macro-syntax forms, not a runtime API. The runtime *also* exposes `connect()` / `from!` / `to!` standalone, which is the more important API. Buiy should keep observer/event-binding APIs accessible from ECS spawn code first, BSN authoring second; inline-in-markup is a downstream ergonomic layer, not the primitive.

4. **`run!` is a microcosm of the system-param ergonomics problem.** belly invented `run!` because writing out the full `SystemParam` tuple for an inline closure is verbose. Bevy itself has improved on this since 0.13 with `IntoSystem` and observer ergonomics. If Buiy ships a markup macro at any layer, the inline-handler ergonomics should ride on current Bevy primitives, not duplicate belly's macro.

## Sources

- example `counter-binds.rs` — https://github.com/jkb0o/belly/blob/v0.5.0/examples/counter-binds.rs
- example `connections.rs` — https://github.com/jkb0o/belly/blob/v0.5.0/examples/connections.rs
- example `counter-signals.rs` — https://github.com/jkb0o/belly/blob/v0.5.0/examples/counter-signals.rs
- belly v0.5.0 README (binding + signal sections) — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- belly_core source — https://github.com/jkb0o/belly/tree/v0.5.0/crates/belly_core
- Buiy foundation non-goals (signal layer deferred) — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1 non-goals
