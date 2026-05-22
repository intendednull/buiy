**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — architecture: three-crate workspace, eleven-stage `StyleSystems` pipeline, Servo-cssparser substrate

# Architecture

## 1. Workspace shape

bevy_flair ships as a Cargo workspace with three crates plus the user-facing meta-crate. Verified from `Cargo.toml` on `main` (workspace version 0.8.0-unreleased; 0.7.0 published 2026-02-03 is structurally identical):

| Crate | Role | Notable types |
|---|---|---|
| `bevy_flair_core` | Reflection bridge from CSS properties to Bevy component fields | `PropertyRegistry`, `CssPropertyRegistry`, `ComponentProperty`, `ComponentProperties`, `PropertyValue`, `ReflectValue`, `PropertyMap`, `RegisterComponentPropertiesExt`, `PropertyRegistryPlugin` |
| `bevy_flair_style` | Cascade engine + animations + transitions + the per-frame style pipeline | `FlairStylePlugin`, `Styled`, `StyleSheet` (asset), `StyleData`, `StyleMarkers`, `NodePseudoState`, `StyleSystems` enum, `ReflectAnimationsPlugin` |
| `bevy_flair_css_parser` | Parses `.css` text into Bevy `Asset<StyleSheet>` via the Servo cssparser + selectors crates | `FlairCssParserPlugin`, `parse_duration`, `CalcAdd`/`CalcMul`/`parse_calc_value`, `InlineCssStyleSheetParser` |
| `bevy_flair` | Meta-crate that re-exports common API and ships `FlairPlugin` | `FlairPlugin` (composes the three above) |

The workspace pattern matches the three-crate Servo / cssparser ecosystem split: registry/reflection ↔ cascade ↔ parsing. It is also the same shape bevy_ui itself uses internally (`bevy_ui` ↔ `bevy_ui_render` ↔ `bevy_ui_widgets`) — a deliberate stylistic match.

## 2. The CSS parser

bevy_flair does not write its own CSS tokenizer. It pulls in three crates from the Servo toolchain:

- **[`cssparser`](https://crates.io/crates/cssparser) 0.35** — Servo's CSS tokenizer + at-rule / declaration parsers. Re-exported from `bevy_flair_css_parser`, so consumers can write custom parsers that produce `StyleSheet` assets directly.
- **[`cssparser-color`](https://crates.io/crates/cssparser-color) 0.3** — color value parsing (named colors, hex, `rgb()`, `hsl()`, `oklch()`, …). bevy_flair adds Oklab interpolation on top of this for transitions (per 0.3 CHANGELOG).
- **[`selectors`](https://crates.io/crates/selectors) 0.32** — Servo's selector engine, the same library Servo / Stylo (Firefox) / Servo-derived browsers ship in production. WASM compatibility was a 0.4 fix when the `selectors` crate updated upstream (per 0.4 CHANGELOG).

The implication is large: bevy_flair's selector behavior is *not* a small custom parser; it inherits the Servo selector spec, including specificity calculation, the `:is()` / `:where()` / `:not()` / `:has()` semantics, attribute-selector matching, and pseudo-class arithmetic (`:nth-child(2n+1)`). This is the **single highest-leverage architectural decision** in bevy_flair: leasing the cascade-and-selector substrate from a real browser-engine project rather than reimplementing it.

## 3. `FlairPlugin` setup and plugin ordering

The user-side surface is one line:

```rust
App::new()
    .add_plugins((DefaultPlugins, FlairPlugin))
```

`FlairPlugin` internally adds, in order:

1. `PropertyRegistryPlugin` (from `bevy_flair_core`) — initializes `PropertyRegistry` + `CssPropertyRegistry` resources, registers default Bevy-UI component-property mappings (`background-color` → `BackgroundColor.0`, `padding-left` → `Node.padding.left`, etc.).
2. `FlairStylePlugin` (from `bevy_flair_style`) — installs the per-frame `StyleSystems` pipeline.
3. `FlairCssParserPlugin` (from `bevy_flair_css_parser`) — registers the `.css` asset loader, the inline-style parser (`RawInlineStyle`), and writes parsed declarations into the registry.

Apps that want to register custom property mappings (e.g. `--my-custom-color` → a custom component) do so via `RegisterComponentPropertiesExt` *before* the `FlairPlugin` is built.

## 4. The `StyleSystems` per-frame pipeline

This is the load-bearing detail. `FlairStylePlugin` runs eleven labeled system stages **in `PostUpdate`**, in this order (verified from `bevy_flair_style/src/lib.rs`):

```
Prepare
  → SetStyleData
  → MarkEntitiesForRecalculation
  → TickAnimations
  → CalculateStyles
  → SetPropertyValues
  → ComputeProperties
  → ResolveAnimations
  → SetAnimationValues
  → ApplyComputedProperties
  → EmitRedrawEvent
```

What each stage does:

- **Prepare** — gather precondition data; populate `Styled` ↔ root-entity links.
- **SetStyleData** — propagate `StyleData` updates: which stylesheet does this entity inherit from, what is its current pseudo-state set (`hovered`, `pressed`, `focused`, `disabled`, `checked`).
- **MarkEntitiesForRecalculation** — set `StyleMarkers` bits on entities whose match set may have changed (new sibling, new pseudo-state, ancestor stylesheet change).
- **TickAnimations** — advance `transition` and `@keyframes` clocks by `Time<Real>` delta (the change from 0.5: animations were on `Time<Virtual>` before).
- **CalculateStyles** — run the selector engine against the marked entities, collect matching rulesets with specificity, apply `@layer` / `@import` ordering, resolve `var()` and `calc()` in property values.
- **SetPropertyValues** — write the cascaded `PropertyValue`s into the `PropertyMap` for each entity.
- **ComputeProperties** — convert `PropertyValue` (which can be a token, a `var()`, a `calc()`, or a raw value) into a concrete `ReflectValue` ready to write into a Bevy component field.
- **ResolveAnimations** — generate per-property interpolations for any in-flight transition / keyframe animation; this stage produces individual property streams (the 0.7 overhaul).
- **SetAnimationValues** — apply the interpolated values, overriding the static computed values for the duration of the animation.
- **ApplyComputedProperties** — the actual `world.entity_mut(e).insert(...)` calls that write Bevy components. Components without a corresponding property in the registry are auto-removed (0.6 behavior).
- **EmitRedrawEvent** — request a redraw if any animation is still in flight (drives Bevy's reactive redraw scheduling).

The pipeline is `PostUpdate`-scheduled, *after* user game logic and Bevy-input updates but *before* layout and rendering. So a single frame is: user mutates state → bevy_ui input updates pseudo-states (hover, focus) → bevy_flair cascades → bevy_ui lays out → render.

## 5. Property reflection

`bevy_flair_core` is the secret sauce that lets a CSS property name turn into a write to a Bevy component field without a hand-written match arm per property. The `PropertyRegistry` stores entries like:

```rust
// pseudocode of the registered mapping
ComponentProperty {
    css_name: "background-color",
    component_type_id: TypeId::of::<BackgroundColor>(),
    field_path: ".0", // via bevy_reflect path
    parser: parse_color,
}
```

When the cascade resolves `background-color: red` for an entity, the `ApplyComputedProperties` stage looks up the `ComponentProperty`, reflects into the entity's `BackgroundColor` component, and writes the value. Components that the entity doesn't yet have are **inserted automatically** (per README: `"the corresponding component is inserted automatically when the corresponding property is used"`). Components whose properties go fully unset get auto-removed (0.6).

Shorthand properties (`margin: 8px 16px`) are expanded by the parser into longhand declarations before reaching the registry.

## 6. Selector resolution + cascade

The selector engine is the Servo `selectors` crate's `MatchingContext` driven by a bevy_flair adapter that exposes Bevy entities as the DOM the selector engine queries. Supported selectors (verified against CHANGELOG + README, full coverage in [`css-coverage.md`](css-coverage.md)):

- Type selectors (Bevy component / `TypeName` component, 0.4+)
- ID, class, attribute selectors
- `:hover`, `:active`, `:focus`, `:disabled`, `:checked` pseudo-classes (sourced from `NodePseudoState`)
- `:nth-child()`, `:first-child`, `:last-child`
- `:not()`, `:has()`, `:is()`, `:where()` (0.2+)
- Combinators: descendant (` `), child (`>`), adjacent sibling (`+`), general sibling (`~`)
- Nested selectors via `&` (CSS Nesting Level 1)
- Pseudo-elements `::before`, `::after` (0.4+) via `PseudoElementsSupport`

Specificity follows CSS spec via the `selectors` crate. `!important` is **detected and parsed** but **ignored with a warning**; the cascade doesn't honor it. This is a deliberate non-goal (per README), justified on the grounds that without `!important` the cascade is simpler and there are fewer foot-guns; the cost is that overriding a deeply-specific selector is harder.

`@layer` (cascade layers, 0.4+) is supported and changes cascade order before specificity. `@import` (0.2+) inlines another stylesheet's rules into the current cascade context.

## 7. Hot-reload

Stylesheets are Bevy `Asset`s loaded by `FlairCssParserPlugin`'s asset loader. Editing the `.css` file on disk triggers Bevy's standard `AssetEvent::Modified`; the `MarkEntitiesForRecalculation` stage observes the event and flags all entities downstream of the affected `Styled` root for recalculation on the next `PostUpdate`. The whole cascade is rerun on the next frame; there is no diff-based partial reapplication.

In practice (per README + community discussion): hot-reload is one of the headline features. "edit your `.css` file and see styles re-applied on the fly" is the framing.

## 8. Cross-references

- The cascade-engine choice (Servo `selectors` 0.32) parallels bevy_ui's own choice to lean on Taffy for layout: lease a real-spec implementation rather than reinvent it. Same lesson as [`bevy-ui/architecture.md`](../bevy-ui/architecture.md) § Taffy integration.
- The eleven-stage `PostUpdate` pipeline is a parallel to bevy_ui's `BuiySet::Layout → Style → Input → Animate → Picking → A11yUpdate → Render` ordering ([`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.8). bevy_flair runs in `PostUpdate`; Buiy's would run in its own `BuiySet::Style` stage.
- Reflection-driven property mapping aligns with Buiy's BSN-friendly reflection requirement ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)) — `bevy_flair_core::ComponentProperty` is essentially what a stylesheet layer over BSN-friendly Buiy components would need.

## Sources

- bevy_flair `Cargo.toml` on `main` — https://github.com/eckz/bevy_flair/blob/main/Cargo.toml
- `bevy_flair_style/src/lib.rs` — module docs for `StyleSystems` ordering — https://github.com/eckz/bevy_flair/blob/main/crates/bevy_flair_style/src/lib.rs
- `bevy_flair_core/src/lib.rs` — `PropertyRegistry` API — https://github.com/eckz/bevy_flair/blob/main/crates/bevy_flair_core/src/lib.rs
- `bevy_flair_css_parser/src/lib.rs` — cssparser + selectors integration — https://github.com/eckz/bevy_flair/blob/main/crates/bevy_flair_css_parser/src/lib.rs
- CHANGELOG — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- cssparser crate — https://crates.io/crates/cssparser
- selectors crate — https://crates.io/crates/selectors
- Sibling: [`css-coverage.md`](css-coverage.md), [`api.md`](api.md), [`integration.md`](integration.md)
