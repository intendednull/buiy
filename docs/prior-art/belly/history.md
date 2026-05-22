**Date:** 2026-05-22
**Status:** active
**Subject:** belly — project history, 0.x release cadence, and the 2024 stall

# History

belly is the work of a single developer, `jkb0o` (Konstantin "jkb0o" Korol), with occasional contributor PRs. The project started as a personal exploration of declarative UI patterns on Bevy and grew into a feature-complete-enough prototype to attract 436 GitHub stars over ~2 years.

## Release timeline

From the GitHub releases page (verbatim tag list + dates):

| Tag | Date | Bevy version targeted | Notes |
|---|---|---|---|
| `v0.1.1` | 2023-03-01 | 0.9 | "Major rework introducing new connections system and refactored Elements system parameter" |
| `v0.2.0` | 2023-04-01 | 0.10 | "bevy-0.10 support" |
| `v0.4.0` | 2024-03-13 | 0.12 | "bevy-0.12 support" |
| `v0.4.1` | 2024-03-16 | 0.12 | "Bevy 0.12 support (+Fix wrong dependency from 0.4)" |
| `v0.5.0` | 2024-04-20 | **0.13** | "Now compatible with bevy 0.13" — contributor Threadzless |

Five tagged releases over ~14 months of active development. No tag `v0.3.0` is visible in the releases list — either skipped intentionally or the release was created and removed.

## Activity pulse

Reconstructed from `git log` on the public repo:

- **Spring 2023** — `v0.1.1` / `v0.2.0`. Core authoring + cascade + bindings runtime in place.
- **Summer 2023** — Bevy 0.11 work (PR #65 merged 2023-09-20, with `[bump] 0.3` and `<follow>` widget added).
- **March–April 2024** — Bevy 0.12 (`v0.4.0` / `v0.4.1`) and 0.13 (`v0.5.0`) migrations land in rapid succession.
- **April 2024 → present** — **no commits to `main`.** Issue #83 ("Need help with updating to Bevy 0.14") was opened 2024-07-19 and remains unanswered.

The maintainer-side picture: belly was developed at hobby-pace through the Bevy 0.9–0.13 era, then development stopped abruptly around the Bevy 0.13 → 0.14 transition. No public statement of intent appears on the repo or in the issues. The repo is **not formally archived** — the maintainer simply stopped pushing.

## The crates.io question

The README explicitly states: `"As far as the project has no cargo release yet, the only way to discover all the features it has is to clone the repo and check out the examples."`

belly has **never been published to crates.io**. Verified:

```
$ curl https://crates.io/api/v1/crates/belly
{"errors":[{"detail":"crate `belly` does not exist"}]}
```

This is unusual for a project of belly's visibility (436 stars, ~14 months of development, four tagged releases). Most Bevy ecosystem crates of comparable scope are on crates.io. No public statement from the maintainer explains the choice; the working theory from community discussion threads is "the maintainer wanted to ship a stable API before claiming a crates.io name," which never happened before the project stalled.

The practical effect: anyone using belly in production must pin to a git ref. Cargo's git-dep semantics work, but git-deps disqualify a crate from being published to crates.io itself (Cargo policy). This effectively bars any other Bevy crate from depending on belly transitively unless that consumer is also git-only — a small but real ecosystem barrier.

## Genesis and authorship

belly's author `jkb0o` has the GitHub profile of a long-term Bevy ecosystem contributor. The project predates several now-better-known Bevy UI alternatives (bevy_lunex, sickle_ui, bevy_flair). Earliest public commits date to late 2022. The motivation appears to have been a personal need for declarative-UI ergonomics that bevy_ui (at the time, pre-Required-Components, pre-decomposed-style-components) did not offer.

The only contributor of note other than `jkb0o` is `Threadzless`, who contributed the Bevy 0.13 migration PR (#82) that became `v0.5.0` — the project's last release. This makes Threadzless the *last person to ship belly code*; the maintainer-of-record remains `jkb0o`.

## The stall

The most concrete signal of the project's status is issue [#83](https://github.com/jkb0o/belly/issues/83): "Need help with updating to Bevy 0.14", opened 2024-07-19, unanswered. The Bevy 0.14 release shipped 2024-07-04; belly's stall happened immediately as the upstream migration window opened.

Subsequent Bevy releases (0.15, 0.16, 0.17, 0.18 through 2026-01) have shipped without any corresponding belly update. The project has effectively been pinned to Bevy 0.13 — an API surface that is now ~2 years and 5 major versions stale.

The repo's lack of an archive flag is significant in one direction only: it preserves the possibility that `jkb0o` returns to maintain the project. There is no evidence in commit history or issue replies suggesting that is imminent.

## Implications for Buiy

The history file is short because the project's history is short — but the read-out is sharp:

1. **A single-developer ecosystem crate stalls when life changes.** belly's stall is the canonical Bevy-ecosystem version of the bus-factor problem. Buiy must not adopt belly as a runtime dependency, and any future Buiy stylesheet sub-spec must be in-tree.

2. **The crates.io decision is path-dependent.** belly's "no release yet" was a temporary stance that hardened into permanent unavailability. If Buiy ever ships extension crates, publish them to crates.io early — even at `0.0.x` — so the publication path is exercised before the project takes its first stall.

3. **The Bevy minor-release migration tax is real.** belly fell behind in one cycle and never caught up. Buiy's foundation [README.md § 1 goal 5](../../specs/2026-05-07-buiy-foundation/README.md) commits to "rolling latest-stable" — that's a pace belly's history says one person cannot sustain for long.

## Sources

- belly releases — https://github.com/jkb0o/belly/releases
- belly issue #83 — https://github.com/jkb0o/belly/issues/83
- belly v0.5.0 README — https://github.com/jkb0o/belly/blob/v0.5.0/README.md
- belly v0.5.0 Cargo.toml — https://github.com/jkb0o/belly/blob/v0.5.0/Cargo.toml
- crates.io check (`belly` does not exist) — https://crates.io/api/v1/crates/belly returns 404
- contributor Threadzless (PR #82) — https://github.com/jkb0o/belly/pull/82
- Cargo git-dep publishing policy — https://doc.rust-lang.org/cargo/reference/publishing.html
- Buiy foundation rolling-Bevy commitment — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1 goal 5
