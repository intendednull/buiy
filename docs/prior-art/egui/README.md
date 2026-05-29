**Date:** 2026-05-22
**Status:** active
**Subject:** egui — the dominant Rust immediate-mode GUI, Emil Ernerfeldt + Rerun.io stewardship, paradigm contrast with Buiy's retained-mode bet

# egui

`egui` is the most-downloaded Rust GUI library — 16,963,701 lifetime crates.io downloads at 0.34.2 (2026-05-04), 3.72M of them in the last 90 days. It is **immediate-mode**: every frame, user code calls procedural widget functions (`ui.button("Save")`, `ui.label("FPS: 60")`) that allocate a rectangle, paint into it, and return a `Response`; there is no widget tree, no component model, no persistent layout cache. The architecture was designed by Casey Muratori (2005), popularized in C++ by Omar Cornut's Dear ImGui (2014), and brought to Rust by **Emil Ernerfeldt** — who started Emigui on a train in late 2018, renamed it to egui in 2020, and now develops it full-time at **Rerun.io** (his streaming-data visualizer startup, founded 2022, whose Viewer is the canonical egui-at-scale production app). License is MIT OR Apache-2.0 dual; MSRV 1.92 on Rust 2024 edition. See [`architecture.md`](architecture.md) for the per-frame mechanics and [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) for the paradigm itself.

**The corpus exists so Buiy designers can read egui's choices through the immediate-mode-vs-retained-mode lens.** Buiy is retained-mode (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.3). egui is immediate-mode. The two paradigms are not points on a continuum; they are different architectures with different costs that suit different workloads. **Buiy is not trying to be a better egui** — the two solve different problems. A future "retained-mode Bevy UI for production games" reader should treat egui as a *paradigm contrast*, not a direct comparison. See [`lessons.md`](lessons.md) § "Top-of-file finding."

## Honest assessment

egui is **dominant in dev tooling** and **absent from production game UI**. Its canonical at-scale user (Rerun) is a streaming-data viewer whose UI is mostly 3D custom content with egui chrome around it — a workload where rebuild-every-frame cost is amortized against work you'd be doing anyway. Its strengths are real: rapid iteration (a working UI in 30 minutes), low API surface, reflection-driven inspectors (`bevy-inspector-egui` is the canonical demo), strong WASM, Emil's seven-year stewardship, multi-viewport since 0.24, multipass since 0.29, the Plugin trait API in 0.33.0, the unified Panel API in 0.34.0, AccessKit **always-on as of 0.34.0** (2026-03-26 — a major shift from the 0.20.0-introduced opt-in default), and the 0.34.0 text-stack swap from `ab_glyph` to **`skrifa` + `vello_cpu`** for variable-font support and noticeably sharper text.

Its weaknesses are also real: flat `Style`/`Visuals` (no token system, no cascade, no OS-preference binding for `prefers-contrast`/`prefers-reduced-motion`/`forced-colors`), animation primitives that stop at `animate_value`/`animate_bool`, layout simpler than Flexbox/Grid, ad-hoc `Id` hashing (collision pitfalls in loops), no first-class APG keyboard contracts, no first-class live regions, no HarfBuzz-level complex-script shaping, no BiDi by spec, no Skia backend (epaint owns rendering end-to-end), no flagship game UI shipped on egui, and the well-known **"looks like egui" homogeneity** — by ~2024 the community joke had crystallized that "you can spot an egui app at 100 meters." Visual distinctiveness is expensive enough that almost no one outside Rerun pays the cost.

The honest verdict, from [`critiques.md`](critiques.md): egui is the **right tool** for Rust dev tools, internal dashboards, Rerun-shaped streaming-data apps, settings panels, level editors, modding UI, and prototypes. It is the **wrong tool** for AAA in-game UI, polished consumer apps where distinctiveness matters, accessibility-critical apps where APG widget contracts are non-negotiable, complex-script i18n-critical apps, and 10k+-widget data-grid apps. Buiy occupies a different cell on the same scope-fit map.

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `egui` |
| Latest stable | **0.34.2** (2026-05-04) |
| Recent versions | 0.34.1 (2026-03-27), 0.34.0 (2026-03-26), 0.33.3 (2025-12-11), 0.33.2 (2025-11-13), 0.33.0 (2025-10-09) |
| Versions published | 61 |
| Total downloads | 16,963,701 |
| Recent downloads (90 d) | 3,721,205 |
| First release | 0.1.0, 2020-05-30 (Emigui predecessor started 2018-12-23; renamed 2020-08-10) |
| License | **MIT OR Apache-2.0** (dual) |
| MSRV | 1.92 |
| Edition | 2024 |
| Repo | https://github.com/emilk/egui |
| Author / lead | Emil Ernerfeldt (`@emilk`) |
| Commercial steward | Rerun.io (Emil co-founded ~2022; salary cover for several core contributors) |
| Paradigm | Immediate-mode (UI is a function call, not a data structure) |
| Workspace sub-crates | `eframe`, `egui_extras`, `egui-wgpu`, `egui_glow`, `egui-winit`, `egui_demo_lib`, `epaint` |
| External sibling crate | `egui_plot` (separate repo `emilk/egui_plot`; 6,765,876 downloads; 0.35.0 latest) |
| Backends | egui-wgpu (default), egui_glow (OpenGL/WebGL, smaller WASM) — **no Skia backend** |
| Text engine since 0.34.0 | **skrifa** (Linebender font parsing) + **vello_cpu** (CPU rasterizer) — replaced `ab_glyph` |
| AccessKit status | **always-on since 0.34.0** (2026-03-26); opt-in from 0.20.0 (2022-12-08) onward |
| Plugin API | Trait-based `egui::Plugin` since 0.33.0 (2025-10-09) — replaces older callback hooks |
| Panel API | Unified `Panel` since 0.34.0; legacy `SidePanel`/`TopBottomPanel`/`CentralPanel` aliased |
| Notable production user | **Rerun** (streaming-data visualizer; the dogfooding driver) |
| Notable tooling user | **Embark Studios** (Emil's former employer; internal dev tools) |
| Bevy bridge | `bevy_egui` (third-party, vladbat00; 2M downloads) — see [`../bevy-egui/`](../bevy-egui/) |
| Canonical Bevy consumer | `bevy-inspector-egui` (1.22M downloads, ~60% of bevy_egui traffic) |

## Contents

The corpus is organized as evidence files (Agent A: architectural / conceptual; Agent B: project lens / ecosystem) plus three synthesis files (this README, `lessons.md`, `glossary.md`).

**Synthesis (start here)**

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, contents, framing disclosure. |
| [`lessons.md`](lessons.md) | **The consult-this-when-designing decision file.** Validates / Avoid / Borrow for Buiy. |
| [`glossary.md`](glossary.md) | egui-specific terms used across the corpus. |

**Technical / architectural evidence (Agent A)**

| File | Subject |
|---|---|
| [`architecture.md`](architecture.md) | `Context` / `Ui` / `FullOutput` per-frame pipeline, `Memory`, `Id`, multi-pass, the plugin trait, backend abstraction. |
| [`api-surface.md`](api-surface.md) | Widget vocabulary, `Window`/`Area`/`Panel`/`ScrollArea`/`Grid`, `Response`/`Sense`, text-input, `egui_plot`. |
| [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) | **The conceptual hinge.** History (Muratori 2005 → Dear ImGui 2014 → egui 2018-2020), what you give up (animation, layout caching, stable a11y tree), when immediate-mode is right vs wrong, the dev/ship pattern. |
| [`styling-and-theming.md`](styling-and-theming.md) | `Style` + `Visuals`, dark/light presets, scoped overrides, the homogeneity pathway. |
| [`text-rendering.md`](text-rendering.md) | The text pipeline; the 0.34.0 `ab_glyph` → `skrifa` + `vello_cpu` switch; gaps vs cosmic-text. |

**Project lens evidence (Agent B)**

| File | Subject |
|---|---|
| [`history.md`](history.md) | Emigui (2018-12-23) → egui rename (2020-08-10) → 0.1.0 (2020-05-30) → 0.34.2 (2026-05-04); pandemic push, Embark dogfooding, Rerun stewardship. |
| [`governance.md`](governance.md) | Benevolent-dictator-plus-employer-investment model; Rerun's commercial role; contributor cluster; no foundation, no RFC process. |
| [`distribution.md`](distribution.md) | Crate split, Cargo features, MSRV (1.92), Rust 2024 edition, dual license, release cadence, platform support. |
| [`ecosystem.md`](ecosystem.md) | Rerun (canonical at-scale user), Embark (tooling), bevy_egui (Bevy bridge), `egui_plot` / `egui_extras`, community integrations. |
| [`comparisons.md`](comparisons.md) | Two-axis matrix (paradigm × scope) vs Slint, Iced, Dioxus, Druid/Xilem, Floem, GPUI, Dear ImGui, Buiy. |
| [`critiques.md`](critiques.md) | Homogeneity, performance at scale, a11y maturity ceiling, custom-widget complexity, touch/mobile, styling/layout/text-shaping limits, the production-game-UI gap, `Id` pitfalls. |
| [`open-problems.md`](open-problems.md) | Full APG/WCAG 2.2 AA conformance, BiDi + vertical writing + complex scripts, theme expressiveness, touch/gamepad UX, 10k+-widget scale, multi-window context lifecycle, custom render-pass interleaving, WASM bundle size, animation primitives, mobile maturity, the production-game-UI gap, Linebender stack absorption. |

## How to use this prior-art doc

1. **If you are designing a Buiy feature**, start at [`lessons.md`](lessons.md). The Top-of-file finding distinguishes which Buiy choices egui *validates* by its absence-in-production-game-UI from which pitfalls Buiy must *avoid* and which primitives are worth *borrowing* (the honest non-goals pattern, the Plugin trait, the unified Panel API, the AccessKit always-on stance, the `skrifa` + `vello_cpu` text stack as an alternative to study).
2. **If you are evaluating the immediate-mode-vs-retained-mode tradeoff**, start at [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md). It is the conceptual hinge for the whole corpus.
3. **If you are checking what production users prove about egui**, start at [`ecosystem.md`](ecosystem.md). Rerun is the legitimate at-scale counterexample — but Rerun's streaming-data workload amortizes immediate-mode cost in a way that game UIs do not. Zed is *not* on egui (common misconception) — Zed is on GPUI.
4. **If you are tracking what shipped when**, start at [`history.md`](history.md). The 0.20.0 AccessKit landing, the 0.29 multipass, the 0.33.0 plugin trait, the 0.34.0 AccessKit-always-on + skrifa + Panel-unification — every notable feature maps to a version.
5. **If you are auditing the AccessKit / a11y story**, start at [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) § "Stable accessibility tree" and follow into [`critiques.md`](critiques.md) § "Accessibility maturity" and [`open-problems.md`](open-problems.md) § "Full ARIA APG / WCAG 2.2 AA conformance." The structural ceiling is the load-bearing finding.
6. **If you are auditing the Bevy story**, cross-link to [`../bevy-egui/`](../bevy-egui/). That folder covers the bridge crate; this folder covers the upstream `egui` itself.

## Framing disclosure

This corpus is written from a **Buiy-retained-mode + Taffy-based + cosmic-text-based + AccessKit-first + WCAG-2.2-AA + BSN-friendly + web-platform-parity** stance. The immediate-mode-vs-retained-mode paradigm is **foundational** to how every `Implication for Buiy` line reads: the corpus interprets egui's choices through Buiy's parallel-stack bet. Most evidence files frame egui's strengths as "wins for the dev-tool axis Buiy doesn't target" and egui's weaknesses as "validation of Buiy's retained-mode bet for production UI."

Future readers should weigh this carefully:

1. **Buiy is not trying to be a better egui.** The two solve different problems. egui's success in dev tooling is genuine and outside Buiy's scope. The honest read of the corpus is "egui validates immediate-mode for dev tools; the absence of flagship game UI on egui validates retained-mode for production UI; the two coexist." A future reader auditing "should Buiy support immediate-mode authoring inside the retained tree?" should treat this corpus as Buiy-stance evidence, not a neutral catalog — and pressure-test against [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) directly.
2. **The corpus has an incentive to soft-pedal egui's strengths.** Rerun's production deployment of egui (multi-pane, streaming-data, performance-sensitive) is the legitimate counterexample to "egui doesn't scale." The corpus reports this honestly but under-emphasis is a risk — particularly the part where most pixels in Rerun's viewer are *custom 3D content*, not egui widgets, so the immediate-mode rebuild cost amortizes against work Rerun would be doing anyway. Game UIs lack this property.
3. **The corpus has an incentive to over-emphasize the "production game UI gap."** Hobbyist Bevy games genuinely do ship UI on egui (via bevy_egui, often with the default look — the homogeneity problem). "No flagship commercial game UI" is true but "no shipped game UI at all" is not. Readers evaluating egui as a real choice for a small-team Bevy game should weigh this.
4. **The single-author-with-commercial-steward governance is a structural strength for egui at its current scale, and a risk that Buiy can't replicate.** Buiy is currently single-author (intendednull) without an analog of Rerun's salary cover; the documentation-first discipline (`docs/specs/`, `docs/plans/`, `docs/prior-art/`) is the mitigation, not a substitute. Emil + Rerun's model is borrowable in shape but not in funding.

## Sources

- egui crate on crates.io — https://crates.io/crates/egui
- egui crates.io API metadata (fetched 2026-05-22) — https://crates.io/api/v1/crates/egui
- egui repository — https://github.com/emilk/egui
- egui README @ master — https://raw.githubusercontent.com/emilk/egui/master/README.md
- egui CHANGELOG @ master — https://raw.githubusercontent.com/emilk/egui/master/CHANGELOG.md
- egui_plot — https://github.com/emilk/egui_plot
- egui_plot crates.io — https://crates.io/crates/egui_plot
- PR #7701 — AccessKit always-on — https://github.com/emilk/egui/pull/7701
- PR #7694 — skrifa migration — https://github.com/emilk/egui/pull/7694
- Rerun.io — https://www.rerun.io/
- AccessKit — https://accesskit.dev
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- Casey Muratori — *Immediate-Mode Graphical User Interfaces* (2005) — https://caseymuratori.com/blog_0001
- Dear ImGui — https://github.com/ocornut/imgui
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_egui prior-art folder — [`../bevy-egui/`](../bevy-egui/)
- bevy_egui lessons — [`../bevy-egui/lessons.md`](../bevy-egui/lessons.md)
