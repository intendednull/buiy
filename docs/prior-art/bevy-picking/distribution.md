**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — distribution, governance, release cadence, platform support

# Distribution & governance

## Repo & crate

- **Repo:** https://github.com/bevyengine/bevy (workspace folder `crates/bevy_picking/`).
- **Crate:** `bevy_picking` on crates.io.
- **Predecessor crate:** `bevy_mod_picking` on crates.io (archived 2025-03-04, no further releases).

## License

`MIT OR Apache-2.0` (Bevy's standard dual-licence). Identical to the rest of the Bevy workspace, identical to Buiy's expected licence. No external-dep licence frictions: external runtime deps are `uuid` (MIT OR Apache-2.0), `tracing` (MIT), `crossbeam-channel` (MIT OR Apache-2.0).

## Cargo features

The crate exposes one feature in its own `Cargo.toml`:

- `mesh_picking` — pulls in `bevy_mesh` + `crossbeam-channel`, enables the in-tree mesh ray-cast backend.

Backend features for the UI and sprite backends live in **the sibling crates** (`bevy_ui` exposes `bevy_ui_picking_backend`; `bevy_sprite` exposes `bevy_sprite_picking_backend`), and the parent `bevy` crate aggregates them under a top-level `picking` umbrella feature.

## Dependency footprint

Internal (Bevy crates):

- `bevy_app`, `bevy_asset`, `bevy_derive`, `bevy_ecs`, `bevy_input`, `bevy_math`, `bevy_camera`, `bevy_reflect`, `bevy_time`, `bevy_transform`, `bevy_window`, `bevy_platform` (std features).
- Optional via `mesh_picking`: `bevy_mesh`.

External runtime:

- `uuid` (v4 generation, for `PointerId`).
- `tracing` (logging).
- `crossbeam-channel` (mesh_picking only).

No `std`-feature-gating outside the `bevy_platform` `std` flag — bevy_picking requires `std`.

## Release cadence

Tied to Bevy's release train. Bevy's cadence is roughly quarterly minor releases (0.15, 0.16, 0.17, 0.18 land at ~3-4 month intervals), with patch releases between. Each Bevy minor publishes a corresponding `bevy_picking` minor on the same day. No independent release cadence — the version-pinning policy in [`architecture.md` § 2.9](../../specs/2026-05-07-buiy-foundation/architecture.md) ("rolling latest-stable Bevy") implies Buiy gets a new bevy_picking minor every Bevy minor with no choice.

Release-candidate window is typically ~5-8 weeks before the stable cut (e.g. 0.19.0-rc.1 published 2026-05-13; 0.19.0 expected ~2026-Q3).

## Platform support

Tracks Bevy's platform matrix:

- **Desktop** (Windows, macOS, Linux) — full support via mouse + touch input.
- **Android, iOS** — touch input works via `bevy_input` / `bevy_winit`'s mobile backends. Bevy's mobile support is best-effort; bevy_picking inherits whatever the underlying input crate provides.
- **Web (wasm32)** — supported; bevy_input maps DOM pointer events to `PointerInput`.

WCAG-relevant gaps inherited from Bevy:

- iOS / Android assistive technologies (VoiceOver, TalkBack) don't have AccessKit adapters yet (in progress upstream as of 2026-Q2). Without an adapter, AT-driven activation can't route through the synthetic-pointer path documented in [`integration.md`](integration.md#a11y-bridge-accesskit--bevy_picking). Buiy's spec stages these platforms as manual-release-gate per [`architecture.md` § 2.9](../../specs/2026-05-07-buiy-foundation/architecture.md).

## Maintainers / governance

bevy_picking is governed under Bevy's maintainership model: a small set of maintainers approve PRs, no single owner. The 0.15 upstreaming credit names @aevyrie, @NthTensor, @TotalKrill, @jnhyatt, @Jondolf — and Jondolf is the named author of the mesh backend PR (#15800). Post-upstream, no single SME is in place as picking lead, which is a mild risk: API decisions are made by whoever picks up the relevant issue/PR, and there's no "picking maintainer" role.

For a load-bearing dependency, this matters because the API churn rate is non-trivial (see [`history.md`](history.md) — `PickingBehavior` → `Pickable` in 0.16, `Down`/`Up` → `Press`/`Release` in 0.17). Buiy will absorb at least one bevy_picking rename per Bevy minor for the foreseeable future, with no obvious escalation path if a rename breaks a Buiy invariant. [`open-problems.md`](open-problems.md) flags this.

## Bevy's `MinimalPlugins` vs `DefaultPlugins`

`DefaultPlugins` includes `DefaultPickingPlugins` by default (since 0.15). Apps wanting a custom picking setup typically still call `add_plugins(DefaultPlugins)` and configure via `PickingSettings`, rather than excluding the bundle. Buiy is plugin-add agnostic — `BuiyPlugin` adds `DefaultPickingPlugins` itself if not present (via `App::is_plugin_added` check).

## Sources

- https://crates.io/crates/bevy_picking
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_picking/Cargo.toml
- https://github.com/bevyengine/bevy (governance / contributor docs)
- https://bevy.org/news/bevy-0-15/ (contributor list, default plugin inclusion)
- Buiy: `docs/specs/2026-05-07-buiy-foundation/architecture.md` §2.9
