**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — production usage, showcase community, adjacent crates, comparative landscape

# Ecosystem

bevy_lunex is the third-party choice for developers who specifically want **worldspace UI** — UI that lives in the 3D scene, anchored to entities, animated like game objects. Outside that niche its adoption is modest. The headline numbers — 913 GitHub stars, 40,504 total crates.io downloads, 2,126 recent — sound substantial in isolation but should be read against the Bevy UI landscape's two-tier dominance: `bevy_egui` (immediate-mode) and `bevy_ui` (official retained) together represent roughly 98% of downloads in the Bevy UI category.

## Adoption math (the honest framing)

| Crate | Total downloads | Recent | Stars |
|---|---|---|---|
| `bevy_ui` | 4,901,387 | 943,255 | (in Bevy monorepo) |
| `bevy_egui` | ~3M+ (per [thisweekinbevy.com] mentions) | — | 1.6k+ |
| `bevy_lunex` | **40,504** | **2,126** | 913 |
| `sickle_ui` | ~30K (lib.rs) | — | ~400 |
| `woodpecker_ui` | ~15K (lib.rs) | — | ~250 |
| `bevy_feathers` (in-tree, experimental) | 191,700 (since 2025-09) | — | — |

bevy_lunex sits comfortably in the second tier of third-party UI options — ahead of `sickle_ui` and `woodpecker_ui` on raw downloads, behind `bevy_egui` by ~75×, behind `bevy_ui` by ~120×. The "40K downloads sounds significant but is small relative to bevy_ui's 4.9M" reading from the orchestrator brief is accurate.

Stars-to-downloads ratio (913 / 40,504 ≈ 22.5) is **high** — meaning the project is starred by far more people than actually use it. This is consistent with "interesting design, niche application" — developers admire the worldspace-UI idea but few have a project that needs it.

## Production usage (verified)

There is **no commercially shipped game** built on bevy_lunex that has been publicly named in 2025 or 2026. The closest references:

- **Bevypunk** (`https://github.com/IDEDARY/Bevypunk`) — the flagship demo, built by the bevy_lunex maintainer himself. A WASM build is on itch.io (`https://idedary.itch.io/bevypunk`). The repo's own framing describes it as a *"production ready example"* not a shipped product. As of 2026-01-06 the demo was updated. 218 GitHub stars.
- **bevy-codex** (crates.io reverse-dep) — a HUD/menu manager crate, ~v0.6.2, depends on `bevy_lunex ^0.2.3`. Effectively unmaintained against current Bevy.
- **bevy_kot** / **bevy_kot_ui** (crates.io reverse-deps) — a Bevy toolkit, v0.11.0, depends on `bevy_lunex ^0.0.9` as a dev-dependency. Pre-rewrite; effectively defunct against current bevy_lunex.

Only three reverse-deps total on crates.io, two of them stale. This is a meaningful adoption-ceiling signal: bevy_lunex is used directly in applications rather than as a foundation for derived libraries. There is no widget-kit-on-bevy_lunex ecosystem comparable to `sickle_ui` on `bevy_ui`.

GitHub search across the `bevy-lunex` topic surfaces tutorials, ports, and learning projects rather than shipped games. The Bevy Discord and "This Week in Bevy" mentions are sporadic — bevy_lunex shows up in showcase posts a few times per year, typically associated with Bevypunk-style aesthetic experiments.

**The "no flagship game" question is real.** No Steam release, no major itch.io publication, no recognizable studio has publicly identified bevy_lunex as their UI choice. See [`critiques.md`](critiques.md) § "The no-flagship-game question."

## The bevy_lunex showcase community

The project has a small but visible **showcase community** organized around the aesthetic ambitions of Bevypunk:

- **HUDs and game menus** that look like Cyberpunk 2077 / Deus Ex panel UIs — animated, with glow effects, scanline filters, holographic projection feel.
- **Worldspace UI experiments** — UI billboards anchored to NPCs, in-world arcade machines with interactive screens, AR/VR-style HUD elements that need true 3D placement.
- **Custom shader-rendered UI panels** — bevy_lunex's render-through-`bevy_sprite` route means custom materials Just Work, which `bevy_ui` notoriously struggles with.

The community is centered on the Bevy Discord (#showcase and #ui channels) and the bevy_lunex GitHub Discussions tab. There is no separate Discord, no separate forum, no separate subreddit. The maintainer's IDEDARY profile is the de-facto curator.

## Adjacent crates

bevy_lunex is **orthogonal** to the official Bevy UI stack — it does not extend `bevy_ui`, does not consume `bevy_ui_widgets` or `bevy_feathers`, and is not consumed by them. The coexistence model is "use them for different windows / different scenes." See [`comparisons.md`](comparisons.md) for the row-by-row breakdown.

| Crate | Relationship to bevy_lunex | Notes |
|---|---|---|
| `bevy_ui` | Orthogonal | Parallel UI stack. Both can coexist in one app on different windows; same-tree mixing is not supported. |
| `bevy_egui` | Orthogonal | Different paradigm (immediate-mode). Same coexistence story — debug overlays in egui, game UI in lunex, no shared focus or hit-testing. |
| `sickle_ui` | Orthogonal | `sickle_ui` extends `bevy_ui`; bevy_lunex replaces it. No direct interop. |
| `woodpecker_ui` | Orthogonal | Both parallel-to-bevy_ui but use different runtimes; no interop. |
| `bevy_feathers` | Orthogonal | `bevy_feathers` is the official widget kit on `bevy_ui`; bevy_lunex addresses a different (worldspace) problem. No interop. |
| `bevy_picking` | **Consumed by bevy_lunex.** | bevy_lunex 0.3+ uses `bevy_picking` for hit-testing. This is the single shared subsystem with the rest of the Bevy ecosystem. |
| `bevy_a11y` / AccessKit | **Not consumed.** | bevy_lunex has no accessibility integration. See [`critiques.md`](critiques.md) and [`open-problems.md`](open-problems.md). |
| `cosmic-text` | Consumed | bevy_lunex 0.5+ uses `cosmic-text` 0.16 directly for shaping, parallel to `bevy_ui`'s use of the same. Different render paths, same shaping engine. |
| `bevy_rich_text3d` | Consumed (optional, `text3d` feature) | The 3D-worldspace text rendering. Same-author crate (IDEDARY ecosystem). |
| `blueprint` (bytestring-net) | Built on top of bevy_lunex | Aspirational "ECS UI framework for applications" by the same author; very early (7 stars). |

The picking integration (0.3+) is the **one** subsystem bevy_lunex shares with both `bevy_ui` and the broader Bevy ecosystem. Everything else is independent.

## Comparative landscape (Bevy UI options, 2026)

For positioning Buiy in this landscape:

| Option | Paradigm | Built on | Worldspace UI? | Status |
|---|---|---|---|---|
| **`bevy_ui`** | Retained, screen-space, Taffy layout | Bevy core | No (screen-space only) | Official, stable |
| **`bevy_ui_widgets` + `bevy_feathers`** | Headless primitives + opinionated widget kit | `bevy_ui` | No | Official, experimental |
| **`sickle_ui`** | Themed widgets, builder API | `bevy_ui` | No | Third-party, active |
| **`bevy_egui`** | Immediate-mode | Own render path | No (screen-space only) | Third-party, mature, dominant |
| **`bevy_lunex`** | Retained, `Transform`-based, anchored layout | Own runtime + `bevy_picking` | **Yes (first-class)** | Third-party, niche |
| **`woodpecker_ui`** | Reactive, declarative DSL, vello renderer | Own runtime | No | Third-party, active |
| **`kayak_ui`** | Retained, custom DSL | Own runtime | No | **Archived 2024** |
| **Buiy** (this project) | Retained, parallel-stack, web-platform-parity | Taffy + cosmic-text + AccessKit + bevy_picking + Bevy render graph directly | Yes (planned per architecture) | Pre-foundation (spec phase) |

The Buiy positioning relative to bevy_lunex:

- **Same parallel-stack stance.** Both bypass `bevy_ui` entirely.
- **Same bevy_picking choice.** Both use the official picking system (bevy_lunex since 0.3, Buiy planned).
- **Same worldspace-UI ambition.** Both treat UI nodes as `Transform`-positionable; Buiy keeps general `Transform` per [cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md), aligning with bevy_lunex's choice.
- **Different layout engine.** bevy_lunex: anchored/percent positioning, no flexbox/grid. Buiy: full Taffy (flexbox + grid + block).
- **Different accessibility stance.** bevy_lunex: none. Buiy: AccessKit-first from day one per [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md).
- **Different feature scope.** bevy_lunex: layout + picking + worldspace; no widgets, no theming, no animation primitives. Buiy: full web-platform-parity catalog per [media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md).

## Game studios / indie devs known to use bevy_lunex

**None publicly named** as of 2026-05-22. Searches across:

- Hacker News (no substantive discussion threads found).
- This Week in Bevy newsletter (occasional mentions, no studio-naming).
- Bevy showcase channels (Bevypunk only).
- Crates.io reverse-deps (the three listed above; all unmaintained or pre-rewrite).

This is the empirical answer to "who ships bevy_lunex?" — at present, no one with a visible commercial presence. The flagship is Bevypunk, which is a tech demo by the maintainer.

## Sources

- bevy_lunex crates.io reverse-deps — `https://crates.io/crates/bevy_lunex/reverse_dependencies`.
- Bevypunk repo + itch.io — `https://github.com/IDEDARY/Bevypunk`, `https://idedary.itch.io/bevypunk`.
- GitHub topic page — `https://github.com/topics/bevy-lunex`.
- This Week in Bevy archive — `https://thisweekinbevy.com/`.
- bevy_ui downloads — `https://crates.io/api/v1/crates/bevy_ui` (fetched 2026-05-22).
- bevy_lunex downloads + versions — `https://crates.io/api/v1/crates/bevy_lunex` (fetched 2026-05-22).
- "How do Nice UI in Bevy?!?" (Dead Money, ecosystem overview) — `https://deadmoney.gg/news/articles/how-do-nice-ui-in-bevy`.
- "A vision for Bevy UI" — `https://hackmd.io/@bevy/HkjcMkJFC`.
- kayak_ui (archived precedent) — `https://github.com/StarArawn/kayak_ui`.
