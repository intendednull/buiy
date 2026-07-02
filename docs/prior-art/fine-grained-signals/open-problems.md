**Date:** 2026-06-26
**Status:** active
**Subject:** Fine-grained reactive signals — Solid.js + Leptos (+ reactive_graph), and the ECS / Bevy Changed<T> bridge

# Open problems — what the model structurally does NOT solve

Siblings: [README](README.md) · [architecture](architecture.md) ·
[composition-state-events](composition-state-events.md) · [styling-theming](styling-theming.md) · [lessons](lessons.md)

What follows is what fine-grained signals (Solid / Leptos / `reactive_graph` /
Floem) **cannot** do for Buiy — the honest boundary of the borrow. None of these
are bugs; they are structural limits of the model or of its fit to Buiy's
constraints.

## 1. Parallelism — the runtime is serial by design

The single biggest non-solve. Solid/Leptos/Floem run a **single-threaded,
synchronous, glitch-free flush** over a **global, dynamic** dependency graph.
Bevy's value proposition is a **parallel** scheduler fed by **static, declared**
data-access slices. A global dynamic graph that "reaches outside the slice" is the
precise thing the parallel scheduler cannot see (#10212). Every working Bevy port
proves it: bevy_reactor runs the whole graph as one exclusive `&mut World` system;
bevy_lazy_signals threads a serial pass and eats frame-delayed/lossy propagation;
jonmo lowers each combinator into a system and gives up within-frame
glitch-freedom; Floem only achieves Solid-grade behavior *because it is not an
ECS*. **No one has shipped fine-grained value-derive that is both glitch-free and
parallel-scheduler-native.** Treat it as unsolved (**F7**). Bevy's own #17917
answer is "reactions run serially in index order."

## 2. The a11y / semantic tree (F1) — orthogonal, no help

Signals propagate **values**; they say nothing about exporting an AccessKit tree.
A signal layer neither helps nor hurts Buiy's "app state IS the a11y tree"
question. The fine-grained lineage has no concept analogous to AccessKit-first
output — accessibility in Solid/Leptos is whatever the DOM yields. Buiy's F1
decision (should app STATE be separate from the a11y tree) gets **zero** guidance
here; the borrow is purely about value/derive ergonomics, not about what the tree
*is for*.

## 3. Typed change events at the widget boundary (F2) — only partial

Signals give *derive*; they do not by themselves give a typed "this slider's value
changed" *contract*. The controlled `value`/typed-`onChange` convention
([composition-state-events](composition-state-events.md) §2) is a good **API
shape** to copy, but it is a convention layered on callbacks, not something the
reactive runtime provides. Bevy **Observers** (typed `OnInsert`/`OnRemove`/custom
triggers, shipped 0.14) cover the *event* axis better than signals do. Buiy still
has to **design** the typed widget-change API either way — signals are not a
shortcut to it.

## 4. `bsn!` authoring of reactive bindings (F7-authoring) — half-closed

Signals are a **runtime** concern; `bsn!` authors **static** scenes. Adding
signals at runtime lets you mutate state dynamically, but `bsn!` itself still
cannot express a reactive binding *inline* — the "no dynamic content in scenes"
gap (**F7**) is only half-closed. (This is the **F7** authoring axis, distinct
from **F4**, which is the narrower static gap that the fluent `Style` builder is a
`Bundle` so `bsn!` can't author it at all.) Solid/Leptos solve authoring with JSX/`view!`
macros that **embed signal reads in the template** (`text=move || …`); `bsn!` is
**Bevy's macro and not Buiy's to extend** with read-tracking syntax (HARD
CONSTRAINT). Any Buiy reactive-binding ergonomics must be a construct *around*
`bsn!` (a signal-valued component a system drives, a closure field re-applied by a
system), not a macro feature — feasible but **unbuilt**, and this is where D6/D7
actually bites the authoring layer.

## 5. Sub-component change granularity — unsolved on ECS

Solid/Leptos added **Stores** for path-targeted, leaf-granular updates over a
nested record. Bevy components are coarser than store leaves: `Changed<T>` fires
at component granularity, not per-field. Sub-component change-granularity is
**unsolved** (and is half of why D6 tempts at all). The web answer (wrap each
field in a signal, or use a proxy store) does not have a parallel-ECS equivalent
that stays cheap.

## 6. Async correctness — stale closures, even in Solid

Glitch-free holds only *within one synchronous flush*. Across
`setTimeout`/async boundaries Solid itself exhibits **stale-closure** bugs: an
effect's captured `count()` is "captured at the time the callback is created, not
when it executes," and inside an async callback "the global scope no longer has a
registered subscriber," so tracking silently breaks (**F3** in the wild). A signal
layer does **not** make async-derived state safe — Buiy would inherit this footgun
class if it copies the closures-capturing-getters mechanism.

## 7. Leak machinery / disposal — bespoke, and redundant on ECS

The observer pattern is "inherently leaky" (Carniato): "Signals don't need cleanup
but any subscription does." Solid avoids leaks with a hierarchical **owner graph**
(`createRoot`/owner); Floem with `Scope`. This machinery is **non-trivial and
bespoke**. In ECS, **entity despawn already is the ownership/disposal event** — a
second arena keyed by reactive scope would fight Bevy's lifecycle and re-create the
leak surface it was meant to fix. So the model's own solution to its own problem
is a *liability* to port, not an asset.

## 8. Out of scope entirely

The reactive model says nothing about Buiy's **F5** (one widget, 4 spellings),
**F6** beyond reactivity (typed token *vocabulary* and completeness checking — the
prior art delegates to untyped CSS, see [styling-theming](styling-theming.md)),
or **F8** verbosity beyond what derive removes. These are not failures of the
model; they are simply outside what a value/derive layer addresses.

## Bottom line

The fine-grained-signal model solves **value propagation ergonomics** and nothing
else. It does **not** solve parallelism, accessibility output, typed widget
contracts, inline scene authoring, sub-component granularity, async safety, or
disposal-without-bespoke-machinery. For Buiy, the borrow is narrow and the
runtime is a trap; see [lessons](lessons.md) for the Validates/Avoid/Borrow
decision.

## Sources

- https://github.com/bevyengine/bevy/discussions/10212 · #10978 · #17917
- https://github.com/bevyengine/bevy/pull/10839 (Observers, 0.14) · https://bevy.org/examples/ecs-entity-component-system/observers/
- https://bevy-cheatbook.github.io/programming/change-detection.html · https://docs.rs/bevy/latest/bevy/ecs/change_detection/trait.DetectChangesMut.html
- https://dev.to/ryansolid/building-a-reactive-library-from-scratch-1i0p
- https://app.studyraid.com/en/read/8387/231143/avoiding-common-reactivity-pitfalls · https://vladislav-lipatov.medium.com/solidjs-pain-points-and-pitfalls-a693f62fcb4c
- https://github.com/lapce/floem · https://docs.floem.dev/llms-full.txt
- https://github.com/databasedav/jonmo · https://github.com/knutsoned/bevy_lazy_signals
