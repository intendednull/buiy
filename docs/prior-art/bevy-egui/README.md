**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — the egui-on-Bevy bridge crate; the canonical immediate-mode UI in the Bevy ecosystem; paradigm contrast with Buiy's retained-mode bet

# bevy_egui

`bevy_egui` is a third-party Bevy plugin (vladbat00) that wraps the upstream `egui` immediate-mode GUI library (Emil Ernerfeldt / Rerun.io). It forwards Bevy input into `egui::RawInput` each frame, runs the user's egui code inside a Bevy schedule, and submits egui's tessellated `ClippedPrimitive` output through Bevy's render graph. It does *not* own a component model, a layout solver, or a persistent widget tree — egui itself doesn't have those. With **2,020,092 lifetime downloads** (286,785 in the last 90 days, 2026-05-22), it is by a wide margin the most-installed third-party Bevy UI plugin. Its closest paradigm cousin is Dear ImGui (Omar Cornut, 2014); its closest Bevy-ecosystem peer is `bevy_ui` itself, which is retained-mode and lives in a different problem space. See [`architecture.md`](architecture.md) for the per-frame mechanics and [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md) for the conceptual contrast.

**The corpus exists so Buiy designers can read bevy_egui's choices through the immediate-mode-vs-retained-mode lens.** Buiy is retained-mode (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.3). bevy_egui is immediate-mode. The two paradigms are not points on a continuum; they are different architectures with different costs that suit different workloads. **Buiy is not trying to be a better bevy_egui** — the two solve different problems. Most Bevy projects will consume both: bevy_egui for dev overlays / inspectors / editor panels, Buiy (or bevy_ui today) for shipped player-facing UI. See [`use-cases.md`](use-cases.md) § "The dev mode + ship mode pattern."

**Honest assessment.** bevy_egui is **dominant in dev tooling** and **absent from production game UI**. Its canonical consumer is `bevy-inspector-egui` (1.22M lifetime downloads, ~60% of bevy_egui's traffic by transitive dependency). Its strengths are genuine: rapid iteration, low API surface, reflection-driven inspectors, multi-window support since 2021, mesh-picking diegetic UI since 2025, opt-in AccessKit since 0.38 (2025-10-13), strong WASM. Its weaknesses are also genuine: flat `Visuals` styling (no token system, no cascade), animation primitives that stop at `animate_bool`/`animate_value`, layout simpler than Flexbox / Grid, AccessKit opt-in and structurally limited by per-frame tree rebuild, "looks like egui" homogeneity, MIT-only license (vs Bevy's dual), and a single solo maintainer behind the wrapper layer. No flagship commercial Bevy game uses bevy_egui for production UI (Tiny Glade and Foresight Spar both built custom UI). The Bevy editor experiments are shifting *away* from bevy_egui toward in-tree retained-mode (`bevy_feathers` + `bevy_ui_widgets`). See [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`open-problems.md`](open-problems.md).

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `bevy_egui` (third-party, not in Bevy workspace) |
| Latest stable | **0.39.1** (2026-02-06) |
| Bevy version pinned | 0.18.0 |
| egui version pinned | 0.33 (egui upstream is 0.34.2 — **one minor behind**) |
| License | **MIT only** (upstream egui is MIT OR Apache-2.0; Bevy is MIT OR Apache-2.0) |
| MSRV | rust-version 1.89.0 |
| Edition | 2024 |
| First release | 0.1.0, 2020-08-14 (4 days after Bevy launched) |
| Versions published | 70 |
| Lifetime downloads | 2,020,092 |
| 90-day downloads | 286,785 |
| Maintainer | Vladyslav Batyrenko (vladbat00, Mariupol, Ukraine) — solo, hobby |
| Upstream egui maintainer | Emil Ernerfeldt — Rerun.io co-founder + CTO; commercially backed |
| Repo | https://github.com/vladbat00/bevy_egui |
| Canonical downstream consumer | `bevy-inspector-egui` (1,224,818 lifetime, ~60% of bevy_egui traffic) |
| Multi-window since | 0.4.0 (2021-04-10) |
| `bevy_picking` integration | 0.32.0 (2025-01-06) |
| Multi-pass default since | 0.35.0 (2025-06-30) — single-pass DEPRECATED |
| Per-camera contexts since | 0.35.0 (2025-06-30) — was per-window before |
| Mesh-picking diegetic UI | 0.35.0 (2025-06-30) |
| AccessKit timeline | scaffolded 0.34 → disabled 0.37 → re-enabled OPT-IN 0.38 (2025-10-13) |
| `EguiPickingOrder` dynamic resource | 0.39.0 (replaced `PICKING_ORDER` const) |

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, ToC, framing disclosure. |
| [`lessons.md`](lessons.md) | **The consult-this-when-designing decision file.** Validates / avoid / borrow. |
| [`glossary.md`](glossary.md) | System-specific terms used across the corpus. |
| [`architecture.md`](architecture.md) | Plugin shape, schedule integration, render-graph node, per-camera contexts, picking integration, multi-pass loop. |
| [`api-surface.md`](api-surface.md) | The egui widget vocabulary (Window/Area/Panel/ScrollArea/Grid/etc.), `Ui` closure pattern, `Style`/`Visuals`/`Spacing`, AccessKit surface, touch/gamepad/IME. |
| [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md) | The conceptual hinge: what immediate-mode means, the `Id` system, when it wins (dev tools), when retained-mode wins (Buiy), lineage from Dear ImGui. |
| [`integration.md`](integration.md) | Setup, per-frame mechanics, coexistence with bevy_ui / bevy_lunex, multi-window via cameras, WASM, mobile, render-to-texture diegetic UI. |
| [`use-cases.md`](use-cases.md) | Where bevy_egui wins (dev tools, inspectors, debug overlays, prototypes) and where it doesn't (production HUD/menus, productivity apps with serious widget counts, WCAG-strict apps, BSN-authored UIs). The "dev mode + ship mode" pattern. |
| [`history.md`](history.md) | Origin of egui upstream (2018 as "Emigui," renamed 2020), bevy_egui genesis 2020-08-14, per-release milestone timeline through 0.39.1, Rerun.io's stewardship of egui. |
| [`distribution.md`](distribution.md) | Cargo features (12), Bevy compat matrix, egui-pin matrix (the two-layer pin), release cadence, MSRV, platform support. |
| [`governance.md`](governance.md) | Two-layer maintenance model (Rerun-funded upstream + solo-volunteer wrapper), funding asymmetry, license divergence, bus factor 1. |
| [`ecosystem.md`](ecosystem.md) | bevy-inspector-egui as the killer app, Bevy editor experiments shifting away, indie/hobbyist game adoption patterns, egui's own ecosystem (eframe, egui_extras, egui_plot, egui_tiles, catppuccin/egui). |
| [`critiques.md`](critiques.md) | Performance cost, styling limits, a11y structural limits, custom-widget complexity, touch/mobile gaps, homogeneity, pin lag, animation weakness, layout simplicity, WCAG gaps, the dev-tool-not-production-UI framing. |
| [`comparisons.md`](comparisons.md) | Row-by-row vs bevy_ui, bevy_lunex, bevy_feathers, woodpecker_ui, Dear ImGui, imgui-rs, Slint, Iced, Dioxus, and **Buiy** — the dev-tool-vs-production-UI axis chart. |
| [`open-problems.md`](open-problems.md) | What bevy_egui structurally doesn't solve: full APG conformance, i18n, theme tokens, gamepad UX, performance at 1000+ widgets, multi-window context lifecycle, render-graph evolution, WASM size, cadence misalignment, the production-game-UI gap. |

## How to use this corpus

1. **If you are designing a Buiy feature**, start at [`lessons.md`](lessons.md). The Top-of-File finding distinguishes which Buiy choices bevy_egui *validates* (most of them — Buiy and bevy_egui don't compete) from which pitfalls Buiy must *avoid* and which primitives are worth *borrowing* (multi-pass, per-camera contexts, mesh-picking, picking-order resource, etc.).
2. **If you are deciding whether a Buiy dev tool should ship on Buiy itself or on bevy_egui**, start at [`use-cases.md`](use-cases.md) § "The dev mode + ship mode pattern." The honest answer for most dev tooling: ship it on bevy_egui; build production UI on Buiy.
3. **If you are evaluating the immediate-mode-vs-retained-mode tradeoff**, start at [`immediate-mode-paradigm.md`](immediate-mode-paradigm.md). The file is the conceptual hinge for the whole corpus.
4. **If you are checking what production users prove about egui**, start at [`ecosystem.md`](ecosystem.md). Rerun is the legitimate "egui at scale" counterexample — but Rerun's streaming-data workload amortizes immediate-mode cost in a way that game UIs do not. Zed is *not* on egui (common misconception) — Zed is on GPUI.
5. **If you are tracking what shipped when**, start at [`history.md`](history.md). The per-release milestone list maps every notable feature to a version.
6. **If you are auditing the AccessKit / a11y story**, start at [`api-surface.md`](api-surface.md) § "Accessibility" and follow into [`critiques.md`](critiques.md) § "Accessibility" and [`open-problems.md`](open-problems.md) § "Accessibility."

## Cross-document inconsistencies surfaced

These were flagged during synthesis. Each is resolved in the linked file but called out here so future readers know where the original ambiguity lived.

- **egui pin currency.** As of 2026-05-22, bevy_egui 0.39.1 pins egui 0.33 while upstream egui is at 0.34.2 — one minor behind. [`distribution.md`](distribution.md) and [`history.md`](history.md) both report this; the pin moves with each bevy_egui release.
- **Single-pass deprecation.** The README on `main` says single-pass "may become deprecated," but the 0.35.0 changelog already marks the deprecation as having shipped. [`architecture.md`](architecture.md) and [`integration.md`](integration.md) report the deprecation as effective; the README phrasing is stale.
- **AccessKit feature default.** The 0.38.0 changelog says "AccessKit support re-enabled"; readers should not infer default-on. The `accesskit` feature is **opt-in** (not in `default-features`). [`distribution.md`](distribution.md), [`api-surface.md`](api-surface.md), and [`integration.md`](integration.md) all report opt-in.
- **Per-camera vs per-window contexts.** Pre-0.35.0 the model was one egui context per window. Since 0.35.0 each *camera* carries its own context, supporting split-screen and render-to-texture. [`architecture.md`](architecture.md), [`integration.md`](integration.md), and the [`history.md`](history.md) 0.35.0 entry all report the post-0.35 shape.
- **"Fortnight Studios" pre-amble claim.** Could not be verified — likely a typo for either "Fortnite" (which is Unreal C++, not egui) or for a different studio name. [`ecosystem.md`](ecosystem.md) carries this disclaimer rather than the unverified claim.
- **Zed and egui.** Zed (zed.dev) is sometimes mis-cited as an egui consumer in community discussions. Zed is on **GPUI** (Nathan Sobo's bespoke retained-mode Rust framework), not egui. [`use-cases.md`](use-cases.md) carries the correction; GPUI will be a separate prior-art folder.
- **License divergence.** bevy_egui is MIT-only; upstream egui and Bevy are both MIT OR Apache-2.0. [`distribution.md`](distribution.md) and [`governance.md`](governance.md) report this with the practical implications.

## Framing disclosure

This corpus is written from a **Buiy-retained-mode + Taffy-based + AccessKit-first + WCAG-2.2-AA + BSN-friendly + web-platform-parity** stance. The immediate-mode-vs-retained-mode paradigm is **foundational** to how every `Implications for Buiy` line reads: the corpus interprets bevy_egui's choices through Buiy's parallel-stack bet. Most evidence files frame bevy_egui's strengths as "wins for the dev-tool axis Buiy doesn't target" and bevy_egui's weaknesses as "validation of Buiy's retained-mode bet for production UI."

Future readers should weigh this carefully:

1. **Buiy is not trying to be a better bevy_egui.** The two solve different problems. bevy_egui's success in dev tooling is genuine and outside Buiy's scope. The honest read of the corpus is "bevy_egui validates immediate-mode for dev tools; the absence of flagship game UI on bevy_egui validates retained-mode for production UI; the two coexist."
2. **The corpus has an incentive to soft-pedal egui's strengths.** Rerun's production deployment of egui (multi-pane, streaming-data, performance-sensitive) is the legitimate counterexample to "egui doesn't scale." The corpus reports this honestly but it would be easy to under-emphasize. Future readers auditing "is immediate-mode really paradigm-tied to small-scale UI?" should pressure-test Rerun's pattern carefully — game UIs lack the always-changing-data property that makes Rerun's amortization work.
3. **The corpus has an incentive to over-emphasize bevy_egui's "dev tool only" framing.** Hobbyist Bevy games genuinely do ship UI on bevy_egui (often with the default look — the homogeneity problem). "No flagship commercial UI" is true but "no shipped UI" is not. Readers evaluating bevy_egui as a real choice for a small-team Bevy game should weigh this.
4. **The two-layer license + maintenance asymmetry is a structural risk Buiy avoids by owning its substrate deps directly.** This is real, not framing. Buiy depends on Taffy / cosmic-text / AccessKit / bevy_picking / wgpu directly, not through a wrapper crate — so Buiy doesn't inherit bevy_egui's two-cadence chase or the solo-wrapper-maintainer bus factor.

## Sources

- bevy_egui on crates.io — https://crates.io/crates/bevy_egui
- bevy_egui crates.io API metadata (fetched 2026-05-22) — https://crates.io/api/v1/crates/bevy_egui
- bevy_egui repository — https://github.com/vladbat00/bevy_egui
- bevy_egui README @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/README.md
- bevy_egui CHANGELOG @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/CHANGELOG.md
- bevy_egui Cargo.toml @ main — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/Cargo.toml
- bevy_egui releases — https://github.com/vladbat00/bevy_egui/releases
- egui upstream — https://github.com/emilk/egui
- Rerun.io (egui's commercial backer) — https://www.rerun.io/
- bevy-inspector-egui — https://github.com/jakobhellermann/bevy-inspector-egui
- AccessKit — https://accesskit.dev
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_ui prior-art folder — [`../bevy-ui/`](../bevy-ui/)
- bevy_ui lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
