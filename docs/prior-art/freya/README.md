**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — Skia-rendered, Dioxus-reactive native GUI framework for Rust

# Freya

Freya is a cross-platform, native (non-web) GUI library for Rust **powered by Skia** for rendering and **Dioxus** for reactivity. It was created by **Marc Espín Sanz** (`marc2332`, Barcelona) in **July 2022** as a personal "spare-time" project and remains a primarily single-maintainer codebase three and a half years later. It is the closest existing-art for *"Skia + reactive Rust UI"* in the ecosystem — but Buiy chose **wgpu (via Bevy's render graph)**, not Skia, and **Bevy ECS observers**, not Dioxus signals, so the borrowable lessons are the *authoring shape* and the *Skia primitive set as inspiration for Buiy shaders*, not the substrate itself.

## Honest assessment

**Strong points.**

- **Three+ years of continuous development.** Repo created 2022-07-27; current pre-release is **0.4.0-rc.19** (2026-04-23), with rc.18 (2026-04-11), rc.17 (2026-04-03), rc.16 (2026-03-26), rc.15 (2026-03-21) — release-candidate cadence is roughly twice a month. The 0.4.0 line has been in rc since at least 2026-02. Active by any reasonable bar.
- **Skia integration is real.** Uses `freya-skia-safe 0.96.1` (a fork of `rust-skia`'s `skia-safe`) with `textlayout`, `svg`, and `webp` features enabled. Inherits Skia's full rendering surface: rounded clipping, gradients (linear/radial/conic), shadows (drop + inner), blur, color filters, blend modes, SVG, text rendering (Skia's text layout, **not cosmic-text**).
- **AccessKit integration is in-tree.** `accesskit 0.24.0` + `accesskit_winit 0.32.0` are workspace dependencies — Freya is an AccessKit-producing framework on every native target.
- **Own layout engine ("Torin").** Custom pure-Rust layout crate at `crates/torin/`, version 0.4.0-rc.19, with its own model (not flexbox-as-spec; see [`layout-and-styling.md`](layout-and-styling.md)). Crate description: *"UI layout Library designed for Freya."* Note: Torin is theoretically usable outside Freya but has no other known consumers.
- **MIT-licensed; small but committed contributor base.** Marc has 7 GitHub Sponsors. He is affiliated with `@tauri-apps` and `@dioxus-community` organizations.
- **Crates.io traction is modest but consistent.** 33,720 total downloads, 3,768 recent (last 90 days). ~2.8k GitHub stars; ~120 forks.

**Weak points / honest caveats.**

- **Single-maintainer.** Marc Espín is the primary contributor. The "rest of contributors" are mostly drive-by PRs. No backing company — Marc explicitly self-describes the project as *"working on Rust projects in my spare time."* Bus-factor risk is real.
- **Pre-1.0, with no committed 1.0 timeline.** Stable line is **0.3.4** (released ~June 2025); the **main** branch has been rewritten substantially since then (PR #1351 referenced on the website as "a huge percentage of Freya rewritten"). The 0.4.0 line has been on release-candidate for several months and counting.
- **Dioxus-locked.** Freya is structurally a Dioxus renderer: it uses `dioxus 0.6.3+` for reactivity, components, and the `rsx!` macro. Any breaking change in Dioxus core flows directly into Freya. See [`reactive-model.md`](reactive-model.md).
- **Skia C++ dependency is large.** `skia-safe` wraps Skia C++; build times are long, binary sizes large, and the dependency chain includes CMake + Clang. Not in the spirit of "pure Rust." See [`skia-rendering.md`](skia-rendering.md) and [`critiques.md`](critiques.md).
- **Small production adoption.** The official site lists *features* but no production-user gallery. No verified flagship production app. Compare to Slint's KDAB/OTIV references or Iced's `pop-os`/`Zed`-side adopters.
- **Misattribution risk.** Earlier briefs for this corpus claimed Freya uses **cosmic-text**. That is **wrong** (verified). Freya uses Skia's `textlayout` feature directly for text — see [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) line 35 and [`skia-rendering.md`](skia-rendering.md) below.

**For Buiy specifically.** Freya is the *spiritual cousin* of what Buiy would be if Buiy had picked Skia + Dioxus instead of wgpu + Bevy. The architectural lessons are interesting; the substrate is not borrowable. The strongest single takeaway is in [`lessons.md`](lessons.md): Dioxus's reactivity primitives are reusable beyond Dioxus core, validating Buiy's *option* of adding a signal layer above Bevy observers later (foundation [open question § Reactivity layer](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)).

## Key facts (verified)

| Fact | Value |
|---|---|
| Crate | [`freya`](https://crates.io/crates/freya) |
| Latest stable | **0.3.4** (June 2025) |
| Latest pre-release | **0.4.0-rc.19** (2026-04-23) |
| Recent rcs | rc.18 (2026-04-11), rc.17 (2026-04-03), rc.16 (2026-03-26), rc.15 (2026-03-21), rc.14 (2026-03-16) |
| Repo | https://github.com/marc2332/freya |
| Created | 2022-07-27 |
| License | MIT |
| Stars / forks | ~2.8k / ~120 |
| Total downloads | 33,720 |
| Recent downloads | 3,768 (90-day) |
| Maintainer | Marc Espín Sanz (`marc2332`) — Barcelona, Spain |
| Sponsors | 7 (GitHub Sponsors) |
| Render substrate | **Skia** (via `freya-skia-safe 0.96.1`) |
| Text rendering | **Skia textlayout** (NOT cosmic-text) |
| Layout engine | **Torin** (own, in `crates/torin/`) |
| Reactivity | **Dioxus 0.6.3+** (signals + components + `rsx!`) |
| Accessibility | **AccessKit 0.24.0** + `accesskit_winit 0.32.0` |
| Windowing | `winit` |
| Async runtime | `tokio` |
| Platforms | Windows, macOS, Linux (desktop-only; no mobile, no WASM) |

## Table of contents

1. [`architecture.md`](architecture.md) — Skia + Dioxus + Torin + AccessKit composition; the `rsx!` macro shape; how reactivity drives Skia draws.
2. [`skia-rendering.md`](skia-rendering.md) — Skia integration via `freya-skia-safe`; the primitives Freya exposes (gradients, shadows, blur, rounded clip); text rendering via Skia textlayout.
3. [`reactive-model.md`](reactive-model.md) — Dioxus signals + components in Freya; differences from Dioxus core (desktop-only); hooks pattern.
4. [`layout-and-styling.md`](layout-and-styling.md) — Torin layout engine (own model, not Taffy); CSS-like styling props; theming.
5. [`accessibility.md`](accessibility.md) — AccessKit producer status; what's wired, what isn't.
6. [`history.md`](history.md) — 2022 genesis → 0.x evolution → the Dioxus dependency → the 0.4 rewrite.
7. [`distribution.md`](distribution.md) — License, single maintainer, governance, platform matrix.
8. [`ecosystem.md`](ecosystem.md) — Position in the Rust GUI landscape, comparisons vs Dioxus core / Iced / Slint / GPUI / Buiy.
9. [`critiques.md`](critiques.md) — Open problems, pre-1.0 churn, Skia C++ dep size, single-maintainer risk, small adoption.
10. [`lessons.md`](lessons.md) — **THE DECISION FILE.** Validates / Avoid / Borrow for Buiy.
11. [`glossary.md`](glossary.md) — Freya / Torin / Dioxus-renderer terminology.

## Brief corrections from pre-amble

- **"Freya uses cosmic-text"** — **WRONG.** Verified: Freya uses **Skia's `textlayout`** (`freya-skia-safe` with the `textlayout` feature). The cross-link in [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) line 35 already noted this. Re-verified against the workspace `Cargo.toml`.
- **"Layout engine: own? Taffy?"** — Resolved: **Torin** (own), in `crates/torin/`. Description: *"UI layout Library designed for Freya."* Dependencies: `euclid`, `rustc-hash`, `itertools`, `tracing`. See [`layout-and-styling.md`](layout-and-styling.md).
- **"AccessKit integration status"** — Resolved: **integrated** at workspace level (`accesskit 0.24.0` + `accesskit_winit 0.32.0`). Depth of tree-building unverified beyond presence — see [`accessibility.md`](accessibility.md).
- **"Maintainer name/details"** — Resolved: **Marc Espín Sanz** (`marc2332`, marc@mespin.me), Barcelona; self-described web frontend developer working on Rust in spare time; member of `@tauri-apps` and `@dioxus-community` organizations.

## Sources

- Freya repo — https://github.com/marc2332/freya
- Freya site — https://freyaui.dev/
- Freya docs.rs — https://docs.rs/freya/latest/freya/
- Freya releases — https://github.com/marc2332/freya/releases
- Freya workspace `Cargo.toml` — https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- Torin `Cargo.toml` — https://raw.githubusercontent.com/marc2332/freya/main/crates/torin/Cargo.toml
- Marc Espín GitHub — https://github.com/marc2332
- GitHub API metadata — https://api.github.com/repos/marc2332/freya
- Cross-references: [`../dioxus/lessons.md`](../dioxus/lessons.md), [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) (esp. line 35 — Freya does NOT use cosmic-text), [`../accesskit/lessons.md`](../accesskit/lessons.md).
- Buiy foundation — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md), [`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md).
