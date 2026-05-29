**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — Bevy's official ECS-native UI crate; the anchor reference for the Buiy parallel-stack design bet

# bevy_ui

`bevy_ui` is the official UI crate in the Bevy engine workspace (`bevyengine/bevy`). It is a CSS-flavoured retained-mode UI library built on Bevy's ECS: a hierarchy of `Node` entities that drive Taffy for layout, parley + swash (post-0.19) / cosmic-text (≤ 0.18) for text, AccessKit for accessibility, `bevy_picking` for hit-testing, and a render-graph node for visuals. It is the default UI library Bevy users reach for, the substrate that the official `bevy_feathers` widget kit + `bevy_ui_widgets` headless primitives sit on, and the architectural anchor against which **every Buiy design decision is taken** — Buiy is intentionally parallel to bevy_ui, integrating the same Taffy / cosmic-text / AccessKit / bevy_picking / wgpu primitives directly with its own component model and render pipeline (see [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.1–2.3). bevy_ui therefore deserves the largest, most version-pinned prior-art folder in this corpus: every Buiy spec author will consult it for "what does upstream do here, and why are we doing it differently?"

**Honest assessment.** bevy_ui ships in every Bevy app by default and has had AccessKit integration since 0.10 (March 2023, PR #6874) and Taffy as the layout substrate since 0.8 (July 2022, PR #4716) — these are real, durable assets. But it has **no flagship commercial game shipping with bevy_ui as the UI layer** (the most-cited Bevy production title, Tiny Glade, wrote its own UI renderer); it has no published bench data at productivity-app node counts (1000s of nodes — performance issues #677 / #276 / #2451 are old but never closed with bench data); and its renderer caps several capabilities (non-rectangular clipping, `backdrop-filter`, `mix-blend-mode`, isolation, true CSS top layer) that web-platform parity requires. Issue [#22345](https://github.com/bevyengine/bevy/issues/22345) (viridia, 2026-01-02) documents this as upstream's own diagnosis: `"We currently only support rectangular clipping regions ... which are inadequate for the kinds of UIs we want to build"`; the proposed fix is labeled "`require substantial architectural redesign`." This corpus reports those gaps verbatim — they are the load-bearing rationale for Buiy's parallel-stack decision and the most concrete validation of it.

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `bevy_ui` (workspace crate inside `bevyengine/bevy`) |
| License | MIT OR Apache-2.0 |
| Latest stable | **0.18.1** (published 2026-03-04) |
| Pre-release | **0.19.0-rc.1** (published 2026-05-13) |
| Workspace HEAD | 0.19.0-dev |
| MSRV (0.19) | rust-version 1.95.0, edition 2024 |
| Lifetime downloads | 4,901,387 |
| 90-day downloads | 943,255 |
| Bevy repo stars | ~46,200 |
| Bevy age | ~6 years (first crates.io publish 2020-08-10) |
| Cargo features | `default` (empty), `serialize`, `bevy_picking`, `ghost_nodes` (experimental) |
| First `bevy_ui` release | 0.4.0, 2020-12-19 |
| Stretch → Taffy | 0.8, 2022-07-30, PR #4716 |
| AccessKit integration | 0.10, 2023-03-06, PR #6874 (Nolan Darilek) |
| ab_glyph → cosmic-text | 0.15, 2024-11-29 (PR #10193 merged 2024-07-04) |
| Required Components / NodeBundle deprecation | 0.15, 2024-11-29, PR #14791 (cart) |
| `bevy_feathers` | 0.17, 2025-09-30, PR #19730 (ickshonpe) |
| `bevy_ui_widgets` (headless) | 0.17, 2025-09-30 |
| `AutoDirectionalNavigation` | 0.18, 2026-01-13 |
| cosmic-text → parley + swash | 0.19-dev (issue #21765, 2025-11-06; **post-0.19 divergence point with Buiy**) |
| BSN (PR #20158) | **Still draft / unmerged** as of 2026-05-22; cart wrote: `"not intended to be merged in current form"` |
| Bevy Foundation status | Washington 501(c)(3) public charity |
| Foundation board | cart (President + Interim Treasurer), Alice Cecile (Secretary), François Mockers, Robert Swain, James Liu |

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, ToC, framing disclosure. |
| [`lessons.md`](lessons.md) | **The consult-this-when-designing decision file.** Validates / avoid / borrow. |
| [`glossary.md`](glossary.md) | System-specific terms used across the corpus. |
| [`architecture.md`](architecture.md) | Plugin placement, system ordering, `Node`/`ComputedNode` decomposition, Taffy integration, render pipeline shape and its current caps. |
| [`component-model.md`](component-model.md) | Today's component surface, the 0.15 Required-Components migration, issue #17644 (BSN-hostility lesson), `bevy_feathers` / `bevy_ui_widgets` extension surface, authoring patterns. |
| [`layout.md`](layout.md) | Layout primitives (flex, grid, block), what Taffy ships and what it doesn't, scroll containers, subgrid / anchor / container-query status. |
| [`styling.md`](styling.md) | Visual styling primitives, `bevy_feathers` theming, the absent stylesheet, missing user-preference support. |
| [`text-and-input.md`](text-and-input.md) | Text rendering stack timeline (ab_glyph → cosmic-text → parley + swash), text-edit status, picking integration, focus model, gamepad / spatial nav. |
| [`history.md`](history.md) | Chronological release timeline (0.4 → 0.19-rc.1) with verified crates.io dates. |
| [`distribution.md`](distribution.md) | Release cadence, Cargo features, platform matrix, MSRV, coexistence with sibling crates. |
| [`governance.md`](governance.md) | Bevy Foundation, board, SMEs, RFC process, funding, roadmap mechanics. |
| [`ecosystem.md`](ecosystem.md) | First-party companion crates and third-party UI stacks (bevy_lunex, sickle_ui, woodpecker_ui, kayak_ui, bevy_egui, bevy_flair, bevy_cosmic_edit). |
| [`critiques.md`](critiques.md) | First-party and third-party criticisms of design, renderer, ergonomics; cart's own self-critiques. |
| [`comparisons.md`](comparisons.md) | Head-to-head vs the parallel and complementary UI stacks, including a cross-reference matrix. |
| [`open-problems.md`](open-problems.md) | What bevy_ui structurally doesn't solve as of 0.18.1 / 0.19-rc.1. |

## How to use this corpus

1. **If you are designing a Buiy feature**, start at [`lessons.md`](lessons.md). Find the relevant `Avoid` row (a bevy_ui pitfall) or `Borrow` entry (a primitive worth studying). Each row links to the specific evidence file.
2. **If you are evaluating a third-party Bevy UI stack** (bevy_lunex / sickle_ui / woodpecker_ui / etc.), start at [`comparisons.md`](comparisons.md) for the head-to-head, then [`ecosystem.md`](ecosystem.md) for context.
3. **If you are auditing a renderer-feature gap** (clipping, blend modes, backdrop-filter, top layer), start at [`architecture.md`](architecture.md) § "Renderer caps" and follow into [`critiques.md`](critiques.md) § "The renderer feature gaps."
4. **If you are tracking what shipped when** (which Bevy release added a UI primitive, which release rewrote which subsystem), start at [`history.md`](history.md).
5. **If you are writing a Buiy spec that depends on Bevy's release cadence or platform matrix**, start at [`distribution.md`](distribution.md).

## Cross-document inconsistencies surfaced

These were flagged during synthesis. Each is resolved in the linked file but called out here so future readers know where the original ambiguity lived.

- **AccessKit version pin.** `bevy_feathers` HEAD depends on `accesskit 0.24`; the 0.17.3 stable tag depended on `accesskit 0.21`. Both numbers are right for different refs. [`component-model.md`](component-model.md) reports the HEAD version; [`lessons.md`](lessons.md) carries this as a "verify-per-release" reminder.
- **cosmic-text adoption release.** Adoption PR #10193 was merged 2024-07-04 (during the 0.14 cycle); the migration *shipped* in 0.15 (2024-11-29). [`text-and-input.md`](text-and-input.md) and [`history.md`](history.md) both report shipped-in-0.15.
- **BSN landed in 0.18.** Pre-amble incorrectly assumed BSN had landed. PR #20158 is **still draft / unmerged as of 2026-05-22**; cart wrote it is `"not intended to be merged in current form."` This is the single most important finding in [`lessons.md`](lessons.md) and reframes the Buiy foundation spec's BSN-friendly-components rule from "design *because* BSN landed" to "design *so that* BSN can land later."
- **bevy_feathers release.** Shipped in 0.17 (2025-09-30), not 0.16 — confirmed in [`history.md`](history.md), [`component-model.md`](component-model.md), and [`ecosystem.md`](ecosystem.md).
- **Post-0.19 text-shaper divergence.** Bevy main migrated `bevy_text` from cosmic-text to parley + swash (issue #21765, opened 2025-11-06). Buiy commits to cosmic-text (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.2). Buiy and post-0.19 bevy_ui therefore diverge on text shaper from this point forward. The second most important finding in [`lessons.md`](lessons.md).

## Framing disclosure

These docs are written from a **Buiy-parallel-to-bevy_ui + web-platform-parity + WCAG 2.2 AA + BSN-friendly-by-construction + AccessKit-first** stance. Most `Implications for Buiy` lines frame bevy_ui's choices through that lens: a bevy_ui limitation becomes a Buiy design opportunity, a bevy_ui maturation moment becomes a Buiy validation. Future readers auditing whether *parallel-stack* is itself the right primitive should weigh the corpus accordingly. It is a learn-from-bevy_ui-into-Buiy artifact, not a neutral catalog.

A secondary disclosure: bevy_ui is a load-bearing **architectural neighbour** (Buiy ships in the same `App`, registers its own picking backend and AccessKit adapter alongside bevy_ui's, can coexist per-window). The corpus has an incentive to soft-pedal Buiy-specific risks that flow from Bevy's choices (rolling-latest-stable migration treadmill, the bevy_a11y replacement burden, render-graph node-ordering coordination). Where the corpus is silent on a risk that Bevy's choices create for Buiy, default-assume the silence is bias and pressure-test.

## Sources

- bevy_ui on crates.io — https://crates.io/crates/bevy_ui
- bevy_ui crates.io API metadata (fetched 2026-05-22) — https://crates.io/api/v1/crates/bevy_ui
- Bevy repository — https://github.com/bevyengine/bevy
- Bevy Foundation — https://bevy.org/foundation/
- Bevy 0.4 release notes — https://bevy.org/news/bevy-0-4/
- Bevy 0.8 release notes — https://bevy.org/news/bevy-0-8/
- Bevy 0.10 release notes — https://bevy.org/news/bevy-0-10/
- Bevy 0.15 release notes — https://bevy.org/news/bevy-0-15/
- Bevy 0.17 release notes — https://bevy.org/news/bevy-0-17/
- Bevy 0.18 release notes — https://bevy.org/news/bevy-0-18/
- Issue #17644 (bevy_a11y BSN-unfriendly) — https://github.com/bevyengine/bevy/issues/17644
- Issue #22345 (Unified Bevy User Interface) — https://github.com/bevyengine/bevy/issues/22345
- Issue #21765 (cosmic-text → parley migration) — https://github.com/bevyengine/bevy/issues/21765
- PR #20158 (BSN, still draft) — https://github.com/bevyengine/bevy/pull/20158
- AccessKit project — https://accesskit.dev
- Buiy foundation spec — ../../specs/2026-05-07-buiy-foundation/README.md
- Buiy architecture — ../../specs/2026-05-07-buiy-foundation/architecture.md
