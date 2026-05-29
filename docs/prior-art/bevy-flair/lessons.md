**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — Validates / Avoid / Borrow + the honest case on whether Buiy should adopt a CSS stylesheet layer

# Lessons for Buiy

This is the consult-this-when-designing decision file. The other files in this corpus are evidence; this file is the synthesis. The Buiy foundation spec poses the open question ([README.md § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)):

> **CSS-flavored stylesheet.** Never, or as a future layer above tokens? bevy_flair sets one precedent; the right answer depends on user demand.

This file does not answer that question — only the Buiy team can, after gathering user-demand evidence. But it frames the trade-off honestly.

## Top of file: the critical question

**Should Buiy adopt a CSS-flavored stylesheet layer as a future sub-spec, above the token system?**

The honest answer is **"it depends, and the answer is not obvious."** The arguments on each side:

### Arguments FOR a future Buiy stylesheet layer

1. **Web-developer onboarding.** A meaningful fraction of potential Buiy users will come from web development. They expect to write `.css` files. The cost of *not* providing that path is invisible — those users go to a different framework.
2. **Hot-reload of visuals without recompile.** Even with BSN hot-reload, BSN reloads structure + components; pure-visual edits (color tweaks, padding adjustments) feel heavier than CSS hot-reload's "edit, save, see." Designers iterate faster against stylesheets than against typed Rust assets.
3. **The cascade is a real authoring primitive.** Inheritance + specificity is a different (and sometimes better) primitive from token-based explicit binding. "All text in this dialog uses smaller font" is one line in CSS, N lines in tokens-explicitly-bound.
4. **Servo's `cssparser` + `selectors` exist.** The expensive part — a real CSS parser + selector engine — is off-the-shelf, MIT/Apache-2.0, and battle-tested in production browsers. The integration cost is the bridge, not the cascade engine.
5. **bevy_flair has proven the bridge is feasible** in ~16 months by one developer. A Buiy stylesheet layer, built on Buiy's decomposed-component-friendly foundation, would inherit fewer of bevy_flair's limitations.

### Arguments AGAINST a future Buiy stylesheet layer

1. **Tokens already cover the static-styling story.** Foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system) commits to semantic tokens consumed by components; F-tier [`var()` / `calc()` / `min` / `max` / `clamp`](../../specs/2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering) value functions; OS-pref-bound variants. Adding a stylesheet layer is **not necessary to ship a designed UI**.
2. **Bevy's stated direction is against CSS-style stylesheets.** The community discussions ([#1522](https://github.com/bevyengine/bevy/discussions/1522), [#9652](https://github.com/bevyengine/bevy/discussions/9652)) consistently land on "ECS-native, not web-style." Buiy is parallel to bevy_ui but lives in the Bevy ecosystem; siding with the minority view has community-fit cost.
3. **Two layers fragment the mental model.** "Where does this color live? In a token? In a stylesheet? In a programmatic write? In an inline style?" — multiplied per property per component. Authoring debuggability suffers when there are too many overlapping precedence rules.
4. **Cascade-correctness is hard.** bevy_flair leases `selectors` but still implements the entity-tree bridge, and that bridge has unverified edge cases ([`critiques.md`](critiques.md) § Open problems). Buiy would either inherit those edge cases or duplicate the work. Either way, it's not free.
5. **The Buiy-token system already exposes `var()` syntax in tokens.** A designer can write `var(--color-surface-primary)` against the token system without a stylesheet wrapper. The marginal value of `.css` files over typed token assets is "selectors + inheritance" — which is real, but localized.
6. **Bus-factor of the prior art is 1.** [`governance.md`](governance.md). The reference implementation is one developer's side project. Buiy can't bake against it, and reimplementing the cascade-on-Buiy-components is non-trivial scope.
7. **Tokens are easier to lint.** The Buiy contrast linter (foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)) validates token sets at load time. CSS cascade computation is harder to lint because the *applied* value depends on the entity tree at runtime.

### Pragmatic stance

The decision should track user demand:

- **If, post-foundation-v1, Buiy users (especially app-not-game users) consistently request `.css` files**, draft `buiy-css-stylesheet-design` as a follow-up sub-spec. The token system + reflection registry is the substrate; the stylesheet layer is a parser + cascade engine on top.
- **If user demand stays low / focused on better tokens**, do not ship a stylesheet layer. The token system is sufficient.

Foundation [README.md § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) is correct as written: keep this as an open question, do not pre-commit, gather evidence first.

---

## Validates

These Buiy design choices are confirmed by bevy_flair's experience:

- **Decomposed visual components ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns)).** bevy_flair's whole approach — auto-inserting `BackgroundColor`, `BorderColor`, `BorderRadius`, `Outline`, `BoxShadow` components when CSS rules reference them — only works because bevy_ui has already split those visuals into separate components. The pattern would be impossible against megacomponent designs. Buiy's commitment to small, decomposed, public-fielded components is exactly what would make a *future Buiy* stylesheet layer feasible.
- **Reflection + property registries ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)).** bevy_flair's `PropertyRegistry` is the BSN-friendly reflection pattern in production. Every Buiy component already requires `Reflect + FromReflect + Default + Clone + Component`; the same registry could be built for Buiy.
- **`var()` / `calc()` as F-tier ([visuals.md § 3.3 "Custom properties + value functions"](../../specs/2026-05-07-buiy-foundation/visuals.md)).** bevy_flair ships these as foundational features and they're heavily used in the game-menu example. Buiy treating them as F-tier is validated.
- **Hot-reloadable theme assets ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)).** bevy_flair's hot-reload story works in production; Buiy's commitment to hot-reloadable tokens via the asset system is the same primitive.
- **OS-pref-driven variant binding ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)).** bevy_flair partially does this via `@media (prefers-color-scheme)`; the partial coverage validates the primitive but proves the OS-pref → cascade binding is a real piece of work, not free.

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Single-maintainer hard dependency.** bevy_flair is one person's side project; baking Buiy against it would inherit the bus-factor. | [`governance.md`](governance.md) | Treat bevy_flair as a *design reference*, never a runtime dependency. If a stylesheet layer ships, it's Buiy-owned code. |
| **`!important` parsed but silently ignored.** Bevy_flair logs a warning and proceeds; users debug for hours before noticing. | [`css-coverage.md`](css-coverage.md) "Cascade" | If Buiy ships a stylesheet layer, either honor `!important` or error-on-parse. Don't warn-and-discard. |
| **Clobber semantics undocumented.** bevy_flair's `ApplyComputedProperties` clobbers programmatic component writes for any component it manages. Not in any docs. | [`api.md`](api.md) § 6, [`integration.md`](integration.md) § 3 | If Buiy ships a stylesheet layer, document precedence between stylesheet / inline / programmatic / BSN authoring *in the spec*, not as a footnote. |
| **bevy_feathers blind spot.** bevy_flair styles bevy_ui surfaces; bevy_feathers widget interiors are opaque. Half-styled UI. | [`integration.md`](integration.md) § 4 | Buiy widgets must expose every styleable surface as a separate component end-to-end. If a stylesheet layer ships, it must reach every visual property of every Buiy widget without exception. |
| **No published benchmarks.** Performance is asserted, not measured. | [`critiques.md`](critiques.md) | Buiy verification harness ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) must benchmark any future stylesheet layer at 1000-node + high-pseudo-state-churn fixtures, with per-frame budgets. |
| **Forced-colors / prefers-contrast / prefers-reduced-motion not handled.** WCAG-relevant OS prefs absent from bevy_flair's `@media` support. | [`critiques.md`](critiques.md) Open problems | Buiy foundation requires all five OS prefs to flow into theme variants ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)). A stylesheet layer must extend `@media` to honor all of them, not just `prefers-color-scheme`. |
| **Cascade inheritance set undocumented.** Default-inherited properties (`color`, `font-family`, …) are not publicly enumerated. | [`css-coverage.md`](css-coverage.md) Inheritance | If Buiy ships a stylesheet layer, the default-inherited property set is part of the spec. No "and others" language. |
| **String-keyed `var()` decoupled from typed tokens.** bevy_flair `var(--name)` resolves at consumer-parse time; type errors surface late. | [`critiques.md`](critiques.md) Cascade-vs-tokens | A Buiy stylesheet layer's `var(--token-name)` should resolve against the typed Buiy token registry directly. Type errors at load time, not at apply time. |
| **`:focus-visible` absent because bevy_ui doesn't model it.** | [`css-coverage.md`](css-coverage.md) | Buiy ships its own focus model with `:focus-visible` semantics ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns)). A Buiy stylesheet layer **must** expose `:focus-visible` — it's a WCAG-relevant pseudo-class. |
| **One stylesheet per entity tree.** Multi-stylesheet layering only via `@import`. Limits modular composition. | [`api.md`](api.md) § 1 | Buiy stylesheet layer should support multiple stylesheets on one tree with explicit cascade-layer assignment, or document why it doesn't. |

## Borrow

Concrete patterns worth studying if Buiy ever adds a stylesheet layer:

1. **Three-crate split: registry ↔ cascade ↔ parsing** ([`architecture.md`](architecture.md) § 1). bevy_flair_core / bevy_flair_style / bevy_flair_css_parser cleanly separates "what does each Bevy component field look like in CSS" from "what is the cascade engine" from "what is the parser." Mirroring this in `buiy_style_core` / `buiy_style_cascade` / `buiy_style_css_parser` would let a CSS parser be one of several frontends (TOML, JSON, programmatic) over the same registry + cascade.

2. **Servo `cssparser` + `selectors` as the parser substrate.** Production-quality, browser-tested, MIT/Apache-2.0. Reinventing a CSS tokenizer + selector engine is months of work that bevy_flair sensibly skipped. Buiy would do the same.

3. **The eleven-stage `StyleSystems` pipeline in `PostUpdate`** ([`architecture.md`](architecture.md) § 4). The decomposition into Prepare → SetStyleData → MarkEntitiesForRecalculation → TickAnimations → CalculateStyles → SetPropertyValues → ComputeProperties → ResolveAnimations → SetAnimationValues → ApplyComputedProperties → EmitRedrawEvent is *over-decomposed for its scale* but it gives sub-specs and tests clear ordering points. Buiy's `BuiySet::Style` would similarly split into labeled stages.

4. **Marker-driven recalculation.** Only entities whose match-set changed (sibling added, pseudo-state flipped, ancestor stylesheet swapped) get the cascade re-run. The bookkeeping pattern via `StyleMarkers` is a useful primitive.

5. **`PseudoElementsSupport` for `::before` / `::after`.** Synthetic child entities for pseudo-elements. The pattern generalizes to anywhere Buiy needs "logical-but-not-spawned" children — e.g. focus rings, ARIA descriptions injected for AT.

6. **Auto-insert components when properties reference them** ([`api.md`](api.md) § 6). Spawning an entity with `Styled` and no `BackgroundColor` works because the cascade *inserts* `BackgroundColor` when a rule writes to it. The inverse: auto-removal when no rule matches. This is the right ergonomic — users don't have to pre-spawn every possible visual component.

7. **`-bevy-*` vendor-prefix convention** ([`css-coverage.md`](css-coverage.md) "Custom Bevy extensions"). For Bevy-specific properties without a CSS equivalent. Buiy would use `-buiy-*` similarly — preserves spec compatibility for the standard properties, namespaces extensions cleanly.

8. **Oklab interpolation for color transitions** (0.3 CHANGELOG). Perceptually uniform color interpolation is what the foundation's `oklab()` / `oklch()` color formats ([visuals.md § 3.3](../../specs/2026-05-07-buiy-foundation/visuals.md)) want for animation. bevy_flair already does this; Buiy's animation system should too.

9. **`InlineCssStyleSheetParser` for string stylesheets** (0.5). Loading stylesheets from non-file sources (network, embedded literal, devtools input) is useful in tests + tooling. A Buiy stylesheet layer should expose the same.

10. **Hot-reload as first-class, not afterthought.** From 0.1 onward bevy_flair treated `.css` hot-reload as the headline feature. The Buiy theme system already commits to hot-reload; a stylesheet layer would inherit the same primitive.

## How to use this file

When the question "should Buiy ship a CSS-flavored stylesheet layer?" is on the table:

1. Read the **Top of file** section. The two argument lists are the structured trade-off — bring your own data on user demand to either side.
2. Read **Validates** to confirm what bevy_flair proves about the substrate Buiy is already committed to.
3. Read **Avoid** before writing any spec. Every row is a constraint a future `buiy-css-stylesheet-design` sub-spec must address.
4. Read **Borrow** as the implementation cookbook. None of these patterns is original to Buiy; all are taken from bevy_flair's working code.

If the decision is **don't ship a stylesheet layer**, this file remains useful as the record of *why* — and the record bevy_flair's design lessons aren't lost just because Buiy didn't adopt its surface.

## Sources

- bevy_flair repository — https://github.com/eckz/bevy_flair
- bevy_flair CHANGELOG — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- Buiy foundation README open question — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5
- Buiy foundation architecture theming section — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5
- Buiy foundation visuals tier list — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.3
- bevy_ui lessons (megacomponent anti-pattern, decomposition payoff) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Bevy community CSS-skepticism — https://github.com/bevyengine/bevy/discussions/1522, https://github.com/bevyengine/bevy/discussions/9652
- Sibling evidence: [`architecture.md`](architecture.md), [`css-coverage.md`](css-coverage.md), [`api.md`](api.md), [`integration.md`](integration.md), [`critiques.md`](critiques.md), [`ecosystem.md`](ecosystem.md), [`governance.md`](governance.md), [`history.md`](history.md)
