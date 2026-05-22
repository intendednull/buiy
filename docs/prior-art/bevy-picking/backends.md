**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — backends (in-tree + ecosystem + Buiy's slot)

# Backends

A bevy_picking backend is "just a system that reads `PointerLocation` and writes `PointerHits` in `PickingSystems::Backend`." The crate ships exactly one backend in its own source tree (`mesh_picking`, feature-gated); the rest live as sibling Bevy crates each behind a Cargo feature.

## In-tree backends

| Backend | Crate | Hit shape | Depth source | Backend `order` | Default state |
| --- | --- | --- | --- | --- | --- |
| `mesh_picking` | `bevy_picking::mesh_picking` (this crate, `mesh_picking` feature) | Per-triangle ray-cast | Ray `t` distance | Camera order | Off by default; opt-in via `MeshPickingCamera` |
| `bevy_ui::picking_backend` | `bevy_ui` (`bevy_ui_picking_backend` feature) | Axis-aligned rect intersection w/ ancestor-clip walk | Stack index (`0.00001` increments) | `camera_order + 0.5` | On when feature enabled |
| `bevy_sprite::picking_backend` | `bevy_sprite` (`bevy_sprite_picking_backend` feature) | Sprite rect; alpha-aware since 0.16 (default α≤0.1 passes through) | Sprite z | Camera order | On when feature enabled |
| `bevy_picking::window` | `bevy_picking::window` (this crate) | Whole-window catch-all | Sentinel | Lowest | On with `PickingSettings::is_window_picking_enabled` |

The `window` backend is the "fallback" pick — it reports the window entity as hit whenever no higher-priority backend covers the pointer. It exists so global drop targets / "click on background" handlers work.

### `mesh_picking` (the in-crate backend)

Adds: `MeshPickingPlugin`, `MeshPickingCamera` (opt-in marker), `MeshPickingSettings`, `MeshRayCast` (`SystemParam` for ad-hoc ray casts), `SimplifiedMesh` (component pointing at a lower-poly proxy mesh), `RayCastBackfaces`, `RayCastVisibility`, `RayMeshHit`. Implementation is the upstreamed `bevy_mod_raycast` per PR #15800 (merged 2024-10-13, Jondolf + Aevyrie). Naive ray-triangle intersection — no BVH acceleration. Bevy's release notes explicitly defer optimised mesh picking to physics ecosystem crates (`bevy_rapier`, `avian`).

### `bevy_ui` backend

`UiPickingPlugin` registers `ui_picking` in `PickingSystems::Backend`. Iterates `UiStack` in reverse paint order so closer-to-camera nodes are tested first. Per node, walks ancestor `ComputedNode`s to honour clipping; honours `InheritedVisibility`. Text nodes get an extra `pick_ui_text_section` sub-test. Depth increments by `0.00001` per stack position — purely an ordering convention, not a real z. The backend `order` of `camera_order + 0.5` is the reason UI consistently floats over 3D content rendered by the same camera.

### `bevy_sprite` backend

Rect-based hit test on each sprite's quad in world space. Since Bevy 0.16, alpha-aware: by default, pixels with α ≤ 0.1 in the sprite texture pass through to the entity below. Behaviour configurable via `SpritePickingSettings`.

## Ecosystem backends

The "backend = just a system" pattern means many ecosystem crates ship their own:

- `bevy_rapier` / `avian` — accelerated mesh-pick via physics colliders' broad+narrow-phase (alternative to the naive in-tree `mesh_picking`).
- `bevy_egui` — egui-region pick that suppresses bevy_picking hits while egui has the pointer.
- `bevy_lunex` — its own pick.
- **Buiy** (this project) — its own pick backend; see [`integration.md`](integration.md).

## Backend priority — how the system picks a winner

When multiple backends report hits for the same pointer in the same frame, the hover module sorts by:

1. `PointerHits.order` **descending** (backend priority).
2. `HitData.depth` **ascending** (closer first within a backend).

Then walks the sorted list top-down, applying `Pickable::should_block_lower` to short-circuit.

Crucially, `PointerHits.order` is **a single global f32 per backend per frame**, not a per-window or per-camera arbitration knob. If two backends both want to "own" a window, they must agree on the ordering convention. The bevy_ui backend's `camera_order + 0.5` is convention, not enforced. This is the root cause of why Buiy's foundation spec ([`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) only commits to **per-window** coexistence with bevy_ui — see [`open-problems.md`](open-problems.md#backend-priority-api) for the structural critique.

## Registering an external backend

The contract for a custom backend is minimal:

1. Add a system to `PickingSystems::Backend` in `PreUpdate`.
2. Read `PointerLocation`, query whatever world data you need.
3. For each pointer that hits something, write a `PointerHits { pointer, picks: Vec<(Entity, HitData)>, order: f32 }` event.

That's it. No trait to implement, no central registry. The picking core picks up `PointerHits` events from all sources via `EventReader`/`MessageReader`.

Buiy's backend follows this pattern, with the additional rule (from [`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) that it filters its picks to **windows Buiy owns** — Buiy reads `PointerLocation.target` and skips pointers whose target isn't a Buiy window. Conversely, `bevy_ui`'s `UiPickingPlugin` is filtered to bevy_ui-owned windows (one consequence: in a Buiy-only app, `UiPickingPlugin` reports no hits and is functionally a no-op).

See [`integration.md`](integration.md) for Buiy's full backend slot, [`api.md`](api.md) for the per-entity `Pickable` shape Buiy entities use, and [`lessons.md` § Borrow](lessons.md) for what Buiy borrows wholesale from the backend-as-system pattern.

## Sources

- https://docs.rs/bevy_picking/0.18.1/bevy_picking/backend/index.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/mesh_picking/index.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/window/index.html
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/picking_backend.rs (cited via webfetch; URL is the conceptual reference — exact path may vary by Bevy version)
- https://bevy.org/news/bevy-0-15/ (initial backends shipped: UI, sprite, mesh)
- https://bevy.org/news/bevy-0-16/ (sprite alpha-aware picking)
- Bevy PR #15800 — Jondolf + Aevyrie, "Add mesh picking backend and `MeshRayCast`", merged 2024-10-13
- Buiy: `docs/specs/2026-05-07-buiy-foundation/cross-cutting.md` § 3.18
