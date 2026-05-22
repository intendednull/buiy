**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — Cargo features, dependencies, platform matrix, MSRV, release cadence, assets

# Distribution

`bevy_feathers` is published on crates.io as part of the Bevy workspace (`bevyengine/bevy`). It is **not** a standalone project — its release cadence is Bevy's, its MSRV is Bevy's, and its dependency surface is the Bevy 0.x sibling-crate set. Latest stable: **0.18.1** (published 2026-03-04). Pre-release: **0.19.0-rc.2** (2026-05-22). License: **MIT OR Apache-2.0**. Total downloads: **191,700** (fetched 2026-05-22).

The "editors and utilities" framing (per the crate description verbatim) is load-bearing for the distribution shape: feathers is intentionally hidden behind an experimental feature flag, ships a deliberately narrow widget set, and is not on a path to general game-UI consumption (see [`critiques.md`](critiques.md) and [`history.md`](history.md)).

## Cargo features (verified on `v0.18.1` tag)

```toml
[features]
default = []
custom_cursor = ["bevy_window/custom_cursor"]
webgl = []
webgpu = []
```

Four features only — no opt-in widget subsets, no opt-out a11y, no theme-source toggles. The entire feathers surface is gated by the umbrella feature `experimental_bevy_feathers` (default-off in the `bevy` umbrella crate); enabling it also pulls in `experimental_bevy_ui_widgets` automatically. Both umbrella features are explicitly labelled "experimental" by Bevy.

## Cargo dependencies (verified on `v0.18.1` tag)

`bevy_feathers` depends on **20 sibling Bevy crates** plus 2 external dependencies. Workspace path-dependencies (versions all pinned to `0.18.0`):

| Group | Crates |
|---|---|
| Core ECS | `bevy_app`, `bevy_ecs`, `bevy_derive`, `bevy_reflect`, `bevy_platform`, `bevy_log` |
| Math / color | `bevy_math`, `bevy_color` |
| Rendering | `bevy_render`, `bevy_camera`, `bevy_shader`, `bevy_text` |
| UI substrate | `bevy_ui` (with `bevy_picking` feature on), `bevy_ui_render`, `bevy_ui_widgets` |
| Input / focus | `bevy_input_focus`, `bevy_picking` |
| Assets / windowing | `bevy_asset`, `bevy_window` |
| Accessibility | `bevy_a11y` |

External dependencies:

- **`accesskit = "0.21"`** (on 0.18.1) — bumped to **`"0.24"`** on `main` (0.19-dev). This is the BSN-relevant a11y bridge. See [`critiques.md`](critiques.md) § "AccessKit pin drift."
- **`smol_str = "0.2"`** — small string interning.

The critical implication for Buiy: feathers transitively depends on `bevy_ui`, `bevy_ui_render`, `bevy_ui_widgets`, and `bevy_a11y`. Buiy's parallel-stack stance ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) means Buiy depends on none of these — it integrates Taffy, cosmic-text, AccessKit, and bevy_picking directly. A Buiy-only app that pulls in `DefaultPlugins` will still have `bevy_ui` compiled (as a transitive dep of unrelated crates), but feathers is not a Buiy dependency in any configuration.

**Known gotcha:** issue [#24369](https://github.com/bevyengine/bevy/issues/24369) — using `bevy_feathers` without enabling the `ui` feature on the parent `bevy` crate produces missing-resource errors during initialization. The feature surface does not fully encode its own prerequisites.

## Asset dependencies (verified in `src/assets/`)

Feathers ships these assets in-tree as part of the crate:

- **Fonts (`src/assets/fonts/`):** `FiraMono-Medium.ttf`, `FiraSans-Bold.ttf`, `FiraSans-BoldItalic.ttf`, `FiraSans-Italic.ttf`, `FiraSans-Regular.ttf`, plus the corresponding `FiraMono-LICENSE` and `FiraSans-License.txt` files.
- **Shaders (`src/assets/shaders/`):** custom shader files used by the alpha-pattern / color-picker widgets.

No icon atlas, no SVG, no variable font (issue [#19854](https://github.com/bevyengine/bevy/issues/19854) tracks "Swap to a variable font in bevy_feathers" — open as of 2026-05-22). Icons in feathers are drawn from the `display/icon.rs` module rather than packaged as an atlas.

Apps consuming feathers do **not** need to supply their own font — feathers loads its bundled Fira family by default. Custom theming can supply alternative fonts via the `font_styles.rs` API, but the default theme's text rendering works out of the box.

## Platform support (inherited from `bevy_ui`)

| Platform | Status | Notes |
|---|---|---|
| Windows | First-class | UI Automation a11y bridge via `accesskit_windows`. |
| macOS | First-class | NSAccessibility via `accesskit_macos`. |
| Linux (X11) | First-class | AT-SPI via `accesskit_unix`. |
| Linux (Wayland) | First-class | Same `accesskit_unix`; AT-SPI behavior diverges. |
| WASM | Best-effort | Visual renders; AccessKit web adapter not shipped — a11y degraded. |
| Android | Best-effort | Visual renders; TalkBack via in-progress `accesskit_android`. |
| iOS | Best-effort | Visual renders; UIAccessibility bridge in-progress in AccessKit. |

The `webgl` and `webgpu` Cargo features are no-op markers (no feature-gated code as of 0.18.1) — they exist to participate in the umbrella's WGSL/WGSL2 selection without adding bevy_feathers-specific surface area. See `bevy_ui` [`prior-art/bevy-ui/distribution.md`](../bevy-ui/distribution.md) for the underlying platform matrix; feathers adds nothing beyond it.

## MSRV

- **0.18.1:** `rust-version = "1.89.0"` (workspace-declared in `bevy`'s root `Cargo.toml`).
- **0.19-dev / `main`:** `rust-version = "1.95.0"`.

The orchestrator pre-amble cited 1.95.0 for Bevy 0.19 — that matches `main`. The 0.18.1 MSRV at 1.89.0 is one minor Rust version older. Feathers re-declares no per-crate MSRV; it inherits.

## Release cadence

Feathers ships on Bevy's ~3-month minor cadence (in practice 3.5–5 months — see [`prior-art/bevy-ui/distribution.md`](../bevy-ui/distribution.md) for the per-release gap table). Every Bevy minor is a breaking-change event for feathers; there is no semver-stable surface, no API-stability promise, and the `experimental_` feature-flag gate makes that policy explicit.

The crate's earliest crates.io publish was a `0.0.0` placeholder on 2025-07-08; the first real release was **0.17.0 on 2025-09-30**, riding the Bevy 0.17 release. Patch releases (0.17.1 / 0.17.2 / 0.17.3 / 0.18.1) shipped within weeks of each minor and contain regression fixes only.

## Sources

- `bevy_feathers` Cargo.toml v0.18.1 — `https://raw.githubusercontent.com/bevyengine/bevy/v0.18.1/crates/bevy_feathers/Cargo.toml`.
- `bevy_feathers` Cargo.toml main — `https://raw.githubusercontent.com/bevyengine/bevy/main/crates/bevy_feathers/Cargo.toml`.
- `bevy_feathers` crates.io metadata — `https://crates.io/api/v1/crates/bevy_feathers` (fetched 2026-05-22).
- `bevy_feathers` source tree v0.18.1 — `https://github.com/bevyengine/bevy/tree/v0.18.1/crates/bevy_feathers/src`.
- Bevy umbrella Cargo.toml v0.18.1 — `https://raw.githubusercontent.com/bevyengine/bevy/v0.18.1/Cargo.toml`.
- Bevy workspace Cargo.toml main — `https://raw.githubusercontent.com/bevyengine/bevy/main/Cargo.toml`.
- Issue #19854 (variable font) — `https://github.com/bevyengine/bevy/issues/19854`.
- Issue #24369 (incomplete deps) — `https://github.com/bevyengine/bevy/issues/24369`.
- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- AccessKit — `https://accesskit.dev`.
