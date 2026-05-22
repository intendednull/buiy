**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — release timeline, Bevy version pinning per release, post-release commit activity, archive event.

# History

A short-lived crate by Bevy-ecosystem standards. First releases shipped against Bevy 0.11 in mid-2023; final release shipped against Bevy 0.15 in late 2024; archive declared in early 2025.

## Bevy version compatibility table

From the README at the final tag (copied here verbatim for archival purposes):

| Bevy version | bevy_cosmic_edit version |
|---|---|
| 0.15.0 | 0.26 (latest at archive) |
| 0.14.0 | 0.21 – 0.25 |
| 0.13.0 | 0.16 – 0.20 |
| 0.12.* | 0.15 |
| 0.11.* | 0.8 – 0.14 |

The pattern: bevy_cosmic_edit's *major* version bumped on each Bevy *minor* release, but **only forward**. There was no LTS branch; once a new Bevy came out, the old Bevy line stopped getting patches.

## Cosmic-text version pinning

Cosmic-text was an *implicit* dependency at the final tag — the visible `Cargo.toml` listed only `bevy = "0.15"`, and cosmic-text came in transitively via `bevy_text` (Bevy 0.15 pulled cosmic-text 0.12.x). Earlier versions of bevy_cosmic_edit had explicit `cosmic_text = "x.y.z"` pins; the CHANGELOG notes 0.17 explicitly updated to cosmic-text 0.11.2.

This is a meaningful detail: **bevy_cosmic_edit ceased to control its cosmic-text version somewhere in the 0.20-series**, instead riding whatever bevy_text pulled in. When bevy_text migrated off cosmic-text entirely on `main` (issue [#21765](https://github.com/bevyengine/bevy/issues/21765)), bevy_cosmic_edit had no upgrade path that didn't involve re-introducing an explicit cosmic-text dep.

## Notable releases (per CHANGELOG)

The CHANGELOG was sparse. Verified entries:

| Version | Date (approx) | Highlight |
|---|---|---|
| 0.8 | mid-2023 | First Bevy 0.11 release. |
| 0.15 | late 2023 | Bevy 0.12 bump. |
| 0.16 | early 2024 | Bevy 0.13 bump. |
| **0.17** | mid-2024 | Cosmic-text → 0.11.2. **Removed** placeholders, password fields, undo/redo from the core. Maintainer's note: these "could be restored via internal plugins if users request them through pull requests." |
| 0.18 | mid-2024 | Re-added placeholder plugin. |
| 0.19 | mid-2024 | "Fix text mode that allows arbitrary length string." |
| 0.21 | mid-2024 | Bevy 0.14 bump. |
| 0.25 | late 2024 | Final Bevy 0.14 release. |
| **0.26.0** | **2024-12-07** | Bevy 0.15 bump. **Last released version.** |

The 0.17 feature-removal is significant: the maintainer explicitly chose to *shrink* the surface to keep maintenance tractable, deferring features to opt-in sub-plugins. This was a load-bearing maintenance decision; whether it bought enough runway is debatable in light of the 2025-03 archive ~9 months later.

## Post-release commit activity (Dec 2024 — Feb 2025)

After 0.26.0 was published:

- **2024-12-12** — PR #167 "bevy::picking integration and refactoring" merged (ActuallyHappening). Brought hit-testing onto bevy_picking instead of the bespoke picking path. **Never released.**
- **2024-12-13** — PR #168 "Basic 3D Support" opened (ActuallyHappening). **Never merged; left open at archive.**
- **2025-02-04** — PR #170 "Lil cleanup" merged (bytemunch). The last commit on `main`.
- **2025-03-02** — Issue #171 "Placeholder should not operate on the actual text buffer" opened (databasedav). The last issue filed. **Never resolved.**
- **2025-03-21** — Repository archived by owner (Dimchikkk).

Between the last commit (2025-02-04) and the archive (2025-03-21) was a six-week silent period. No commit, no comment, no PR review activity. The archive event was unannounced (no commit message, no README note added pre-archive — the archive banner is the only signal).

## Maintainer involvement

`Dimchikkk` (Dima) is the GitHub owner and de-facto maintainer. The crates.io publishing handle is `StaffEngineer`, a separate GitHub account with no public repositories — likely an alternate identity Dimchikkk used for crate publishing, or a shared org-style alias.

Repeat contributors (most frequent → less frequent, by commits over 2024):

- **ActuallyHappening** — bevy_picking integration (#167), basic 3D support (#168, never merged), Bevy 0.15 bump (#166), code quality (#164).
- **databasedav** — webgl2 default + webgpu opt-in (#163), focus-on-despawn hook (#159), placeholder issue (#171).
- **bytemunch** — final cleanup (#170).
- **iancormac84** — window-query error handling (#161).

These contributors did not take over maintainership at archive; no fork is canonically active (see [`ecosystem.md`](ecosystem.md) for the fork landscape).

## Lifetime in numbers

- Total releases: ~26 (0.1 through 0.26.0, not all numbered consecutively per the README compat table).
- Lifetime downloads at archive: 40,832 (per crates.io).
- Stars at archive: 110.
- Forks at archive: 14.
- Total PRs: 98 (97 closed/merged, 1 left open).
- Total issues: 10 open at archive (no closed count published, but ~170+ implied by issue numbering).
- Active lifetime: ~21 months (mid-2023 → 2025-03-21).
- Time from 0.26.0 → archive: **~3.5 months** (2024-12-07 → 2025-03-21). The crate was not deeply abandoned before archive; the archive event preceded any extended silent decay.

This last point matters: **bevy_cosmic_edit was not archived because it had been silently dead for years**. It was archived ~3 months after its last release, during a window when Bevy 0.16 was the next imminent bump. The archive looks more like an active decision than a passive lapse — see [`why-archived.md`](why-archived.md).

## Sources

- README compat table — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/README.md
- CHANGELOG — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/CHANGELOG.md
- Commit log — https://github.com/Dimchikkk/bevy_cosmic_edit/commits/main
- PR list — https://github.com/Dimchikkk/bevy_cosmic_edit/pulls?q=is%3Apr
- Issue #171 — https://github.com/Dimchikkk/bevy_cosmic_edit/issues/171
- crates.io listing — https://crates.io/crates/bevy_cosmic_edit
- Archive notice — https://github.com/Dimchikkk/bevy_cosmic_edit
- Bevy issue #21765 (cosmic-text → Parley) — https://github.com/bevyengine/bevy/issues/21765
