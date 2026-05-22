**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — distribution shape: crate features, default-plugin posture, MSRV, platform support

# Distribution

`bevy_a11y` ships as a workspace crate inside [`bevyengine/bevy`](https://github.com/bevyengine/bevy/tree/main/crates/bevy_a11y). It is published to crates.io on the same cadence as the Bevy meta crate. Consumers usually never depend on it directly; they pick it up transitively through `bevy` (via `bevy_internal` → `bevy_app` / `bevy_ui` / `bevy_winit`).

For Buiy's purposes the relevant facts are: how the crate is feature-gated, who actually pays the dependency cost, what AT-SPI activation requires, and which platforms it currently reaches. Each shapes a different Buiy commitment in [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md), and [`cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).

## Crate metadata

- **License:** `MIT OR Apache-2.0` (matches the rest of the Bevy workspace and AccessKit; matches Buiy's expected dual license).
- **Latest stable:** `0.18.1` (released 2026-03-04 per crates.io).
- **Latest pre-release:** `0.19.0-rc.2` (2026-05-22, same day as this folder); `0.19.0-rc.1` landed 2026-05-13.
- **Total downloads:** ~4,236,097 across all versions, with ~925k in the recent window (crates.io statistics as of 2026-05-22).
- **Edition:** Rust 2024.
- **MSRV:** tracks the parent Bevy workspace's MSRV — there is no independent MSRV declaration. Buiy inherits the same constraint via `architecture.md § 2.9`.

## Cargo features

`bevy_a11y`'s own feature set is minimal (verified against `crates/bevy_a11y/Cargo.toml` at the `v0.18.1` tag and on `main` for 0.19.0-rc.1):

| Feature | Default? | Effect |
|---|---|---|
| `std` | yes | Use of the `std` crate. AccessKit-driven a11y needs `std` regardless; the `no_std` story for the crate covers types only, not the active integration. |
| `bevy_reflect` | yes | Enables reflection support so the crate's types register with `bevy_reflect` for inspector / editor introspection. |
| `serialize` | no | Serde + AccessKit serde derives for `Node` / `Action` / `Role`. Buiy's `BuiyMaterial` / theme assets are serde-driven independently; this flag is for app-side a11y-tree serialization (snapshot tests, inspector exports). |
| `critical-section` | no | Enables `critical-section` synchronization primitives for `no_std` environments. Not load-bearing for the desktop integration. |

Notably, **the AT-SPI backend on Unix is gated at the meta-crate level, not in `bevy_a11y` itself**. The `bevy` meta crate's default feature set is `["2d", "3d", "ui"]`; `accesskit_unix` is a separate, opt-in feature that forwards to `bevy_internal/accesskit_unix`:

```toml
# Enable AccessKit on Unix backends (currently only works with experimental
# screen readers and forks.)
accesskit_unix = ["bevy_internal/accesskit_unix"]
```

This is the operative gate for actual screen-reader engagement on Linux. A Bevy app with default features ships `bevy_a11y` but **does not** speak AT-SPI by default — the data structures exist, the adapter is not wired. Buiy's Linux story has to either turn this on for Buiy-owned windows or carry its own equivalent flag; see [`coexistence.md`](coexistence.md) and [`open-problems.md`](open-problems.md) below.

## Default-plugin posture: ships with most Bevy apps

`bevy_a11y` is **not** behind a top-level `bevy/a11y` feature toggle. It is depended on by `bevy_app`, `bevy_ui`, and `bevy_winit`, all of which are part of the default Bevy feature set (`ui` pulls it transitively; `bevy_app` and `bevy_winit` pull it unconditionally). Every Bevy app that uses `DefaultPlugins` therefore takes a `bevy_a11y` dependency. The ~4.2M download count reflects this — it is the download volume of "every Bevy app," not the volume of "every Bevy app that exposed an accessible UI." See [`ecosystem.md`](ecosystem.md) for the disconnect between download volume and actual deployment.

The `AccessibilityPlugin` (the one type that registers the plugin's resources and event channels) is added by `DefaultPlugins` via `bevy_internal`. Opt-out requires the consumer to skip `DefaultPlugins` and add only the sub-plugins it wants — there is no `app.disable_plugin::<AccessibilityPlugin>()` shortcut visible in the public API.

## Platform support

Inherited from AccessKit's platform adapters (see [`prior-art/accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)):

| Platform | Adapter | Status as of 2026-05-22 | Bevy-side coverage |
|---|---|---|---|
| Windows (UIA) | `accesskit_windows` | Production since Dec 2021 | Active via `bevy_winit`. Default plugins wire the adapter to each window. |
| macOS (NSAccessibility) | `accesskit_macos` | Production since Nov 2022 | Active via `bevy_winit`. |
| Linux (AT-SPI, X11 + Wayland) | `accesskit_unix` | Production but feature-gated via `accesskit_unix` flag | Opt-in via the meta crate's `accesskit_unix` feature; not on by default. The release-notes phrasing "experimental screen readers and forks" reflects historical Orca / Speakup quirks, not a current production block. |
| Android (TalkBack) | `accesskit_android` | Pre-1.0 (0.7.x line) | Not wired by `bevy_winit` for the Android backend in a published release; manual-release-gate at best. |
| iOS (UIAccessibility) | `accesskit_ios` | `0.1.0` shipped 2026-05-11 (eleven days before this folder) | Not yet integrated in Bevy as of `0.18.1`; an open issue tracks the wiring. |
| Web (ARIA bridge) | `accesskit_web` | **Does not exist on crates.io.** No WIP PR visible. | Bevy WASM target has no AccessKit story. The `bevy_a11y` types exist but the adapter slot is empty on the web. |

Buiy's `architecture.md § 2.9` commits to desktop (Windows / macOS / Linux) for v1 with full CI coverage and treats Android / iOS / web as manual-release-gate until each adapter exposes a headless harness usable in CI. This matches `bevy_a11y`'s effective coverage today, which is part of why "replace `bevy_a11y` for Buiy windows" is realistic rather than aspirational — there is no platform reach Buiy is giving up.

## Sources

- crates.io `bevy_a11y` page (download counts, version history): https://crates.io/crates/bevy_a11y
- `bevy_a11y` v0.18.1 `Cargo.toml`: https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_a11y/Cargo.toml
- `bevy_a11y` HEAD `Cargo.toml` (0.19.0-dev): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/Cargo.toml
- Bevy meta-crate v0.18.1 `Cargo.toml`: https://github.com/bevyengine/bevy/blob/v0.18.1/Cargo.toml
- Issue [#16312](https://github.com/bevyengine/bevy/issues/16312) "Feature-gate accessibility in `bevy_ui`" (Niashi24, 2024-11-09, S-Ready-For-Implementation — the `no_std`-driven motivation for splitting a11y out of `bevy_ui`).
- AccessKit platform-adapter status: [`prior-art/accesskit/platform-adapters.md`](../accesskit/platform-adapters.md).
- Buiy foundation — architecture § 2.9 (compatibility & policy): [`docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md).
