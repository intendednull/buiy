**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — visual styling primitives, bevy_feathers theming, the absent stylesheet, and user-preference support

## Core styling primitives (in bevy_ui itself)

bevy_ui's "styling" is decomposed component-level visual decoration. There is no stylesheet, no selector system, no inheritance, no cascade. Each visual property is its own component on the node entity:

- **`BackgroundColor(Color)`** — fill.
- **`BorderColor`** — border. As of Bevy 0.17, this is per-side (`top` / `right` / `bottom` / `left`), enabling tricks like faked button depth ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)).
- **`Outline`** — outline drawn outside the border edge; does not affect layout.
- **`BackgroundGradient` / `BorderGradient`** — 0.17+. Linear, Conic, Radial; color stops with configurable interpolation color space ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)). Both stack with `BackgroundColor` / `BorderColor`.
- **`BoxShadow`** — outer-only drop shadow. Multiple shadows per node since 0.16 ([0.15→0.16 migration](https://bevy.org/learn/migration-guides/0-15-to-0-16/)). No inset.
- **`border_radius`** — a *field on `Node`* as of 0.18 (was a separate component pre-0.18, see [component-model.md](component-model.md)).

Colors are `bevy_color::Color`, which supports `Srgba`, `LinearRgba`, `Hsla`, `Hsva`, `Hwba`, `Laba`, `Lcha`, `Oklaba`, `Oklcha`, `Xyza`. Per-property conversion happens at extract-time. There is no first-class P3 / Rec.2020 / wide-gamut output path — bevy_ui renders sRGB-encoded by default.

What is *not* present in bevy_ui core:

- No `filter` analogue (CSS `blur`, `drop-shadow`-as-filter, `grayscale`, etc.).
- No `backdrop-filter`. See [architecture.md § "Renderer caps"](architecture.md).
- No `mix-blend-mode` per node. `UiMaterial` lets per-material blend state be specialised, but there is no node-level CSS-mix-blend semantic.
- No `mask-image` / `clip-path` beyond the rectangular-plus-border-radius case.
- No CSS `filter` color-matrix.

## No token system in bevy_ui itself

bevy_ui ships *property-level components* but not a token vocabulary. There is no `Token<color::surface::primary>` indirection, no theme registry that maps a semantic name to a `Color`. Every component carries a literal value. Sharing a color across a UI is on the application — typically by extracting the color into a `Resource` or const and referencing it at spawn time.

This is a deliberate "primitives first" stance and is the layer Buiy's foundation spec critiques: see `docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.5 (semantic-tokens-only authoring, OS-pref-bound variants, hot-reloadable theme assets).

## bevy_feathers theming

`bevy_feathers` (0.17+, experimental) sits on top of bevy_ui and ships a thin theme layer aimed at the upcoming Bevy editor and developer tooling. Per its [Cargo.toml](https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/Cargo.toml): `"A collection of UI widgets for building editors and utilities in Bevy."`

The theming module is, structurally, a small set of files ([feathers src tree](https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src)):

- `theme.rs` — `UiTheme` resource, theme application machinery.
- `tokens.rs` — design-token names (semantic identifiers).
- `palette.rs` — the Feathers standard palette (concrete colors).
- `dark_theme.rs` — dark-variant token-to-color map.
- `font_styles.rs` — typographic tokens.
- `rounded_corners.rs`, `constants.rs`, `cursor.rs`, `focus.rs`, `alpha_pattern.rs` — auxiliary visual primitives used by the standard widgets.

Behaviour, in broad strokes:

- `UiTheme` is a resource that maps token identifiers to concrete styling values.
- Feathers widgets read the active theme to pull background/border/text colors at spawn or update time.
- The dark theme is the default ships-with-Bevy theme; there is a light theme placeholder.
- Themes are *not* hot-reloadable assets in 0.18. They are constructed in code.

What Feathers' theming does NOT provide as of 0.18:

- No hot-reloadable theme assets.
- No `prefers-contrast` or forced-colors integration.
- No automatic OS color-scheme detection (light/dark switches via app code, not OS).
- No multiple-variant runtime swap (e.g. `high-contrast` is not a shipped variant).
- No contrast linter or accessibility validation.
- No theme inheritance via subtree override.

The Feathers theme is mostly a vocabulary for the editor's own visuals to be consistent, not a generalised app-theming system. Games building bespoke visuals are expected to fork the pattern (or skip it and theme `bevy_ui_widgets` directly), not adopt Feathers' theme as their own.

## No CSS-flavored stylesheet in bevy_ui core

bevy_ui has no built-in selector-based stylesheet language. There is no `.button { background: red }` analogue, no inheritance cascade, no `:hover` / `:focus` pseudoclass styling rule (state is handled imperatively via `Interaction` reads, see [text-and-input.md](text-and-input.md)).

The notable third-party precedent is **`bevy_flair`** — a crate that adds a CSS-flavoured stylesheet on top of bevy_ui's component model, with selectors, properties, and hot-reload from `.css` files. Buiy's foundation spec marks the question of a Buiy-side stylesheet as open ([README.md § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)): "CSS-flavored stylesheet. Never, or as a future layer above tokens? bevy_flair sets one precedent; the right answer depends on user demand." A dedicated `bevy_flair` prior-art folder is in scope as a separate corpus.

## User-preference support (reduced-motion, forced-colors, prefers-contrast)

bevy_ui as of 0.18.1 / 0.19-rc.1 does **not** honour OS-level UI user preferences automatically:

- **`prefers-reduced-motion`** — not surfaced. UI animations (transitions, the few `TryStableInterpolate` color/`Val` blends added in 0.18) run regardless.
- **`prefers-color-scheme`** — not surfaced. Theme switching is app-driven.
- **`prefers-contrast`** — not surfaced. No automatic high-contrast variant.
- **`forced-colors`** (Windows High Contrast) — not honored. There is no replacement of `BackgroundColor`/`BorderColor` with system colors when forced-colors mode is active.
- **`prefers-reduced-transparency`** — not surfaced. Gradients, low-alpha colors render as authored.
- **`inverted-colors`** — not surfaced.

These are all in the Buiy foundation spec as `F` (foundation) / `C` (core) tier requirements ([accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md), if present), with `UserPreferences` as a `Resource` driving theme-variant binding. The gap in bevy_ui is part of the rationale for Buiy owning a token-driven theme system from the start.

## Summary

bevy_ui's styling story is: decomposed visual components for primitives, no token layer, no stylesheet, no user-preference plumbing. `bevy_feathers` adds a closed-shop token vocabulary for the editor. `bevy_flair` is the precedent for a stylesheet layer. The composition of "tokens + OS-pref-bound variants + hot-reloadable theme assets" that Buiy commits to is not present in any layer of bevy_ui today.

## Sources

- https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/Cargo.toml
- https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src
- https://bevy.org/news/bevy-0-17/
- https://bevy.org/news/bevy-0-18/
- https://bevy.org/learn/migration-guides/0-15-to-0-16/
- https://bevy.org/learn/migration-guides/0-17-to-0-18/
