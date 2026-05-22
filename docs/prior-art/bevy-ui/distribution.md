**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — distribution, release cadence, features, platform matrix, MSRV

# Distribution

bevy_ui is published as `bevy_ui` on crates.io (homepage `https://bevy.org`, repository `https://github.com/bevyengine/bevy`). It is a crate inside the Bevy workspace, not a standalone project — its release cadence is Bevy's. Lifetime downloads on crates.io: **4,901,387**; 90-day downloads: **943,255** (fetched 2026-05-22). License: **MIT OR Apache-2.0**. The latest stable is **0.18.1** (published 2026-03-04); workspace HEAD is **0.19.0-dev** with **0.19.0-rc.1** on crates.io (2026-05-13).

See [history.md](history.md) for the full per-release timeline; this file covers the *policy* layer.

## Release cadence

Bevy ships a minor release **roughly every 3 months**. The pattern in recent history:

| Release | Date | Gap from previous |
|---|---|---|
| 0.14.0 | 2024-07-04 | — |
| 0.15.0 | 2024-11-29 | ~5 months |
| 0.16.0 | 2025-04-24 | ~5 months |
| 0.17.0 | 2025-09-30 | ~5 months |
| 0.18.0 | 2026-01-13 | ~3.5 months |
| 0.19.0 (rc.1) | 2026-05-13 | ~4 months |

The "~3-month" advertised cadence has in practice been closer to 4-5 months since 0.14, though the gap has tightened back toward 3 months through 0.18. The Bevy README states verbatim: "A new version of Bevy containing breaking changes to the API is released approximately once every 3 months."

Each minor release ships with a **migration guide** at `https://bevy.org/learn/migration-guides/<from>-to-<to>/`. There is **no back-compatibility promise** across minor versions; every minor is a breaking-change event.

Patch releases (e.g. 0.18.1) ship within weeks of the minor, typically for regression fixes. They do not introduce new API.

## Cargo features

bevy_ui's `Cargo.toml` (verified against `0.18.1` tag) exposes a deliberately small feature surface:

```toml
[features]
default = []
serialize = ["serde", "smallvec/serde", "bevy_math/serialize", "bevy_platform/serialize"]
bevy_picking = ["dep:bevy_picking", "dep:uuid"]
ghost_nodes = []  # experimental
```

- **`default`** — empty. bevy_ui is opt-in via the parent `bevy` crate's `bevy_ui` feature, which *is* on by default in the umbrella.
- **`serialize`** — Serde derive on UI types; needed for scene I/O and asset loading.
- **`bevy_picking`** — Wires bevy_ui into bevy_picking for hit-testing. Optional because bevy_picking carries `uuid` as a dep and not all consumers want it.
- **`ghost_nodes`** — Experimental. "Ghost" UI nodes are layout-only entities that don't render — used for grouping without visual artifacts. Marked unstable.

bevy_feathers (the widget kit) is a *separate* crate gated behind the umbrella feature `experimental_bevy_feathers`. bevy_ui_widgets (headless primitives) is also a separate crate, gated under `bevy_ui_widgets` in the umbrella. See [ecosystem.md](ecosystem.md).

## bevy_ui's `bevy_*` dependencies

Per `Cargo.toml` on `main` (0.19.0-dev): bevy_ui pulls in **17 sibling crates** — `bevy_a11y`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_derive`, `bevy_ecs`, `bevy_image`, `bevy_input`, `bevy_input_focus`, `bevy_log`, `bevy_math`, `bevy_picking` (optional), `bevy_platform`, `bevy_reflect`, `bevy_sprite`, `bevy_text`, `bevy_time`, `bevy_transform`, `bevy_utils`, `bevy_window`. This is the rough boundary of "what bevy_ui is glued to" — Buiy is glued to roughly the same set minus `bevy_a11y` (Buiy talks to AccessKit directly, per architecture.md § 2.6).

## Platform support

bevy_ui inherits Bevy's platform matrix. As of 0.18:

| Platform | Status | Notes |
|---|---|---|
| Windows | First-class | UI Automation a11y bridge via AccessKit's `accesskit_windows` adapter. |
| macOS | First-class | NSAccessibility via `accesskit_macos`. |
| Linux (X11) | First-class | AT-SPI via `accesskit_unix`. |
| Linux (Wayland) | First-class | Same `accesskit_unix`; AT-SPI behavior diverges slightly. |
| WASM (browser) | Best-effort | UI renders; AccessKit web adapter is not yet shipped — a11y is degraded. (Buiy spec § 5 open question lists this.) |
| Android | Best-effort | UI renders; TalkBack via `accesskit_android` (in-progress upstream as of 0.17). |
| iOS | Best-effort | UI renders; UIAccessibility bridge is in-progress upstream in AccessKit. |

The general pattern: visual + input + layout work on every supported Bevy target; a11y works only on platforms where AccessKit has shipped an adapter (Windows/macOS/Linux first-class, others lagging). The Buiy foundation spec's "Platform support — staged" policy (architecture.md § 2.9) follows the same shape — Buiy commits desktop only for v1 and defers mobile / WASM until each platform's AccessKit adapter has a headless harness.

## MSRV

Bevy bumps its MSRV every minor or two. Workspace `Cargo.toml` on `main` (0.19.0-dev) lists `rust-version = "1.95.0"`. bevy_ui's own `Cargo.toml` does not redeclare `rust-version`, so it inherits the workspace value. **Buiy tracks Bevy's MSRV** (foundation README § 2.9).

The 0.18.1 release tag did not declare MSRV at the crate level; this is intentional — Bevy treats MSRV as a workspace-level promise, not a per-crate guarantee.

## Coexistence with other Bevy crates

bevy_ui is one of several rendering / interaction crates that all consume the same Bevy substrate:

- **`bevy_render`** — render-graph host. bevy_ui registers its own render-graph node (`UiPass`).
- **`bevy_picking`** — pointer hit-testing. bevy_ui registers a picking *backend* (when `bevy_picking` feature is on); other crates (sprites, mesh-picking) register their own backends. Backend priority is configurable per pointer.
- **`bevy_a11y`** — AccessKit bridge. bevy_ui populates the AccessKit tree through `AccessibilityNode` components. **Buiy replaces this layer** (see [critiques.md](critiques.md) and [architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)).
- **`bevy_text`** — cosmic-text wrapper. bevy_ui uses `bevy_text`, but `bevy_text` is also used standalone by 2D rendering.
- **`bevy_input_focus`** — focus-management primitive (`InputFocus` resource, `Tab` and 2D-spatial navigation strategies). Decoupled from bevy_ui as of 0.15.

Buiy coexists with bevy_ui on a **per-window basis only** (foundation cross-cutting.md § 3.18) — both stacks can run in the same `App` but cannot share a window.

## Sources

- bevy_ui crates.io metadata — `https://crates.io/api/v1/crates/bevy_ui` (fetched 2026-05-22).
- bevy_ui Cargo.toml v0.18.1 — `https://raw.githubusercontent.com/bevyengine/bevy/v0.18.1/crates/bevy_ui/Cargo.toml`.
- bevy_ui Cargo.toml main — `https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/Cargo.toml`.
- Bevy workspace Cargo.toml main — `https://github.com/bevyengine/bevy/blob/main/Cargo.toml`.
- Bevy migration guides index — `https://bevy.org/learn/migration-guides/`.
- Bevy 0.18 release notes — `https://bevy.org/news/bevy-0-18/`.
- AccessKit project — `https://accesskit.dev`.
