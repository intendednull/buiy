**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — stewardship within the Bevy Foundation, SME contributors, decision-making for a11y design, relationship to AccessKit / Pneuma Solutions, future-direction signals

# Governance

`bevy_a11y` is a workspace crate inside `bevyengine/bevy` and inherits the Bevy Foundation's overall governance. There is no separate a11y-focused org, SIG charter, or working-group constitution; design decisions move through the same Bevy issue → draft PR → maintainer review → merge path that every other Bevy crate uses, with a small group of named SMEs whose review weight is informal but consistent. This file pins who those SMEs are, what the recent decision history says about their authority, and where the upstream / Pneuma Solutions boundary sits.

## Bevy Foundation context

The Bevy Foundation is a 501(c)(3) nonprofit (formed 2024) that holds the Bevy trademark and employs `@cart` (Bevy's lead). The foundation's role is structural — copyright stewardship, fiscal sponsorship, full-time-maintainer funding — not technical-design authority. Technical decisions still flow through the GitHub repo's maintainer process. `bevy_a11y` has never received a foundation-level design statement; its trajectory is shaped by issue-and-PR activity from a handful of named contributors.

## Active a11y-area contributors

Verified from recent `bevy_a11y` + `bevy_ui::accessibility` PR / issue history (2024–2026):

- **Nolan Darilek (`@ndarilek`)** — original author of PR [#6874](https://github.com/bevyengine/bevy/pull/6874) and primary maintainer of the AccessKit dependency-bump rhythm (PRs #8655, #16234, etc.). His contribution pattern is "absorb AccessKit majors and keep `accesskit_winit` aligned." Domain background outside Bevy: independent a11y software engineer; longtime blind-developer-community advocate; author of multiple Rust accessibility crates. Treat him as the **a11y SME for the integration mechanic** (adapter wiring, `ActionRequest` routing, the activation gate).
- **`@viridia`** — opened issue [#17644](https://github.com/bevyengine/bevy/issues/17644) (BSN-unfriendly megacomponent), authored PR [#24308](https://github.com/bevyengine/bevy/pull/24308) (the `AccessibleLabel` partial fix), authored issue [#22345](https://github.com/bevyengine/bevy/issues/22345) (the architectural-redesign framing for bevy_ui renderer caps), and is the editorial voice of "A Vision for Bevy UI" (HackMD). Treat him as the **a11y SME for component-model surface** — the person framing what BSN-compatibility means in practice. viridia is also the BSN advocate driving PR #20158, so the a11y-vs-BSN crossover is a single person's design through-line.
- **Alice Cecile (`@alice-i-cecile`)** — long-time Bevy maintainer with cross-cutting review authority. Her involvement in a11y issues is review-and-merge oversight rather than original design, but a11y-PR merges typically require her or `@cart`'s approval. The "10 Challenges" (issue #11100) author Tim Jentzsch and Alice Cecile share the cross-cutting-API review role for bevy_ui-touching PRs.
- **Matt Campbell (`@mwcampbell`, upstream)** — AccessKit's primary architect, credited in the Bevy 0.10 release notes for reviewing the integration. He is not a Bevy maintainer and does not have merge rights, but a11y-design questions that touch AccessKit's contract are routed through him via cross-repo discussion. The Bevy 0.10 acknowledgement is the load-bearing example: "Special acknowledgement goes to `@mwcampbell`, AccessKit's lead author, for reviewing the integration." See [`prior-art/accesskit/governance.md`](../accesskit/governance.md) for his Pneuma Solutions context.

> *Correction vs preamble:* the preamble listed Campbell as a "former NVDA developer." Per [NV Access's about page](https://nvaccess.org/about-nv-access/) NVDA was created by Michael Curran in 2006 and co-led by James Teh. Campbell's screen-reader experience is Serotek + Microsoft, then Pneuma Solutions (which he co-founded in 2020 as CTO with Mike Calvo). Not NVDA. The AccessKit folder corrects this same point.

## Is there a Bevy a11y working group?

**No formal working group exists.** The closest analogue is the cluster of contributors named above plus the "A Vision for Bevy UI" HackMD doc viridia maintains — that doc is the de facto coordination point for UI-and-a11y direction discussion. Bevy's "SIG" model is informal: contributors self-organize around a subsystem, and decision authority remains with the merge-rights maintainer (`@cart` and the small group with explicit merge rights).

Discussion #1968 ("Accessibility Features") on the Bevy repo, opened in 2021, is the historical accessibility-direction venue but is now mostly inactive — the discussion shifted to per-issue threads after `bevy_a11y` landed in 0.10.

## Decision-making path for `bevy_a11y` design changes

The trajectory of the megacomponent incident (issue #17644 → PR #24308) demonstrates the path:

1. **Issue filed by SME** — viridia, with concrete examples and a proposed direction ("idiomatic API that transforms to AccessKit").
2. **Sit in queue** — issue opened 2025-02-02; no PR for 15 months.
3. **Partial fix authored by same SME** — `AccessibleLabel`, 2026-05-15. Scope intentionally small: one property, hook-synchronised, BSN-compatible.
4. **Review + merge by maintainer cluster** — Alice Cecile and others, six-day turnaround.
5. **Issue closed** — even though the broader decomposition implied by the issue's framing is unfinished (issue [#20524](https://github.com/bevyengine/bevy/issues/20524) remains the follow-up).

The path is: SME identifies, SME or adjacent SME drafts, maintainer cluster reviews. There is no RFC stage, no design-document gate, no a11y-WG vote. The lightweight RFC process is identified in the parent `bevy-ui/governance.md` folder as a Bevy-wide pattern and a noted weakness — the 22-month BSN saga (Discussion #14437 → PR #20158 still draft) is the canonical demonstration. `bevy_a11y` benefits from the same lightweight model when changes are small and SME-aligned; it suffers from it when changes need coordination (the megacomponent decomposition is structurally a multi-PR migration with no single owner).

## Relationship to upstream AccessKit

The `bevy_a11y` ↔ AccessKit relationship is **producer ↔ schema**: Bevy is one of multiple AccessKit producers (egui, Slint, Freya, Xilem/Masonry — see [`ecosystem.md`](ecosystem.md)). Bevy does not vendor AccessKit, does not have committers on AccessKit, and absorbs AccessKit majors through `ndarilek`'s bump-PR rhythm.

Matt Campbell engages with Bevy-side a11y design questions through cross-repo issues but does not have repo authority. The 2023-03 acknowledgement from `@cart` is the only formal recognition. There is no MOU, no joint roadmap, no shared release calendar. Buiy inherits this same loose-coupling — the AccessKit cadence policy is an open question in `architecture.md § 2.9` precisely because the relationship is informal on both sides.

## Pneuma Solutions and the soft-corporate edge

Matt Campbell is CTO of Pneuma Solutions ([Pneuma Solutions about page](https://pneumasolutions.com/about/)); the company affords him AccessKit-development time, but neither Pneuma nor any other company has a formal sponsorship arrangement with `bevy_a11y`. The AccessKit folder's `governance.md` calls this "company-adjacent stewardship" — Buiy's a11y story sits one layer further out: Buiy → AccessKit → Pneuma-adjacent maintainer. If Pneuma's posture toward AccessKit changes, Buiy's accessibility story has no contractual continuity. This is a documented contingency in [`open-problems.md`](open-problems.md).

## Future-direction signals

What the SME cluster's public writing says about where `bevy_a11y` is going (as of 2026-05-22):

- **viridia's "A Vision for Bevy UI" (HackMD)** — calls for "idiomatic API that transforms to AccessKit" framing. The `AccessibleLabel` PR is the first concrete step. The vision document does not commit to a timeline; it identifies the direction.
- **viridia's issue #22345 (2026-01-02)** — frames the bevy_ui renderer story as requiring "substantial architectural redesign." The bevy_ui-a11y crossover (the feathers-gallery example as the integration testbed) implies the a11y component-model rework is coupled to whatever bevy_ui structural change lands first.
- **Alice Cecile's posts on Bevy's editor + tooling direction** — emphasize that `bevy_feathers` is the editor's widget set, which makes feathers's a11y conformance a foundation-priority concern. This is the strongest "a11y must improve" signal from the maintainer side.
- **`@cart`'s BSN PR #20158 status** ("not intended to be merged in current form" — see [`prior-art/bevy-ui/governance.md`](../bevy-ui/governance.md)) — means the BSN-friendliness motivation for `bevy_a11y` decomposition is still anticipating a moving target. The component decomposition needs to be BSN-friendly *before* BSN lands so that a refit isn't needed *after*.

**Net-net.** `bevy_a11y` has identified SMEs, a known direction, no formal decomposition plan, and a partial-fix shipping rhythm (one property at a time, hook-synchronised onto the megacomponent). For Buiy this is good news on one axis (the direction matches Buiy's choices) and ambient risk on another (the cadence is "one property per Bevy minor at the current rate" — full decomposition is years out, while Buiy needs the decomposed surface at v1). The replace-`bevy_a11y`-for-Buiy-windows posture in `architecture.md § 2.6` is the structural answer to that mismatch.

## Sources

- PR [#6874](https://github.com/bevyengine/bevy/pull/6874) — original integration, `ndarilek` author, `mwcampbell` reviewer-credit.
- Issue [#17644](https://github.com/bevyengine/bevy/issues/17644) — `viridia` author, BSN-incompatibility framing.
- PR [#24308](https://github.com/bevyengine/bevy/pull/24308) — `viridia` author, `AccessibleLabel` partial-fix landing 2026-05-21 against 0.19.
- Issue [#22345](https://github.com/bevyengine/bevy/issues/22345) — `viridia` author, bevy_ui structural-redesign framing.
- Issue [#20524](https://github.com/bevyengine/bevy/issues/20524) — broader a11y decomposition follow-up referenced from PR #24308.
- Discussion [#1968](https://github.com/bevyengine/bevy/discussions/1968) "Accessibility Features" — historical direction venue (now mostly inactive).
- "A Vision for Bevy UI" HackMD doc by viridia: https://hackmd.io/@bevy/HkjcMkJFC.
- Bevy Foundation context: bevy.org/foundation.
- Pneuma Solutions context (Matt Campbell as CTO): [`prior-art/accesskit/governance.md`](../accesskit/governance.md), [Pneuma Solutions about page](https://pneumasolutions.com/about/).
- NV Access about page (NVDA founders, for the lineage-correction note): https://nvaccess.org/about-nv-access/.
- Sibling files: [`history.md`](history.md), [`component-model-incident.md`](component-model-incident.md), [`ecosystem.md`](ecosystem.md), [`open-problems.md`](open-problems.md).
- Buiy foundation cross-references: [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).
