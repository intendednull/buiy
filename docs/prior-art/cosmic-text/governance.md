---
**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — stewardship, funding, release cadence, contribution model
---

# Governance

cosmic-text is a **company-stewarded open-source project**. System76 funds the work as part of building the COSMIC desktop and Pop!_OS. There is no foundation, no formal RFC process, no semver guarantees beyond pre-1.0 Cargo conventions. Contributions arrive via GitHub PRs; maintainer decisions land via merge.

Cross-links: [history.md](history.md) for the timeline, [critiques.md](critiques.md) for the consequences of single-company stewardship.

## Stewardship

The crate's `authors = [...]` field through 0.19.0 names a single person:

> **Jeremy Soller `<jeremy@system76.com>`**

Jeremy Soller is the System76 principal engineer and the primary maintainer of Redox OS, in addition to founding cosmic-text and the broader COSMIC desktop work. He shows up as committer on most cosmic-text releases (recent committed-by line on the 0.18 / 0.19 cycle).

Other frequent committers on recent cosmic-text history (verified via commit list, not via stated CODEOWNERS — the repo has none):

- **valadaptive** — landed PR #417 (rustybuzz → HarfRust migration, 2025-09-09) and surrounding cleanup. Appears to be a regular external contributor.
- **benstigsen** — recent HarfRust update commits (December 2025).
- **jackpot51** — Jeremy Soller's GitHub handle. Primary committer.

There is no public list of "maintainers" or "core team" — the repo has no `MAINTAINERS.md`, `CODEOWNERS`, or `GOVERNANCE.md`. Authority is inferred from commit access and review activity. The pre-amble's named candidates "Michael Murphy, Eduardo Flores" were not verified as cosmic-text-specific committers — they are System76 staff who work on broader COSMIC components.

## Commercial model

System76 funds cosmic-text out of its hardware-and-software bundle revenue. The company sells Linux laptops and desktops; Pop!_OS (and the future COSMIC desktop replacement) ships on them. The text engine is essential to the desktop and the apps that ship on it. System76 has no monetization of cosmic-text itself — no commercial license, no support contract, no sponsorship-required tier. Permissive licensing (MIT OR Apache-2.0) signals the intent that downstream consumers (Iced, Bevy, anyone) can use it without obligation.

This commercial structure resembles `redox-os`'s structure (System76 also funds Redox via Jeremy Soller's time) and `rustybuzz` / `fontdb` / `usvg`'s structure (RazrFalcon's pure-Rust font crates ecosystem, several of which cosmic-text depends on). It does NOT resemble Linebender's Parley project (foundation-style governance, multi-org steering) — see [ecosystem.md](ecosystem.md) for the Parley contrast.

## Funding

- System76 employs the primary maintainer (Jeremy Soller) for time on cosmic-text indirectly: the work is part of building COSMIC.
- No GitHub Sponsors page exists for the cosmic-text repo specifically. The COSMIC project at large solicits donations indirectly via System76 product sales.
- No grants, no NLnet funding, no Sovereign Tech Fund involvement that we could verify (none mentioned in repo or commits).

## Licensing

**MIT OR Apache-2.0** dual-licensing — the Rust-ecosystem default. The repo's `LICENSE-MIT` and `LICENSE-APACHE` files are both present. Contributors agree to dual-licensing via standard Apache-2.0 §5 inbound terms; no CLA is required. This is the lowest-friction permissive arrangement and matches Iced, Bevy, and Buiy.

## Release cadence

Verified from crates.io publish dates (see [history.md](history.md) for the full table):

- Pre-0.14: rough 3–6 month cadence with hot-fix point releases between.
- 0.14 → 0.18 (March 2025 → February 2026, ~11 months): **six minor releases**, ~7–10 week minor cadence with same-day or same-week patch releases when regressions surface.
- 0.18 → 0.19 (February → April 2026): two-month gap.
- 0.18.1 + 0.18.2 both published 2026-02-20, same day as 0.18.0 — a same-day double-hotfix pattern that indicates the maintainer is responsive but also that 0.x releases sometimes ship with surface defects the embedders find immediately.

Pre-amble's "monthly-ish to quarterly" lines up: the 2025–2026 cadence is closer to monthly minors, with patch releases bunched around the minor.

## Issue triage

The repo uses **GitHub Issues** with no labels-policy publicly documented. Issue triage is informal:

- The maintainer (jackpot51) closes resolved issues as PRs land.
- There's no auto-stale bot. Old issues (e.g. #10 IME, open since 2022-10-24) stay open indefinitely.
- ~98 open issues as of May 2026 (`gh api repos/pop-os/cosmic-text/issues` showed 98 open issues at the time of folder authoring).
- The most-asked-for missing features (IME, vertical writing, hyphenation) have either an old open tracking issue (IME #10) or no tracking issue at all (vertical writing, hyphenation — search returned no results).

## RFC / design process

**None formal.** Substantial design decisions are discussed in the PR thread that lands them, not in a separate proposal step. Examples:

- The `Renderer` trait introduced in 0.16.0 — discussion lives in the PR, not an RFC.
- The HarfRust migration (PR #417) — design discussion is in the PR thread `valadaptive` opened; no separate RFC.

For a small library this is fine; for a load-bearing dependency this means **downstream consumers (Buiy included) cannot easily predict breaking changes ahead of release**. Watch the PR queue, not a roadmap document.

## Contribution model

- PRs welcome via standard GitHub flow.
- No CLA. Apache-2.0 §5 inbound terms cover contribution licensing.
- Code review is single-maintainer (jackpot51) with occasional second-pair from frequent contributors (valadaptive).
- No formal testing-coverage policy; the Universal Declaration of Human Rights ~500-language corpus is the canonical integration test, but unit-test coverage of internal modules is light.

## Implications for Buiy

- **Bus factor.** Effectively one company. Jeremy Soller is the keystone. If System76 deprioritized COSMIC or Jeremy moved on, cosmic-text would slow significantly. Buiy depends on this; the [critiques.md](critiques.md) file flags the single-steward risk.
- **No deprecation window.** Pre-1.0 + no formal release policy means a 0.20 release can rename `Action` variants without warning. Buiy's MSRV / version-pin discipline matters more than for a foundation-governed dep.
- **No public roadmap.** Buiy's `buiy-text-rendering-design` sub-spec should pin a cosmic-text version explicitly rather than assuming features will land on a known schedule.
- **Permissive licensing matches Buiy.** No licensing friction; MIT-OR-Apache-2.0 stacks with Buiy's chosen license.

## Sources

- `Cargo.toml` at HEAD — https://github.com/pop-os/cosmic-text/blob/main/Cargo.toml (authors field)
- PR #417 (HarfRust migration, valadaptive) — https://github.com/pop-os/cosmic-text/pull/417
- Recent commit history — https://github.com/pop-os/cosmic-text/commits/main
- crates.io publish dates — https://crates.io/crates/cosmic-text/versions
- System76 / COSMIC project — https://system76.com/cosmic
- Redox OS (Jeremy Soller, primary maintainer parallel project) — https://www.redox-os.org/
