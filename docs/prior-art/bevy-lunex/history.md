**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — project genesis, version-by-version timeline, Bevy coupling

# History

bevy_lunex is the longest-running third-party parallel-UI stack for Bevy. It predates `sickle_ui`, `woodpecker_ui`, and the Bevy Foundation's own headless-widgets work (`bevy_ui_widgets` / `bevy_feathers`) by roughly a year. Its design bet — `Transform`-based UI living in the same coordinate space as game entities — was made before Bevy itself had a coherent UI story and has remained the project's distinctive identity through every rewrite.

## Genesis (2023)

- **First crates.io publish: 0.0.1 on 2023-08-24.** This is the project's externally-visible birthday. Commits predate it (the workspace exists earlier on GitHub), but the public release window opened in late August 2023.
- **Author:** IDEDARY (Czech Republic), publishing under the `bytestring-net` organization. The bio on the project's documentation describes the maintainer as a "university student" with the explicit warning *"don't expect updates during the semester."* See [`governance.md`](governance.md).
- **Initial framing.** From the earliest README onward, the pitch is *"make your own custom UI using regular ECS like every other part of your app"* — not "wrap egui," not "wrap `bevy_ui`," but a parallel system using only Bevy's `Transform` and ECS primitives. This is the **transform-based bet** ([`comparisons.md`](comparisons.md) § "vs bevy_ui").

The "why parallel to bevy_ui" decision is implicit rather than stated. At project start (Aug 2023), `bevy_ui` was already 2 years old but widely considered immature: layout via Taffy was incomplete, text rendering was glyph-brush (later swapped for cosmic-text), and worldspace UI (UI anchored to entities in 3D space) was outright impossible because `bevy_ui` is screen-space-only. bevy_lunex's worldspace-UI capability — possible because UI nodes are just `Transform` entities — was the differentiating feature that justified the parallel-stack decision.

## 0.0.x: experimental phase (Aug 2023 – Jan 2024)

- **0.0.1 – 0.0.6** (Aug 27 – Sep 15, 2023): rapid iteration on Bevy 0.11. Three releases inside a week (0.0.2, 0.0.3, 0.0.4), suggesting hotfix cycles on early API shape.
- **0.0.7 – 0.0.9** (Nov 14 – Nov 20, 2023): tracking Bevy 0.12 with bumps and bugfixes.
- **0.0.10 / 0.0.11** (Jan 5, 2024): same-day patch pair on Bevy 0.12.

This period established the **path-based UI tree abstraction**: UI nodes addressed by string paths through a `UiTree` resource (`"root/menu/button_play"`). This abstraction survived through 0.2 and was largely replaced in the 0.3 rewrite — see below.

## 0.1.x: first stable identity (May – Jun 2024)

- **0.1.0-alpha** (2024-05-11) — first stable-track release on Bevy 0.13.
- **0.1.0-alpha.2** (2024-06-04).
- **0.1.0** (2024-06-16) — the first non-alpha stable.
- **0.1.1** (2024-07-04, **yanked**) — same-day yank for unspecified breakage.

The 0.1.x line marked bevy_lunex's transition from "experimental" to "use this for your game" framing. Bevypunk — IDEDARY's own Cyberpunk-styled production-example UI demo — started taking shape during this period and remains the project's flagship reference application (see [`ecosystem.md`](ecosystem.md)).

## 0.2.x: Bevy 0.14 stabilization (Jul – Sep 2024)

- **0.2.0** (2024-07-04): Bevy 0.14 bump; added 2D mesh support and a Kira audio integration feature.
- **0.2.1** (2024-07-09): added `UiTextSize` component decoupling font size from layout size to fix text blurriness at scale.
- **0.2.2** (2024-07-14): gamepad cursor support via `GamepadCursor` marker component — early hint at navigation/input ambitions.
- **0.2.3** (2024-07-22): cursor picking improvements; added `UiTree::new3d` API — the worldspace-3D bet getting first-party API support.
- **0.2.4** (2024-09-21): renamed `UiPlugin` → `UiDefaultPlugins`; improved cursor bundles; documentation pass.

The 0.2 line is where bevy_lunex's identity solidified: path-based `UiTree`, `UiNode` components, anchored layout, gamepad cursor, 3D worldspace as a first-class mode. This is also where the "no flexbox" stance became permanent — the maintainer chose to keep the anchored layout system rather than integrate Taffy, and that decision sticks through every subsequent rewrite. (See Agent A's [`layout.md`](layout.md) for the layout model.)

## 0.3.x: the rewrite (Feb – Mar 2025)

**0.3.0 (2025-02-28) is the project's largest break.** The release notes describe it as a *"complete project rewrite"* aligned with Bevy 0.15. The key migrations:

- Adoption of Bevy's **required-components** pattern (introduced in Bevy 0.15), replacing the older bundle-based composition.
- Migration to **observers** for event handling (a Bevy 0.14/0.15 ECS feature), replacing the older Run-systems-on-events pattern.
- **State machines overhaul** for interactive components.
- **100% inline-docs coverage** declared.
- **Lunex picking backend** migrated to integrate with `bevy_picking` (the unified Bevy picking system that landed in core in 0.15).

This is the rewrite that committed bevy_lunex to Bevy's modern ECS idioms. It also abandoned much of the path-based `UiTree` API in favor of native Bevy hierarchy. The architectural through-line — `Transform`-based, parallel-to-bevy_ui — survived; everything else was up for renegotiation.

- **0.3.1** (2025-03-02): hotfix batch for 0.3.0 edge cases.
- **0.3.2** (2025-03-10): **first-party text 3D support** via `bevy_rich_text3d` integration; viewport scaling fixes.

The 3D text addition at 0.3.2 is the inflection point for the **3D-anchored UI feature evolution**: text in worldspace was the last missing piece to make 3D UI a fully first-class mode. From this point forward, the maintainer's framing of bevy_lunex shifts toward "worldspace UI is the differentiator" — visible in commits, book chapters, and the Bevypunk demo's hologram/HUD use cases.

## 0.4.x: Bevy 0.16 catch-up (Apr – Jun 2025)

- **0.4.0** (2025-04-25): Bevy 0.16 bump.
- **0.4.1** (2025-04-25): same-day patch with fixes from an outside contributor — the first sign of community-PR activity beyond IDEDARY.
- **0.4.2** (2025-06-14): wildcard-import fix, custom-mesh-for-UI-node example, miscellaneous polish.

0.4 is structurally a Bevy-bump release rather than a feature release; the post-0.3-rewrite codebase mostly held shape across the Bevy 0.15 → 0.16 transition.

## 0.5.0: contributor-driven Bevy 0.17 bump (Oct 20, 2025)

**0.5.0 was bumped to Bevy 0.17 by an outside contributor (S4ndf1re, PR #122).** This is the first bevy_lunex minor release where the Bevy version bump was not done by IDEDARY. The 4-month gap between 0.4.2 (Jun 14) and 0.5.0 (Oct 20) lines up with the maintainer's "university semester" note — and the bump only happened because a community contributor stepped up.

This is the **bus-factor signal** the project has openly carried throughout its life. See [`governance.md`](governance.md).

Around this release the codebase was also restructured into separate workspace crates (commit "migrated to separate crates", 2025-05-17), splitting the monolithic `bevy_lunex` into a workspace with `crate/` + `examples/*` members.

## 0.6.0: Bevy 0.18 (Jan 22, 2026)

- **0.6.0** (2026-01-22): Bevy 0.18 bump, presumably done by IDEDARY (commit message simply: "Updated to 0.18").
- Subsequent (Feb 24, 2026): "Add HUD example and Fix tracing ANSCII" — the only post-release commit on `main` as of 2026-05-22.

0.6 introduced no documented headline features — it is a Bevy-tracking release. Edition was bumped to **edition = "2024"** in the workspace manifest, requiring Rust 1.85+. The `text3d` feature became default-on.

The project has been **quiet since February 2026** (three months as of this writing). One Bevy 0.18 patch (0.18.1) has shipped upstream without a bevy_lunex equivalent.

## The transform-based bet: when committed, why

The decision to make UI nodes be `Transform`-positioned entities — rather than wrapping a layout engine (Taffy) and projecting to screen-space — was made **at project inception (Aug 2023)** and has never been reconsidered. The rationale, never spelled out explicitly but inferable from features and book chapters:

- **Worldspace UI without special-casing.** Because UI nodes are normal `Transform` entities, they can be parented to 3D meshes, animated, rotated, projected onto curved surfaces — anything you can do to a game entity. `bevy_ui` cannot do this without a separate render path; `sickle_ui` and `bevy_feathers` inherit this limitation.
- **No layout-engine porting tax.** Anchored layout (top/left/right/bottom percentages, plus pixel offsets) is mechanically simple to implement on top of `Transform`. The maintainer chose this over integrating Taffy and inherited none of Taffy's bug surface — but also gave up flexbox/grid (see [`critiques.md`](critiques.md)).
- **One coordinate system.** UI hit-testing, animation, and audio positioning all use the same `Transform` math. This is cited in book sections about interactivity.

The cost: bevy_lunex is structurally **not the right tool for desktop application UIs** (the docs say so explicitly), because anchored positioning is awkward for forms, panels, and densely-packed widgets where flexbox/grid is the natural choice.

## Bevy version coupling per minor

| bevy_lunex minor | Bevy minor | Release date |
|---|---|---|
| 0.0.1 – 0.0.6 | 0.11 | 2023-08 – 2023-09 |
| 0.0.7 – 0.0.11 | 0.12 | 2023-11 – 2024-01 |
| 0.1.x | 0.13 | 2024-05 – 2024-07 |
| 0.2.x | 0.14 | 2024-07 – 2024-09 |
| 0.3.x | 0.15 | 2025-02 – 2025-03 |
| 0.4.x | 0.16 | 2025-04 – 2025-06 |
| 0.5.x | 0.17 | 2025-10 |
| 0.6.x | 0.18 | 2026-01 – present |

One bevy_lunex minor per Bevy minor, no overlap, no LTS. See [`distribution.md`](distribution.md) for the migration-tax implications.

## Sources

- bevy_lunex releases — `https://github.com/bytestring-net/bevy-lunex/releases`.
- bevy_lunex crates.io versions — `https://crates.io/crates/bevy_lunex/versions` (fetched 2026-05-22).
- bevy_lunex commit history (`main`) — `https://github.com/bytestring-net/bevy-lunex/commits/main`.
- Bevy Lunex book — `https://bytestring-net.github.io/bevy_lunex/`.
- IDEDARY profile — `https://github.com/IDEDARY`.
- Bevypunk repo — `https://github.com/IDEDARY/Bevypunk`.
- PR #122 (Bevy 0.17 bump by S4ndf1re) — `https://github.com/bytestring-net/bevy-lunex/pull/122`.
- Bevy 0.15 release (required-components, observers) — `https://bevy.org/news/bevy-0-15/`.
