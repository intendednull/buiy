**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Integration, plugins, coexistence

# Integration

## Adding the crate

```toml
# Cargo.toml
[dependencies]
bevy = "0.18.1"                  # bevy_ui_widgets re-exported as bevy::ui_widgets
# OR direct, decoupled from the meta-crate:
bevy_ui_widgets = "0.18.1"
```

```rust
use bevy::prelude::*;
use bevy::ui_widgets::UiWidgetsPlugins;
// or: use bevy_ui_widgets::UiWidgetsPlugins;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,             // includes bevy_ui as a sub-plugin
            UiWidgetsPlugins,           // registers every widget's observers
        ))
        .run();
}
```

For finer-grained opt-in, add individual widget plugins:

```rust
use bevy_ui_widgets::{ButtonPlugin, CheckboxPlugin, SliderPlugin};

App::new()
    .add_plugins((DefaultPlugins, ButtonPlugin, CheckboxPlugin, SliderPlugin))
    .run();
```

The plugin list, from `lib.rs` on `main` @ 2026-05-22:

```
PopoverPlugin                    — popover positioning system (PostUpdate)
ButtonPlugin                     — Button observers
CheckboxPlugin                   — Checkbox observers
MenuPlugin                       — Menu observers + Update-schedule lifecycle systems
RadioGroupPlugin                 — Radio observers
ScrollbarPlugin                  — Scrollbar drag observers + PostUpdate thumb update
SliderPlugin                     — Slider observers + PostUpdate visual sync
EditableTextInputPlugin          — text_input observers (PreUpdate keyboard, PostUpdate scroll/layout)
```

## Cargo features

```toml
# Cargo.toml on main
[features]
default = []
```

**`bevy_ui_widgets` has no cargo features as of 0.19.0-rc.2.** All widgets are always compiled in; the unit of opt-in is the plugin, not a feature flag. (Prior to 0.18, an `experimental` feature flag existed and gated the crate from the `bevy` meta-crate's default features. [PR #22934](https://github.com/bevyengine/bevy/pull/22934), alice-i-cecile, merged 2026-02-18, removed it for 0.18 — the crate is now included by default in `bevy::ui_widgets`. The source-level `## Warning: Experimental` doc-comment remains.)

## Required substrate plugins

`UiWidgetsPlugins` does NOT pull in its dependencies. The app must add:

- `bevy_app::AppExit` and other Bevy core (auto in `DefaultPlugins`).
- `bevy_ui::UiPlugin` (auto in `DefaultPlugins`).
- `bevy_picking::DefaultPickingPlugins` (auto in `DefaultPlugins`) — `Pointer<*>` events come from here.
- `bevy_input_focus::InputFocusPlugin` + `bevy_input_focus::tab_navigation::TabNavigationPlugin` — required for menu / radio / text-input keyboard. **`TabNavigationPlugin` is NOT auto-added by `DefaultPlugins`.** The widget examples explicitly add it (`.add_plugins((DefaultPlugins, TabNavigationPlugin))`).
- `bevy_a11y::AccessibilityPlugin` (auto in `DefaultPlugins`) — `AccessibilityNode` is wired here.
- `bevy_text::TextPlugin` (auto in `DefaultPlugins`) for `text_input`.

## Coexistence patterns

### 1. With `bevy_feathers` (the canonical pairing)

```toml
bevy = "0.18.1"
bevy_feathers = "0.18.1"
```

```rust
use bevy::prelude::*;
use bevy_feathers::FeathersPlugin;
use bevy_ui_widgets::UiWidgetsPlugins;

App::new()
    .add_plugins((DefaultPlugins, UiWidgetsPlugins, FeathersPlugin))
    .run();
```

Feathers spawns `bevy_ui_widgets::Button`, `Checkbox`, etc. as the brain of its styled widgets. Both plugins must be added — Feathers does not re-export `UiWidgetsPlugins`. The flagship integration target is **the in-development Bevy editor**, which is the primary user of Feathers + bevy_ui_widgets together.

### 2. With custom (app-side) styled widgets

The canonical "use headless without Feathers" path. The app:

- Adds `UiWidgetsPlugins`.
- Spawns its own `Node` tree per widget with marker components from `bevy_ui_widgets`.
- Writes its own `update_*_style` polling systems that read `Pressed` / `Hovered` / `Checked` / `InteractionDisabled` / `SliderValue` / etc. and update `BackgroundColor` / `BorderColor` / `Node` positions / custom `UiMaterial` properties.
- Writes its own observers for `Activate` and `ValueChange<T>` to drive app state.

Example: `examples/ui/widgets/standard_widgets.rs` is exactly this pattern.

### 3. With third-party widget kits (`sickle_ui`, `woodpecker_ui`, `bevy_lunex`)

Third-party kits historically pre-date bevy_ui_widgets (`sickle_ui` shipped its own widget catalog before 0.17). Migration to bevy_ui_widgets as the headless brain is an open question per third-party kit; see [`ecosystem.md`](ecosystem.md) for the matrix. The technical possibility is identical to "custom app-side styled widgets" — those kits can replace their own state-machines with `bevy_ui_widgets` markers + observers and keep their own visual layer.

### 4. With non-Bevy UI in the same window

Not supported by bevy_ui_widgets — all widgets hard-depend on `bevy_ui::*` components. A window owned by a non-bevy_ui UI stack (Buiy, egui, iced, native) cannot host bevy_ui_widgets widgets.

### 5. With Buiy (the parallel-stack)

Per Buiy's [foundation cross-cutting](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) (§ 3.18, *Coexistence with bevy_ui*) the rule is **per-window**, not per-app-shared-window. In an app that wants both:

- A Bevy `Window` displaying bevy_ui can host `bevy_ui_widgets` widgets. `UiWidgetsPlugins` is registered globally; only entities with `bevy_ui::Node` and `Camera` targeting that window receive widget behavior.
- A separate `Window` displaying Buiy hosts `buiy_widgets` widgets. Buiy's own widget catalog runs against `buiy::Node`.
- The AccessKit adapter is per-window. Buiy owns the adapter on Buiy windows ([foundation architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)); bevy_a11y owns the adapter on bevy_ui windows. `UiWidgetsPlugins`'s widgets reach AccessKit via `bevy_a11y::AccessibilityNode` — which only works on bevy_a11y-owned windows.
- The same physical window cannot host both UI stacks simultaneously. (Buiy explicitly does not support sharing a window with bevy_ui.)
- `bevy_picking` backends and render-graph nodes run in parallel; the picking-backend priority on a pointer can be configured per backend, but Buiy's spec says picking-backend priority and render-graph ordering versus `bevy_ui`'s own passes is defined per-window and Buiy's passes do not contractually cooperate with bevy_ui's.

### 6. Headless-only (no Feathers, no styling)

Spawning `Button` alone (without a `BackgroundColor`, without a child `Text`, etc.) gets you a functional but invisible button. The widget will fire `Activate` on a configured click, but the user has no way to see it. This is useful for:

- Tests / fixtures that exercise interaction without rendering.
- Programmatic-only invocation paths (e.g. a button driven by a gamepad action map, never displayed).
- Headless layout / a11y harnesses.

## Bevy version compatibility

bevy_ui_widgets is **lockstep with the Bevy minor**. Each Bevy minor publishes a matching `0.X.Y` of `bevy_ui_widgets`. You cannot mix Bevy 0.18 with bevy_ui_widgets 0.17 — the internal `bevy_ui::Pressed` / `Checked` / `Checkable` ABI has churned across minors. See [`distribution.md`](distribution.md) for the release table.

## Implications for Buiy

- Buiy does **not** ship `bevy_ui_widgets` as a dependency. Buiy's widget catalog is its own crate (`buiy_widgets`), composed of equivalent markers + observers parameterized over `buiy::Node`, not `bevy_ui::Node`. A Buiy app can still add `UiWidgetsPlugins` in the same `App` if it has windows displaying `bevy_ui`; the two crates won't interfere because their widgets attach to disjoint entity sets.
- The "per-window coexistence" pattern (Buiy in window A, bevy_ui+bevy_ui_widgets+Feathers in window B) is the load-bearing migration path for apps moving from bevy_ui to Buiy. Buiy must keep this story working end-to-end: the AccessKit-adapter-per-window key (winit `WindowId`, not Bevy `Entity`) is the lynchpin.
- `TabNavigationPlugin` is not in `DefaultPlugins` — Buiy can do better by including its sequential + spatial focus model in `BuiyPlugin` by default (per [foundation architecture.md § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md), `BuiyPlugin` adds sub-plugins in `core → theme → a11y → focus → input → text → widgets → animation → forms → devtools` order).

## Sources

- `crates/bevy_ui_widgets/Cargo.toml`, `src/lib.rs` (@ main, 2026-05-22)
- `examples/ui/widgets/standard_widgets.rs`, `standard_widgets_observers.rs`, `feathers_gallery.rs`
- PR #22934 (remove `experimental` feature flag) — https://github.com/bevyengine/bevy/pull/22934
- Buiy cross-cutting (per-window coexistence) — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- Sibling: [`architecture.md`](architecture.md), [`distribution.md`](distribution.md), [`ecosystem.md`](ecosystem.md)
