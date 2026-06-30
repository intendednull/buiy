# MVU Spike Wave 2 — TodoMVC with Real Buiy Widgets

> **THROWAWAY PROTOTYPE — DO NOT MERGE.** This report is the deliverable; the code
> is an unmerged reference in `examples/mvu_spike/`.

**Date:** 2026-06-26  
**Gate status:** 18/18 headless tests green + GUI boots clean (no panic, no B0004, real
content renders on RX 6700 XT).

---

## 1. What was built

`TodoMVC` driven by the Wave 1 MVU runtime, using **real Buiy widgets**
(`checkbox(text)`, `button("×")`, `button(label)`, `text_input_single_line`).
Not the gallery's retained-mode plugin — a fresh structural path that validates
the MVU integration directly.

Key additions over Wave 1:
- `TodoList` model: `items: Vec<Todo>`, `filter: Filter`, keyed by `TodoId(u64)`.
  Pure `update()` — no ECS references, no spawn/despawn.
- `reconcile_rows` exclusive system: diffs model vs `RowKey` children, spawns/
  despawns real widget rows. All structural ECS mutations live here, outside `update`.
- `update_footer` system: reads `Changed<TodoList>`, writes "N items left" to a
  `FooterText`-marked `Text` entity.
- `route_add_submit`: routes `EditSubmitted` from the add-field text input to
  `Add(text)` in `Inbox<TodoList>`.
- 11 headless tests covering all 5 message types, keyed reconcile, filter, footer
  derivation, purity, and MsgLog.

---

## 2. Q6 — Keyed dynamic collection: CONFIRMED

**Finding:** ECS entity identity naturally implements keyed reconcile. `RowKey(TodoId)`
is the stable domain ID placed on each row entity. `reconcile_rows` diffs
`TodoList.items` vs `RowKey` children: spawn for new items, despawn for removed items,
no-op for unchanged items. No VDOM, no virtual key reconciler.

**Proof:** `test_destroy_removes_middle_row_by_id` — destroy item[1] of 3; items[0]
and [2] survive in their original entities. Index-shift is impossible by construction:
the reconcile key is `TodoId`, not Vec position.

**Bevy 0.19 friction:** `Children::iter()` yields `Entity` directly (via
`RelationshipTarget::iter()` → `Copied<Iter<Entity>>`). Calling `.copied()` on top
produces `Copied<Copied<...>>` which is a type error. All `.copied()` calls after
`children.iter()` must be removed. Additionally, `filter` passes `&Entity` to the
predicate (standard iterator semantics); `filter_map` passes `Entity` by value. Pattern
to use: `c.iter().filter(|e| f(*e))` and `c.iter().filter_map(|e| f(e))`.

---

## 3. Q2 — Derived footer via query: CONFIRMED (with boilerplate cost)

**Finding:** The "derived via query" path works and is semantically clean. The
`update_footer` system reads `Changed<TodoList>` (filter prevents spurious rewrites)
and writes the footer text via `set_if_neq`. Cost: 1 dedicated system + 1 marker
component (`FooterText`) + ~12 lines.

**Comparison with Elm:** In Elm, `view model` computes `active_count()` inline — no
extra system, no marker. The ECS model pays a fixed overhead per derived view field.
For 1–5 derived fields this is tolerable. At 20+, it becomes a scaffolding tax.

**Layout topology determines the pattern:** `BoundText<M>` (Wave 1) requires the text
entity to be a DESCENDANT of the model entity (ancestor-walk FROM the text TO the model).
The footer is a SIBLING of the list container, not a descendant — `BoundText` won't
reach it. Rule: descendant topology → use `BoundText<M>`; sibling/cross-subtree topology
→ use a dedicated `Changed`-gated system.

---

## 4. Purity boundary: ENFORCED BY TYPES

**Finding:** The `update()` purity boundary is enforced structurally by Rust's type
system, not by convention. The signature is:
```rust
fn update(&mut self, msg: TodoListMsg) -> Cmd<TodoListMsg>
```
There is no `World`, no `Commands`, no `Entity` parameter. You literally cannot call
`world.spawn()` inside `update()`. Structural ECS work lives ONLY in `reconcile_rows`
(an exclusive system: `fn reconcile_rows(world: &mut World)`).

**Proof:** `test_update_purity_no_ecs_access` calls `list.update(msg)` standalone (no
world, no app, no Bevy context) and it compiles and runs cleanly.

**Value:** purity enables the record/replay + agent-driving properties that motivated
the MVU design. Bevy ECS makes the boundary feel natural, not artificial.

---

## 5. Controlled vs self-updating toggle: DOUBLE-WRITE, NO FIGHT (normally)

**Finding:** Two writes to `A11yToggled` per toggle event:

1. `advance_toggle_on_press` (in `BuiySet::Input`): self-advances `A11yToggled` optimistically before MVU routing runs.
2. `reconcile_rows` (after `MvuSet::Drain`): writes `A11yToggled` from `items[id].done` — the model-authoritative write.

Both writes agree on flip direction, so no conflict in the happy path. The checkbox
visually updates from step 1, then reconcile reinforces the same state in step 2.

**Where this breaks:** if the model rejects the toggle (e.g. a validation gate that
returns `Cmd::none()` without flipping `done`), `advance_toggle_on_press` flipped the
visual to `done=true`, but reconcile writes it back to `done=false`. This is a one-frame
visual flicker (both happen within the same `app.update()` call — actual timing depends
on whether the rendering pipeline reads between the two writes).

**Production fix:** suppress `advance_toggle_on_press` on checkbox entities that carry
`OnPressMsg<M>`. The controlled-model path should be model-authoritative from the start,
not optimistic-then-corrected. This is a worthwhile addition to `WidgetsPlugin` for the
final design.

---

## 6. View integration: OnPressMsg on real widgets — CONFIRMED

**Finding:** Placing `OnPressMsg::<TodoList>::new(msg)` on a real `Button` or `Checkbox`
entity (after `world.spawn_scene(button("×"))`) is clean authoring. The full routing
path — pick-click → `Messages<OnPress>` → `route_on_press` → `Inbox<TodoList>` →
`drain` → `update()` → reconcile — works end-to-end.

**Proof:** `test_destroy_via_on_press` writes `OnPress(destroy_btn_e)` via
`world.write_message(OnPress(entity))`, calls `app.update()`, and the row is despawned
by reconcile. This is the same seam the picking/keyboard/AT driver uses.

**Wave 1 gap closed:** Wave 1 used bare entities (`spawn(OnPressMsg)`), not real widget
entities. The picking path never ran. Wave 2 uses `spawn_scene(button("×"))` then
`entity_mut(e).insert(OnPressMsg::new(msg))` — real widgets, real path.

---

## 7. Infrastructure findings

These are fixed costs paid once when setting up the headless test harness. Not blockers,
but must be documented so the final design doesn't re-discover them.

**LayoutPlugin is required alongside WidgetsPlugin.** `WidgetsPlugin` registers systems
in `BuiySet::Layout` which requires `LayoutPlugin`'s resources. Gallery tests include
it; any test that uses `WidgetsPlugin` must also include `LayoutPlugin`.

**`FocusPlugin::handle_tab` requires `Res<ButtonInput<KeyCode>>` (non-optional).**
`MinimalPlugins` does not include `InputPlugin` (which registers `ButtonInput<KeyCode>`).
Fix: `app.init_resource::<ButtonInput<KeyCode>>()` before `app.add_plugins(FocusPlugin)`.
Pattern already documented in `gallery/tests/modal_layout.rs`; now also in `wave2.rs`.

**`MvuSet::Route.after(BuiySet::Input)` belongs in the app plugin, not MvuPlugin.**
`MvuPlugin` cannot add this ordering because Wave 1 tests use `MinimalPlugins` without
`CorePlugin`/`BuiySet`. The ordering constraint is added in `TodoMvcMvuPlugin.build()`
only when the full Buiy stack is present.

**System ordering for `reconcile_rows + update_footer`:**
```
BuiySet::Input → MvuSet::Route → MvuSet::Drain → reconcile_rows / update_footer → BuiySet::A11yUpdate
```
`reconcile_rows` must run after `Drain` (model has been updated) and before `A11yUpdate`
(outbound a11y tree snapshot). `update_footer` can run alongside `reconcile_rows`
(`Changed<TodoList>` is already set by `Drain`'s `get_mut`).

---

## 8. Summary of open questions and next candidates

| Q | Status | Notes |
|---|--------|-------|
| Q1 Routing | ✅ RESOLVED (Wave 1) | Ancestor-walk + explicit-address both confirmed |
| Q2 Composition | ✅ ANSWERED | Works; derived-via-query costs 1 system + 1 marker per derived field |
| Q3 Parent↔child (OutMsg) | ❌ NOT YET | Self-contained composite widget → parent model. Wave 2b. |
| Q4 Effects (Cmd::task) | ❌ NOT YET | Async task round-trip, fold-back, cancellation on despawn. Wave 3. |
| Q5 View (bind!/retained) | ⚠️ PARTIAL | `BoundText<M>` validated (Wave 1); `bind!` macro not built |
| Q6 Keyed collection | ✅ RESOLVED (Wave 2) | `RowKey(TodoId)`, keyed reconcile, no VDOM needed |
| Q7 Record/replay | ⚠️ PARTIAL | `MsgLog` built; replay mode not yet implemented |

**Wave 2b / final design candidates (in priority order):**

1. **Suppress `advance_toggle_on_press` for controlled checkboxes** — the double-write
   finding (§5) needs a production fix. Either `WidgetsPlugin` checks for `OnPressMsg`
   before self-advancing, or the MVU runtime suppresses the pre-advance for controlled
   models.

2. **OutMsg composite (Q3)** — a self-contained `TextInput` widget that emits an output
   Msg to its parent model. The "adoption edge" pattern. This is the common case for
   form widgets.

3. **Cmd::task round-trip (Q4)** — async effect: spawn a task, fold the result back as
   a Msg to the originating entity. The `InFlight<Msg>` component on the entity as the
   fold-back address. Drop = cancel via Bevy 0.19 task drop semantics.

4. **Reorder with transient state preserved** — drag-reorder a todo row entity; the row
   entity IS the identity, so widget state (focus, text cursor) is preserved across
   reorders without extra logic. This is the "move-entity" reorder strategy.

5. **Human-gate before any `bind!` macro work** — `BoundText<M>` is sufficient for the
   prototype; a proper `bind!` macro is final-design scope.

---

## Gate verdict

Wave 2 is **COMPLETE** for the questions it was chartered to answer (Q2, Q6, purity
boundary, view integration, controlled toggle probe). All 18 headless tests are green;
GUI runs clean on RX 6700 XT.

The code in `examples/mvu_spike/` is throwaway. The findings above, plus the Wave 1
journal entries, are the inputs to the human-gated final design brainstorm.
