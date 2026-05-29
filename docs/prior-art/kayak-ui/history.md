**Date:** 2026-05-22
**Status:** archived
**Subject:** kayak_ui — version timeline, Bevy compat per release, rise + plateau + silent abandonment.

# History

## Genesis (late 2021 – mid 2022)

The kayak_ui GitHub repository was **created on 2021-11-24** by StarArawn (John). The pre-0.1 development period (~12 months, late 2021 → late 2022) was on Bevy `main`-tracking branches; the first crates.io release waited for the architecture to settle. StarArawn was simultaneously the maintainer of `bevy_ecs_tilemap` (his largest project by stars; 1.2k★ as of 2026-05) and had earlier shipped `harmony` (a wgpu game-engine experiment, now archived). The Bevy community context: pre-1.0 engine, quarterly breaking-release cadence, no canonical UI story upstream, multiple competing third-party UI experiments (`belly`, `bevy_egui`, `bevy_lunex`, `kayak_ui`).

## Version timeline (crates.io)

Verified from [crates.io API](https://crates.io/api/v1/crates/kayak_ui) on 2026-05-22:

| Version | Date | Bevy compat | Notable changes |
|---|---|---|---|
| 0.1.0 | 2022-11-13 | 0.9 | Initial crates.io release. |
| 0.2.0 | 2022-12-11 | 0.9 | Multi-context support, native **MSDF font rendering**, computed styles (breaking), subpixel text, grid-layout style props. |
| 0.3.0 | 2023-04-16 | 0.10.x | Context-management overhaul, batched rendering, infinite-loop bug in sibling check fixed. Bevy 0.10 retrofit. |
| 0.4.0 | 2023-04-30 | 0.10.x | **SVG rendering**, widget transitions / animations, box shadows, custom rendering capabilities, clipping fixes. |
| 0.4.1 | 2023-05-02 | 0.10.x | Patch: tree-issues-on-widget-removal, `&str` (not `&'static str`) for widget keys, `OnEvent`-clone crash fix. |
| **0.5.0** | **2024-02-11** | **0.12** | **Final release.** Focus tree as a resource, dashmap improvements, key-entity / widget-state-management fixes, tree-removal bug resolution. |

Cadence interpretation: 0.1 → 0.4.1 fit a brisk **six-month sprint** (Nov 2022 → May 2023) tracking Bevy 0.9 → 0.10. Then **a nine-month gap** before 0.5.0 — first sign of slowing maintainer bandwidth. 0.5.0 skipped Bevy 0.11 entirely; consumers ran on the `bevy-track` branch (per README compat table) if they needed 0.11 support.

## The release-vs-Bevy timing problem

Bevy 0.13 shipped on **2024-02-17** — six days after kayak_ui 0.5.0 (2024-02-11). kayak_ui 0.5.0 was already obsolete-on-arrival the week of its release. From that point onward, every additional Bevy release widened the gap:

| Bevy release | Date | kayak_ui status |
|---|---|---|
| 0.13 | 2024-02-17 | no kayak_ui release |
| 0.14 | 2024-07-04 | no kayak_ui release (last `main` commit 4 days later: 2024-07-08) |
| 0.15 | 2024-11-30 | no kayak_ui release |
| 0.16 | 2025-04-23 | no kayak_ui release |
| 0.17 | 2025-09-30 | no kayak_ui release |
| 0.18 | 2026-01-30 | no kayak_ui release |

Five major Bevy releases without a kayak_ui follow-up, spanning ~24 months from 0.13 (Feb 2024) to 0.18 (Jan 2026).

## The decline — what actually happened

1. **0.5.0 ships pinned to Bevy 0.12** (2024-02-11). The release notes call out fixes for the load-bearing widget-state-management bugs — implying the maintainer had been working on the engine internals, not on new widgets.
2. **Bevy 0.13 ships six days later** (2024-02-17). kayak_ui doesn't release a 0.13 compat point. Issue tracker is quiet — no "blocked on 0.13 migration" ticket appears.
3. **Last `main` commit lands 2024-07-08** — five months after 0.5.0. The commits in this window are small fixes, not a 0.13 migration.
4. **woodpecker_ui repo created 2024-07-18** — ten days after the last kayak_ui commit. StarArawn pivots to a successor with explicit "simpler internals" framing (woodpecker_ui README: "*Kayak UI suffered from overly complicated internals that made contributing much too difficult and caused quite a few fundamental bugs.*" / "*reduced the primary system from over 1,000 lines to fewer than 200*").
5. **No archive banner, no deprecation notice on kayak_ui's README** — the maintainer simply moved. The crates.io page still shows 0.5.0 as the default-version; no `cargo yank`s; no README rewrite.
6. **Issue tracker freezes ~mid-2023** — the most recent open issue is from 2023-07-29 (#277), predating even 0.5.0. The community itself stopped filing new issues against kayak_ui before the maintainer stopped responding to old ones.

The community-side silence before the maintainer-side silence is notable. By 0.5.0, downstream consumers had already drifted — either to `bevy_egui` (immediate-mode, much smaller maintenance surface), to `bevy_lunex` (different layout/component model), or to hand-rolling on bevy_ui directly (which by Bevy 0.13 had grown enough to be tolerable for many use cases).

## Why abandoned without ceremony

See [`why-abandoned.md`](why-abandoned.md) for the structural analysis. The short version: a solo-maintained, pre-1.0, parallel-stack, custom-DSL UI crate against an engine on a quarterly breaking-release cadence has no equilibrium. The maintainer chose to start over rather than continue paying the migration tax — a defensible engineering decision that is *also* a structural data point Buiy should learn from.

## The transition to woodpecker_ui

woodpecker_ui (created 2024-07-18, latest release 0.1.1 on 2025-05-31, last commit 2025-06-07) is the explicit successor. Per its README:

- "*A Bevy ECS driven user interface crate.*" — emphasis on **ECS-driven**, a swing back from kayak_ui's React-driven model.
- Renderer: **Vello**, not kayak_ui's custom MSDF + quad pipeline.
- Layout: **Taffy**, not morphorm.
- Text: **Parley**, not kayak_ui's MSDF text.
- "*Similar syntax to Kayak UI*" but "*much much simpler backend.*"

The substrate flips (Vello + Taffy + Parley) align woodpecker_ui with what bevy_ui's own roadmap is converging on — particularly the Bevy 0.19-dev migration of `bevy_text` from cosmic-text to Parley + swash (see [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Top-of-file finding 2). woodpecker_ui has also slowed (~one year between last commit and the present moment); the structural problem may be re-emerging in v2 form. This is worth tracking but is **out of scope for this folder** — that's woodpecker_ui's prior-art folder to write, when one is needed.

## Sources

- kayak_ui crates.io API — https://crates.io/api/v1/crates/kayak_ui
- kayak_ui releases — https://github.com/StarArawn/kayak_ui/releases
- kayak_ui repo metadata (creation, last-push, archive status) — https://api.github.com/repos/StarArawn/kayak_ui
- Bevy release dates — https://github.com/bevyengine/bevy/releases
- Bevy 0.12 release post — https://bevy.org/news/bevy-0-12/
- Bevy 0.13 release post — https://bevy.org/news/bevy-0-13/
- woodpecker_ui README — https://github.com/StarArawn/woodpecker_ui#readme
- woodpecker_ui repo metadata — https://api.github.com/repos/StarArawn/woodpecker_ui
- StarArawn GitHub profile — https://github.com/StarArawn
- kayak_ui open issues — https://github.com/StarArawn/kayak_ui/issues
