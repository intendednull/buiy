**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — what it does and does not do today

# Capabilities

A focused inventory of what bevy_picking 0.18.1 can and cannot do, framed from Buiy's required-feature lens. The full lessons synthesis lives in [`lessons.md`](lessons.md); this file is the raw capability ledger.

## Can

- **Rect hit-testing** (bevy_ui, bevy_sprite backends).
- **Ray-cast hit-testing** against 3D meshes (`mesh_picking` feature) — naive ray-triangle, no BVH; bevy_rapier/avian provide accelerated variants.
- **Alpha-aware sprite hit-testing** (since 0.16) — default threshold α ≤ 0.1 passes through; tunable via `SpritePickingSettings`.
- **Ancestor clip respect** in the bevy_ui backend — clipped-out regions don't hit.
- **Multi-pointer** — touch automatically gets one `PointerId` per finger; pointers operate independently. Drag state is per-pointer.
- **Custom pointers** — third-party code can spawn its own `PointerId` and drive the pipeline as if it were a real device (the hook for gamepad-driven virtual pointers and Buiy's accessibility-driven simulated pointer).
- **Multiple backends per frame** — independent backends compose; UI + sprite + mesh + custom-Buiy can all run concurrently.
- **Backend opt-in / opt-out** via Cargo features (`bevy_ui_picking_backend`, `bevy_sprite_picking_backend`, `mesh_picking`).
- **Observer-based event handling** with hierarchical bubbling and `propagate(false)`.
- **Drag-and-drop event lifecycle** between picked entities (`DragStart` → `Drag` → `DragEnd`, plus `DragEnter` / `DragOver` / `DragLeave` / `DragDrop` on targets).
- **Per-pointer global state** — `PointerLocation`, `PointerPress`, `PointerInteraction` components readable by any system.
- **Global enable/disable knobs** via `PickingSettings`.
- **`Pickable::IGNORE`** for transparent overlay nodes that shouldn't intercept input.
- **`should_block_lower: false`** for "pick-through" entities, useful for overlay decorations.
- **Multi-click**: detection via the `Click` event firing on each click; the picking core does **not** ship a built-in double-click event (apps detect via timing).

## Cannot (or only partially)

### Hit-testing shape limitations

- **Non-rect / non-mesh shapes** — there is no built-in primitive for rounded-rect hit testing, clip-path-shape hit testing, or polygonal hit testing. A bevy_ui node with `border-radius: 12px` is still hit-tested as the full bounding rect. Buiy's spec ([`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)) requires hit-shape that respects `border-radius`, `clip-path`, and overflow clipping — Buiy implements this in its own backend (see [`open-problems.md`](open-problems.md)).
- **Subpixel hit-testing for text** — bevy_ui's `pick_ui_text_section` only identifies the section (e.g. coloured span), not the cluster or grapheme. Text-caret placement on click requires extending this. Buiy owns the cosmic-text path and does subpixel-aware hit testing inside `buiy_text` rather than via bevy_picking.

### Input source gaps

- **Gamepad as a pointer source** — no first-class gamepad-driven pointer in `PointerInputPlugin`. Apps either implement a custom pointer that emulates a cursor from analog stick input, or skip the pointer abstraction entirely and route gamepad straight to focus/spatial-nav. Buiy takes the spatial-nav route per [`interaction.md` § 3.7 Gamepad](../../specs/2026-05-07-buiy-foundation/interaction.md).
- **Keyboard-driven activation** — picking has no role here; keyboard activation goes via `KeyboardInput` → focus system → app's own `Activate` event. Buiy's focus system bridges this independently of bevy_picking.

### Accessibility integration gaps

- **No AT-driven picking** — when an assistive technology says "activate the third button," there is no canonical path from AccessKit's `ActionRequest::Default` to a `Pointer<Click>` event on the right entity. Apps wire this themselves. Buiy owns this bridge inside `buiy_core` per [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md).
- **No `:focus-visible` interaction with picking** — picking doesn't update `:focus-visible` heuristic state; that's a separate Buiy concern.

### Multi-window / multi-camera

- **Per-window backend filtering is by convention, not enforcement.** The `order` field on `PointerHits` is a single global f32 — there's no API to say "this backend owns this window, ignore everyone else." Coexistence of bevy_ui's UI backend and Buiy's UI backend in the same app relies on each backend filtering its own picks to its own windows. See [`open-problems.md`](open-problems.md#backend-priority-api).
- **Multi-window pointer sharing** — when the cursor leaves window A and enters window B, the pointer's `Location.target` updates, but cross-window drag handoff is not built in; apps that want it implement it.

### Mesh picking limitations

- **No acceleration structure** — naive ray-triangle test, no BVH; the release notes explicitly defer optimised mesh picking to `bevy_rapier` / `avian`.
- **`SimplifiedMesh` is a manual escape hatch** — performance fix is "point at a low-poly proxy," not "the system uses a BVH."

### Other

- **No built-in double-click event** — apps detect via timing on `Click`.
- **No hover-delay / hover-after timing** — for tooltips, apps implement their own timer.
- **No press-and-hold / long-press primitive** — apps implement via `Press` + timer + cancel on `Move > threshold`.
- **Cursor stack** is not in the crate; cursor selection is per-window via `bevy_winit::CursorIcon`.

## Performance shape

bevy_picking's per-frame cost scales with:

- pointers × backends × visible-entity-counts (rect/mesh tests).
- bevy_ui backend is `O(UiStack length)` — every UI node visited per pointer per frame.
- mesh backend is `O(meshes × triangles)` without acceleration.

For Buiy: bevy_picking's hover-event bubbling adds a per-entity-hierarchy walk per hover transition, which scales with hierarchy depth, not breadth. Buiy's deep widget trees (a typical complex form has 20+ deep ancestor chains) should still be fine — observers don't run unless they exist.

## Sources

- https://docs.rs/bevy_picking/0.18.1/bevy_picking/
- https://docs.rs/bevy_picking/0.18.1/bevy_picking/struct.PickingSettings.html
- https://bevy.org/news/bevy-0-15/ (initial mesh picking limitations note)
- https://bevy.org/news/bevy-0-16/ (sprite alpha-aware picking; SpritePickingSettings)
- Bevy PR #15800 (mesh picking origin, naive-raycast acknowledgement)
- Buiy: `docs/specs/2026-05-07-buiy-foundation/{visuals,interaction,accessibility,cross-cutting}.md`
