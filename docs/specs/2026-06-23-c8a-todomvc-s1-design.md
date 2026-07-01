# C8-a — S1 TodoMVC screen + inspection-driver acceptance (design)

**Date:** 2026-06-23
**Status:** landed
**Kind:** decision — a scoped design note (records the picks + the runner-up for S1)

Realizes child **C8** (`docs/specs/2026-06-22-buiy-widget-catalog-design/widget-gallery-exemplar.md`) **S1 only**.

> Two reasonable approaches existed for the "add a todo" commit seam and for the
> screen's app-logic placement; this note records the picks + the runner-up,
> per the project `CLAUDE.md` "answer not obvious? design" rule. S2–S5 are later
> C8 slices and are out of scope here.

## 1. Scope

The first capstone screen: a TodoMVC composed **purely** from the landed P1d
widget bundles (TextInput single-line, tri-state Checkbox, Button), the C5/C6
containers + styling, the C3b `MultiClick` gesture, and the A11yLive `Status`
live region. C8 defines **no** primitive — it arranges, styles, renders, and
wires app logic. Plus the inspection-driver acceptance: every interaction branch
driven through `buiy_core::a11y::inprocess` (the in-process driver) and asserted
through the a11y snapshot, with the same flips confirmed via the C7
`PointerHarness` (real synthetic pointer) and synthetic keyboard.

Behaviors: add (type field + Enter → row appended), toggle (Checkbox →
`A11yToggled` flips + strike/dim visual), destroy (Button → row removed),
clear-completed, filter All/Active/Completed, "N items left" count in an
`A11yRole::Status` `A11yLive` region, and edit-in-place (double-click a label →
the label becomes an editable single-line TextInput; Enter/blur commits).

## 2. Crate placement (C8 §3.5)

New `examples/buiy_gallery/` crate (lib.rs `screen_todomvc()` scene-fn +
`DEMO_SEEDS` + the `TodoMvcPlugin` app-logic plugin + main.rs booting it), and
the `buiy_verify` fixture wrapper `fixtures/gallery/todomvc.rs` (thin
`Camera2d + spawn_scene` shell) registered in `coverage/mod.rs`. `buiy_verify`
gains a dev-dependency on `buiy_gallery`. Mirrors `hello_bsn`.

The gallery crate depends on `buiy` (prelude + `bsn!` + widgets/scene-fns) and
directly on `buiy_core` for the a11y-state components + messages the prelude
does not flatten (`A11yToggled`/`Toggled`/`A11yLive`/`EditSubmitted`/…) — the
same import convention the widget tests use.

## 3. Decisions

### 3.1 The scene is one tree; behavior is a `TodoMvcPlugin` (systems) on top

**Decided.** `screen_todomvc()` authors the *static* tree (the add-field, an
empty `#TodoList` container, the footer with the Status region + filter buttons +
clear-completed). The runtime behavior — append/remove rows, toggle, filter,
recount — is a `TodoMvcPlugin` that registers retained-mode systems
(`.after(BuiySet::Input)`), exactly the prototype's KEEP model (C8 §3.4). The
fixture spawns only the scene (static snapshot); the acceptance test adds the
plugin (live behavior). This is the C8 §2.4 "fixture is the snapshot, the live
gate is separate" split, and it is *why* `build_app` (CPU-only, no WidgetsPlugin
systems) can still snapshot the screen.

**Rejected — bake behavior into the scene via observers attached in the
scene-fn.** It couples the static fixture to the behavior systems and makes the
layout snapshot non-deterministic (observers firing during the snapshot's single
update). Keeping logic in a plugin keeps the fixture a pure tree.

### 3.2 Add-todo commit seam = `EditSubmitted` (single-line Enter), the canonical path

**Decided.** The add-field is `text_input_single_line`; a single-line Enter emits
`EditSubmitted(entity)` (`buiy_core::text::edit`). `add_todo_on_submit` reads it,
snapshots the field's `TextEditState::value()`, appends a row if non-empty, and
clears the field via the driver-equivalent `EditCommand::SelectAll` +
`EditCommand::Insert("")` lowering (C8 §4.3 — no `EditCommand::SetValue`). The
inspection driver reaches the same path: focus the field, `set_value("buy
milk")` (lowers through SelectAll+Insert), then a synthetic Enter `KeyboardInput`
drives `apply_keyboard_edits` → `EditSubmitted` → the append.

**Rejected — a dedicated "Add" button as the only commit.** TodoMVC's canonical
affordance is Enter-in-the-field; an Add button is redundant for S1 and would add
a second activation path to reconcile. Enter is the one commit seam. (A driver
`click` on a button is already exercised by the destroy/clear/filter branches, so
button-activation coverage is not lost.)

### 3.3 Row toggle/visual reads `A11yToggled`; app state derives from it

**Decided.** Each row's Checkbox carries the tri-state `A11yToggled` (the P1d
bundle's state — C8 §2.5(6)/§4.3). The shared `advance_toggle_on_press` consumer
(WidgetsPlugin) flips it on any `OnPress` (pointer/keyboard/AT-Click converge).
`restyle_completed` reads `Changed<A11yToggled>` on rows and applies the
strike/dim (here: a `TextColor` dim + a `CompletedRow` marker; a real
strike-through decoration is a render-layer follow-up). `update_count` recounts
incomplete rows on any row change and writes the Status `A11yLabel`. C8 defines
**no** parallel `Checked`/`ToggleState` component (C8 §2.7 supersede).

### 3.4 Filter = `Hidden`-marker hide, not `Display` rewrite (C8 §3.4)

**Decided for the a11y-correct hide.** `apply_filter` adds/removes the
`A11yHidden` marker on rows that do not match the active filter. `A11yHidden`
prunes the row (and subtree) from the a11y tree (`build_tree` §7.4 PRUNE) — so a
filtered-out row leaves the tree, which is the correct semantic (C8 §3.4 rejects
`CssVisibility/ContentVisibility::Hidden` for the filter case). C5's resolved-
display `Hidden` *layout* marker is the visual half; for S1 the load-bearing,
testable property is the **a11y prune**, which `A11yHidden` delivers on main
today, so S1 uses `A11yHidden` and also sets `CssVisibility::Hidden` for the
visual collapse. (The C5 layout `Hidden` marker integration is an S2+ container
concern; S1's filter correctness is fully observable through the a11y tree.)

### 3.5 Edit-in-place = in scope (C3b `MultiClick`)

**Decided to land it.** A double-click (`On<MultiClick>` with `count >= 2`) on a
row's label swaps the label `Text` child for a `text_input_single_line` seeded
with the label text; Enter (`EditSubmitted`) or focus-loss commits the new text
back to the label and restores the static label. It is tractable on the landed
surfaces (MultiClick observer + dynamic child swap + EditSubmitted) and is the
exemplar's signature interaction, so it ships in S1 rather than deferring.

## 4. Verification

- **Inspection-driver acceptance** (`crates/buiy_verify/tests/verify_headless/todomvc_c8a.rs`):
  a headless app (`MinimalPlugins + CorePlugin + A11yPlugin + BuiyTextPlugin +
  FocusPlugin + WidgetsPlugin + TodoMvcPlugin`, `KeyboardInput` message +
  `ButtonInput<KeyCode>` seeded) spawns the screen, then drives each branch
  THROUGH `inprocess` (`get_by_role`/`set_value`/`click`/`snapshot`/`wait_for`)
  and asserts via the a11y snapshot. Pointer convergence uses the C7
  `PointerHarness`-style synthetic `PointerInput`; keyboard convergence uses
  `KeyboardInput`/`ButtonInput<KeyCode>`. Every assertion reads the a11y tree,
  not internal state — that IS the inspection.
- **Layout snapshot fixture** (`fixtures/gallery/todomvc.rs`): the static screen
  enrolls across the CPU structured tiers by construction.
- **Runnable example**: `cargo run -p buiy_gallery`.

S2–S5 (scroll/long-list, overlay/menu, modal/focus-trap, F-tier showcase), the
`Matrix::gallery_screen()` reduced matrix, `AUTHORING.md`, and the canonical
goldens are later C8 slices, not this one.
