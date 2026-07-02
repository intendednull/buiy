**Date:** 2026-06-26
**Status:** active
**Subject:** Fine-grained reactive signals — Solid.js + Leptos (+ reactive_graph), and the ECS / Bevy Changed<T> bridge

# Lessons — the decision file (D6 signal layer vs D7 lean on `Changed<T>`)

Siblings: [README](README.md) · [architecture](architecture.md) ·
[composition-state-events](composition-state-events.md) · [styling-theming](styling-theming.md) · [open-problems](open-problems.md)

**Framing rule (carried through every item):** the value+derive **ergonomics** are
the port target; the **runtime** (serial, synchronous, single-flush graph over
dynamic global dependencies) largely does **not** port onto Bevy's parallel,
slice-partitioned scheduler; and **`Changed<T>` is already a coarse signal**, so
the real question is granularity/derivation, not existence. Fine-grained derive
*inside* a parallel scheduler is an **open problem**, not a solved borrow. Each
item is tagged with the friction it bears (F1..F8) and an ECS+`bsn!`
transferability rating.

## Validates

- **V1 — A value→memo→effect layer is the right *authoring* model for
  dynamic/derived state.** Derive once; propagation is automatic; no hand-written
  "when X changes, walk the tree and patch Y." *(F7, F2, F8)* —
  **Transferability: HIGH for the API shape, LOW for the runtime.** The split is
  clean; `@solidjs/signals` and `reactive_graph` being separable crates prove the
  surface is an independent artifact.
- **V2 — Derived values must be lazy + glitch-free.** Leptos memos don't run until
  read and fire a diamond's downstream effect once, not twice. *(F7)* —
  **Transferability: MED.** The *property* is desirable and `Changed<T>` does
  **not** provide it (a `Changed<A>`+`Changed<B>` consumer can run redundantly);
  achieving glitch-free derive on ECS ticks is itself the open problem.
- **V3 — Automatic dependency tracking is what removes the boilerplate.** Tracking
  by "which signals you read" eliminates manual subscribe/marker code — the direct
  antidote to F7 marker sprawl. *(F7, F8)* — **Transferability: MED.**
  Auto-tracking is the exact thing that *fights* ECS (a system declares its access
  slice up front; a derive that reads "whatever it touched" can't be statically
  scheduled). Ports as a **goal**, not a mechanism.
- **V4 — Buiy already has a coarse signal: `Changed<T>`.** "Did this change since I
  last ran," per component, per system, free, in parallel. *(F7)* —
  **Transferability: HIGH.** It is literally Bevy; the decision is whether
  coarse-frame-granular suffices, not whether it exists.

## Avoid

- **A1 — Do not import the serial/synchronous runtime.** A global dynamic graph
  flushed single-threaded is the precise thing the parallel scheduler cannot see
  (#10212); every Bevy port that took it gave back the parallelism. *(F7)* —
  **Transferability of the warning: HIGH.**
- **A2 — Do not adopt closures-capturing-getters as the propagation mechanism
  (silent-wrong footgun).** Floem: *"reactivity relies on closures … we run the
  closure again whenever the signal updates."* Forget the closure → you bind a
  static snapshot, it silently never updates, **no error**. *(F3, F7)* —
  **Transferability: HIGH.** If Buiy builds a derive layer, make the stale-value
  case a **compile error or panic**, not silence.
- **A3 — Do not reinvent a parallel ownership/disposal arena.** Solid's owner
  graph / Floem's `Scope` duplicate what **entity despawn already is** in ECS; a
  second arena fights Bevy's lifecycle and re-creates the leak surface. *(F7)* —
  **Transferability: MED.** Tie any derive lifetime to entity/component lifetime.
- **A4 — Do not let "everything is a signal" creep.** Solid's own pitfalls docs
  flag *unnecessary reactivity* and referential-equality surprises as the most
  common mistakes; over-signalizing trades F7 boilerplate for F3 surprises. *(F3)*
  — **Transferability: MED.**

## Borrow

- **B1 — The primitive vocabulary: source signal / lazy memo / edge effect.** If
  Buiy builds D6, name and shape these three; keep memos lazy + glitch-free; keep
  effects at the boundary (paint, **AccessKit export**). *(F7, F2)* —
  **Transferability: HIGH (shape) / LOW (engine).**
- **B2 — "Optimize for not over-notifying, not for propagation speed."** Leptos
  appendix (verbatim): *"The measurement of a good reactive system is not how
  quickly it propagates changes, but how quickly it propagates changes without
  over-notifying."* Concrete D7 borrow: gate writes with **`set_if_neq`** so no-op
  mutations don't trip `Changed<T>`. *(F7, F3)* — **Transferability: HIGH;
  actionable on today's Bevy.**
- **B3 — Borrow the design *target*, not the code.** `reactive_graph` is built for
  *"long-lived interactive applications … prioritizing the efficiency of side
  effects over raw update speed"* — a retained UI verbatim. — **Transferability:
  HIGH.**
- **B4 — Use Bevy Observers for the *event* axis; reserve any signal/derive for
  the *value* axis.** Observers (0.14) give typed, immediate push reactions
  (`OnInsert`/`OnRemove`/custom triggers) — chips at **F2** *without* a signal
  runtime: Buiy's untyped `OnPress(Entity)` can become typed observer events now.
  *(F2)* — **Transferability: HIGH (native Bevy).**
- **B5 — Typed controlled `value` + typed `onChange`, per widget.** The Leptos
  two-way idiom (`prop:value` + typed `on:input`) is exactly what `OnPress(Entity)`
  lacks: expose `value: Signal<T>` in / `on_change: impl Fn(T)` out, typed to the
  widget's domain. *(F2)* — **Transferability: HIGH for the API shape;** needs a
  callback/`SystemId` mechanism, no signal runtime required.
- **B6 — Two list primitives because there's no VDOM.** Keyed-by-reference
  (`<For>`) vs keyed-by-position (`<Index>`) is a real semantic choice retained
  mode must expose; bevy_reactor reconciles with an LCS keyed diff. *(F7)* —
  **Transferability: MED.** Concept ports; `bsn!` is static, so dynamic keyed
  lists need a reconciler (spawn/despawn/move children by key) *around* `bsn!` —
  real work, not a macro feature.
- **B7 — Typed, checked design tokens are an OPPORTUNITY, not a borrow.** All three
  signal UIs delegate tokens to CSS (untyped, unchecked, silent-default on typo) —
  that's **F6** in the prior art too. Buiy can beat them: tokens as a typed
  enum / `Resource`, resolved by a system, `Changed<Theme>`-driven. *(F6)* —
  **Transferability: HIGH;** Rust types + ECS resources are strictly better than
  CSS variables; nothing about the web token model is worth copying.

## The runtime non-borrow (the warning, restated)

- **L-RT — The reactive runtime is serial/persistent and does NOT port; the bridge
  is UNSOLVED upstream.** bevy_reactor = one exclusive `run_reactions` system with
  `&mut World`; bevy_lazy_signals = deferred/lossy/frame-delayed and stuck on Bevy
  0.14; jonmo (the only maintained one) lowers each combinator into a **system**,
  trading glitch-freedom for scheduler-parallelism; Floem is synchronous *because*
  it has no ECS; Bevy's own #17917 answers "reactions run serially in index
  order." *(F7)* — **Transferability: LOW (engine) / framing of an OPEN PROBLEM.**
  Do not treat the bridge as solved; `Changed<T>` (D7) is the parallel-native
  coarse signal to build the *ergonomics* on instead.

## Bottom line — D6 vs D7

Frame the signal layer as: **borrow the ergonomics (signal/memo/effect vocabulary,
lazy glitch-free derive, "don't over-notify"), reject the runtime (serial global
dynamic graph), and recognize `Changed<T>` as the coarse signal you already
have.** The cheap, scheduler-native moves available *today* are **`set_if_neq`
value-gating (B2)** and **typed Observers for the event axis (B4/B5)**, plus a thin
typed-token layer (B7). A true fine-grained derive layer (**D6**) on a parallel ECS
is, on current evidence, **unsolved** — every ecosystem attempt is alpha or serial
— so treat it as an **open research bet**, not a borrow to copy. D6/D7 is recorded
as an open decision in the
[UI-DX & composition prior-art report](../../reports/2026-06-25-ui-dx-composition-prior-art.md)
(directions D1–D7 + its open-questions section, where the "spike D6 / ship D7 /
commit to one?" question lives) — append the resolution there, the canonical
ledger, when it is made. The recommended
first step is a thin **`Derived<T>`-over-`Changed<T>`** layer (D7-plus: a normal,
parallel, equality-gated system that recomputes an output component when its
source components are `Changed`) before committing to a borrowed runtime; watch
bevy_reactor's 0.19/`bsn!` branch as the closest living reference. None of this
touches **F1** (a11y-tree-as-state), **F5** (widget spellings), or the F4/F7
**authoring** gap — `bsn!` is Bevy's static macro and a reactive inline-binding
ergonomic must be designed *around* it, not copied from JSX/`view!`.

### Strawman: `Derived<T>`-over-`Changed<T>` (illustrative shape, not a spec)

The recommended first step has no parallel-hostile graph — it is a normal,
schedulable, equality-gated system that writes an output component only when its
sources changed:

```rust
#[derive(Component, PartialEq)] struct Derived<T>(T);   // the derived output cell

// Runs in parallel with everything whose access slice is disjoint; only touches
// rows where A or B changed; `set_if_neq` (B2) keeps it from tripping
// `Changed<Derived<Out>>` on a no-op recompute (the F3 over-notify guard).
fn derive_out(mut q: Query<(&A, &B, &mut Derived<Out>), Or<(Changed<A>, Changed<B>)>>) {
    for (a, b, mut out) in &mut q { out.set_if_neq(Derived(compute(a, b))); }
}
```

It is coarse (component-granular, one-frame latency, no within-frame glitch-free
cascade across `Derived`-of-`Derived`) — the exact D6 gap to measure *before*
reaching for a borrowed runtime.

## Sources

- https://book.leptos.dev/appendix_reactive_graph.html · https://docs.rs/reactive_graph/latest/reactive_graph/
- https://docs.solidjs.com/advanced-concepts/fine-grained-reactivity · https://dev.to/ryansolid/building-a-reactive-library-from-scratch-1i0p
- https://github.com/bevyengine/bevy/discussions/10212 · #10978 · #17917
- https://github.com/bevyengine/bevy/pull/10839 · https://bevy.org/news/bevy-0-14/
- https://bevy-cheatbook.github.io/programming/change-detection.html · https://docs.rs/bevy/latest/bevy/ecs/change_detection/trait.DetectChangesMut.html
- https://github.com/viridia/bevy_reactor · https://github.com/knutsoned/bevy_lazy_signals · https://github.com/databasedav/jonmo · https://github.com/lapce/floem
