**Date:** 2026-05-22
**Status:** active
**Subject:** egui — maintainership, commercial stewardship, and contribution model

# Governance

egui has no foundation, no RFC process, and no committee. It has one architect-maintainer (Emil Ernerfeldt) employed by one company (Rerun.io) that uses egui as its core product surface. The model is **benevolent-dictator-plus-employer-investment**, and it has held for seven years.

## People

| Role | Person | Affiliation |
|---|---|---|
| Architect / lead maintainer | **Emil Ernerfeldt** (`@emilk`) | Rerun.io (co-founder, ~2022) |
| Major-contributor cluster (2024–2026) | `@lucasmerlin`, `@valadaptive`, `@rustbasic`, `@IsseW`, `@KonaeAkira` | Rerun.io employees + community |
| AccessKit integration owner | `@DataTriny` (initial) → `@lucasmerlin` (current) | Rerun.io |
| Skrifa migration owner | `@valadaptive` | Rerun.io |

The contributor list at the bottom of every 0.30+ changelog is dominated by Rerun employees; `@emilk` himself authors most of the architectural changes (rename Panel APIs, Ui-as-entrypoint, etc.). Community contributors land mostly in widget polish + bug-fix lanes.

## Commercial steward: Rerun.io

Rerun.io is a streaming-data visualization startup (ML / robotics audience). The Rerun Viewer — desktop + web — is built on egui. Rerun employs Emil and several other core egui contributors.

The relationship is **mutually load-bearing**:

- **egui benefits from Rerun's engineering investment.** Multi-viewport, multipass, the Plugin trait, the kitdiff snapshot-diff tool, the skrifa migration — all driven by Rerun's daily use. Without Rerun's salary cover, egui would be the volunteer-bandwidth-limited project it was in 2020.
- **Rerun benefits from egui's community.** egui's ~17M downloads and bevy_egui's ~2M downloads give Rerun a recruiting surface, a credibility surface, and a free-engineering surface (community bug reports + PRs). Open-sourcing the GUI layer is a deliberate Rerun strategy, not an afterthought.

This makes egui a **commercially-stewarded open-source project** — closer to Apache Kafka under Confluent or PostgreSQL under EnterpriseDB than to a foundation-governed library like Iced. The implication for Buiy: when Rerun's product needs and egui's general-purpose needs diverge, Rerun wins. Most decisions don't diverge, but the policy is real.

A worked example: the multi-viewport feature (0.24, 2023-11-23) was Rerun-driven. Rerun's UX needed dockable floating panels that could detach to OS-level windows. The community had been asking for nothing of the sort. The feature landed because Rerun needed it; the API shape reflects Rerun's needs.

## License decision: MIT OR Apache-2.0

Dual-licensed (verified in workspace `Cargo.toml`: `license = "MIT OR Apache-2.0"`). This is the **standard Rust ecosystem dual-license**, chosen for maximum compatibility:

- MIT — for downstreams who want permissive simplicity.
- Apache-2.0 — for downstreams who want explicit patent grants.

Both files (`LICENSE-MIT`, `LICENSE-APACHE`) live in the repo root. Every workspace crate inherits via `license.workspace = true`. No CLA — Emil accepts standard inbound=outbound licensing per the Apache convention.

Cross-link to bevy_egui: bevy_egui chose **MIT-only**, a strict subset of egui's dual license. See [`prior-art/bevy-egui/governance.md`](../bevy-egui/governance.md) for the rationale (bevy_egui inherits Bevy's MIT-only convention).

## RFC process: none

No formal RFC mechanism. The decision flow is:

1. **GitHub issue** opened — community or contributor.
2. **Emil weighs in** — usually within days for substantive proposals. Sometimes longer for sweeping API changes.
3. **PR** opened — by the proposer or a contributor.
4. **Emil reviews** + lands. Major architectural PRs sometimes sit weeks while Emil considers; minor ones land in days.

There is no design-doc-first culture. The README, the CHANGELOG, and the issue history are the design record. This works because the bus factor is small enough that the architect can hold the design in his head; it would not scale to a 20-maintainer project.

**No public roadmap exists.** Rerun's needs implicitly drive direction; community PRs land for polish. When Buiy needs to know "is feature X coming soon," the answer is "ask Emil on Discord or wait for a release-notes mention."

## Contributors over time

From counting unique PR authors in the CHANGELOG since 0.20 (~2022-12):

- **0.20 → 0.24 (2022-2023):** ~30–50 unique authors per release.
- **0.25 → 0.29 (2024):** ~50–80.
- **0.30 → 0.34 (2024-2026):** ~60–100.

The top 5 authors per release consistently account for ~60% of merged PRs — the Rerun-employed core. The long tail is dominated by single-PR contributors (typo fixes, single-widget improvements).

## Discord + GitHub Discussions

- **Discord server** — `discord.gg/JFcEma9bJq`, ~3000 members (2026-05). Active Q&A, occasional Emil presence.
- **GitHub Discussions** — moderate volume, used for design questions Emil wants on the record.
- **Discord ≠ design venue.** Architectural decisions land in GitHub issues + PRs, not Discord.

## Bus factor

Honest assessment: **bus factor 1.** Emil is the architect; Rerun employs him; Rerun's continued health is the project's continuity. The Rerun co-founders are funded (Series A 2023), so the immediate horizon is fine. The five-year horizon depends on Rerun staying healthy and Emil staying with Rerun.

This bus factor is not unique to egui — Iced (`@hecrj` + System76), Slint (`@ogoffart` + SixtyFPS GmbH), Linebender's Druid/Xilem (Raph Levien + Google/Adobe), Bevy (`@cart` + Bevy Foundation) all sit at similar single-architect risk. The Rust GUI ecosystem is broadly architect-driven and small-company-funded.

## Implications for Buiy

Buiy is not coupled to egui (we are a parallel UI stack, not an egui consumer; bevy_egui handles the egui-on-Bevy bridge for dev tools). But the governance model is informative:

- **Single-architect + employer-investment is the dominant Rust-GUI funding shape.** Buiy's funding model (if it ever needs one) will look similar.
- **Honest non-roadmap is acceptable.** egui ships without a public roadmap; the project still moves; users tolerate the opacity because releases are frequent.
- **RFC processes are not free.** egui shipping without one for seven years is evidence that small UI libraries can govern themselves without RFC overhead. Buiy's `docs/specs/` + `docs/plans/` convention is a heavier process by Buiy's own choice — fine, but understand it's a heavier process than the dominant alternative.

## Sources

- egui repo — https://github.com/emilk/egui
- Workspace `Cargo.toml` — https://raw.githubusercontent.com/emilk/egui/main/Cargo.toml
- LICENSE-MIT, LICENSE-APACHE @ repo root — https://github.com/emilk/egui/tree/main
- Rerun.io — https://rerun.io
- egui CHANGELOG (contributor attribution) — https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md
- egui Discord — https://discord.gg/JFcEma9bJq
- bevy_egui governance (cross-link) — `prior-art/bevy-egui/governance.md`
