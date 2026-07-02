**Date:** 2026-06-26
**Status:** active
**Subject:** Fine-grained reactive signals — Solid.js + Leptos (+ reactive_graph), and the ECS / Bevy Changed<T> bridge

# Fine-grained reactive signals — Solid.js · Leptos / `reactive_graph` · the ECS `Changed<T>` bridge

Prior-art folder for Buiy. The fine-grained-signal lineage (Solid.js → Leptos →
the extracted `reactive_graph` crate, plus the native-Rust outlier Floem) is the
clearest articulation of the model where **state is a value, derived state is a
pure function of values, and the framework keeps them consistent** — surgically,
without component re-render or virtual-DOM diff. Every "reactive Bevy" experiment
(`bevy_reactor`, `bevy_lazy_signals`, `bevy_cobweb`, `haalka`/`jonmo`) is a
descendant. This folder exists to inform **Buiy's hardest open decision**: for
dynamic/derived UI state (friction **F7**), do we **build a signal/derive layer
(D6)** or **lean on Bevy's `Changed<T>` (D7)**?

The thesis carried through every file: the **value+derive ergonomics are the
port target**; the **runtime is not** (a serial, synchronous, single-flush graph
over dynamic global dependencies fights Bevy's parallel, slice-partitioned
scheduler); and **`Changed<T>` is already a coarse signal**, so the real question
is granularity and derivation, not existence. The bridge to fine-grained derive
*inside* a parallel scheduler is, on current evidence, an **open problem** — not
a pattern Buiy can copy.

## Key facts (verified 2026-06-26)

| System | Version / status | License | Steward | Runtime shape |
|---|---|---|---|---|
| **Solid.js** (`solid-js`) | 1.9.13 stable; 2.0 beta (`@solidjs/signals`) | MIT | Ryan Carniato / Open Collective | persistent serial graph, synchronous, single-thread |
| **Leptos** (`leptos`) | 0.8.20 stable; 0.9.0-alpha | MIT | Greg Johnston + Ben Wishovich (`leptos-rs`) | same; engine extracted to `reactive_graph` |
| **`reactive_graph`** | 0.2.14 stable; 0.3.0-alpha (~1.6M dl) | MIT | `leptos-rs` | push-pull ("Reactively"-style), lazy, glitch-free |
| **Floem** (`floem_reactive`) | 0.2.0 | MIT | Lapce team | persistent graph, **not** ECS; view tree built once |
| **bevy_reactor** | unpublished R&D; Bevy 0.19-dev + `bsn!` | none declared | viridia (Talin) | one exclusive `run_reactions` system, serial |
| **bevy_lazy_signals** | 0.5.2-alpha; pinned Bevy 0.14; ~2yr stale | MIT/Apache-2.0 | knutsoned | deferred, lossy, frame-delayed |
| **haalka / jonmo** | 0.7.1 / 0.7.0 (Bevy 0.18) | MIT/Apache-2.0 | databasedav | signals lowered into Bevy systems (frame-granular) |
| **Bevy `Changed<T>`** | shipped (0.19) | — | Bevy | tick dirty-bit; per-component; fires on any `DerefMut` |

## Contents

- [architecture.md](architecture.md) — what it is; the runtime-mechanism vs
  API-surface-convention split; distribution/versioning; Floem as the road not
  taken; the structural ECS conflict.
- [composition-state-events.md](composition-state-events.md) — the core DX:
  components-run-once, props/slots, signals/stores, the controlled
  `value`/typed-`onChange` convention, and what the working Bevy ports actually
  do (go serial).
- [styling-theming.md](styling-theming.md) — dynamic styling *is* signal
  reactivity; `class:`/`style:`/`classList` attachment; the honest token verdict
  (CSS strings, untyped, unchecked — **F6** unsolved in the prior art too).
- [open-problems.md](open-problems.md) — what the model structurally does **not**
  solve (parallelism, the a11y tree, typed widget-change contracts, `bsn!`
  inline bindings, async correctness, leak machinery).
- [lessons.md](lessons.md) — **the decision file**: Validates / Avoid / Borrow,
  each tagged F1..F8 + ECS+`bsn!` transferability, ending with a D6-vs-D7 bottom
  line.

## How to use

These docs are written from Buiy's stance: **ECS-native (Bevy 0.19),
retained-mode, AccessKit-first, authored via Bevy's `bsn!` macro and bundle
constructors.** The "for Buiy" notes, the transferability ratings, and the
runtime-vs-convention framing all reflect that bias **by design** — this is not a
neutral survey of reactive frameworks, it is a targeted read of one lineage
against Buiy's constraints (stay ECS + parallel + retained; AccessKit is a
non-negotiable output; `bsn!` is Bevy's macro, build around it, do not fork it).
Where the prior art simply does not address a Buiy friction (F1, F5, F6), the
docs say so plainly rather than inventing a borrow. Read `lessons.md` for the
decision; read the others for the evidence behind it.

## Glossary (stub)

- **Signal** — a read/write value cell (`createSignal` / `signal()` /
  `RwSignal`); reading inside a tracking scope subscribes the reader.
- **Memo / computed** — a cached derived value; recomputes lazily, notifies
  dependents only if the result actually changed (equality-gated).
- **Effect** — a leaf side-effect that re-runs when its tracked inputs change
  (paint, DOM mutation, AccessKit export in a Buiy analogue).
- **Fine-grained** — dependency tracking at the level of individual values; only
  the exact readers of a changed value re-run (no component re-render).
- **Glitch-free** — within one flush, a node reachable by two paths (diamond)
  fires its downstream effect once, after all inputs settle.
- **`Changed<T>`** — Bevy's coarse change-detection filter; an entity matches if
  its `T` was added or mutably dereferenced since the reader last ran.

**Where the F-codes and D-codes are defined.** The eight frictions **F1–F8** are
enumerated in the
[UI-DX & composition prior-art research](../../reports/2026-06-25-ui-dx-composition-prior-art.md)
(intro), drawn from the
[developer-experience audit](../../reports/2026-06-25-developer-experience-audit.md)
(§3 friction inventory); the seven candidate design directions **D1–D7** are in
that same prior-art report. The codes this folder leans on:

- **F1** — the state model *is* the AccessKit tree (no domain layer).
- **F2** — one untyped `OnPress(Entity)` sink across ~5 widget kinds (no typed
  per-widget change contract).
- **F3** — silent-wrong failure modes (`#[require]`-suppression, typo'd token →
  magenta, a no-op `DerefMut` tripping `Changed<T>`).
- **F4** — the fluent `Style` builder is a `Bundle`, so `bsn!` cannot author it.
- **F5** — one widget has up to four spellings.
- **F6** — stringly-typed, half-wired theme tokens.
- **F7** — retained-mode boilerplate / no dynamic content in scenes (this
  folder's primary target).
- **F8** — verbosity (no `text("..")`, manual child wiring, value towers).
- **D6 / D7** — Buiy's open decision: build a signal/derive layer (**D6**) vs lean
  on `Changed<T>` (**D7**) for dynamic/derived state.

## Sources

- Defining reports (F1–F8, D1–D7): [developer-experience audit](../../reports/2026-06-25-developer-experience-audit.md) · [UI-DX & composition prior-art research](../../reports/2026-06-25-ui-dx-composition-prior-art.md)
- https://www.npmjs.com/package/solid-js
- https://crates.io/crates/leptos · https://crates.io/crates/reactive_graph
- https://github.com/lapce/floem
- https://github.com/viridia/bevy_reactor · https://github.com/knutsoned/bevy_lazy_signals
- https://github.com/databasedav/haalka · https://github.com/databasedav/jonmo
- https://github.com/bevyengine/bevy/discussions/10212
- https://bevy-cheatbook.github.io/programming/change-detection.html
