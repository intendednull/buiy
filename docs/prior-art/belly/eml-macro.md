**Date:** 2026-05-22
**Status:** active
**Subject:** belly's `eml!` macro — HTML-like authoring syntax for spawning Bevy UI entities

# The `eml!` macro

`eml!` (Element Markup Language) is a procedural macro inside `belly_macro`. It accepts HTML-like syntax and expands to Bevy spawn code. It is belly's authoring surface — the alternative to `commands.spawn((NodeBundle { … }, …))` chains.

## Canonical example

```rust
use belly::prelude::*;

fn setup(mut commands: Commands) {
    commands.add(StyleSheet::load("stylesheet.ess"));
    commands.add(eml! {
        <body s:padding="50px">
            "Hello, " <strong>"world"</strong> "!"
        </body>
    });
}
```

What this generates: an entity for `body` (a bevy_ui `NodeBundle` with belly's `Body` marker), a child entity for `strong` (similar bundle with `Strong` marker), and text-node children for the three string literals. The `s:padding="50px"` attribute applies an inline style. The stylesheet asset loaded a line earlier provides additional cascade rules.

## Syntax elements

### Elements

Tag names map to widget components. The built-in set:

- Layout: `<body>`, `<div>`, `<span>`, `<br>`, `<strong>`
- Media: `<img>`
- Inputs: `<button>`, `<textinput>`, `<slider>`, `<buttongroup>`
- Display: `<label>`, `<progressbar>`

Each tag is resolved to a Rust function call that spawns the corresponding entity bundle. User-defined widgets register via the `#[widget]` attribute macro on a function or struct.

### Children

Children appear between open and close tags. Three forms:

1. **Nested elements** — `<body><div/></body>`
2. **String literals** — `<span>"hello"</span>` (spawns a text node)
3. **Rust expressions** in braces — `<label>{value}</label>` (interpolates a runtime value)

Self-closing form `<br/>` works for void elements.

### Attributes

| Form | Meaning | Example |
|---|---|---|
| `name="value"` | Plain attribute, forwarded to the widget's spawn function | `<img src="logo.png"/>` |
| `s:property="value"` | Inline style. Equivalent to one rule in the cascade for this entity only. | `s:padding="50px"` |
| `c:class-name` | Adds a class. Multiple `c:` attributes can stack. No value — presence-only. | `<button c:red c:large>` |
| `bind:field=from!(…)` | Reactive binding source/target. See [`data-binding.md`](data-binding.md). | `bind:value=from!(counter, Counter:count)` |
| `on:event=run!(…)` | Event handler. See [`data-binding.md`](data-binding.md). | `on:press=run!(\|c: &mut Counter\| c.0 += 1)` |
| `with=expr` | Adds an additional component to the spawned entity | `<button with=MyMarker>` |
| `entity=expr` | Use a pre-existing entity instead of spawning a new one | `<div entity=existing>` |

The `s:`-prefix is belly's choice to namespace style attributes; HTML uses one undifferentiated `style="…"` attribute. belly's split is arguably cleaner — each property is a separate attribute key.

The `c:`-prefix-for-classes is belly's most surprising choice. HTML uses `class="red large"`; belly chose `<button c:red c:large>`. Two consequences:

1. Macro hygiene becomes easier — class names are identifiers in the macro grammar, not strings inside a string.
2. Class lists are not data-driven without expression escapes — `c:{some_var}` is the workaround. HTML `class="…"` is trivially data-driven.

### Slots and `for` loops

belly supports template repetition:

```rust
eml! {
    <body>
        <for item in items>
            <div>{item}</div>
        </for>
    </body>
}
```

The `<for>` element is not a widget — it's a macro-level construct expanded at spawn time. (It is **not** a reactive list — the loop runs once when `eml!` executes.)

Slots, when present, are a static composition mechanism: a widget definition declares `<slot/>` placeholders, and consumers fill them at spawn. See examples `for-loop.rs` and the `tabview.rs` example for canonical usage.

### Scripting

**There is no scripting layer.** The `eml!` body is Rust, not a sandboxed expression language. Inline Rust expressions appear in `{ … }` braces and execute at spawn time only. There is no equivalent of HTML's `<script>` tag, no JS-like reactivity inside the markup itself, no template-string evaluation post-spawn.

This is intentional and correct for a Bevy plugin — the runtime is Bevy ECS, not a VM. But it means `eml!` is a *spawn-time* DSL, not a *runtime templating language*. Re-rendering on data change goes through the bindings runtime, not through re-running the macro.

## Asset form

In addition to the inline macro, belly supports `.eml` files loaded as assets. The grammar is the same; the file is parsed and produces a function that, when invoked, spawns the tree. Hot-reload works via Bevy's asset system. This is the closest belly comes to BSN — but the format is HTML-like text, not Rust-reflection-driven scene data, so it cannot round-trip with the BSN ecosystem.

## What `eml!` is not

- **Not a virtual DOM.** No diffing, no reconciliation. The macro spawns entities once.
- **Not BSN-compatible.** belly's `.eml` files are a separate format and don't interoperate with `.bsn` scene assets.
- **Not reflection-driven.** `eml!` resolves widget names at macro-expansion time by symbol lookup, not by runtime reflection over the `TypeRegistry`. Adding a new widget requires a macro-visible function/struct, not just a `register_type` call.
- **Not a styling primitive.** The macro produces entities with styles; the cascade engine (`.ess`) writes to those styles. They're separable concerns.

## Implications for Buiy

The `eml!` macro proves three things:

1. **HTML-like syntax is implementable in Rust.** procedural macros can comfortably absorb the `<tag attr="value">children</tag>` shape; the lexer + parser fit inside one crate. If Buiy ever ships a stylesheet sub-spec, an `eml!`-equivalent is feasible — but the BSN ecosystem already covers the *spawn-tree* niche, so an HTML-flavored markup adds little.

2. **The HTML-as-DSL choice is opinionated.** belly took it; BSN (PR #20158) didn't. The Bevy community discussions ([#1522](https://github.com/bevyengine/bevy/discussions/1522), [#9652](https://github.com/bevyengine/bevy/discussions/9652)) consistently prefer ECS-native authoring. Buiy should not ship an HTML-shaped macro alongside BSN — it would fragment the authoring story.

3. **Attribute prefixing (`s:`, `c:`, `bind:`, `on:`) is the macro-hygiene-friendly path** to namespacing the four orthogonal concerns of declarative markup (styles, classes, bindings, events). Even if Buiy never ships HTML markup, the prefix-namespacing pattern is portable to any extensible attribute system — including BSN's metadata fields.

## Sources

- belly v0.5.0 README (macro overview + examples) — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- example `selectors.rs` (`c:` class usage) — https://github.com/jkb0o/belly/blob/v0.5.0/examples/selectors.rs
- example `for-loop.rs` (`<for>` element) — https://github.com/jkb0o/belly/blob/v0.5.0/examples/for-loop.rs
- example `style-sheet.rs` (`s:` inline style usage) — https://github.com/jkb0o/belly/blob/v0.5.0/examples/style-sheet.rs
- example `counter-binds.rs` (`bind:` / `on:` attributes) — https://github.com/jkb0o/belly/blob/v0.5.0/examples/counter-binds.rs
- belly_macro crate — https://github.com/jkb0o/belly/tree/v0.5.0/crates/belly_macro
- Bevy CSS-skepticism discussion #1522 — https://github.com/bevyengine/bevy/discussions/1522
- Bevy authoring discussion #9652 — https://github.com/bevyengine/bevy/discussions/9652
- Bevy BSN draft PR #20158 — https://github.com/bevyengine/bevy/pull/20158
