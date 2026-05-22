**Date:** 2026-05-22
**Status:** active
**Subject:** egui — crate split, cargo features, platform support, MSRV, release cadence

# Distribution

egui ships as a small family of Rust crates under one Cargo workspace at `github.com/emilk/egui`. All crates are dual-licensed **MIT OR Apache-2.0** (verified in workspace root `Cargo.toml`). Every crate in the workspace shares one version number — the workspace releases atomically.

## Crate facts (verified 2026-05-22)

| Field | Value |
|---|---|
| Latest version | **0.34.2** (2026-05-04) |
| Versions on crates.io | 61 |
| First crates.io release | **2020-05-30** (0.1.0); GitHub project started 2018-12-23 as "Emigui" |
| Workspace edition | **2024** |
| Workspace MSRV (`rust-version`) | **1.92** (raised from 1.88 in 0.34.0 — see [history.md](history.md)) |
| Workspace license | MIT OR Apache-2.0 (dual) |
| Author | Emil Ernerfeldt (`emilk`) |
| Steward | Rerun.io (employer; see [governance.md](governance.md)) |
| Total downloads (egui crate) | 16,963,701 |
| Recent downloads (90 d, egui crate) | 3,721,205 |

The pre-amble's "61 versions since 2020-04" rounds the timeline: serious work began 2020-04-01 (Emil's pandemic project), the first crates.io publish was 2020-05-30, and the "egui" rename happened 2020-08-10. The GitHub repo itself goes back to 2018-12-23 as Emigui.

## Workspace crate split

From `Cargo.toml` `[workspace] members`:

| Crate | Role |
|---|---|
| `egui` | The immediate-mode UI core: widgets, layout, `Context`, `Ui`, input handling. |
| `epaint` | 2D paint primitives + tessellator. egui's render output is `epaint::ClippedPrimitive`s. |
| `emath` | Tiny math crate (`Vec2`, `Pos2`, `Rect`). Used by egui + epaint. |
| `ecolor` | Color types (`Color32`, `Rgba`, etc.). |
| `epaint_default_fonts` | Bundled font assets (Hack, Ubuntu-Light, NotoEmoji, emoji-icon-font). |
| `egui-winit` | winit integration — input translation. |
| `egui-wgpu` | wgpu render backend. |
| `egui_glow` | OpenGL render backend (via `glow`). |
| `eframe` | The end-user "write an egui app" runtime (web + native). |
| `egui_extras` | Optional extras: `TableBuilder`, `DatePicker`, syntax highlighting, image loaders. |
| `egui_demo_lib` | The demo widget gallery. |
| `egui_demo_app` | The packaged demo binary (`egui.rs/#demo`). |
| `egui_kittest` | Kittest-based snapshot + a11y testing harness. |

`egui_plot` lives in a separate repo (`emilk/egui_plot`, latest 0.35.0); spun out so plot churn doesn't gate egui releases. Cross-link: see [api-surface.md](api-surface.md) for what `egui_extras` actually contains.

## egui crate cargo features

Verified from `crates/egui/Cargo.toml`:

| Feature | Effect |
|---|---|
| `default_fonts` (default) | Bundle Hack + NotoEmoji + Ubuntu-Light at compile time. |
| `bytemuck` | Cast `epaint::Vertex` / `emath::Vec2` to `&[u8]`. |
| `callstack` | On-hover debug UI with backtrace to the widget's source line (native only). |
| `cint` | Color-library interop via `cint`. |
| `color-hex` | Enable `hex_color!` macro. |
| `mint` | Math-library interop (`glam`, `nalgebra`) via `mint`. |
| `persistence` | Memory persistence (window positions, expand state) via `serde` + `ron`. |
| `rayon` | Parallel tessellation. |
| `serde` | Serde support; also enables `accesskit/serde`. |
| `unity` | Vertex layout compatible with Unity (used by Unity-side embedders). |

**AccessKit is always on** as of 0.34.0 — the `accesskit` feature was removed in PR #7701 and `accesskit = "0.24.0"` is now a hard dependency. Before 0.34.0 it was opt-in.

## eframe cargo features

Verified from `crates/eframe/Cargo.toml`:

| Feature | Effect |
|---|---|
| `accesskit` (default) | Platform a11y APIs via AccessKit + winit adapter. |
| `default_fonts` (default) | Forwarded to `egui/default_fonts`. |
| `glow` | OpenGL renderer via `egui_glow`. |
| `wgpu` (default) | wgpu renderer (Vulkan/Metal/DX12/WebGPU/WebGL). |
| `wgpu_no_default_features` | wgpu without auto-enabled backends (user picks). |
| `persistence` | App-state persistence to disk via `home` + `ron` + `serde`. |
| `wayland` | Wayland support on Linux. |
| `x11` | X11 support on Linux. |
| `android-game-activity` / `android-native-activity` | Android backend choice. |
| `web_screen_reader` | WebSpeech screen-reader fallback (web only; non-AccessKit). |

The default `eframe = "0.34"` pulls in: AccessKit + default fonts + wgpu + the chosen-by-platform winit backends. Users typically toggle `default-features = false` + `accesskit` + `glow` to shrink WASM bundles (see [open-problems.md § WASM bundle size](open-problems.md)).

## Platform support

| Platform | Status |
|---|---|
| Linux (X11) | First-class; `eframe` feature `x11`. |
| Linux (Wayland) | First-class; `eframe` feature `wayland`. |
| macOS | First-class. |
| Windows | First-class. |
| Web (WASM) | First-class via `eframe` + `wgpu` (WebGPU preferred, WebGL fallback) or `glow`. |
| Android | Supported via `eframe` (game-activity or native-activity); documented as "still rough." |
| iOS | Workable via the `egui-winit` + custom embedding path; no official `eframe` support. |
| Embedded / no-std | Not supported; egui requires `std`. |

The first-class targets share one codebase — egui's "write once, run native+web" claim is real for desktop+web; mobile is workable but second-class. Cross-link: [open-problems.md § mobile maturity](open-problems.md).

## Release cadence

Verified from the crates.io version list:

- 0.10 → 0.20: roughly **every 4–8 weeks** (2021-02 through 2022-12).
- 0.21 → 0.30: ~**3 months apart** on average (2023-02 through 2024-12).
- 0.31 → 0.34: still ~**3 months apart**, with point releases between (0.32.3, 0.33.3 etc).

The "roughly quarterly" framing in the pre-amble holds for 2023-onward. Patch releases land within days when a regression hits (0.34.2 came 39 days after 0.34.1, which itself was 1 day after 0.34.0). Breaking changes are confined to minor bumps; Rerun absorbs the migration cost first and many breakages flow from Rerun's needs.

## Sources

- Workspace `Cargo.toml` @ main — https://raw.githubusercontent.com/emilk/egui/main/Cargo.toml
- `crates/egui/Cargo.toml` @ main — https://raw.githubusercontent.com/emilk/egui/main/crates/egui/Cargo.toml
- `crates/eframe/Cargo.toml` @ main — https://raw.githubusercontent.com/emilk/egui/main/crates/eframe/Cargo.toml
- `crates/egui/CHANGELOG.md` @ main — https://raw.githubusercontent.com/emilk/egui/main/crates/egui/CHANGELOG.md
- crates.io API (`egui`, `eframe`, `egui_extras`, `egui_plot`) — https://crates.io/api/v1/crates/egui
- README @ main — https://raw.githubusercontent.com/emilk/egui/main/README.md
