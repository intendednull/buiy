**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — chronological history from Stretch (Visly, 2018) through DioxusLabs stewardship and the 0.10 line

# Taffy — history

Taffy is the third generation of a single Rust flexbox engine that has changed hands twice. The lineage matters because the current stewardship + API posture is shaped by the prior two phases.

## 1. Stretch — Visly Inc., 2018–2020

**Original author:** Emil Sjölander, at Visly Inc. (a now-defunct Stockholm-based design tool company; `visly.app`). The project was open-sourced 2018-12-29 with first release `stretch 0.1.0`.

**What it was:** "High performance & cross-platform Flexbox implementation" — Yoga rewritten in Rust. Bindings for Android, iOS, JavaScript/TypeScript, and Rust itself. Powered the Visly design app.

**Release cadence (verified [crates.io/crates/stretch](https://crates.io/api/v1/crates/stretch)):**

- `0.1.0` — 2018-12-29
- `0.2.0` — 2019-04-08
- `0.3.0` — 2019-07-04
- `0.3.2` — 2019-07-05 (final crates.io release)

**Last commit:** 2020-05-22 (commit `6879b9a`, *"Merge pull request #70 from adamnemecek/pr3"*, authored Emil Sjölander). The repo was never marked archived but stopped receiving any maintenance.

**Why it stalled:** Visly the company wound down. No external maintainer picked it up immediately. Bevy UI shipped on `stretch 0.3.2` for two years (Bevy 0.5 through Bevy 0.8) carrying known bugs that couldn't be fixed without a maintained upstream.

## 2. stretch2 — Jonathan Kelley fork, 2022

**Fork author:** Jonathan Kelley (`@jkelleyrtp`), the Dioxus lead. Forked under his personal name to crates.io as `stretch2` on 2022-03-09. First release `0.4.0`, then `0.4.1`, `0.4.2` over March 2022.

**What changed in stretch2:** Minimal — primarily made the existing Stretch codebase build on current Rust and ship to crates.io as a new name. Same algorithm. The fork existed to unblock Dioxus and Bevy UI consumers waiting on a maintained Stretch.

**Handoff to Alice Cecile:** `stretch2 0.4.3` (final release, 2022-05-23) lists Alice Cecile (`@alice-i-cecile`, Bevy lead UI maintainer) as author. This is the bridge release — by mid-2022 the active maintenance burden moved from Kelley to a small group around Bevy + Dioxus.

`stretch2` is also defunct on crates.io (no releases since 2022-05-23). It exists as a frozen sibling of `taffy 0.1.0`.

## 3. Taffy — the rename, June 2022

**`taffy 0.1.0`** was published to crates.io **2022-06-10** by Jonathan Kelley. The crate was renamed from `stretch2` (and briefly from `sprawl`, the working name during the fork — see [PR #79 — Rename `sprawl::node::Stretch` → `sprawl::node::Sprawl`](https://github.com/DioxusLabs/taffy/pull/79), merged 2022-06-01). The repo lives at `github.com/DioxusLabs/taffy` and the crate is co-owned by Kelley, Cecile, and Nico Burns (`@nicoburns`), the three named crate owners as of 2026-05.

The DioxusLabs org owns the repo, so the project is *stewarded* by DioxusLabs even though its primary maintainer (Nico Burns) is independent and the load-bearing engineering work happens via Burns and Cecile.

## 4. Major releases (verified [crates.io/crates/taffy](https://crates.io/crates/taffy) + [CHANGELOG](https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md))

| Version | Date | Headline |
|---|---|---|
| `0.1.0` | 2022-06-10 | Initial release, renamed from stretch2. Flexbox only. |
| `0.2.0` | 2022-11-24 | `gap` property; `AlignContent::SpaceEvenly`; up to 90× perf improvements on deep hierarchies. |
| `0.3.0` | 2023-02-12 | **CSS Grid algorithm.** First Rust-native Grid implementation in a layout engine. |
| `0.4.0` | 2024-02-13 | **Block layout** (`display: block`). Overflow property. Measure function API redesign (low-level traits). Module hierarchy simplified. |
| `0.5.0` | 2024-05-30 | Measure function API refined (Style parameter exposed). |
| `0.6.0` | 2024-10-10 | `Style` struct traitified — `BlockContainerStyle`, `BlockItemStyle`, `FlexboxContainerStyle`, etc. `box_sizing` support. Computed margins exposed in `Layout`. |
| `0.7.0` | 2024-12-12 | Low-level API restructured. `cache_mut` replaced with separate `CacheTree` trait (see [architecture.md § 2](architecture.md#2-the-trait-stack)). |
| `0.8.0` | 2025-04-01 | **`calc()` values** in low-level API. `Dimension`/`LengthPercentage` switched to tagged-pointer `CompactLength` — engine became `!Send + !Sync`. |
| `0.9.0` | 2025-08-07 | **Named grid lines + grid areas.** `Style` became generic over a `CheapCloneStr`. |
| `0.10.0` | 2026-03-31 | **`direction` property** for LTR/RTL. **`float` and `clear` properties.** CSS-string parsing via `FromStr`. MSRV bumped to **1.71**. |
| `0.10.1` | 2026-04-14 | CSS Grid auto-repeat + minimum-size fixes. Current stable. |

The 0.3-line is unusually long: `0.3.0` through `0.3.19` shipped over 2023, mostly Bevy-driven bugfixes. The pace shifted around mid-2024 with `0.4` — that's when block layout, low-level traits, and the deliberate API-revision cadence started.

Three **experimental versions** are also live on crates.io as of 2026-05-15:

- `0.10.1-experimental-cache-fix.1`
- `0.10.2-experimental-cache-fix.2`
- `0.11.0-experimental-cache-fix.3`

These are correctness-of-cache fixes that touch the trait surface; Blitz pins `=0.11.0-experimental-cache-fix.3` because cache correctness is load-bearing for it. Production embedders that aren't chasing it stay on `0.10.1`.

## 5. Notable contributors and their contributions

Three Cargo.toml-listed authors (verified `[package].authors = ["Alice Cecile", "Johnathan Kelley", "Nico Burns"]` on main `Cargo.toml`; note: README/crate-owners spell the second as "Johnathan" and "Jonathan" inconsistently — the Cargo.toml has the typo):

- **Nico Burns (`@nicoburns`)** — the primary technical lead since at least 2023. The CSS Grid algorithm (`0.3.0`), the `CacheTree` split (`0.7.0`), the `CompactLength` tagged-pointer migration (`0.8.0`), the `float`/`clear` implementation (`0.10.0`), the WPT-test importer (issue #639, closed), the WPT roadmap (issue #345). Substantially the maintainer of record. Independent; blog at `nicoburns.com`.
- **Alice Cecile (`@alice-i-cecile`)** — bridge from the Bevy UI team. Carried the maintenance handoff in `stretch2 0.4.3`. Continues as a crate owner and reviewer; primary technical work happens upstream of Bevy's needs.
- **Jonathan Kelley (`@jkelleyrtp`)** — Dioxus founder, original fork author from Stretch. Now provides the steward org (DioxusLabs) but is not the day-to-day maintainer.

Other recurring authors visible in the issue/PR tracker: `@robtfm` (Bevy contributor), `@TimJentzsch` (Bevy UI), `@cart` (Bevy lead — occasional API reviewer), Bevy UI WG contributors generally. The contributor count is small (10–15 active in any given quarter), matching the project's scope.

## 6. DioxusLabs stewardship

DioxusLabs took stewardship via the rename event on 2022-06-10 — same day as `taffy 0.1.0`. The repo has always lived at `DioxusLabs/taffy`. There is no separate steward-handover commit because the rename was the handover.

The Dioxus parent org is venture-backed (Series A from FutureWei + others, per `dioxuslabs.com`); Taffy benefits indirectly via Burns + Cecile having sponsored time on it, but the project has no separate budget line. There is no `funding.json`, no GitHub Sponsors integration on the repo, no Open Collective.

## 7. License decision

License is **MIT** throughout the lineage:

- Stretch: MIT (Visly Inc., `Copyright (c) 2018 Visly Inc.`).
- stretch2: MIT (same).
- Taffy: MIT (verified `[package].license = "MIT"` in current `Cargo.toml`, every version on crates.io since `0.1.0`).

This is *not* MIT OR Apache-2.0 — the brief's expectation was right. The pre-amble correction: Taffy is single-licensed MIT, not dual-licensed. This is unusual in the Rust ecosystem (most crates are MIT-or-Apache); it traces back to Visly's original choice and was never changed. Downstream embedders that want patent-grant coverage (e.g. corporate consumers nervous about MIT-only) have to either accept or fork — there's no Apache pathway.

## 8. Release cadence

Irregular. Patch releases cluster around stability-fix work (the `0.3.x` line ran twenty patches over 2023; the `0.7.x` line ran seven over 2024-12 → 2025-01). Minor releases are roughly quarterly when work is happening; sometimes gappier (`0.8.0` was 2025-04-01, `0.9.0` was 2025-08-07 — four months; `0.9.0` to `0.10.0` was almost eight months, due to the float-layout work). MSRV bumps to 1.71 in `0.10.0` (previously 1.65 since `0.4.0`).

## Sources

- crates.io taffy metadata: https://crates.io/api/v1/crates/taffy
- crates.io stretch metadata: https://crates.io/api/v1/crates/stretch
- crates.io stretch2 metadata: https://crates.io/api/v1/crates/stretch2
- Visly stretch repo (last commit 2020-05-22): https://github.com/vislyhq/stretch/commits/master
- Taffy PR #79 (sprawl rename): https://github.com/DioxusLabs/taffy/pull/79
- Taffy main Cargo.toml (verified MIT, 1.71 MSRV, three authors): https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml
- Taffy CHANGELOG: https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- Bevy PR #6743 (upgrade to Taffy 0.2): https://github.com/bevyengine/bevy/pull/6743
- Bevy 0.10 announcement (Taffy 0.3 upgrade): https://bevy.org/news/bevy-0-10/
- Sibling: [governance.md](governance.md), [architecture.md](architecture.md), [ecosystem.md](ecosystem.md)
