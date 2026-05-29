**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — glossary of crate-specific terms

# Glossary

| Term | Meaning |
| --- | --- |
| **Backend** | A Bevy system, conventionally inside `PickingSystems::Backend`, that reads `PointerLocation` and emits `PointerHits` events. No trait; the contract is the system signature. bevy_ui, bevy_sprite, mesh, window, Buiy each register one. |
| **DefaultPickingPlugins** | Convenience plugin bundle: `PointerInputPlugin` + `PickingPlugin` + `InteractionPlugin`. Added by Bevy's `DefaultPlugins`. |
| **DirectlyHovered** | Component marker indicating this entity is the topmost hovered entity (not an ancestor of one). |
| **DragDrop** | Event fired on the target entity when a dragged source is released over it. Carries the source entity in event data. |
| **EntityEvent** | Bevy core trait `Pointer<E>` implements so events can be triggered on a specific target entity and bubble via the `Traversal` strategy. |
| **HitData** | `{ depth: f32, position: Option<Vec3>, normal: Option<Vec3>, camera: Entity }`. Per-entity-hit payload. Smaller `depth` = closer to camera. |
| **HoverMap** | Resource: per-pointer mapping of `Entity → HitData` for the topmost N entities currently hovered (after `Pickable::should_block_lower` filtering). |
| **Hovered** | Component marker indicating this entity is hovered (it or one of its descendants is the topmost hit). |
| **InteractionPlugin** | The stage-4 plugin: consumes `PointerHits`, walks the hover diff, emits `Pointer<E>` events. |
| **Location** | `{ target: NormalizedRenderTarget, position: Vec2 }`. A pointer's screen-space position within a render target (window or render-texture). |
| **mesh_picking** | The in-crate ray-cast backend for 3D meshes. Cargo feature, off by default, opt-in per camera via `MeshPickingCamera`. Naive O(triangles). |
| **MeshPickingCamera** | Marker component opting a camera into mesh ray-cast picking. |
| **MeshRayCast** | `SystemParam` for ad-hoc ray casts against meshes. Used by the mesh backend and by user code that wants imperative ray casts. |
| **Pickable** | Per-entity opt-out / behaviour-tweak component. `{ should_block_lower, is_hoverable }`. Default `{true, true}`. `Pickable::IGNORE` ⇒ invisible to picking. Renamed from `PickingBehavior` in 0.16. |
| **PickingBehavior** | **Stale name.** Renamed to `Pickable` in bevy_picking 0.16. |
| **PickingPlugin** | The stage-2-and-shared-infrastructure plugin. Adds the system sets, the `PointerMap`, `HoverMap`, etc. Does not include input gathering or event generation by itself. |
| **PickingSettings** | Resource with four bool toggles: `is_enabled`, `is_input_enabled`, `is_hover_enabled`, `is_window_picking_enabled`. All default `true`. |
| **PickingSystems** | System-set enum: `Input`, `PostInput`, `ProcessInput`, `Backend`, `Hover`, `PostHover`, `Last`. Backends slot into `Backend`; observers fire after `PostHover`. |
| **Pointer<E>** | The high-level event wrapper: `{ entity, pointer_id, pointer_location, event: E }`. `Pointer<E>: EntityEvent + Event`. Bubbles through entity hierarchy via `PointerTraversal`. |
| **PointerHits** | Backend output event: `{ pointer: Entity, picks: Vec<(Entity, HitData)>, order: f32 }`. Multiple backends emit independently; the hover stage merges. |
| **PointerId** | Enum identifying a pointer source. Mouse and touch are auto-spawned. Custom variants supported (gamepad-emulated, AT-driven, test-synthetic). |
| **PointerInput** | Per-tick event/message: the deltas (move, press, release, scroll, cancel) for a single pointer. |
| **PointerInputPlugin** | The stage-1 plugin: gathers raw mouse / touch / pen input from `bevy_input` and emits `PointerInput`. |
| **PointerInteraction** | Component on pointer entities holding the sorted `Vec<(Entity, HitData)>` of currently-hovered entities. |
| **PointerLocation** | Component on pointer entities holding the current `Location` (target + position). |
| **PointerMap** | Resource: `HashMap<PointerId, Entity>` for `PointerId → pointer entity` lookup. |
| **PointerPress** | Component on pointer entities holding current button-press state. |
| **PointerTraversal** | The `Traversal<Pointer<E>>` strategy that determines how `Pointer<E>` events bubble (typically up the entity hierarchy). |
| **Press / Release** | Pointer events. **Renamed** from `Down` / `Up` in 0.17. |
| **RayMap** | Resource: per-camera-per-pointer ray construction for ray-based backends. |
| **ViewportNode** | bevy_ui node that hosts a render-target. Since 0.17, can act as a pick surface for the contained scene's entities (when `bevy_ui_picking_backend` feature enabled). |
| **window backend** | The catch-all backend that reports the window entity as hit when no higher-priority backend matches. Lives in `bevy_picking::window`. Toggle via `PickingSettings::is_window_picking_enabled`. |

## Sources

- https://docs.rs/bevy_picking/0.18.1/bevy_picking/
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/all.html
- Sibling files in this folder.
