**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — why it was archived: the structural anti-pattern of bridge-crate maintenance between two fast-moving Rust ecosystems.

# Why archived

The repository was archived by its owner Dimchikkk on **2025-03-21**, three and a half months after the final release (0.26.0, 2024-12-07) and six weeks after the last commit (2025-02-04). The archive event was unannounced — no commit message, README note, blog post, Discord thread, or migration doc. The archive banner itself is the only public signal.

This file is the structural analysis: **why** a once-active, ~110-star, ~40k-download crate was archived during a window when it was still actively used and its maintainer was clearly still around. The short answer: it was a **bridge crate** between two fast-moving Rust UI ecosystems, and bridge crates of that shape are not sustainable without funded stewardship.

## The structural problem

bevy_cosmic_edit had to track **two independent upstream projects** to stay alive:

1. **Bevy** — major release every ~3 months. Each release breaks the component model, the render graph, the input system, or all three. Every Bevy release required a bevy_cosmic_edit migration.
2. **cosmic-text** — major release every ~1–2 months pre-0.20, slowing to ~quarterly post-0.20. Each release changed the `Buffer` / `Editor` / `FontSystem` shape, the `Shaping::Advanced` API, or the `SwashCache` API.

The schedules were independent. A typical 90-day window contained:

- 1 Bevy minor release (breaking).
- 2-3 cosmic-text minor releases (each potentially breaking).
- ~1 supporting-crate breakage (winit, wgpu, image, arboard, fontdb).

bevy_cosmic_edit had to absorb every one of these and ship a new release on top, pinning specific upstream versions and verifying the integration tests still passed. The maintenance budget is roughly **the sum of two upstream's breaking-change cadences**, which over a year is a substantial unfunded volunteer load.

Compare to a "ecosystem-internal" crate like `bevy_input_focus` (which lives inside Bevy and only tracks Bevy's own internal changes): its maintenance budget is one upstream's cadence, and the maintainer is typically a Bevy core team member with allocated time.

## The catch-up problem

Independent of the maintenance cost, **the bridge's value decreases over time** as the upstreams catch up to what the bridge originally provided.

When bevy_cosmic_edit first shipped (mid-2023, Bevy 0.11), bevy_ui's `Text` widget was display-only — no input, no caret, no selection, no IME. cosmic-text was new and not yet integrated anywhere in Bevy. There was a real gap, and bevy_cosmic_edit filled it.

By the time of archive (early 2025):

- Bevy 0.15 (the last bevy_cosmic_edit target) had moved `bevy_text` onto cosmic-text 0.12 as a first-class internal dependency. The font-system, glyph rasterization, and atlas were now bevy_text's responsibility.
- bevy_picking had been merged into Bevy 0.16 (after archive); the dedicated bridge bevy_cosmic_edit maintained for pointer events became less differentiated.
- `bevy_input_focus` (Bevy 0.16+, [PR #15388](https://github.com/bevyengine/bevy/pull/15388)) provided a real focus tree, making bevy_cosmic_edit's `FocusedWidget` singleton obsolete.
- `bevy_feathers` (Bevy 0.17, late 2025) shipped a text-input widget for the widget set, intended as bevy_ui's own native text editing.
- Bevy `main` (0.19-dev, post-archive) migrated `bevy_text` off cosmic-text entirely onto Parley + swash (issue [#21765](https://github.com/bevyengine/bevy/issues/21765), opened 2025-11-06 — eight months post-archive but the direction was already visible in 2025).

In other words: at archive time, **bevy_ui's own text-edit story was actively catching up**, and bevy_cosmic_edit's value proposition was shrinking. A maintainer looking at the next 12 months of work could reasonably conclude that the bridge would have to keep tracking both upstreams *while bevy_ui got there on its own*, with no end in sight.

## The bus-factor problem

bevy_cosmic_edit's contributor graph at archive (per [`history.md`](history.md)):

- 1 owner (Dimchikkk).
- 4 repeat contributors (ActuallyHappening, databasedav, bytemunch, iancormac84) — none of whom had push rights to crates.io.
- No corporate sponsorship visible.
- No GitHub Sponsors / OpenCollective tier.
- No Cosmonic / System76 / Foresight Spar / Loop equivalent of a vendor-stakeholder funding maintenance.

Compare cosmic-text itself: stewarded by System76 with COSMIC Desktop as the dogfood substrate (see [`../cosmic-text/governance.md`](../cosmic-text/governance.md)). cosmic-text has corporate-funded weekly attention. bevy_cosmic_edit had none.

When a single volunteer maintainer has to absorb two upstreams' breaking changes on independent quarterly cadences, **without a corporate stakeholder paying for time**, the equilibrium is "either the maintainer burns out or finds an exit." The archive was the exit.

## What we can read from the archive timing

The archive timing tells us this was a **decision**, not a passive lapse:

- The last release was only 3.5 months prior. The crate was not silently dead.
- The last commit (Feb 4, 2025) was 6 weeks before archive — typical for active maintenance, not abandonment.
- An open PR (#168 "Basic 3D Support") and an open issue (#171) were left unaddressed. The archive happened *with work in flight*.
- No deprecation announcement, migration recommendation, or successor pointer was added before archiving.

A reasonable read: Dimchikkk had been weighing the Bevy 0.16 migration (released in early 2025, post-archive), saw bevy_text's trajectory (Parley migration discussions were already public in late 2024), and chose to walk away rather than do another upstream-tracking cycle for a use case bevy_ui was about to absorb.

This is the **structural lesson**: bridge crates between two fast-moving Rust UI ecosystems don't fail dramatically. They get archived quietly when the maintainer does the cost-benefit math and the math says "stop."

## Comparable archived bridge crates

bevy_cosmic_edit is not the only Bevy-ecosystem bridge that fits this shape:

- **`bevy_mod_picking`** — Aevyrie's third-party picking crate. Archived **after** its functionality was upstreamed into Bevy as `bevy_picking` (Bevy 0.15). Different shape — that was a *successful* absorption, not an abandonment — but the same "third-party-bridge → upstream-absorbs → archive" trajectory. See [`../bevy-picking/README.md`](../bevy-picking/README.md).
- **`bevy_egui`** — a long-running bridge between egui and Bevy. Not archived, but the maintenance burden is visible in the release notes; each Bevy bump is a weeks-long porting effort. Survives because (a) egui's API is stable, (b) Vladislav Loginov has been the maintainer for years, (c) Foresight Mining and others use it commercially.
- **`bevy_kira_audio`** — bridge between Kira (audio) and Bevy. Same archetype, also surviving — Kira's API is stable.

The pattern: **bridges survive when one of the two upstreams has a stable API**. cosmic-text + Bevy both had unstable APIs through 2023-2025. bevy_cosmic_edit had no stable side to rest on.

## What this validates for Buiy

The whole point of Buiy's parallel-to-bevy_ui stance (foundation [`architecture.md` § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) is to **not be a bridge crate**. Buiy depends on Bevy (one upstream) and integrates cosmic-text directly (one substrate, owned), not Bevy's text widget. The two-upstream burden bevy_cosmic_edit could not sustain is exactly the burden Buiy structurally avoids.

bevy_cosmic_edit's archive is therefore the **canonical validation** of Buiy's text-edit ownership commitment ([text.md § 3.5](../../specs/2026-05-07-buiy-foundation/text.md#35-text-editing), [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) row "Depending on `bevy_cosmic_edit`"). See [`lessons.md`](lessons.md) for the synthesis.

## Sources

- Archive banner — https://github.com/Dimchikkk/bevy_cosmic_edit
- PR #168 (Basic 3D Support, left open) — https://github.com/Dimchikkk/bevy_cosmic_edit/pull/168
- Issue #171 (last issue, unresolved) — https://github.com/Dimchikkk/bevy_cosmic_edit/issues/171
- Bevy issue #21765 (cosmic-text → Parley) — https://github.com/bevyengine/bevy/issues/21765
- bevy_input_focus PR #15388 — https://github.com/bevyengine/bevy/pull/15388
- cosmic-text governance — [`../cosmic-text/governance.md`](../cosmic-text/governance.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy text.md — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
