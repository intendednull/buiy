**Date:** 2026-05-22
**Status:** active
**Subject:** belly — plugin shape, workspace partition, and how `eml!` + `ess` + bindings compose on top of bevy_ui

# Architecture

## Plugin shape

`belly::BellyPlugin` is a single Bevy `Plugin` consumers add to their `App`. Internally it registers:

- The `eml!` macro's runtime support — element nodes, attribute resolution, slot expansion.
- The `.ess` stylesheet asset loader — a custom Bevy `AssetLoader` that parses text files into a stylesheet asset and applies the resulting rules.
- The bindings runtime — `from!` / `to!` connection tables, `connect()` event plumbing, change-propagation systems.
- A widget library — `button`, `slider`, `textinput`, `progressbar`, `label`, `body`, `div`, `span`, `br`, `strong`, `img`, `buttongroup`.
- Default styles + a built-in font.

belly is built **on top of** bevy_ui, not parallel to it. Every belly element ultimately spawns a tree of `NodeBundle` / `TextBundle` / `ImageBundle` entities with bevy_ui's `Style`, `Node`, `BackgroundColor`, etc. The `eml!` macro is a more ergonomic spawner; the `ess` stylesheet is a cascade engine that writes back into bevy_ui's components.

This is the same architectural niche bevy_flair occupies — but bevy_flair stops at stylesheets, while belly bundles authoring + styling + bindings into one plugin.

## Workspace partition

The repo is a Cargo workspace with these member crates (from `Cargo.toml` at v0.5.0):

| Crate | Role |
|---|---|
| `belly` | Umbrella crate. Re-exports the others. The entry point consumers depend on. |
| `belly_core` | Cascade engine + bindings runtime + ECS plumbing. No widgets, no syntax sugar. |
| `belly_macro` | The `eml!`, `from!`, `to!`, `run!` procedural macros. |
| `belly_widgets` | The widget library (button, slider, textinput, progressbar, label, …). Depends on `belly_core`. |
| `bevy_stylebox` | Nine-slice image styling support. Used for `stylebox-*` properties. |
| `tagstr` | Interned string type used throughout for selectors, attribute names, class names. |

This is roughly the same three-crate split bevy_flair uses (parser / cascade / registry) plus belly's two additions: a widget crate and a macro crate. Future-Buiy stylesheet sub-spec consideration: belly's partition validates that "macro + cascade + widgets + interned-strings" is the right boundary set for a one-stop declarative-UI plugin — but only if the team is willing to own all four. Buiy's foundation [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns) already commits to the widget + interned-token equivalents (semantic tokens, decomposed components, theme assets); only the parser+cascade are missing.

## The three composing pieces

**1. `eml!` macro — authoring (markup)**

```rust
commands.add(eml! {
    <body s:padding="50px">
        "Hello, " <strong>"world"</strong> "!"
    </body>
});
```

A procedural macro that lexes HTML-like syntax and emits Bevy spawn code. Attributes prefixed with `s:` are inline styles (cascade equivalent of HTML's `style=""` attribute). Class attributes use a `c:className` syntax. Bindings use `bind:value=` / `on:press=`. See [`eml-macro.md`](eml-macro.md) for the full grammar.

**2. `ess` stylesheet — styling (cascade)**

```css
button { width: 100px; height: 100px; }
button:hover .content { background-color: white; }
.red .content { background-color: lightcoral; }
```

A Bevy asset loaded via `StyleSheet::load("stylesheet.ess")`. The parser is hand-written inside `belly_core` (not Servo `cssparser`, unlike bevy_flair). At runtime a cascade pass matches selectors against the entity tree and writes resolved values back into bevy_ui style components. Hot-reload via the asset system. See [`ess-stylesheets.md`](ess-stylesheets.md) for property coverage.

**3. `from!` / `to!` / `connect` — reactivity (bindings)**

```rust
commands.add(
    from!(counter, Counter:count|fmt.c("Value: {c}")) >> to!(label, Label:value)
);
commands.connect()
    .entity(btn)
    .on(button_pressed)
    .handle(run!(|counter: &mut Counter| { counter.0 += 1; }));
```

A separate runtime — *not* part of the cascade — that wires component fields together (`from!`/`to!`) and routes events to handlers (`connect`/`on`/`handle`). The `run!` macro packages a closure with the system-param signature belly needs. See [`data-binding.md`](data-binding.md).

## What belly extends on bevy_ui

belly does not own a render pipeline; bevy_ui handles all rendering. belly extends bevy_ui in three ways:

1. **Authoring ergonomics.** `eml!` is a faster way to spawn nodes than `commands.spawn((NodeBundle{…}, …))`. No new component model — the macro is sugar over bevy_ui's existing components.
2. **Stylesheet cascade.** A `.ess` file can write to any bevy_ui style field. The cascade is belly-owned; bevy_ui has no knowledge it's happening. belly's `ApplyComputedProperties`-equivalent system runs in `Update` and clobbers programmatic writes for the fields the cascade controls (same precedence pitfall flagged in bevy_flair — see [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md)).
3. **Bindings runtime.** Independent of bevy_ui — works on any Bevy component. The `from!`/`to!` graph lives in `belly_core`'s resources and runs its own change-detection systems.

## Stylebox: belly's one render-pipeline-adjacent piece

`bevy_stylebox` (workspace member, separately authored by jkb0o) implements nine-slice image rendering — a feature bevy_ui does not natively provide. It plugs into bevy_ui's render via a custom `Material2d`. Properties like `stylebox-source` / `stylebox-slice` / `stylebox-region` route through this crate.

This is the only place belly does anything bevy_ui can't. Everything else is authoring/cascade/binding sugar over bevy_ui's existing surfaces.

## Pipeline ordering

belly runs its systems in `Update`. The intra-frame order, from observation of the code:

1. Asset loader processes any reloaded `.ess` files → emits stylesheet asset updates.
2. Bindings runtime applies `from!` → writes resolved values to target components.
3. Cascade pass walks entity tree, matches rules, writes to style components.
4. Event handlers fire (`connect()` / `on()` handlers) in response to button press / hover / etc.
5. bevy_ui's own layout + paint runs after `Update`.

This is significantly less decomposed than bevy_flair's eleven-stage pipeline (cf. [`../bevy-flair/architecture.md`](../bevy-flair/architecture.md)). belly's pipeline has no explicit "MarkEntitiesForRecalculation" / "TickAnimations" / "EmitRedrawEvent" partitioning — the cascade pass is monolithic. This is part of why belly's cascade has no documented optimization story.

## Implications for Buiy

A future Buiy stylesheet layer that wants belly's full ergonomic surface (markup + cascade + bindings) costs three runtime systems, three macros, and at least three crates — closer to a small framework than a thin layer. The Buiy foundation already owns the markup question (BSN), the cascade question (tokens, optionally with a future stylesheet), and the reactivity question (observers + change detection, optionally with future signals). The relevant lesson is in the partitioning: belly's choice to keep authoring + styling + bindings in one crate is a packaging convenience for end users, but it ships them as inseparable. Buiy should keep each axis as a separately graduable sub-spec.

## Sources

- belly v0.5.0 `Cargo.toml` (workspace members) — https://github.com/jkb0o/belly/blob/v0.5.0/Cargo.toml
- belly v0.5.0 README (macros + plugin) — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- belly_core source — https://github.com/jkb0o/belly/tree/v0.5.0/crates/belly_core
- belly_macro source — https://github.com/jkb0o/belly/tree/v0.5.0/crates/belly_macro
- bevy_stylebox source — https://github.com/jkb0o/belly/tree/v0.5.0/crates/bevy_stylebox
- example `style-sheet.rs` — https://github.com/jkb0o/belly/blob/v0.5.0/examples/style-sheet.rs
- example `connections.rs` — https://github.com/jkb0o/belly/blob/v0.5.0/examples/connections.rs
- example `counter-binds.rs` — https://github.com/jkb0o/belly/blob/v0.5.0/examples/counter-binds.rs
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_flair architecture (compare) — [`../bevy-flair/architecture.md`](../bevy-flair/architecture.md)
