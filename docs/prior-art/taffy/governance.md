**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — stewardship, maintainers, funding, and release governance

# Taffy — governance

Taffy is a small-team project with one primary technical lead. Stewardship is DioxusLabs (the org owns the repo and the crate), but the day-to-day maintenance is concentrated in one engineer with a handful of recurring reviewers.

## 1. Steward: DioxusLabs

The repository lives at [`github.com/DioxusLabs/taffy`](https://github.com/DioxusLabs/taffy). DioxusLabs is the venture-backed parent org for the Dioxus framework; its lead is Jonathan Kelley (`@jkelleyrtp`), the founder. Taffy moved to DioxusLabs at the rename event on 2022-06-10 (see [history.md § 3](history.md#3-taffy--the-rename-june-2022)).

The org has the GitHub admin bit on the repo. It does not provide a separate engineering budget for Taffy; the maintenance happens via the three crate owners (see § 2).

## 2. Primary maintainers

Three named crate owners (verified [crates.io/crates/taffy/owners](https://crates.io/api/v1/crates/taffy/owners)):

- **Nico Burns (`@nicoburns`)** — primary technical lead. Independent (not employed by DioxusLabs). Personal site `nicoburns.com`. The author of the load-bearing work since 2023: CSS Grid (0.3), block layout (0.4), the trait restructure (0.6), `CacheTree` split (0.7), `CompactLength` (0.8), named grid lines (0.9), float/direction (0.10). Also the author of the WPT-importer scripts and most of the issue tracker triage. Reviews the majority of merged PRs.
- **Alice Cecile (`@alice-i-cecile`)** — Bevy lead UI maintainer. Bridge from the Bevy UI team since the `stretch2 0.4.3` handoff. Reviews API-breakage PRs from a downstream-impact perspective; not the day-to-day technical lead.
- **Jonathan Kelley (`@jkelleyrtp`)** — Dioxus founder, original fork author from Stretch. Owns the rights but is not the day-to-day maintainer. Largely absent from the recent issue tracker.

There is **no `CODEOWNERS` file** (`/.github/CODEOWNERS` 404s). Approvals are by convention, not enforcement.

## 3. Funding model

**Taffy is unfunded as an independent line item.** No GitHub Sponsors integration on the repo. No Open Collective. No `funding.json`. No corporate sponsorship banner.

Indirect support paths:

- **DioxusLabs is YC S23-seed-funded** (~$500K) + Pioneer Fund + GitHub Accelerator (per Crunchbase + Tracxn + YC company page). FutureWei is listed as a sponsor, not an equity investor. Taffy itself has no separate funding line. See `../dioxus/governance.md`.
- **Nico Burns** is independent and supports the work via consulting + sporadic GitHub Sponsorship on his personal account (`github.com/sponsors/nicoburns`). The Taffy repo does not surface this.
- **Bevy Foundation** indirectly underwrites Alice Cecile's time; her Taffy review work is a side-quest.
- **Servo / Igalia** — Servo has migrated to Taffy as its layout engine (via `servo-layout 0.10`). Igalia (the company underwriting Servo) has not, as of 2026-05, announced direct Taffy sponsorship, but Servo's correctness needs drive a substantial fraction of Burns's review backlog.

Net: the funding posture is "load-bearing on one independent maintainer." This is a known bus-factor concern; see [critiques.md § 6](critiques.md#6-bus-factor).

## 4. License decision: MIT

Single-license **MIT**, throughout the Stretch → stretch2 → Taffy lineage. See [history.md § 7](history.md#7-license-decision). This is *not* MIT OR Apache-2.0; the brief expected MIT OR Apache-2.0, the verified ground truth is MIT-only.

Practical consequences for embedders:

- **No patent grant.** Corporate consumers nervous about MIT-only (a sometimes-issue for legal review at large companies) have to escalate. The Rust convention is dual MIT/Apache-2.0, so Taffy is an outlier.
- **License-compatibility check:** Apache-2.0 downstream projects can incorporate MIT-licensed Taffy without issue. The asymmetry only matters for projects that want patent-grant coverage.
- **No relicensing pathway** — Visly's 2018 copyright remains in the LICENSE file; relicensing would require the three current crate owners + Visly's successor (none) to agree. Nobody is asking.

## 5. Triage cadence

The issue tracker (89 open issues, 25 open PRs as of 2026-05) is triaged by Burns and Cecile. There is no formal triage SLA; visible patterns from the public tracker:

- **"good first issue" health is low.** A handful of issues carry the label but they have aged out — they're labelled as approachable but actually require deep CSS-spec knowledge.
- **"controversial" label exists** (e.g. issue #308 "Support Morphorm/Subform layout", issue #911 "More correct caching logic") — flagged as requiring heightened review.
- **"performance" label** carries five active issues including #917 ("Improve layout recalculation performance for small scoped changes"), #911, #685.
- **Roadmap** lives in issue #345 "Roadmap" — the public list of long-term goals. Subgrid (#468), masonry (#910), writing-mode (#752), anchor positioning (#703) are all tracked there.
- **Most PRs sit open 1-4 weeks**; merge cadence is bursts around release dates. There is no continuous merge stream.

## 6. Release governance

No release manager rotation; Nico Burns cuts every release. The version-number policy is informal:

- **Minor bump = breaking change.** Each `0.x.0` since 0.4 has carried API breakage (the trait restructures in 0.6/0.7, the `CompactLength` change in 0.8, the `CheapCloneStr` generic in 0.9, the MSRV bump in 0.10). Downstream embedders read the CHANGELOG and accept breakage per minor.
- **Patch releases for bugfixes.** Mostly Bevy-driven; the 0.3 line carried 19 patches over 10 months.
- **No deprecation policy.** API surface that's removed is removed; the CHANGELOG is the migration guide.
- **No security policy.** No `SECURITY.md` (as of 2026-05). For a layout engine the surface is small — there's no untrusted-input parsing path; the `parse` feature is opt-in and parses author-trusted CSS strings. But the absence of a documented policy is itself a posture statement.

Experimental versions (`0.11.0-experimental-cache-fix.3` etc.) are published on demand to crates.io when correctness work needs cross-embedder feedback (specifically Blitz, which pins them exact). These do not get LTS treatment.

## 7. Bus factor

**One.** Nico Burns can absorb a vacation; he cannot absorb a multi-month absence without the project visibly stalling. This was also true of Stretch (Emil Sjölander was the bus-factor) and stretch2 (Jonathan Kelley was the bus-factor); the pattern has held across three stewardships.

The mitigating factors:

- Code is small (~15k LoC excluding tests and benches). A new maintainer could ramp.
- Issue tracker is well-organized; the roadmap (#345) is explicit.
- Crate ownership is shared across three names, so the crate cannot be lost to GitHub-account-death.
- Bevy + Blitz are large enough downstreams that *somebody* would fork if Burns disappeared.

Buiy's exposure to bus-factor is the same as Bevy's: full. The fork-if-necessary contingency is realistic but expensive (months of ramp before a new maintainer ships fixes at Burns's quality).

## 8. Community surfaces

- **GitHub Discussions** ([taffy/discussions](https://github.com/DioxusLabs/taffy/discussions)) — active for Q&A; recent topics include "Grid Template Areas", "How to hide a node in layout?", "Do `Style` properties matter for leaf node?", and "A Declarative + Json Hot-Reloadable Companion Library for Taffy". Average ~1 thread per week.
- **Discord** — Taffy lives in the Dioxus Discord (no dedicated Taffy server). Channel `#taffy`.
- **No mailing list, no IRC, no Matrix.** All real-time community is Discord-shaped.

## Sources

- crates.io owners endpoint: https://crates.io/api/v1/crates/taffy/owners
- Taffy main Cargo.toml (verified MIT, three authors): https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml
- Taffy issue #345 (Roadmap): https://github.com/DioxusLabs/taffy/issues/345
- Taffy discussions: https://github.com/DioxusLabs/taffy/discussions
- DioxusLabs org: https://github.com/DioxusLabs
- Nico Burns site: https://nicoburns.com
- Bevy Foundation: https://bevy.org
- Sibling: [history.md](history.md), [critiques.md](critiques.md), [ecosystem.md](ecosystem.md)
