**Date:** 2026-05-22
**Status:** archived
**Subject:** kayak_ui — third-party React-style declarative UI library for Bevy; abandoned-without-banner case study.

# kayak_ui

> **Why archived (in this corpus).** Last release `0.5.0` on **2024-02-11** — ~27 months silent as of 2026-05-22. The crate was pinned to **Bevy 0.12**; Bevy 0.13 shipped six days later (2024-02-17) and kayak_ui never gained 0.13 compatibility. The GitHub repository is **not formally archived** (no archive banner; `archived: false` per the GitHub API on 2026-05-22), but the last commit on `main` was **2024-07-08** and the maintainer pivoted to a successor crate, [`woodpecker_ui`](https://github.com/StarArawn/woodpecker_ui), created **2024-07-18** — ten days after kayak_ui's last commit. We document kayak_ui here as a **passive-abandonment** case study (contrast: `bevy_cosmic_edit` is a deliberate-archive case study, see [`../bevy-cosmic-edit/`](../bevy-cosmic-edit/)). The synthesis lives in [`lessons.md`](lessons.md) — read that first if you came here from a Buiy spec.

## What it was

kayak_ui was a third-party Bevy plugin that brought a **React-style declarative UI** model to Bevy. Widgets were declared via a custom `rsx!` proc-macro, composed into trees, painted by kayak_ui's own renderer on top of `bevy_render`, and laid out by the [morphorm](https://github.com/vizia/morphorm) one-pass layout engine (not Taffy, not bevy_ui's layout). The crate ran in parallel to `bevy_ui`, not as an extension to it — its own context, its own focus tree, its own render pipeline, its own widget vocabulary. At peak (Bevy 0.10–0.12 era, mid-2023) it was the most-cited answer in the Bevy community to "I want a real declarative UI in Bevy."

It is the canonical instance of a different anti-pattern from `bevy_cosmic_edit`'s bridge-crate failure: **solo-maintained, pre-1.0, parallel-stack third-party UI with a custom DSL**, where the maintenance load (Bevy migration every ~3 months × `rsx!` macro × custom renderer × custom layout × custom focus) exceeded one volunteer's bandwidth. See [`why-abandoned.md`](why-abandoned.md).

## Key facts

| Field | Value |
|---|---|
| Crate name | `kayak_ui` |
| Latest (final) version | **0.5.0** (2024-02-11) |
| Bevy compat at final release | **0.12** |
| Bevy 0.13 ship date (never followed) | 2024-02-17 (6 days after kayak_ui 0.5.0) |
| Last `main` commit | **2024-07-08** |
| Repository | `https://github.com/StarArawn/kayak_ui` |
| GitHub archive flag | **false** (verified 2026-05-22 via `api.github.com/repos/StarArawn/kayak_ui`) — repo is **not formally archived** |
| Maintainer | StarArawn (John); solo |
| Other maintained projects (same maintainer) | `bevy_ecs_tilemap` (1.2k★, active), `woodpecker_ui` (kayak_ui successor, 70★) |
| Successor crate | [`woodpecker_ui`](https://github.com/StarArawn/woodpecker_ui), created 2024-07-18 (10 days after kayak_ui's last commit) |
| License | Dual **MIT OR Apache-2.0** (per LICENSE file). crates.io reports "Non-standard" because `Cargo.toml` declares `license-file = "LICENSE"` and omits the `license` field. |
| Total downloads (lifetime, May 2026) | 18,774 |
| Stars (2026-05-22) | 482 |
| Forks | 48 |
| Open issues at silent-abandonment | 29 (most recent opened 2023-07-29 — community itself fell silent before maintainer did) |
| Layout engine | **morphorm** 0.3 (one-pass, not Taffy) |
| Authoring DSL | `rsx!` proc-macro (custom; not BSN, not derive-macro-friendly) |
| Component model | React-style: function widgets + state hooks + props, on top of an ECS context |
| Description (crates.io) | "A UI library built using the bevy game engine!" |

## Table of contents

- [`architecture.md`](architecture.md) — the React-style declarative paradigm in Rust, the `rsx!` macro, plugin shape (`KayakContextPlugin` + `KayakUIPlugin` trait), render path, layout via morphorm, focus tree, reactivity model.
- [`api.md`](api.md) — the widget declaration API, widget vocabulary at 0.5.0 (`KayakApp`, `KButton`, `TextBox`, `KWindow`, ...), composition patterns, state + effect equivalents.
- [`history.md`](history.md) — version timeline 0.1 (Nov 2022) → 0.5 (Feb 2024), Bevy compat per release, peak community interest in 2023, the decline.
- [`why-abandoned.md`](why-abandoned.md) — the structural analysis. Custom-DSL + parallel-stack + solo-maintainer + pre-1.0 + Bevy quarterly cadence = unsustainable. Why this is a **passive abandonment** (no archive banner), contrasted with `bevy_cosmic_edit`'s deliberate archive.
- [`integration.md`](integration.md) — setup at 0.5.0, Cargo features, Bevy version compat table, license clarification, MSRV.
- [`critiques.md`](critiques.md) — peak production usage + community reception + critiques (custom DSL friction, single maintainer, pre-1.0 churn, weak APG/WCAG coverage) + comparisons (vs `bevy_ui`, vs `woodpecker_ui`, vs `bevy_egui`, vs Buiy).
- [`lessons.md`](lessons.md) — **the consult-this-when-designing file.** Validates / Avoid / Borrow for Buiy.
- [`glossary.md`](glossary.md) — system-specific terms.

## How to use this folder

This is a **learn-from-failure** artifact, paired with [`../bevy-cosmic-edit/`](../bevy-cosmic-edit/) as the two canonical structural-failure case studies for third-party Bevy UI work. They are different failure modes:

| Crate | Failure mode | Final state |
|---|---|---|
| `bevy_cosmic_edit` | Deliberate archive (2025-03-21). Maintainer publicly archived the repo; bridge-crate burden between cosmic-text + bevy_ui became untenable. | GitHub `archived: true`. Read-only. |
| `kayak_ui` | Passive abandonment (2024-02 → present). No archive banner; maintainer pivoted to `woodpecker_ui` without ceremony. Issues sit open. | GitHub `archived: false`. Last commit 2024-07-08. |

Both teach Buiy the same root lesson: **third-party Bevy UI crates with custom architecture (bridge, DSL, parallel-stack, ...) and a single maintainer do not survive Bevy's quarterly breaking-release cadence past ~2 years.** See [`lessons.md`](lessons.md) for the split.

When designing a Buiy feature: start at [`lessons.md`](lessons.md); read [`why-abandoned.md`](why-abandoned.md) before deciding any pattern from this corpus is worth borrowing; ignore [`api.md`](api.md) unless you specifically need to compare declarative-DSL shapes.

**Framing disclosure.** This folder is written from a Buiy-stance: parallel-to-bevy_ui, ECS-native, BSN-friendly-by-construction (per `../bevy-ui/lessons.md` § 1), explicitly *not* a custom-DSL crate. The "Lessons for Buiy" framing in [`lessons.md`](lessons.md) treats kayak_ui's silent fade as **validating** that stance. A reader auditing whether Buiy should instead try to revive or extend kayak_ui (or build on `woodpecker_ui`) should weigh this corpus accordingly: it's a learn-from-kayak_ui-into-ECS-native-not-custom-DSL artifact, not a neutral catalog.

## Cross-references

- [`docs/prior-art/bevy-cosmic-edit/`](../bevy-cosmic-edit/) — sister archived case study (deliberate archive, bridge-crate failure mode). Read [`bevy-cosmic-edit/lessons.md`](../bevy-cosmic-edit/lessons.md) for the deliberate-archive counterpart pattern.
- [`docs/prior-art/bevy-ui/`](../bevy-ui/) — the host ecosystem. Read [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Top-of-file finding 1 ("BSN has not landed; design for it") — kayak_ui's `rsx!` macro is exactly the *wrong shape* for the BSN-friendly-by-construction stance.
- [`docs/prior-art/taffy/`](../taffy/) — the layout engine bevy_ui + Buiy use. kayak_ui chose **morphorm** instead; see [`architecture.md` § Layout](architecture.md#layout).
- [`docs/prior-art/woodpecker-ui/`](../woodpecker-ui/) — kayak_ui's successor by the same maintainer.
- [`docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) — the Buiy architecture spec this folder informs (parallel-stack rationale, ECS-native authoring, no-custom-DSL stance).

## Glossary stub

See [`glossary.md`](glossary.md) for the full list. The most-cited terms:

- **`rsx!`** — kayak_ui's React-style JSX-analog proc-macro for declaring widget trees. Expands to ECS spawn calls. Not BSN, not Bevy-side; entirely third-party.
- **`KayakContextPlugin`** — the entry-point Bevy plugin consumers added to their `App`. Sets up systems and resources for kayak_ui's context.
- **`KayakRootContext`** — the per-app kayak_ui state container; holds widget tree, focus tree, layout cache, render data. Roughly analogous to a React root.
- **`KayakUIPlugin`** — a kayak_ui-internal trait (NOT a Bevy `Plugin`) that lets kayak_ui sub-modules extend a `KayakRootContext`.
- **morphorm** — the one-pass layout engine kayak_ui chose. Maintained by the [vizia](https://github.com/vizia) project. Not Taffy.

## Sources

- kayak_ui repository — https://github.com/StarArawn/kayak_ui
- kayak_ui crates.io listing — https://crates.io/crates/kayak_ui
- kayak_ui docs.rs (0.5.0) — https://docs.rs/kayak_ui/0.5.0/kayak_ui/
- GitHub API (archive-status verification, 2026-05-22) — https://api.github.com/repos/StarArawn/kayak_ui
- StarArawn GitHub profile — https://github.com/StarArawn
- woodpecker_ui (successor) — https://github.com/StarArawn/woodpecker_ui
- morphorm layout engine — https://github.com/vizia/morphorm
- Sibling files in this folder.
