**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — history: 0.1 → 0.4, the Bevy 0.15 cliff, the obsolescence notice

# History

sickle_ui's lifecycle is concise: ~3 months of pre-1.0 development culminating in 0.4.0 in October 2024, then 19 months of silence, then a primary-source obsolescence notice. This file traces it.

## Project genesis

The earliest published trace is the **crates.io `0.2.1` publication on 2024-07-20** — the initial public release under the `sickle_ui` name on crates.io. Earlier 0.1.x versions exist as git history but were not published as crates (the published versions on crates.io start at 0.2.1). The crate creation timestamp on crates.io for `sickle_ui` is 2024-07-20T17:12:46Z; for `sickle_ui_scaffold` 2024-07-20T17:10:56Z (the scaffold was split out at first publication).

The author is **UmbraLuminosa**, identified on GitHub as a Switzerland-based developer who maintains `Proof-of-Concept-Editor-in-Bevy` (created 2023-12-18) as their other public Bevy work. The editor proof-of-concept appears to be the motivating use-case that produced sickle: a developer building a Bevy-native editor needed editor-grade widgets, didn't find them in `bevy_ui`, and wrote them as a separate library.

## Version timeline (verified against crates.io API)

| Version | Published | Downloads | Notes |
|---|---|---|---|
| 0.2.1 | 2024-07-20 | 2,715 | Initial public release. Bevy 0.14 from the start. |
| 0.2.2 | 2024-08-17 | 1,267 | Same-day with 0.2.3 (~1 hour gap). 0.2.2 was effectively retracted by 0.2.3 — both ship within 2 hours, suggesting a publish-fix-republish cycle. |
| 0.2.3 | 2024-08-17 | 4,728 | The most-downloaded version of any minor — the de-facto 0.2 line. |
| 0.3.0 | 2024-09-14 | 2,458 | Theming engine rework (PseudoTheme / DynamicStyle reaches its current shape per docs.rs). |
| 0.4.0 | **2024-10-03** | 3,952 | **Final release.** No 0.4.1, no 0.5.x. |

`sickle_ui_scaffold` tracks the same version-and-date scheme: 0.2.1 (2024-07-20), 0.2.2 (2024-08-17), 0.2.3 (2024-08-17), 0.3.0 (2024-09-14), 0.4.0 (2024-10-03). The version lockstep is enforced — the catalog crate pins the scaffold to the same minor.

Total active development on crates.io spans **~75 days** (2024-07-20 to 2024-10-03), with five releases. That cadence is reasonable for a pre-1.0 widget library; the **silence after 0.4.0** is what defines the project.

## The Bevy 0.15 cliff (2024-11-26)

Bevy 0.15.0 released on 2024-11-26, ~54 days after sickle 0.4.0. Among the changes that affected sickle were:

- **PR #14791 (Required Components)** landed: `#[require(Companion)]` on a Component derive replaced the explicit-bundle-spawning pattern. sickle's authoring code (which manually spawns each widget as a `Node`-plus-companions bundle) becomes idiomatically wrong — the new world expects `MyWidget` to declare `#[require(Node, BackgroundColor, ...)]` and let Bevy auto-insert the rest.
- **`bevy_ui` node component decomposition continued** — `BackgroundColor`, `BorderColor`, `Outline`, `BoxShadow` etc. evolved across 0.15 / 0.16 / 0.17 (see [`../bevy-ui/history.md`](../bevy-ui/history.md)). sickle's `UiStyleExt::background_color(...)` setter family targets a specific component shape that does not survive these migrations cleanly.
- **`bevy_text` migrated from cosmic-text 0.16 to ab_glyph in some paths and stayed on cosmic-text in others; later (0.19-dev) migrated to parley.** sickle's text-shaping code touched `bevy_text` only indirectly via `Label`, but every text-related migration introduced churn.

The maintainer's posted decision (in the surviving-fork README, verbatim): `"sickle_ui has been made obsolete by changes introduced in Bevy 0.15.0 and will not be publicly maintained. This is the last release, compatible with Bevy 0.14.2."`

This is unusually explicit for an abandoned crate — many Bevy crates simply stop receiving updates without any maintainer statement. Sickle's notice **names Bevy 0.15 as the trigger**, which makes the post-mortem clear: the project was structurally Bevy-0.14-shaped, the 0.15 changes were too invasive for a one-person migration to be feasible, and the maintainer chose to declare it over rather than half-port and abandon mid-migration.

## The repository removal

At some point between 0.4.0 (2024-10-03) and the writing of this corpus (2026-05-22), the `UmbraLuminosa/sickle_ui` GitHub repository was **deleted**. The exact date is not externally verifiable (no archived snapshot we can reach), but the present state is:

- `https://github.com/UmbraLuminosa/sickle_ui` returns **HTTP 404**.
- The `UmbraLuminosa` organization profile **still exists** with one remaining public repo: `Proof-of-Concept-Editor-in-Bevy`.
- The `sickle_ui` crate on crates.io still **links to the dead URL** — crates.io does not auto-detect upstream-repo deletion, and the maintainer has not republished with a corrected `repository` field.

The repository deletion is a stronger abandonment signal than the obsolescence notice: notices can be revised, but the upstream codebase moving to "fork-only" archives the project in the GitHub-archeology sense. Most Bevy users searching for sickle_ui in 2026 will hit the dead URL first and either give up or find one of the surviving forks.

## Surviving forks (active vs archive)

Two forks matter:

- **`UkoeHB/sickle_ui`** (9 stars, last update 2024-10-04, ~1 day after the 0.4.0 release). Carries the **obsolescence notice** in its README — this fork is the closest thing to an "official archive" of the last-known-good state. UkoeHB is the author of `bevy_cobweb_ui` (also archived 2026-01-13) and integrated portions of sickle's scaffold layer there.
- **`danec020/sickle_ui`** (9 stars, 31 forks, last commit 2025-02-27). The most-recently-committed-to fork. Continues experimental work on Bevy 0.14 (commit messages reference thumbnails, docking, container fixes). Has **not** migrated to Bevy 0.15+ — the work is on the 0.14 codebase. No release published to crates.io.

Several other forks exist (`TheSeekerGame`, `tungtose`, `slyedoc`, `komodo472`) — all snapshot states from 2024, no public migration work.

## Removal from `bevyengine/bevy-assets`

The official Bevy ecosystem listing (`bevyengine/bevy-assets/Assets/UI/`) does **not** contain a `sickle_ui.toml`, as verified 2026-05-22. The exact removal commit was not found in the public commit search (the commit may have predated the directory's current structure). The practical effect: a user discovering Bevy UI libraries via the official channels will not see sickle_ui listed, even though the crates.io page still exists.

bevy-assets removal is the official-channels analog of the GitHub repo deletion — once a project is dropped from this listing, it leaves the ecosystem's discoverability surface entirely.

## Has sickle been overtaken?

Yes, structurally. Two official replacements occupy the same niche:

- **`bevy_ui_widgets`** (Bevy 0.17+) — headless widget primitives. Same role as the behavioral parts of sickle's widgets (FluxInteraction, value-emitting events). Lives in the engine's own crate workspace. See [`../bevy-ui-widgets/`](../bevy-ui-widgets/).
- **`bevy_feathers`** (Bevy 0.17+) — styled widget kit on top of `bevy_ui_widgets`. Same role as sickle's catalog. Maintained by ickshonpe + viridia per [`../bevy-feathers/governance.md`](../bevy-feathers/governance.md).

The combined `bevy_ui_widgets` + `bevy_feathers` stack provides (a) more widgets than sickle, (b) a published-with-Bevy release cadence, (c) the headless-behavior / styled-presentation split that sickle conflated, (d) integration with `bevy_input_focus` + `bevy_picking` + `bevy_a11y`. They are not better in every dimension — sickle's editor-docking primitives (`docking_zone`, `floating_panel`, `sized_zone`) are not replicated — but for the editor-and-utilities scope sickle claimed, the official combination is now the default answer.

## The lesson

sickle_ui is a clean expression of how third-party Bevy UI libraries die: a solo maintainer ships a substantial body of work, a Bevy minor release reshapes the substrate enough to require non-trivial migration, the maintainer assesses the cost as too high, and the project is declared obsolete rather than half-ported. The story plays out repeatedly in the Bevy ecosystem (compare `kayak_ui`, `bevy_megaui`, `bevy_ninepatch`, others). The structural fix is **either** absorbing the library into the engine's workspace so it migrates in lockstep (what `bevy_feathers` does), **or** committing to a parallel stack that decouples from `bevy_ui`'s churn (what Buiy does), **or** writing the library against a stable abstraction layer that hides Bevy version churn (no such layer exists in the Bevy ecosystem).

## Sources

- crates.io API (publish dates) — https://crates.io/api/v1/crates/sickle_ui
- `sickle_ui_scaffold` API — https://crates.io/api/v1/crates/sickle_ui_scaffold
- Surviving fork README (obsolescence notice) — https://github.com/UkoeHB/sickle_ui
- Bevy 0.15 release date — https://bevy.org/news/bevy-0-15/
- Bevy `RequiredComponents` PR — https://github.com/bevyengine/bevy/pull/14791
- bevy-assets UI listing (no sickle entry) — https://github.com/bevyengine/bevy-assets/tree/main/Assets/UI
- Deleted upstream — https://github.com/UmbraLuminosa/sickle_ui (404 as of 2026-05-22)
- UmbraLuminosa org — https://github.com/UmbraLuminosa
- danec020 fork — https://github.com/danec020/sickle_ui
- bevy_cobweb_ui (scaffold salvage) — https://github.com/UkoeHB/bevy_cobweb_ui
- bevy_feathers history — [`../bevy-feathers/history.md`](../bevy-feathers/history.md)
- bevy_ui history — [`../bevy-ui/history.md`](../bevy-ui/history.md)
