**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — governance, SMEs, inclusion-decision rationale, direction signals

# Governance

`bevy_feathers` lives inside the Bevy monorepo (`bevyengine/bevy`) and is therefore governed by the **Bevy Foundation** (Washington-state non-profit, 501(c)(3); see [`../bevy-ui/governance.md`](../bevy-ui/governance.md) for the foundation-level structure). Feathers does not have an independent governance overlay — it is one crate among ~70 in the workspace and inherits the project-wide RFC / SME / release-management apparatus.

## Subject-matter experts (feathers / widgets area)

Reconstructed from PR review history and commit activity, 2025-Q3 → 2026-Q2:

- **@viridia** — de-facto feathers area lead. Initiated the Standard Headless Widgets discussion (#16900, 2024-12-19); primary author of PR #19730 introducing feathers; author of issue #17644 (the bevy_a11y BSN-unfriendliness incident); author of the post-mortem PR #24308 (AccessibleLabel). Drives the design and the corrective work both.
- **@ickshonpe** — highest-volume widget-PR contributor through 0.17 / 0.18 / 0.19-rc cycles. Co-author of PR #19730. Touches feathers controls, scrolling, gradients. The orchestrator pre-amble's "ickshonpe as feathers area-SME" framing is half-right: he is a core contributor but **viridia is the introducer + de-facto lead**.
- **@alice-i-cecile** — UI-area SME, secretary on Bevy Foundation board, merger of PR #19730. Co-author of `bevy_ui_widgets`. Reviews almost every feathers-touching PR.
- **@Atlas16A** — co-author of PR #19730; theme tokens / OKLCH palette.
- **@amedoeyes** — co-author of PR #19730; virtual-keyboard work.
- **@bushrat011899** — UI / picking integration reviewer.
- **@cart** (Carter Anderson) — Bevy president and project lead; not directly active on feathers PRs but holds final architectural authority and is the author of the BSN draft (PR #20158) feathers is expected to migrate to.

The SME set is not formally enumerated on a public page; this list is reconstructed from PR review history. The Buiy spec authors should not assume the set is stable across Bevy minor releases.

## The Bevy 0.17 inclusion decision

Feathers was merged into the Bevy monorepo rather than published as a stand-alone third-party crate. Three signals point to the rationale:

1. **Bevy Editor is the motivating consumer.** The 0.17 release notes state directly: "it will be used to build the upcoming Bevy Editor." Putting the editor's widget kit in the Bevy workspace simplifies cross-version migration — when Bevy makes a breaking change to `bevy_ui`, the editor's widget kit migrates in the same PR.

2. **Experimental feature flag.** The crate ships behind `experimental_bevy_feathers` (default-off in the umbrella). The 0.17 notes call out: "It is currently hidden behind the `experimental_bevy_feathers` feature flag." The bevy maintainers wanted in-tree velocity for editor work without committing to a stable widget API for general consumers.

3. **No existing official option.** Three years after `bevy_ui` shipped, third-party kits (kayak_ui archived, sickle_ui niche, bevy_egui paradigm-different, bevy_lunex parallel-stack) had not converged on an official-blessed kit. The Bevy team chose to take editorial ownership of the widget catalog rather than continue to point users at fragmented ecosystem alternatives.

The merge comment from @alice-i-cecile is verbatim: "I'm happy with this as a base...let's get this merged and let people start experimenting :)" — explicit acknowledgement that the API is not stabilized at merge time.

## RFC process for feathers changes

Bevy does not use a formal RFC repository (see [`../bevy-ui/governance.md`](../bevy-ui/governance.md) for project-wide RFC process). Feathers-specific design discussions happen through:

- **GitHub Discussions** for early-stage proposals — e.g., #16900 (Standard Headless Widgets).
- **Issues with `A-UI` label** for tracking and bug-triage. As of 2026-05-22 there are ~12 open issues tagged `A-UI` that mention `bevy_feathers` directly, including #20047 (focus rings), #19854 (variable font), #23178 (checkbox initialization with variables), #20905 (slider text formatting).
- **Long-form PRs** with embedded design rationale in the description — PR #19730 itself, PR #24308.

The "experimental" framing means breaking changes are expected per Bevy minor release and are not gated by an RFC. The migration-guide pattern (`https://bevy.org/learn/migration-guides/<from>-to-<to>/`) covers feathers in the same way it covers `bevy_ui`.

## Future direction signals

- **BSN migration.** The 0.17 release notes promised feathers would migrate to BSN "in 0.18." BSN did not land in 0.18; PR #20158 remains draft (per cart's own commentary: `"not intended to be merged in current form"`). The migration is therefore pending. See [`open-problems.md`](open-problems.md) and [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § "BSN has not landed."
- **Bevy Editor cadence.** Feathers' release tempo is coupled to editor demand. The editor itself is in pre-release; as more widgets become editor-blocking they ship in feathers (text input landed for 0.19; number input landed for 0.19; menu landed for 0.19). The widget catalog grows in lockstep with editor needs, not with general-game-UI demand.
- **AccessKit cadence.** Feathers HEAD depends on AccessKit 0.24; 0.18.1 depended on 0.21. Mid-Bevy-release AccessKit major bumps will continue to drive breaking changes for feathers consumers. This is the same drift Buiy treats as an open question ([README.md § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)).
- **Game-UI applicability.** The "Feathers _can_ be used in games, but that is not its motivating use case" framing has not softened in any 0.17 → 0.19-rc release note. The signals continue to point at tooling/editor consumption, not games.

## Sources

- PR #19730 — `https://github.com/bevyengine/bevy/pull/19730`.
- Discussion #16900 — `https://github.com/bevyengine/bevy/discussions/16900`.
- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- Bevy 0.18 release notes — `https://bevy.org/news/bevy-0-18/`.
- Bevy Foundation page — `https://bevy.org/foundation/`.
- PR #20158 (BSN draft) — `https://github.com/bevyengine/bevy/pull/20158`.
- PR #24308 (AccessibleLabel) — `https://github.com/bevyengine/bevy/pull/24308`.
- Open A-UI issues mentioning feathers — `https://github.com/bevyengine/bevy/issues?q=is:issue+is:open+label:A-UI+bevy_feathers`.
