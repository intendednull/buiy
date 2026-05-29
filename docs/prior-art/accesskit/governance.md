**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — stewardship (Pneuma Solutions + community contributors), funding posture, release cadence, license decision, relationship to W3C ARIA / ACCNAME

## Stewardship: independent project, company-adjacent

AccessKit is an open-source project under the [`AccessKit` GitHub org](https://github.com/AccessKit/accesskit). It is **not formally owned by a company**; the org is named after the project, not after a parent. The project's primary architect and most active maintainer is Matt Campbell, who is also CTO and co-founder of Pneuma Solutions ([Pneuma Solutions about page](https://pneumasolutions.com/about/)).

This is a "company-adjacent" stewardship model: AccessKit is developed in Campbell's open-source capacity, with Pneuma Solutions presumably affording him the time to do it. There is no public statement that Pneuma is the formal sponsor; AccessKit's website does not list a corporate steward. Treat the relationship as **load-bearing-but-informal** — if Pneuma's posture toward AccessKit changed (acquisition, founder departure, refocus), there is no contractual continuity for the project.

## Pneuma Solutions

- **Founded:** 2020 by Matt Campbell (CTO) and Mike Calvo (CEO), in a re-formation of their earlier collaboration at Serotek Corporation (Calvo founded Serotek in 2001).
- **Focus:** Cloud-based accessibility products — Remote Incident Manager (remote support tooling), Scribe (document remediation), Sero (cloud screen reader), DocuScan Plus.
- **Size:** Small company. The about page describes the team as "most of us are blind or visually impaired ourselves" and emphasises lived-experience design. Public LinkedIn data is unclear (one of the LinkedIn entries for "Pneuma Solutions" turned out to be an unrelated project-management firm in Tampa with the same name).
- **Matt Campbell's prior path:** Late 1990s onward in Linux accessibility, then Serotek's `System Access to Go` browser-distributed screen reader, then Microsoft on the Windows Accessibility Team. AccessKit started **2021**.

**Correction vs the brief preamble:** the preamble described Campbell as a "former NVDA developer." Per [NV Access's about page](https://nvaccess.org/about-nv-access/) NVDA was created by Michael Curran (2006) and co-led by James Teh. Campbell's screen-reader experience is Serotek + Microsoft, not NVDA. This folder treats Campbell as "ex-Serotek, ex-Microsoft, current Pneuma Solutions CTO" per the Pneuma about page, which is the authoritative source for his bio.

## Other regular contributors

The [AUTHORS file](https://raw.githubusercontent.com/AccessKit/accesskit/main/AUTHORS) lists four copyright holders:

- Matt Campbell — primary architect, most active maintainer.
- Arnold Loubriat — significant contributor (Linux/AT-SPI adapter work, ongoing platform-adapter maintenance).
- Google LLC — corporate-employee contributions, almost certainly the Chromium-derived schema code (the README states "Significant portions of AccessKit are derived from Chromium and are covered by its BSD-style license"). This is not the same as "Google sponsors AccessKit" — it's a copyright attribution for code Google's employees wrote that AccessKit absorbed.
- Leonard de Ruijter — contributor.

The README directs readers to "the revision history in source control" for the full contributor list. The AUTHORS file is intentionally short — a copyright record, not a credit roll.

## Funding

**No public funding announcement** for AccessKit by name (no Series-A, no foundation grant, no Sovereign-Tech-Fund disbursement tagged with AccessKit). The project's funding model is best characterised as:

- **Pneuma Solutions in-kind** (Campbell's time, presumably).
- **Corporate contributor labour** (Google's Chromium-team alumni contributing on company time, per AUTHORS).
- **Downstream-toolkit contributor labour** (egui's Emil Ernerfeldt, Slint's team, Linebender contributors patching adapters that affect their stacks).
- **GitHub Sponsors** — the [accesskit.dev](https://accesskit.dev/) site links to `github.com/sponsors/mwcampbell`. The exact tier amounts and disclosed sponsor list are not pinned in this folder; a refresh should query the GitHub Sponsors page directly.

There is **no AccessKit foundation** or formal funding entity.

## License

`MIT OR Apache-2.0` (dual-licensed, contributor's choice at adoption), with Chromium-derived portions carrying a BSD-style license — per the [`LICENSE-APACHE` / `LICENSE-MIT` / `LICENSE.chromium`](https://github.com/AccessKit/accesskit/blob/main/README.md) noted in the README's License section. This is the standard Rust-ecosystem choice and matches Buiy's prospective license (the `accessibility.md` spec doesn't pin a license but the Bevy ecosystem norm is the same dual licensing).

## Release cadence

**Roughly quarterly minor releases**, with the core `accesskit` crate at 39 published versions on crates.io ([crates.io](https://crates.io/crates/accesskit)). Recent verified cadence:

- 0.18.0 → 0.19.0: 2025-03-06 → 2025-05-07 (~2 months).
- 0.19.0 → 0.20.0: 2025-05-07 → 2025-06-26 (~7 weeks).
- 0.20.0 → 0.21.0: 2025-06-26 → 2025-07-16 (~3 weeks).
- 0.21.0 → 0.22.0: 2025-07-16 → 2026-01-03 (~6 months — the largest gap in the recent window).
- 0.22.0 → 0.23.0: 2026-01-03 → 2026-01-15 (~2 weeks).
- 0.23.0 → 0.24.0: 2026-01-15 → 2026-02-01 (~3 weeks).

The cadence is **not regular** — there are bursts (the 0.22 → 0.23 → 0.24 cluster early 2026) and gaps (the second half of 2025). This is the structural answer to the open question in [Buiy `architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md): AccessKit majors do not align with Bevy/winit majors. Buiy needs a documented "absorb AccessKit major in a patch release" policy; relying on AccessKit cadence to match Bevy's is structurally untenable.

The **platform-adapter crates release on their own cadences** (e.g. `accesskit_winit 0.33.0` and `accesskit_ios 0.1.0` both landed 2026-05-11). This is part of the "release-please" automated workflow — see CONTRIBUTING.

## Conventional Commits + release-please

Per the [CONTRIBUTING.md](https://github.com/AccessKit/accesskit/blob/main/CONTRIBUTING.md):

- PR titles must follow **Conventional Commits** (`feat:`, `fix:`, `chore:`, etc.).
- CHANGELOG.md is **auto-generated by `release-please`**, not hand-edited.
- The repository commits `Cargo.lock`; contributors are asked to make minimal updates and not run `cargo update` unprompted.

The release-please automation is what produces the "10 simultaneous release notes on 2026-05-11" pattern visible in the GitHub releases page — each workspace member gets its own release entry on the same automation run.

## Issue triage practices

- Issues flow through GitHub Issues with no formal RFC process. The CONTRIBUTING.md does not mention any RFC or design-doc workflow.
- Bug reports are expected to include reproduction steps; feature requests need a clear rationale.
- Tests are required across all affected platforms; the repo has platform-specific test commands (macOS, Unix, Windows) beyond `cargo test`.
- MSRV is **1.85** (workspace `rust-version` per the [workspace `Cargo.toml`](https://raw.githubusercontent.com/AccessKit/accesskit/main/Cargo.toml)); contributors must ensure changes build on it.

There is **no formal RFC process** — design discussions happen on GitHub Issues and PRs.

## Relationship to W3C / standards bodies

AccessKit is **not a W3C deliverable** and not formally aligned with any standards body. The schema "comes largely from ARIA" (per the [Role docs](https://docs.rs/accesskit/latest/accesskit/enum.Role.html)) but AccessKit makes its own decisions where ARIA and the platform APIs disagree:

- Unifies `aria-checked` and `aria-pressed` into one `Toggled` enum (see [`capabilities.md`](capabilities.md)).
- Reorders Role variants by frequency for serialization efficiency, not for spec ordering.
- Spec-references "the latest draft" of ARIA rather than pinning to a particular published version.

The **ACCNAME 1.2 algorithm** (the W3C Accessible Name and Description Computation 1.2 spec) is the canonical name-computation algorithm AccessKit consumers are expected to implement — but **AccessKit itself does not compute the accessible name**. It holds the references (`set_labelled_by`, `set_described_by`) and the platform adapters pass them through to the OS API; the consuming toolkit is responsible for resolving them. This is one of the reasons the Buiy [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) spec moved ACCNAME 1.2 implementation into `buiy_core` explicitly.

## Bus factor

The contributor concentration is uncomfortable: per AUTHORS + observed commit volume, AccessKit is heavily Matt-Campbell-centric, with Arnold Loubriat as the main co-maintainer (especially on the Linux/AT-SPI side). Adopting AccessKit as a load-bearing dependency in the Buiy spec means Buiy inherits this concentration risk — see [`critiques.md`](critiques.md).

## Cross-links

- Adopter list and ecosystem detail: [`ecosystem.md`](ecosystem.md).
- The version-cadence open question for Buiy: [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md).
- Founders + dates context: [`history.md`](history.md).

## Sources

- https://github.com/AccessKit/accesskit/blob/main/README.md
- https://github.com/AccessKit/accesskit/blob/main/CONTRIBUTING.md
- https://raw.githubusercontent.com/AccessKit/accesskit/main/AUTHORS
- https://raw.githubusercontent.com/AccessKit/accesskit/main/Cargo.toml
- https://crates.io/crates/accesskit
- https://accesskit.dev/
- https://pneumasolutions.com/about/
- https://nvaccess.org/about-nv-access/
- https://www.w3.org/TR/accname-1.2/
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md
