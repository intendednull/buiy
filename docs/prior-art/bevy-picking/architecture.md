**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — architecture & pipeline

# Architecture

bevy_picking's organizing idea: a **four-stage pipeline** of system sets, where the only "frontends" most code touches are observers on `Pointer<E>` events. Backends are independent plugins that report hits; the core handles ordering, hover-map computation, and event generation.

```
Pointers ──► Backend ──► Hover ──► Events
(Input + ProcessInput)    (Backend)   (Hover + PostHover)    (event observers fire)
```

## Plugin shapes

- **`DefaultPickingPlugins`** — the convenience bundle: adds `PointerInputPlugin`, `PickingPlugin`, `InteractionPlugin`.
- **`PickingPlugin`** — core infrastructure: shared types, `PointerMap`, `HoverMap`, `PreviousHoverMap`, `PickingSettings` resource, system-set registration.
- **`PointerInputPlugin`** — gathers mouse / touch / pen events from `bevy_input` and emits `PointerInput` messages; updates `PointerLocation` components.
- **`InteractionPlugin`** — consumes `PointerHits` from backends, walks them into the `HoverMap`, and emits the high-level `Pointer<E>` events. Implements bubbling via the `PointerTraversal` strategy.
- **Backend plugins** ship separately (`UiPickingPlugin`, `SpritePickingPlugin`, `MeshPickingPlugin`, window backend, etc.). See [`backends.md`](backends.md).

## The `PickingSystems` enum (system sets)

Per-frame system ordering is defined as an enum so external code orders systems against named sets. As of 0.18:

| Variant | Schedule | Purpose |
| --- | --- | --- |
| `Input` | `First` | Gather raw input → emit `PointerInput` events. |
| `PostInput` | `First` | Runs after input emission, before command flush. |
| `ProcessInput` | `PreUpdate` | Apply `PointerInput` events to `PointerLocation` / `PointerPress` state. |
| `Backend` | `PreUpdate` | Backends run here, read `PointerLocation`, write `PointerHits`. |
| `Hover` | `PreUpdate` | Consume `PointerHits`, update `HoverMap` and hover-state components. |
| `PostHover` | `PreUpdate` | Runs after hover stabilises, before event listeners. |
| `Last` | `PreUpdate` | Final cleanup / late observers. |

Buiy's `BuiySet::Picking` system label (per [`architecture.md` § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md)) sequences against the bevy_picking sets — Buiy's backend system runs in `PickingSystems::Backend`, Buiy's reactive event consumers observe `Pointer<E>` events after `PickingSystems::PostHover`.

## Pointer abstraction

`PointerId` enum identifies pointers. Mouse and touch pointers are spawned automatically. Custom pointers are supported — you just supply a unique ID. This is the mechanism by which **gamepad-driven virtual pointers** or **AT-driven simulated pointers** can integrate. Per-pointer state:

- `PointerLocation` (component) — `Location { target: NormalizedRenderTarget, position: Vec2 }`.
- `PointerPress` (component) — which buttons are currently held.
- `PointerInteraction` (component) — the entities under this pointer, sorted by depth (post-hover).
- `PointerInput` (message/event) — the per-tick deltas (move/press/release/scroll/cancel).
- `PointerMap` (resource) — `HashMap<PointerId, Entity>` lookup.

## Data flow

```
bevy_input::{Mouse,Touch,...}
         │
         ▼ PickingSystems::Input
    PointerInput messages
         │
         ▼ PickingSystems::ProcessInput
    update PointerLocation / PointerPress per pointer
         │
         ▼ PickingSystems::Backend
    backends emit PointerHits { pointer, picks: Vec<(Entity, HitData)>, order: f32 }
         │
         ▼ PickingSystems::Hover
    Hover module sorts hits across backends:
      sort by (order desc, depth asc)
      walk top-down, applying Pickable::should_block_lower
      build HoverMap / PreviousHoverMap, Hovered / DirectlyHovered components
         │
         ▼ PickingSystems::PostHover
    InteractionPlugin diffs hover state, generates events
         │
         ▼ events fire as EntityEvents with Pointer<E> wrapper
    observers run, bubble up via PointerTraversal
```

### `HitData`

A backend reports each hit as `HitData { depth: f32, position: Option<Vec3>, normal: Option<Vec3>, camera: Entity }`. `depth` is **semantic z-order** — smaller depth means closer to the camera/viewer. `position` and `normal` are populated by backends that have world-space data (e.g. mesh-raycast); rect-based backends typically leave them `None`.

### `PointerHits.order`

The `order` f32 on `PointerHits` is the **backend-priority knob**. When two backends report the same pointer, the higher-`order` backend's picks come first. The bevy_ui backend uses `camera_order + 0.5` so UI floats above the camera's 3D content rendered at `camera_order`. Buiy's backend reuses the same convention and sets its own per-window priority (see [`integration.md`](integration.md), and [`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

## Event system

High-level events are `Pointer<E>` where `E ∈ { Over, Move, Out, Press, Release, Click, DragStart, Drag, DragEnd, DragEnter, DragOver, DragLeave, DragDrop, Scroll, Cancel }`. The wrapper struct is:

```rust
pub struct Pointer<E> {
    pub entity: Entity,
    pub pointer_id: PointerId,
    pub pointer_location: Location,
    pub event: E,
}
```

`Pointer<E>: EntityEvent + Event`. Events bubble through the entity hierarchy via the `PointerTraversal` traversal strategy — observers attached to ancestor entities receive the event after the target observer, and any observer can call `event.propagate(false)` to halt bubbling.

This observer-on-entity-event model — wrapper struct, generic event payload, hierarchy bubble — is the pattern Buiy adopts wholesale for its own interaction events. See [`lessons.md` § Borrow](lessons.md).

## `Pickable` component

Optional opt-in / opt-out component. Two fields:

- `should_block_lower: bool` (default `true`) — when this entity is hit, prevent lower-depth entities from being marked hovered too.
- `is_hoverable: bool` (default `true`) — does this entity participate in hover state at all?

Constants:

- `Pickable::IGNORE` = `{ should_block_lower: false, is_hoverable: false }` — fully transparent to the picking system, useful for purely-decorative overlays.

Absent component ⇒ default behaviour. Note this is the **API churn point** between 0.15 and 0.16: the component was once called `PickingBehavior`; bevy_picking renamed it to `Pickable` during the 0.16 cycle. See [`history.md`](history.md).

## Sources

- https://docs.rs/bevy_picking/0.18.1/bevy_picking/ (crate-level docs)
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/enum.PickingSystems.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/backend/index.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/events/struct.Pointer.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/struct.Pickable.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/pointer/index.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/struct.HitData.html
