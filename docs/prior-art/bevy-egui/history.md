**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — origin of egui upstream, bevy_egui genesis, per-release milestones, Rerun.io's stewardship of egui

# History

bevy_egui sits at the intersection of two timelines: **egui** (Emil Ernerfeldt's immediate-mode Rust GUI, started 2018) and **bevy_egui** (vladbat00's Bevy integration plugin, started 2020). This file walks both lines and shows where they meet.

## egui upstream genesis (2018–2020)

egui was started by Emil Ernerfeldt in **2018** as a personal project — originally named **Emigui** (a portmanteau of his initials and "GUI") and renamed to **egui** in 2020. Ernerfeldt had been writing Rust since ~2014 and full-time since 2018; egui grew out of his desire for a Rust-native immediate-mode UI in the style of Dear ImGui (the C++ progenitor — see [`comparisons.md`](comparisons.md) § "vs imgui"). The initial public crate landed on crates.io as `egui 0.1.0` in mid-2020.

Design lineage worth naming:

- **Dear ImGui** (Omar Cornut, 2014) — the C++ progenitor of the immediate-mode-GUI pattern that egui's API shape consciously echoes. The `ui.button("Click me")` idiom, the per-frame rebuild, the auto-layout-from-call-order all trace to ImGui.
- **Rust full-time** — Ernerfeldt's pivot from C++ to Rust dates to ~2018 and egui was the project he chose to push Rust GUI ergonomics on.
- **Rerun.io** (founded 2022, formally launched 2023) — Ernerfeldt co-founded Rerun, a computer-vision / robotics visualization SDK, and Rerun's viewer is built on egui. Rerun has been the primary commercial backer of egui's development since (see § "Rerun.io stewardship" below).

## bevy_egui genesis (2020-08-14)

`bevy_egui 0.1.0` was first published on crates.io on **2020-08-14** by Vladyslav Batyrenko ("vladbat00"). Bevy itself had been publicly announced **2020-08-10** (four days earlier) — bevy_egui is therefore one of the earliest third-party Bevy plugins of any kind, certainly the earliest UI integration. The very first release supported Bevy 0.4 and an early egui version (0.1.x); both projects were under a year old at the time of integration.

vladbat00 has been the sole maintainer from 2020-08-14 to 2026-05-22 — 5 years 9 months of continuous solo maintenance, 70 published versions. See [`governance.md`](governance.md) § "vladbat00 as solo maintainer."

## Per-release milestone timeline

Major milestones across bevy_egui's life. Patch versions (`.1`, `.2`) elided unless they shipped a notable fix.

### 2020 — bootstrap era (0.1)

- **0.1.0 (2020-08-14)** — first publish. Bevy 0.4 + egui 0.1.x. Basic single-window rendering, mouse + keyboard input forwarding, no clipboard, no WASM.
- **0.1.3 (2021-01-20)** — texture-copy alignment fix; closing out the bootstrap era.

### 2021 — keeping pace with Bevy + egui breaking changes (0.2–0.10)

- **0.2.0 (2021-02-08)** — egui 0.9 bump.
- **0.3.0 (2021-03-02)** — egui 0.10.
- **0.4.0 (2021-04-10)** — **multiple-windows support** lands. egui 0.11.
- **0.5.0 (2021-05-22)** — egui 0.12.
- **0.6.0 (2021-06-29)** — egui 0.13.
- **0.7.0 (2021-09-05)** — egui 0.14.
- **0.8.0 (2021-11-27)** — egui 0.15.
- **0.9.0 (2022-01-01)** — egui 0.16.
- **0.10.0 (2022-01-08)** — Bevy 0.6 bump.

Roughly every 6–10 weeks a release shipped, matching the upstream cadence of either Bevy or egui. The pattern of "follow upstream releases reactively" was established in this period and continues today.

### 2022 — clipboard + render-graph maturity (0.11–0.18)

- **0.11.0 (2022-02-04)** — mutable context getters introduced; immutable getters feature-gated (the `immutable_ctx` feature flag that still exists in 0.39 dates to this release).
- **0.12.0 (2022-03-12)** — egui 0.17; **arboard clipboard** replaces the prior platform-specific solution. Internal texture ID tracking.
- **0.13.0 (2022-04-16)** — Bevy 0.7.
- **0.14.0 (2022-05-01)** — egui 0.18; new-tab URL opening.
- **0.15.0 (2022-07-30)** — Bevy 0.8; **default-fonts replacement feature** (the `default_fonts` Cargo feature gate dates to this release).
- **0.16.0 (2022-08-24)** — egui 0.19.
- **0.17.0 (2022-11-13)** — Bevy 0.9; Windows resize fix.
- **0.18.0 (2022-12-11)** — egui 0.20.

### 2023 — touch + WebGPU stabilization (0.19–0.23)

- **0.19.0 (2023-01-15)** — swapchain texture panic fix.
- **0.20.0 (2023-03-08)** — Bevy 0.10 + egui 0.21; **multi-window support enhanced**; AltGr modifier support added.
- **0.21.0 (2023-07-10)** — Bevy 0.11 + egui 0.22; **touch-event support** lands (the load-bearing primitive for mobile / tablet input).
- **0.22.0 (2023-10-07)** — egui 0.23; component + resource extraction refactored.
- **0.23.0 (2023-11-05)** — Bevy 0.12; WebGPU color-attachment fix.

The 2023 line is significant: touch support in 0.21 was the first plausible path to mobile, and the WebGPU work in 0.23 made WASM rendering a viable target on modern browsers.

### 2024 — IME + worldspace + paint callbacks (0.24–0.31)

- **0.24.0 (2023-12-11)** — egui 0.24.
- **0.25.0 (2024-02-19)** — Bevy 0.13 + egui 0.26; **`render` feature gate** introduced (bevy_egui can now build without the render pipeline).
- **0.26.0 (2024-03-18)** — **web-clipboard support**; texture options now respected end-to-end.
- **0.27.0 (2024-04-18)** — fallible primary-window getter variants.
- **0.28.0 (2024-07-06)** — Bevy 0.14 + egui 0.28.
- **0.29.0 (2024-08-18)** — **worldspace UI** lands (initial capability); **paint-callback support** added — apps can inject custom render passes inside egui paint, the load-bearing primitive for shader effects and embedding 3D viewports.
- **0.30.0 (2024-10-04)** — `prepare_render` step support; **mobile virtual keyboard (web)**; **IME support** finally lands.
- **0.31.0 (2024-11-30)** — Bevy 0.15.

The 2024 line is when bevy_egui matured from "wrap egui in a Bevy plugin" to "first-class Bevy citizen with worldspace, render-pass injection, IME, and mobile-web inputs."

### 2025 — picking, multi-pass, AccessKit re-enable (0.32–0.38)

- **0.32.0 (2025-01-06)** — **`bevy_picking` integration**: egui participates in Bevy's unified pointer-pick pipeline. Type-conversion helpers between egui and Bevy types.
- **0.33.0 (2025-02-16)** — egui 0.31; `CopyImage` command support.
- **0.34.0 (2025-04-25)** — **multi-pass rendering** support: egui's multi-pass mode (`Context::request_discard`) wired through bevy_egui. **Input absorption + run conditions**: egui can selectively consume input events. **AccessKit integration** scaffolded (pending upstream egui release).
- **0.35.0 (2025-06-30)** — **mesh-picking for diegetic UI**: worldspace egui surfaces on 3D meshes (not just sprite-like quads). Context attachment refactored to per-camera rather than per-window. Result systems supported.
- **0.36.0 (2025-08-04)** — egui 0.32; **rendering position configurable relative to bevy_ui** (egui can paint *under* `bevy_ui`, not only over it).
- **0.37.0 (2025-10-01)** — Bevy 0.17; **bindless textures** for large UI texture sets; IME refinement.
- **0.38.0 (2025-10-13)** — egui 0.33; **AccessKit support re-enabled** as an opt-in feature (it had been disabled in 0.37 while waiting for egui upstream); IME toggle option; **partial texture updates** for large texture pages.

### 2026 — Bevy 0.18 (0.39)

- **0.39.0 (2026-01-14)** — Bevy 0.18; deprecated picking-order constant removed; Linux IME issues fixed.
- **0.39.1 (2026-02-06)** — text anti-aliasing fix (alpha pre-multiplication for egui textures).

The 0.39 line is the current stable. See [`distribution.md`](distribution.md) § "Release cadence" for the per-release pace table.

## egui upstream major versions (the second layer)

The egui-side timeline that bevy_egui has had to follow:

| egui | Notable |
|---|---|
| 0.1–0.5 (2020) | Bootstrap; rename from "Emigui" |
| 0.10 (2021-02) | Persistence layer |
| 0.16 (2022-01) | epaint extracted as a sub-crate |
| 0.18 (2022-05) | Painter cleanups; tessellator API |
| 0.20 (2022-12) | First-class accessibility scaffolding |
| 0.22 (2023-05) | Multi-pass infrastructure begun |
| 0.24 (2023-11) | Visuals refactor |
| 0.27 (2024-03) | Web text input + IME upstream |
| 0.28 (2024-07) | Atoms (token-based widget construction) groundwork |
| 0.31 (2025-01) | Atoms widening — Button/Checkbox/RadioButton on atoms |
| 0.32 (2025-07) | Atoms-based widget rebuild continues |
| 0.33 (2025-10) | AccessKit upstream-ready |
| 0.34 (2026-03) | Font rendering switched from `ab_glyph` to `skrifa` + `vello_cpu` — sharper text, font hinting + variations. ScrollArea fade edges. |

egui has been on a consistent ~3-month release cadence since ~2022, similar to Bevy's, but the cadences are *misaligned* — egui and Bevy don't release in the same week, so bevy_egui is regularly catching one or the other (rarely both at once). See [`open-problems.md`](open-problems.md) § "Misaligned upstream cadences."

## Rerun.io stewardship (~2022 onward)

Rerun.io was co-founded by Emil Ernerfeldt in 2022 (formally launched 2023) as a computer-vision / robotics visualization SDK. The Rerun viewer is built on egui — large-scale, performance-sensitive, multi-pane, multi-pass — and Rerun is the primary commercial entity sponsoring egui development. The relationship is unusual in Rust-UI-land: most Rust UI projects are unfunded volunteer work; egui has a paid backer with first-party production usage. Effects on the codebase:

- **Performance focus**: egui's per-frame layout cost has been progressively reduced as Rerun's data volumes scaled.
- **Multi-pane / tiling layouts**: `egui_tiles` (a tiling layout engine for egui) is a Rerun-published companion crate.
- **Custom widgets**: Rerun ships many custom widgets atop egui that motivate upstream API changes (e.g. atoms-based widget reconstruction in 0.28+).

The relationship does **not** extend to bevy_egui — Rerun is invested in egui upstream, not in the Bevy integration layer. bevy_egui remains a hobby project maintained outside any commercial structure (see [`governance.md`](governance.md) § "Funding asymmetry").

## vladbat00's role as maintainer

Vladyslav Batyrenko ("vladbat00") has been the sole maintainer of bevy_egui from 2020-08-14 to today, 5+ years. The pattern: vladbat00 absorbs upstream breaking changes (from either Bevy or egui), publishes a new release within days to weeks, and merges community PRs in between. The README of the repository ends with a note from vladbat00 about being from Mariupol, Ukraine, with a Patreon link — a personal context that the project does not formally depend on (no employer is associated with the project) but that the community is aware of.

The bus factor implication is treated in [`governance.md`](governance.md) § "Bus factor analysis."

## Sources

- bevy_egui CHANGELOG — `https://github.com/vladbat00/bevy_egui/blob/main/CHANGELOG.md`.
- bevy_egui releases — `https://github.com/vladbat00/bevy_egui/releases`.
- bevy_egui first crates.io publish — `https://crates.io/crates/bevy_egui` (initial version date 2020-08-14).
- egui repository — `https://github.com/emilk/egui`.
- egui release tags — `https://github.com/emilk/egui/releases`.
- "Emigui → egui" rename note — egui README on `main`.
- Software Engineering Daily podcast (Ernerfeldt) — `https://softwareengineeringdaily.com/2024/08/07/creating-guis-in-rust-emil-ernerfeldt/`.
- Rerun.io company background — `https://www.rerun.io/`; `https://rerun.io/blog/why-rust`.
- Emil Ernerfeldt GitHub profile — `https://github.com/emilk/emilk`.
- Bevy 0.1 release (engine launch context) — `https://bevy.org/news/introducing-bevy/`.
- Sibling files: [`distribution.md`](distribution.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`comparisons.md`](comparisons.md), [`open-problems.md`](open-problems.md).
