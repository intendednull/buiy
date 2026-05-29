**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — public API surface (entity-side)

# API

This file documents the surface most Buiy code touches: per-entity components, the `Pointer<E>` events, the drag lifecycle, cursor handling. For internal plumbing (system sets, schedules) see [`architecture.md`](architecture.md); for the backend trait pattern see [`backends.md`](backends.md).

## Per-entity components

### `Pickable`

Opt-in / opt-out knob.

```rust
pub struct Pickable {
    pub should_block_lower: bool, // default true
    pub is_hoverable: bool,       // default true
}

impl Pickable {
    pub const IGNORE: Self = Self { should_block_lower: false, is_hoverable: false };
}
```

| Combination | Behaviour |
| --- | --- |
| absent | default (block lower + hoverable) |
| `{ true, true }` | default explicit |
| `{ false, true }` | hoverable, but hits also fall through to entities behind |
| `{ true, false }` | "occluder" — blocks pick from reaching lower entities, but emits no events |
| `Pickable::IGNORE` (`{ false, false }`) | invisible to picking |

`Pickable` was renamed from `PickingBehavior` during the 0.16 cycle. Old docs / posts using `PickingBehavior` are stale.

### `Hovered` / `DirectlyHovered`

Marker components written by the hover stage. `Hovered` indicates "this entity or one of its descendants is under a pointer." `DirectlyHovered` is the stricter "this entity is the topmost hovered." Use as `Query<&Hovered>` / `Query<&DirectlyHovered>`.

### `PointerLocation` / `PointerPress` / `PointerInteraction`

These hang off pointer entities (not picked entities). Useful when you need raw pointer state — e.g. drag-with-arbitrary-tracking, custom cursor rendering. `PointerInteraction` carries the sorted `Vec<(Entity, HitData)>` of who the pointer is on right now.

## Events: `Pointer<E>`

Wrapper:

```rust
pub struct Pointer<E> {
    pub entity: Entity,
    pub pointer_id: PointerId,
    pub pointer_location: Location,
    pub event: E,
}
```

`Pointer<E>: EntityEvent + Event`. Use via observers attached to entities:

```rust
commands.spawn((Node::default(), Pickable::default()))
    .observe(|trigger: Trigger<Pointer<Click>>| {
        info!("clicked entity {}", trigger.target());
    });
```

`Pointer<E>` events bubble up the entity hierarchy via the `PointerTraversal` strategy. Any observer can halt bubbling with `event.propagate(false)` (semantic equivalent of DOM `stopPropagation`).

### Event taxonomy (as of 0.18)

| Event | Fires when… |
| --- | --- |
| `Over` | pointer enters entity's bounds |
| `Move` | pointer moves while over entity |
| `Out` | pointer leaves entity's bounds |
| `Press` | button pressed while over entity |
| `Release` | button released while over entity |
| `Click` | press followed by release on the same entity (no drag in between) |
| `DragStart` | first move after a press, while still pressed |
| `Drag` | continued motion during a drag |
| `DragEnd` | release that ends a drag |
| `DragEnter` | dragged content enters a (potentially-other) entity's bounds |
| `DragOver` | continued motion of dragged content while over a drag target |
| `DragLeave` | dragged content leaves a drag target |
| `DragDrop` | release while over a drag target (target receives this; carries the source entity in event data) |
| `Scroll` | wheel / pinch scroll while over entity |
| `Cancel` | pointer cancelled (e.g. touch lost, OS interruption) |

**Naming churn:** Prior to ~0.17, the press/release events were named `Down` / `Up`. They were renamed to `Press` / `Release` during the 0.17 cycle's broader observer / event terminology cleanup. Old tutorials and `bevy_mod_picking`-era code use `Down`/`Up`.

### Drag lifecycle, end-to-end

```
Press                 → entity is pressed
Move(...) Move(...)   → emits per-move
DragStart             → first move after press is the drag start
Drag Drag Drag ...    → continuous during motion
(if pointer enters a target)
    target sees DragEnter, then DragOver per move, DragLeave on exit
Release               → ends drag
DragEnd               → fires on the *source* entity
(if released over a target)
    target receives DragDrop carrying the source entity
```

Drag state is per-pointer, so multi-pointer drag (e.g. two-finger touch on different entities) works without extra code.

## `PickingSettings` (resource)

Global on/off knobs, four `bool` fields (defaults all `true`):

- `is_enabled` — master switch.
- `is_input_enabled` — gather raw input?
- `is_hover_enabled` — update hover-state components?
- `is_window_picking_enabled` — let the window backend fire?

Useful for "freeze picking during a modal animation" or "suspend during a teardown frame." `bevy_egui` toggles `is_enabled` to claim the pointer.

## Cursor icon API

bevy_picking does **not** ship a cursor-icon stack itself. Cursor selection on hover lives in `bevy_winit`'s `CursorIcon` component (set per window). The pattern in-tree is: an observer on `Pointer<Over>` writes a `CursorIcon` to the window entity; an observer on `Pointer<Out>` restores the previous icon. Several ecosystem crates wrap this into a stack abstraction. Buiy's spec keeps cursor management inside Buiy's input layer (per [`interaction.md` § 3.7](../../specs/2026-05-07-buiy-foundation/interaction.md)) rather than depending on a third-party cursor-stack crate.

## Drag-and-drop integration with OS

bevy_picking's drag events are **purely in-process** between picked entities. OS-level drag (file drop from Finder/Explorer, cross-app drag) is `bevy_winit::WindowEvent::DragAndDrop` — a separate channel. Apps that want both need to bridge the two in their own code. Buiy will own this bridge (per [`interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md), drag-and-drop bullet) because OS drag is a foundation requirement.

## Sources

- https://docs.rs/bevy_picking/0.18.1/bevy_picking/struct.Pickable.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/events/struct.Pointer.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/events/index.html (event taxonomy)
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/struct.PickingSettings.html
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/pointer/index.html
- https://bevy.org/news/bevy-0-17/ (Event/Observer renaming context)
