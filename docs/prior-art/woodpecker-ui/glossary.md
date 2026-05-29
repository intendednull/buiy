**Date:** 2026-05-22
**Status:** active
**Subject:** woodpecker_ui — glossary of crate-specific terms

# Glossary

woodpecker_ui-specific terms (and a few cross-referencing entries to Bevy and dependencies). Buiy-specific terminology lives in the foundation spec; bevy_ui terminology lives in [`../bevy-ui/glossary.md`](../bevy-ui/glossary.md) when present.

## A — F

- **`auto_update`** — `#[auto_update(render)]` attribute on `#[derive(Widget)]`. Tells the proc-macro to auto-generate the `update() -> bool` function by diffing the declared `#[props]` / `#[state]` / `#[context]` / `#[resource]` inputs. Pairs with `#[widget_systems(update, render)]` as the alternative (manual mode).

- **`bevy-trait-query`** — third-party crate (version 0.16 in woodpecker_ui) enabling runtime trait-object dispatch over ECS components. woodpecker_ui uses it to register widgets as `dyn Widget` so the runner can iterate widget systems polymorphically.

- **`bevy_vello`** — Linebender's Bevy integration for the vello path-rendering library. Provides `VelloPlugin`, `VelloScene`, `VelloView`, `VelloFont`, AA configuration. woodpecker_ui's render pipeline emits a vello scene per frame and lets `bevy_vello` rasterize it.

- **`Change<T>`** — generic event wrapper used for the canonical "widget state changed" events: `Change<TextChanged>`, `Change<ToggleChanged>`, `Change<CheckboxChanged>`, `Change<SliderChanged>`, `Change<DropdownChanged>`, `Change<ColorPickerChanged>`.

- **`Clip`** — built-in widget that pushes a vello-scene clip rect for its subtree (push/pop layer pattern).

- **`Corner`** — four-corner radius struct (top-left, top-right, bottom-left, bottom-right). Used by `WoodpeckerStyle.border_radius` and rounded `Quad` rendering.

- **`CurrentFocus`** — Bevy `Resource` holding the currently focused widget entity. Defaults to `Entity::PLACEHOLDER`. Only one focus globally; no `:focus-visible` distinction.

- **`CurrentWidget`** — `Resource` wrapping the current widget's `Entity`. Injected by the runner into each widget's `update` / `render` systems via Bevy's `Res<>` extraction.

- **dioxus-devtools** — Dioxus framework's developer-tools subsystem. Used by woodpecker_ui's optional `hotreload` feature (pinned at `0.7.0-alpha.0`) for live-patch.

- **`Edge`** — four-side struct (top, right, bottom, left). Used by `padding`, `margin`. Constructors: `Edge::new(t, r, b, l)`, `Edge::all(v)`.

- **`Element`** — generic widget; the woodpecker equivalent of an HTML `<div>`. Takes `WoodpeckerStyle` and an optional `WidgetRender` content slot.

- **`#[hot]`** — proc-macro from `woodpecker_ui_macros` (gated by the `hotreload` feature) marking a render system as hot-patchable.

## G — N

- **`HookHelper`** — `ResMut` resource implementing React-style hooks. Methods include `use_state<T>(commands, current_widget, initial)`, `use_context<T>(...)`, `use_prev_resource<T>(...)`. State entities are keyed off `(parent_widget_entity, hook_index)` and tracked across re-renders via `PreviousWidget`.

- **`ImageManager`** — `Resource` tracking image-asset handles available for `WidgetRender::Image`. Extracted to the render world via `ExtractResourcePlugin`.

- **kayak_ui** — the predecessor Bevy UI crate by the same author (StarArawn / John). Last release 2024-02-11 (`0.5.0`). woodpecker_ui's README Q3 explicitly names kayak_ui as the lineage. See [`history.md`](history.md).

- **`Mounted`** — marker component that fires once when an entity is first inserted by the reconciler; lifecycle hook for first-mount setup.

## P — T

- **Parley** — Linebender's high-level text-shaping library. Pinned to `0.4` in woodpecker_ui's `Cargo.toml`. Note: `src/lib.rs` doc-comment still references cosmic-text — that's pre-migration residue.

- **`ParentWidget`** — `Resource` wrapping the parent widget's `Entity`. Converted to `CurrentWidget` via `.as_current()`.

- **`PassedChildren`** — slot-projection mechanism — lets a widget forward its children slot to a descendant.

- **`PreviousWidget`** — `HookHelper` internal mapping from current widget entity to its prior incarnation, used to preserve hook state across re-renders.

- **`PreviousResource<T>`** — wrapper resource holding the prior-frame value of a tracked Bevy resource. Used with `#[resource(...)]` for diff-driven re-rendering.

- **`#[props(...)]`** — attribute on `#[derive(Widget)]` declaring which components on the widget entity are equality-tracked as props.

- **`Quad`** — `WidgetRender::Quad` variant — filled rectangle with rounded corners, the base box-decoration primitive.

- **`register_widget::<T>()`** — `App` extension method from `WidgetRegisterExt`. Registers a `#[derive(Widget)]` type as a `dyn Widget` (via `bevy-trait-query`) and registers its auto-generated systems with `WoodpeckerContext`.

- **`RenderSettings`** — `Resource` holding render-pipeline configuration: `layer: RenderLayers`, `antialiasing: AaConfig`, `use_cpu: bool`.

- **`runner`** — `src/runner.rs`, the main reactive scheduler. The README Q3 advertises this as the load-bearing simplicity win vs kayak_ui (200 lines vs 1k).

- **skrifa** — Linebender's font-parsing / glyph-rasterization library. Pinned to `0.30.0`.

- **`#[state(...)]`** — attribute on `#[derive(Widget)]` declaring state component types tracked via `HookHelper`.

- **Taffy** — the layout engine ([github.com/DioxusLabs/taffy](https://github.com/DioxusLabs/taffy)). Pinned to `0.7` with `flexbox` + `grid` features (no `block`, no `float`).

## U — Z

- **`Units`** — enum used throughout `WoodpeckerStyle`: `Pixels(f32)`, `Percentage(f32)`, `Auto`.

- **`use_state`** — `HookHelper::use_state` method. React-style state hook; returns an `Entity` holding the state component. State entity is created on first call and preserved across re-renders.

- **vello** — Linebender's GPU-accelerated path-rendering library. The substrate underneath `bevy_vello`. Capabilities include rounded clip, `clip-path`, gradients, drop-shadow, blur — most of what `bevy_ui` lacks.

- **`WButton`** — built-in button widget. (Named with leading `W` to avoid clash with the `Button` term that other libraries use.)

- **`Widget`** — trait implemented by `#[derive(Widget)]`. Defines `get_name()`, `update()`, `render()`. Registered with `bevy-trait-query` for polymorphic iteration.

- **`#[derive(Widget)]`** — proc-macro from `woodpecker_ui_macros` that turns a user component into a registered widget type. Companion attributes: `widget_systems`, `auto_update`, `props`, `state`, `context`, `resource`.

- **`WidgetChildren`** — `Component` representing the desired children of a widget. Built fluently: `WidgetChildren::default().with_child::<T>(bundle).with_observe(entity, system)`. `apply(parent)` reconciles the actual entity hierarchy with the spec.

- **`WidgetLayout` / `WidgetPreviousLayout`** — output components from the Taffy layout pass. Hold resolved size and position.

- **`WidgetMapper`** — `Resource` mapping the widget tree to entity-tree state for reconciliation.

- **`WidgetMetrics`** — `Resource` holding per-widget-type counts and system-timing metrics. Optional (`metrics` Cargo feature).

- **`WidgetRender`** — leaf-content enum attached to a widget that has visual content: `Text { content, word_wrap }`, `Image`, `Svg { handle }`, `Quad`, `Layer` / `Clip`, `Custom(WidgetRenderCustom)`.

- **`WidgetRenderCustom`** — trait for escape-hatch direct vello scene emission. Implement to draw arbitrary vello primitives from a custom widget.

- **`#[widget_systems(update, render)]`** — manual-mode attribute on `#[derive(Widget)]`. User writes both `update()` and `render()` themselves rather than using `#[auto_update(render)]`.

- **`WoodpeckerApp`** — root container widget (one per UI tree).

- **`WoodpeckerContext`** — `Resource` holding the root widget entity and the registered widget-systems map. `set_root_widget(entity)` is the canonical setup call.

- **`WoodpeckerStyle`** — single ~50-field style component (layout + box-decoration + text + visibility). The megacomponent critiqued in [`critiques.md`](critiques.md) and [`lessons.md`](lessons.md).

- **`WoodpeckerStyleProp`** — wrapper component for passing `WoodpeckerStyle` as a prop (for change-detection purposes).

- **`WoodpeckerUIPlugin`** — entry-point Bevy plugin. `app.add_plugins(WoodpeckerUIPlugin::default())`.

- **`WoodpeckerView`** — marker component for the camera that woodpecker_ui will render into. Tied to `bevy_vello`'s `VelloView`.

- **`WoodpeckerWindow`** — built-in draggable in-app window widget (game-UI-flavored).

## Sources

- `src/lib.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/lib.rs
- `src/styles/mod.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/styles/mod.rs
- `src/widgets/mod.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/src/widgets/mod.rs
- `crates/woodpecker_ui_macros/src/lib.rs` — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/crates/woodpecker_ui_macros/src/lib.rs
- README — https://raw.githubusercontent.com/StarArawn/woodpecker_ui/main/README.md
