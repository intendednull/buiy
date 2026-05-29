**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — Buiy integration, a11y bridge, coexistence with bevy_ui

# Integration

How Buiy plugs into bevy_picking, where the seams are, and the rules for coexisting with bevy_ui's own backend on the same Bevy `App`.

## Buiy's backend slot

Buiy registers exactly one bevy_picking backend: `buiy_core::BuiyPickingPlugin`. It follows the in-tree contract (see [`backends.md`](backends.md)):

1. A system in `PickingSystems::Backend` (Bevy's `PreUpdate` schedule).
2. Reads `PointerLocation` for every active pointer.
3. Filters by `PointerLocation.target` — only processes pointers whose target is a window Buiy owns (per the per-window keying convention in [`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).
4. For each kept pointer, walks the Buiy hierarchy in paint order, doing **shape-aware** hit testing (rect, rounded-rect, clip-path) per node — going beyond bevy_ui's rect-only test (see [`capabilities.md`](capabilities.md)).
5. Emits a `PointerHits { pointer, picks, order }` event. Buiy uses `order = camera_order + 0.5` to match the bevy_ui convention so existing third-party tools (e.g. devtools, egui) that assume that convention keep working.

Buiy entities carry `Pickable` with the default `{ should_block_lower: true, is_hoverable: true }` unless explicitly opted out (e.g. via Buiy's `PointerEvents::None` analogue, which writes `Pickable::IGNORE`).

## Sequencing

`BuiySet::Picking` (per [`architecture.md` § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md)) is positioned so Buiy's backend runs inside `PickingSystems::Backend`. Buiy's reactive observers fire on `Pointer<E>` events after `PickingSystems::PostHover`. The picking pipeline does the hover diff once per frame regardless of how many backends are present, so adding Buiy's backend doesn't double the work.

## Coexistence with bevy_ui's backend

Per [`cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md), Buiy + bevy_ui coexist **per window**, not per-shared-window. Concretely:

- A given winit `WindowId` is **either** a Buiy window or a bevy_ui window — never shared.
- Buiy's backend filters its picks to Buiy windows. bevy_ui's `UiPickingPlugin` (when present) operates on bevy_ui windows; in a Buiy-only app, `UiPickingPlugin` reports zero hits and is functionally a no-op (but cheap).
- The `PointerHits.order` global priority knob doesn't matter for coexistence because the two backends never produce hits for the same pointer in the same frame — they target disjoint windows.
- Sprite / mesh backends are window-agnostic; they coexist with Buiy on Buiy windows because their hits are filtered out by `Pickable::should_block_lower` of the topmost Buiy element under the cursor.

This rule is **enforced by Buiy's plugin construction**, not by bevy_picking — bevy_picking itself doesn't know about windows except via `PointerLocation.target`. If a downstream app accidentally adds Buiy + bevy_ui nodes to the same window, both backends will emit hits and the global `order` arbitration applies (and the result will be messy). [`open-problems.md`](open-problems.md#backend-priority-api) flags this as a structural gap.

## a11y bridge: AccessKit → bevy_picking

AccessKit's `ActionRequest` (the "AT wants to activate node N" event) is **not** delivered as a `Pointer<E>` event. The chain is:

```
AT (NVDA / VoiceOver / TalkBack)
   │ platform a11y protocol
   ▼
accesskit_winit::Adapter
   │ ActionRequest
   ▼
Buiy's action plumbing  (per architecture.md §2.6)
   │ translate ActionRequest::Default → synthetic Pointer<Click>
   ▼
observer on the target entity
```

The translation is Buiy's responsibility, not bevy_picking's. Buiy does this by either:

- spawning a **synthetic `PointerId`** ("AT pointer") and pushing a `PointerInput::Press` + `PointerInput::Release` at the entity's centre — keeps everything inside the bevy_picking pipeline, observers fire as if it were a real click; **or**
- dispatching `Pointer<Click>` directly via `commands.trigger_targets(Pointer { .. }, [entity])` — bypasses the hit-test stage entirely.

The spec ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)) leans toward the synthetic-pointer route because it preserves event bubbling, drag semantics, and per-pointer state correctly. Both routes are evaluated in the foundation plan.

## a11y bridge: focus → picking

Buiy's focus system is **independent** of bevy_picking — focus moves via keyboard / gamepad / a11y action and writes its own `Focused` component. The interaction is one-way: hover updates `Hovered`; focus updates `Focused`; the two don't talk. The `:focus-visible` heuristic (recent input was keyboard, not pointer) consumes signals from both layers (in Buiy: pointer activity decays `:focus-visible`-eligibility; keyboard navigation re-enables it). Buiy handles this; bevy_picking is unaware.

## Drag accessibility (WCAG 2.5.7)

WCAG 2.5.7 (Dragging Movements, Level AA) requires every drag-driven interaction to also be available via single-pointer non-drag (button, menu, keyboard). Buiy must provide this for every widget that uses bevy_picking's `Pointer<Drag>` events — the foundation spec marks this as **F** (foundation tier) in [`interaction.md` § 3.7](../../specs/2026-05-07-buiy-foundation/interaction.md). bevy_picking has no opinion here; it provides the drag events, Buiy provides the alternatives.

## OS drag-and-drop bridge

OS-level drag (file from Finder) arrives via `bevy_winit::WindowEvent::DragAndDrop`, not bevy_picking. Buiy's drop handling synthesises bevy_picking `DragEnter` / `DragOver` / `DragLeave` / `DragDrop` events on Buiy entities under the OS pointer so app code can write a single drag-target observer that handles both intra-app and OS-sourced drag. The bridge is Buiy's, not bevy_picking's.

## Cargo dependency

Buiy depends on `bevy_picking` directly (it's part of Bevy's default plugins, but Buiy doesn't assume `DefaultPlugins` is added). `mesh_picking` feature is **off** — Buiy doesn't need ray-cast mesh picking for v1; that's a `buiy_3d` future concern.

## Sources

- https://docs.rs/bevy_picking/0.18.1/bevy_picking/
- Buiy: `docs/specs/2026-05-07-buiy-foundation/architecture.md` §2.6, §2.8, §2.9
- Buiy: `docs/specs/2026-05-07-buiy-foundation/cross-cutting.md` §3.18
- Buiy: `docs/specs/2026-05-07-buiy-foundation/interaction.md` §3.7 (drag-and-drop, gamepad, drag accessibility)
- Buiy: `docs/specs/2026-05-07-buiy-foundation/accessibility.md`
- AccessKit docs (action-request semantics) — https://docs.rs/accesskit
