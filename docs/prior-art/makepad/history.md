**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — history of Rik Arends's live-coding thesis from Cloud9 (2010s) through Makepad genesis (2019) to 1.0 (2025-05)

# History

Makepad is the third chapter of **Rik Arends's** career-long pursuit of live-coding IDEs. The first chapter was **Cloud9 IDE** (2010–2016, acquired by Amazon, became AWS Cloud9 in 2016). The second was an unreleased browser-based IDE. The third is Makepad — a Rust + GPU-rendered live-editable IDE plus the framework that powers it.

## Pre-Makepad: Rik Arends's lineage

- **Cloud9 IDE (2010–2016).** Browser-based collaborative IDE, written in JavaScript. Acquired by Amazon in 2016, rebranded as **AWS Cloud9** (now in maintenance / sunset within AWS).
- **Live-coding thesis.** Throughout the 2010s Arends championed the idea that compilation latency is the primary IDE friction, and that real-time visual feedback — including live shader editing — is the right way to reduce it. Public talks emphasize this.
- **Pre-Makepad prototypes.** Public posts on Arends's social channels from 2017–2018 show experiments with browser-based GPU IDEs predating the Rust shift.

The Makepad GitHub repo was created on **2019-02-20**. The earliest tag — `pre-alpha` — was published on **2019-12-08** as a GitHub Release marked prerelease. From there, no further GitHub Releases were tagged (the project tracks `dev` branch as the working line and uses crates.io publishing rather than GitHub releases for milestones).

## Crates.io publication history

Per the crates.io API for `makepad-widgets`:

| Version | Published | Notes |
|---|---|---|
| 0.3.0 | 2022-10-20 | First crates.io publication (`makepad-widgets` crate created) |
| 0.4.0 | 2022 | (date in API; intermediate iteration) |
| 0.5.0 | (intermediate, 2022/2023) | |
| 0.6.0 | **2023-09-21** | Last release before the long pre-1.0 gap |
| 0.9.0 | **2025-05-12** | The 1.0 lead-up |
| 0.9.1 | **2025-05-12** | Same-day patch |
| **1.0.0** | **2025-05-13** | The 1.0 release. Five days after the 1.0 lead-up version |

The **20-month gap** between 0.6.0 (Sept 2023) and 0.9.0 (May 2025) tells the real story: the project worked toward 1.0 in `dev` for nearly two years without a crates.io release. Whatever 1.0 means for Makepad, the crates.io versioning is misleading — `dev` branch was the actively-developed surface; the crates.io 0.6.0 → 0.9.0 → 1.0.0 versions were release-candidate snapshots of `dev`, not incremental published progress.

## Co-architects and timeline

- **Eddy Bruël** (Dutch spelling: `Bruël`) — public Makepad co-developer. Bruël has Mozilla / Firefox JavaScript engine history (SpiderMonkey-area work). Joined Makepad early; GitHub `eddybruel` profile shows the `ungit` repo (Rust) and continues to contribute to Makepad core. Top contributor by historical commits in the original codebase generation.
- **Sebastian Michailidis** (Germany) — public Makepad co-developer. GitHub handle is `okapii`. **#1 by raw contribution count** (154 commits per GitHub contributors API) on the current `dev` branch. Twitter `@SebMichailidis` / Bluesky.

The crates.io publisher account is `makepaddev` — a single bot / shared account used for all seven crate releases. The GitHub repo organization is `makepad/` (not under any of the three architects' personal accounts), which is the right shape for a multi-author project.

## Project Robius — the downstream community (2023–)

Around 2023, an independent community formed to build cross-platform apps on Makepad. **Project Robius** (`github.com/project-robius`) is led by **Kevin Boos** (Principal Architect at **Futurewei Technologies**, PhD from Rice University, creator of Theseus OS), with multiple non-Makepad-core contributors. Robius's mission: "fully open-source, decentralized, community-driven effort to enable multi-platform application development in Rust" with a mobile-first emphasis.

Robius outputs that exist as of folder-write:

- **Robrix** — Matrix chat client (the most visible Makepad app outside of Makepad Studio itself). v1.0.0-alpha.1 released 2026-05-05. 448 GitHub stars. Presented at Rust China Conf 2025, GOSIM China 2024, GOSIM Europe 2024. Maintained primarily by Kevin Boos.
- **makepad_wechat** — WeChat-like app port (28 stars).
- **makepad_wonderous** — Makepad port of the Wonderous app (4 stars).
- **android-build** — Compile Android/Java files during Rust compilation (general-purpose tooling, used by Robrix).
- **robius** — Multi-platform abstractions for platform APIs (battery, notifications, network state, etc.).
- **matrix-rust-sdk fork** + **ruma fork** — Matrix protocol stack for Robrix.

The Robius / Makepad relationship is functionally similar to: a Rust UI framework (Makepad core team) + an ecosystem-driving downstream community (Robius / Futurewei). Buiy's analogous shape would be: Buiy core team + community apps built on Buiy. The Robius community's existence is **load-bearing for Makepad's adoption story** — without Robrix as a visible "real app built on Makepad," the framework's relevance would be narrower.

## The 1.0 release (2025-05-13)

The 1.0 was published quietly on crates.io with no announcement post on the project site, no major blog post, no GitHub release tag. The 1.0 is essentially a stable-version commitment on the `makepad-widgets` API surface. Robrix's later release of `v1.0.0-alpha.1` on 2026-05-05 (a year after Makepad 1.0) suggests the Robius community was waiting on the Makepad 1.0 milestone before stabilizing their own API.

The README still describes Makepad as "an AI-accelerated application development environment" — language that postdates the 1.0 release and signals a 2026-era pivot toward AI-driven authoring (the project site `makepad.dev` "Makepad is loading" SPA shell suggests an AI-integrated demo in progress).

## What 1.0 means for Makepad

Conservatively: **the `makepad-widgets` 1.0 crate is a stable API marker for the widget catalog**. It does **not** mean Makepad has reached:

- Documentation maturity (5.92% documented per docs.rs).
- Accessibility (no AccessKit; #196 open).
- WCAG conformance (not in scope at all).
- Cross-editor tooling (Makepad Studio dependency).
- Mass adoption (16,974 lifetime downloads).
- Foundation-style governance (single de facto core team).

This matters for Buiy's comparison framing: "Makepad reached 1.0" is technically true but should not be over-read as "Makepad solved the problems Buiy is solving." It solved the problems Makepad set out to solve — a live-editable GPU-rendered cross-platform UI framework with a custom DSL. That's a different problem set than "comprehensive WCAG 2.2-AA-conformant UI library for Bevy." See [`README.md`](README.md) honest-assessment section.

## Implications for Buiy

- **Live-coding-IDE thesis is real product-grade — but it's not a UI library thesis.** Arends's commitment to live editing is what makes Makepad's hot-reload story shine. Buiy's BSN hot-reload sub-spec ([`buiy-bsn-integration-design`](../../specs/2026-05-07-buiy-foundation/README.md)) inherits the validation: real-time-reload of UI source is a 6+-year-proven product. The DSL-as-source-of-truth choice is a separable bet.
- **Founder competence + small team + long arc.** Six years between repo creation (2019-02) and 1.0 (2025-05) with a 3-architect de facto core team. Buiy should not assume a similar shape is fast; the analog timeline for Buiy is years, not quarters. Foundation README § 5 open question on platform support staging implicitly acknowledges this.
- **Downstream community is load-bearing for adoption.** Robius / Futurewei made Makepad visible at conferences (Rust China Conf 2025, GOSIM 2024) in a way the Makepad core team's own efforts wouldn't have. Buiy's own community-cultivation strategy (currently implicit in foundation README) should explicitly plan for a Robius-equivalent — third-party app efforts that justify the framework's existence to a broader audience.
- **The "AI will do a11y" posture postdates Arends's earlier views.** The README's 2026-era "AI-accelerated" framing is in tension with the 2023 community a11y request (#196). The "AI replaces accessibility infrastructure" framing is a *recent* Makepad posture, not an unchanging one. Buiy should explicitly reject this framing in its own messaging — accessibility is infrastructure, not a feature AI can ship later. See [`critiques.md`](critiques.md).

## Sources

- Makepad repo creation date / metadata: GitHub API (`repos/makepad/makepad`)
- Crates.io `makepad-widgets` version history: https://crates.io/api/v1/crates/makepad-widgets
- Rik Arends Cloud9 IDE: archived materials (Cloud9 founded ~2010, acquired by AWS 2016)
- Eddy Bruël profile: https://github.com/eddybruel
- Sebastian Michailidis profile: https://github.com/okapii
- Kevin Boos / Project Robius: https://github.com/project-robius, https://github.com/kevinaboos
- Robrix: https://github.com/project-robius/robrix
- AWS Cloud9 retrospective (acquisition lineage): publicly documented
- Issue #196 (accessibility): https://github.com/makepad/makepad/issues/196
- Sibling files: [`README.md`](README.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`critiques.md`](critiques.md)
