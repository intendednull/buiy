**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — Cargo features, dependencies, platform matrix, MSRV, release cadence

# Distribution

`bevy_lunex` is a third-party crate published from `github.com/bytestring-net/bevy-lunex` (org: bytestring-net; primary author: IDEDARY). Latest stable: **0.6.0** (published 2026-01-22). License: **MIT OR Apache-2.0**. Total downloads: **40,504**; recent: **2,126** (fetched 2026-05-22). For context: `bevy_ui` is at **4,901,387** total / **943,255** recent downloads — bevy_lunex's reach is roughly 0.8% of the official kit's by download volume. See [`ecosystem.md`](ecosystem.md) for the adoption math.

The crate is a **parallel UI stack**: it does not depend on `bevy_ui`, `bevy_ui_render`, `bevy_ui_widgets`, or `bevy_a11y`. It depends directly on a curated subset of Bevy sibling crates plus `cosmic-text` and (optionally) `bevy_rich_text3d`. This is the same parallel-stack stance Buiy takes — see [`comparisons.md`](comparisons.md) for the full row.

## Cargo features (verified on `main` for 0.6.0)

```toml
[features]
default = ["text3d"]
text3d  = ["dep:bevy_rich_text3d"]
wasm    = ["dep:getrandom"]
```

Three features, two of them dependency-toggles. There are no opt-in widget subsets, no opt-out a11y (because there is no a11y to opt out of — see [`open-problems.md`](open-problems.md)), no theme-source toggles, and no debug-only feature gates. `text3d` is on by default and pulls `bevy_rich_text3d` to render text in 3D space; turning it off saves a non-trivial dependency but removes worldspace text rendering. `wasm` adds `getrandom` for browser RNG; without it WASM builds will fail to link `rand`.

**Feature-flag churn between releases:** the `wasm` feature first appeared after the 0.3 rewrite; the `text3d` feature is new in 0.5+ (introduced when worldspace text became first-party at 0.3.2 and made a default feature once `bevy_rich_text3d` matured). Older code that pins `bevy_lunex = "0.2"` will not compile under the 0.6 feature surface — there is no semver-stable feature contract, and consumers must re-audit their `[features]` line on every minor bump. This is documented nowhere except the release notes.

## Cargo dependencies (verified on `main` for 0.6.0)

The crate depends on **~22 Bevy sibling crates** at version `0.18.0` (granular, not the `bevy` umbrella):

| Group | Crates |
|---|---|
| Core ECS | `bevy_app`, `bevy_ecs`, `bevy_derive`, `bevy_reflect`, `bevy_platform`, `bevy_log` |
| Math / color | `bevy_math`, `bevy_color` |
| Rendering | `bevy_render`, `bevy_camera`, `bevy_shader`, `bevy_sprite`, `bevy_text`, `bevy_image` |
| Assets / windowing | `bevy_asset`, `bevy_window`, `bevy_winit` (unix: `x11` feature) |
| Input / picking | `bevy_input`, `bevy_picking` |
| Time / hierarchy | `bevy_time`, `bevy_transform` |

External (non-Bevy) dependencies:

- **`cosmic-text = 0.16`** with `shape-run-cache` — text shaping and font rendering.
- **`bevy_rich_text3d ^0.6`** — optional (`text3d` feature), for 3D worldspace text rendering.
- **`getrandom`** — optional (`wasm` feature), browser RNG.
- **`colored`** — terminal color output (logs/debugging).
- **`rand`** — RNG.
- **`radsort`** — radix sort, presumably for fast z-order sorting of layout nodes.

**Notably absent:** `accesskit` (any version), `taffy` (uses its own anchor-based layout instead — see Agent A's [`layout.md`](layout.md)). The absence of AccessKit is the load-bearing fact for Buiy: bevy_lunex has **no accessibility integration**. See [`critiques.md`](critiques.md) § "Accessibility posture" and [`open-problems.md`](open-problems.md) § "AccessKit integration."

## MSRV

The `crate/Cargo.toml` does **not** declare a `rust-version` field. The workspace `Cargo.toml` declares `edition = "2024"`, which requires **Rust 1.85+** (the version that stabilized edition 2024 in February 2025). There is no published MSRV policy; treat 1.85 as the effective floor for 0.6.0 and watch for unannounced bumps on minor releases.

For comparison, Bevy 0.18's workspace MSRV is `rust-version = "1.89.0"` (declared in `bevyengine/bevy` root `Cargo.toml`). bevy_lunex inherits Bevy's transitive MSRV implicitly — any Bevy sibling crate that bumps requirements forces bevy_lunex up with it on the next release.

## Bevy version coupling (the migration tax)

Every bevy_lunex minor is pinned to one Bevy minor. There is no overlap, no LTS, no extended-support branch. The coupling history:

| bevy_lunex | Bevy | Date | Gap from Bevy release |
|---|---|---|---|
| 0.0.x (Aug–Nov 2023) | 0.11 / 0.12 | 2023-08-24 → 2024-01-05 | days–weeks |
| 0.1.0 (alpha → stable) | 0.13 | 2024-05-11 → 2024-06-16 | within month |
| 0.2.0–0.2.4 | 0.14 | 2024-07-04 → 2024-09-21 | days |
| **0.3.0** (rewrite) | 0.15 | 2025-02-28 | ~3 months after Bevy 0.15 |
| 0.3.1 / 0.3.2 | 0.15 | 2025-03-02 → 2025-03-10 | patch |
| 0.4.0 / 0.4.1 / 0.4.2 | 0.16 | 2025-04-25 → 2025-06-14 | within week of Bevy 0.16 |
| **0.5.0** | 0.17 | 2025-10-20 | ~4 weeks after Bevy 0.17 |
| **0.6.0** | 0.18 | 2026-01-22 | ~1 week after Bevy 0.18 |

Three observations:

1. **The 0.3 rewrite (Feb 2025) coincided with the Bevy 0.15 jump and IDEDARY's "complete rewrite"** ([`history.md`](history.md)). The gap from Bevy 0.15's late-Nov-2024 release to bevy_lunex 0.3 (Feb 2025) — three months — is the longest the project has run behind Bevy.
2. **0.4 → 0.5 (Apr → Oct 2025) is the longest in-stable gap** (~6 months), and 0.5 only happened because a contributor (S4ndf1re) did the Bevy 0.17 bump (commit history). This is the bus-factor signal — see [`governance.md`](governance.md).
3. **Minor → minor is always a breaking change** for consumers (renamed components, restructured plugins, new required-components, observer migrations). The 0.2 → 0.3 rewrite was explicit; subsequent minors are smaller but still breaking. There is no semver-stable surface.

## Platform support

bevy_lunex inherits Bevy's platform matrix at the render layer (it renders through `bevy_sprite` and `bevy_text`). There is no first-party documentation of per-platform status; the matrix below is inferred from the dependency graph and the WASM feature flag:

| Platform | Status | Notes |
|---|---|---|
| Linux (X11) | First-class | `bevy_winit` with `x11` feature enabled on unix targets. |
| Linux (Wayland) | Works (Wayland via `bevy_winit` default), but a long-standing bug (#102, opened 2025-03-24) reports `SystemCursor` misbehavior on Linux. |
| Windows | Works | No platform-specific code in the crate. |
| macOS | Works | Same as Windows. |
| WASM | Opt-in via `wasm` feature | Requires `getrandom`. Bevypunk ships a WASM demo on itch.io with documented stutter ("limited performance & stutter due to running on a single thread"). |
| Android | Untested / undocumented | No platform-specific code; inherits Bevy mobile maturity. |
| iOS | Untested / undocumented | Same as Android. |

The crate ships no mobile examples and no mobile-targeted docs. WASM works but is single-threaded; Bevypunk's own demo flags this. AccessKit's platform adapters (UI Automation, NSAccessibility, AT-SPI) are not integrated regardless of platform — see [`critiques.md`](critiques.md).

## Release cadence

Irregular, contributor-driven, tracks Bevy minors. Quantitatively (from the 28-version history):

- 2023-08 → 2024-01: 11 patch releases on Bevy 0.11/0.12 (~5 months, high cadence).
- 2024-05 → 2024-09: 0.1 → 0.2.4 (~4 months, 7 releases on Bevy 0.13/0.14).
- 2024-09 → 2025-02: **5-month gap** through the Bevy 0.15 release while the rewrite happened.
- 2025-02 → 2025-06: 0.3.0 → 0.4.2 (~3.5 months, 6 releases on Bevy 0.15/0.16).
- 2025-06 → 2025-10: **4-month gap** before 0.5 (Bevy 0.17 bump done by external contributor).
- 2025-10 → 2026-01: 0.5 → 0.6 (~3 months, normal Bevy-tracking cadence).
- 2026-01 → present (2026-05): no 0.6.x patch yet; one commit on `main` (Feb 24, 2026: "Add HUD example and Fix tracing ANSCII"). The project has been quiet for ~3 months.

The pattern: bursts of activity around Bevy minors, gaps of 3–5 months between them. This is consistent with the maintainer's own note in the book — *"This crate is being maintained by a university student. Don't expect updates during the semester."* See [`governance.md`](governance.md).

## Sources

- bevy_lunex crates.io metadata — `https://crates.io/api/v1/crates/bevy_lunex` (fetched 2026-05-22).
- bevy_lunex crate Cargo.toml — `https://raw.githubusercontent.com/bytestring-net/bevy-lunex/main/crate/Cargo.toml`.
- bevy_lunex workspace Cargo.toml — `https://raw.githubusercontent.com/bytestring-net/bevy-lunex/main/Cargo.toml`.
- bevy_ui downloads — `https://crates.io/api/v1/crates/bevy_ui` (4,901,387 total, 943,255 recent; fetched 2026-05-22).
- Bevy 0.18 workspace MSRV — `https://github.com/bevyengine/bevy/blob/v0.18.1/Cargo.toml`.
- Issue #102 (Linux cursor) — `https://github.com/bytestring-net/bevy-lunex/issues/102`.
- Bevypunk demo — `https://idedary.itch.io/bevypunk`.
- Bevy Lunex book intro — `https://bytestring-net.github.io/bevy_lunex/`.
- Release tags — `https://github.com/bytestring-net/bevy_lunex/releases`.
