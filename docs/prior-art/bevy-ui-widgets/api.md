**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Component / event / observer API conventions

# API

How an app interacts with `bevy_ui_widgets`. Three surfaces: **components** (declare a widget), **events** (react to user actions), **observers** (custom interaction hooks).

## 1. Component API: declare a widget

Spawn the widget's marker on a `bevy_ui::Node` entity. The marker's `#[require(...)]` chain auto-inserts every companion component the widget needs.

```rust
use bevy::prelude::*;
use bevy_ui_widgets::{Button, Checkbox, Slider, SliderValue, SliderRange};

// Button — single marker, AccessibilityNode auto-inserted
commands.spawn((Node::default(), Button));

// Checkbox — marker auto-inserts AccessibilityNode + Checkable; you add Checked
commands.spawn((Node::default(), Checkbox, Checked));   // checked by default

// Slider — marker auto-inserts AccessibilityNode + SliderDragState + SliderValue
//          + SliderRange + SliderStep. Override range explicitly:
commands.spawn((
    Node::default(),
    Slider::default(),
    SliderValue(0.5),
    SliderRange::from_range(0.0..=1.0),
    SliderStep(0.05),
)).with_children(|p| {
    p.spawn((Node::default(), SliderThumb));   // identifies which child is the thumb
});
```

Conventions across the crate:

- **Markers are zero-sized or near-zero-sized** (`Button`, `Checkbox`, `RadioGroup`, `RadioButton`, `MenuItem`, `MenuButton`, `SliderThumb`, `ScrollbarThumb`). `Slider` and `Scrollbar` carry small config (orientation, track-click policy, target entity).
- **State lives in separate components** (`Checked`, `Pressed`, `Hovered`, `InteractionDisabled`, `SliderValue`, `SliderRange`, `SliderStep`, `SliderPrecision`, `SliderDragState`, `ScrollbarDragState`). Most are `#[component(immutable)]` or `Copy + Default` to keep change-detection cheap.
- **Required-component chains do the wiring.** `Slider`'s `#[require(...)]` pulls in AccessibilityNode + state. Removing the marker does not remove the companions — they're independent. Apps that need only the state (e.g. a custom non-headless slider with the same value semantics) can spawn `SliderValue` + `SliderRange` directly.
- **No two-way bindings.** A `bsn!` template like `Slider [ value: 0.5 ]` (when BSN lands) would set the initial `SliderValue(0.5)`, but nothing thereafter writes `SliderValue` automatically. The app's observer for `ValueChange<f32>` updates it.

## 2. Event API: react to user actions

Two crate-level events:

```rust
// Activation (button click, menu item activation)
#[derive(EntityEvent)] pub struct Activate { pub entity: Entity }

// Value edit (slider drag, checkbox toggle, radio select, text input)
#[derive(EntityEvent)] pub struct ValueChange<T> {
    #[event_target] pub source: Entity,
    pub value: T,
    pub is_final: bool,
}
```

Plus widget-specific events:

```rust
// Drive a checkbox from outside the pointer/key path:
pub struct SetChecked { pub entity: Entity, pub checked: bool }
pub struct ToggleChecked { pub entity: Entity }

// Drive a slider:
pub struct SetSliderValue { pub entity: Entity, pub value: f32 }

// Menu lifecycle (bubbles via portal relation):
pub struct MenuEvent { #[event_target] pub source: Entity, pub action: MenuAction }
pub enum MenuAction { Open(NavAction), Toggle, CloseAll, FocusRoot }
```

Subscribe via `commands.entity(w).observe(...)` or `app.add_observer(...)`:

```rust
commands.entity(button_entity).observe(
    |trigger: On<Activate>, mut counter: ResMut<Counter>| {
        counter.0 += 1;
    },
);

commands.entity(slider_entity).observe(
    |trigger: On<ValueChange<f32>>, mut commands: Commands| {
        // Update the controlled value:
        commands.entity(trigger.source)
            .insert(SliderValue(trigger.value));
    },
);
```

**The `is_final` discipline.** Sliders fire `ValueChange<f32>` on every drag tick with `is_final: false`, then a final one with `is_final: true` on release. Apps doing expensive work (e.g. recomputing a layout) check `is_final`; UIs with live preview ignore it.

## 3. Observer API: custom interaction hooks

The crate exposes one helper for the BSN-style bundle pattern:

```rust
// from observe.rs
pub fn observe<E: EntityEvent, B: Bundle, M, I: IntoObserverSystem<E, B, M>>(
    observer: I,
) -> AddObserver<E, B, M, I>
```

`observe(...)` returns an `AddObserver` bundle that, when inserted as part of a `spawn(...)` tuple, attaches the observer to the spawned entity. This lets you co-locate the observer with the spawn site:

```rust
commands.spawn((
    Node::default(),
    Button,
    observe(|_: On<Activate>, mut counter: ResMut<Counter>| { counter.0 += 1; }),
));
```

The source comment notes: *"This probably doesn't belong in bevy_ui_widgets, but I am not sure where it should go. It is certainly a useful thing to have."* The helper relies on `unsafe` for the empty-bundle storage trick. **Cross-cutting infrastructure that landed in the widget crate by accident** — likely to migrate (see [`open-problems.md`](open-problems.md)).

## 4. The self-update convenience: `checkbox_self_update`

Most widgets do not auto-update their state. Checkbox is the one exception — `checkbox_self_update` is shipped as an **opt-in** observer:

```rust
use bevy_ui_widgets::checkbox_self_update;

commands.spawn((Checkbox, observe(checkbox_self_update)));
// Now clicking the checkbox automatically inserts/removes Checked.
```

This is the only batteries-included "uncontrolled" mode in the crate. The deliberate choice: most widgets want app-side state (game-state mirror, undo stack, network sync), but checkbox is the most common case where the local toggle is the whole story.

## 5. Building a custom-styled widget on top

The canonical recipe (from `examples/ui/widgets/standard_widgets.rs`):

```rust
// 1. Marker for your visual style
#[derive(Component)] struct DemoButton;

// 2. Spawn the headless + visual companions
fn spawn_button(commands: &mut Commands) {
    commands.spawn((
        Node { padding: UiRect::all(Val::Px(10.0)), .. },
        BackgroundColor(NORMAL_BUTTON),
        Button,                       // <-- bevy_ui_widgets
        DemoButton,                   // your style tag
        children![ Text::new("Click me") ],
    ));
}

// 3. Polling system: read state components, update visuals
fn update_button_style(
    mut q: Query<
        (&mut BackgroundColor, Has<Pressed>, Has<Hovered>, Has<InteractionDisabled>),
        With<DemoButton>,
    >,
) {
    for (mut bg, pressed, hovered, disabled) in &mut q {
        *bg = BackgroundColor(match (disabled, pressed, hovered) {
            (true, _, _)     => DISABLED_BUTTON,
            (_, true, _)     => PRESSED_BUTTON,
            (_, _, true)     => HOVERED_BUTTON,
            _                => NORMAL_BUTTON,
        });
    }
}
```

The shape:

- **Visual decoration is app-side polling on state components**, not a widget API. There is no `Button::on_hover()` callback — the app queries `Hovered` from `bevy_picking` and reacts.
- The headless crate has **no opinion** on whether you use `BackgroundColor`, a `UiMaterial`, or a Feathers-style theme token. It exposes state; you read state.
- **Bevy's change-detection** does the optimization — the polling system only writes when state changes (in practice; the example above writes on every frame for clarity).

## 6. Reflection + BSN posture

- Most widget components derive `Reflect + Component` and call `app.register_type::<T>()` in their plugin's `build`. `RadioButton`, `Slider`'s state types, `Scrollbar`, and `ScrollbarThumb` are reflected.
- The marker components themselves (`Button`, `Checkbox`, `MenuItem`, etc.) are often **not reflected** — they're zero-sized and BSN-spawnable via the type registry implicitly.
- [PR #23924](https://github.com/bevyengine/bevy/pull/23924) (fallible-algebra, merged 2026-04-22) added `FromTemplate` to "most" bevy_ui_widgets components — the bridge to BSN-style templating. Coverage is partial, not universal; the crate is still in motion w.r.t. BSN.

## 7. The "compose your own renderer" promise

The promise from the discussion #16900 design and Bevy 0.17 announcement: bevy_ui_widgets provides interaction logic; *anyone* can build a styled widget kit (Feathers, third-party, your own) on top by composing the same state components + observers. Cosmonaut [viridia's framing](https://github.com/bevyengine/bevy/discussions/16900): *"headless UI component libraries provide high-quality widget implementations with no built-in styling."*

In practice, the promise holds with two caveats:

1. **The visual side is much more work than the headless side suggests.** A Feathers-style button is the headless Button plus dozens of lines of node tree, theme-token plumbing, and polling systems. The headless crate gives you ~15% of a usable widget.
2. **`bevy_ui::*` is a hard dependency.** Every widget assumes `ComputedNode`, `UiTransform`, `UiGlobalTransform`, `BackgroundColor`, `BorderColor`, `Node`, `ScrollPosition`. A non-`bevy_ui` UI stack (Buiy, egui, iced) cannot reuse the observers as-is. The widget crate is "headless w.r.t. styling, not headless w.r.t. layout engine."

## Implications for Buiy

- The two-event vocabulary (`Activate`, `ValueChange<T>`) is **clean and worth borrowing** — Buiy's widget catalog spec ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) should adopt the same shape rather than per-widget bespoke event types.
- `is_final` as a single flag on `ValueChange<T>` is a clean way to model the "interim vs commit" distinction without per-widget `DragStart` / `DragEnd` / `Change` ceremony.
- `observe(...)` as a bundle-effect for declarative observer attachment is the BSN-friendly idiom and worth borrowing for Buiy's own widget API; the "doesn't belong here" source comment suggests the upstream may eventually move it to `bevy_ecs` — Buiy can put its analog in `buiy_core`.
- **Do not borrow** the hard `bevy_ui::*` dependency posture. Buiy's headless widgets must be parameterized over its own `buiy::Node` / `buiy::ComputedNode` types so they integrate with Buiy's render + layout pipeline, not Bevy's. This is the load-bearing reason Buiy ships its own widget catalog rather than reusing `bevy_ui_widgets`.

## Sources

- `crates/bevy_ui_widgets/src/lib.rs`, `observe.rs`, per-widget source files (@ main, 2026-05-22)
- `examples/ui/widgets/standard_widgets.rs` (canonical custom-styling recipe)
- PR #23924 (`FromTemplate` derive) — https://github.com/bevyengine/bevy/pull/23924
- Discussion #16900 — https://github.com/bevyengine/bevy/discussions/16900
- Sibling: [`widgets.md`](widgets.md), [`architecture.md`](architecture.md), [`lessons.md`](lessons.md)
- Buiy foundation — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
