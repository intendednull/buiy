**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — the `rsx!` proc-macro: JSX-like authoring DSL for Rust

# The `rsx!` macro

`rsx!` is Dioxus's authoring DSL — a `proc_macro` that consumes a JSX-like syntax inside a Rust expression and emits VNode-construction code. It is the visible interface most Dioxus code touches; it is also the most-iterated component of the framework, having been re-parsed for partial-parse autocomplete in 0.6 and extended with hot-patchable token-formatting in 0.7.

## Surface syntax

```rust
rsx! {
    div {
        class: "container {variant}",
        h1 { "Hello, {name}!" }
        button {
            onclick: move |_| counter += 1,
            disabled: counter > 10,
            "Click me — count is {counter}"
        }
        ul {
            for item in items.iter() {
                li { key: "{item.id}", "{item.label}" }
            }
        }
        if show_footer {
            footer { "© 2026" }
        }
    }
}
```

Salient features:

- **Element keyword followed by braces** — `div { ... }`, `button { ... }`. Elements are *not* `<div></div>` and *not* `<Div>` — they are plain identifiers, matched against `dioxus_html::elements::*` or against user-defined `fn Component(props: P) -> Element`.
- **Attributes as `key: value`** inside the braces, before children. Comma-separated.
- **Children inline after attributes.** Text literals are `"..."`. Interpolation uses Rust's format-string syntax `{name}` — implemented by the macro at parse time, so `name` is captured as a closure-style identifier reference.
- **Control flow** — `if`, `for`, `match` work *inside* `rsx!` and produce nested VNodes. No `{condition && jsx}` ternary tricks needed.
- **Components** distinguished by capitalisation (PascalCase). `Counter { initial: 5 }` calls the user's `fn Counter(props: CounterProps) -> Element` with derived props.
- **Event handlers** — `onclick: move |ev| { ... }` — closure values typed as `EventHandler<MouseEvent>`.
- **Keys** for list reconciliation — `key: "{id}"`.
- **Spread / fragments** — `{...iter}` spread, `Fragment {}` (or just multiple top-level children inside `rsx! { ... }`).
- **Conditional class composition** is plain Rust: `class: format!("base {}", if active { "active" } else { "" })` or via the `class:` shorthand with formatting.

## Comparison: rsx! vs JSX vs HTML vs BSN

| Aspect | JSX (React) | HTML | `rsx!` (Dioxus) | BSN (Bevy, draft PR #20158) |
|---|---|---|---|---|
| Element syntax | `<div>...</div>` | `<div>...</div>` | `div { ... }` | `Node { Style { ... }, Children( ... ) }` |
| Attribute syntax | `key={value}` (JS expr) | `key="value"` | `key: value` | `Component { field: value }` |
| Conditionals | `{cond && jsx}` | template lang | `if cond { ... }` (statement) | `if`-pattern (draft) |
| Loops | `{arr.map(x => jsx)}` | template lang | `for x in arr { ... }` (statement) | `for`-pattern (draft) |
| Components | `<Foo prop={value}/>` | n/a | `Foo { prop: value }` | `Foo { prop: value }` (component-marker entity) |
| Type system | TypeScript (optional) | none | **Full Rust types** | **Full Rust types** |
| Compiles to | `React.createElement()` calls | parsed by browser | `VNode` constructors | ECS spawn commands |
| Hot-reload | per-bundler tooling | edit + refresh | Subsecond binary-patching | hot-reload story TBD |

The line items that differ in `rsx!` from JSX are deliberate Rust-shaped choices: braces-not-angle-brackets makes the macro a proper Rust expression (matched braces nest cleanly with the rest of the language); `if`/`for` as statements rather than expression-only matches Rust syntax; and types are checked by `rustc` against the component's `Props` struct, not at runtime.

The line items that resemble BSN are not coincidental. BSN (the Bevy draft scene format) and `rsx!` both want a typed, Rust-shaped, IDE-friendly authoring DSL where components/structs are the unit of composition. **Buiy's authoring story is BSN-by-construction** ([foundation architecture § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)); `rsx!` is the closest existing exemplar of a Rust UI DSL that has shipped at scale, and BSN's syntactic decisions visibly track ergonomic lessons from frameworks like Dioxus. See [`lessons.md`](lessons.md) § "Borrow."

## What `rsx!` can express

- Static element trees with attributes + text.
- Dynamic text via `{ident}` interpolation.
- Conditional subtrees (`if`/`match`).
- Loops (`for`).
- Component invocation with typed props.
- Event handlers (closures).
- Keys for reconciliation.
- Attribute booleans, numbers, strings, expressions.
- Children spread.
- Fragment grouping.

## What `rsx!` cannot express directly

- Side-effects outside attributes/children (must use `use_effect` outside the macro).
- Untyped attributes — every attribute is resolved to a typed slot on either the HTML element schema or the component's `Props`. Custom HTML-like attributes need a registered element.
- Imperative DOM manipulation — no `ref` callback that lets you grab a raw DOM node; the `Element` opaque handle exists but is intentionally narrow.

## Tooling integration

- **rust-analyzer:** partial-parse support added in 0.6, completion + hover work inside the macro for elements, attributes, component props.
- **Hot-reload:** Subsecond (0.7) hot-patches `rsx!` content; format-string changes, attribute changes, component reorderings, nested rsx blocks all reload without a full rebuild.
- **Formatting:** `dx fmt` (the Dioxus CLI command) formats the macro body; `rustfmt` does not understand the DSL.
- **Lints:** custom `clippy`-style lints don't exist; type-checking happens through ordinary Rust resolution.

## Implications for Buiy

- **Authoring-DSL ergonomics matter at scale.** Dioxus's six-year iteration on `rsx!` (partial-parse, hot-reload, format-string interpolation, conditional/loop statements) is the longest-running Rust UI authoring DSL effort. The fact that 0.6 (Dec 2024) prioritized rust-analyzer partial-parse, and 0.7 (Oct 2025) prioritized hot-patching the macro body, signals what users complain about. BSN should pay attention to both: rust-analyzer integration and hot-reload-of-the-DSL are not "version 2" features, they are the day-one quality-of-life that determines adoption.
- **Statement-shape control flow (`if`/`for`) beats expression-shape (`{cond && jsx}`).** Dioxus chose statements inside `rsx!`; BSN's draft tracks the same shape. Validates BSN's direction. See [foundation architecture § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md).
- **Format-string interpolation `"{name}"` is the single highest-leverage feature.** It compresses the typical "string concat with bindings" pattern to a single line and is hot-reloadable. BSN should support it.
- **Don't borrow the JSX pseudo-HTML element naming.** Dioxus uses lowercase `div`/`button`/`span` because it ships an HTML element schema; Buiy's authoring vocabulary is **Bevy component names** (`Node`, `Text`, `Button`, etc.), and capitalisation matches Rust idiom. Resist the temptation to make BSN look like HTML; it isn't HTML, and pretending it is would confuse the ECS-shape underneath.

## Sources

- `dioxus_core::Element` + `rsx!` docs: https://docs.rs/dioxus-core/
- Dioxus 0.6 release notes (autocomplete improvements): https://dioxuslabs.com/blog/release-060
- Dioxus 0.7 release notes (Subsecond hot-patching of rsx): https://dioxuslabs.com/blog/release-070
- Bevy BSN tracking PR #20158 (cross-reference, draft): https://github.com/bevyengine/bevy/pull/20158
- Buiy foundation architecture: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
