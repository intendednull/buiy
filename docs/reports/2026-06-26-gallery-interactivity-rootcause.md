# Gallery interactivity root-cause + the live-interaction test tier

**Date:** 2026-06-26
**Branch:** `parity-final` (open PR #83)
**Status:** `[landed]` — fix confirmed faithful, full gate green, new test tier closes the gap.

## TL;DR

Running `cargo run -p buiy_gallery` rendered all five screens perfectly but was
**completely non-interactive** — no click did anything. Root cause: the picking
backend (`emit_picks`) hit-tested **every** node with a `ResolvedLayout` +
`GlobalTransform`, ignoring visibility. The S4 **Modal Dialog** is a detached,
full-window, top-layer `Dialog` that sits at `CssVisibility::Hidden` at rest;
because it is **paint-skipped but keeps its full-window layout box and is topmost
in the stack**, it absorbed every click while painting nothing. `A11yRole::Dialog`
is not activatable, so `OnPress` never fired → the whole app read as dead.

The fix (one guard in `emit_picks`): skip any node carrying `ComputedPaintSkip`,
so the **pick-set equals the paint-set**. This is the visibility analogue of the
existing pick-order == paint-order co-drive (SC-3).

The entire prior test suite missed it because **no test ever composed the real
shell and drove a real pointer click through the real picking pipeline**. This
report records the investigation, the fix, why the suite was blind, and the new
live-interaction tier (`examples/buiy_gallery/tests/interaction.rs` +
`crates/buiy_core/tests/crosscut/picking_backend.rs`) that closes the gap.

## Symptom

- `cargo run -p buiy_gallery` paints the full IDE shell + all five screens
  faithfully (the headless layout-snapshot + GPU-golden gates were all green).
- **Every pointer click is inert**: clicking a nav-rail button does not switch
  screens, clicking a todo checkbox does not toggle, clicking an accent swatch
  does not re-theme. Keyboard focus paths (Tab, arrows) still worked — only the
  pointer was dead.

## The multi-layer pipeline investigation

The pointer path has many layers, any of which could swallow a click:

```
winit cursor → PointerInput → PickingPlugin (receive → emit_picks → hovermap)
   → Pointer<Over/Press/Click> → C3b pointer_click_emits_on_press → OnPress
   → the app's collect_* intent systems → the appliers
```

1. **winit → `PointerInput` proven live.** Instrumenting the input reader (and a
   live `xdotool` click probe on `DISPLAY=:0`) confirmed real cursor moves and
   button presses were producing `PointerInput` messages and updating
   `PointerLocation` / `PointerPress` — the OS→ECS edge was healthy.
2. **Elimination down the chain.** `emit_picks` *was* emitting `PointerHits`
   every frame, and `Pointer<Over>`/`Pointer<Press>`/`Pointer<Click>` *were*
   firing — but the **target** of every `Pointer<Click>` was the same entity
   regardless of where the cursor was.
3. **The decisive finding.** That entity was the **`ModalDialog`**. Every click,
   anywhere on screen, resolved to the closed modal dialog. Because the dialog is
   `A11yRole::Dialog` (not in `is_activatable_role` = `Button`/`Checkbox`/`Switch`),
   `pointer_click_emits_on_press` never lowered it to `OnPress`, and — crucially —
   the dialog (a default-`Pickable`, `should_block_lower` occluder at the top of
   the paint order) **truncated the candidate list** in `resolve_picks`, so the
   real button beneath it was never in the pick set. Total click deadness.

### Why the dialog is present on *every* screen

The S4 dialog (`build_modal_dialog`) is spawned **detached**: `spawn_modal` returns
`(create_invoker, dialog, delete_invoker)` and adds only the invokers/caption under
`#ModalRoot`; the dialog is never parented under `#ModalRoot` (or `#ScreenContent`).
The screen router (`apply_screen_router` / `set_active_screen`) toggles
`Display::None` only on the `ScreenRoot` subtrees under `#ScreenContent`. The
dialog lives **outside** that, so the router never hides it — it is a permanent,
full-window, `CssVisibility::Hidden`, `TopLayer::Modal` root on the Todo screen,
the Scroll screen, every screen. That is why the dead-click was universal, not
modal-screen-specific. (The new harness re-confirms this directly: an *unscoped*
`Query<Entity, With<Switch>>` over the composed app returns the dialog's
`#ModalRegisterSwitch` first, even while the Showcase screen is active — the hidden
dialog's contents are always laid out.)

## Root cause

**Pick-set ≠ paint-set.** The render pipeline computes a single per-entity skip
marker, `ComputedPaintSkip`, stamped by the `write_paint_skip` render-prep pass
across every `Display::None` / `CssVisibility::Hidden` / `OffscreenAuto` subtree
(`crates/buiy_core/src/render/visibility.rs`). Extract reads it as the single skip
source, so a hidden node paints nothing. But `emit_picks` did **not** consult it —
it hit-tested every node with a `ResolvedLayout` + `GlobalTransform`. So a node
that is hidden (paints nothing) but retains a layout box was still a pick target,
and a hidden **occluder** at the top of the stack swallowed clicks for everything
beneath it. The closed modal dialog is exactly that: hidden (so invisible), full
window (so it covers everything), top-layer (so it is the topmost paint /
pick candidate).

## The fix

One guard in `emit_picks` (`crates/buiy_core/src/picking/backend.rs`): the `nodes`
query gains `Option<&ComputedPaintSkip>`, and a node carrying it is skipped before
it becomes a `PickCandidate`:

```rust
if paint_skip.is_some() {
    continue;
}
```

Now the **pick-set == the paint-set**: a node the renderer skips is not a
hit-target. This excludes hidden top-layer overlays (the closed modal dialog) that
keep a layout box but paint nothing, and it matches web semantics
(`visibility:hidden` / `display:none` are not hit-targets). It is the visibility
analogue of the existing pick-order == paint-order co-drive (SC-3): the two paths
that derive "what is on top" now also agree on "what is hidden".

## Why the entire prior test suite missed it

Every existing interaction test exercised a path that **bypasses the composed-app
hit-test**:

- **State-injection tests** (e.g. the gallery behavior tests) write the widget
  state directly (`A11yToggled`, `A11yValue`, `ScrollIntents.select`, `OnPress`
  messages) or stage app intents — never routing a pointer through `emit_picks`.
- **a11y-driver tests** drive the AccessKit tree / action router — a different
  modality that never touches the picking backend.
- **The one real-pointer test** (`crates/buiy_widgets/tests/button.rs`
  `clicking_a_button_emits_on_press`) composes a **minimal** app: one button, a
  window, a camera — no shell, no hidden modal, no overlapping top-layer occluder.
- **Layout-snapshot / GPU-golden tests** assert static structure and pixels; a
  fully-painted-but-unclickable app passes every one of them.

So a hidden-node-absorbs-clicks bug in the **composed** app was invisible: no test
stood up the real shell *and* drove a real click *through* the real backend. The
headless gate also never ran the binary (this echoes the
[2026-06-24 live-run report](2026-06-24-widget-catalog-rendering-and-crash-bugs.md):
"headless-complete ≠ works — always RUN the GUI").

## The new live-interaction tier (closing the gap)

### Tier A — shell-integration interaction tests

`examples/buiy_gallery/tests/interaction.rs` (16 tests). A headless harness
composes the **same plugin set the binary boots** — `BuiyPlugin` (which pulls the
real `bevy_picking::PickingPlugin`, Buiy's `PickingPlugin` +
`BuiyPickingBackendPlugin`, layout, the transform bridge, **and** the
`write_paint_skip` render-prep pass) plus every screen plugin
(`ScreenRouterPlugin` / `InspectorPlugin` / `TodoMvcPlugin` / `ScrollListPlugin` /
`OverlayMenuPlugin` / `ModalPlugin` / `ShowcasePlugin` / `ToastPlugin`) — over a
synthetic primary window + the shell's own `Camera2d`. A `click_entity(e)` helper
resolves the entity's `ResolvedLayout` + `GlobalTransform` center and injects a
synthetic `PointerInput` **move → press → release** through the live backend, so
the click travels the real `emit_picks → Pointer<Click> → OnPress` path. Each test
asserts the **observable** state change:

| Feature | Test | Asserts |
| --- | --- | --- |
| Nav rail → each screen | `nav_clicks_switch_each_screen` | clicking each rail button moves `ScreenRouter` + flips which `ScreenRoot` is `Display::None` |
| Nav active-state | `nav_click_reflects_rail_active_state` | the clicked nav button takes the active `surface.card` bg |
| Accent swatch | `accent_swatch_click_retones_the_theme_accent` | clicking Green moves the live `Theme` accent token to green |
| Todo checkbox | `todo_checkbox_click_toggles_completion` | a completed row's checkbox toggles `A11yToggled` off |
| Todo filters | `todo_filter_pill_click_sets_the_active_filter` | clicking Active sets `Filter` + hides the completed row |
| Todo clear-done | `todo_clear_done_click_removes_completed_rows` | Clear done despawns the completed row (3 → 2) |
| Todo add | `todo_add_via_field_focus_type_and_enter_appends_a_row` | click-to-focus + type + Enter appends a row with the typed text |
| Menu open + activate | `menu_button_click_opens_then_keyboard_activates_an_item` | ⋮ click opens (`A11yExpanded`/`CssVisibility`); Enter on the active item records the activation |
| Menu dismiss | `menu_outside_click_light_dismisses_the_open_menu` | an outside click closes the menu |
| Modal open/trap/Esc | `modal_invoker_click_opens_and_traps_focus_then_esc_closes_and_restores` | invoker click opens + moves focus inside; Esc closes + restores focus to the invoker |
| Modal close button | `modal_dialog_close_button_click_closes_the_open_dialog` | a `DialogClose` click closes the open dialog |
| Showcase switch | `showcase_switch_click_toggles_it` | the switch's `A11yToggled` flips |
| Showcase segmented | `showcase_segmented_click_selects_the_option` | the clicked option becomes the accent selection |
| Showcase stepper | `showcase_stepper_plus_and_minus_clicks_change_the_count` | +/− change `ShowcaseStepper.count` |
| Showcase slider | `showcase_slider_keyboard_adjust_raises_the_value_and_preview` | ArrowRight on the focused slider raises `A11yValue` + the preview radius |
| Showcase disclosure | `showcase_disclosure_click_expands_and_collapses` | clicking the header toggles `A11yExpanded` + the body `Display` |

**Faithfulness, proven empirically.** Temporarily reverting the `emit_picks`
paint-skip guard makes **15 of the 16** Tier-A tests fail (every pointer-driven
one — the hidden modal absorbs the click); restoring it makes all 16 pass. The
sole pass with the fix reverted is the slider test, which is keyboard-driven and
never issues a pointer click. The nav-switch test alone is decisive: with the
revert, clicking the "Virtual List" rail button leaves `ScreenRouter` on `Todo`
(the dialog ate it).

### Tier B — picking-backend unit regression

`crates/buiy_core/tests/crosscut/picking_backend.rs`
`paint_skipped_overlay_never_absorbs_the_click_beneath_it`. A focused unit test:
an activatable target node with a **hidden, full-window, higher-paint-order**
overlay (`ComputedPaintSkip`) stacked on top of it. It asserts (1) the emitted
`PointerHits` is exactly the target — the paint-skipped overlay is neither a hit
nor an occluder — and (2) a full click still lowers to `OnPress` on the target.
Reverting the fix flips both assertions (the overlay becomes the sole pick and
occludes the target). This pins the invariant directly at the backend.

## Verification (full gate, all green)

- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` — clean.
- `cargo test --workspace --locked` (headless gate) — **1808 passed, 106 ignored,
  0 failed** (+17 over the prior baseline: the 16 Tier-A + the 1 Tier-B test).
- GPU lane `cargo test -p buiy_core --locked -- --ignored --test-threads=1` — **69
  passed, 0 failed** (the picking fix does not regress the render path).
- GPU lane `cargo test -p buiy_verify --locked -- --ignored --test-threads=1` —
  **22 passed, 0 failed**.
- `cargo deny check` — advisories / bans / licenses / sources all ok.

## Residual concerns / follow-ups

- **Menu-item pointer activation is a documented gap, not this bug.** A `MenuItem`
  has role `MenuItem` (not in `is_activatable_role`), and a per-item *pointer*
  activation handler is explicitly deferred (`buiy_widgets/src/menu.rs`
  `guard_menu_clicks` — "a future per-item pointer handler"). Menu items activate
  via the keyboard (the design's roving-focus model: open → active-descendant →
  Enter), which the Tier-A test exercises faithfully. Clicking a menu item with
  the mouse currently does nothing — a real but **pre-existing, documented**
  limitation, out of scope for the click-absorption fix. Worth a follow-up if
  pointer item-activation is desired.
- The Tier-A harness mounts the full 1000-row scroll screen (the real binary
  seeding), so each test app costs a one-time ~1.3 s spawn. Acceptable; if the
  suite grows, parameterizing the mount count would speed it up.
