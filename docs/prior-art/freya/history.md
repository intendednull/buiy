**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — history (2022 genesis → 0.4 rewrite)

# History

Freya's history is one of **a single maintainer's spare-time project** that has accreted commits, sponsors, and release-candidates for three and a half years without (a) committing to a 1.0, (b) attracting sustained co-maintainers, or (c) achieving demonstrable production adoption. The longevity is impressive; the bus factor remains 1.

## Timeline

| Date | Event |
|---|---|
| **2022-07-27** | Repo created on GitHub by Marc Espín Sanz (`marc2332`). Initial scope: a Dioxus renderer using Skia. |
| **late 2022 – 2023** | Early `0.0.x` and `0.1.x` versions. Element model + `rsx!` macro integration. Torin layout engine emerges in `crates/torin/`. |
| **2024** | `0.2.x` series. Component library (`freya-components`) fleshes out — Slider, Calendar, VirtualScrollView. AccessKit integration lands. |
| **2024 – early 2025** | `0.3.x` series. Hot-reload (Subsecond-based) support arrives via Dioxus 0.6's hot-patch support. |
| **2025-06-02** | **`0.3.4`** released — the most recent **stable** tag (latest non-RC). |
| **late 2025 – early 2026** | The **0.4 rewrite** begins. Per the official site: *"a huge percentage of Freya is being rewritten in PR #1351."* The `main` branch diverges substantially from `0.3.x`. |
| **2026-02-24** | First public 0.4.0 release candidate: `v0.4.0-rc.10`. |
| **2026-03-01** | `v0.4.0-rc.11`. |
| **2026-03-05** | `v0.4.0-rc.12`. |
| **2026-03-08** | `v0.4.0-rc.13`. |
| **2026-03-16** | `v0.4.0-rc.14`. |
| **2026-03-21** | `v0.4.0-rc.15`. |
| **2026-03-26** | `v0.4.0-rc.16`. |
| **2026-04-03** | `v0.4.0-rc.17`. |
| **2026-04-11** | `v0.4.0-rc.18`. |
| **2026-04-23** | **`v0.4.0-rc.19`** — current latest (as of this corpus's date 2026-05-22). |

The release-candidate cadence — **roughly every 1–2 weeks for several months** — is a strong active-development signal but also indicates that 0.4 stable is not close. There is no public 0.4 ship-date commitment.

## The 0.4 rewrite (PR #1351)

The Freya website describes 0.4 as *"a huge percentage of Freya rewritten in PR #1351."* The rewrite's scope, per public commit-message and site-text scanning:

- **Render scheduler overhaul** — frame-loop and dirty-tracking restructured.
- **Torin layout API refinements.**
- **Dioxus 0.6 integration depth** — broader use of `Store`-style accessors and the newer signal patterns.
- **Hot-reload reliability** — Subsecond-driven hot-patch coverage extended.

This kind of mid-version rewrite parallels what [`../dioxus/history.md`](../dioxus/history.md) calls Dioxus's 0.4→0.5 "100K-line, 1,400-commit, multi-quarter rewrite." Pre-1.0 Rust UI frameworks routinely undergo a "version that rewrites a substantial percentage of the codebase" — Freya 0.3→0.4 fits the pattern.

## What the timeline tells you

- **Multi-year continuous commitment.** 3.5 years of nights-and-weekends from one maintainer is not nothing. Compare to the half-life of similar Rust GUI experiments (`kayak-ui` archived 2024, `bevy_cosmic_edit` archived 2025, `woodpecker-ui` mostly dormant).
- **Pre-1.0 with no 1.0 in sight.** The 0.x → 0.x churn is not slowing; each minor undergoes substantial API changes. Production adoption requires API stability that Freya has not committed to.
- **0.4 rc churn is the dominant story right now.** Anyone evaluating Freya today is evaluating *0.4-rc behavior*, not 0.3.x — the rc cadence implies the rc API is itself evolving rapidly.
- **Single-maintainer bus factor.** Marc Espín is the only person making strategic decisions. The 7 GitHub sponsors signal community appreciation but not co-maintainership. If Marc steps away, there is no documented succession.

## Comparison to similar-track projects

| Project | Created | Latest stable | Latest pre-release | Lead maintainer | Adoption signal |
|---|---|---|---|---|---|
| Freya | 2022-07 | 0.3.4 (2025-06) | 0.4.0-rc.19 (2026-04-23) | marc2332 (1) | Modest |
| Slint | 2020 | 1.16.1 (2026-04-23) | n/a (1.x stable) | SixtyFPS GmbH (~10) | OTIV, KDAB, Espressif |
| Iced | 2019 | 0.13.x | n/a | Héctor Ramón (1–2 core + many contrib) | pop-os, Halloy |
| egui | 2020 | 0.30.x | n/a | Emil Ernerfeldt (1 core + many contrib) | Rerun, Mullvad, many tools |
| Dioxus | 2022 | 0.7.x | n/a | DioxusLabs (~3–5 paid + many contrib) | YC + sponsors funding |
| Floem | 2023 | 0.2.x | n/a | Lapce contributors | Lapce editor |
| GPUI | (Zed internal) | within Zed crates | n/a | Zed Industries (paid team) | Zed editor itself |

Freya is the only project in this list with both **active multi-year solo development** *and* **rc-churn as the current ship state**. Slint and Iced both crossed the post-1.0 / mature-0.x line; Freya has not.

## How this informs Buiy's planning

- **Validates that single-substrate Rust GUI projects are viable long-term.** Freya is 3.5 years in and shipping. Buiy's commitment to Bevy-as-substrate has the same long-term-viability profile if approached with similar discipline.
- **Demonstrates that pre-1.0 rewrites are normal.** Buiy will undergo at least one substantial mid-version refactor before 1.0. Plan accordingly — don't promise API stability before it's earned.
- **Bus factor is the load-bearing risk for solo projects.** Buiy must avoid the Freya-style bus-factor-1 situation. Foundation governance is open ([open question § Crate-split refinement and others](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)) — single-maintainer-as-policy is not acceptable for the Buiy use cases.

## Sources

- Freya releases — https://github.com/marc2332/freya/releases
- GitHub repo metadata (created 2022-07-27) — https://api.github.com/repos/marc2332/freya
- Freya site — https://freyaui.dev/ (mentions PR #1351 rewrite)
- Cross-references: [`../dioxus/history.md`](../dioxus/history.md), [`../slint/history.md`](../slint/history.md), [`README.md`](README.md), [`critiques.md`](critiques.md).
