**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Architecture: the headless widget primitive pattern

# Architecture

`bevy_ui_widgets`'s defining decision is that **a widget is not a renderable element. A widget is an interaction protocol expressed in components + observers.** The crate ships no rendering, no styling tokens, no theme — only the ECS shapes that make a widget interactive. Apps and styled-widget kits (notably `bevy_feathers`) compose visuals on top.

## The headless primitive pattern

Each widget conforms to a recurring shape:

1. **A marker component** identifies the widget (`Button`, `Checkbox`, `Slider`, `RadioGroup`, `RadioButton`, `MenuPopup`, `MenuItem`, `MenuButton`, `Scrollbar`).
2. The marker uses Bevy's `#[require(...)]` to pull in **default-constructed companion components** for accessibility (`AccessibilityNode` with the correct `accesskit::Role`), interaction state (`Checkable`, `Pressed`, `Checked`, `InteractionDisabled` — these live in `bevy_ui`, not in `bevy_ui_widgets`), and per-widget state (e.g. `Slider` requires `SliderDragState + SliderValue + SliderRange + SliderStep`).
3. **A `Plugin` per widget** (`ButtonPlugin`, `CheckboxPlugin`, …) registers **observers** that respond to:
   - `FocusedInput<KeyboardInput>` — keyboard activation per the APG keyboard contract.
   - `Pointer<Press>`, `Pointer<Release>`, `Pointer<Click>`, `Pointer<DragEnd>`, `Pointer<Cancel>` — pointer state-machine transitions via `bevy_picking`.
4. The observers **mutate Bevy state components** (`insert(Pressed)`, `remove::<Pressed>()`, etc.) and **emit `EntityEvent`s** — primarily `Activate { entity }` (button/menu activation) and `ValueChange<T> { source, value, is_final }` (anything that edits a scalar/bool/entity-id).
5. **No widget owns its own value/checked/etc. state.** The lib.rs is explicit: *"the widgets do not automatically update their own internal state, but instead rely on the app to update the widget state (as well as any other related game state) in response to a change event emitted by the widget. The primary motivation for this is to avoid two-way data binding in scenarios where the user interface is showing a live view of dynamic data coming from deeper within the game engine."*

The Checkbox provides a sharp illustration: clicking the checkbox does **not** toggle the `Checked` component. It triggers `ValueChange<bool> { source, value: !is_checked, is_final: true }`. The app then either runs the app-supplied event handler that flips `Checked`, or (for convenience) registers `checkbox_self_update` — an observer the user opts into to get the conventional auto-toggle behavior. This is the "controlled component" pattern from React, ported to ECS.

## Plugin shape

```rust
// from crates/bevy_ui_widgets/src/lib.rs
#[derive(Default)]
pub struct UiWidgetsPlugins;

impl PluginGroup for UiWidgetsPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PopoverPlugin)
            .add(ButtonPlugin)
            .add(CheckboxPlugin)
            .add(MenuPlugin)
            .add(RadioGroupPlugin)
            .add(ScrollbarPlugin)
            .add(SliderPlugin)
            .add(EditableTextInputPlugin)
    }
}
```

The unit of opt-in is a single widget. An app that only needs Button + Slider can `app.add_plugins((ButtonPlugin, SliderPlugin))` and skip the rest. Note the name: **`UiWidgetsPlugins` (plural)** — it is a `PluginGroup`, not a single `Plugin`. (The brief's "`UiWidgetsPlugin`?" placeholder was correct to flag the uncertainty.)

## System ordering

`bevy_ui_widgets` does not define new `SystemSet`s. It registers:

- **Observers** (event-driven, run synchronously when the event fires) for keyboard, pointer, and state events on widget entities.
- **`Update`-schedule systems** for the few cases that need polled state: `menu_acquire_focus`, `menu_on_lose_focus` (menu lifecycle gated on `InputFocus` changes).
- **`PostUpdate`-schedule systems** that run alongside `bevy_ui`'s layout: `position_popover` runs in `PostUpdate` ordered against `UiSystems::Layout` so popover positions resolve after their anchor's layout is final; `Scrollbar::update_scrollbar_thumb` likewise.
- **`PreUpdate`-schedule systems** for `text_input`: `on_focused_keyboard_input` runs after `InputSystems` and before `AccessibilitySystems` to convert key events into queued `TextEdit`s on `EditableText`.

The relevant Bevy substrate sets used are `UiSystems::Layout`, `InputSystems`, `InputFocusSystems`, `AccessibilitySystems` — all defined in upstream crates, not in `bevy_ui_widgets`.

## Substrate dependencies

```
bevy_ui_widgets
├── bevy_app, bevy_ecs              — plugin + observer scaffolding
├── bevy_a11y                       — AccessibilityNode + accesskit::Role per widget
├── accesskit 0.24                  — accessibility-tree types
├── bevy_picking                    — Pointer<Press/Release/Click/DragStart/Drag/DragEnd/Cancel> events
├── bevy_input + bevy_input_focus   — KeyboardInput, FocusedInput, InputFocus, tab_navigation
├── bevy_ui                         — Checkable, Checked, Pressed, InteractionDisabled,
│                                     ComputedNode, UiTransform, Node, UiSystems, BorderRadius,
│                                     ScrollPosition, FocusPolicy, widget::{Button, scroll_editable_text}
├── bevy_text                       — EditableText, PreeditCursor, TextEdit (for text_input.rs)
├── bevy_window                     — Ime, PrimaryWindow (for IME composition)
├── bevy_camera                     — Visibility (for scrollbar visibility gating)
├── bevy_math                       — Vec2, Rect, Affine2 (for popover placement)
├── bevy_reflect                    — Reflect derive on a subset of components
├── bevy_log                        — warn_once, warn (for misuse diagnostics)
├── parley 0.9.0                    — present in Cargo.toml but only as a transitive shape via bevy_text; the actual shaper used by 0.19 main
└── smol_str 0.2                    — small-string optimization in events
```

`bevy_ui_widgets` is a **pure consumer** of the substrate. It introduces no new render passes, no new picking backend, no new accessibility plumbing — it merely **composes** the existing pieces into reusable interaction protocols.

## Relationship to `bevy_ui` and `bevy_feathers`

Three crates, three concerns:

| Crate | Owns | Touches widget behavior? |
|---|---|---|
| **`bevy_ui`** | Layout (Taffy), render passes (rect, image, text, gradient, shadow, border, scroll), picking backend, interaction-state primitive components (`Pressed`, `Checked`, `Checkable`, `InteractionDisabled`, `FocusPolicy`). | No — provides the substrate components, not interaction observers. (Exception: `bevy_ui::widget` ships `Button` as a tiny historical marker, plus the editable-text-layout systems consumed by `bevy_ui_widgets::text_input`.) |
| **`bevy_ui_widgets`** | Marker components, observers, events that compose the substrate into APG-shaped widgets. | Yes — this is the interaction protocol. |
| **`bevy_feathers`** | Styled visuals: tokens, fonts, atlas icons, themed materials, widget builders that compose `bevy_ui_widgets` + a `bevy_ui` visual tree. | Indirectly — Feathers spawns `Button`, `Checkbox`, etc. from `bevy_ui_widgets` as the brain, then attaches its own visuals as children. |

The layering is **strict bottom-up**. `bevy_ui_widgets` depends on `bevy_ui`; `bevy_feathers` depends on `bevy_ui_widgets`. Neither `bevy_ui` nor `bevy_ui_widgets` depend on `bevy_feathers`. An app can use `bevy_ui_widgets` without Feathers (Buiy can use neither). See [`integration.md`](integration.md) for the coexistence matrix.

## How custom styling composes

The example `examples/ui/widgets/standard_widgets.rs` shows the canonical pattern:

1. Spawn a `Node` hierarchy that gives the widget its visual layout (background `Node`, child track `Node`, child thumb `Node` with `SliderThumb`, etc.).
2. On the root, add the headless marker (`Slider { ... }`) — this auto-inserts `AccessibilityNode(accesskit::Node::new(Role::Slider))`, `SliderDragState`, `SliderValue`, `SliderRange`, `SliderStep` via the `#[require(...)]` chain.
3. Register an **observer on the entity** for `ValueChange<f32>` that updates `SliderValue` (the "controlled" handler) and any app-side mirror state.
4. Register a **polling system** that reads `SliderValue` and updates the thumb's `Node::left` / `Node::top` to reflect the value.
5. Optionally read `Hovered`, `Pressed`, `InteractionDisabled`, `Checked` components in another polling system to swap `BackgroundColor` / `BorderColor` for visual feedback.

The split is clean: **state lives in components, behavior lives in observers, visuals are app-side polling on those components.** No abstraction owns the rendering. The visual decoration is entirely outside the headless crate's vocabulary.

## Implications for Buiy

- Buiy's own widget architecture is the same shape — marker components, decomposed state, observer-driven events, no rendering coupling — but **Buiy owns the renderer too**. The `bevy_ui_widgets` pattern is the protocol layer; Buiy's `buiy_widgets` crate plays the same role *and* its own `BuiyMaterial` render pipeline plays the role `bevy_ui`/`bevy_feathers` play in the Bevy stack. See [`lessons.md`](lessons.md) for what to borrow.
- The "external state management" stance maps directly onto Buiy's "observers + change detection only, no signals" reactivity ([architecture.md § 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md)).
- Buiy registers its own `bevy_picking` backend in parallel to `bevy_ui`'s (per [foundation cross-cutting](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)). `bevy_ui_widgets`'s `Pointer<*>` observer wiring runs against whatever backend is registered; on Buiy-owned windows it would route through Buiy's backend. The widgets themselves would be invoked on `bevy_ui` entities — they have hard `bevy_ui::*` dependencies (`ComputedNode`, `UiTransform`, etc.) — and cannot be lifted onto `buiy::Node` without rewriting the observers. This is why Buiy ships its own widget catalog rather than reusing `bevy_ui_widgets`.

## Sources

- `crates/bevy_ui_widgets/src/lib.rs` (@ main, 2026-05-22) — https://raw.githubusercontent.com/bevyengine/bevy/main/crates/bevy_ui_widgets/src/lib.rs
- `crates/bevy_ui_widgets/src/button.rs`, `checkbox.rs`, `slider.rs`, `radio.rs`, `scrollbar.rs`, `menu.rs`, `popover.rs`, `text_input.rs` — same path prefix.
- `examples/ui/widgets/standard_widgets.rs` — https://raw.githubusercontent.com/bevyengine/bevy/main/examples/ui/widgets/standard_widgets.rs
- Discussion #16900 — https://github.com/bevyengine/bevy/discussions/16900
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling prior-art: [`../bevy-ui/architecture.md`](../bevy-ui/architecture.md), [`../bevy-feathers/`](../bevy-feathers/), [`../bevy-picking/`](../bevy-picking/), [`../bevy-a11y/`](../bevy-a11y/)
