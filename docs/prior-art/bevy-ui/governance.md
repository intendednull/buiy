**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — governance, maintainers, RFC process, funding

# Governance

bevy_ui is governed under the Bevy Foundation umbrella. The Foundation is a **Washington-state non-profit with 501(c)(3) public-charity status at the federal level** — donations are tax-deductible for US donors. The Foundation describes its mission as "promote, protect, and advance the free and open source Bevy Engine." Carter Anderson (cart) leads as **President and Project Lead**, and is also Interim Treasurer.

The repository carries **46.2k+ GitHub stars** and has 11,495+ commits on `main` (counted 2026-05-22). The orchestrator pre-amble cited 38k stars and "13+ years old"; both numbers were wrong — Bevy was first published on crates.io 2020-08-10, making it **~6 years old**, and stars are closer to **46k** than 38k.

## Board of Directors (Bevy Foundation)

Five-person board, per the Bevy Foundation page (fetched 2026-05-22):

- **Carter Anderson (cart)** — President; Interim Treasurer; founder.
- **Alice Cecile (alice-i-cecile)** — Secretary; UI-adjacent maintainer.
- **François Mockers (mockersf)** — Director; release manager.
- **Robert Swain** — Director; rendering area.
- **James Liu** — Director.

Per current policy: "Bevy Maintainers are offered Director status through board vote." This means the maintainer set and the board overlap by design.

## Subject-Matter Experts (UI area)

Bevy uses an **SME ("Subject-Matter Expert") model** for review authority. The UI SMEs as of 2026-05-22, inferred from recent commit / PR activity:

- **@alice-i-cecile** — most UI PR reviews; coordinates Vision-for-Bevy-UI direction.
- **@viridia** — author of issue #17644 (bevy_a11y BSN-unfriendliness), discussion #16900 (Standard Headless Widgets), PR #19366 (core button), PR #19730 collaborator on Feathers. De-facto lead on widget primitives.
- **@ickshonpe** — highest-volume UI committer 2025-2026. Authored bevy_feathers PR #19730, scrolling PR #20093, UI gradients PR #18139, cosmic-text 0.16 upgrade PR #22308.
- **@nicoburns** — Taffy upstream maintainer; not a Bevy SME but the de-facto layout-pipeline reviewer on PRs that touch Taffy version bumps.
- **@bushrat011899** — UI / picking integration reviewer.

The SME set is not formally enumerated on a public page; this list is reconstructed from PR review history. Buiy spec authors should not assume the set is stable across Bevy minor releases.

## RFC process

Bevy does **not** use a formal RFC repository (unlike Rust's `rust-lang/rfcs`). Major design discussions happen through:

1. **GitHub Discussions** for early-stage proposals. Examples: BSN tracking (#14437), Standard Headless Widgets (#16900), 10 Challenges for Bevy UI Frameworks (#11100).
2. **Long-form draft PRs** for designs with a working prototype attached. Example: BSN PR #20158 — opened in draft on 2025-07-16 explicitly as "a public experimentation phase," not for merge.
3. **HackMD documents** for vision-level direction. Example: alice-i-cecile's "A Vision for Bevy UI" (https://hackmd.io/@bevy/HkjcMkJFC) — note the document itself states it "lacks full maintainer consensus."
4. **Discord working groups** for active design. cart explicitly mentions a "dedicated Discord channel" was set up for BSN collaboration.

This is lightweight by design — Bevy is small enough that a formal RFC process is overhead — but it has costs: design state lives across multiple platforms (issue, PR, Discord, HackMD), and the absence of a canonical RFC log makes it hard for downstream projects (Buiy included) to audit the history of a decision. The Buiy docs/specs/plans tree is in part a response to this — Buiy maintains its own canonical-doc log instead of relying on Bevy's distributed one.

## Funding

The Bevy Foundation funds:

- **Maintainer hiring** — paid technical and social leadership for development.
- **Infrastructure** — website, CI, docs hosting.
- **Operational costs** — non-profit compliance, administrative overhead.

Funding sources: community donations (one-time and recurring), and corporate sponsorships. Exact figures are not on the Foundation page. The 501(c)(3) status was obtained per the Foundation blog post `https://bevy.org/news/bevy-foundation-501c3/` (date not captured).

## Roadmap process

Bevy's roadmap is **cart-driven** at the strategic level (BSN, Required Components, the editor) and **milestone-driven** at the tactical level. The current open milestone is **0.19**, with PRs tagged against it on GitHub.

There is no published "Bevy 1.0" date. cart has stated publicly that the editor (which depends on Feathers, which depends on `bevy_ui_widgets`, which depends on BSN landing) is the main 1.0 blocker; he has not committed a year.

## Implications for Buiy

- **Bevy's lightweight RFC process is part of why Buiy maintains its own canonical doc log.** No single page describes the decision history behind, e.g., the cosmic-text adoption or the bevy_a11y design. Buiy's [docs/specs/](../../specs/) folder is the analog for Buiy decisions, so downstream consumers don't have to reconstruct them from Discord.
- **The UI SME set is small (~3–5 people) and overlaps with the board.** This means UI-area decisions move quickly when there is consensus and slowly when there isn't (cf. BSN at ~22 months from #14437 to a still-unmerged PR). Buiy should not assume a Bevy-side decision on any UI direction will land in the next minor.
- **Bevy Foundation is a real legal entity.** Buiy depends on bevy_ui's substrate (Taffy, cosmic-text, AccessKit, bevy_picking, the render graph) but is *not* a Foundation project; if Buiy wants long-term maintenance protection it should establish its own legal structure independently.

## Sources

- Bevy Foundation page — `https://bevy.org/foundation/`.
- Bevy Foundation 501(c)(3) announcement — `https://bevy.org/news/bevy-foundation-501c3/`.
- Bevy repository (star count, commit count) — `https://github.com/bevyengine/bevy`.
- cart's GitHub profile — `https://github.com/cart`.
- "A Vision for Bevy UI" by alice-i-cecile — `https://hackmd.io/@bevy/HkjcMkJFC`.
- BSN tracking discussion #14437 — `https://github.com/bevyengine/bevy/discussions/14437`.
- BSN PR #20158 — `https://github.com/bevyengine/bevy/pull/20158`.
- Issue #17644 — `https://github.com/bevyengine/bevy/issues/17644`.
- Discussion #16900 Standard Headless Widgets — `https://github.com/bevyengine/bevy/discussions/16900`.
