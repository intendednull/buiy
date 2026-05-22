**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — lessons for Buiy (validates / avoid / borrow)

# Lessons

**The consult-this-when-designing file.** When a Buiy spec author hits a question about pointer input, hit-testing, or interaction events, read this first; the other files are evidence.

## Validates

### Backend-as-system is the right abstraction

bevy_picking's "a backend is just a system in `PickingSystems::Backend` that reads `PointerLocation` and writes `PointerHits`" pattern is **load-bearing-good**. It's why bevy_ui, bevy_sprite, mesh, and ecosystem crates (bevy_rapier, bevy_egui, bevy_lunex) all compose without a central registry. Buiy's backend slots in the same way (see [`integration.md`](integration.md)).

What this validates for Buiy: don't invent a Buiy-specific picking abstraction. Reuse bevy_picking's. The shape is right.

### Observer-on-entity events with hierarchical bubbling

The `Pointer<E>` wrapper + `EntityEvent + Traversal` model is exactly the pattern Buiy's interaction layer wants:

- Per-entity event delivery without `EventReader` boilerplate.
- Hierarchical bubbling so wrappers can intercept events on behalf of descendants.
- `propagate(false)` for explicit halt — semantic equivalent of DOM `stopPropagation`.
- Generic over event payload, so the same observer machinery handles `Click`, `Drag`, `Scroll`, etc.

Buiy's foundation events (`Activate`, `ValueChange`, focus events) should follow the **same pattern**: wrapper struct + `EntityEvent` + traversal + propagation. Don't invent a parallel event model for Buiy.

### Per-pointer state via entity + component

bevy_picking spawns each pointer as a Bevy entity with `PointerLocation` / `PointerPress` / `PointerInteraction` components. This is the right shape because:

- ECS queries naturally express "for each active pointer, ..." logic.
- Per-pointer state is observable, change-detectable, and serialisable for replay (Buiy's verification harness benefits, per [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)).
- Multi-pointer is a free consequence.

### Custom pointer ID lets synthetic input pass through the normal pipeline

The ability to spawn a custom `PointerId` and feed it `PointerInput` events is **how Buiy bridges AT-driven activation into the picking pipeline** (per [`integration.md`](integration.md)). It's how synthetic input for testing works. It's how a future gamepad-emulated cursor could be added without forking the crate.

Lesson: when designing Buiy's own synthetic-input paths (test replay, AT bridge, spatial-nav-as-pointer experimentation), keep them inside the bevy_picking pointer abstraction rather than building parallel synthetic-event channels.

### `PickingSettings` global on/off knobs are useful

Apps need to disable picking during modal transitions, splash screens, teardown. The four-bool `PickingSettings` resource is right-sized: small, observable, ECS-native. Buiy should have similar global knobs for its own subsystems.

## Avoid

### Default-on `Pickable` is wrong for Buiy widget internals

bevy_picking's default-no-`Pickable`-component-still-blocks behaviour means decorative wrapper nodes inadvertently participate in hover state. For Buiy widgets composed of dozens of internal nodes (a button's text + icon + ripple-effect wrapper, a form field's label + input + error-text + helper-text), most of these should be `Pickable::IGNORE` by default.

**Buiy mitigation:** every widget's internal nodes that aren't the widget's "main interactive surface" get `Pickable::IGNORE` written by the widget's constructor. The "main interactive surface" inherits the default. Author code never touches `Pickable` directly.

### Single global `PointerHits.order` is brittle for multi-stack

The current "convention" of `camera_order + 0.5` for UI works only because UI is the only registered UI-shaped backend per window. When Buiy and bevy_ui coexist on the same Bevy `App`, the only way to prevent races is per-backend window filtering. There is **no API guarantee** that this filtering is happening.

**Buiy mitigation:** the [`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) rule that windows are exclusively Buiy or bevy_ui. Per-window keying enforced by Buiy's plugin construction, not by bevy_picking. Document loudly.

### Pre-1.0 API churn rate

`PickingBehavior` → `Pickable` (0.16), `Down`/`Up` → `Press`/`Release` (0.17). Buiy will see at least one rename per Bevy minor. **Don't expose `bevy_picking` types in Buiy's public API surface** without wrapping them. Every place a Buiy public type names a bevy_picking type is a place Buiy users have to migrate when Bevy renames.

**Buiy mitigation:** re-export bevy_picking types Buiy users need (`Pickable`, `Pointer`, event types) through `buiy::prelude` with stable Buiy names. When upstream renames, only Buiy's re-export file changes.

### Treating `Pointer<E>` as the only interaction event channel

bevy_picking handles pointer + drag + scroll. It does **not** handle keyboard activation, gamepad activation, AT-driven activation, focus change. If Buiy widgets only observe `Pointer<Click>`, they're inaccessible.

**Buiy mitigation:** every widget observes both `Pointer<Click>` and a separate `Activate` event (Buiy-native). Either fires for activation. Spec-level invariant per [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md).

### Naive mesh picking ⇒ 3D pick is a separate concern

`buiy_3d` (per [`architecture.md` § 2.8`](../../specs/2026-05-07-buiy-foundation/architecture.md)) will need 3D-anchored UI elements with click semantics. Don't reuse the in-tree `mesh_picking` backend for production 3D UI — pull `avian` or write our own BVH-backed backend.

## Borrow

### The `Pickable` + `IGNORE` opt-out pattern

Concrete pattern Buiy borrows wholesale. `Pickable::IGNORE` is a clean way to express "this node exists in the hierarchy but is invisible to picking" — Buiy uses it for all decorative wrappers, label fragments, indirect children of an interactive surface.

### The `Pointer<E>` event tree

The exact event taxonomy (`Over` / `Out` / `Move` / `Press` / `Release` / `Click` / `Drag*` / `Scroll` / `Cancel`) is well-shaped for general UI work. Buiy's widget contracts can directly reference `Pointer<E>` events rather than inventing parallel names.

### Hit-testing pipeline staged via `PickingSystems` enum

Buiy's `BuiySet` enum (per [`architecture.md` § 2.8`](../../specs/2026-05-07-buiy-foundation/architecture.md)) borrows the pattern: explicit named system sets, ordered, externally addressable. Lets ecosystem crates inject systems at the right point without monkeypatching.

### Custom-pointer entry point for AT integration

Spawn a synthetic `PointerId` for assistive-tech-driven activation (per [`integration.md`](integration.md)). Reuses the entire `Pointer<E>` pipeline including bubbling and propagation. Cleaner than dispatching synthetic `Pointer<Click>` events directly.

### Observer + `propagate(false)` for event-handling primitives

Buiy's interactive widgets attach observers via the same pattern. Don't reinvent.

### `PointerHits.order = camera_order + 0.5` convention for UI

Buiy's backend uses the same convention so external tools (devtools, egui integration, future ecosystem crates) that assume "UI sits at camera + 0.5" continue to work transparently. Convention compatibility > novel ordering scheme.

### `bevy_egui`-style claim mechanism

`bevy_egui` toggles `PickingSettings::is_enabled` to claim the pointer while egui is active. Buiy uses the same mechanism in reverse: when a Buiy modal opens, Buiy doesn't claim globally, but its backend filters its picks via existing per-window logic. The lesson: **`PickingSettings` is the right place for "everyone stop picking now" gestures**; don't invent a parallel claim mechanism.

## Decision checklist for Buiy spec authors

When designing a new Buiy interaction surface, ask:

1. **Does the entity participate in picking?** If decorative wrapper → `Pickable::IGNORE`. If interactive surface → default `Pickable`.
2. **Does the event have a keyboard / AT equivalent?** If no → spec gap; widget is inaccessible. Add `Activate` observer + APG keyboard contract per [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md).
3. **Is the hit shape rect?** If non-rect (rounded corners, clip-path) → Buiy backend must handle it; bevy_ui's backend won't.
4. **Is there drag?** If yes → WCAG 2.5.7 alternative is **required** (`F` tier per [`interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)).
5. **Is bevy_picking type exposed in Buiy's public API?** If yes → re-export through `buiy::prelude` to shield users from upstream renames.

## Sources

- All sibling files in this folder.
- Buiy: `docs/specs/2026-05-07-buiy-foundation/{architecture,interaction,accessibility,cross-cutting,visuals,verification}.md`
- bevy_picking docs.rs as cited in [`api.md`](api.md), [`architecture.md`](architecture.md).
