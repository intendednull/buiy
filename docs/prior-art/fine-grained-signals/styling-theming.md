**Date:** 2026-06-26
**Status:** active
**Subject:** Fine-grained reactive signals — Solid.js + Leptos (+ reactive_graph), and the ECS / Bevy Changed<T> bridge

# Styling · theming · tokens

Siblings: [README](README.md) · [architecture](architecture.md) ·
[composition-state-events](composition-state-events.md) · [open-problems](open-problems.md) · [lessons](lessons.md)

This facet tests Buiy's **F6** (stringly-typed unchecked theme tokens) against the
actual prior art, and shows that reactive theming is *the same machinery* as
reactive derived state — so you can't get one without taking a position on
D6/D7 (**F7**).

## 1. The substrate insight: dynamic styling IS signal reactivity

In Solid and Leptos there is **no separate "style state" system**. A dynamic
class/style is just a signal read inside a tracking scope; the framework wires the
dependency and re-runs the *one* DOM mutation. Styling reactivity and app-state
reactivity are **the same graph**.

```tsx
const [theme, setTheme] = createSignal("light");
<div class={theme() === "light" ? "light-theme" : "dark-theme"}>…</div>;
```

The `theme()` *read* subscribes the class expression; `setTheme(...)` re-runs only
that binding. **Crux for Buiy:** Solid-style "theme change → exactly the affected
paint updates" is the *same* machinery as "any derived UI value updates" — you
don't get reactive theming for free; it is the D6/D7 decision wearing a styling
hat.

## 2. How styling attaches

### Solid — `class` / `style` / `classList`, all signal-bindable

- inline `style={{ color: "red" }}` (object keys dash-case, values strings);
- classes from imported CSS files (inserted into `<head>`);
- **`classList`** — `{ class → bool }`, "often more efficient than `class`"
  because it "selectively toggles only the classes that require alteration, while
  `class` will be re-evaluated each time."
- **Footgun (verbatim):** mixing reactive `class` and `classList` — "when the
  `class` value changes, Solid will set the entire `class` attribute. This will
  remove any classes set by `classList`." A web-CSS analogue of Buiy's **F3**
  silent-wrong footguns.

### Leptos — `class:` / `style:` / `class=(…)` tuple in `view!`

`class:active=move || is_active()`, `style:color=move || …`, the
`class=("name", move || expr)` tuple (so scoped-CSS libs can inject a generated
class), and a trailing `class = …` arg applying one class to *every* element.
Leptos "has no opinions about CSS" — scoping is **out-of-framework**: **Stylers**
(inline CSS in Rust, compile-time scoped), **Stylance** (sibling `.css`,
compile-time scoped, hot-reload-friendly), **Styled** (runtime-injected).

### What NEITHER gives you

**No built-in design-token type, no completeness check, no typed cascade.**
"Theming" = swap a CSS class driven by a signal, or flip CSS custom properties
(`--fg`, `--space-2`). CSS variables are **stringly-typed and unchecked** — a typo
`var(--colr-fg)` fails silently to the cascade default. This is exactly Buiy's
**F6**, and the prior art does **not** solve it.

## 3. The token / theming model — answered honestly

(The Floem column draws on the sibling [floem](../floem/) folder —
[layout-and-styling](../floem/layout-and-styling.md) and
[fine-grained-reactivity](../floem/fine-grained-reactivity.md) — rather than
re-deriving Floem's `Style`/`StyleClass` model here.)

| Question | Solid | Leptos | Floem |
|---|---|---|---|
| Typed tokens? | No (CSS strings/vars) | No (CSS strings/vars) | Partial — Rust `Style` builder + `StyleClass`, but values are plain Rust types, not a token *vocabulary* |
| Checked / complete? | No | No | No (no "all tokens defined" guarantee) |
| Cascade? | CSS cascade | CSS cascade | own class/inheritance cascade (Taffy-backed `.style()`) |
| Reactive? | yes (signals) | yes (signals) | yes (signals) |

**Buiy takeaway (F6):** typed, compile-checked theme tokens are an *opportunity*,
not a borrow. Web signal frameworks deliberately delegate the token model to CSS,
inheriting CSS's untyped/unchecked nature. Floem is closest to "tokens in the host
language" (Rust `.style()` builder + style classes) but still has no completeness
guarantee. Buiy, being Rust + ECS, can do strictly better: tokens as a typed
enum / `Resource`, resolved by a system, with `Changed<Theme>` driving
re-resolution — *which is itself the D6/D7 question.*

## 4. The signal runtime, briefly (full treatment in [architecture](architecture.md))

Solid: `const [count, setCount] = createSignal(0)` — reads inside a tracking scope
(`createEffect`/`createMemo`) subscribe; propagation synchronous, single-threaded.
Leptos `reactive_graph`: signals are roots, memos interior nodes, effects leaves;
**push** marks `Dirty`/descendants `Check`, **pull** recursively checks whether
inputs truly changed before recomputing (memos equality-check) so a converging
node "only runs once" — synchronous, single-threaded, arena-allocated in a
thread-local runtime. Those two properties — synchronous immediate propagation +
global single-threaded ownership — are precisely what collide with Bevy.

## 5. CRUCIAL — signals vs Bevy's parallel scheduler (styling lens)

### 5.1 The tension, stated by the person who tried hardest

Bevy #10212 (viridia/Talin): *"ECS is based around the idea that the game state is
partitioned into isolatable units … The problem, though, is that reactivity cuts
across those boundaries … reactive dependencies may reach outside of that slice."*
He rejects both escape hatches — deferring ("you start to introduce lag, state
changes have to wait multiple frames") and recompute-everything-every-frame
("scaling issues because every interactive game object has formula") — and
concludes: *"I'm guessing the eventual answer is going to be some kind of hybrid.
But I don't know what that really looks like yet."* As of 0.19 there is still **no
first-party Bevy reactive layer**.

### 5.2 The attempts (honest status)

- **bevy_reactor** (viridia): fine-grained, "inspired by Carniato's
  Reactive-from-Scratch"; **unpublished**, tracks `bevy 0.19.0-dev` + a `formulae`
  VM whose TODO admits the hard part (field references / lifetimes) is unsolved.
- **bevy_lazy_signals**: **0.5.2-alpha, ~2yr stale**, self-described "ad hoc,
  informally-specified, bug-ridden … 1/3 of MIT-Scheme."
- **Floem**: signals work *beautifully* **when you abandon the parallel
  scheduler** — persistent graph, view tree built once, single-threaded.
- **haalka + jonmo** (the live, maintained Bevy answer): signals are *"output
  handles to nodes of a Bevy system dependency graph"*; *"every frame, the outputs
  of systems are forwarded to their dependants, recursively."* Each combinator
  compiles to a **special Bevy system**.

### 5.3 The porting boundary

jonmo shows the *only* demonstrated way to run signal ergonomics on a parallel ECS
scheduler: **lower every combinator into a system**. Consequence: propagation
becomes **frame-granular and scheduler-mediated**, not synchronous-immediate; you
**lose** Solid/Leptos's within-frame **glitch-free** guarantee; you **gain**
coexistence with the parallel scheduler (signal nodes = systems with declared
access, parallelized for free). So **value+derive ergonomics port; the runtime
does not.**

### 5.4 `Changed<T>` is already a coarse signal (the D7 baseline)

Bevy change detection: `Changed<T>` is true when a component was added or
**mutably dereferenced** since the reader last ran (tick compare). haalka
literally bridges it: `signal::from_component_changed::<Counter>(entity)` lifts
ECS change ticks into the FRP graph — existence proof that D7 *is* a signal
source, just a blunt one. Limits vs fine-grained: per-component-per-entity
**granularity**; **no equality gate** (fires on `DerefMut` even on a no-op write —
**F3**); **no dependency tracking** (no derived chains, no diamond dedup);
**frame-latency** (derived-from-derived settles over frames unless hand-ordered).
For "re-resolve theme / re-layout / re-paint when X changed," `Changed<T>` is
sufficient and idiomatic. For "B=f(A); C=g(B); update exactly C's consumers, once,
glitch-free," it is not — that is the gap D6 would fill, at the runtime-mismatch
cost above.

**Net (styling lens only):** for reactive theming/layout/paint, **D7
(`Changed<T>`) + a thin typed-token layer** is the cheap, native, parallel-safe
baseline and covers *this* facet today. Whether Buiy *also* needs fine-grained
derived chains with glitch-free within-frame settling — and what jonmo's
lower-signals-into-systems precedent implies for building it — is the global
D6-vs-D7 decision, argued (not re-derived) in [lessons](lessons.md).

## Sources

- https://github.com/bevyengine/bevy/discussions/10212 · #10978 · #17917
- https://github.com/viridia/bevy_reactor · https://github.com/knutsoned/bevy_lazy_signals · https://crates.io/crates/bevy_lazy_signals
- https://github.com/databasedav/haalka · https://github.com/databasedav/jonmo
- https://github.com/lapce/floem · https://crates.io/crates/floem
- https://docs.solidjs.com/concepts/signals · solidjs/solid-docs class-style + styling guide
- https://book.leptos.dev/interlude_styling.html · https://book.leptos.dev/view/02_dynamic_attributes.html · https://book.leptos.dev/appendix_reactive_graph.html
- https://github.com/abishekatp/stylers · https://deepwiki.com/leptos-rs/book/9.1-styling-approaches
- https://docs.rs/bevy_ecs/ (change-detection) · https://bevy-cheatbook.github.io/programming/change-detection.html
