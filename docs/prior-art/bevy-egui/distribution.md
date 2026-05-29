**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — Cargo features, dependencies, egui-pin matrix, Bevy compat, platform support, MSRV, release cadence

# Distribution

`bevy_egui` is published on crates.io as a standalone third-party crate (`https://crates.io/crates/bevy_egui`); the repository is `https://github.com/vladbat00/bevy_egui`. Unlike `bevy_ui` / `bevy_feathers`, it is **not** part of the Bevy workspace — it tracks Bevy minor releases out-of-tree, on its own release schedule. Latest stable: **0.39.1** (2026-02-06). License: **MIT** (single-license, *not* MIT-OR-Apache-2.0 like Bevy itself or egui — see [`governance.md`](governance.md) § "License divergence"). Total downloads: **2,020,092**; 90-day downloads: **286,785**; 70 published versions since 2020-08-14 (fetched 2026-05-22). The highest-downloaded third-party Bevy UI plugin by a wide margin — see [`ecosystem.md`](ecosystem.md).

This file covers the *policy* layer; per-release timeline lives in [`history.md`](history.md). The "two-layer" upstream / downstream pin model is the load-bearing distribution shape; see [`governance.md`](governance.md) § "Two-layer maintenance model" for the *why* and this file for the *what*.

## Release cadence

vladbat00 ships **frequently** — typically within days-to-weeks of a new Bevy minor or a new egui minor. The pattern across the last two years:

| bevy_egui | Date | Trigger | Gap from previous |
|---|---|---|---|
| 0.30.0 | 2024-10-04 | egui 0.29 / paint callback | — |
| 0.31.0 | 2024-11-30 | Bevy 0.15 | ~2 months |
| 0.32.0 | 2025-01-06 | `bevy_picking` integration | ~5 weeks |
| 0.33.0 | 2025-02-16 | egui 0.31 | ~6 weeks |
| 0.34.0 | 2025-04-25 | multi-pass, input absorption, run conditions | ~10 weeks |
| 0.35.0 | 2025-06-30 | mesh-picking diegetic UI | ~9 weeks |
| 0.36.0 | 2025-08-04 | egui 0.32, render-position config | ~5 weeks |
| 0.37.0 | 2025-10-01 | Bevy 0.17, bindless textures | ~8 weeks |
| 0.38.0 | 2025-10-13 | egui 0.33, AccessKit re-enable | ~2 weeks |
| 0.39.0 | 2026-01-14 | Bevy 0.18 | ~13 weeks |
| 0.39.1 | 2026-02-06 | text anti-aliasing fix | ~3 weeks |

Every Bevy minor and every egui minor is a breaking-change event for bevy_egui. The crate makes no semver-stable promises — `0.x` is the only version line. Patch releases (`.1`, `.2`) ship for regressions within weeks; they do not introduce API.

## Bevy compatibility matrix

Verified against the README on `main` (2026-05-22). Mapping is `bevy` major-minor → `bevy_egui` range:

| Bevy | bevy_egui |
|---|---|
| 0.18 | 0.39 |
| 0.17 | 0.37–0.38 |
| 0.16 | 0.34–0.36 |
| 0.15 | 0.31–0.33 |
| 0.14 | 0.28–0.30 |
| 0.13 | 0.25–0.27 |
| 0.12 | 0.23–0.24 |
| 0.11 | 0.21–0.22 |
| 0.10 | 0.20 |
| 0.9 | 0.17–0.19 |
| 0.8 | 0.15–0.16 |
| 0.7 | 0.13–0.14 |
| 0.6 | 0.10–0.12 |
| 0.5 | 0.4–0.9 |
| 0.4 | 0.1–0.3 |

There is **no overlap window**: 0.39 supports only Bevy 0.18; previous bevy_egui lines do not back-port. Apps that need to support two Bevy versions simultaneously have to ship two `bevy_egui` lines. See [`critiques.md`](critiques.md) § "Version pinning frustration."

## egui version pins (the downstream layer)

bevy_egui pins a specific egui version per release. The pin lags egui upstream by **2–8 weeks** typically (egui ships, vladbat00 takes a release to absorb breaking changes, then bumps). Recent pins:

| bevy_egui | egui | egui release date |
|---|---|---|
| 0.39.1 | 0.33 | 2025-10-09 |
| 0.38.0 | 0.33 | 2025-10-09 |
| 0.37.0 | 0.32 | 2025-07-10 |
| 0.36.0 | 0.32 | 2025-07-10 |
| 0.35.0 | 0.31 | (early 2025) |
| 0.34.0 | 0.31 | (early 2025) |
| 0.33.0 | 0.31 | (early 2025) |
| 0.32.0 | 0.30 | (late 2024) |
| 0.31.0 | 0.30 | (late 2024) |

egui itself is currently at **0.34.2** (2026-05-04) on upstream — bevy_egui's 0.39.1 is therefore **one minor behind egui head** (egui 0.34 not yet absorbed; the next bevy_egui likely bumps to it). This **pin lag** is structural to the two-layer maintenance model and is the most common point of friction for consumers — see [`critiques.md`](critiques.md) § "egui pin lag."

## Cargo features (verified on `main`, 2026-05-22)

```toml
[features]
default = ["manage_clipboard", "open_url", "default_fonts", "render", "bevy_ui", "picking"]
accesskit = ["egui/accesskit", "bevy_a11y"]
immutable_ctx = []
manage_clipboard = ["arboard", "thread_local", "bytemuck", "egui/bytemuck"]
open_url = ["webbrowser"]
default_fonts = ["egui/default_fonts"]
render = ["bevy_asset", "bevy_core_pipeline", "bevy_image", "bevy_mesh", "bevy_render",
          "bevy_color", "bevy_shader", "bevy_transform", "encase", "bytemuck",
          "egui/bytemuck", "wgpu-types", "itertools"]
bevy_ui = ["bevy_ui_render"]
picking = ["render", "bevy_picking"]
serde = ["egui/serde"]
log_input_messages = []
log_file_dnd_messages = []
```

Twelve feature flags. The notable shape:

- **`default`** is broad — clipboard, URL opening, default fonts, render, bevy_ui integration, picking are all on. Apps that want a smaller dep tree have to `default-features = false` and pick what they need.
- **`accesskit`** is **opt-in, not default**. As of 0.38 (2025-10-13) AccessKit support was *re-enabled* (it had been turned off through 0.37 while waiting for upstream egui's a11y integration to stabilize) — but only as an optional feature. The default-off stance is deliberate: AccessKit pulls in `bevy_a11y` and bevy_egui's a11y story is still incomplete relative to retained-mode UI. See [`critiques.md`](critiques.md) § "Accessibility gaps."
- **`render`** is a meaningful gate — bevy_egui can run as a *measurement-only* dep (no shader pipeline) for headless use cases, which is unusual for a UI crate.
- **`bevy_ui`** is the integration with Bevy's own UI render order; off means egui paints over `bevy_ui` unconditionally.
- **`picking`** wires bevy_egui into `bevy_picking` so worldspace egui (mesh-attached) participates in hit-testing. Added in 0.32 (2025-01-06).
- **`immutable_ctx`** is a backward-compatibility flag — pre-0.11 (2022-02) the `EguiContext` resource was immutable; flipping this on restores the older shape for migration. The flag persists across releases as a permanent compatibility lever.
- **`log_input_messages`** / **`log_file_dnd_messages`** are debug feature flags that pipe input events to `bevy_log` — useful for harness work, default-off.

## Cargo dependencies

`bevy_egui` 0.39.1 depends on:

| Group | Crates / versions |
|---|---|
| egui (core, non-optional) | `egui = 0.33` |
| Bevy core (non-optional) | `bevy_app`, `bevy_camera`, `bevy_ecs`, `bevy_input`, `bevy_log`, `bevy_math`, `bevy_window`, `bevy_winit` — all `0.18.0` |
| Bevy render (optional, `render` feature) | `bevy_asset`, `bevy_core_pipeline`, `bevy_image`, `bevy_mesh`, `bevy_render`, `bevy_color`, `bevy_shader`, `bevy_transform` — all `0.18.0`; plus `bevy_ui_render` for `bevy_ui` feature, `bevy_picking` for `picking` feature, `bevy_a11y` for `accesskit` feature |
| Windowing | `winit = 0.30` |
| External (optional) | `arboard = 3.2.0` (clipboard, non-WASM/Android), `webbrowser = 1.0.1`, `encase = 0.12`, `itertools = 0.14`, `wgpu-types`, `bytemuck`, `thread_local` |

The non-optional Bevy crate list is **8 sibling crates** — narrower than `bevy_ui`'s 17 — because bevy_egui paints directly through Bevy's render graph rather than going through `bevy_ui`'s scene-style hierarchy. Adding `default-features` activates rendering and pulls the full set. See [`integration.md`](integration.md) for how the pieces fit together.

## Platform support

| Platform | Status | Notes |
|---|---|---|
| Windows | First-class | Full feature set. |
| macOS | First-class | Full feature set. |
| Linux (X11) | First-class | Requires XCB dev packages: `libxcb-render0-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`. IME issues on Linux fixed in 0.39.0. |
| Linux (Wayland) | First-class | Same XCB deps. |
| WASM (browser) | **Well-supported** | Live example at `vladbat00.github.io/bevy_egui/ui`. Web clipboard wired up since 0.26 (2024-03-18). Virtual keyboard since 0.30 (2024-10-04) but "rough around the edges" — only works when `prevent_default_event_handling = false`. See [`critiques.md`](critiques.md) § "Mobile / touch ergonomics." |
| Android | Best-effort | `arboard` clipboard disabled (use platform clipboard). Touch events wired up since 0.21 (2023-07-10). |
| iOS | Best-effort | Same shape as Android; no targeted testing in CI as of 2026-05-22. |

WASM support is one of bevy_egui's standout strengths relative to `bevy_ui` — egui upstream is heavily exercised on the web (via `eframe` and `egui_web`), so the substrate is mature, and bevy_egui inherits that. See [`use-cases.md`](use-cases.md) § "Web target."

## MSRV

`bevy_egui` 0.39.1 declares `rust-version = "1.89.0"` in `Cargo.toml`. This matches Bevy 0.18's MSRV. The crate does not promise a lower MSRV than the Bevy version it targets — it inherits whatever Bevy requires. See [`governance.md`](governance.md) for the policy implication.

## Sources

- bevy_egui Cargo.toml main — `https://raw.githubusercontent.com/vladbat00/bevy_egui/main/Cargo.toml` (fetched 2026-05-22).
- bevy_egui README main — `https://raw.githubusercontent.com/vladbat00/bevy_egui/main/README.md`.
- bevy_egui CHANGELOG main — `https://github.com/vladbat00/bevy_egui/blob/main/CHANGELOG.md`.
- bevy_egui releases — `https://github.com/vladbat00/bevy_egui/releases`.
- bevy_egui crates.io metadata — `https://crates.io/crates/bevy_egui` (fetched 2026-05-22).
- egui release tags — `https://github.com/emilk/egui/releases`.
- egui CHANGELOG — `https://github.com/emilk/egui/blob/main/CHANGELOG.md`.
- WASM demo — `https://vladbat00.github.io/bevy_egui/ui`.
- Sibling files: [`history.md`](history.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`integration.md`](integration.md), [`use-cases.md`](use-cases.md).
