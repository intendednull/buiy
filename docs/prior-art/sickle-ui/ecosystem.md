**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — ecosystem: production usage, comparisons against the post-0.15 alternatives

# Ecosystem & Comparisons

## Production usage

**Sparse and trailing.** The verifiable signals:

- **15,120 total crates.io downloads** across all sickle_ui versions (0.2.1 → 0.4.0). Modest by Bevy-crate standards; `bevy_egui` is several orders of magnitude higher.
- **517 recent (90-day) downloads.** Still nonzero post-archive, indicating apps on Bevy 0.14 continue to pull sickle from caches and CI runs.
- **Zero reverse dependencies on crates.io that we could verify** for sickle_ui specifically. (The reverse-deps endpoint returned an unauthenticated page; we could not enumerate them via the API tools available in this session. The most likely figure is in the low single digits, given the download rate.)
- **No flagship-game / flagship-app deployment surface.** No public game or app trade-press release credits "built with sickle_ui." Compare bevy_egui (deployed in multiple commercial Bevy games) and `bevy_lunex` (deployed in Foresight Spar's UI per the Lunex README).
- **No tutorial / blog ecosystem.** A search for `"sickle_ui"` tutorial content returned the project's own README and crates.io page as the dominant hits; no third-party walkthroughs, no YouTube tutorial series, no conference talk transcripts.

The realistic usage profile: **small-to-medium hobbyist Bevy editor experiments on Bevy 0.14**, with a long-tail of apps that never migrated forward. The crate is functional within its Bevy version pin; its absence from the post-0.15 ecosystem means new development is not happening on it.

The `simple_editor` example in the (former) upstream repository is the canonical worked example. It is also the only worked example. With the upstream repo gone, the `UkoeHB/sickle_ui` fork preserves it.

## Comparisons

The 2026 Bevy UI landscape has several mature options; sickle_ui occupies a specific historical niche. The comparison below names the post-sickle alternatives Buiy treats as live, and locates each one against sickle.

### vs `bevy_feathers` — the official successor in scope

`bevy_feathers` is a widget kit on top of `bevy_ui_widgets` on top of `bevy_ui`, maintained in-tree at `crates/bevy_feathers/` since Bevy 0.17 (PR #19730, merged 2025-06-28). It is explicitly scoped to `"a collection of UI widgets for building editors and utilities in Bevy"` — the same scope claim sickle made.

| Dimension | sickle_ui 0.4.0 | bevy_feathers (Bevy 0.18 era) |
|---|---|---|
| Maintained | No (obsolete) | Yes (in-tree, ickshonpe + viridia) |
| Bevy version | 0.14 only | tracks engine |
| Widget catalog scale | ~30 widgets | ~12 controls + containers + display |
| Headless / styled split | Conflated (one kit) | Clean (`bevy_ui_widgets` headless + feathers styled) |
| Theming model | Theme<C> + PseudoTheme + DynamicStyle | `ThemeBackgroundColor` / `ThemeTextColor` token-rewriting observers |
| AccessKit | None | Partial via `bevy_a11y` |
| Focus model | None | Integrates `bevy_input_focus` (Tab nav) |
| BSN-compat | Hostile (extension-trait DSL) | Hostile (megacomponent `AccessibilityNode` issue, see [`../bevy-feathers/critiques.md`](../bevy-feathers/critiques.md)) |
| Editor-docking primitives | Yes (docking_zone / floating_panel / sized_zone) | No |
| Animation | DynamicStyle's interactive + animated attributes | None |

**Verdict:** `bevy_feathers` is the official answer for the niche sickle occupied. sickle still has a *more interesting* set of editor-docking widgets, but those are not enough to overcome the unmaintained-on-Bevy-0.14 disadvantage.

**The bevy_feathers question — does sickle still have a use case post-Bevy 0.17?** No, for new development. The only reason to use sickle in 2026 is if you have a pre-existing Bevy 0.14 app that is not migrating. Even then, vendoring sickle's source (and accepting the maintenance burden) is the realistic plan; depending on the crate via `cargo` ties you to the unmaintained upstream.

### vs `bevy_ui_widgets` — headless primitives

`bevy_ui_widgets` is the engine-in-tree headless-widget-primitive crate (Bevy 0.17+). It ships `Slider`, `Checkbox`, `MenuItem`, `Button` (markers + event types) with no rendering — apps add their own visual layer. See [`../bevy-ui-widgets/`](../bevy-ui-widgets/).

Sickle conflated headless behavior with styled presentation; `bevy_ui_widgets` is the canonical separation. The lesson is the same as the WAI-ARIA "headless" library pattern (`react-aria`, `radix-ui` primitives) and validates the design choice both `bevy_ui_widgets` and Buiy's `buiy_widgets_core` make.

**Verdict:** `bevy_ui_widgets` represents the structural improvement over sickle — same widgets, cleaner architecture, in-tree maintenance. Buiy's `buiy_widgets` should be the headless layer and `buiy_widgets_theme_default` (or equivalent) the styled layer.

### vs `bevy_lunex` — parallel-stack alternative

`bevy_lunex` is a Bevy UI library that runs **parallel to `bevy_ui`** rather than on top of it. It owns its own layout (CSS-like, not Taffy-based), its own rendering surface, its own component model. See [`../bevy-lunex/`](../bevy-lunex/) if it has been documented; otherwise the upstream README is the entry point.

| Dimension | sickle_ui | bevy_lunex |
|---|---|---|
| Architecture | extends bevy_ui | parallel to bevy_ui |
| Layout engine | Bevy/Taffy via Node | Custom (Lunex-owned) |
| Render pipeline | Bevy's bevy_ui pipeline | Own pipeline |
| Maintenance status (2026-05) | Obsolete | Active |
| Bevy version | 0.14 only | tracks engine |
| Widget catalog | Rich (editor focus) | Sparser (game focus) |
| 3D-anchored UI | No | Yes (the headline lunex feature) |
| BSN-compat | Hostile | Mixed — has worked toward bsn-friendliness |

**Verdict:** they sit on opposite sides of the extends-vs-parallel decision. Buiy chose parallel (foundation [README § 1.4](../../specs/2026-05-07-buiy-foundation/README.md)) for the same reasons lunex did: bevy_ui's renderer caps and component-model churn are too tight a coupling for a comprehensive UI library. sickle is the cautionary example of the extends-bevy_ui side.

### vs `woodpecker_ui` — different paradigm

`woodpecker_ui` (formerly part of `kayak_ui`) is a React-style declarative widget library that takes a different paradigm — VDOM-like reconciliation against an entity hierarchy. Different architectural premise from both sickle and Buiy. It still exists but is also slow-moving and Bevy-version-coupled.

**Verdict:** orthogonal. The lesson from woodpecker is "VDOM-on-ECS is a real shape," but it doesn't change Buiy's analysis of sickle.

### vs `bevy_egui` — different paradigm (immediate-mode)

`bevy_egui` is an integration layer for the `egui` immediate-mode UI library. Different paradigm entirely (immediate-mode vs retained-mode). It does not target the same niche — `bevy_egui` is for tooling overlays, debug UI, prototyping; not for production app UI or game HUDs at production polish.

| Dimension | sickle_ui | bevy_egui |
|---|---|---|
| Mode | Retained (entities) | Immediate (per-frame redraw) |
| Theming | Programmatic (Theme<C>) | egui's built-in styling |
| Performance ceiling | Limited by bevy_ui | Limited by egui's geometry budget |
| Best fit | Editor widgets in Bevy | Dev tools, prototypes, internal panels |
| Maintenance status | Obsolete | Active (sustained) |

**Verdict:** they barely overlap. An app using sickle for production widgets would still use egui for debug overlays. The comparison exists only because both ship in the "Bevy UI library" category on lib.rs.

### vs `bevy_cobweb_ui` — the spiritual successor that also archived

`bevy_cobweb_ui` (UkoeHB) is the project where sickle's scaffold layer landed (as `cob_sickle_math` / `cob_sickle_macros` / `cob_sickle_ui_scaffold` at v0.8.0). Cobweb adds the **COB asset format** — a custom declarative scene format that solves part of what BSN would solve for Bevy core. It ran on Bevy 0.17, and was actively developed through 2025.

**Cobweb itself is archived as of 2026-01-13**, which is a separate story but reinforces the broader pattern: the design-DNA of sickle (theming + dynamic-style + flux-interaction) survived two projects without finding a maintenance home that outlasted a Bevy minor-release cycle.

| Dimension | sickle_ui | bevy_cobweb_ui |
|---|---|---|
| Sickle DNA carried forward | — | Yes (scaffold layer) |
| New layer | — | COB asset format |
| Bevy version | 0.14 | 0.17 |
| Maintenance status | Obsolete (2024-10) | Archived (2026-01) |

**Verdict:** cobweb is the most-direct lineage continuation of sickle. Its archival in early 2026 closes the family. Buiy is not a lineage continuation — Buiy is a clean-slate parallel stack — but the cobweb post-mortem is instructive for the same "Bevy minor releases as breaking-migration events" problem.

### vs Buiy — the design-target

| Dimension | sickle_ui 0.4.0 | Buiy (target shape) |
|---|---|---|
| Position vs bevy_ui | Extends | **Parallel** |
| Authoring model | Extension-trait DSL (Rust-only) | **Components-first + BSN-friendly** (when BSN lands) |
| Theming | Runtime closures | **Theme assets (declarative, hot-reloadable)** |
| AccessKit | None | **AccessKit-first, decomposed a11y components from day 1** |
| Focus model | Pointer-only | **Single focus tree: :focus-visible, traps, restoration, gamepad spatial nav** |
| Widget catalog scope | Editor focus | **Game + App, both** (foundation goal 6) |
| Bevy version policy | Pinned to 0.14, frozen | **Tracks latest stable, each minor is a migration event** |
| Verification | None | **CI gates: visual regression, AccessKit snapshots, APG keyboard contracts** (foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) |
| Render pipeline | Bevy's bevy_ui | **Own render pipeline** (rounded clipping, top-layer, backdrop-filter, blend modes) |
| Text shaping | Bevy/cosmic-text via bevy_text | **Own cosmic-text integration** (decoupled from Bevy 0.19 parley migration) |

Buiy's design is largely a corrective response to the failure modes of libraries like sickle — solo-maintained, Bevy-version-coupled, no-a11y, no-verification. sickle's design choices are not wrong in isolation (most of its primitives are well-shaped); they are wrong **in combination with the build-on-bevy_ui foundation that makes them brittle to engine churn.**

## Implications for Buiy

1. **The "editor-and-utilities" scope claim is a smell.** Both sickle and bevy_feathers claim it; both end up with a widget catalog that doesn't cover production-app needs (no text input, no live regions, no APG-keyboard tabs). Buiy's "Game and app, both" scope is deliberately broader.
2. **Headless-behavior / styled-presentation must be separate crates from day 1.** Validated by the `bevy_ui_widgets` + `bevy_feathers` split. sickle conflated them and the conflation became a migration burden.
3. **Lineage continuity across Bevy versions is the load-bearing maintenance choice.** sickle survived as scaffold-only-DNA into cobweb, which then died too. Buiy's "parallel stack" choice is partly a bet that *the Bevy team's component-model churn is too fast for any in-tree-on-bevy_ui library to compound non-trivial design surface across multiple minors.*
4. **Production-app proof matters more than github-stars proof.** sickle has 9 stars on the surviving fork, low download numbers, no flagship-app credit. The lack of a real-world battle test is data; lunex's Foresight Spar credit is also data. Buiy's verification pipeline (CI + manual release gates per foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) substitutes machine-testable claims for community-anecdote claims.

## Sources

- crates.io API (downloads / versions) — https://crates.io/api/v1/crates/sickle_ui
- docs.rs — https://docs.rs/sickle_ui/0.4.0/sickle_ui/
- Surviving fork — https://github.com/UkoeHB/sickle_ui
- bevy_feathers prior-art — [`../bevy-feathers/`](../bevy-feathers/)
- bevy_ui_widgets prior-art — [`../bevy-ui-widgets/`](../bevy-ui-widgets/)
- bevy_lunex prior-art — [`../bevy-lunex/`](../bevy-lunex/)
- bevy_egui prior-art — [`../bevy-egui/`](../bevy-egui/)
- bevy_cobweb_ui (archived successor) — https://github.com/UkoeHB/bevy_cobweb_ui
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation verification spec — [`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
