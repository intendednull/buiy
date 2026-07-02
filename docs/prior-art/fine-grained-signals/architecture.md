**Date:** 2026-06-26
**Status:** active
**Subject:** Fine-grained reactive signals — Solid.js + Leptos (+ reactive_graph), and the ECS / Bevy Changed<T> bridge

# Architecture — runtime mechanism vs API-surface convention

Siblings: [README](README.md) · [composition-state-events](composition-state-events.md)
· [styling-theming](styling-theming.md) · [open-problems](open-problems.md) · [lessons](lessons.md)

## What it is

A UI state model where app state lives in **signals** (read/write cells), derived
state is expressed as **memos** (pure functions of signals), and side effects are
**effects** that re-run when their tracked inputs change. The defining property is
*fine granularity*: the runtime tracks dependencies **automatically and
dynamically** at the value level, so a write re-runs only the exact
memos/effects/bindings that read it — no component re-render, no VDOM diff. Solid
popularized it for the web; Leptos ported it to Rust/WASM and extracted the engine
into the standalone **`reactive_graph`** crate. Floem is the native-Rust outlier
that took the runtime wholesale by *not* using an ECS.

## The load-bearing split: RUNTIME vs API-SURFACE

For Buiy these are cleanly separable, and only one of them ports.

### Runtime mechanism (does NOT port to a parallel ECS)

All of Solid / Leptos (`reactive_graph`) / Floem share one shape:

- **A persistent reactive graph** lives in a global/thread-local runtime for the
  UI's lifetime. Nodes = signals (sources), memos (cached derived), effects
  (sinks); edges = dependencies.
- **Automatic dynamic dependency tracking.** When a memo/effect runs, the runtime
  installs it as the "current observer"; every `signal.get()` during that run
  *registers an edge*. Dependencies are discovered by **execution**, are
  **dynamic** (re-collected each run), and are **invisible** in source — the
  developer never declares them. `reactive_graph` uses a **push-pull
  ("Reactively"-style)** algorithm: writes push "maybe-dirty" marks down the
  graph (three-state `Clean`/`Check`/`Dirty`); reads pull, recomputing a memo only
  if a dependency truly changed (lazy, glitch-free, at most once per change —
  the diamond problem is solved before any intermediate value is observed).
- **Synchronous, single-threaded propagation** to quiescence on one thread, in
  dependency order, *between* external events. The graph mutates as it runs
  (edges added/removed). Solid's setup fn, Leptos's component body, Floem's view
  fn all **run exactly once**; thereafter only fine-grained nodes re-run.
- **Bespoke ownership/disposal.** Solid's hierarchical owner graph
  (`createRoot`/owner), Floem's `Scope` — a side-channel lifetime model so
  subscriptions don't leak (the observer pattern is "inherently leaky" per
  Carniato).

**Why it does not port:** Bevy schedules systems **in parallel across threads**,
each declaring up front the disjoint World **slices** it touches — that static,
conflict-checked partition is *what buys the parallelism*. Fine-grained
reactivity is the inverse: a single mutable graph whose edges are **dynamic and
cross-cutting**, discovered at run time, propagated **serially** to a fixpoint.
Talin (author of Bevy discussion #10212 *and* of bevy_reactor) names it:
reactivity "cuts across" the ECS boundaries — "the lines of dependency are
invisible and dynamic … but reactive dependencies may reach outside of [the
declared] slice." A signal runtime is structurally **a serial scheduler with a
global mutable dependency graph**; it can only run as an *island* off to the side
of Bevy's schedule, or defer reactions into a later system (reintroducing
multi-frame lag on deep chains).

### API-surface convention (this IS the port target)

Independent of the runtime, the *authoring ergonomics* survive any propagation
mechanism — including coarse, frame-based ECS change detection:

- **A value cell with `.get()`/`.set()` and a derived `move || …`** — the mental
  model "state is a value, derived state is a pure fn of values."
- **Co-location** of dynamic/derived UI at the binding site (`text=move || c.get()`)
  instead of marker components + tree-walking systems that re-read state each
  frame (Buiy's **F7**).
- **No manual subscribe/unsubscribe** — the spreadsheet-formula feel.

These are a *shape of API*. That is the seam Buiy gets to design.

## Floem — the explicit road not taken

(Full Floem dossier — distribution, governance, text stack, AccessKit gap — in the
sibling [floem](../floem/) folder, esp.
[fine-grained-reactivity](../floem/fine-grained-reactivity.md); the claims below are
sourced from it, not independently re-derived.)

Floem is the cleanest proof the runtime is a *choice*: a Lapce-team Rust GUI
"built around reactive primitives inspired by `leptos_reactive`," view tree built
**once**, a **persistent** reactive graph holding state across frames, on its own
single-threaded runtime. Floem chose the signal runtime **instead of an ECS** —
reactive-graph-as-architecture, not retained-mode-over-ECS. Buiy's hard
constraint (stay ECS + parallel + retained) is exactly what Floem *declined* —
which is why Floem ports the runtime wholesale and Buiy cannot.

## The ECS-bridge picture (open, not solved)

- **Bevy has no first-party answer.** #10212 ends "I'm guessing the eventual
  answer is some kind of hybrid. But I don't know what that really looks like
  yet." #10978 and #17917 continue without resolution. What *shipped* is
  **Observers + lifecycle hooks (0.14)** — *push/event* reactivity
  (`OnAdd`/`OnInsert`/`OnRemove`), **not** value-derive memos.
- **Every ECS signal bridge is an island or alpha/stale.** bevy_reactor runs the
  whole graph as one exclusive `run_reactions` system (unpublished, **unlicensed**
  → not safely vendorable); bevy_lazy_signals is 0.5.2-alpha, deferred/lossy,
  pinned to Bevy 0.14, ~2yr stale; haalka/jonmo (the only *maintained* one)
  lowers each combinator into a **Bevy system** so propagation becomes
  **frame-granular and scheduler-mediated** (losing within-frame glitch-freedom,
  gaining coexistence). **bevy_mod_reaction** (matthunz — author of the "bottom up"
  blog in Sources) is the one experiment that *attempts* the parallel-native shape:
  a `Reaction` wraps a `ReactiveSystem` driven by Bevy change detection, reactions
  are claimed to **run in parallel**, with `ReactiveQuery` for targeted per-entity
  tracking — but it is **0.2.0-alpha.1** and unproven, i.e. exactly the
  "glitch-free *and* parallel" point the rest of this survey calls unshipped. See
  [composition-state-events](composition-state-events.md) §4.
- **`Changed<T>` is already a coarse signal** — see [styling-theming](styling-theming.md)
  §5.4 and [open-problems](open-problems.md). The D6/D7 decision is whether
  coarse-but-parallel `Changed<T>` (+ Observers for push events) suffices, or
  whether Buiy needs a fine-grained derive layer on top — and whether that layer
  can stay parallel (nobody has shipped that).

## Distribution / versioning / who ships it

- **Solid.js** — npm `solid-js` (MIT), semver, `solidjs` org, Open Collective
  governance. 2.0 re-packages the core as the framework-agnostic
  **`@solidjs/signals`** — itself evidence the runtime is a separable unit.
- **Leptos** — crates.io `leptos` (MIT), `leptos-rs`, no foundation; minor bumps
  are semver-breaking by policy. Engine ships separately as **`reactive_graph`**
  (MIT, ~1.6M dl, first published 2024-04-28 with the 0.7 reactive rewrite that
  retired `leptos_reactive`) — renderer-agnostic, the realistic crate to *study*
  if Buiy ever wants a real engine (but it would run as an island).
- **ECS bridges** — community, none Bevy-official, none a stable dependency Buiy
  could lean on.
- **Floem** — Lapce team; ships the persistent-graph runtime as the toolkit
  foundation (not separable, by design).

**Unverified:** exact MSRV/`rust-version` for `leptos`/`reactive_graph` (inherits
from workspace); Solid 1.9.13's exact publish date (1.9.13 confirmed as npm's
current `solid-js` latest, but the timestamp is ~May/June 2026 per npm tooling,
not pinned). *(Floem's release number is now verified: crates.io max `0.2.0`,
published 2024-11-15.)*

## Sources

- https://docs.rs/reactive_graph/latest/reactive_graph/ · https://book.leptos.dev/appendix_reactive_graph.html
- https://docs.solidjs.com/advanced-concepts/fine-grained-reactivity
- https://github.com/lapce/floem
- https://github.com/bevyengine/bevy/discussions/10212 · https://github.com/bevyengine/bevy/discussions/10978 · https://github.com/bevyengine/bevy/discussions/17917
- https://github.com/bevyengine/bevy/pull/10839 (Observers, 0.14)
- https://crates.io/crates/leptos · https://crates.io/crates/reactive_graph · https://www.npmjs.com/package/solid-js
- https://github.com/viridia/bevy_reactor · https://github.com/knutsoned/bevy_lazy_signals · https://github.com/databasedav/jonmo · https://github.com/matthunz/bevy_mod_reaction
- https://machinewords.hashnode.dev/reactivity-in-bevy-from-the-bottom-up-part-1
