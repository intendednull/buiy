**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — user-facing API style, widget vocabulary, composition, state

# API

woodpecker_ui's authoring API is **derive-macro-driven + ECS-component-typed**. A widget is a Bevy `Component` decorated with `#[derive(Widget)]` and a small set of attributes that declare its render function and reactive inputs. Children are composed by a fluent builder (`WidgetChildren`), state is plumbed by a React-style `use_state` hook, and styling is a single ~50-field `WoodpeckerStyle` component. The shape is *kayak_ui-flavored* but cleaner: the README's Q3 explicitly frames woodpecker as "what Kayak made great, with the backend much, much simpler" (see [`history.md`](history.md)).

## The `Widget` derive

```rust
#[derive(Widget, Component, Reflect, PartialEq, Default, Debug, Clone)]
#[auto_update(render)]
#[props(CounterWidget)]
#[state(CounterState)]
#[require(WoodpeckerStyle, WidgetChildren)]
pub struct CounterWidget {
    initial_count: u32,
}
```

The proc-macro lives in the `woodpecker_ui_macros` workspace sub-crate (`crates/woodpecker_ui_macros/`). Attributes it understands (verified from `crates/woodpecker_ui_macros/src/lib.rs`):

- **`#[auto_update(render)]`** — generate the `update` function automatically by diffing the declared `props` / `state` / `context` / `resource`. Pair with `#[props(...)]`, `#[state(...)]`, `#[context(...)]`, `#[resource(...)]`.
- **`#[widget_systems(update, render)]`** — manual mode; you write `fn update() -> bool` and `fn render(...)` yourself.
- **`#[props(ComponentA, ComponentB, ...)]`** — when these components on the widget entity change, re-render. The marker component itself is conventionally listed first.
- **`#[state(StateComponentA, ...)]`** — declares state entities tracked via `HookHelper`.
- **`#[context(ContextA, ...)]`** — declares nearest-ancestor context lookups.
- **`#[resource(ResourceA, ...)]`** — declares global Bevy resources to track.
- **`#[require(...)]`** — Bevy's standard `#[require(...)]` on `Component` (see [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Borrow #1) — auto-inserts companions like `WoodpeckerStyle` and `WidgetChildren` when the widget is spawned.

User then registers the widget on the app:

```rust
app.register_widget::<CounterWidget>()
```

This calls `register_component_as::<dyn Widget, T>()` (the `bevy-trait-query` integration) and registers the auto-generated `update` + `render` systems with `WoodpeckerContext`.

## Render function shape

```rust
fn render(
    current_widget: Res<CurrentWidget>,
    mut commands: Commands,
    mut query: Query<(&CounterWidget, &mut WidgetChildren)>,
    state_query: Query<&CounterState>,
    mut hooks: ResMut<HookHelper>,
) {
    let Ok((widget, mut children)) = query.get_mut(**current_widget) else { return; };
    let state_entity = hooks.use_state(&mut commands, *current_widget, CounterState::default());
    let Ok(state) = state_query.get(state_entity) else { return; };

    *children = WidgetChildren::default()
        .with_child::<Element>((/* ... */))
        .with_child::<WButton>((/* ... */, WidgetChildren::default().with_child::<Element>(/* label */)))
        .with_observe(current_widget, move |_: Trigger<Pointer<Click>>, /* ... */| { /* ... */ });

    children.apply(current_widget.as_parent());
}
```

The function reads props from a `Query<&Self>`, calls hooks for state, computes the new child tree, and applies it. Bevy `Trigger` observers handle interaction events via `.with_observe(entity, system)`.

`CurrentWidget` and `ParentWidget` are `Resource`-wrapped `Entity` newtypes that the runner injects into widget systems via Bevy's `Res<...>` extraction. Each widget invocation runs as a one-shot system with the current widget entity in scope.

## Hooks

`HookHelper` is a `ResMut` resource with the canonical React-style API:

- **`use_state<T: Component>(commands, current_widget, initial)` → `Entity`** — creates a state entity on first call, returns the same entity on re-render. Queryable like any normal component.
- **`use_context<T: Component>(...)`** — nearest-ancestor lookup (declared via `#[context(...)]`).
- **`use_prev_resource<T: Resource>(...)`** — read the previous frame's resource value (via the `PreviousResource<T>` wrapper).

Internally, hook state is keyed off `(parent_widget_entity, hook_index)`; `PreviousWidget` tracks the mapping across re-renders so state survives.

This is the *only* reactive primitive offered. There are no signals, no computed graphs, no effects. State changes drive re-render via the standard `update(...) -> bool` diff.

## Composition: `WidgetChildren`

```rust
WidgetChildren::default()
    .with_child::<MyWidget>((MyWidget { /* props */ }, /* required components */))
    .with_child::<Element>((Element, WoodpeckerStyle { /* ... */ }, WidgetRender::Text { /* ... */ }))
    .with_observe(parent_entity, observer_system);
```

`WidgetChildren` is a `Component` on the parent. Each `with_child::<T>(bundle)` appends a child specification typed by `T`. `apply(parent)` reconciles: spawns new children, updates existing matches, despawns removed.

A `Mounted` marker fires once when an entity is first inserted; `PassedChildren` lets a widget forward its children slot to a descendant (slot-projection analogue).

`Element` is the generic-container widget — equivalent to HTML `<div>` — with a `WoodpeckerStyle` and optional `WidgetRender` leaf.

## Style component

`WoodpeckerStyle` is a single ~50-field `#[derive(Component, Reflect)]` struct (verified from `src/styles/mod.rs`). Field categories:

- **Layout (Taffy):** `display`, `position`, `flex_direction`, `flex_wrap`, `flex_grow`, `flex_shrink`, `flex_basis`, `gap`, `align_items`, `align_content`, `align_self`, `justify_content`, `justify_self`, `justify_items`, `padding`, `margin`, `width`, `height`, `min_width`/`max_width`/`min_height`/`max_height`, `top`/`right`/`bottom`/`left`, `overflow`.
- **Box decoration:** `background_color`, `border`, `border_color`, `border_radius` (`Corner`), `opacity`.
- **Text:** `font`, `font_size`, `color`, `text_alignment`, `text_wrap`, `line_height`, `letter_spacing`.
- **Visibility:** `visibility` (`Visible` / `Hidden`).

`Edge` is a four-side struct (`new(top, right, bottom, left)` + `all(v)`); `Corner` is the four-corner radius. `Units` is `Pixels(f32) | Percentage(f32) | Auto`.

**Critique vs Buiy.** This is a *megacomponent* in exactly the shape [`bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid-row "Megacomponents that are BSN-hostile" warns against. All ~50 fields live on one struct; BSN templates (when they land) cannot patch `background_color` independently of `display`. Buiy's foundation architecture commits to decomposed visual components (`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.3) — `BackgroundColor`, `BorderColor`, `BorderRadius`, etc., are each their own component. woodpecker_ui made the opposite call. See [`lessons.md`](lessons.md) entry on "Avoid: megacomponent styles."

## Widget vocabulary

The built-in widget set (verified from `src/widgets/mod.rs`):

| Widget | Purpose | APG-pattern equivalence |
|---|---|---|
| `WoodpeckerApp` | Root container | — |
| `Element` | Generic styled container (≈ `<div>`) | — |
| `WButton` | Button | Button (APG-tier **F**) |
| `Clip` | Clipping container | — |
| `TextBox` | Single-line text input | Textbox (APG-tier **F**) |
| `Modal` | Modal dialog | Dialog/AlertDialog (APG-tier **F**) — partial |
| `IconButton` | Icon-only button | — |
| `Toggle` | Toggle switch | Switch (APG-tier **F**) |
| `Slider` | Single-value slider | Slider (APG-tier **F**) — partial |
| `Checkbox` | Checkbox | Checkbox (APG-tier **F**) |
| `WoodpeckerWindow` | Draggable in-app window | — (game-UI-focused) |
| `WindowingContextProvider` | Window stacking context | — |
| `Dropdown` | Dropdown selector | Listbox/Combobox (APG-tier **F**) — partial |
| `TabButton` / `TabContextProvider` | Tabs | Tabs (APG-tier **F**) — partial |
| `ColorPicker` | Color picker | Color picker (APG-tier **C**) |
| `ScrollBox` / `ScrollBar` / `ScrollContent` / `ScrollContextProvider` | Scroll container + bar | Scrollbar (APG-tier **C**) |

**Missing vs Buiy widget catalog** ([`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)):

- **Foundational missing:** Link, Heading (with level), Label, Group/Section/Article/Region semantic containers, all 8 landmarks.
- **Selection missing:** Radio Group, full Listbox (multi-select), full Combobox, Spinbutton, Searchbox, Date/Time/File pickers.
- **Navigation missing:** Menu, Menubar, Menu Button, Toolbar, Breadcrumb, Tree, Treegrid.
- **Containers missing:** Alert Dialog (only `Modal`), Popover with light-dismiss, Anchored popover, Tooltip, Disclosure, Accordion, Window splitter, Fullscreen surface.
- **Feedback missing:** Progressbar, Meter, Alert / Status / Log / Timer live regions, Toast / Snackbar, Carousel, Feed, Card, Rating.
- **Tabular missing:** Table, Grid, Sortable/filterable.

This is **a game-UI starter set, not an APG-coverage library**. The Q1 in the README is honest about this: *"A few helper widgets to get you started."*

## State management

Three layers, all ECS-typed:

1. **Per-widget local state** via `HookHelper::use_state` — keyed off the current widget entity, stored in dedicated state entities.
2. **Cross-widget context** via `#[context(...)]` + `use_context` — nearest-ancestor lookup, similar to React Context.
3. **Global state** via plain Bevy `Resource` + `#[resource(...)]` — equality-checked across re-renders.

Reactivity model is plain dirty-bit propagation. No signals, no selectors, no derived/computed graph. The README Q2 names this as a deliberate design: *"They tend to want ownership of the data which means it must live outside of bevy's ECS world. I have problems with this."*

## Sources

- `src/widgets/mod.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/widgets/mod.rs
- `src/styles/mod.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/styles/mod.rs
- `crates/woodpecker_ui_macros/src/lib.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/crates/woodpecker_ui_macros/src/lib.rs
- README (Counter example) — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
- Buiy foundation widget catalog — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Sibling: [`architecture.md`](architecture.md), [`critiques.md`](critiques.md), [`lessons.md`](lessons.md)
