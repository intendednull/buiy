**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — native Rust UI library with fine-grained reactivity, the UI substrate for the Lapce editor

Floem is a native Rust UI library by the Lapce editor team. Its defining choices are (1) a fine-grained signal/effect reactivity layer inspired by `leptos_reactive`, (2) view functions that return `View`-implementing values composed declaratively, (3) the Linebender text stack (Parley + Swash + Fontique), and (4) a multi-backend renderer (vger / vello / skia / tiny-skia). It is the UI substrate that ships in Lapce, the Rust code editor by the same team.

Floem is interesting to Buiy for three reasons:

1. **Reactivity-layer reference.** Buiy's foundation §2.7 says "Observers + change detection only; no signal/computed/effect layer in v1." Floem is the closest production-shipping example of what a Rust-native signal layer for UI looks like. If/when Buiy revisits §2.7 in a follow-up sub-spec, Floem (alongside Dioxus 0.5+ and Xilem) is one of the three references to read.
2. **Parley vs cosmic-text divergence.** Floem uses Parley + Swash + Fontique (the Linebender stack). Buiy chose cosmic-text (see [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md)). The Floem choice is the same choice Bevy 0.19-dev is making with its `bevy_text` migration (issue #21765). Floem is therefore a data point on the Parley side of the divergence; the Buiy stance on cosmic-text holds independently, but Parley's ecosystem traction is real.
3. **Single-flagship dogfooding.** Floem is dogfooded by exactly one production app — Lapce. That makes Floem a useful comparand for any Buiy planning about how many flagship users the foundation needs before claiming v1.

## Key facts

| Item | Value |
|---|---|
| Crate | `floem` |
| Latest published version | **0.2.0** (2024-11-15) |
| Published version count | 3 (0.1.0, 0.1.1, 0.2.0) |
| Total downloads (crates.io) | ~15,352 |
| Repo | https://github.com/lapce/floem |
| Most recent `main` commit | **2026-05-11** ("recover from lost/outdated wgpu surface on Wayland", PR #1074) |
| License | **MIT** (single, not dual) |
| Total commits on `main` | ~1,036 |
| Steward | Lapce team |
| Primary downstream | Lapce editor (https://lapce.dev) |
| Reactivity model | Fine-grained signals (inspired by `leptos_reactive`) |
| Text engine | Parley 0.7.0 + Swash 0.2 + Fontique 0.7.0 |
| Layout engine | Taffy 0.9.2 (with `grid` feature) |
| Render backends | wgpu via `vger` or `vello`; Skia (GPU); `tiny-skia` (CPU fallback) |
| Window/event loop | Custom winit fork (`lapce/winit` aka `floem-winit`) |
| Platform support | Windows, macOS, Linux (experimental WASM in 0.2.0) |
| Accessibility (AccessKit) | **Not integrated.** Issue [#8](https://github.com/lapce/floem/issues/8) "Support Accessibility via AccessKit" open since 2023-04-14 with no progress. |
| MSRV | 1.91 |

## Active-or-archived assessment

**Status: active**, despite a 17–18-month gap between published crates.io versions.

The signal we initially flagged — "only 3 published versions in 4 years; 18 months silent on crates.io" — is real but **misleading taken alone**. Direct inspection of the `main` branch shows continuous PR activity:

- 2026-05-11 — PR #1074, Wayland wgpu surface recovery
- 2026-04-25 — PR #1071, Rust 1.95 clippy cleanup
- 2026-04-11 — PR #1063, "Faster style v2"
- 2026-03-30 — PR #1059, `understory` bump
- … and steady commit cadence going back through 2025.

What appears to be happening: Floem is developed primarily as Lapce's UI dependency, consumed by Lapce as a git dependency (not a crates.io release), with crates.io releases cut only occasionally. This is a real pattern in the Rust ecosystem (cf. winit forks held by editors) but it has consequences:

- Downstream non-Lapce users on crates.io are pinned to 0.2.0 from November 2024 and miss 17+ months of fixes.
- The `floem-winit` and `understory_*` dependencies are Lapce-team-owned forks/sister-crates, which deepens the Lapce-only practical pathway.

For Buiy: treat Floem as **active for Lapce's purposes, but not as a production-stable dependency for outside consumers** until the cadence question resolves. See [`distribution.md`](distribution.md) for the release-cadence discussion and [`critiques.md`](critiques.md) for the broader cadence + dogfooding critique.

## Table of contents

- [`architecture.md`](architecture.md) — Runtime structure: view functions, reactive runtime, render pipeline, custom-winit event loop.
- [`fine-grained-reactivity.md`](fine-grained-reactivity.md) — Signals, effects, derived, batch. Lineage: Solid.js → `leptos_reactive` → Floem. Comparison with Dioxus signals and React's coarse re-render.
- [`text-and-parley.md`](text-and-parley.md) — Parley + Swash + Fontique. Editor / syntax-editor examples. Comparison with the cosmic-text choice Buiy made.
- [`layout-and-styling.md`](layout-and-styling.md) — Taffy 0.9.2 integration, the "Faster style v2" pipeline (PR #1063), theme system, responsive module.
- [`accessibility.md`](accessibility.md) — The AccessKit gap. Issue #8, contrast with egui / fltk-rs.
- [`history.md`](history.md) — Lapce origin (~2018, Dongdong Zhou), Floem extraction (~2023), 0.1 → 0.2 timeline, 18-month crates.io silence.
- [`distribution.md`](distribution.md) + [`governance.md`](governance.md) — Combined: MIT license, Lapce-team stewardship, no full-time devs (HN 2024), release-cadence reality.
- [`ecosystem.md`](ecosystem.md) + [`comparisons.md`](comparisons.md) — Combined: Lapce is the only confirmed production user; comparison with Dioxus, Xilem, Iced, egui, Solid.js, Buiy.
- [`critiques.md`](critiques.md) + [`open-problems.md`](open-problems.md) — Combined: release cadence, single-app dogfooding, AccessKit gap, ecosystem health, documentation, mobile/WASM gaps.
- [`lessons.md`](lessons.md) — **The decision file.** Validates / Avoid / Borrow for Buiy.
- [`glossary.md`](glossary.md) — Signal, Effect, Derived, View, Element, etc.

## Recommended reading order

For a Buiy designer evaluating a Floem-inspired choice:

1. [`README.md`](README.md) (you are here) — orient.
2. [`lessons.md`](lessons.md) — the decision distilled.
3. [`fine-grained-reactivity.md`](fine-grained-reactivity.md) — the substantive borrow target if Buiy revisits §2.7.
4. [`critiques.md`](critiques.md) + [`open-problems.md`](open-problems.md) — the cost side.
5. Subsystem files as needed.

## Cross-links

- [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) — Floem uses Parley; cosmic-text is Buiy's pick. The divergence is documented there and in [`text-and-parley.md`](text-and-parley.md).
- [`../accesskit/lessons.md`](../accesskit/lessons.md) — Floem has no AccessKit integration; #8 has been open since 2023. Buiy commits to AccessKit-first.
- [`../dioxus/signals-and-state.md`](../dioxus/signals-and-state.md) — Dioxus 0.5+ signals; same lineage (Solid.js → leptos_reactive) but different framing.
- Buiy foundation §2.7 — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) — the "no signal layer in v1" decision Floem helps frame.

## Sources

- Floem repo — https://github.com/lapce/floem
- Floem on crates.io — https://crates.io/crates/floem
- Floem on docs.rs — https://docs.rs/floem/latest/floem/
- Lapce — https://lapce.dev and https://github.com/lapce/lapce
- AccessKit integration request — https://github.com/lapce/floem/issues/8
- Floem 0.2.0 release notes — https://github.com/lapce/floem/releases
- Cargo.toml on `main` (workspace deps) — https://github.com/lapce/floem/blob/main/Cargo.toml
- Lapce HN discussion (2024-02) confirming no full-time devs — https://news.ycombinator.com/item?id=39423493
- InfoQ "Lapce is a Native Open-Source Code Editor Written in Rust" — https://www.infoq.com/news/2024/03/lapce-rust-editor/
