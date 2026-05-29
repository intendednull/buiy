**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — user-facing API surface: `Styled` component, `StyleSheet` asset, hot-reload, inline styles, programmatic overrides

# API

bevy_flair's API has stayed remarkably small across six minor releases. The full user-facing surface is one component, one asset type, one plugin, and a handful of optional knobs. This file documents what the user actually writes.

## 1. Attaching a stylesheet to an entity tree

The single load-bearing component is `Styled` (renamed from `NodeStyleSheet` for 0.8; both names refer to the same idea — `NodeStyleSheet` is the 0.7-and-prior name). One field: a `Handle<StyleSheet>`. Spawn it on the root entity of the subtree you want styled:

```rust
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn((
        Node::default(),
        Styled::new(asset_server.load("game_menu.css")),
        children![
            (Button, children![Text::new("Play")]),
            (Button, children![Text::new("Settings")]),
            (Button, children![Text::new("Quit")]),
        ],
    ));
}
```

(Verified verbatim against the example structure in `examples/game_menu.rs`.)

Styling propagates through `UiChildren` and `UiRootNodes` (0.6 change — previously used Bevy's standard `Children`). 0.6 also added `GhostNode` support, so a `GhostNode` between a `Styled` ancestor and a stylable descendant doesn't break propagation. The 0.8 `Styled` rename is justified by extending styling to non-`Node` entities (Text-based hierarchies in particular — see 0.8 CHANGELOG "Resolved support for non-UI entities in style hierarchy").

**Constraint:** one `Styled` per entity-tree-root. There is no global stylesheet, and stacking two `Styled` ancestors does not cascade — the closest ancestor wins. Multiple stylesheets per tree are composed via `@import` inside a single `.css` file.

## 2. Loading a stylesheet

Stylesheets are Bevy `Asset<StyleSheet>` values. Load them via the standard `AssetServer`:

```rust
let handle: Handle<StyleSheet> = asset_server.load("ui/menu.css");
```

The asset loader is `FlairCssParserPlugin`'s addition; it parses the `.css` text via `cssparser` + `selectors` and produces a `StyleSheet` asset. Errors (parse errors, missing fonts, unsupported properties) are surfaced as Bevy log warnings; the asset still loads, just with the broken rules elided.

`@import url("base.css")` inside the loaded `.css` file resolves relative to the file's path on disk. Imported stylesheets become recursive `Handle<StyleSheet>` dependencies and hot-reload propagates through them (0.6 fixed font-face registration during imports).

Inline stylesheets — strings rather than files — work via `InlineCssStyleSheetParser` (0.5+). Useful for embedding small style snippets without a separate asset file.

## 3. Inline styles on individual entities

bevy_flair supports `style="..."`-equivalent inline declarations as of 0.4. The component is `RawInlineStyle` (immutable per 0.7, built via `Ruleset`). Use case: per-entity overrides that don't deserve a class.

```rust
commands.spawn((
    Button,
    RawInlineStyle::parse("background-color: red; padding: 8px;").unwrap(),
));
```

Specificity rules: inline styles beat any selector-matched rule, regardless of selector specificity (matching CSS spec).

## 4. Pseudo-state inputs

Pseudo-state (the `:hover`, `:active`, `:focus`, `:disabled`, `:checked` set) is sourced from `NodePseudoState` — a component bevy_flair attaches to interactive entities and that **bevy_ui's own input systems update**. bevy_flair does not implement hover-tracking itself; it relies on `bevy_ui::Interaction` and `bevy_input_focus`. This means:

- bevy_flair pseudo-state behavior is exactly as accurate as bevy_ui's interaction tracking — `:hover` follows `Interaction::Hovered`, `:active` follows `Interaction::Pressed`, `:focus` follows `bevy_input_focus`'s focus model.
- bevy_flair does **not** ship a `:focus-visible` distinction (keyboard-vs-pointer-driven focus) because bevy_ui doesn't separate those sources cleanly yet.

## 5. Custom property registration

Apps that want to drive their own component fields from CSS register them at startup:

```rust
app.register_component_properties::<MyComponent>(); // RegisterComponentPropertiesExt
```

This pulls `MyComponent`'s reflect-registered fields into the `PropertyRegistry` so that `.my-class { my-component-field: 12px; }` becomes a write to `MyComponent.my_component_field`. The 0.6 release restructured `ComponentProperty` to make this ergonomic for custom components beyond bevy_ui.

`CssPropertyRegistry` exposes the inverse direction: given a CSS property name and value text, resolve to a `PropertyValue` ready to write.

## 6. Programmatic overrides

Conflict semantics between bevy_flair and direct component writes:

- Each frame, the `ApplyComputedProperties` stage **overwrites** the component fields bevy_flair manages for entities matching at least one rule.
- Components bevy_flair manages for an entity (because at least one rule writes to them) are owned by bevy_flair for that frame. A user-side `world.entity_mut(e).insert(BackgroundColor(red))` will be clobbered on the next `PostUpdate` if a rule still matches.
- Components bevy_flair does **not** manage (because no rule mentions them) are left untouched.
- Auto-removal (0.6 behavior): components whose properties go fully unset (e.g. all rules removed) are removed from the entity, returning the entity to its pre-style state.

The pattern is: programmatic styling and bevy_flair styling **do not cleanly coexist for the same component on the same entity**. Pick one mode per component. The README does not document this; it's inferred from the pipeline. See [`integration.md`](integration.md) for the practical implications.

## 7. Hot-reload

Edit the `.css` file → save → next frame the cascade reruns and the UI redraws. Mechanics (from [`architecture.md`](architecture.md) § 7):

- Bevy's `AssetServer` emits `AssetEvent::<StyleSheet>::Modified`.
- `FlairStylePlugin`'s system observes the event in the `Prepare` stage.
- All entities downstream of any `Styled` referencing the modified asset get their `StyleMarkers` recalculation bit set.
- `CalculateStyles` reruns the selector match + cascade for those entities.
- `ApplyComputedProperties` writes the new component values.
- `EmitRedrawEvent` schedules a redraw.

Hot-reload covers stylesheet edits, font-face edits (0.6 fix for imports), and `@keyframes` edits. It does **not** cover Rust-side `register_component_properties` changes — those require a restart.

## 8. Custom value parsers (0.4.1+)

Public parsing helpers (`parse_duration`, `parse_calc_value`) and a dedicated `examples/custom_parsing.rs` make it possible to extend property value grammar from app code — useful for game-specific units (e.g. `tile`s instead of `px`).

## 9. What's deliberately not in the API

- No `Stylesheet::set_property()` runtime mutation — once loaded, a stylesheet is immutable; only hot-reload swaps it.
- No JavaScript-equivalent `getComputedStyle()` — though `StyleData` does expose the resolved property map per entity for inspection.
- No layered-cascade for multiple stylesheets on one tree — use `@import`.
- No `:focus-visible` (see § 4).
- No mediaQueryList-style imperative queries — `@media` is purely declarative in the stylesheet.

## Cross-references

- The `Styled` component is the analog of bevy_ui's per-entity styling, except scoped to a subtree by inheritance. The "single Styled per tree" constraint is a real limitation if app architecture needs per-screen style scoping.
- Inline-style support overlaps with Buiy's own commitment to ECS-native authoring ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)) — but `RawInlineStyle::parse(...)` is a *string*-based escape hatch, while Buiy's ECS authoring is typed.
- Custom property registration via `RegisterComponentPropertiesExt` is the closest bevy_flair feature to Buiy's token system, but inverted: bevy_flair binds CSS names → Bevy fields; Buiy tokens flow Bevy fields ← typed semantic names.

## Sources

- bevy_flair README — https://github.com/eckz/bevy_flair/blob/main/README.md
- bevy_flair `examples/game_menu.rs` — https://github.com/eckz/bevy_flair/blob/main/examples/game_menu.rs
- bevy_flair `examples/custom_parsing.rs` — https://github.com/eckz/bevy_flair/blob/main/examples/custom_parsing.rs
- CHANGELOG — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- Sibling: [`architecture.md`](architecture.md), [`css-coverage.md`](css-coverage.md), [`integration.md`](integration.md)
