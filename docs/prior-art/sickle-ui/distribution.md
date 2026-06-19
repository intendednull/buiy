**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — distribution + governance: license, cadence, solo-maintainer bus factor

# Distribution & Governance

This file covers the licensing, release cadence, governance, and post-archive distribution status of sickle_ui. Folded into one file because the project's small scale and effective abandonment make a separate `governance.md` redundant.

## License

**MIT OR Apache-2.0 dual-licensed.** This is the dominant Rust-ecosystem convention; sickle adopts it without modification. Verified against the 0.4.0 Cargo.toml and the `sickle_ui_scaffold` Cargo.toml — both crates use the same dual-license declaration.

There is no LICENSE-MIT vs LICENSE-APACHE file split published with the crate (some Rust crates publish both files; sickle publishes a single SPDX declaration in Cargo.toml). For an integrator concerned with provenance the SPDX expression is the source of truth.

## Minimum supported Rust version (MSRV)

Not explicitly declared in Cargo.toml. By transitive constraint: sickle 0.4.0 depends on Bevy 0.14, which required Rust 1.79 as its MSRV. An integrator using sickle 0.4.0 in 2026 needs at least Rust 1.79 — but they also need Rust to still compile Bevy 0.14, which (since Bevy 0.14 is itself receiving no patch releases) is governed by whatever standard-library and rustc-API stability holds going forward. Bevy 0.14 was last patched at 0.14.2.

## Release cadence

Five releases over ~75 days (2024-07-20 → 2024-10-03), then 19 months of silence as of this writing. The cadence during active development was reasonable — roughly one minor release per month — but the post-0.4.0 silence dominates the project's history. See [`history.md`](history.md) for the full timeline.

The cadence was set by a single maintainer's free-time pace and the substantive work-batch each minor required (Theming engine rework in 0.3; final polish in 0.4). There is no public release schedule, no roadmap, no Bevy-pinning policy stated by the maintainer in advance — the cadence-by-availability pattern is typical of solo-maintained Bevy crates and was not visible as a risk signal until the project went silent.

## Governance — solo maintainer

**One author, one decision-maker.** UmbraLuminosa is named on the crate as the publisher (`owners` on crates.io list one user). There is no organization on GitHub backing the project (the `UmbraLuminosa` GitHub identity is an individual user account dressed as an org — verified by org member listing returning no public members). No co-maintainers, no contributor-team, no advisory group.

Public contribution surface:

- `has_issues: false` on the (now-deleted) original repository, per the cached search-API metadata. The maintainer disabled GitHub Issues, suggesting issue triage was not a workflow they wanted to maintain.
- No `CONTRIBUTING.md` published with the crate.
- No public Discord channel, no project-specific community forum.

Other Bevy crates of similar scope (`bevy_lunex`, `bevy_egui`, `bevy_cosmic_edit`) maintain public issues + active Discord presence. Sickle's deliberately minimal community surface kept maintainer overhead down but also meant that **no community could form around contributing fixes**, which compounded the bus-factor problem when the maintainer chose not to migrate past Bevy 0.14.

## Bus factor

**1, with the bus already gone.** The maintainer's posted decision to declare the project obsolete is the bus departing. There is no formal succession (no "I'm passing this to X" handoff post). The two surviving forks (`UkoeHB/sickle_ui`, `danec020/sickle_ui`) are unofficial archives, not successor projects.

UkoeHB's later integration of sickle's scaffold layer into `bevy_cobweb_ui` is the closest thing to a "design lineage handoff" — the underlying theming and dynamic-style primitives carry forward into a different project's namespace. But UkoeHB also archived `bevy_cobweb_ui` on 2026-01-13, so even the lineage-successor is now dormant.

## Funding model

**None.** No GitHub Sponsors button on the (former) repository, no `funding.yml`, no `crates.io` patron link. The project was, in every visible sense, a free-time effort by one developer.

Compare: `bevy_egui` and `bevy_cosmic_edit` have GitHub Sponsors and OpenCollective bookmarks; `bevy_lunex` solicits Patreon contributions. Funding is not a sufficient condition for sustained maintenance, but its absence in combination with solo-maintainer + free-time-only is a high-risk pattern.

## Distribution channels — what an integrator finds in 2026

- **crates.io:** sickle_ui 0.4.0 is still published, still installable. Total downloads 15,120; recent 90-day 517 (downloads continue post-archive — likely a mixture of caches, transitive pulls from forks, and apps still on Bevy 0.14).
- **docs.rs:** documentation hosted at `docs.rs/sickle_ui/0.4.0/`. The "2.9% documented" coverage figure is on display at the top of every module page.
- **GitHub upstream:** `UmbraLuminosa/sickle_ui` returns 404 (deleted).
- **GitHub forks:** `UkoeHB/sickle_ui` (last release archive with obsolescence notice), `danec020/sickle_ui` (active fork, Bevy 0.14 only).
- **`bevyengine/bevy-assets`:** **no entry** for sickle_ui in `Assets/UI/`. Removed from official ecosystem discoverability.
- **lib.rs:** lists sickle_ui under "User Interface" alongside other Bevy UI crates, but the listing is auto-generated from crates.io metadata (which still references the dead URL).

The effect: an integrator can install sickle_ui via `cargo add`, but the upstream-on-GitHub link from crates.io is broken, and there is no migration documentation, no maintainer to file issues against, and no community forum. The project exists as a stable artifact on crates.io with no living infrastructure.

## Implications for Buiy

1. **Solo-maintainer + free-time-only + Bevy-version-coupled = high abandonment risk.** This is the canonical pattern. Buiy's governance commitment (per CLAUDE.md and foundation README) is to keep the project documented enough that contribution is possible even if any one maintainer drops off.
2. **The repository-deletion failure mode is the worst signal.** Buiy should preserve its public-archive contract: even if the project becomes inactive, the repository stays public and the obsolescence notice (if any) lives in-tree, so future archeologists find a working LICENSE + README + git history rather than a 404. The `Status: archived` flag in our own prior-art header is the same discipline.
3. **No-issues-tracker is a maintainability anti-pattern.** Even disabled issues let people search closed reports. Buiy should keep GitHub Issues open even when the maintainer team is small, accepting the moderation cost as part of project hygiene.
4. **License default (MIT OR Apache-2.0) is correct and Buiy already adopts it.** No lesson to learn here; sickle got this right.

## Sources

- crates.io API — https://crates.io/api/v1/crates/sickle_ui
- sickle_ui_scaffold API — https://crates.io/api/v1/crates/sickle_ui_scaffold
- 0.4.0 Cargo.toml — https://docs.rs/crate/sickle_ui/0.4.0/source/Cargo.toml
- docs.rs (documentation coverage display) — https://docs.rs/sickle_ui/0.4.0/sickle_ui/
- Surviving fork (obsolescence notice) — https://github.com/UkoeHB/sickle_ui
- Deleted upstream (404) — https://github.com/UmbraLuminosa/sickle_ui
- UmbraLuminosa org — https://github.com/UmbraLuminosa
- bevy-assets UI listing (no sickle entry) — https://github.com/bevyengine/bevy-assets/tree/main/Assets/UI
- Buiy CLAUDE.md (dev guidelines) — `CLAUDE.md`
