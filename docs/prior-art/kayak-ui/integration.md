**Date:** 2026-05-22
**Status:** archived
**Subject:** kayak_ui — distribution: setup at 0.5.0, Cargo features, Bevy compat table, license clarification, MSRV.

# Integration & distribution

What plugging kayak_ui into a Bevy app looked like at its final 0.5.0 release. Frozen snapshot; nothing here has shifted since 2024-02-11.

## `Cargo.toml`

```toml
[dependencies]
bevy = "0.12"
kayak_ui = "0.5"
```

The crate depended on **bevy 0.12** directly and pulled morphorm 0.3 transitively as the layout engine. Total transitive dependency footprint at 0.5.0 was modest by Bevy-plugin standards (~30 crates beyond the Bevy tree itself).

## Bevy compatibility table (from kayak_ui README)

| Bevy | kayak_ui |
|---|---|
| 0.12 | **0.5** |
| 0.10.x | 0.4 or 0.3 |
| 0.9 | 0.2 or 0.1 |
| Bevy `main` | `bevy-track` branch |

The `bevy-track` branch (used by consumers who needed a Bevy newer than 0.12) was never crates.io-released, never tagged, never documented as production-suitable. It also never reached Bevy 0.13 in any usable form — the branch was abandoned alongside the rest of the project. See [`history.md`](history.md) § The release-vs-Bevy timing problem.

## Cargo features

kayak_ui 0.5.0 shipped a small Cargo-feature surface:

- Default features included `bevy_ui` (rendering integration), `bevy_render`, `bevy_winit`.
- No documented feature gates for SVG, scroll widgets, or accordion (these were always-on).
- No `serde` feature for theme / styles serialization (a gap — see [`critiques.md`](critiques.md)).

The crate did NOT have, at 0.5.0:
- A `wasm` feature (no documented WASM support).
- An `accesskit` or `a11y` feature (no AccessKit integration — see [`critiques.md`](critiques.md) § Accessibility).
- A `hot_reload` feature (no theme / asset hot-reload story).

## License

This is a small landmine for consumers. Three pieces:

- **The LICENSE file** in the repo is the standard Rust dual-license: **MIT OR Apache-2.0**. Both files are present at repo root.
- **`Cargo.toml`** declares `license-file = "LICENSE"` and **omits the `license = "..."` field.**
- **crates.io** therefore displays the license as **"Non-standard"** — its UI only recognizes the SPDX-format `license` field, not the `license-file` reference.

The practical effect: anyone running automated license-scanning tooling (e.g. `cargo deny`) against a dependency tree containing `kayak_ui` will see it flagged as "unknown / non-standard" and must investigate manually. The actual legal status is permissive dual-license. The Cargo.toml metadata is the bug; the file contents are the truth. (Buiy's `cargo deny` config — when it lands — should know to map `kayak_ui`'s license-file declaration to `MIT OR Apache-2.0` if it ever appears in the dependency tree, *which it shouldn't*.)

## MSRV

kayak_ui 0.5.0's `Cargo.toml` declared **no `rust-version` field** — it implicitly tracked whatever Bevy 0.12's MSRV was (approximately Rust 1.73 at Bevy 0.12 ship time). The absence of an explicit MSRV is itself a small Bevy-ecosystem-of-2023 artifact; the practice spread later but kayak_ui's lifecycle predated it. For a present-day consumer, kayak_ui 0.5.0 still builds on modern Rust toolchains because it is pure Rust + Bevy without compiler-pinned features, but Bevy 0.12 itself is the binding constraint (which in turn binds the rest of the dependency tree to mid-2023 versions).

## Platform support at 0.5.0

Whatever Bevy 0.12 supported, kayak_ui inherited:

- Windows / macOS / Linux: yes.
- Android / iOS: untested for kayak_ui specifically; Bevy 0.12 supported them experimentally.
- Wayland vs X11 on Linux: no kayak_ui-specific handling; followed bevy_winit.
- **WASM**: no documented support. The MSDF font rendering and custom render pipeline had not been validated against `wasm32-unknown-unknown` targets per any kayak_ui issue or PR.

## Coexistence with `bevy_ui`

kayak_ui ran **in parallel** to `bevy_ui` — own render pass, own layout engine, own focus tree, own widget vocabulary. Consumers could in principle use both in the same app (different camera setups, different entity subtrees), but in practice the community treated them as either-or because the input-routing + camera-attachment story didn't easily compose.

This was a foreshadowing of Buiy's per-window coexistence policy (per [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)): two parallel UI stacks in the same window is intentionally not supported. kayak_ui's experience suggests the same conclusion — the value of being parallel-to-bevy_ui is "you can opt out of bevy_ui's renderer caps," not "you can mix the two seamlessly."

## What plugging it in actually looked like (final form)

For reference; the canonical `0.5.0` "hello world" was approximately:

```rust
use bevy::prelude::*;
use kayak_ui::prelude::*;
use kayak_ui::widgets::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(KayakContextPlugin)
        .add_plugin(KayakWidgets)
        .add_systems(Startup, startup)
        .run();
}
```

A modern (2026-05-22) developer trying to run this against current Bevy will hit a wall at the first `add_plugins(DefaultPlugins)` line — `add_plugin` is deprecated in Bevy 0.13+; the entire Bevy 0.12 API surface has shifted under it. The minimum work to bring kayak_ui to Bevy 0.18 is *substantially* more than a cosmetic API rename; it involves the render-graph migration (0.13), entity-relations changes (0.16+), and required-components conventions (0.15+). No community fork has emerged that does this; the structural-burden lesson [`why-abandoned.md`](why-abandoned.md) ties this back to.

## Sources

- kayak_ui `Cargo.toml` at v0.5.0 — https://github.com/StarArawn/kayak_ui/blob/v0.5.0/Cargo.toml
- kayak_ui LICENSE file — https://github.com/StarArawn/kayak_ui/blob/main/LICENSE
- kayak_ui crates.io listing (Non-standard license display) — https://crates.io/crates/kayak_ui
- kayak_ui README compat table — https://github.com/StarArawn/kayak_ui#readme
- Bevy 0.12 release notes — https://bevy.org/news/bevy-0-12/
- Bevy 0.13 release notes — https://bevy.org/news/bevy-0-13/
- morphorm crate (transitive dep) — https://crates.io/crates/morphorm
