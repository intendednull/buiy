**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — chronological history and version evolution

# History

The bevy_picking lineage runs across two distinct projects:

- **`bevy_mod_picking`** — external community crate by Aevyrie (`@aevyrie`), 2020–2024.
- **`bevy_picking`** — Bevy first-party workspace crate, 2024-present.

The first-party crate is the upstreamed continuation of the external one, with refinements during upstreaming. The external crate is archived.

## Pre-history (2020–2021)

Bevy 0.1 through ~0.5 had no first-class picking. Each project that needed pointer→entity mapping rolled its own — typically a one-off ray-mesh test for 3D selection. No shared abstraction, no event bubbling, no consistent multi-pointer.

Aevyrie published the first `bevy_mod_picking` release **2020-09-21** (four days after the placeholder `bevy_picking` 0.0.1 squat on 2020-09-17). Initial scope: ray-cast pick of meshes under the mouse, with selection events.

## `bevy_mod_picking` era (2020–2024)

The crate became the canonical community solution for picking. Over 44 versions, it grew to include:

- Backend abstraction (`PickingPlugin` + pluggable backends) — the architectural pattern later upstreamed wholesale.
- `bevy_mod_raycast` as a sibling crate doing the actual ray-mesh math.
- Pointer abstraction supporting mouse, touch, custom pointers.
- Hierarchical event bubbling on `Pointer<E>` events.
- Drag, drop, scroll, focus signals.

Latest non-archived version: **0.20.1** (2024-07-09), supporting Bevy 0.14. Repo archived **2025-03-04**.

Key external dependencies during this era: `bevy_eventlistener` (event bubbling, also Aevyrie), `bevy_mod_raycast` (mesh ray-cast math). The first was upstreamed as observer-with-traversal; the second was upstreamed as `mesh_picking` + `MeshRayCast`.

## The upstreaming (Bevy 0.15, 2024-Q4)

Tracked across multiple PRs:

- **#13677**, **#14686**, **#14695**, **#14757** — the body of the upstream.
- **#15800** (merged 2024-10-13, Jondolf + co-author Aevyrie) — added the mesh picking backend and `MeshRayCast` system parameter, completing the in-tree story (UI, sprite, mesh + custom).

Contributors named in the 0.15 release notes: **@aevyrie**, **@NthTensor**, **@TotalKrill**, **@jnhyatt**, **@Jondolf**.

`bevy_picking 0.15.0` published to crates.io **2024-11-29**, replacing the 4-year-old placeholder squat (`0.0.1` from 2020-09-17). The 0.15 release shipped three in-tree backends: UI (full), sprite (rect-only, no alpha), mesh (naive raycast, disabled by default). Release notes explicitly defer optimised mesh picking to physics ecosystem crates.

## 0.15.x → 0.16 (2024-12 → 2025-04)

- 0.15.1, 0.15.2, 0.15.3 — bug fixes through 2025-02-24.
- 0.16 cycle (rc.1 published 2025-03-19, 0.16.0 released **2025-04-24**):
  - **Sprite alpha-aware picking** — default α-threshold 0.1; configurable via `SpritePickingSettings`.
  - **`PickingBehavior` → `Pickable` rename** — the per-entity behaviour component was renamed for clarity. Old code using `PickingBehavior` does not compile against 0.16+.

## 0.16 → 0.17 (2025-04 → 2025-09)

- 0.16.1 (2025-05-30) — patch.
- 0.17 cycle (rc.1 2025-09, 0.17.0 released **2025-09-30**):
  - **`Pointer<Down>` / `Pointer<Up>` → `Pointer<Press>` / `Pointer<Release>` rename** — part of Bevy 0.17's broader observer / event terminology cleanup (Trigger → On, etc.).
  - **`ViewportNode` picking** — UI nodes can now act as pick surfaces for render-target content (the `bevy_ui_picking_backend` feature interplay).
- 0.17.1, 0.17.2, 0.17.3 — patches through 2025-11.

## 0.17 → 0.18 (2025-09 → 2026-01)

- 0.18 cycle (rc.1 2025-12, 0.18.0 released **2026-01**):
  - Edition bump to **Rust 2024**.
  - Continued observer / event API refinement.
  - **Settings consolidation** — `PickingSettings` ended at four bools (enabled, input, hover, window-picking). Previous prereleases experimented with more knobs.
- 0.18.1 (**2026-03-04**) — current latest stable.

## 0.18 → 0.19 (in progress, 2026-Q2)

- 0.19.0-rc.1 published **2026-05-13** — currently in release-candidate phase. Tracks Bevy 0.19 development.
- This is the version Buiy's foundation spec is being written against (rolling latest-stable Bevy policy from [`architecture.md` § 2.9](../../specs/2026-05-07-buiy-foundation/architecture.md)).

## Naming-churn summary

For future Buiy maintainers grepping old code or tutorials:

| Old name | New name | Renamed in |
| --- | --- | --- |
| `bevy_mod_picking::PickingPlugin` | `bevy_picking::PickingPlugin` | 0.15 |
| `bevy_mod_picking::PickingPluginsSettings` | `bevy_picking::PickingSettings` | 0.15 |
| `PickingBehavior` | `Pickable` | 0.16 |
| `Pointer<Down>` | `Pointer<Press>` | 0.17 |
| `Pointer<Up>` | `Pointer<Release>` | 0.17 |
| `Raycast` (sibling crate) | `MeshRayCast` (system param) | 0.15 |
| `bevy_eventlistener::EntityEvent` | Bevy core `EntityEvent` / observer with `Traversal` | 0.15 |

## Aevyrie's role

`@aevyrie` is the original architect of the picking-as-backends-and-pointer-events design. Aevyrie owned `bevy_mod_picking` for ~4 years before upstreaming. Post-upstreaming, contributions to bevy_picking are primarily from the broader Bevy maintainer pool (Jondolf et al.); Aevyrie remains active in the Bevy ecosystem but is not the sole owner of the in-tree crate. The crate's overall shape — backend-as-system, pointer-as-entity, observer-bubble — is Aevyrie's design.

## Sources

- https://crates.io/crates/bevy_picking (version list with dates)
- https://crates.io/api/v1/crates/bevy_picking/versions
- https://crates.io/crates/bevy_mod_picking
- https://github.com/aevyrie/bevy_mod_picking (archived 2025-03-04)
- https://bevy.org/news/bevy-0-15/ (upstreaming announcement, named contributors)
- https://bevy.org/news/bevy-0-16/ (sprite alpha picking; `PickingBehavior` → `Pickable` context)
- https://bevy.org/news/bevy-0-17/ (Event/Observer rename context; ViewportNode picking)
- Bevy PRs #13677, #14686, #14695, #14757, #15800
