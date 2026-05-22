**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — architecture: how it extends bevy_ui, plugin shape, the UiBuilder DSL

# Architecture

sickle_ui sits **on top of** `bevy_ui`. It does not own a layout solver, a render pipeline, a text shaper, a focus model, or an a11y bridge — it consumes Bevy's. It contributes (a) a fluent widget-builder DSL anchored on `UiBuilder<E>`, (b) a state-driven dynamic-styling engine (`DynamicStyle` + `FluxInteraction`), (c) a typed theming model (`Theme<C>` + `PseudoTheme<C>` + `PseudoState`), and (d) a fixed widget catalog of ~30 components. The crate's own description is verbatim: `"A widget library built for Bevy, in Bevy."`

This document covers the architectural shape. See [`api.md`](api.md) for the user-facing API, [`widgets.md`](widgets.md) for the widget catalog, and [`integration.md`](integration.md) for plugin setup.

## The stack

```
+-----------------------------------------------------------------+
| sickle_ui            — widget catalog + UiBuilder DSL           |
+-----------------------------------------------------------------+
| sickle_ui_scaffold   — Theme<C> / PseudoTheme / DynamicStyle /  |
|                        FluxInteraction / UiBuilder / ui_style   |
+-----------------------------------------------------------------+
| sickle_macros + sickle_math  — derive macros, math helpers      |
+-----------------------------------------------------------------+
| bevy_ui              — Node, Style, Interaction, picking, ...   |
+-----------------------------------------------------------------+
| bevy 0.14            — ECS, render, winit, input, text          |
+-----------------------------------------------------------------+
```

The split between `sickle_ui` (catalog of concrete widgets) and `sickle_ui_scaffold` (the theming + builder + style-engine substrate) is deliberate. Third-party widget authors are expected to depend on `sickle_ui_scaffold` and add their own widgets without pulling the entire `sickle_ui` catalog. That separation is *the* design choice the ecosystem later salvaged — `bevy_cobweb_ui` forks the scaffold layer (as `cob_sickle_ui_scaffold`) without re-importing the catalog.

## Plugin shape — `SickleUiPlugin`

`app.add_plugins(SickleUiPlugin)` is the entire onboarding. The docs note one constraint: it `"Must be added after DefaultPlugins"`. The plugin internally chains the following sub-plugins (verified against the docs.rs module index for 0.4.0):

- **Theming layer** — `ThemePlugin`, `ComponentThemePlugin<C>` (per-component theme registration via the user's `app.add_plugins(ComponentThemePlugin::<MyWidget>::default())`).
- **Dynamic-style engine** — `DynamicStylePlugin` (the `DynamicStyleEnterState` / `DynamicStylePostUpdate` system pair, plus `DynamicStyleStopwatch` for animation timing).
- **Pseudo-state plumbing** — `AutoPseudoStatePlugin` (with `HierarchyToPseudoState`, `VisibilityToPseudoState`, `FlexDirectionToPseudoState` derived signals).
- **Interaction tracking** — `FluxInteractionPlugin` (the `FluxInteractionUpdate` and `FluxInteractionStopwatch` resources/systems).
- **Drag / drop / scroll** — `drag_interaction`, `drop_interaction`, `scroll_interaction` modules each register their own systems.
- **Widget plugins** — one per widget family: `ButtonPlugin`, `CheckboxPlugin`, `SliderPlugin`, `RadioGroupPlugin`, `DropdownPlugin`, `MenuPlugin`, `SubmenuPlugin`, `MenuBarPlugin`, `ContextMenuPlugin`, `TabContainerPlugin`, `ScrollViewPlugin`, `SizedZonePlugin`, `DockingZonePlugin`, `FoldablePlugin`, `FloatingPanelPlugin`, `PanelPlugin`, `ColumnPlugin`, `RowPlugin`, `LabelPlugin`, `IconPlugin`, `ContainerPlugin`, plus `WidgetsPlugin` (the umbrella that adds them).

There is no `bevy_ui` replacement here — every widget composes `bevy_ui::Node` entities with sickle-owned marker components and state machines layered on top.

## The UiBuilder DSL

`UiBuilder<E>` is the fluent-API anchor. It is, mechanically, a typed wrapper that carries (a) a `Commands` reference for spawning, (b) an entity ID `E` (often `Entity` for nested calls, `UiRoot` for the root scope), and (c) the ambient context needed for parent/child chaining. The docs call it the `"heart of sickle_ui"`. Authoring a UI looks like (paraphrased from the crate's `simple_editor` example):

```rust
commands.ui_builder(UiRoot).column(|column| {
    column.label(LabelConfig::from("Hello"));
    column.row(|row| {
        row.button(...).style().background_color(...);
        row.checkbox(None, false);
    });
    column.slider(SliderConfig::horizontal(...));
});
```

The DSL is *not* a macro. Every method (`column`, `row`, `button`, `checkbox`, ...) is a regular Rust method dispatched via an extension trait (`UiColumnExt`, `UiRowExt`, `UiButtonExt`, `UiCheckboxExt`, ...) implemented for `UiBuilder<Entity>`. The closure-passing pattern (`|column| { ... }`) is sickle-specific and gives the child scope a fresh `UiBuilder<Entity>` rooted at the just-spawned parent.

The `UiBuilderGetId` trait formalizes "any type that can return its entity id gains the full builder method surface." This is the composition pattern that lets `UiBuilder<MySliderHandleEntity>` automatically inherit `column`, `row`, `style`, etc. without re-implementing them per widget.

See [`api.md`](api.md) for the full extension-trait pattern and a BSN-compatibility analysis.

## Module layout — verified against `docs.rs` for 0.4.0

Top-level modules in `sickle_ui::`:

- **`widgets/`** — the widget catalog. Sub-modules: `inputs/` (checkbox, slider, radio_group, dropdown), `layout/` (container, column, row, panel, foldable, icon, label, scroll_view, sized_zone, docking_zone, floating_panel, resize_handles, tab_container), `menus/` (context_menu, menu, menu_bar, menu_item, menu_separators, submenu, shortcut, extra_menu, toggle_menu_item).
- **`theme/`** — `Theme<C>`, `PseudoTheme<C>`, `ThemePlugin`, `ComponentThemePlugin<C>`, `ThemeRegistry`, plus the sub-modules `dynamic_style`, `dynamic_style_attribute`, `pseudo_state`, `style_animation`, `theme_colors`, `theme_data`, `theme_spacing`, `typography`, `icons`. `DefaultTheme` is the user-implementable trait for declaring "here's how my widget looks by default."
- **`ui_builder/`** — `UiBuilder<E>`, `UiContextRoot`, `UiRoot`, `UiBuilderExt`, `UiBuilderGetId`.
- **`ui_commands/`** — command-style extension traits over `Commands` / `EntityCommands` (`SetBackgroundColor`, `SetMargin`, ...).
- **`ui_style/`** — `UiStyle` + `UiStyleExt` + the ~200 `Set<Property>Ext` extension traits (e.g. `SetWidthExt`, `SetFlexDirectionExt`, `SetPaddingExt`) that drive the style-application surface. `LockedStyleAttributes` lets a widget guard certain properties against user override. The `attribute`, `builder`, `generated`, `manual`, `prelude` sub-modules separate hand-written from macro-generated style commands.
- **`flux_interaction/`** — `FluxInteraction` enum, `FluxInteractionStopwatch`, `TrackedInteraction`, `FluxInteractionConfig`, `FluxInteractionPlugin`. The pointer state machine.
- **`drag_interaction/` / `drop_interaction/` / `scroll_interaction/`** — three parallel interaction state-machine modules layered on top of `FluxInteraction`.
- **`ease` / `lerp`** — animation primitives consumed by `DynamicStyle`.
- **`input_extension`** — keyboard / pointer plumbing helpers.
- **`ui_utils`** — small composition helpers.
- **`prelude`** — re-export of the user-facing surface (60+ structs, 25+ enums, **200+ traits** per docs.rs, plus 4 derive macros: `DefaultTheme`, `EventHandler`, `StyleCommands`, `UiContext`).

The trait-count is worth flagging — `prelude` exposes 200+ traits. That is the cost of the extension-trait DSL: every widget and every CSS-like property gets its own trait. See [`critiques.md` § "The extension-trait DSL surface"](critiques.md).

## DynamicStyle — state-on-style as a first-class engine

`DynamicStyle` is a component attached to any entity that wants its visual properties to respond to interaction state without per-widget glue code. The shape (paraphrased — the actual fields are private behind builder methods):

- A vector of `DynamicStyleAttribute`s, each tagged as one of: **static** (apply once on insertion), **interactive** (apply differently per `FluxInteraction` variant), **animated** (interpolate between two endpoints over time with an easing curve).
- A `ContextStyleAttribute` for cases where the attribute targets a sub-entity (e.g. a button's background applies to the button itself, but the focus ring applies to a child entity — `UiContext` resolves the target).

Resolution happens in `DynamicStylePostUpdate` (system schedule entry). When `FluxInteraction` or `PseudoStates` change on the entity, the engine re-resolves each attribute and writes the result into the appropriate concrete component (`BackgroundColor`, `BorderColor`, `Node` fields, etc.). `DynamicStyleStopwatch` is the per-entity clock that animated attributes consume.

The cleanest equivalence: `DynamicStyle` is sickle's answer to CSS `:hover` / `:active` / `:focus` / `:checked` selectors, expressed as a runtime data structure rather than a stylesheet. **This is the primitive Buiy should study most carefully** — see [`lessons.md` § Borrow](lessons.md).

## FluxInteraction — the pointer state machine

`FluxInteraction` is a refined version of `bevy_ui::Interaction`. The enum (verified against docs.rs):

```rust
pub enum FluxInteraction {
    None,
    PointerEnter,
    PointerLeave,
    Pressed,         // pressing started, not completed/cancelled
    Released,        // pressing completed over the node
    PressCanceled,   // pressing cancelled by releasing outside
    Disabled,
}
```

Helper predicates `is_pressed()`, `is_released()`, `is_canceled()`, `is_disabled()` flatten the multi-state cases. `FluxInteractionStopwatch` records how long the entity has been in its current variant — used by animated `DynamicStyle` attributes (e.g. "fade background over 200ms after entering Pressed"). `TrackedInteraction` wraps the previous-and-current pair so widget systems can detect transitions without retaining their own state.

The `PointerEnter` / `PointerLeave` split (separating them from `None` / `Pressed`) is the load-bearing distinction over `bevy_ui::Interaction` (which exposes only `None / Hovered / Pressed`). Hover-enter and hover-leave-without-press are different visual transitions; sickle gives them distinct variants. Buiy's input-events sub-spec ([`../../specs/2026-05-07-buiy-foundation/interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)) should note this when designing its own state model.

## Theme<C> — typed per-component theming

A user adding theming for a custom widget `MyWidget` does roughly:

1. Implement `DefaultTheme for MyWidget` to declare the baseline style (a `Style::build(...)` chain).
2. Optionally declare additional `PseudoTheme<MyWidget>`s keyed by `&[PseudoState::Checked]`, `&[PseudoState::Open]`, etc.
3. `app.add_plugins(ComponentThemePlugin::<MyWidget>::default())` to register the theme with the `ThemeRegistry`.

At runtime, when an entity holding `MyWidget` and `PseudoStates` (set of currently-active pseudo-states) changes, the theme engine looks up the matching `PseudoTheme<MyWidget>` and rebuilds the entity's `DynamicStyle` from its `DynamicStyleBuilder` closures. The closures receive the `ThemeData` (the per-app color palette, spacing scale, typography), so a single widget definition produces dark-mode and light-mode variants from the same code.

`UiContext` (a separate trait) lets a widget expose named sub-entity slots — e.g. a slider implements `UiContext::get(name)` to return the bar/handle entity for `name == "bar"` / `"handle"`. The theme then targets sub-entities by string, allowing one `PseudoTheme<Slider>` definition to style multiple entities in the slider's internal hierarchy.

## System ordering

sickle hangs its systems off Bevy's existing schedules (`PreUpdate`, `Update`, `PostUpdate`) and exposes a few named system sets:

- **`FluxInteractionUpdate`** — refreshes `FluxInteraction` from `Interaction` + pointer position.
- **`DynamicStyleEnterState` / `DynamicStylePostUpdate`** — apply state-driven style on enter, then re-evaluate animated attributes per-frame.
- **`WidgetLibraryUpdate`** — umbrella system set covering per-widget reconciliation (e.g. slider value clamping, dropdown popup positioning).
- **`TabContainerUpdate`**, **`SizedZonePreUpdate`** — per-widget reconciliation passes scheduled specifically.

There is no `SickleSet::*` enum analogous to Buiy's `BuiySet::Layout / Style / Input / Animate / Picking / A11yUpdate / Render`. The ordering rationale lives in code comments and inferred-from-usage rather than as an explicit named-set discipline.

## What sickle_ui does NOT own

- **Layout solving** — Bevy's `Node` + `Style` (Taffy under the hood) is used directly.
- **Render pipeline** — Bevy's UI render passes; no custom shaders for non-rectangular clipping, backdrop-filter, mix-blend-mode, etc. Inherits all of `bevy_ui`'s renderer caps (see [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)).
- **Text shaping** — `bevy_text` (cosmic-text in Bevy 0.14).
- **Focus model** — relies on Bevy 0.14's `Interaction` + manual focus tracking; no `:focus-visible`, no focus traps, no roving tabindex helpers.
- **AccessKit / a11y** — **none.** No `AccessibilityNode`, no role/label/state mapping. Screen readers will not perceive sickle widgets as anything more than generic UI nodes. See [`critiques.md` § "Accessibility absence"](critiques.md).
- **Animation library** — only the in-line `ease` + `lerp` helpers used by `DynamicStyle`; no transitions API, no keyframes, no spring physics.
- **Input localization / IME / clipboard** — not addressed.

## Implications for Buiy

1. The **scaffold split** (theming + builder + style-engine as a separable crate, widget catalog on top) is a structurally good idea — Buiy's [`buiy_widgets` vs `buiy_widgets_core` split](../../specs/2026-05-07-buiy-foundation/architecture.md) should mirror it.
2. The **FluxInteraction state machine** with explicit `PointerEnter` / `PointerLeave` / `PressCanceled` variants is more useful than `bevy_ui::Interaction`'s three states. Buiy's input-events sub-spec inherits this lesson.
3. The **DynamicStyle component** as the runtime data structure for state-on-style (rather than CSS-style sheets) is BSN-friendly *in principle* — `DynamicStyle` could be a Buiy component if its internals were decomposed. As shipped in sickle, it is *one* component carrying a `Vec<DynamicStyleAttribute>` with builder-style construction; that's a megacomponent in the sense Buiy's foundation rules out. See [`lessons.md`](lessons.md).
4. The **extension-trait DSL** is not borrowable. It dispatches widget construction through method calls on `UiBuilder`, which BSN-as-data-format cannot statically reach. Buiy's widget vocabulary must be expressed as components (BSN-authorable) plus optional helper functions (not the primary surface). See [`api.md` § "BSN-compat assessment"](api.md).
5. The **lack of AccessKit / focus model** is exactly the gap Buiy exists to fill. sickle_ui validates the demand for a widget kit at sickle's API ergonomics level — it does not validate skipping a11y.

## Sources

- crates.io API (versions / dates / downloads) — https://crates.io/api/v1/crates/sickle_ui
- docs.rs module index — https://docs.rs/sickle_ui/0.4.0/sickle_ui/
- `ui_builder` module — https://docs.rs/sickle_ui/0.4.0/sickle_ui/ui_builder/index.html
- `flux_interaction` module — https://docs.rs/sickle_ui/0.4.0/sickle_ui/flux_interaction/index.html
- `FluxInteraction` enum variants — https://docs.rs/sickle_ui/0.4.0/sickle_ui/flux_interaction/enum.FluxInteraction.html
- `theme` module — https://docs.rs/sickle_ui/0.4.0/sickle_ui/theme/index.html
- `theme::dynamic_style` — https://docs.rs/sickle_ui/0.4.0/sickle_ui/theme/dynamic_style/index.html
- `theme::pseudo_state` — https://docs.rs/sickle_ui/0.4.0/sickle_ui/theme/pseudo_state/index.html
- `ui_style` module — https://docs.rs/sickle_ui/0.4.0/sickle_ui/ui_style/index.html
- Companion crate — https://crates.io/crates/sickle_ui_scaffold
- Surviving fork README — https://github.com/UkoeHB/sickle_ui
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_ui critiques (for the renderer-caps inheritance) — [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)
