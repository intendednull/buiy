**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — independent CSS-stylesheet layer on top of bevy_ui; the only published precedent for "CSS in Bevy UI" as of 2026-05-22

# bevy_flair

`bevy_flair` is a third-party crate that loads `.css` files as Bevy assets and applies them to `bevy_ui` entity hierarchies. It is **not** in the Bevy monorepo, it is **not** an official Bevy project, and it ships with one full-time maintainer ([`eckz`](https://github.com/eckz) / Erick Z; 3 GitHub followers, no employer visible, sole `published_by` for every release on crates.io). It is, however, the **only published crate as of 2026-05-22** that gives a Bevy app a working `Styled::new(asset_server.load("menu.css"))` workflow with selectors, cascade, `@media`, `@keyframes`, transitions, `var()`, `calc()`, `@layer`, `@import`, and hot-reload. Every Bevy "should we have CSS?" discussion since early 2025 ends up referencing this crate.

For Buiy this is a **load-bearing precedent for an explicitly open question.** The foundation spec ([README.md § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)) frames it directly:

> **CSS-flavored stylesheet.** Never, or as a future layer above tokens? bevy_flair sets one precedent; the right answer depends on user demand.

This folder documents what that precedent actually is — what CSS subset bevy_flair supports, how the cascade and animation systems work, what the per-frame cost looks like, where the single-maintainer bus factor bites, and where the design genuinely succeeds. It is reference material for the *future* Buiy sub-spec that may exist (`buiy-css-stylesheet-design`), not a recommendation that one should exist.

## Framing disclosure

These docs are written from a **token-based-theming-is-the-primitive, CSS-stylesheet-is-a-question** stance. The Buiy foundation already commits to semantic tokens consumed by components ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)), with `var()`, `calc()`, `min()`, `max()`, `clamp()` as F-tier "Custom properties + value functions" ([visuals.md § 3.3](../../specs/2026-05-07-buiy-foundation/visuals.md)). What is open is whether a separate **stylesheet layer** sits above those tokens (parsed from `.css` files or otherwise) or whether the token system + reflection + BSN is enough. The "Implications for Buiy" framing in [`lessons.md`](lessons.md) reads bevy_flair's design through that lens — it is a learn-from-bevy_flair-into-Buiy artifact, not a neutral catalog. The honest answer to whether Buiy should adopt a stylesheet layer is in [`lessons.md`](lessons.md), and it is genuinely "it depends."

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `bevy_flair` (independent, NOT in `bevyengine/bevy`) |
| Repo | https://github.com/eckz/bevy_flair — 130 stars, 11 forks |
| License | **MIT OR Apache-2.0** (dual; the README header alone says MIT but `Cargo.toml` + crates.io metadata confirm dual) |
| Latest stable | **0.7.0**, published 2026-02-03 |
| Workspace HEAD | 0.8.0 (unreleased) — `Styled` component rename from `NodeStyleSheet` |
| First release | 0.1.0, 2025-01-24 (~16 months of public history) |
| Total downloads | **5,885** (1,336 in the most-recent 90 days; up vs 692 for 0.6 alone Nov 2025) |
| Sole maintainer | [`eckz`](https://github.com/eckz) / Erick Z — sole `published_by` on every release, 3 GitHub followers, no employer listed, no co-maintainers visible |
| MSRV (0.7.0) | rust 1.89.0, edition 2024 |
| Bevy pairing | 0.7 ↔ Bevy 0.18; 0.5–0.6 ↔ Bevy 0.17; 0.2–0.4 ↔ Bevy 0.16; 0.1 ↔ Bevy 0.15 |
| Workspace crates | `bevy_flair_core` (property registry / reflection) · `bevy_flair_style` (cascade engine, animations) · `bevy_flair_css_parser` (cssparser + selectors integration) |
| Top-level plugin | `FlairPlugin` (composes `PropertyRegistryPlugin` + `FlairStylePlugin` + `FlairCssParserPlugin`) |
| CSS parser substrate | [`cssparser`](https://crates.io/crates/cssparser) 0.35 + [`cssparser-color`](https://crates.io/crates/cssparser-color) 0.3 + [`selectors`](https://crates.io/crates/selectors) 0.32 — the **Servo** CSS toolchain, same crates used by browsers built on Servo derivatives |
| Cargo features | `default = []` · `experimental_ghost_nodes` (forwarded to `bevy_flair_style`) |

## Table of contents

- [`architecture.md`](architecture.md) — Three-crate workspace, FlairPlugin shape, the eleven-stage `StyleSystems` pipeline in `PostUpdate`, parser + selector engine choice, cascade implementation.
- [`css-coverage.md`](css-coverage.md) — Which CSS features are in / out: selectors, properties, at-rules, value functions, the explicit non-goals.
- [`api.md`](api.md) — The `Styled` component, `StyleSheet` asset, hot-reload behavior, inline-style support, programmatic-override semantics.
- [`integration.md`](integration.md) — `app.add_plugins(FlairPlugin)`, asset directory layout, coexistence with programmatic `bevy_ui` styling, the bevy_feathers question, per-frame cost.
- [`history.md`](history.md) — 0.1 → 0.7 timeline (Jan 2025 → Feb 2026), Bevy-version pairing, feature additions per release.
- [`governance.md`](governance.md) + distribution — Single-maintainer (eckz), no commercial backing visible, license, Cargo features, bus factor of 1.
- [`critiques.md`](critiques.md) + open problems — Adoption, scope, performance, maintenance, selector subset limits, the cascade-vs-tokens tension, hot-reload reliability, cross-window stylesheets.
- [`ecosystem.md`](ecosystem.md) — Comparisons: vs no-stylesheet (programmatic), vs sickle_ui, vs woodpecker_ui, vs bevy_ui's own future direction; web-CSS-spec alignment.
- [`lessons.md`](lessons.md) — **The consult-this-when-designing decision file.** Validates / Avoid / Borrow + the honest framing of whether Buiy should adopt a stylesheet layer.
- [`glossary.md`](glossary.md) — bevy_flair-specific terms.

## Glossary stub

- **FlairPlugin** — the top-level Bevy plugin a user adds; composes the three sub-plugins.
- **Styled** — the component (formerly `NodeStyleSheet` pre-0.8) that attaches a stylesheet asset to a root entity; styling propagates down through `UiChildren`.
- **StyleSheet** — the Bevy `Asset` type produced by parsing a `.css` file via the `FlairCssParserPlugin` asset loader.
- **PropertyRegistry / CssPropertyRegistry** — the reflection-based registries that map CSS property names (`background-color`) onto Bevy component fields (`BackgroundColor.0`).
- **`-bevy-*` properties** — non-standard CSS properties (vendor-prefixed) for Bevy-specific things like `-bevy-image-mode`, `-bevy-image-rect`.

See [`glossary.md`](glossary.md) for the full list.

## Recommended reading order

1. [`architecture.md`](architecture.md) — what the plugin actually does each frame.
2. [`css-coverage.md`](css-coverage.md) — what subset of CSS is real, what isn't.
3. [`integration.md`](integration.md) — what the user-side workflow looks like.
4. [`critiques.md`](critiques.md) — what doesn't work yet / never will.
5. [`lessons.md`](lessons.md) — the synthesis. Read last.

## Sources

- bevy_flair repository — https://github.com/eckz/bevy_flair
- bevy_flair on crates.io — https://crates.io/crates/bevy_flair
- bevy_flair CHANGELOG — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture § 2.5 — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling prior-art: [`bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`bevy-ui/styling.md`](../bevy-ui/styling.md)
