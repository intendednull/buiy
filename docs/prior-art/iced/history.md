**Date:** 2026-05-22
**Status:** active
**Subject:** iced — chronological history from Elm-port experiment to 0.14

# History

This file walks iced's version-by-version timeline, focused on the architectural pivots that matter for Buiy (renderer choice, text-engine evolution, AccessKit posture, mobile/WASM positioning, the COSMIC partnership). Companion to [`distribution.md`](distribution.md) (release table) and [`governance.md`](governance.md) (who shipped what).

## Pre-history: 2019 — the Elm port

Héctor Ramón ([hecrj](https://github.com/hecrj)) began iced in 2019. His own framing on the GitHub Sponsors page: *"5 years ago, I decided to create my own games while contributing to open-source"* — iced spun out of his earlier `coffee` game engine experiments.

- **2019-05-29** — `iced 0.0.0` placeholder published to crates.io.
- **2019-09-05** — `0.1.0-alpha`. First substantive release. The library is a port of the Elm Architecture (Model / Message / update / view) to Rust, originally as a sister project to `coffee`.
- **2019-11-25** — `0.1.0-beta`.

## 0.1 (2020-04-02) — first stable; wgpu and event subscriptions

The 0.1 series established the major architectural primitives that survive to 0.14:

- The **Elm Architecture** (`Model` / `Message` / `update` / `view`) as the application contract. See [Agent A's `elm-architecture.md`](elm-architecture.md) for the full pattern.
- A **wgpu-based renderer** as the default — iced bet on wgpu before wgpu had even hit 0.5, ahead of every other major Rust GUI library. This was a strategic bet: own the GPU pipeline, don't depend on system GTK/Qt/native widgets.
- **Event subscriptions** as the side-effect primitive (long-lived streams that produce `Message` values). Cited in the original 0.1.0-alpha entry.
- **`0.1.1`** (2020-04-15) — bug-fix follow-up.

## 0.2 (2020-11-26) — `iced_glow`, the OpenGL alternative

iced added an OpenGL-via-`glow` renderer as a fallback for the still-young wgpu. From the CHANGELOG: *"An OpenGL renderer powered by `iced_graphics`, `glow`, and `glutin`. It is an alternative to the default `wgpu` renderer."* This branched the renderer architecture into two backends — a pattern that survives today as `iced_wgpu` + `iced_tiny_skia`, though `iced_glow` itself was retired in 0.10 (see below).

## 0.3 (2021-03-31) — first big API churn

`wgpu` bumped to 0.7. Widget API refinements that anchored the public surface used through 0.9. No major architectural changes.

## 0.4 (2022-05-02) — async runtime, experimental WebGL

The async story consolidated around `iced_futures`. `webgl` feature added — the first time a `wasm32-unknown-unknown` target was made plausible for production-style usage. The 0.4 release notes describe this as **"experimental WebGL support"** and the qualifier still applies in 0.14.

## 0.5 → 0.9 (Nov 2022 – Apr 2023) — the COSMIC era begins

System76 announced the COSMIC desktop's Rust rewrite in 2022 and picked iced as the UI substrate. This is the inflection point of iced's adoption. The release cadence visibly accelerated:

- **0.5.0** (2022-11-10) — widget styling overhaul.
- **0.6.0** (2022-12-07) — `Canvas` widget improvements.
- **0.7.0** (2023-01-14) — **widget-driven animations**, the first animation primitive that wasn't subscription-based.
- **0.8.0** (2023-02-18) — *Custom Application Theming*. Themes become a first-class concern.
- **0.9.0** (2023-04-13) — `Theme` API consolidation.

## 0.10 (2023-07-28) — the text-engine switch (ab_glyph → cosmic-text)

**The single largest text-rendering change in iced's history.** PR [#1697](https://github.com/iced-rs/iced/pull/1697) — *"Text shaping, font fallback, and `iced_wgpu` overhaul"* — landed on 2023-02-24 and shipped in 0.10. The PR description: *"introduces support for text shaping and font fallback"* via integration with [`cosmic-text`](https://github.com/pop-os/cosmic-text), replacing the `wgpu_glyph` + `glyph-brush` dependency stack.

Concrete effects:

- iced gains complex-script shaping (Arabic, Indic, CJK), font fallback against system fonts, color emoji.
- The `wgpu_glyph` / `glyph-brush` / `ab_glyph` rasterization stack is dropped. (Note: the CHANGELOG doesn't mention `ab_glyph` by name; it was a transitive dependency through `glyph-brush`. PR #1697's text replaces this whole tower.)
- A new sister library, [`glyphon`](https://github.com/grovesNL/glyphon), is introduced as the cosmic-text → wgpu adapter and consumed by `iced_wgpu`.
- `iced_glow` and `iced_glutin` (OpenGL renderers) are deprecated in this same release. The renderer pair becomes `iced_wgpu` (GPU) + `iced_tiny_skia` (CPU software fallback).
- The CHANGELOG also lists *"Software renderer, runtime renderer fallback, and core consolidation"* — `iced_tiny_skia` is introduced.

This is a load-bearing fact for Buiy: **iced's text stack is cosmic-text, not Parley**. The brief that produced this folder asserted "Parley text"; that is wrong as of 0.14. Buiy's own cosmic-text commitment (foundation [architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) thus shares a substrate with iced, not a competing engine. See [Agent A's `text-and-cosmic.md`](text-and-cosmic.md) for the full text-rendering walkthrough and [`docs/prior-art/cosmic-text/lessons.md`](../cosmic-text/lessons.md) for cosmic-text's own state.

## 0.11 — never released

There is no `0.11.x` on crates.io. The 0.10 → 0.12 gap (~7 months) corresponds to an internal architectural refactor laying groundwork for multi-window support. iced's release-cadence policy is "ship when ready, not on a calendar" — see [`distribution.md`](distribution.md) § "Release cadence."

## 0.12 (2024-02-15) — multi-window

PR [#1964](https://github.com/iced-rs/iced/pull/1964) — *Multi-window support*. The single most-requested feature in the previous two years. The single `Application` trait becomes capable of managing multiple winit windows from one `Model` / `update` / `view` triad. Each window can have its own `view` projection.

This is the architectural decision that, for Buiy, makes iced's per-window approach legible: iced commits to *one process = one Model = many windows*, exactly the model bevy_ui adopted via its `Window` entity per winit `WindowId`. See [`docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Borrow #7 for the parallel.

`0.12.1` (2024-02-22) followed with bug fixes.

## 0.13 (2024-09-18) — task model and subscription overhaul

The async surface was reworked. The old `Command<Message>` was replaced by `Task<Message>`, with cleaner composition (`Task::done`, `Task::batch`, `Task::map`). Subscription-management simplified. `0.13.1` (2024-09-19) bug-fix follow-up.

The 0.13 release was the first cycle to deliberately *not* add new widgets, focusing on infrastructure quality.

## 0.14 (2025-12-07) — testing, devtools, animation API, headless

The largest single release in iced's history, ~15 months in the making. Key additions (PR numbers from CHANGELOG, verified against the 0.14.0 release notes):

- **Reactive rendering** ([#2662](https://github.com/iced-rs/iced/pull/2662)) — only re-render dirty regions.
- **Time-travel debugging** ([#2910](https://github.com/iced-rs/iced/pull/2910)) — gated behind the `time-travel` feature; uses `iced_devtools`.
- **First-class end-to-end testing** ([#3059](https://github.com/iced-rs/iced/pull/3059)) — `iced_tester` ships.
- **Headless testing** ([#2698](https://github.com/iced-rs/iced/pull/2698)) — for CI runs.
- **Hot reloading** ([#3000](https://github.com/iced-rs/iced/pull/3000)) — `hot` feature.
- **Animation API for application code** ([#2757](https://github.com/iced-rs/iced/pull/2757)) — first proper animation primitive, replacing the manual `Subscription::frame` polling pattern. See [`critiques.md`](critiques.md) § "Animation primitives."
- **Input method (IME) support** ([#2777](https://github.com/iced-rs/iced/pull/2777)) — preedit handling improvements.
- **Auto text-shaping strategy** ([#3048](https://github.com/iced-rs/iced/pull/3048)) — `text::Shaping::Auto` picks Basic vs Advanced per run.
- **`cryoglyph`** — iced-rs forked `glyphon` to `iced-rs/cryoglyph` in March 2025 and replaced the dependency. Same cosmic-text underneath, different upstream control.
- **wgpu 22.0 → 27.0** bump.
- **Rust edition 2024**, MSRV 1.88.
- New widgets: `table`, `grid`, `pin`, `float`, `wrap`, `sensor`, plus a richer `stack`.
- **Oklch-color-space theme-palette generation** — theme contrast computation now uses Oklch, an APCA-adjacent perceptual model.

## What hasn't happened (as of 0.14)

- **No AccessKit integration.** Issue [#552](https://github.com/iced-rs/iced/issues/552) (*"Implement accessibility support"*) has been **open since 2020-10-05**. See [`open-problems.md`](open-problems.md).
- **No iOS / Android target.** The README still lists only Windows / macOS / Linux / Web.
- **No Parley migration.** The Bevy-side `bevy_text` cosmic-text → Parley migration (issue [#21765](https://github.com/bevyengine/bevy/issues/21765), 2025-11-06) has no iced counterpart. iced's stake in cosmic-text is independent of Bevy's stake. See [Agent A's `text-and-cosmic.md`](text-and-cosmic.md).
- **No grid layout in the layout engine.** The new `grid` widget composes a fixed-shape table; it isn't a CSS-Grid-like solver. See [Agent A's `layout-engine.md`](layout-engine.md) and [`critiques.md`](critiques.md) § "No Grid layout."

## Sources

- iced CHANGELOG — https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- iced 0.14.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.14.0
- crates.io API — https://crates.io/api/v1/crates/iced
- PR #1697 (text shaping + font fallback + iced_wgpu overhaul) — https://github.com/iced-rs/iced/pull/1697
- PR #1964 (multi-window) — https://github.com/iced-rs/iced/pull/1964
- cryoglyph crate — https://crates.io/crates/cryoglyph
- iced book, Philosophy chapter — https://book.iced.rs/philosophy.html
- Bevy issue #21765 (bevy_text cosmic-text → Parley) — https://github.com/bevyengine/bevy/issues/21765
- iced issue #552 (accessibility) — https://github.com/iced-rs/iced/issues/552
