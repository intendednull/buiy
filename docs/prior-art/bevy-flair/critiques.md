**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — critiques and open problems: adoption, scope, performance, maintenance, cascade-vs-tokens tension

# Critiques & open problems

This file folds the critiques and the open-problems lists together — for a crate this small the two lists overlap significantly. Honest tone, no soft-pedaling.

## Critiques

### Adoption is small.

5,885 total downloads from January 2025 through May 2026 (16 months). The recent-90-day burst (1,336 downloads = 23% of total) suggests rising adoption post-0.7, but absolute numbers are tiny by Rust-ecosystem standards. By comparison, the substrate `selectors` crate has ~75 million downloads, `cssparser` ~84 million. bevy_flair's adoption is **three orders of magnitude smaller than its own dependencies' general-purpose adoption**, even allowing for the niche.

130 GitHub stars + 11 forks confirms the picture: drive-by interest, not active contributor base. Three followers on the maintainer account is unusual — most Bevy-ecosystem maintainers with comparable star counts cluster at 50–500 followers because they cross-pollinate with neighbouring projects.

**Implication:** if Buiy adds a stylesheet layer, the *bevy_flair user base* is not the user base it will inherit. There is no large incumbent CSS-in-Bevy community to migrate; Buiy would create the user base or extend it.

### Single-maintainer bus factor.

Documented in [`governance.md`](governance.md). One author. No co-maintainers. No funding. No fallback fork visible. The Bevy 0.18 → 0.19 jump is upcoming (rc.1 already published 2026-05-13 per [`../bevy-ui/`](../bevy-ui/)); whether bevy_flair tracks 0.19 in time, given the parley/swash text-stack migration disrupting text-related properties, is the next maintenance signal.

### Scope: bevy_flair only paints what bevy_ui already supports.

The README is explicit: `"we don't implement missing Bevy UI features."` So:

- `backdrop-filter`, `mix-blend-mode`, `isolation`, true CSS top layer — none of these work in bevy_flair because bevy_ui can't render them ([`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Renderer caps).
- Container queries — Taffy doesn't yet support container-relative units (`cqw`, `cqh`) cleanly; bevy_flair doesn't expose what it can't drive.
- Logical properties (`inline-size`, `block-size`, `padding-inline-*`) — not mentioned anywhere.
- `:focus-visible` — bevy_ui doesn't distinguish keyboard-vs-pointer focus, so bevy_flair can't either.

For a project committed to "comprehensive web parity" (Buiy goal 1), bevy_flair's stylesheet does **not** unlock web parity — the renderer + layout substrate caps still bind.

### Performance is asserted but not benchmarked publicly.

The README's "efficient and reactive, no unnecessary re-application" claim relies on marker-driven recalculation. The `benches/` directory exists in the repo but no results are published in README or release notes. There is **no public answer to "at what node count does selector matching dominate frame time?"** This is the same gap [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md) flags about bevy_ui itself, and it compounds — bevy_flair's pipeline runs *on top of* bevy_ui's per-frame work.

The Servo `selectors` crate is fast at browser scale (millions of nodes amortized across multiple frames). bevy_flair runs the cascade fresh against bevy_ui's marker-set every `PostUpdate` for affected entities. At 1000+ nodes with pseudo-state churn (e.g. a productivity-app form), the cost has no published bound.

### Cascade-vs-tokens is a real tension that bevy_flair takes one side of.

bevy_flair's cascade is **inheritance-based + specificity-resolved**, exactly like CSS. Properties like `color` and `font-family` inherit from ancestors automatically. This is great for matching web semantics; it is a *different* primitive from Buiy's token system, where a component consumes a typed semantic token (`color.surface.primary`) by name, with no inheritance.

The two models are not contradictory but they have different ergonomics:

- **Inheritance:** "everything under this subtree picks up `color: var(--text-primary)` unless overridden."
- **Tokens:** "every component that needs a foreground color references `color.text.primary` by name; tokens are typed, hot-reloadable assets."

Tokens are easier to lint (the contrast linter, foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)) because every consumer is explicit. Inheritance is easier to author (less typing) and matches what designers expect from a web background. The Buiy stance is tokens-first; bevy_flair would add inheritance on top, and the design question is whether the resulting two-layered system is coherent or fragmented.

### `!important` ignored is a foot-gun.

Detected, parsed, logged-as-warning, **not honored**. Web developers reach for `!important` reflexively when a more-specific selector is misbehaving; bevy_flair silently failing on `!important` will produce hours of confused debugging. The non-goal is principled (cascade is simpler without `!important`) but the warning-not-error stance is a usability bug.

### Documentation lag.

- README license line says "MIT"; actual license is "MIT OR Apache-2.0" (per `Cargo.toml` and crates.io). One-line confusion, mid-priority fix.
- The Styled-vs-NodeStyleSheet rename is in CHANGELOG but the README example as of 2026-05-22 still uses the new `Styled` name without flagging the pre-0.8 alias.
- No docs.rs prose beyond inline rustdoc. No "writing your first stylesheet" tutorial. No "migrating from programmatic styling" guide.
- The implicit clobber-semantics between bevy_flair and direct component writes (covered in [`api.md`](api.md) § 6, [`integration.md`](integration.md) § 3) is **not documented anywhere in the repo**. Users discover it experimentally.

### The bevy_feathers blind spot.

bevy_flair styles bevy_ui entities. bevy_feathers widgets are *built on* bevy_ui but encapsulate visual state in widget-internal components that bevy_flair has no mapping for. Result: a user adopting both crates gets a half-styled app — the outer container reacts to CSS, the widget interiors don't. There is no documented integration path, no `bevy_feathers_flair` adapter crate, no upstream effort visible.

For Buiy: if Buiy ships widgets *and* stylesheets, every styleable widget surface must be a separate Buiy component end-to-end (foundation [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns) commits to this for visual components). bevy_feathers's encapsulation is exactly the anti-pattern.

## Open problems

### Selector subset extent.

The 0.32-vintage Servo `selectors` crate ships a near-complete selector engine, but bevy_flair does not exercise every feature. Unverified:

- `:nth-of-type()`, `:nth-last-child()`, `:nth-last-of-type()` (inherited from `selectors`; not explicitly tested).
- `:dir(ltr/rtl)` (writing-mode-aware selector).
- `:lang()`, `:scope`.
- Logical pseudo-classes for form validation (`:user-valid`, `:user-invalid`, `:placeholder-shown`).
- The `[attr i]` / `[attr s]` case-sensitivity modifier.

A future audit (Stage 4 deep-dive for a follow-up corpus refresh) would map every Servo-selectors pseudo-class onto bevy_flair test coverage.

### CSS Cascade Layers (`@layer`) edge cases.

0.4 added `@layer` support. The spec edges around nested layers, layer revertes (`revert-layer`), and import-with-layer (`@import url(...) layer(...)`) are not exercised by examples and not mentioned in CHANGELOG fixes. Cascade-correctness in these corners is unverified.

### Container queries.

Not supported. Taffy is the bottleneck — until Taffy ships container-relative-units + container-context tracking, bevy_flair can't expose them either. The Buiy foundation tier ([visuals.md § 3.2](../../specs/2026-05-07-buiy-foundation/visuals.md)) lists container queries as **C** (core), so a future Buiy stylesheet layer would need to absorb container-query support concurrently with Taffy's upstream work, *not* inherit it from bevy_flair.

### CSS variables ↔ Buiy tokens integration.

bevy_flair `var(--name)` is a *string-keyed lookup* inside the stylesheet — typed only by its consumer (a color property expects a parseable color value). Buiy tokens are *typed, hot-reloadable assets* (foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)).

The integration question: if Buiy adds a stylesheet layer, does `var(--color-surface-primary)` resolve to a Buiy token asset, or is the variable a separate string namespace? The former preserves the token system's typing + linting; the latter is closer to CSS spec. This is a real design fork, not a small detail.

### Animations / transitions reliability.

0.6 fixed multiple `:hover`-animation-restart bugs; 0.7 overhauled the entire animation system to be per-property. The system is young — the per-property model has shipped for ~4 months as of May 2026, and reliability under composition (multiple `transition` + multiple `@keyframes` + `var()` in animation values + custom `Time` source) is not documented under stress.

### Hot-reload reliability.

Hot-reload works for stylesheet content edits. Less clear:
- Adding a new `@font-face` to a file that didn't previously have one (0.6 fixed font-face during `@import`, but the standalone case is not explicitly tested).
- Changing `@layer` order between two stylesheet versions.
- Removing a class from a rule that was actively matching when the change shipped (does the auto-removal trigger correctly?).

The 0.6 release notes fix "@keyframes var() usage error reporting" — meaning error feedback on hot-reload failures was, as of 0.5, silent. Improved but not benchmarked.

### Cross-window stylesheets.

A `Handle<StyleSheet>` is shareable across windows. Whether `@media (resolution: ...)` resolves per-window or per-app is unverified. Multi-window apps with different DPI displays (4K main monitor + Retina laptop) need per-window media-query evaluation; bevy_flair's behavior here is undocumented.

### `:focus-visible` and form-validation pseudo-classes.

Both blocked on bevy_ui not modeling them. If Buiy ships its own focus model (foundation [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns) commits to `:focus-visible` semantics) and form-validation pseudo-classes (foundation [interaction.md](../../specs/2026-05-07-buiy-foundation/interaction.md) "Forms" section), a Buiy stylesheet layer can support them. bevy_flair-on-bevy_ui cannot.

### Forced-colors / prefers-contrast / prefers-reduced-motion.

`@media (prefers-color-scheme: dark)` works. `forced-colors` / `prefers-contrast` / `prefers-reduced-motion` are not in the CHANGELOG; presumably **not honored**. For Buiy WCAG 2.2 AA compliance (foundation [README.md goal 2](../../specs/2026-05-07-buiy-foundation/README.md)) this is non-optional — a Buiy stylesheet layer must extend `@media` to bind OS prefs into the cascade.

### Reflection-cost overhead.

`bevy_flair_core::ApplyComputedProperties` uses `bevy_reflect` to write into component fields. Reflection has measurable overhead vs direct field writes. At what node count does this dominate? Unbenched.

### Production game shipping.

No published production game uses bevy_flair as the styling layer as of 2026-05-22. The repo's flagship example is `examples/game_menu.rs` — a menu screen, not a full game UI. Same `no flagship` gap [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) calls out about bevy_ui itself.

## Sources

- bevy_flair README "Limitations" — https://github.com/eckz/bevy_flair/blob/main/README.md
- CHANGELOG (per-version fix lists) — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- crates.io download data — https://crates.io/crates/bevy_flair
- Servo `selectors` crate (substrate) — https://crates.io/crates/selectors
- Buiy foundation README (open question on CSS stylesheets) — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5
- Buiy theming spec scaffold — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5
- bevy_ui critiques (renderer caps) — [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)
- Sibling: [`governance.md`](governance.md), [`lessons.md`](lessons.md)
