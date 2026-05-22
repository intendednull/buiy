**Date:** 2026-05-22
**Status:** active
**Subject:** iced — distribution, crate layout, features, platform support, MSRV

# Distribution

This file inventories how iced ships: the workspace of crates published to crates.io, the Cargo features that gate optional capabilities, the platforms it claims to support, the MSRV, and the release cadence. Companion to [`architecture.md`](architecture.md) (system-level layout) and [`history.md`](history.md) (version-by-version chronology).

## Crate

- **Name:** `iced`
- **Latest version:** **0.14.0** (published 2025-12-07; `master` is 0.15.0-dev as of 2026-05-22)
- **License:** **MIT** (single — see [`governance.md`](governance.md) for divergence-from-Bevy-norm context)
- **Downloads:** 1,885,134 lifetime; 358,674 recent (90-day) per crates.io
- **Repository:** https://github.com/iced-rs/iced
- **Homepage:** https://iced.rs/
- **First publish:** 2019-05-29 (`0.0.0` placeholder); first real release `0.1.0-alpha` on 2019-09-05; `0.1.0` on 2020-04-02

## Workspace crates

iced is a Cargo workspace. The 0.14.0 release ships these member crates (every release publishes them in lockstep):

| Crate | Role |
|---|---|
| `iced` | The umbrella crate users depend on. Re-exports the public API. |
| `iced_core` | Core types: `Color`, `Point`, `Rectangle`, `Element`, layout primitives, font and text types. No renderer-specific code. |
| `iced_widget` | The widget catalogue (`button`, `text`, `text_input`, `scrollable`, `column`, `row`, `container`, `pick_list`, `slider`, `canvas`, `image`, `svg`, `markdown`, `qr_code`, plus the 0.14-era additions: `table`, `grid`, `pin`, `float`, `wrap`, `sensor`, `stack`). |
| `iced_runtime` | Runtime layer that sits above the renderer/winit: tasks, subscriptions, clipboard, window control. |
| `iced_renderer` | Façade that picks the actual backend (`iced_wgpu` or `iced_tiny_skia`) at compile time. |
| `iced_wgpu` | The default GPU renderer. Uses `wgpu` 27.0 and `cryoglyph` (an iced-rs fork of `glyphon` from March 2025) for text. |
| `iced_tiny_skia` | The CPU-software fallback renderer. Used when wgpu init fails. |
| `iced_winit` | Window + event-loop integration via `winit` 0.30. |
| `iced_graphics` | Backend-agnostic graphics types (mesh, gradient, paragraph, geometry); depends on `cosmic-text` directly. |
| `iced_futures` | Subscription / executor plumbing. Optional `tokio` and `smol` integrations. |
| `iced_highlighter` | Syntax highlighting for code-editor surfaces. |
| `iced_debug` / `iced_devtools` / `iced_tester` / `iced_test` / `iced_program` / `iced_beacon` / `iced_selector` | Newly added 0.14-era crates supporting time-travel debugging, hot reload, headless testing, end-to-end testing. |

## Cargo features (0.14.0)

Default feature set: `["wgpu", "tiny-skia", "crisp", "web-colors", "thread-pool", "linux-theme-detection", "x11", "wayland"]`.

Notable feature gates:

- `wgpu`, `wgpu-bare`, `tiny-skia` — backend selection. Both renderers can compile in the same binary; `iced_renderer` picks at runtime when wgpu init fails.
- `webgl` — wgpu's WebGL2 backend, the path used for `wasm32-unknown-unknown` builds.
- `image`, `image-without-codecs`, `svg`, `canvas`, `qr_code`, `markdown`, `lazy` — opt-in widgets.
- `tokio`, `smol`, `thread-pool` — async executor backend (one of three).
- `debug`, `time-travel`, `hot`, `tester` — devtools.
- `advanced` — exposes the renderer/widget-internals API for custom widgets.
- `basic-shaping` (default off) vs `advanced-shaping` — controls whether the text path uses full HarfBuzz-grade shaping (via cosmic-text + harfrust) or a fast-path Latin-only shaper. The widget API exposes `text::Shaping::Basic / Advanced / Auto` and 0.14 added the `Auto` strategy.
- `x11`, `wayland` (default) and `linux-theme-detection` — Linux-specific runtime knobs.
- `fira-sans` — bundles the Fira Sans font in the binary.
- `sipper` — opt-in for the streaming-task primitive used by long-lived `Task` returns.
- `selector` — registers a CSS-selector-like API for testing.

## Platform support

The README states: *"Cross-platform support (Windows, macOS, Linux, and the Web)"* — iOS and Android are **not** listed. Verified against the master branch README on 2026-05-22.

- **Desktop:** Windows, macOS, Linux. Full first-class support. winit-driven; both X11 and Wayland on Linux (default-on features).
- **Web:** wgpu's WebGL2 backend (`webgl` feature) targets `wasm32-unknown-unknown`. Functional for many examples, but it's a constrained subset (no system font enumeration via fontdb, no clipboard parity, no native file dialogs, no multi-window). The book and tutorials are desktop-first.
- **Mobile:** unofficial. No iOS or Android target in CI, no touch-first widgets, no soft-keyboard handling beyond what winit offers. See [`critiques.md`](critiques.md) § "Mobile support is limited" and [`open-problems.md`](open-problems.md) § "Mobile target maturity."

For comparison, see [`comparisons.md`](comparisons.md) — Slint and Dioxus both invest in mobile in ways iced has not.

## MSRV

`rust-version = "1.88"` in 0.14.0's workspace `Cargo.toml`. Edition 2024. Master branch (0.15.0-dev) has bumped to 1.92.

The MSRV bump pattern is aggressive — iced does not commit to a minimum-supported-Rust window the way Bevy or many SDK-style crates do. Each release tracks the latest stable Rust within ~3 months.

## Release cadence

Irregular. Releases-since-0.4 timeline (from the [CHANGELOG](https://github.com/iced-rs/iced/blob/master/CHANGELOG.md)):

| Version | Date | Gap |
|---|---|---|
| 0.4.0 | 2022-05-02 | — |
| 0.5.0 | 2022-11-10 | ~6 mo |
| 0.6.0 | 2022-12-07 | ~1 mo |
| 0.7.0 | 2023-01-14 | ~5 wk |
| 0.8.0 | 2023-02-18 | ~5 wk |
| 0.9.0 | 2023-04-13 | ~8 wk |
| 0.10.0 | 2023-07-28 | ~15 wk |
| 0.12.0 | 2024-02-15 | ~7 mo (no 0.11 released) |
| 0.13.0 | 2024-09-18 | ~7 mo |
| 0.14.0 | 2025-12-07 | ~15 mo |

Note: there is **no 0.11.x release**. The 0.10 → 0.12 gap reflects an internal architectural change (multi-window, in [`history.md`](history.md)).

The 14-month gap between 0.13 and 0.14 is the largest in the project's history and tracks the addition of the testing / devtools / animation API surface — see [`history.md`](history.md) § "0.14 cycle" and [`critiques.md`](critiques.md) § "Release-cadence drift."

## Sources

- crates.io API — https://crates.io/api/v1/crates/iced
- iced 0.14.0 Cargo.toml — https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml
- iced 0.14.0 graphics/Cargo.toml — https://github.com/iced-rs/iced/blob/0.14.0/graphics/Cargo.toml
- iced 0.14.0 wgpu/Cargo.toml — https://github.com/iced-rs/iced/blob/0.14.0/wgpu/Cargo.toml
- iced master CHANGELOG — https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- iced README — https://github.com/iced-rs/iced
- cryoglyph crate — https://crates.io/crates/cryoglyph
