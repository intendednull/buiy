**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Glossary

# Glossary

Terms used across this corpus.

## Widget-related

**Headless widget** — A widget whose interaction logic (state components + event observers) is shipped, but whose visuals are not. The downstream consumer supplies rendering, styling, theme. Coined for JS libraries like Headless UI and Radix Primitives; ported to Bevy in `bevy_ui_widgets`.

**Marker component** — A zero-sized or near-zero-sized `Component` that identifies an entity as a particular widget (e.g. `Button`, `Checkbox`, `Slider`, `MenuItem`). Carries no state of its own beyond the marker; state lives in companion components.

**Companion component** — A component auto-inserted by a widget marker's `#[require(...)]` chain. E.g. spawning `Slider` auto-inserts `AccessibilityNode + SliderDragState + SliderValue + SliderRange + SliderStep`.

**Controlled component** — A widget that does not own its own state; the app supplies the state and listens for change events. From React. The opposite is **uncontrolled** — the widget owns and self-updates its state.

**External state management** — `bevy_ui_widgets`'s preferred mode: widgets emit events, app updates components. The lib.rs makes this explicit, citing the "live view of dynamic data" use case.

**Self-update observer** — An opt-in observer (like `checkbox_self_update`) that converts the external-state pattern into the controlled-component-with-batteries-included pattern: the observer applies the obvious state update so the app doesn't have to.

**APG keyboard contract** — The keyboard interactions a widget must support per the [WAI-ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/). E.g. radio group: arrow keys navigate (wrap), Home/End jump to ends, Tab moves out.

**Light dismiss** — A popover / dialog dismissal pattern where clicking anywhere outside the surface closes it. (HTML popover state machine: `closedby="any"`.)

## Bevy / ECS-related

**ECS** — Entity Component System; Bevy's runtime model. Widgets are entities with markers + state components; observers + systems react.

**Observer** — A Bevy ECS callback registered against an `EntityEvent` or generic event. `bevy_ui_widgets`'s primary plumbing primitive — every keyboard / pointer interaction is an observer.

**`EntityEvent`** — A Bevy ECS event type that has an entity target. Bubbles through ancestor chains via the portal relation. `Activate`, `ValueChange<T>`, `MenuEvent` are entity events.

**`#[require(...)]` / RequiredComponents** — Bevy's mechanism (PR #14791, since 0.15) for declaring components that must accompany a marker. Spawning the marker auto-inserts default-constructed required components. See [`../bevy-ui/component-model.md`](../bevy-ui/component-model.md) for the broader pattern.

**`Plugin::build`** — Where a Bevy plugin registers observers, systems, components, reflection. `ButtonPlugin::build` adds the Button observers.

**`PluginGroup`** — A `Plugin` family with a builder. `UiWidgetsPlugins` is the plugin group that registers every widget's plugin.

**System set** — A named label for a group of systems, used to order. `UiSystems::Layout`, `InputSystems`, `InputFocusSystems`, `AccessibilitySystems` are the substrate sets `bevy_ui_widgets` interacts with.

**Schedule** — The ordered execution of systems each frame. `bevy_ui_widgets` registers systems in `Update` (menu lifecycle), `PostUpdate` (popover positioning, scrollbar thumb update, text scroll/layout), and `PreUpdate` (text input keyboard handling).

**`InputFocus`** — Resource holding the currently focused entity. Set/cleared by `bevy_input_focus`. Widget keyboard observers read `FocusedInput<KeyboardInput>` which queries this.

**`bevy_picking`** — Bevy's hit-testing primitive. Emits `Pointer<Press>`, `Pointer<Release>`, `Pointer<Click>`, `Pointer<Drag*>`, `Pointer<Cancel>` events to entities under the cursor. Widget pointer observers consume these.

**`AccessibilityNode`** — A wrapper component from `bevy_a11y` carrying an `accesskit::Node` for the entity. Required-component-inserted by every widget marker; converts the entity into an a11y-tree node.

**BSN** — Bevy Scene Notation. The (still-draft) declarative scene authoring format. PR #20158, not yet landed. `bevy_ui_widgets` is being adjusted to be BSN-friendly (`FromTemplate` derives in PR #23924) in anticipation.

**Reflection / Reflect** — Bevy's runtime type-info system. Components derive `Reflect + FromReflect + Default + Clone + Component`. Required for BSN's reflection-driven loading.

## AccessKit / a11y-related

**AccessKit** — Cross-platform accessibility-tree library used by Bevy. Apps build a tree of `accesskit::Node`s; AccessKit adapters push the tree to OS screen readers (NVDA, VoiceOver, Orca, etc.).

**`accesskit::Role`** — Enum of widget roles (Button, CheckBox, Slider, RadioButton, RadioGroup, MenuItem, MenuListPopup, etc.) corresponding to ARIA roles.

**`accesskit_winit::Adapter`** — The winit-window-bound AccessKit adapter. Owned per window. `bevy_a11y` (and Buiy) manage one adapter per winit `WindowId`.

**`TreeUpdate`** — An AccessKit diff describing changes to the accessibility tree since the last push.

**ACCNAME 1.2** — [W3C Accessible Name and Description Computation 1.2](https://www.w3.org/TR/accname-1.2/). The algorithm for deriving a widget's "accessible name" from label, content, ARIA attributes, etc.

## bevy_ui-related state components

**`Pressed`** — Marker component on `bevy_ui` widget entities indicating active pointer-press / key-press. Inserted by widget pointer-down observers; removed on release/cancel/drag-end.

**`Checked`** — Marker component indicating the widget is in its "checked" state (Checkbox, RadioButton, Switch).

**`Checkable`** — Companion marker required by Checkbox + RadioButton + Switch indicating the widget *can* be checked (vs always-checked or stateless).

**`InteractionDisabled`** — Marker component disabling all widget interaction. Widget observers early-out when present.

**`Hovered`** — Marker component from `bevy_picking::hover::Hovered` indicating the pointer is over the entity.

## WAI-ARIA APG patterns referenced

Each of the following has a corresponding page at `https://www.w3.org/WAI/ARIA/apg/patterns/<pattern>/`:

- button, checkbox, radio, slider, slider-multithumb, menubar, dialog-modal, alertdialog, alert, tabs, toolbar, breadcrumb, treeview, treegrid, grid, table, listbox, combobox, spinbutton, textbox, switch, link, disclosure, accordion, tooltip, windowsplitter, carousel, feed, meter, progressbar.

## Buiy-related (cross-corpus)

**Buiy** — The project this corpus documents prior art *for*. A parallel UI stack to bevy_ui; integrates Taffy + cosmic-text + AccessKit + bevy_picking + Bevy's render graph directly; ships its own component model + render pipeline.

**Parallel-stack** — Buiy's chosen relationship to bevy_ui: not layered, not forking, not extending — running alongside, with per-window coexistence. See [foundation README § 1.4](../../specs/2026-05-07-buiy-foundation/README.md).

**Per-window coexistence** — In an app with multiple winit windows, each window is owned by one UI stack (Buiy *or* bevy_ui). The two stacks do not share a window or render layer. See [foundation cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).

**WCAG 2.2 SC** — Web Content Accessibility Guidelines 2.2 Success Criteria. Buiy's widget catalog targets specific SCs as per-widget contracts (e.g. 2.5.7 drag alternative, 2.5.8 hit target ≥24×24, 1.4.13 tooltip dismiss).

**Tier (F / C / E / O)** — Buiy's feature classification: Foundation / Core / Extended / Out. Per [foundation README § "Tier legend"](../../specs/2026-05-07-buiy-foundation/README.md).

## Sources

- `crates/bevy_ui_widgets/src/lib.rs` and per-widget files
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- AccessKit — https://accesskit.dev / https://docs.rs/accesskit/
- Bevy ECS docs — https://bevyengine.org/learn/quick-start/getting-started/ecs/
- Buiy foundation specs — [`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)
- Sibling: [`../bevy-ui/glossary.md`](../bevy-ui/glossary.md), [`../accesskit/`](../accesskit/), [`../bevy-a11y/`](../bevy-a11y/)
