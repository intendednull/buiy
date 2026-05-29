**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Per-widget enumeration: components, events, APG contract

# Widgets

Every widget shipped in `crates/bevy_ui_widgets/src/` on `main` @ 2026-05-22. Components, events, and keyboard contracts are read directly from source. **What is not in this list does not ship.** See [`apg-coverage.md`](apg-coverage.md) for the gap map.

## Button — `button.rs` (~140 lines)

```rust
#[derive(Component, Default, Debug, Clone)]
#[require(AccessibilityNode(accesskit::Node::new(Role::Button)))]
pub struct Button;

#[derive(Component, Default, Debug, Clone)]
pub struct ActivateOnPress;  // optional marker — fire on pointer-down (menu buttons)
```

- **State components consumed:** `Pressed`, `InteractionDisabled` (from `bevy_ui`).
- **Events emitted:** `Activate { entity }` on un-press (the un-press semantics matches the APG button pattern: activate on key-up, not key-down, so the user can drag off to cancel).
- **Keyboard contract:** `Enter` or `Space` while focused → `Activate`. Repeats ignored. `propagate(false)` on the focused-input event.
- **Pointer contract:** `Pointer<Press>` → insert `Pressed`; `Pointer<Release>` → remove `Pressed`; `Pointer<Click>` (between matched press/release) → `Activate` (if `ActivateOnPress` is absent); `Pointer<DragEnd>` / `Pointer<Cancel>` → remove `Pressed` without activation.
- **APG pattern:** [Button](https://www.w3.org/WAI/ARIA/apg/patterns/button/). Toggle-button variant (`aria-pressed`) is **not** built in — the Checkbox with `Role::Switch` override is the workaround.

## Checkbox — `checkbox.rs` (~280 lines)

```rust
#[derive(Component, Debug, Default, Clone)]
#[require(AccessibilityNode(accesskit::Node::new(Role::CheckBox)), Checkable)]
pub struct Checkbox;
```

- **State components consumed:** `Checked`, `Checkable`, `Pressed`, `InteractionDisabled` (all from `bevy_ui`).
- **Events emitted:** `ValueChange<bool> { source, value: !is_checked, is_final: true }` on activation (click or `Enter`/`Space`).
- **Commands accepted:** `SetChecked { entity, checked }`, `ToggleChecked { entity }` — apps trigger these to drive the checkbox from gamepad/scripts.
- **Keyboard contract:** `Enter` or `Space` → emit `ValueChange<bool>(!is_checked)`.
- **Toggle switch variant:** Documented in-source: *"If you are going to do a toggle switch, you should override the `AccessibilityNode` component with the `Switch` role instead of the `Checkbox` role."* No separate `Switch` widget exists.
- **Tri-state (`aria-checked="mixed"`):** **Not** modeled. `Checked` is a marker component, not an enum; there is no `Mixed` state. APG checkbox tri-state is a gap (see [`apg-coverage.md`](apg-coverage.md)).
- **Self-update observer:** `checkbox_self_update` — opt-in observer that mutates `Checked` for you so you don't have to write the controlled handler.
- **Focus side effect:** Pointer-press sets `InputFocus` to the checkbox and clears `InputFocusVisible` (the focus ring hides on mouse interaction).
- **APG pattern:** [Checkbox](https://www.w3.org/WAI/ARIA/apg/patterns/checkbox/). Tri-state variant ([dual-state APG](https://www.w3.org/WAI/ARIA/apg/patterns/checkbox/examples/checkbox-mixed/)) is the gap.

## Radio — `radio.rs` (~210 lines)

```rust
#[derive(Component, Debug, Clone, Default)]
#[require(AccessibilityNode(accesskit::Node::new(Role::RadioGroup)))]
pub struct RadioGroup;

#[derive(Component, Debug, Clone, Default, Reflect)]
#[require(AccessibilityNode(accesskit::Node::new(Role::RadioButton)), Checkable)]
#[reflect(Component)]
pub struct RadioButton;
```

- **State components consumed:** `Checked`, `Checkable`, `InteractionDisabled`.
- **Events emitted:** Per the lib.rs / radio.rs doc-comment: `ValueChange<Entity>` whose payload is the `Entity` id of the now-selected `RadioButton` (the group emits this when the keyboard navigates). Individual `RadioButton`s also emit `ValueChange<bool>` (`true`) when clicked — the design intent is that the *app* listens for either signal and updates the group's bound value.
- **Group is focusable, buttons are not** — per APG `radio` pattern. The keyboard observer attaches to the group; arrow keys navigate among descendants.
- **Keyboard contract on the group:** `ArrowUp` / `ArrowLeft` → previous (wraps); `ArrowDown` / `ArrowRight` → next (wraps); `Home` → first; `End` → last. Tab moves *out* of the group (per APG).
- **Group does not set `Checked` directly** — *"that is presumed to happen by the app or via some external data-binding scheme"* (radio.rs doc-comment). The pattern is: each button is associated with a Rust value; app sets `Checked` whenever the button's value equals the group's bound value.
- **APG pattern:** [Radio](https://www.w3.org/WAI/ARIA/apg/patterns/radio/).

## Slider — `slider.rs` (754 lines, the largest widget)

```rust
#[derive(Component, Debug, Default, Clone)]
#[require(
    AccessibilityNode(accesskit::Node::new(Role::Slider)),
    SliderDragState, SliderValue, SliderRange, SliderStep
)]
pub struct Slider {
    pub track_click: TrackClick,         // Drag | Step | Snap
    pub orientation: SliderOrientation,  // Auto | Horizontal | Vertical
}

#[derive(Component, Default, Clone)] pub struct SliderThumb;
#[derive(Component, Debug, Default, PartialEq, Clone, Copy)] #[component(immutable)]
pub struct SliderValue(pub f32);

#[derive(Component, Debug, PartialEq, Clone, Copy)] #[component(immutable)]
pub struct SliderRange { start: f32, end: f32 }      // default 0.0..=1.0

#[derive(Component)] pub struct SliderStep(pub f32);
#[derive(Component)] pub struct SliderPrecision(pub u32);  // decimal rounding during drag
#[derive(Component)] pub struct SliderDragState { ... }    // drag offsets
```

- **State components consumed:** `Pressed`, `InteractionDisabled`, `ComputedNode`, `UiGlobalTransform`, `UiScale` (for converting cursor pixels to slider value).
- **Events emitted:** `ValueChange<f32>` with `is_final: false` during drag, `is_final: true` on `DragEnd` or `Release`.
- **Commands accepted:** `SetSliderValue { entity, value }` for gamepad/script control.
- **Vertical support:** Added in [PR #21827](https://github.com/bevyengine/bevy/pull/21827) (DuckyBlender, merged 2025-12-09) for 0.18. `SliderOrientation::Auto` infers from `node.size().y > node.size().x`.
- **Track-click policy:** `TrackClick::Drag` (clicking a track point lets you drag from there), `TrackClick::Step` (clicking increments/decrements by `SliderStep`), `TrackClick::Snap` (clicking snaps the value to the clicked position).
- **Thumb is app-owned** — *"The core slider does not modify the visible position of the thumb: that is the responsibility of the stylist."* App reads `SliderValue` + `SliderRange::thumb_position` and writes the thumb's `Node::left`.
- **Keyboard contract:** ArrowUp/Right increments, ArrowDown/Left decrements (by `SliderStep`). Home/End set to min/max. **Note:** the source implements pointer drag + key handling; PageUp/PageDown larger-step is not explicitly handled.
- **APG pattern:** [Slider](https://www.w3.org/WAI/ARIA/apg/patterns/slider/) (single-thumb). Multi-thumb (`slider-multi-thumb`) is **not** built in.

## Scrollbar — `scrollbar.rs` (~520 lines)

```rust
#[derive(Component, FromTemplate, Reflect, Clone, PartialEq)]
pub struct Scrollbar {
    pub target: Entity,                 // the entity being scrolled
    pub orientation: ControlOrientation, // Horizontal | Vertical
    pub min_thumb_length: f32,
}

#[derive(Component, FromTemplate)]
pub struct ScrollbarThumb { pub border_radius: BorderRadius, pub border: UiRect }
```

- **Unusual posture, called out in source:** *"Scrollbars don't have an `AccessibilityNode` component, nor can they have keyboard focus. This is because scrollbars are usually used in conjunction with a scrollable container, which is itself accessible and focusable."* The scrollable container owns a11y + keyboard; the scrollbar is a pointer-only affordance.
- **Events emitted:** **None** — *"scrollbars don't emit notification events; instead they modify the scroll position of the target entity directly"* (i.e. mutate `bevy_ui::ScrollPosition` on the `target`).
- **Thumb sizing:** Auto-computed from `visible_size / content_size`, floored at `min_thumb_length`.
- **APG pattern:** No APG pattern for scrollbar — ARIA defines `role="scrollbar"` but per Bevy's choice the scrollbar is not exposed in the a11y tree.

## Menu — `menu.rs` (474 lines, since 0.18)

```rust
#[derive(Component, Default, Clone)]
#[require(AccessibilityNode(accesskit::Node::new(Role::MenuListPopup)), TabGroup::modal())]
#[require(MenuFocusState::Closed)]
pub struct MenuPopup { pub layout: MenuLayout }   // Column | Row | Grid

#[derive(Component, Default, Clone)]
#[require(AccessibilityNode(accesskit::Node::new(Role::MenuItem)))]
pub struct MenuItem;

#[derive(Component, Default, Clone)]
pub struct MenuButton;   // pairs with bevy_ui::widget::Button
```

- **Events emitted:** `Activate { entity }` from menu items; `MenuEvent { source, action: MenuAction }` bubbles up through the portal relation. `MenuAction = Open(NavAction) | Toggle | CloseAll | FocusRoot`.
- **Focus model:** Tightly coupled to `bevy_input_focus::InputFocus`. Menu remains open only as long as some child has focus — *"to detect clicks outside the popup box (which cause the menu to close), we look for focus changes"*. `MenuFocusState` is the lifecycle state machine: `Opening(NavAction) → Open → Closed`. `Opening` triggers a deferred focus set (because BSN spawn may be async; you can't set focus immediately on a not-yet-spawned child).
- **Keyboard contract (per `MenuLayout::Column`):** `ArrowUp` / `ArrowDown` → previous/next item; `Home` / `End` → first/last; `Enter` / `Space` on an item → `Activate` + `CloseAll` + `FocusRoot`; `Escape` on the popup → `CloseAll` + `FocusRoot`. For `Row`: `ArrowLeft` / `ArrowRight` swapped in. For `Grid`: keys not mapped — *"you'll need to write your own observer"*.
- **Submenus:** Not yet supported. The source has a TODO: *"Change this logic when we support submenus."*
- **`TabGroup::modal()`:** Tab navigation is trapped inside the open menu (modal tab group).
- **APG pattern:** [Menu and menubar](https://www.w3.org/WAI/ARIA/apg/patterns/menubar/). Menubar (horizontal menu of menus) is not a separate type; `MenuLayout::Row` is the closest construction.

## Popover — `popover.rs` (326 lines, since 0.18, the only `pub mod`)

```rust
pub enum PopoverSide { Top, Bottom, Left, Right }
pub enum PopoverAlign { Start, Center, End }
pub struct PopoverPlacement { side, align, gap }

#[derive(Component, PartialEq, Default)]
pub struct Popover {
    pub positions: Vec<PopoverPlacement>,
    pub window_margin: f32,
}
```

- **Not a widget — a positioning primitive.** Popover holds candidate placements; the `position_popover` system runs in `PostUpdate` after `ui_layout_system` and picks the first placement that fits inside the window (minus `window_margin`); falls back to "least bad" if none fit. Inspired by [Floating UI](https://floating-ui.com/).
- **No a11y role of its own** — apps attach popovers to menu popups, tooltips, etc. The wrapping widget supplies the role.
- **APG pattern:** Used by Menu; intended for Tooltip and others. Not a standalone APG pattern.

## EditableText (text input handler) — `text_input.rs` (529 lines, since 0.19)

Not a marker widget — it's the **input handler that drives `bevy_text::EditableText`**. Spawning an `EditableText` node from `bevy_text` gets you the data model; `EditableTextInputPlugin` from `bevy_ui_widgets` registers the observers that translate keyboard, IME, and pointer events into `TextEdit` actions.

- **Keyboard contract:** Full text-editing keymap with platform-aware modifiers (`Cmd` on macOS, `Ctrl` elsewhere; `Alt` for word-level on macOS, `Ctrl` for word-level elsewhere). Covers:
  - Character insertion (NONE/SHIFT + Character/Space)
  - Caret movement: arrows, Home/End (line), Cmd+Home/End or Cmd+Arrow (document), `Alt+Arrow` or `Ctrl+Arrow` (word).
  - Selection: SHIFT + any movement.
  - Editing: Backspace, Delete, Cmd+A (select all), Cmd+C/X/V (copy/cut/paste), `Shift+Delete` on non-macOS (cut), word-level Backspace/Delete via `WORD` modifier.
  - Escape collapses selection.
  - Enter inserts `\n` when `allow_newlines` is set, otherwise propagates (so submit handlers can run).
- **IME composition:** When `EditableText::is_composing()` is true, all keyboard input (including Tab) propagates to the IME and is suppressed from app-side handlers. `bevy_window::Ime` events feed the composition state.
- **Pointer contract:** Primary-button press → `MoveToPoint` (or `ShiftClickExtension` if Shift); double/triple click → word/line selection (`press.count`). Drag → `MoveToPointExtension`.
- **Filters:** App can attach an `EditableTextFilter` (see example `editable_text_filter.rs`) — content validation hook on incoming edits.
- **APG pattern:** [Textbox](https://www.w3.org/WAI/ARIA/apg/patterns/textbox/). Combobox + listbox bindings are app-side.

## Cross-widget events

```rust
#[derive(Copy, Clone, Debug, PartialEq, EntityEvent)]
pub struct Activate { pub entity: Entity }

#[derive(Copy, Clone, Debug, PartialEq, EntityEvent)]
pub struct ValueChange<T> {
    #[event_target] pub source: Entity,
    pub value: T,
    pub is_final: bool,   // false during drag, true on release
}
```

These two types are the entire crate-level event vocabulary. `is_final` distinguishes interim (drag in flight) from terminal (release) updates — supports both responsive UIs (update on every tick) and conservative apps (commit only on `is_final`).

## Sources

- All widget source files at `crates/bevy_ui_widgets/src/` on `main` @ 2026-05-22.
- WAI-ARIA APG patterns — https://www.w3.org/WAI/ARIA/apg/patterns/
- PR #21827 (vertical slider) — https://github.com/bevyengine/bevy/pull/21827
- Floating UI (popover positioning inspiration) — https://floating-ui.com/
- Sibling: [`api.md`](api.md), [`apg-coverage.md`](apg-coverage.md), [`open-problems.md`](open-problems.md)
