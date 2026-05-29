**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — glossary of crate-specific terms

# Glossary

bevy_flair-specific names and concepts. CSS-standard terms (cascade, specificity, `@media`, selectors) are not re-defined here; consult the CSS spec or [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS) for those.

- **`FlairPlugin`** — Top-level Bevy plugin. Composes `PropertyRegistryPlugin` + `FlairStylePlugin` + `FlairCssParserPlugin`. The one line a user adds: `app.add_plugins(FlairPlugin)`.

- **`FlairStylePlugin`** — Sub-plugin from `bevy_flair_style`. Installs the eleven-stage `StyleSystems` pipeline in `PostUpdate` and registers `Styled`, `StyleSheet`, and related types.

- **`FlairCssParserPlugin`** — Sub-plugin from `bevy_flair_css_parser`. Registers the `.css` asset loader, inline-style parsing, and integrates `cssparser` / `selectors`.

- **`PropertyRegistryPlugin`** — Sub-plugin from `bevy_flair_core`. Initializes `PropertyRegistry` and `CssPropertyRegistry` resources with default Bevy-UI component mappings.

- **`Styled`** — Component that attaches a `Handle<StyleSheet>` to a root entity. Styling propagates down the `UiChildren` hierarchy. **Renamed from `NodeStyleSheet` in the 0.8 dev cycle**; both names refer to the same idea. `Styled` is the long-term name.

- **`StyleSheet`** — Bevy `Asset` type produced by parsing a `.css` file (or string, via `InlineCssStyleSheetParser`). Contains the rules, at-rules, and font-face references for one stylesheet.

- **`StyleData`** — Component holding the resolved style state for an entity: which stylesheet, which pseudo-states are active, what the cascaded property map looks like.

- **`StyleMarkers`** — Component holding the dirty bits used by `MarkEntitiesForRecalculation` — which entities need their cascade re-run this frame.

- **`NodePseudoState`** — Component tracking which pseudo-classes are active on an entity (`hovered`, `pressed`, `focused`, `disabled`, `checked`). Set by bevy_ui's input systems; consumed by the cascade.

- **`StyleSystems`** — Enum labeling the eleven per-frame system stages: `Prepare`, `SetStyleData`, `MarkEntitiesForRecalculation`, `TickAnimations`, `CalculateStyles`, `SetPropertyValues`, `ComputeProperties`, `ResolveAnimations`, `SetAnimationValues`, `ApplyComputedProperties`, `EmitRedrawEvent`. Renamed from `StyleSystemSets` in 0.5.

- **`PropertyRegistry`** — Resource storing the master list of `ComponentProperty` entries: every CSS property name and how it maps to a Bevy component field. Populated by default Bevy-UI mappings + app-side `register_component_properties::<T>()` calls.

- **`CssPropertyRegistry`** — Resource adjunct to `PropertyRegistry`, storing CSS-specific parsing context (parsers for color values, length values, etc.).

- **`ComponentProperty`** — Single (CSS name, component type, field path, parser) tuple. The unit of binding.

- **`ComponentProperties`** — Trait implemented by component types that opt into CSS-driven field writes.

- **`PropertyValue`** — Resolved value of a CSS property after `var()` and `calc()` resolution, before being written to a component. Can hold a parsed literal, an inherited marker, an `unset`, or an animation-interpolated value.

- **`PropertyMap`** — Per-entity map of property names to `PropertyValue`s representing the cascade output.

- **`ReflectValue`** — `bevy_reflect`-friendly wrapper used by `ApplyComputedProperties` to write into typed component fields via reflection.

- **`RawInlineStyle`** — Component holding an inline-style declaration block, equivalent to HTML's `style="..."` attribute. Immutable as of 0.7, built only via `Ruleset`.

- **`InlineCssStyleSheetParser`** — Parser variant (0.5+) for stylesheets sourced from strings rather than `.css` files. Useful for tests, embedded literals, and tooling.

- **`Ruleset`** — Internal representation of a parsed CSS rule (selector + declaration block).

- **`AnimationEvent`** — Event fired when a transition or `@keyframes` animation starts, ends, or iterates (0.5.1+). Lets app code react to animation lifecycle.

- **`ReflectAnimationsPlugin`** — Sub-plugin enabling reflection-driven animations.

- **`TypeName` component** — Component (0.4+) attached to entities that should match a CSS type selector by a custom name. Replaced `TrackTypeNameComponentPlugin` (0.4).

- **`-bevy-*` properties** — Vendor-prefixed CSS properties for Bevy-specific behavior. Examples: `-bevy-image-mode` (stretch / tile / 9-slice), `-bevy-image-rect` (9-slice insets). Convention introduced in 0.3.

- **`PseudoElementsSupport`** — Component opting an entity into `::before` / `::after` pseudo-element generation (0.4+). The pseudo-elements become synthetic child entities.

- **`GhostNode`** — Bevy 0.18 concept for entities that participate in the entity hierarchy but not in layout. bevy_flair preserves styling propagation across `GhostNode`s (0.6+); the `experimental_ghost_nodes` Cargo feature opts into expanded handling.

- **`CalcAdd` / `CalcMul` / `parse_calc_value`** — Internal building blocks for `calc()` expression evaluation. Public-by-feature since 0.4.1 for app-side extension.

- **Bus factor** — Number of contributors whose simultaneous departure would halt the project. For bevy_flair: 1.

## Sources

- bevy_flair workspace crate roots — https://github.com/eckz/bevy_flair/tree/main/crates
- CHANGELOG (for rename + introduction dates) — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- Sibling: [`architecture.md`](architecture.md), [`api.md`](api.md)
