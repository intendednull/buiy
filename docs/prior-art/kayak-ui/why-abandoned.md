**Date:** 2026-05-22
**Status:** archived
**Subject:** kayak_ui — structural analysis of the abandonment: why solo + parallel-stack + custom-DSL + pre-1.0 + Bevy quarterly cadence has no equilibrium.

# Why abandoned

This is the load-bearing-for-Buiy analysis file. Read it before importing any pattern from this corpus into a Buiy spec.

## The headline finding

**kayak_ui was not killed by a single bug, a single migration, or a single competing project.** It was killed by the compound interaction of five structural factors, each individually tolerable, jointly fatal on a ~24-month timescale:

1. **Solo maintainer** (StarArawn, also maintaining the higher-traffic `bevy_ecs_tilemap` — 1.2k★ vs kayak_ui's 482★).
2. **Pre-1.0 Bevy** with a quarterly breaking-release cadence (Bevy 0.9 → 0.18 = ten breaking minor releases in 38 months).
3. **Parallel UI stack** with its own renderer, layout engine (morphorm), focus tree, and component model — every Bevy bump is a non-trivial migration.
4. **Custom authoring DSL** (`rsx!`) outside Bevy's macro / reflection / `Bundle` ecosystem — every Bevy bump *also* touches the macro-expansion surface.
5. **React-style component model in Rust** — implementation complexity that the maintainer himself later cited as the rewrite trigger ("*overly complicated internals that made contributing much too difficult and caused quite a few fundamental bugs*" — woodpecker_ui README).

No single factor proves abandonment. All five together produce a maintenance load that scales super-linearly with each Bevy release. The maintainer hit the wall around Bevy 0.13 (Feb 2024); rather than fight it, he started over with woodpecker_ui (Jul 2024) on a simpler architecture (Taffy + Vello + Parley instead of morphorm + custom-render + MSDF).

## The Bevy quarterly cadence problem (and what it does to a custom DSL)

Bevy ships a breaking minor release every ~3 months. From kayak_ui's perspective, that means every quarter:

- `bevy_render` graph types shift → kayak_ui's custom render node needs an audit.
- `bevy_app` plugin / system / schedule signatures shift → `KayakContextPlugin` needs rewiring.
- `Bundle` semantics shift (a major theme in 0.13 → 0.15) → every `rsx!` expansion site that names a kayak_ui `*Bundle` type needs an audit.
- `bevy_input` / `bevy_winit` event paths shift → input routing needs a re-walk.
- `Component` derive + reflection registration semantics shift → kayak_ui's widget structs need re-derive.

A **bridge-flavored** crate (like `bevy_cosmic_edit`) has a comparable problem but with only one breaking-upstream-axis (cosmic-text or Bevy at a time). A **parallel-stack + custom-DSL** crate (like kayak_ui) has the full surface area on every Bevy bump, plus a macro-expansion site that also needs to keep up. Migration cost compounds.

## The custom-DSL tax, specifically

`rsx!` was kayak_ui's load-bearing ergonomics decision and its load-bearing maintenance burden. Two reasons:

1. **The macro had to know Bevy's `Bundle` shape.** When Bevy changed how bundles work — and Bevy did this repeatedly across 0.10 → 0.15, then again with the `required components` mechanism (Bevy 0.15, [PR #14791](https://github.com/bevyengine/bevy/pull/14791)) — every `rsx!` expansion potentially changed shape.
2. **Bevy itself was simultaneously designing its own declarative-authoring story.** [Discussion #14437](https://github.com/bevyengine/bevy/discussions/14437) (BSN tracking) opened 2024-07-25 — 17 days after the kayak_ui repo's last commit. BSN is not yet merged ([PR #20158](https://github.com/bevyengine/bevy/pull/20158), still draft as of 2026-05-22, per [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Top-of-file finding 1), but its eventual landing makes any third-party DSL **a parallel-and-shrinking surface**: anyone who learned `rsx!` would have to re-learn BSN, and any maintainer of a `rsx!`-style crate has to compete with the engine's official authoring syntax for community attention.

Buiy's response (cemented in [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Top-of-file finding 1, and [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)): no custom DSL; ship BSN-friendly-by-construction now (small, decomposed, public-fielded, reflection-registered) so Buiy authoring rides the engine's official syntax when BSN lands.

## The pivot signal: kayak_ui → woodpecker_ui

The clearest evidence that kayak_ui's structural problem was real (not just "maintainer got bored") is that StarArawn **kept investing in Bevy UI, just on different architecture**. Ten days after the last kayak_ui commit, he created woodpecker_ui. The README compares the two directly:

> "*Kayak UI suffered from overly complicated internals that made contributing much too difficult and caused quite a few fundamental bugs. So, I took what made Kayak UI great and made the backend much much simpler. Reducing the primary system from over 1,000 lines to fewer than 200.*"

Substrate flips (kayak_ui → woodpecker_ui):

| Subsystem | kayak_ui | woodpecker_ui | Note |
|---|---|---|---|
| Layout | morphorm | **Taffy** | Aligns with bevy_ui's choice (Taffy since 0.8). |
| Renderer | custom MSDF + quad shader | **Vello** | A more capable, externally-maintained renderer. |
| Text | MSDF | **Parley** | Aligns with bevy_ui 0.19-dev migration target. |
| Component model | React-style (function widgets + hooks) | **ECS-driven** | Aligns with Bevy idioms. |
| DSL | custom `rsx!` | **similar syntax** but simpler internals | Still a custom DSL (still a long-term risk). |

The structural-correction direction is clear: drop the bespoke substrates, ride upstream-aligned ones (Taffy, Vello, Parley), drop the React paradigm, lean into ECS. **Buiy's foundation spec converges on the same substrates** (Taffy, cosmic-text or Parley, ECS-native, no custom DSL) — independently, but not coincidentally. The same forcing functions push any serious Bevy UI project toward the same shape.

## Passive abandonment vs deliberate archive — and why it matters

[`bevy_cosmic_edit`](../bevy-cosmic-edit/) was **deliberately archived** by its maintainer on 2025-03-21: archive banner, public read-only state, explicit signal to consumers. kayak_ui has **never been deliberately archived**: GitHub API still reports `archived: false` (verified 2026-05-22); README has no deprecation banner; crates.io still serves 0.5.0; no `cargo yank` has been issued.

This is **worse for consumers**, not better. A passively-abandoned crate:

- Still appears in `cargo add` suggestions.
- Still ranks in Google / Reddit for "Bevy UI" queries — search engines have no signal that it's dead.
- Has 18,774 lifetime crates.io downloads still ticking upward.
- Has open issues from years ago that look like "active project, maintainer just busy."
- Lacks the unambiguous "do not depend on this" signal a deliberate archive provides.

The lesson for Buiy is **not** "be more like bevy_cosmic_edit's archive process" — Buiy isn't archive-bound; it's an active project. The lesson is structural: **never ship a load-bearing piece of Buiy as a separate, solo-maintained, third-party crate.** Whatever Buiy maintains as a crate, Buiy commits to maintaining for as long as Bevy exists or Buiy itself does, with bus-factor >1. Anything that can't be maintained at that bar belongs *in* the foundation, not *next to* it.

## Comparable cases

| Project | Failure mode | Lesson |
|---|---|---|
| **`bevy_mod_picking`** | Successful absorption — donated to / replaced by Bevy's official `bevy_picking` (Bevy 0.15+). | Path A: become the engine. Requires upstream alignment + handoff. |
| **`bevy_cosmic_edit`** | Deliberate archive (2025-03-21). Bridge-crate burden between cosmic-text + bevy_ui became untenable. | Path B: archive clean, signal to consumers. |
| **`kayak_ui`** (this folder) | Passive abandonment (Feb 2024 silent, last commit Jul 2024). No banner; maintainer pivoted to successor. | Path C: failure mode to avoid. |
| **`belly`** (out of scope) | Slowed; not the focus of this corpus. | Path D: long tail. |

`bevy_mod_picking` is the success case. `bevy_cosmic_edit` is the clean-exit case. **kayak_ui is the cautionary tale** — and it's cautionary precisely because nothing ceremonial happened. The crate is still in caches, still in `Cargo.lock` files, still in the search results, still in the back of new Bevy users' minds as a possible answer to "I want declarative UI." Buiy's commitment is to be unambiguous about its own status at all times — not to drift into kayak_ui's twilight.

## What Buiy explicitly does to avoid this fate

From the foundation spec ([`../../specs/2026-05-07-buiy-foundation/README.md` § Goals](../../specs/2026-05-07-buiy-foundation/README.md)):

1. **No custom DSL.** Buiy components are BSN-friendly-by-construction; authoring rides Bevy's official syntax (whatever BSN ultimately ships as) without an adapter.
2. **Ride upstream substrates.** Taffy for layout, cosmic-text for text shaping (until / unless Buiy makes a different decision in `buiy-text-rendering-design`), AccessKit for a11y, `bevy_picking` for hit-testing. No bespoke replacements unless the upstream is structurally blocking a Buiy goal.
3. **Parallel to bevy_ui, not third-party-extending-it.** Buiy *is* the UI library, not a layer on top of one. The maintenance burden is single-rooted at Buiy, not split between Buiy and a host UI library.
4. **Single canonical doc tree.** `docs/specs/`, `docs/plans/`, `docs/reports/`, `docs/prior-art/`. No design state in Discord. No "the maintainer's HackMD" as ground truth.
5. **Honest status signaling.** Buiy releases declare which Bevy minor they target. If Buiy stops keeping up, the README says so the same week.

## Sources

- woodpecker_ui README (verbatim "*overly complicated internals*" quote) — https://github.com/StarArawn/woodpecker_ui#readme
- kayak_ui GitHub API metadata (archive status, last push) — https://api.github.com/repos/StarArawn/kayak_ui
- kayak_ui crates.io listing (still default-version 0.5.0) — https://crates.io/crates/kayak_ui
- Bevy BSN discussion #14437 — https://github.com/bevyengine/bevy/discussions/14437
- Bevy BSN PR #20158 — https://github.com/bevyengine/bevy/pull/20158
- Bevy required-components PR #14791 — https://github.com/bevyengine/bevy/pull/14791
- bevy-cosmic-edit sister case study — [`../bevy-cosmic-edit/why-archived.md`](../bevy-cosmic-edit/why-archived.md)
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- bevy-ui top-of-file findings — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
