**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — release history (0.1 → 0.7, Jan 2025 → Feb 2026) and Bevy version pairing

# History

Sixteen months, ten published releases, one maintainer. Verified against crates.io publication timestamps and the project's `CHANGELOG.md`.

## Release timeline

| Version | Published | Bevy target | Headline additions |
|---|---|---|---|
| **0.1.0** | 2025-01-24 | Bevy 0.15 | Initial release. Stylesheet loading, basic selectors, animations / keyframes, custom properties. ~263 lines of code at this point (38 Rust, 225 CSS in examples). |
| **0.2.0** | 2025-04-30 | Bevy 0.16 | `@import`, nested selectors, `:not()` / `:has()` / `:is()` / `:where()`, `var()`, basic `calc()`, shorthand props (border, flex), inherited properties. The first release with the cascade engine close to "real CSS." `:nth-child` recalculation fix on sibling add. |
| **0.3.0** | 2025-05-16 | Bevy 0.16 | `@media` queries (multi-prop), grid + gap + text-shadow + aspect-ratio + line-height properties, transition events, initial-value support, Oklab interpolation, `-bevy-` vendor-prefix convention for non-standard properties. All properties support `inherit`. Internal: depend on individual `bevy_*` crates instead of the umbrella `bevy` crate. |
| **0.4.0** | 2025-08-01 | Bevy 0.16 | `@layer` cascade layers, `!important` token detection (still ignored), attribute selectors, `::before` / `::after` pseudo-elements, WASM compatibility via updated `selectors` crate, `TypeName` component replaces `TrackTypeNameComponentPlugin`. |
| **0.4.1** | 2025-09-15 | Bevy 0.16 | Custom-type-parsing example, public parsing functions for app-side extension. |
| **0.5.0** | 2025-10-03 | **Bevy 0.17** | Gradient support (linear, radial, conic), transform shorthands (`translate`, `scale`, `rotate`), custom `Time` support for animations, `InlineCssStyleSheetParser` for string-based stylesheets, animations default to `Time<Real>`. `StyleSystemSets` → `StyleSystems` rename. (0.5.0-rc.1 was 2025-09-22.) |
| **0.5.1** | 2025-10-11 | Bevy 0.17 | `AnimationEvent` support. |
| **0.6.0** | 2025-11-05 | Bevy 0.17 | `-bevy-image-rect` (9-slice insets), `GhostNode` support in styled hierarchies, `ComponentProperty` restructure for custom-property ergonomics, `UiChildren` / `UiRootNodes` iteration replaces `Children`, auto-removal of components without defined properties. Multiple-animations-per-property fix, `:hover`-during-animation restart fix, `@keyframes var()` error reporting. Font-faces with import statements fixed. |
| **0.7.0** | **2026-02-03** | **Bevy 0.18** | Headline release. Animation/transition system overhauled with individual-property support. `var()` works in animation/transition properties. `TextureSlicer` in `NodeImage`, `BoxShadow` + `TextShadow` interpolation, `unset` value for any CSS property. Individual border props (`border-left`, `border-right`, etc.). `SmolStr` → `Cow<'static, str>` internally. `RawInlineStyle` made immutable. Dynamic resolution of images and fonts in inline styles. |
| **0.8.0** | Unreleased (HEAD as of 2026-05-22) | Bevy 0.18 (presumed) | `NodeStyleSheet` → `Styled` component rename. Styling extended to non-`Node` entities (Text-based hierarchies). Internal `Node*` → `Style*` rename consistency. `Siblings` component removed. |

## Bevy version pairing

The pairing is strict — bevy_flair tracks Bevy minor releases as breaking events, exactly like the Buiy foundation commits to ([README.md goal 5](../../specs/2026-05-07-buiy-foundation/README.md) "Tracks Bevy. Rolling latest-stable.").

| Bevy version | Compatible bevy_flair versions |
|---|---|
| 0.18 | 0.7 (and HEAD 0.8) |
| 0.17 | 0.5.0, 0.5.1, 0.6.0 |
| 0.16 | 0.2.0, 0.3.0, 0.4.0, 0.4.1 |
| 0.15 | 0.1.0 |

Note: bevy_flair has **not** yet shipped a release against Bevy `main` / 0.19-dev. When Bevy 0.19 lands with the `parley` + `swash` migration (cf [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § "post-0.19 text-shaper divergence"), bevy_flair will need to absorb the text-API surface change. Whether eckz keeps cadence under that load is one of the open questions ([`critiques.md`](critiques.md) + [`governance.md`](governance.md)).

## Cadence

- Mean inter-release: ~5 weeks (Jan 2025 → Feb 2026, 9 published versions across ~54 weeks).
- Two long gaps: 0.4.1 → 0.5.0 = 3 weeks (with rc.1); 0.6 → 0.7 = ~13 weeks (the longest, spanning the December 2025 → early-Feb 2026 window — likely a holiday + Bevy 0.18 migration combination).
- Recent-90-day downloads (1,336 of 5,885 total = 23%) suggest accelerating adoption post-0.7, but absolute numbers remain small.

## Feature growth trajectory

Code-size growth from 0.1 → 0.7 (per crates.io `linecounts`, total code lines for the meta-crate):

| Version | Total LOC (meta-crate only) | CSS example LOC | Rust LOC |
|---|---|---|---|
| 0.1 | 263 | 225 | 38 |
| 0.2 | 721 | 614 | 107 |
| 0.3 | 730 | 622 | 108 |
| 0.4 | 776 | 683 | 93 |
| 0.5 | 874 | 786 | 88 |
| 0.6 | 874 | 786 | 88 |
| 0.7 | 1010 | 922 | 88 |

The Rust LOC plateau at 88 reflects the meta-crate's role as a re-export shell; the real growth is in the three workspace crates, not visible from crates.io aggregates. A direct `cargo loc` of the workspace as of 0.7 would be more informative — left as a gap.

## Pre-amble corrections applied

The pre-amble for this folder stated:
- ✓ Latest stable 0.7.0 (2026-02-03) — verified.
- ✓ License MIT OR Apache-2.0 — verified.
- ✓ Total downloads 5,885 — verified.
- ✗ The pre-amble didn't capture the **0.8.0-unreleased HEAD** with the `NodeStyleSheet` → `Styled` rename. This is a material change to user-facing API; the README in this folder uses `Styled` (the HEAD name) with a footnote about the rename, since 0.8 is imminent and `Styled` is the long-term name.
- ✗ The pre-amble listed Cargo features as unverified. Actual: `default = []`, `experimental_ghost_nodes` (forwarded to `bevy_flair_style`).
- ✗ GitHub repo metrics not in pre-amble: 130 stars, 11 forks, 3-follower maintainer.

## Sources

- bevy_flair CHANGELOG.md — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- crates.io `/api/v1/crates/bevy_flair` version listing — https://crates.io/crates/bevy_flair
- GitHub repo — https://github.com/eckz/bevy_flair
- Buiy Bevy-tracking policy — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.9
- Bevy text-shaper migration (parley + swash) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Top of file"
