# Input-event model (`bevy_picking` `Pointer<E>`) — child C3 of the widget-catalog campaign

`2026-06-22` · `[draft]` · Wave 2 · realizes foundation `interaction.md §3.7`, `architecture.md §2.6/§2.8`, `cross-cutting.md §3.18` · depends on C0 (umbrella), C1 (coordinate space), C7 (real-input verification tier)

> Scope per umbrella §4 C3 + §2.2. This child OWNS the **pointer-geometry** layer: the `Pointer<E>` pipeline wiring, the `emit_picks` rewrite (absolute hit-test, `painters_z`/stacking pick-depth, no-hit emission, `Pickable`/`should_block_lower`/window-filter), the `Pickable::IGNORE` convention, the **pointer-side press→release→`OnPress` arming** + the focus-on-click signal, the widget-agnostic `MultiClick` pointer gesture, the `buiy::prelude` re-exports, the migration of every consumer off `just_pressed(Hovered)`, the `Pointer<Scroll>` wheel **entry**, and the staged deletion of `Hovered`/`update_hovered`. It does NOT own the **semantic activation channel** — activation lowers through the **existing `OnPress`** and the **agent-interface campaign's action router** (`Action::Click → OnPress`; keyboard Enter/Space and AT activation are that campaign's Phase 1c, not C3's; see "## Coordination with the agent-interface campaign"). It also does NOT own the coordinate-space decision (C1), the widget-state primitives (C4), the focus tree (C5), the `ScrollOffset` pipeline (C5), the styling/focus-ring paint (C6), the a11y substrate/roles/components, or the verification-tier design (C7) — it depends on or feeds those, per the cross-cutting arbitrations §6.

---

## 1. Problem & current state

Current `main` is the **Phase-0 closeout** thin layer: a single `Hovered(Option<Entity>)` resource everything polls, *not* the `Pointer<E>` model the foundation commits. The five concrete defects, verified at file:line:

### 1.1 The pipeline that emits `Pointer<E>` is never added

`BuiyPlugin::build` adds only `bevy::picking::PickingPlugin` (the core message/system-set infra), guarded against the `DefaultPlugins` double-add (`crates/buiy/src/lib.rs:149-151`). It does **not** add `PointerInputPlugin` (gathers raw pointer input into `PointerInput` messages) or `InteractionPlugin` (the hover + `pointer_events` stage that turns `PointerHits` into `Pointer<Over/Move/Out/Press/Release/Click/Drag*/Scroll/Cancel>`). So even though Buiy's backend writes `PointerHits`, **no high-level `Pointer<E>` event ever fires** — there is no `Trigger<Pointer<…>>`, no `.observe()` on any widget, no `Pickable` component anywhere in the codebase (greps clean except an unrelated GPU-readback golden).

### 1.2 `emit_picks` is structurally wrong on four axes (`crates/buiy_core/src/picking/backend.rs:27-76`)

- **Depth = smallest-area rank.** `hits.sort_by(area)` then `HitData::new(_, i as f32, …)` (`backend.rs:53-66`). Pick-order is "smallest box wins", *not* paint-order. Under any overlay/menu/modal — all of which the gallery scope (umbrella §2.1) needs — a small element painted *under* a large panel still wins the pick. Audit #6 ("Picking depth = smallest-area ignores stacking/top-layer — mis-picks under any overlay").
- **Camera = `Entity::PLACEHOLDER`** (`backend.rs:65`). `HitData.camera` is a lie; any consumer that back-projects through the camera (or any tool inspecting hit provenance) gets a dangling entity.
- **`order` hardcoded `0.0`** (`backend.rs:74`), not `camera_order + 0.5`. Breaks the bevy_ui convention third-party tools assume (lessons.md:98-100).
- **No-hit emission is skipped** (`backend.rs:47-49`: `if hits.is_empty() { continue; }`). When the cursor leaves all Buiy nodes, no `PointerHits` is written, so `update_hovered` (`picking/mod.rs:74-76`) keeps its last value — **hover never clears** (the Phase-0 limitation documented at `mod.rs:77-82`).

### 1.3 Hit geometry tests parent-relative coords as absolute (the Bug-1 class)

`point_in_aabb` (`picking/mod.rs:51-57`) AABB-tests `layout.position`..`+size`, where `ResolvedLayout.position` is Taffy's **parent-relative** location written verbatim by `write_resolved_layout` (audit Bug 1, `systems.rs:2976`). The `components.rs:64` doc comment ("window-relative") is a lie (audit §1 MISSED #2). Render reads absolute `GlobalTransform` (`render/mod.rs:435`), so picking and render only agree when every ancestor sits at the window origin. This is C1's coordinate-space class bug; C3 consumes C1's fix (route absolute consumers through non-optional `GlobalTransform`).

### 1.4 `Pickable` / `should_block_lower` are ignored — the composite-widget wart

`emit_picks` and `hit_test` never read `Pickable`; the smallest-area tiebreak is the *only* arbitration. A button's interactive surface and any co-located label/icon fragment all compete by area (audit Gap "Button label", #10). bevy_picking already ships `Pickable::IGNORE` (`{ should_block_lower: false, is_hoverable: false }`) and `should_block_lower` occlusion; Buiy uses neither.

### 1.5 Consumers poll `Hovered` + `just_pressed`, not events

- **Button** (`crates/buiy_widgets/src/button.rs:118-141`): `emit_on_press_on_click` fires `OnPress` on mouse-**down** (`mouse.just_pressed(Left)` + `hovered.0`). Two `TODO(buiy-widget-catalog-design)` blocks at `button.rs:108-117` acknowledge it must be press-arm → release-on-target = activate, release-off-target = cancel, plus Enter/Space keyboard activation. No focus-on-click.
- **Editor** (`crates/buiy_core/src/text/edit/pointer.rs:131-180`): `pointer_selection` reads `Res<Hovered>` + `ButtonInput<MouseButton>` directly, sets `FocusedEntity` on click (`pointer.rs:159-164`), and drives the already-correct, already-`pub`, already-**absolute** classifier (`ClickTracker`/`PointerGesture`/`classify`, `pointer_to_cursor`, `apply_pointer_gesture` — `pointer.rs:21-112`). The classifier is sound; only its *event source* is wrong.

### 1.6 No wheel/scroll input exists at all

`grep MouseWheel` over core+widgets is empty (audit #5). A long todo/list is unscrollable. The editor's `ScrollOffset` is driven only by `auto_scroll_caret`, never by a wheel.

### 1.7 What is already right and must be preserved

- The editor classifier is absolute and `pub` (audit §1 WRONG corrections: caret math, `pointer_to_cursor`, `ClickTracker` privacy are all *false* alarms).
- Focus is a separate `FocusedEntity`/`FocusVisible`/`Focusable` system (`focus.rs`). The one-way focus↔hover independence (integration.md:62) is already the de-facto model; C3 must not entangle them.
- `StackingContext.painters_z` (`components.rs:119-123`) is the committed single ordering source; render already flattens it into a global paint order via `context_tree_paint_order` / `context_roots` (`render/extract.rs:244-287`). **Picking can reuse that exact derivation** — this is the key simplification (§3.2).

---

## 2. Target design

### 2.1 Wire the full `Pointer<E>` pipeline

In `BuiyPlugin::build` (`crates/buiy/src/lib.rs`), after the core `PickingPlugin` guard, add the two plugins that actually emit events, with the same already-added guard discipline (DefaultPlugins includes them; MinimalPlugins does not):

```rust
// crates/buiy/src/lib.rs — after the existing PickingPlugin guard
if !app.is_plugin_added::<bevy::picking::PickingPlugin>() {
    app.add_plugins(bevy::picking::PickingPlugin);
}
if !app.is_plugin_added::<bevy::picking::input::PointerInputPlugin>() {
    app.add_plugins(bevy::picking::input::PointerInputPlugin::default());
}
if !app.is_plugin_added::<bevy::picking::InteractionPlugin>() {
    app.add_plugins(bevy::picking::InteractionPlugin);
}
```

`PointerInputPlugin` (PreUpdate) reads winit cursor/button/wheel into `PointerInput` and updates `PointerLocation`/`PointerPress`; `InteractionPlugin` runs the hover diff over the composited `PointerHits` and emits `Pointer<E>` after `PickingSystems::PostHover`. Buiy's backend (`emit_picks`) stays in `PickingSystems::Backend`, feeding both. (Verified exact paths against `bevy_picking-0.19.0-rc.3`: `bevy::picking::input::PointerInputPlugin` (`src/input.rs:93`); `bevy::picking::InteractionPlugin` (`src/lib.rs:417`), top-level next to `PickingPlugin`.)

### 2.2 Rewrite `emit_picks` — the heart of C3

New signature and behavior (replaces `backend.rs:27-76`):

```rust
fn emit_picks(
    pointers: Query<(&PointerId, &PointerLocation)>,
    // Cameras whose render target resolves to a window, so we can match a
    // pointer's target window → its camera (§3.1).
    cameras: Query<(Entity, &Camera)>,
    // C1: GlobalTransform is NON-OPTIONAL here. Render hard-requires it
    // (render/mod.rs:421); picking matches render's coordinate source.
    nodes: Query<(Entity, &ResolvedLayout, &GlobalTransform, Option<&Pickable>)>,
    contexts: Query<(Entity, &StackingContext)>,
    mut output: MessageWriter<PointerHits>,
) {
    // Build the global paint order ONCE per frame (shared with render, §3.2):
    //   paint_order[0] = bottom-most ... paint_order[n-1] = top-most.
    let paint_order = global_paint_order(&contexts);
    let z_of: HashMap<Entity, usize> =
        paint_order.iter().enumerate().map(|(i, e)| (*e, i)).collect();

    for (pointer, location) in pointers.iter() {
        let Some(loc) = location.location() else { continue };
        // §3.1 window filter + camera resolution.
        let NormalizedRenderTarget::Window(win) = loc.target else { continue };
        let Some(camera) = camera_for_window(&cameras, win.entity()) else { continue };
        let cursor = loc.position; // logical px, window space

        // Absolute hit-test (C1): node box in window space is
        // gt.translation().xy() + (0,0)..size. point_in_node honors a
        // rounded-rect/clip-path shape in a follow-up; v1 is the axis box.
        let mut hits: Vec<(Entity, usize)> = Vec::new();
        for (entity, layout, gt, pickable) in nodes.iter() {
            if pickable.map(|p| !p.is_hoverable).unwrap_or(false)
                && pickable.map(|p| !p.should_block_lower).unwrap_or(false)
            {
                continue; // Pickable::IGNORE — invisible to picking
            }
            if point_in_node(cursor, layout, gt) {
                let z = *z_of.get(&entity).unwrap_or(&0);
                hits.push((entity, z));
            }
        }
        // Sort top-most-first by paint order (higher paint index = nearer).
        hits.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        // Honor should_block_lower: stop at the first occluder (default).
        let picks = build_picks(&hits, &nodes, camera, &z_of, paint_order.len());

        // ALWAYS emit — empty picks clear hover (§2.2 no-hit).
        let camera_order = cameras.get(camera).map(|(_, c)| c.order).unwrap_or(0);
        output.write(PointerHits::new(*pointer, picks, camera_order as f32 + 0.5));
    }
}
```

Five behaviors this pins:

1. **Absolute hit-test via non-optional `GlobalTransform`** (consumes C1). The node's window-space box is `gt.translation().truncate() .. + layout.size`. No `unwrap_or(layout.position)` fallback (audit §1 "Do NOT carry the fallback"; render hard-requires the transform, so picking matches it and the harness supplies it).
2. **Depth from `painters_z` paint-order** (§3.2): the per-entity pick depth is `(n - 1 - paint_index)` so that bevy_picking's ascending-depth sort (`hover.rs:179`, `sort_by_key(FloatOrd(depth))`) puts the **last-painted (topmost)** node at depth 0. Pick-order == paint-order. This is the §6.1 shared primitive C3 OWNS; C5 consumes it.
3. **No-hit emission**: `PointerHits` is written every frame a pointer targets a Buiy window, even with an empty `picks` Vec — so `InteractionPlugin`'s hover diff emits `Pointer<Out>` and clears `Hovered`/`DirectlyHovered`. (Fixes `mod.rs:77-82`.)
4. **`Pickable` honoring**: `Pickable::IGNORE` nodes are skipped entirely; `should_block_lower: true` (the default) makes the topmost hit an occluder — `build_picks` truncates the pick list at the first blocking entity so lower nodes don't receive events (matches bevy_picking's own block-lower semantics). `should_block_lower: false` lets the hit fall through. (bevy_picking's hover stage *also* applies `should_block_lower`; emitting the full sorted list and letting hover apply it is acceptable, but truncating in the backend keeps the picks list honest and is cheaper — see §3.4.)
5. **Real camera + `order = camera_order + 0.5`** (§3.1).

#### `build_picks` — the occluder/truncation rule (precise)

`build_picks` is the pure function that turns the per-pointer hit set into the final `Vec<(Entity, HitData)>` that goes into `PointerHits`. It is the **occluder rule** C5's light-dismiss / overlay-interception consumes via §6.1, so it is pinned exactly here:

```rust
/// Assemble the final picks for one pointer from the geometric hits.
/// `hits` is the set of entities whose absolute box contains the cursor,
/// each paired with its global paint index (`z_of`); `paint_len` is the
/// number of painted entities (== paint_order.len()), used to map a paint
/// index to a pick depth.
fn build_picks(
    hits: &[(Entity, usize)],            // (entity, paint_index)
    nodes: &Query<(Entity, &ResolvedLayout, &GlobalTransform, Option<&Pickable>)>,
    camera: Entity,
    z_of: &HashMap<Entity, usize>,
    paint_len: usize,
) -> Vec<(Entity, HitData)>;
```

The rule, in order:

1. **Skip `Pickable::IGNORE`.** Any entity whose `Pickable` has `is_hoverable == false` **and** `should_block_lower == false` (i.e. `Pickable::IGNORE`) is dropped — it is invisible to picking and neither receives events nor occludes. (The `emit_picks` loop already filters these before they reach `hits`; `build_picks` re-applies the predicate defensively so the rule is self-contained.)
2. **Sort top-most first by global paint order.** Order the remaining hits by **descending `painters_z` paint index** (`z_of[entity]`): the last-painted (visually topmost) entity is first. The tiebreak is inherited from `global_paint_order` (ECS-tree order within an equal-z tier, §2.3) — there are no equal paint indices, so the sort is total.
3. **Truncate at the first occluder (inclusive).** Walk the sorted list top-down and stop at the **first** entity whose `Pickable.should_block_lower == true` — the bevy_picking default (a node with no `Pickable`, or `Pickable::default()`, blocks). That occluder is **included** in the output; every lower (later-in-the-walk) entity is dropped. An entity with `should_block_lower == false` (an explicit pass-through surface) does **not** terminate the walk — it is emitted *and* lower entities keep receiving events, falling through to the next entity until an occluder (or the end of the list) is reached.
4. **Assign `HitData` from paint order + the real camera.** Each surviving entity `e` gets `HitData { camera, depth: (paint_len - 1 - z_of[e]) as f32, position: None, normal: None }`. The depth is the **reverse** of the paint index (§2.2 behavior 2 / §2.3) so bevy_picking's ascending-depth hover sort (`hover.rs:179`, `sort_by_key(FloatOrd(depth))`) puts the topmost-painted entity at the smallest depth. `position`/`normal` are `None` — a 2D UI hit carries no world-space surface point (those slots are the 3D-mesh-backend contract; a follow-up may fill `position` with the cursor's window-space point if a consumer needs it).

This makes the §1.4 composite-widget wart impossible: a default-pickable button surface is an occluder, so an `IGNORE` label painted over it (step 1 drops the label) or a non-blocking decorative child below it (step 3 truncates at the button) both resolve to the button. It is also exactly the primitive C5 reads to decide whether a pointer landed *inside* an overlay (the overlay surface occludes) or *outside* it (no Buiy hit, or a hit on a sibling that the overlay does not occlude) for light-dismiss.

### 2.3 The shared `painters_z` → pick-depth primitive (§6.1, C3 OWNS)

C3 adds a small pure function in `picking/depth.rs` that reuses the render-side flattening so pick-order can never diverge from paint-order:

```rust
/// The global front-to-back paint order (index 0 bottom-most), derived from
/// every forming StackingContext exactly the way render derives it
/// (render::extract::{context_roots, context_tree_paint_order}). Shared
/// derivation: index within nearest ancestor StackingContext, composed across
/// nested contexts (a nested SC root appears as one atomic entry in its
/// parent's list and its descendants live only in its own painters_z),
/// ECS-tree-order tiebreak across degenerate multi-roots (context_roots sorts
/// by entity). Top-layer members are already at the tail of the root context's
/// painters_z (layout sub-pass 6f), so they correctly sort topmost for free.
pub fn global_paint_order(contexts: &Query<(Entity, &StackingContext)>) -> Vec<Entity>;
```

Implementation reuses `render::extract::context_tree_paint_order` + `context_roots` (today `pub` in `render/extract.rs:244,274`) over a `HashMap<Entity, &[Entity]>` built from the `StackingContext` query. **The derivation is a single source of truth shared between render's `extract_buiy_nodes` and picking** — exactly the simplification-cascade insight (§6.1: "pick-order == paint-order"). The depth a node receives is its reverse index in this list.

> **This is the stacking-aware `hit_test` the agent-interface campaign DEFERRED and DEPENDS on.** The agent-interface campaign explicitly left "stacking-aware `hit_test`" as its named follow-up #3 (its `phasing.md` #3; its `inprocess-api.md` §5.1 caveat) because the current `picking::hit_test` (`picking/mod.rs:37`) is smallest-AABB and **z-order/stacking/top-layer UNAWARE** — it cannot answer "obscured by a modal/top-layer/tooltip," so that campaign ships its `HitTargetable` actionability gate (the in-process driver's `act_when_actionable`) **AABB-only with the limitation documented**. C3's paint-order pick-depth here (and the matching `hit_test` rewrite in §3.8) *is* the stacking-aware hit-test that unblocks it: once C3 lands, the agent-interface campaign's `HitTargetable` can read this paint-ordered hit-test and mean "not obscured." C3 must keep `hit_test`/`global_paint_order` `pub` so that campaign's `inprocess.rs` can consume it (§3.8). Entities **not** in any `painters_z` (the OQ#4 concern — a node whose ancestor forms no context) cannot occur: every `Node` ultimately sits inside the root context (the root `StackingContext` lists every in-flow descendant not owned by a nested context), so the flatten visits every painted entity. A node absent from the flatten is, by construction, not painted — and a not-painted node must not be pickable either; it receives no hit (the `unwrap_or(&0)` is a defensive floor, not a real case).

### 2.4 The `Pickable::IGNORE` convention for widget internals

Widget constructors (C4 builds the widgets; C3 fixes the convention) write `Pickable::IGNORE` on every **decorative/label/wrapper** node and leave the **single interactive surface** at the bevy_picking default (`Pickable::default()` = block-lower + hoverable). Author code never touches `Pickable` (lessons.md:50-52). Concretely for the Button (C4 consumes this rule): the `Button` marker's interactive box is default-pickable; a child label `Text` / icon node spawned by the scene-fn carries `Pickable::IGNORE`. This is the structural fix for the smallest-area co-location wart (§1.4) — a click on the label bubbles to / is occluded by the button surface, not stolen by the smaller text run.

### 2.5 The pointer activation path — `Pointer<Press/Release>` arming → `OnPress` (C3 OWNS the pointer side only)

C3 provides the **pointer** half of activation and nothing more. The semantic activation channel — the one shared sink that *every* modality converges on — is the **existing `OnPress` message**, and the non-pointer producers (keyboard Enter/Space, AssistiveTech) are the **agent-interface campaign's** (see the Coordination section). C3 does **not** define a parallel semantic activation event; an earlier draft of this child invented a Buiy-native `Activate` `EntityEvent` + a dual (`Pointer<Click>` + `Activate`) observe contract — that is **removed** here, because it competes with the agent-interface action router, which already lowers `Action::Click → MessageWriter<OnPress>` ("the same message the pointer path emits" — agent-interface `action-router.md` §4) and wires Button keyboard activation (Enter + Space → `OnPress`) in its Phase 1c. Inventing a second activation type would split the one sink the campaigns already agreed on.

**What C3 owns — the pointer-side press-arm → release-on-target → `OnPress`:**

The `Button` marker (or its scene-fn, C4) attaches two pointer observers, and the convergence is `OnPress`, not a new event:

- `Pointer<Press>` on the interactive surface → set a thin **armed** marker (pointer plumbing; not a semantic event).
- `Pointer<Release>` on the same target while still armed-over-it → emit `MessageWriter<OnPress>(entity)` (release-on-target = activate). Release **off** the target → clear armed (drag-cancel), no `OnPress`.

This press→release→`OnPress` arming is **pointer plumbing**, deliberately *not* a semantic activation event. It produces exactly the `OnPress` message the agent-interface router's `Action::Click` honor and the keyboard Enter/Space handlers also emit, so all three modalities (pointer here, keyboard + AT there) converge on the **single `OnPress` sink** — the convergence widget-contracts.md §4 pins ("a Button's Space handler and its `Click` honor both emit `MessageWriter<OnPress>`"). A widget that needs no pointer-specific press visuals observes nothing extra on the pointer side; the router/keyboard path activates it through `OnPress` directly.

**focus-on-click** is the other pointer-side signal C3 owns (§2.7): the `Pointer<Press>` observer sets `FocusedEntity`. The keyboard/AT path sets `FocusedEntity` through the router's `Focus` action — that is the agent-interface campaign's, not C3's.

### 2.6 `Pointer<Scroll>` wheel entry (§6.3, C3 OWNS the entry)

`InteractionPlugin` already emits `Pointer<Scroll> { unit: MouseScrollUnit, x, y, hit, phase }` over the hovered entity once `PointerInputPlugin` is wired (§2.1) — bevy_picking reads winit `MouseWheel` for us. So the wheel **entry** is free the moment §2.1 lands; C3 does **not** add a raw `MouseWheel` reader. C3's deliverable is the **contract**: `Pointer<Scroll>` is the canonical wheel event, it carries `unit` (`Line`/`Pixel` — the `deltaMode` distinction interaction.md:103 requires, resolving OQ "deltaMode", §3.6), and C5's `ScrollArea` observes it on the nearest scroll container to drive `ScrollOffset` (clamp/overscroll is C5's). C3 ships one trivial smoke observer behind the gallery (or a C7 fixture) proving a `Pointer<Scroll>` reaches an entity, so the entry is verified independent of C5.

### 2.7 Focus-on-click + the `:focus-visible` decay signal (§6.4)

C3 provides the *signals*; C5 owns the focus *tree*, C6 owns the ring *paint*, C4 *consumes* `FocusedEntity`:

- **focus-on-click**: a C3 system observes `Pointer<Press>` and, for an entity that is `Focusable` (or editable), sets `FocusedEntity` to it. This *moves* the focus-on-click coupling out of the editor's `pointer_selection` (`pointer.rs:159-164`) into one place, so it is uniform across widgets, not editor-special (audit §3: "No focus-on-click for Checkbox/Button"). C5 may later refine *which* entity (nearest focusable ancestor) when it builds the scope model; C3 ships the leaf version.
- **`:focus-visible` decay signal**: `focus.rs:16-19` documents that `FocusVisible` is set true on Tab and never reset. C3 adds the pointer half: on `Pointer<Press>` focus-on-click, set `FocusVisible(false)` (pointer focus is not keyboard-visible); the keyboard path in `focus.rs:72` already sets `true`. This is the §6.4 signal C6's ring-lowering consumes. C3 does **not** decide the ring shape or the `FocusVisible` component-vs-resource representation — §6.6 pins that C6 confirms the shape with C3/C5 first; C3 keeps the existing `FocusVisible` resource and only writes the pointer-side `false`.

### 2.8 Consumer migrations

- **Button** (`button.rs`): delete `emit_on_press_on_click` + its `Hovered`/`just_pressed` poll. The `Button` marker (or its scene-fn, C4) attaches two observers: `Pointer<Press>` → set an armed marker; `Pointer<Release>` → if still over the target, emit `MessageWriter<OnPress>(entity)` (release-on-target = activate), else clear armed (drag-cancel). The pointer path's sink is `OnPress` — the **same** message the agent-interface router's `Action::Click` honor and that campaign's Phase-1c Enter/Space keyboard handlers emit (`action-router.md` §4, `widget-contracts.md` §4 "Button=Enter+Space"). Keyboard and AT activation are **not** C3's — they are the agent-interface campaign's. The actual observer bodies are C4's widget contract; C3 deletes the poll and lands the `Pointer<Press/Release>` → `OnPress` pointer plumbing the observers attach to.
- **Editor** (`pointer.rs`): keep `ClickTracker`/`classify`/`apply_pointer_gesture` **byte-identical** (umbrella: "classifier unchanged source-only"). Rewrite only `pointer_selection`: source presses/drags from `Pointer<Press>` / `Pointer<Drag>` observers (or a system reading `PointerPress` + the hit target) instead of `Res<Hovered>` + `just_pressed`. The editor keeps using its **own `ClickTracker`** run directly for *intra-text* double-/triple-click selection (the multi-click window+radius) — bevy_picking 0.19 has `Click.count`/`Press.count` but **no** `DoubleClick` event and no tunable 450ms/4px window (verified against `bevy_picking-0.19.0-rc.3/src/events.rs`), so the classifier stays (matches capabilities.md + audit #17). The *widget-agnostic* double-click that a non-editor widget (a todo row) needs for edit-in-place is the separate `MultiClick` `EntityEvent` (§2.11), derived from the **same** now-public `ClickTracker` heuristic so the two never disagree — C3 owns that event; the editor does not round-trip through it. Remove the editor's own focus-on-click (now in §2.7's shared system). The one-frame `GlobalTransform` lag the editor already documents (`pointer.rs:120-129`) is preserved and inherited by `emit_picks` (§3.3).

### 2.9 Prelude re-exports (lessons.md:62-64)

Re-export every bevy_picking type Buiy users touch through `buiy::prelude` (and `buiy_core` root) with stable Buiy-owned aliases so a pre-1.0 upstream rename touches one file:

```rust
// crates/buiy/src/lib.rs prelude + crates/buiy_core/src/lib.rs root
pub use bevy::picking::Pickable;
pub use bevy::picking::pointer::PointerButton; // carried by MultiClick (§2.11)
pub use bevy::picking::events::{
    Pointer, Over, Out, Move, Press, Release, Click, DragStart, Drag, DragEnd,
    DragEnter, DragOver, DragLeave, DragDrop, Scroll, Cancel,
};
pub use buiy_core::picking::MultiClick; // the pointer-gesture event (§2.11)
```

The bevy hover components `Hovered`/`DirectlyHovered` are **not** re-exported under those names while Buiy's own `Hovered` resource still exists (§3.7 collision); once Buiy's resource is deleted (§2.10), the bevy components become the canonical hover surface and may be re-exported. C3 documents this transition.

### 2.10 Delete `Hovered`/`update_hovered` LAST, behind a shim

`Hovered`/`update_hovered` (`picking/mod.rs:20-82`) are deleted only after every consumer migrates (§2.8). During the migration, keep `Hovered` populated by a thin shim that reads bevy_picking's own `DirectlyHovered` (the topmost-hovered marker, api.md:36-38) so the ~3 reader files and the tests stay green incrementally (§3.7 resolves shim-vs-hard-cut → shim). The shim and the resource are removed in the final migration step (§5 step 8); `hit_test` (`mod.rs:37-49`) is rewritten to the absolute+paint-order semantics or deleted in favor of bevy_picking's `PointerInteraction.sorted_entities` (decided in §3.8).

### 2.11 The Buiy-native multi-click event (§6.9, C3 OWNS the gesture; resolves audit W17 / coverage gap)

bevy_picking 0.19 ships **no `Pointer<DoubleClick>` event** (verified against `bevy_picking-0.19.0-rc.3/src/events.rs`: there is `Pointer<Click>` carrying a `count`, and `Pointer<Press>` with a `count`, but no double-click event and no tunable multi-click window/radius). So nothing today surfaces a *widget-agnostic* double-click that a **non-editor** widget can observe — the editor's `ClickTracker` (`text::edit`) classifies multi-clicks but only for its own selection logic, internal to `pointer_selection`. A todo row needs edit-in-place on double-click (audit W17), and a todo row is **not** an editor: it must be able to observe a double-click without re-implementing the timing/radius heuristic.

C3 owns the **pointer-gesture** model, so C3 defines one widget-agnostic multi-click `EntityEvent`, emitted on **any** picked entity (not editor-internal). This is a **pointer-layer gesture**, deliberately distinct from the agent-interface campaign's semantic action router: `MultiClick` is derived from raw pointer-click timing/radius and rides the `Pointer<E>` bubbling path, whereas the router dispatches exact-`NodeId` AT/agent verbs (`Action::Click`, etc.). They never overlap — a double-click is a pointer affordance, not an accessibility action — so there is no competition with the router here.

```rust
// crates/buiy_core/src/picking/gesture.rs (the gesture model lives next to the
// picking backend, the event entry point; re-exported through buiy::prelude).

/// A committed multi-click on an entity — the widget-agnostic double/triple-click
/// signal. Bubbles up the entity hierarchy like Pointer<E>. Emitted for ANY
/// picked entity (a todo row, a list item, a button), not only the editor, so a
/// non-editor widget can drive edit-in-place / expand / select-all gestures.
/// Derived from the SAME timing+radius heuristic the editor uses (the already-
/// public `text::edit::ClickTracker`), so single-source multi-click semantics:
/// the editor's intra-text selection and a widget's edit-in-place agree on what
/// "a double-click" is.
#[derive(EntityEvent, Clone, Debug)]
pub struct MultiClick {
    /// The entity the gesture targets (the EntityEvent target).
    pub entity: Entity,
    /// Click run length: 2 = double, 3 = triple, … (a single click is plain
    /// `Pointer<Click>`; `MultiClick` is only emitted for count >= 2).
    pub count: u32,
    /// The pointer button that produced the run.
    pub button: PointerButton,
}
```

**Derivation + emission (C3's deliverable):** a C3 system observes the committed `Pointer<Click>` stream and feeds each click's `(entity, position, time)` through the **already-public** `text::edit::ClickTracker` heuristic (the 450ms / 4px window+radius the editor already tunes — `text/edit/pointer.rs`), then emits `MultiClick { count }` on the click's target entity when the run reaches `count >= 2`. The tracker is *promoted* from an editor-private classifier to the **one** multi-click source: the editor's `pointer_selection` (§2.8) consumes the same `ClickTracker` run for its intra-text double-/triple-click selection, and the widget-facing `MultiClick` event is the *same* run surfaced as an `EntityEvent` — there is exactly one definition of "double-click" in Buiy, not a parallel widget one.

**Why not reuse `Pointer<Click>.count`:** bevy_picking's `count` increments on *every* press in a rapid sequence but uses bevy's own (non-tunable) timing and does **not** apply Buiy's radius gate or the editor's window — so a widget keying off `count` would disagree with the editor about what a double-click is, re-introducing the split this event closes. `MultiClick` is the single arbiter.

**Owner / consumers:** C3 **owns** `MultiClick` (the gesture model, derived from `ClickTracker`, emitted widget-agnostically). **C8 consumes** it — the TodoMVC screen's todo row observes `MultiClick { count: 2 }` to enter edit-in-place (audit W17). C4 widgets that want a double-click affordance observe it too; the editor keeps using the underlying `ClickTracker` run directly for intra-text selection (§2.8) rather than round-tripping through the event.

---

## 3. Decisions & rejected alternatives

### 3.1 Camera-reference resolution (resolves OQ#1)

**Decision: resolve the camera by matching the pointer's target window to the `Camera` whose normalized render target is that window.** `Location.target` is a `NormalizedRenderTarget` (verified `bevy_picking-0.19.0-rc.3/src/pointer.rs:212-214`); for a windowed pointer it is `NormalizedRenderTarget::Window(NormalizedWindowRef(Entity))` (verified `bevy_window-0.19.0/src/window.rs:113`). The lookup: filter to `Window` targets (this *is* the per-window filter cross-cutting.md:93 demands — a pointer targeting a non-Buiy window resolves to no Buiy camera and is skipped), then find the `Camera` whose `RenderTarget` normalizes to the same window entity. The `camera.order` of that camera feeds `order = camera_order + 0.5`.

```rust
fn camera_for_window(cameras: &Query<(Entity, &Camera)>, window: Entity) -> Option<Entity> {
    cameras.iter().find_map(|(e, cam)| match &cam.target {
        RenderTarget::Window(WindowRef::Entity(w)) if *w == window => Some(e),
        RenderTarget::Window(WindowRef::Primary) => Some(e), // primary-window camera
        _ => None,
    })
}
```

For the single-window Buiy app this resolves to the one `Camera2d`. **Multi-window** falls out for free: each pointer resolves to its own window's camera; a pointer over a non-Buiy window resolves to `None` and is filtered. **Rejected — a `BuiyCamera` resource holding one camera entity:** simpler lookup but bakes in single-window, needs a registration step, and goes stale on camera respawn; the query-by-render-target is stateless and multi-window-correct. **Rejected — keep `Entity::PLACEHOLDER`:** it is the current lie; a real ref is the whole point of the camera fix and is needed for the per-window filter to mean anything.

*Caveat (deferred to C5/C8):* render-to-texture targets (`NormalizedRenderTarget::Image`) — a Buiy UI drawn into an offscreen texture composited onto a 3D surface — are out of v1 picking scope (3D-anchored UI is `buiy_3d`, cross-cutting.md:70-72). C3 filters to `Window` and documents the `Image`/`TextureView` gap.

### 3.2 Per-entity `painters_z` derivation (resolves OQ#3, OQ#4)

**Decision: derive a single global front-to-back paint order from the `StackingContext` set using render's existing `context_roots` + `context_tree_paint_order`, and set each node's pick depth to its reverse index in that list** (§2.3). Reusing render's own flatten is the simplification cascade (§6.1): there is *one* paint-order derivation, and pick-order is defined as its reverse — they cannot drift. The §6.1 contract pins the derivation as "index within nearest ancestor `StackingContext`, composed across nested contexts, ECS-tree-order tiebreak"; `context_tree_paint_order` is exactly that (descends nested SCs as atomic units; `context_roots` sorts degenerate multi-roots by entity).

OQ#4 (is `painters_z` populated for *all* pickable entities?) is resolved in §2.3: the root context's `painters_z` lists every in-flow descendant not owned by a nested context, and nested contexts recursively list theirs, so the flatten visits every *painted* entity; a non-painted entity is correctly non-pickable.

**Rejected — derive depth from `Stacking.z_index` alone:** `z_index` is only one tier of the paint algorithm (negative / in-flow / positioned-auto / positive — `systems.rs:4197-4212`); using it directly mis-orders in-flow siblings and ignores nested-context atomicity. **Rejected — keep smallest-area:** the documented Phase-0 stopgap that mis-picks under overlays (§1.2). **Rejected — a picking-private re-walk of the ECS tree:** duplicates the layout/render ordering logic, the exact divergence the audit (#6) and §6.1 warn against.

*Timing note (carried risk):* `painters_z` is written layout-side (sub-pass 6f, `systems.rs:4304`) in `Update`; `emit_picks` runs in `PreUpdate`, so it reads **last frame's** `painters_z`. This is the same one-frame posture as the `GlobalTransform` lag (§3.3) and is acceptable+documented; a stacking change takes effect for picking one frame later, identical to how it already takes effect for hover.

### 3.3 One-frame stale `GlobalTransform`/`painters_z` (accepted, documented)

`emit_picks` is in `PreUpdate`; the bridge writes `GlobalTransform` in `Update.after(Animate).before(Picking)` (`buiy_core/src/lib.rs:108-129`) and `painters_z` in layout (`Update`). So `emit_picks` reads both one frame stale. **Accepted** — it is the exact lag the editor already documents (`pointer.rs:120-129`) and the audit flags as acceptable (Bug 1 adversarial point). C3 documents it on `emit_picks`. **Rejected — move `emit_picks` to `Update` after the bridge:** breaks the `PickingSystems::Backend` contract (the whole pipeline expects backends in `PreUpdate`) and would desync from `InteractionPlugin`'s `PreUpdate` hover stage.

### 3.4 `should_block_lower` applied in the backend vs the hover stage

**Decision: apply `should_block_lower` in `emit_picks` (truncate the sorted picks at the first default-blocking entity), AND rely on the hover stage's own block-lower for cross-backend correctness.** bevy_picking's hover stage applies block-lower across *all* backends' hits; truncating in our backend keeps Buiy's own picks list minimal and is the cheaper path for the common single-backend case. The two are consistent (truncating a subset the hover stage would truncate anyway). **Rejected — emit the full sorted list and defer entirely to hover:** correct but emits dead picks every frame for deep trees; the backend already has the paint order in hand, so truncating is free.

### 3.5 The activation channel is `OnPress`, not a Buiy-native `Activate` event (reconciled with the agent-interface campaign)

**Decision (also recorded §2.5): there is NO Buiy-native `Activate` `EntityEvent`. The one activation sink is the existing `OnPress` message; C3 owns only the pointer-side press-arm → release-on-target → `OnPress` plumbing; the keyboard (Enter/Space) and AT (`Action::Click`) producers are the agent-interface campaign's, lowering into the *same* `OnPress`.** An earlier draft of this child invented `Activate`/`ActivateSource` + a dual-observe (`Pointer<Click>` + `Activate`) contract. That is **withdrawn** because the agent-interface campaign already owns the inbound action router (`route_action_requests`/`dispatch_action_request`) and already lowers `Action::Click → MessageWriter<OnPress>` — "the same message the pointer path emits" (`action-router.md` §4) — and wires Button keyboard activation (Enter + Space → `OnPress`) in its Phase 1c (`phasing.md`; `widget-contracts.md` §4: "a Button's Space handler and its `Click` honor both emit `MessageWriter<OnPress>`"). Defining `Activate` would create a *second* activation channel competing with the agreed-on `OnPress` sink, splitting the convergence the two campaigns already share. **Rejected — keep the Buiy-native `Activate` + dual-observe (the prior draft):** competes with the action router's `OnPress` lowering and would force every widget to observe two activation channels; the campaigns coordinate on one sink (`OnPress`). **Rejected — a pointer-only `Pointer<Click>` observe with no shared sink:** inaccessible (no keyboard/AT route); the shared `OnPress` *is* the accessible route, fed by the router. accesskit 0.24 has **no `Action::Default`**; the activation action is `Action::Click` — and the `Action::Click → OnPress` mapping lives in the agent-interface router, not here.

### 3.6 `Pointer<Scroll>` carries `deltaMode` (resolves OQ "deltaMode")

**Decision: rely on `Pointer<Scroll>` — it carries `unit: MouseScrollUnit` (`Line`/`Pixel`), the `deltaMode` distinction interaction.md:103 requires (verified `events.rs:457-472`).** No raw `MouseWheel` reader is needed; the wheel entry is a pure observer. **Rejected — read raw bevy `MouseWheel` for fidelity:** `Pointer<Scroll>` already wraps it with the hit target and unit; a parallel reader would re-implement hit attribution and lose the per-entity bubbling.

### 3.7 `Hovered` removal: shim, not hard-cut (resolves OQ "Hovered removal" + the name-collision OQ)

**Decision: keep `Hovered` as a thin shim during migration, populated from bevy_picking's `DirectlyHovered`, then delete it last (§2.10).** The ~3 reader files + 2 widget tests + 1 core test (`text_mouse_selection.rs`) migrate incrementally behind a green build; a hard-cut would red the whole tree in one commit (umbrella risk #1: "stage the migration; delete `Hovered` last behind a shim"). The **name collision** (bevy's `Hovered` component vs Buiy's `Hovered` resource) is avoided by *not* re-exporting bevy's `Hovered`/`DirectlyHovered` under those names while Buiy's resource lives (§2.9); after deletion, the bevy components are re-exported as the canonical surface. **Rejected — hard-cut in one commit:** violates the staged-migration mandate and the C7-gates-each-step discipline; a single red commit can't be bisected per-consumer.

### 3.8 `hit_test` free function disposition

**Decision: rewrite `hit_test` (`mod.rs:37-49`) to the absolute + paint-order semantics (drop smallest-area), keeping it as a test/inspection helper, but mark library consumers toward bevy_picking's `PointerInteraction.sorted_entities` for live hit state.** The free fn is used by tests; keeping a correct version is cheaper than rewriting every test to spin a full pipeline. It must share §2.3's `global_paint_order` so it can't diverge from `emit_picks`. It must also stay `pub` (and stacking-aware) because the **agent-interface campaign's `inprocess.rs` `act_when_actionable` consumes it** for its `HitTargetable` gate — that campaign's deferred follow-up #3 is exactly "a stacking-aware `hit_test`," and this rewrite supplies it (§2.3). **Rejected — delete `hit_test` outright:** forces every existing unit test onto the full real-input tier prematurely, *and* removes the function the agent-interface campaign's `HitTargetable` depends on; C7's tier exists for the *event* assertions, not for replacing every geometric hit-test.

---

## 4. Contracts & interfaces

### Shared contracts referenced (umbrella §6 — NOT redefined here)
- **§6.1 pick-depth from `painters_z`** — C3 OWNS; derivation = "index within nearest ancestor `StackingContext`, composed across nested contexts, ECS-tree-order tiebreak". Implemented in §2.3/§3.2 by reusing `render::extract::{context_roots, context_tree_paint_order}`. C5 consumes.
- **§6.2 coordinate space (C1) gates picking** — C3 consumes C1's non-optional `GlobalTransform`; `emit_picks` hit-tests absolute (§2.2). `bridge.rs:138` is an invariant to PRESERVE (C3 reads `GlobalTransform`, never writes `ResolvedLayout`).
- **§6.3 `Pointer<Scroll>`** — C3 owns the event **entry** (§2.6); C5 owns nearest-container routing + clamp + overscroll.
- **§6.4 focus** — C3 provides focus-on-click + the `:focus-visible` decay signal (§2.7); C5 owns the tree; C6 owns the ring paint; C4 consumes `FocusedEntity`. `Inert` (C5) gates the hit-test walk (C3 reads it once C5 ships it).
- **§6.6 focus-visible component shape** — C3 keeps the existing `FocusVisible` resource and writes only the pointer-side `false`; C6 confirms the final shape with C3/C5 before the ring-lowering.
- **§6.9 event vocabulary** — C3 defines only the **pointer-gesture** `MultiClick` (§2.11) and the pointer-side press→release→`OnPress` arming (§2.5). The semantic **activation** channel is the existing `OnPress` (no Buiy-native `Activate` event — withdrawn, §3.5); its non-pointer producers (keyboard Enter/Space, AT `Action::Click`) are the **agent-interface campaign's** action router, lowering `Action::Click → OnPress` (`action-router.md` §4). C4 widgets observe `Pointer<Press/Release>` for press visuals and converge on `OnPress`; the a11y state components (`A11yToggled`/`A11ySelected`/…), roles, and the router are the agent-interface campaign's, consumed by C3/C4 for visual state, not redefined.

### Own contracts (C3 defines precisely)
- **`emit_picks` output contract**: for each pointer targeting a Buiy window, exactly one `PointerHits` per frame, `order = camera_order + 0.5`, `picks` sorted top-most-first with `HitData { camera: <real>, depth: (n-1-paint_index), position: None, normal: None }`, truncated at the first `should_block_lower` entity, empty when no node is hit.
- **`Pickable::IGNORE` widget-internal convention** (§2.4): decorative/label nodes = `IGNORE`; the one interactive surface = default. Author code never sets `Pickable`.
- **Pointer-side activation plumbing** (§2.5): `Pointer<Press>` → armed marker; `Pointer<Release>` on-target → `MessageWriter<OnPress>(entity)`; off-target → clear (drag-cancel). The sink is the **existing `OnPress`** — the same message the agent-interface router's `Action::Click` honor and keyboard Enter/Space emit. C3 does NOT define a Buiy-native `Activate` event (withdrawn, §3.5); keyboard/AT activation is the agent-interface campaign's.
- **`MultiClick { entity, count, button }`** (§2.11): the widget-agnostic double/triple-click `EntityEvent`, emitted on **any** picked entity, derived from the now-public `text::edit::ClickTracker` heuristic (the single multi-click source). C3 owns it; C8 consumes it for edit-in-place (audit W17); the editor uses the underlying `ClickTracker` run directly, not the event.
- **`global_paint_order(contexts) -> Vec<Entity>`** (§2.3): the shared pick-depth primitive, reverse-indexed for depth; the §6.1 derivation made concrete.
- **Prelude re-export surface** (§2.9): stable Buiy names for `Pickable` + the `Pointer<E>` family + `MultiClick` (no `Activate` — withdrawn, §3.5).

---

## 5. Migration / build steps (ordered; blast radius)

Each step is gated by C7's Tier-A real-input harness staying green (umbrella §5 Wave-2 gate). C7 lands RED-first in Wave 1.

1. **Wire the pipeline** (§2.1). *Files:* `crates/buiy/src/lib.rs` (+`PointerInputPlugin`/`InteractionPlugin` guarded). *Blast:* no behavior change yet (backend still smallest-area); proves the plugins compose with the existing guard.
2. **Add `global_paint_order` + `point_in_node`** (§2.3, §2.2). *New file:* `crates/buiy_core/src/picking/depth.rs`. *Files:* possibly widen `render::extract::{context_roots, context_tree_paint_order}` visibility (already `pub`; confirm cross-crate-internal reach from `picking`). *Blast:* pure additive; unit-tested headless.
3. **Rewrite `emit_picks`** (§2.2): absolute `GlobalTransform` (consumes C1 — **C1 must land first**, §6.2), `painters_z` depth, no-hit emission, `Pickable` honoring, real camera, `order = camera_order+0.5`. *Files:* `crates/buiy_core/src/picking/backend.rs` (fully rewritten). *Blast:* `picking_backend.rs` test (umbrella risk #5: it hand-writes `ResolvedLayout` and is *blind* to Bug 1 — it must be rebuilt on the C7 harness, not trusted as the gate). Snapshots NOT affected (picking doesn't touch `ResolvedLayout`).
4. **(Withdrawn — no `Activate` type.)** The earlier draft added `Activate` + a `Pointer<Click>→Activate` bridge here; that step is **removed** (§3.5). The pointer-side press→release→`OnPress` plumbing is part of step 5; the keyboard/AT producers into `OnPress` are the agent-interface campaign's Phase 1c, not a C3 step.
5. **Migrate Button** (§2.8): delete `emit_on_press_on_click`; attach `Pointer<Press/Release>` observers that emit `MessageWriter<OnPress>` on release-on-target (bodies are C4's). *Files:* `crates/buiy_widgets/src/button.rs`; `crates/buiy_widgets/tests/button.rs` (`clicking_a_button_emits_on_press` hand-sets `Hovered` — migrate to the C7 synthetic-`PointerInput` tier). *Blast:* `button.rs` test; `OnPress` is the convergence sink the agent-interface router and keyboard path also feed.
6. **Migrate editor `pointer_selection` + add the `MultiClick` gesture** (§2.8, §2.11): source the editor from `Pointer<Press/Drag>`; keep `ClickTracker` for the editor's own double/triple selection; remove editor focus-on-click. Add the widget-agnostic `MultiClick` event + the C3 system that derives it from the (already-`pub`) `ClickTracker` heuristic over the `Pointer<Click>` stream, emitting on any picked entity. *New file:* `crates/buiy_core/src/picking/gesture.rs` (`MultiClick` + the derive system; register in `PickingPlugin`). *Files:* `crates/buiy_core/src/text/edit/pointer.rs` (system only; classifier byte-identical — only its visibility/reuse, not its math); `crates/buiy_core/tests/text_mouse_selection.rs` (migrate off hand-set `Hovered`). *Blast:* the one core test; classifier tests untouched; `MultiClick` is additive.
7. **Add focus-on-click + `:focus-visible` decay** (§2.7). *New system* in `picking` (or `focus`, coordinated with C5). *Files:* observes `Pointer<Press>`, writes `FocusedEntity` + `FocusVisible(false)`. *Blast:* `focus.rs` tests (the keyboard `true` path unchanged); the editor no longer sets focus-on-click (step 6 removed it).
8. **Delete `Hovered`/`update_hovered` + shim, re-export prelude** (§2.9, §2.10). *Files:* `crates/buiy_core/src/picking/mod.rs` (delete resource+system, rewrite/retire `hit_test`); remove `Hovered` from `crates/buiy_core/src/lib.rs:40` + `crates/buiy/src/lib.rs:29`; add the `Pointer<E>`/`Pickable`/`MultiClick` re-exports (no `Activate` — withdrawn, §3.5). *Blast:* every `Hovered` reader (verified set: `picking/mod.rs`, `button.rs`, `text/edit/pointer.rs` — all migrated by now; tests `button.rs`, `text_input.rs`, `text_mouse_selection.rs` — migrated in steps 5/6). `Pointer<Scroll>` entry smoke (§2.6) lands with or after this.

**Total verified blast radius:** 4 production files rewritten (`backend.rs`, `picking/mod.rs`, `button.rs`, `pointer.rs`), 1 plugin file (`buiy/lib.rs`), 2 root re-export files, 2 new files (`depth.rs`, `gesture.rs` — `activate.rs` withdrawn, §3.5); test migration: `button.rs`, `text_input.rs`, `text_mouse_selection.rs` + the rebuilt `picking_backend.rs`. The bulk of `FocusedEntity`-touching test files (text caret/ime/undo/etc.) set focus by hand for *focus-state* reasons unrelated to picking and are **not** in C3's blast radius (C5 owns focus). Layout/display-list goldens: unaffected.

---

## 6. Verification (how C7 gates this; RED-first)

C7's **Tier A** (`PointerHarness`, umbrella §4 C7) is the gate. It builds a real non-origin widget tree, runs layout→bridge→`GlobalTransform` + `InteractionPlugin` + Buiy's backend, injects a synthetic `PointerId` + `PointerInput` (the lessons.md:90-92 / §1 "keep synthetic input inside the bevy_picking abstraction" entry), and asserts on emitted `Pointer<E>` + observer capture. C3-specific predicates, each **proven RED before GREEN**:

1. **Absolute-coordinate pick (gates C1+C3).** A widget at a non-origin offset (parent translated) receives `Pointer<Over>`/`Press` only when the cursor is over its *absolute* box. RED on current `main` (smallest-area + relative `point_in_aabb` mis-picks). Revert C1 → this goes RED (the umbrella §5 "revert C1 → Tier A RED on an offset widget" gate). The existing `picking_backend.rs` (hand-writes `ResolvedLayout`, structurally blind to Bug 1 — umbrella risk #5) is rebuilt on this harness; it is NOT trusted as-is.
2. **Paint-order pick under overlay (§6.1).** Two overlapping nodes where the *larger* is painted on top (higher `painters_z` index): the cursor over the overlap region picks the **top-painted** node, not the smaller one. RED today (smallest-area picks the smaller). A `should_block_lower` occluder over a lower interactive node: the lower node receives **no** event.
3. **No-hit clears hover (§2.2).** Cursor over a node then moved off all Buiy nodes emits `Pointer<Out>` and clears `DirectlyHovered`. RED today (`emit_picks` skips no-hit emission, so hover never clears).
4. **`Pickable::IGNORE` pass-through (§2.4).** A label `Text` with `IGNORE` over a default-pickable button surface: a click reaches/activates the button, not the label.
5. **Pointer activation → `OnPress` (§2.5/§6.9).** A `Button` with the `Pointer<Press/Release>` observers emits `OnPress` on a synthetic press-then-release-on-target. C3's RED-first slice is the *pointer* path: inject press+release, assert `OnPress` fired (and §6 gate #8 covers press-off-target = no `OnPress`). The keyboard (Enter/Space → `OnPress`) and AT (`Action::Click → OnPress`) paths converge on the same `OnPress` sink but are the **agent-interface campaign's** gates (its #6 input-replay / #7 APG), not C3's — C3 only asserts the pointer producer and that all three feed one `OnPress`.
6. **`Pointer<Scroll>` entry (§2.6/§6.3).** A synthetic wheel `PointerInput` over a node fires `Pointer<Scroll>` with the expected `unit`/`y`. RED today (no wheel input exists at all).
7. **Camera ref (§3.1).** `HitData.camera` on an emitted pick equals the real `Camera2d` entity, not `Entity::PLACEHOLDER`; `order == camera.order + 0.5`. RED today (PLACEHOLDER + 0.0).
8. **Button press-arm/release-cancel (§2.8).** Press-on-button then release-off-button does NOT activate (drag-cancel); press-then-release-on-target activates. RED today (fires on mouse-down).
9. **`MultiClick` on a non-editor entity (§2.11/W17).** Two synthetic clicks within the `ClickTracker` window+radius over a plain (non-editor) entity observing `MultiClick` fire it with `count == 2`; a slow second click (outside the window) does NOT. RED today (no widget-agnostic double-click exists; the `ClickTracker` run is editor-internal). Asserts the gesture is derived from the same `ClickTracker` heuristic, not bevy's untuned `Click.count`.

**Tier C** (`#[ignore]` GPU/winit smoke, C7): one fixture exercising the real winit cursor→`PointerLocation` coordinate/scale path end-to-end, since the headless tier injects `PointerInput` directly and never runs `PointerInputPlugin`'s winit reader.

The shared `global_paint_order` (§2.3) gets a **pure headless unit test** asserting it equals render's `context_tree_paint_order` flatten on the same fixture (the no-divergence guarantee, §6.1).

---

## 7. Open questions deferred + dependencies

**Resolved in this spec:** camera-reference resolution (§3.1), per-entity `painters_z` derivation + the OQ#4 "all pickable entities populated?" (§3.2/§2.3), the `build_picks` occluder/truncation rule (§2.2), the **activation channel = `OnPress`** (no Buiy-native `Activate`; the prior draft's `Activate`/dual-observe is withdrawn — keyboard/AT activation is the agent-interface router's `Action::Click → OnPress`, §3.5/§2.5), the widget-agnostic `MultiClick` pointer-gesture double-click for edit-in-place — bevy_picking 0.19 has no `Pointer<DoubleClick>` (§2.11, audit W17), `Hovered` removal sequencing + the name-collision (§3.7), `deltaMode` carriage (§3.6), `should_block_lower` locus (§3.4), `hit_test` disposition (§3.8).

**Deferred (genuinely depend on un-built work):**
- **Shape-aware hit-testing** (rounded-rect / clip-path) beyond the v1 axis box. bevy_picking is rect-only (open-problems.md:60-74); Buiy's backend must eventually hit-test the rounded-rect/clip shape. Deferred because the clip/radius geometry it needs is C6's (border-radius/clip) and the editor keeps its own subpixel cosmic hit regardless. C3 ships the axis box + a documented TODO; not a v1 gallery blocker (the gallery's interactive surfaces are axis rects).
- **`Inert` honoring in the hit-test walk** (§6.4): `emit_picks` should skip `Inert` subtrees so a modal's backdrop blocks picks to inert content. `Inert` is **C5's** marker (not yet built); C3 leaves the one-line filter point documented and C5 adds the read when it ships `Inert`.
- **Render-to-texture / `Image` pointer targets** (§3.1 caveat): 3D-anchored UI picking is `buiy_3d` (cross-cutting.md:70). C3 filters to `Window`.
- **Pointer capture / `gotpointercapture`** (interaction.md:75, C-tier) and the **OS drag-and-drop bridge** (integration.md:68-70): deferred to the drag/`buiy-input-events-design` follow-up; C3 lands the `Pointer<Drag*>` entry the editor and any reorder widget need, not the capture/OS-bridge layer.

**Hard dependencies:**
- **C1 (coordinate space)** — BLOCKING. `emit_picks`'s absolute hit-test requires C1's non-optional `GlobalTransform` decision + the `components.rs:64` doc-comment fix to have landed (step 3 cannot precede C1). §6.2.
- **C7 (verification)** — the Tier-A harness must exist RED-first (Wave 1) before any C3 consumer migration, so the ~3 reader files + tests stay CI-green through the staged migration. §5 every step gated on it.
- **C4 (widget-state + a11y) + the agent-interface campaign** — feeds: C3's `Pointer<Press/Release>` are what C4's widgets observe to drive press visuals and emit `OnPress`; C4 consumes the agent-interface campaign's a11y state components (`A11yToggled`/`A11ySelected`/…) for *visual* state. The keyboard/AT producers into `OnPress` are the **agent-interface campaign's** action router + Phase-1c keyboard wiring (§3.5), not C3's and not C4's invention. C3 lands the pointer plumbing; the agent-interface campaign lands the non-pointer activation producers.
- **C5 (scroll/overlay/focus)** — consumes C3's `painters_z` pick-depth (§6.1), the `Pointer<Scroll>` entry (§6.3), and the focus-on-click signal (§6.4); provides `Inert` back to C3's hit-test walk.
- **C6 (styling)** — consumes C3/C5's `:focus-visible` signal for the ring lowering (§6.6).

---

## Coordination with the agent-interface campaign

The agent-interface campaign OWNS the a11y substrate and the **inbound semantic action channel**; this child (C3) owns the **pointer-geometry + pointer-input** layer beneath it. The split, precisely:

**C3 OWNS (pointer geometry + pointer input — this campaign's deliverables):**
- The `Pointer<E>` pipeline wiring (`PointerInputPlugin`/`InteractionPlugin`, §2.1) and the `emit_picks` rewrite (§2.2): absolute hit-test via non-optional `GlobalTransform` (consumes C1), real camera ref, `order = camera_order + 0.5`, no-hit emission, `Pickable::IGNORE`/`should_block_lower` pick-through, and **replacing smallest-area depth with `painters_z`/stacking paint-order**.
- **The stacking-aware `hit_test` the agent-interface campaign DEFERRED and DEPENDS on.** Its named follow-up #3 (`phasing.md` #3; `inprocess-api.md` §5.1) is "a stacking-aware `hit_test`," needed before its in-process driver's `HitTargetable` actionability gate (`act_when_actionable`) can mean "not obscured by a modal/top-layer/tooltip." C3's paint-order pick-depth (§2.3) + the `hit_test` rewrite (§3.8) *is* that stacking-aware hit-test; C3 keeps `picking::hit_test` + `global_paint_order` `pub` so that campaign's `a11y/inprocess.rs` consumes them. This unblocks the campaign's follow-up #3.
- The `Pointer<Scroll>` wheel **entry** (§2.6) and the widget-agnostic **`MultiClick` pointer-gesture** (double/triple-click, §2.11) — a *pointer-layer* gesture, distinct from the semantic action router, derived from the editor's `ClickTracker` timing/radius. It feeds **C8's edit-in-place** (a todo row observes `MultiClick { count: 2 }`); it does not route through the action router.
- The **pointer side** of activation (§2.5): `Pointer<Press>` → armed; `Pointer<Release>` on-target → `MessageWriter<OnPress>`; off-target → drag-cancel. Plus the **focus-on-click** signal (`Pointer<Press>` → `FocusedEntity`, §2.7).

**C3 CONSUMES / DEFERS TO the agent-interface campaign (does NOT build):**
- **The semantic activation channel.** Activation lowers through the **existing `OnPress`** message and the agent-interface campaign's **action router** (`route_action_requests`/`dispatch_action_request`), which dispatches `Action::Click → MessageWriter<OnPress>` (`action-router.md` §4) and wires **Button keyboard activation** (Enter + Space → `OnPress`) in its Phase 1c (`phasing.md`; `widget-contracts.md` §4). C3 supplies only the pointer producer into that same `OnPress` sink.
- **No competing semantic activation event.** The earlier draft's Buiy-native `Activate` `EntityEvent` + `ActivateSource` + dual-observe contract is **withdrawn** (§2.5/§3.5): it competed with the router's `OnPress` lowering and would split the one agreed-on sink. C3 defines no `Activate` type, no `Activate` observer, no `Pointer<Click>→Activate` bridge; the `activate.rs` file and migration step 4 are removed.
- **The a11y substrate.** A11y roles (`A11yRole`), the decomposed a11y-state components (`A11yToggled`/`A11ySelected`/`A11yExpanded`/`A11yValue`/…), the action router, `EditCommand::SetSelection`, the in-process driver, and the a11y verification gates (#3/#4/#6/#7/#12) are all the agent-interface campaign's. C3 (and C4, downstream) **consume** the a11y-state components for *visual* state only and never redefine them. The keyboard/AT activation gates belong to that campaign's #6/#7, not to C3's verification (§6 gate #5 covers only the pointer producer).
