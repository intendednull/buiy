**Date:** 2026-05-22
**Status:** active
**Subject:** belly — system-specific term glossary

# Glossary

belly invents enough vocabulary that a reader new to the corpus benefits from a single-page reference.

| Term | Meaning |
|---|---|
| **belly** | The Bevy UI plugin crate (umbrella). Also the workspace name. https://github.com/jkb0o/belly |
| **`BellyPlugin`** | The Bevy `Plugin` consumers add to `App`. Registers asset loaders, the bindings runtime, the cascade systems, and the widget library. |
| **`eml!`** | Procedural macro from `belly_macro` that lexes HTML-like syntax and emits Bevy spawn code. See [`eml-macro.md`](eml-macro.md). |
| **`.eml`** | File extension for asset-loaded `eml!`-grammar markup. Equivalent to inline `eml!` but loadable as a Bevy asset (with hot-reload). |
| **`ess`** | "Element Style Sheet." The CSS-like file format belly uses. Parsed by `belly_core`. See [`ess-stylesheets.md`](ess-stylesheets.md). |
| **`.ess`** | File extension for `ess`-format stylesheet assets. |
| **`StyleSheet`** | The Bevy asset type representing a parsed `.ess` file. Constructed via `StyleSheet::load("path.ess")`. |
| **`s:` prefix** | In `eml!` markup, attributes prefixed with `s:` are inline styles. E.g. `s:padding="50px"`. Cascade equivalent of HTML's `style=""`. |
| **`c:` prefix** | In `eml!` markup, attributes prefixed with `c:` add classes. E.g. `<button c:red c:large>`. Macro-hygiene-friendly alternative to HTML's `class="red large"`. |
| **`bind:` prefix** | In `eml!` markup, attributes prefixed with `bind:` create value bindings. E.g. `bind:value=from!(…)`. |
| **`on:` prefix** | In `eml!` markup, attributes prefixed with `on:` register event handlers. E.g. `on:press=run!(…)`. |
| **`from!`** | Procedural macro that constructs a binding *source* descriptor: which component / field, optionally with a transformer. |
| **`to!`** | Procedural macro that constructs a binding *sink* descriptor: which component / field receives the bound value. |
| **`>>` (binding operator)** | Connects a `from!` source to a `to!` sink: `from!(…) >> to!(…)`. One-directional. |
| **`connect()`** | Method on `Commands` (and similar contexts) that opens an event-connection builder. Used as `commands.connect().entity(e).on(signal).handle(callback)`. |
| **`.on(signal)`** | Builder step that specifies which signal kind triggers the connection. Examples: `button_pressed`, `hover_enter`, `text_changed`. |
| **`.handle(callback)`** | Builder step that registers the callback closure. Typically wrapped in `run!`. |
| **`run!`** | Procedural macro that converts an inline closure into a Bevy `System` by deriving the `SystemParam` signature from the closure's argument types. |
| **Transformer** | A function inserted in a value binding to convert from source type to sink type. The most common is `fmt.c("…{c}…")` for format-string conversion. |
| **`fmt.c(…)`** | A built-in transformer for formatting source values as strings via a template string. The `c` placeholder represents the source value. |
| **Signal** | belly's term for an event a widget emits. Internally backed by Bevy events. Examples: `button_pressed`, `slider_changed`, `text_input_changed`. |
| **Selector** | A pattern in `.ess` that matches entities to apply rules to. belly supports tag, class (`.name`), ID (`#name`), descendant (whitespace combinator), and a limited pseudo-class set (`:hover`, `:active`, `:focus`). |
| **`belly_core`** | The workspace crate containing the cascade engine, bindings runtime, ECS plumbing. No widgets, no syntax sugar. |
| **`belly_macro`** | The workspace crate containing all procedural macros (`eml!`, `from!`, `to!`, `run!`). |
| **`belly_widgets`** | The workspace crate containing the widget library (`button`, `slider`, `textinput`, `progressbar`, `label`, `img`, `body`, `div`, `span`, `br`, `strong`, `buttongroup`). |
| **`bevy_stylebox`** | Workspace member crate implementing nine-slice image rendering (belly's only render-pipeline-adjacent code). Wires up the `stylebox-*` properties. |
| **`tagstr`** | Interned-string crate used throughout belly for selectors, class names, attribute names. Avoids `String` allocation in hot paths. |
| **`<for>` element** | Special `eml!` element that expands at spawn time to repeat its body for each item in a Rust iterator. Not a runtime reactive list — one-shot. |
| **Slot** | A `<slot/>` placeholder inside a widget definition that consumers fill at spawn time. Static composition. |
| **`StyleBox` / nine-slice** | A nine-slice image asset used as a scalable rectangular background (e.g. for buttons, panels). Source image divided into 9 regions; corners stay fixed, edges and center scale. |
| **`Counter`** (example component) | The recurring `struct Counter { count: i32 }` component used across `counter-binds.rs`, `counter-signals.rs`, `connections.rs`. Not part of belly's public API — it's example fixture data. |

## Sources

- belly v0.5.0 README — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- belly v0.5.0 examples — https://github.com/jkb0o/belly/tree/v0.5.0/examples
- belly_macro source — https://github.com/jkb0o/belly/tree/v0.5.0/crates/belly_macro
- belly_widgets source — https://github.com/jkb0o/belly/tree/v0.5.0/crates/belly_widgets
- bevy_stylebox source — https://github.com/jkb0o/belly/tree/v0.5.0/crates/bevy_stylebox
