**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — DioxusLabs governance, funding, license, distribution, Cargo features

# Governance and distribution

## Organization

**DioxusLabs** is a US corporation, San Francisco-based, founded **2023** as part of Y Combinator's Summer 2023 batch. The organization owns and stewards:

- **dioxus** — the framework (this file's subject)
- **taffy** — the layout engine ([cross-reference `../taffy/`](../taffy/))
- **blitz** — the HTML/CSS engine for Dioxus Native
- **dioxus-cli** — the `dx` tool
- **manganis** — asset bundling for Rust apps
- **sledgehammer** — DOM mutation library (web target backend)
- **generational-box** — the Copy-arena that powers signals
- Various smaller subcrates (`dioxus-router`, `dioxus-fullstack`, `dioxus-desktop`, `dioxus-native`, `dioxus-mobile`, `dioxus-web`, `dioxus-ssr`, `dioxus-signals`, `dioxus-hooks`, etc.)

The GitHub org is at https://github.com/DioxusLabs (~36k stars on the flagship repo per upstream crawl 2026-05-22). The company website is https://dioxuslabs.com.

## Leadership

**Jonathan Kelley** (@jkelleyrtp) — founder, lead, only public corporate role-holder. Background per his LinkedIn / YC page: undergrad at Olin College of Engineering, prior employers Cloudflare and NASA (in that order), strong physics + hard-engineering self-description. Sole publisher of `dioxus` crate versions on crates.io (`published_by: jkelleyrtp`); crate ownership is `jkelleyrtp` + the `dioxuslabs:publish` team.

**Team size:** 4 (per YC company page, last updated at the company's profile snapshot). The team is not publicly named beyond Kelley in the org's public pages.

**Contributors:** the 0.7 release post thanks 150+ community contributors. Active core contributors include several long-time committers visible in the repo's commit graph; precise current full-time roster is not disclosed publicly.

## Funding (verified — correcting brief)

The brief said **"VC-backed FutureWei + Khosla Ventures Series A."** Verified against public funding records (Crunchbase, Tracxn, Y Combinator launch post, LinkedIn announcements):

| Round | Date | Investors | Amount |
|---|---|---|---|
| YC Seed | 2023 (Summer) | Y Combinator, Pioneer Fund | ~$500K total (Tracxn reports $520K) |
| Sponsorships | 2023 — | FutureWei, Satellite.im, GitHub Accelerator program | undisclosed |

**No Series A round is publicly disclosed.** **No Khosla Ventures involvement is found** in any public record (Crunchbase, Tracxn, Khosla's own portfolio listing). The pre-amble's "FutureWei + Khosla Ventures Series A" claim is unverified and appears to be incorrect.

**FutureWei** is a US-based Huawei research subsidiary. It is a *sponsor* (likely paying for or contributing engineering time toward) Dioxus development; FutureWei is also named as a Dioxus user (Airbus + ESA collision-avoidance system reportedly built with Dioxus, presumably under FutureWei's umbrella per available framing). The sponsorship-vs-investment distinction matters: sponsors don't have equity; their leverage is hiring-the-maintainer-to-fix-their-bugs.

**Satellite.im** is a peer-to-peer messaging product; it has used Dioxus and reportedly sponsors development.

## License

**MIT OR Apache-2.0** — standard Rust dual-license, applied to the flagship `dioxus` crate. Verified via crates.io API. This is the dominant Rust convention and matches Bevy's dual-license. (Compare to [Taffy](../taffy/README.md), which is single-license **MIT only** — unusual for the Rust ecosystem; DioxusLabs is inconsistent about license choice across its crates.)

Blitz is dual-licensed MIT OR Apache-2.0; the `stylo_taffy` glue crate uses **MPL-2.0** to interop with Servo's Stylo (which is MPL-2.0). Stylo itself is MPL-2.0. Embedders of Dioxus Native therefore have one MPL-2.0 dependency in their tree, which matters for some license-audit workflows but is not a blocker (MPL-2.0 is file-level copyleft, not viral like GPL).

## Distribution and versioning

- **crates.io** is the primary distribution channel. 54 published versions of `dioxus` from 2021-01-20 to present.
- **Patch cadence in 0.7:** roughly monthly. Nine patch releases between 2025-11-06 and 2026-05-08 (see [`history.md`](history.md)).
- **Minor cadence:** roughly annual. 0.4 (2023) → 0.5 (Mar 2024) → 0.6 (Dec 2024) → 0.7 (Oct 2025) → 0.8 in flight (May 2026).
- **No semver stability promise pre-1.0.** Like most Rust UI frameworks, breaking changes can occur between minor versions. The 0.5 release notes explicitly call out that significant API churn occurred between 0.4 and 0.5.
- **MSRV (minimum supported Rust version)** for 0.7.9: **1.83.0**, Edition 2021.

## Cargo features

The `dioxus` umbrella crate offers feature flags to control which backend(s) compile in. Per the 0.8.0-alpha.0 metadata:

| Feature | What it pulls in |
|---|---|
| `default` | `launch` + `devtools` + `logger` + `lib` |
| `desktop` | `dioxus-desktop` (Webview backend) |
| `web` | `dioxus-web` |
| `mobile` | `dioxus-mobile` |
| `native` | `dioxus-native` (Blitz/WGPU; experimental) |
| `fullstack` | server-rendering + hydration |
| `router` | `dioxus-router` |
| `document` | document elements (`Title`, `Meta`, `Link`, `Stylesheet`) |
| `asset` | `manganis` + asset resolution |
| `devtools` | hot-reload integration |
| `logger` | log subscriber |
| `axum` | server side bindings via Axum |
| `liveview` | server-driven UI over WebSocket |

App authors typically pick one of `web` / `desktop` / `mobile` / `native` / `fullstack` plus `router` plus `devtools` (during dev).

## RFC / design process

Dioxus does not run a formal RFC process. Design decisions live in:

- **GitHub Discussions** on the main repo
- **Blog posts** at dioxuslabs.com/blog (per-release)
- **Discord** (linked from the website)

There is no equivalent to Rust's RFC repo or Bevy's `docs/specs/` discipline. Major releases (0.5, 0.6, 0.7) are introduced via long-form blog posts that retrospectively explain the design choices; pre-release deliberation is mostly informal.

This is a difference from Bevy's lightweight-RFC pattern ([`prior-art/bevy-ui/lessons.md` § "Avoid" — "Lightweight RFC process"](../bevy-ui/lessons.md)) — Dioxus's deliberation is even more informal, and the 22-month BSN saga that Bevy critiques can be matched by Dioxus's multi-quarter Blitz development cycle, which similarly lived in scattered issues / PRs / Discord channels before culminating in 0.7's release post.

## Bus-factor

Kelley is the lead, sole crates.io publisher, founder, and corporate face. The team is 4 people total per YC's profile; none of the other three are publicly named in their org-level role. Compared to Taffy (single non-employed maintainer Nico Burns), Dioxus has more redundancy because YC-backed companies tend to have a 2-of-N bus-factor floor, but the public-facing exposure is concentrated in one person. The mitigating factor: YC + GitHub Accelerator + 36k stars + active customer base (Airbus, ESA, FutureWei) collectively make abandonment unlikely in any short timeframe.

## Implications for Buiy

- **Don't trust the brief on funding without verification.** The pre-amble's "FutureWei + Khosla Ventures Series A" claim was unverified and is materially wrong. For load-bearing-dep folders, treat every funding/governance claim as needing direct verification against crates.io publisher data, Crunchbase, YC pages, etc. (This is the Iroh-pre-1.0-verification lesson from the `researching-prior-art` skill, applied to organization-level facts.)
- **DioxusLabs as Taffy's steward is significant but not dispositive for Buiy.** Buiy depends on Taffy (`buiy-layout-design`); DioxusLabs owns the Taffy repo at the admin level but does not fund its maintenance line. Nico Burns is the day-to-day Taffy maintainer and is **not** a DioxusLabs employee. The Buiy fork-Taffy-in-a-crisis contingency ([`../taffy/lessons.md` § "Avoid" — "Bus-factor"](../taffy/lessons.md)) is unchanged by DioxusLabs's existence.
- **MIT OR Apache-2.0 is the right choice.** Buiy should match (foundation does not yet specify license; this is a strong default).
- **The informal-deliberation cost is real.** Dioxus's lack of a public design-spec corpus is one reason that adopting techniques *from* Dioxus is harder than it should be — there is no `docs/specs/`-equivalent to read. Buiy's docs discipline (`docs/specs/` + `docs/plans/` + `docs/prior-art/` per the `organizing-buiy-docs` skill) is a deliberate improvement on the Rust UI ecosystem norm.

## Sources

- crates.io API for `dioxus`: https://crates.io/api/v1/crates/dioxus
- crates.io API for `dioxus/0.7.9`: https://crates.io/api/v1/crates/dioxus/0.7.9
- crates.io owners endpoint: https://crates.io/api/v1/crates/dioxus/owners
- Y Combinator company page: https://www.ycombinator.com/companies/dioxus-labs
- Y Combinator launch post: https://www.ycombinator.com/launches/JBK-dioxus-labs-web-desktop-and-mobile-apps-with-one-codebase
- Tracxn (funding cross-check): https://tracxn.com/d/companies/dioxus
- Crunchbase (funding cross-check): https://www.crunchbase.com/organization/dioxuslabs
- Dioxus repo: https://github.com/DioxusLabs/dioxus
- Blitz repo: https://github.com/DioxusLabs/blitz
- Jonathan Kelley LinkedIn: https://www.linkedin.com/in/jonathan-r-kelley
- DioxusLabs GitHub org: https://github.com/DioxusLabs
- Cross-reference: [`../taffy/governance.md`](../taffy/governance.md)
