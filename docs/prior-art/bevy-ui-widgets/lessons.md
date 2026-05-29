**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Validates / Avoid / Borrow decisions for the Buiy widget catalog

# Lessons for Buiy

This is the consult-this-when-designing decision file for the Buiy widget catalog spec(s). Other files in this corpus are evidence; this file is the synthesis. **Validates** (Buiy choices `bevy_ui_widgets` confirms) / **Avoid** (pitfalls + Buiy mitigations) / **Borrow** (primitives worth studying and adapting).

## Validates

These Buiy design choices are confirmed by `bevy_ui_widgets`'s experience:

- **Headless widget pattern: behavior in components + observers, no rendering coupling.** This is exactly the shape Buiy commits to in [foundation architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md) and [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md). bevy_ui_widgets demonstrates the pattern works in ECS — the 5 widgets shipped at 0.17 with the headless model are functional and have AccessKit a11y plumbing baked in. The validation comes from: (a) the design has reached production cadence (lockstep with Bevy minors) under multiple maintainers, (b) `bevy_feathers` builds non-trivially on top of it, (c) the same authors (viridia) that designed `bevy_a11y`'s post-decomposition surface designed this — there's continuity of design thinking. See [`architecture.md`](architecture.md) § "The headless primitive pattern" and [`api.md`](api.md) for full shape.

- **Decomposed state components (`Pressed`, `Checked`, `Checkable`, `InteractionDisabled`, `Hovered`, `SliderValue`, `SliderRange`, `SliderStep`, `SliderPrecision`, `SliderDragState`, `ScrollbarDragState`, `MenuFocusState`).** This is exactly the issue-#17644 post-mortem applied. Buiy's "small, public-fielded, observable, decomposed" component rule ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)) is validated — bevy_ui_widgets does it (with caveats; see Avoid section), apps build custom styling cleanly against the state surface, and there's no megacomponent in sight.

- **External state management ("controlled" mode).** Widget does not own its value; emits `ValueChange<T>`; app updates state. This is the right default for a game / live-data context (per lib.rs rationale: *"a live view of dynamic data coming from deeper within the game engine"*). Buiy's reactivity choice (observers + change detection only, no signals — [architecture.md § 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md)) is aligned. bevy_ui_widgets shows this pattern shipping cleanly with one well-chosen escape hatch (`checkbox_self_update`).

- **AccessKit + role-per-marker via `#[require(...)]`.** Every widget's marker auto-inserts `AccessibilityNode(accesskit::Node::new(Role::X))`. This is BSN-friendly, requires-component-friendly, and means the a11y tree is correct by construction. Buiy's plan to drive AccessKit directly with decomposed `A11yRole / A11yLabel / A11yDescription / A11yStates / A11yRelations` components ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the right next step — the bevy_ui_widgets pattern uses an `AccessibilityNode` wrapper around the underlying accesskit::Node, which Buiy should *not* mirror (per [`../bevy-a11y/`](../bevy-a11y/) the wrapper pre-dates the decomposition lesson).

- **Per-widget plugin, with PluginGroup convenience.** `ButtonPlugin`, `CheckboxPlugin`, `SliderPlugin`, etc., individually addable; `UiWidgetsPlugins` plugin group adds them all. Buiy's `BuiyPlugin` sub-plugin order ([architecture.md § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md)) follows the same pattern — `widgets` is its own sub-plugin in the order. Per-widget plugins remain a worthwhile granularity for opt-in.

- **Two-event vocabulary: `Activate` + `ValueChange<T>`.** A simple, broad-coverage event surface. `Activate` for buttons + menu items + invocations; `ValueChange<T>` for sliders + checkboxes + radios + text inputs + custom-typed-edit widgets. The `is_final: bool` flag distinguishes interim from terminal updates without needing separate `DragStart`/`DragEnd`/`Change` event types per widget. Buiy's widget catalog should adopt this vocabulary directly. See [`api.md`](api.md) § "Event API."

- **`observe(...)` as a declarative-attachment bundle effect** — co-locates the observer with the spawn site without separate `commands.entity(e).observe(...)` ceremony. The shape is BSN-friendly (`Button + observe(...)` is a tuple, just like any other component tuple). Buiy should ship an equivalent in `buiy_core` (per the source-comment hint that this primitive is misplaced in a widget crate).

- **Popover positioning as a separate primitive, not a widget.** `Popover` (the component) is just a positioning rule + candidate-placement list; it is composed by Menu, would be composed by Tooltip / Combobox / Dialog. Buiy's plan for Popover with light-dismiss state machine + anchored positioning ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md), [`visuals.md § 3.2`](../../specs/2026-05-07-buiy-foundation/visuals.md)) follows the same split.

## Avoid

Pitfalls drawn from `bevy_ui_widgets`'s experience, with Buiy's mitigation.

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Shallow decomposition — fields that should be their own components** — `Slider` carries `track_click + orientation`; `Scrollbar` carries `target + orientation + min_thumb_length`; `MenuPopup` carries `layout: MenuLayout`. Each is patchable individually only by editing the whole marker via reflection. | [`open-problems.md`](open-problems.md) § "Critique 2: BSN-friendliness shallow"; [`widgets.md`](widgets.md). | Buiy components apply the issue-#17644 maxim ruthlessly: every field that might end up shared across many nodes or hot-reloaded independently gets its own component. `TrackClick`, `SliderOrientation`, `MenuLayout` are standalone components on the slider/menu entity, not enum fields on the marker. Document the decomposition heuristic in a Buiy spec. |
| **Five-widget starter set + 8-month-and-counting cadence to add 3 more.** | [`history.md`](history.md); [`open-problems.md`](open-problems.md) § "Critique 1: Scope choice." | Buiy spec ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) commits to ~50 widgets at F+C tier. The verification harness ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md), gates 3/4/7) is the load-bearing artifact that makes catalog-scale work tractable. Without the harness, the work historically doesn't get done. |
| **Switch-as-Checkbox-with-role-override.** The current shape asks users to monkey-patch the a11y tree to reuse Checkbox plumbing. | [`open-problems.md`](open-problems.md) § "Critique 4: Switch workaround." | Buiy's `Switch` is its own widget per [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) ("Switch. F"). Distinct from Checkbox; distinct role + lifecycle + visual conventions. |
| **Scrollbar not in a11y tree, but no shipped scrollable-container companion.** The piece that *would* own the a11y is the one not shipped. | [`open-problems.md`](open-problems.md) § "Critique 6"; [`widgets.md`](widgets.md). | Buiy ships both the scrollbar widget (focusable per ARIA `scrollbar` role) AND the scroll-container primitive — apps get the a11y wired for free. See [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) "Scrollbar — focusable scrollbar widget per ARIA `scrollbar` role." |
| **Stale "experimental" doc-comment + removed feature-flag = mixed signal to downstream consumers.** | [`open-problems.md`](open-problems.md) § "Critique 7: Stale experimental labeling." | Buiy uses explicit tiers (F / C / E / O) per widget, gated in CI per [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md). Stability claims per widget are verifiable, not narrative-claim-only. |
| **No first-letter type-ahead in Menu or Radio; no submenus; PageUp/PageDown missing from Slider.** APG keyboard contracts have known holes. | [`apg-coverage.md`](apg-coverage.md) § "Keyboard contract conformance"; [`open-problems.md`](open-problems.md) § "Open Problem 3: Keyboard navigation maturity"; menu.rs in-source TODOs. | Buiy's per-widget contract includes full APG keyboard coverage as a CI gate (gate 7, APG keyboard contract). Submenus are F-tier in the Menu sub-spec. |
| **Geometric-only orientation (no `:dir(rtl)` awareness).** Slider, Radio arrows, Menu Row layout are all geometric — RTL apps get backwards interactions. | [`open-problems.md`](open-problems.md) § "Open Problem 8: Localization / BiDi." | Buiy's `:dir(ltr / rtl)` pseudo-class ([interaction.md § 3.7](../../specs/2026-05-07-buiy-foundation/interaction.md)) and RTL mirroring are foundation-tier ("Every widget below ships, by default, with … RTL mirroring"). Per-widget tests cover both LTR and RTL keyboard interactions. |
| **No 1000-widget stress test; observer-heavy design with no benchmarks.** | [`open-problems.md`](open-problems.md) § "Open Problem 7: Performance at scale." | Buiy's verification harness enumerates 1000+-node productivity-app fixtures explicitly. Per-frame layout and observer cost are gated in CI ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)). |
| **`observe(...)` helper landed in the widget crate by accident** with an in-source TODO. Cross-cutting infrastructure misplaced. | [`open-problems.md`](open-problems.md) § "Critique 5: `observe` misplaced"; [`api.md`](api.md). | Buiy's equivalent lives in `buiy_core` from day one, not in `buiy_widgets`. Cross-cutting primitives are not parked in the highest-level crate; they live next to ECS, not next to widgets. |
| **`bevy_ui::*` hard dependency** — the widget observers reference `ComputedNode`, `UiTransform`, `UiGlobalTransform`, `BackgroundColor`, `BorderColor`, `Node`, `ScrollPosition`. Cannot lift onto a different node type. | [`api.md`](api.md) § "The 'compose your own renderer' promise"; [`integration.md`](integration.md) § "With non-Bevy UI". | Buiy widgets reference `buiy::Node`, `buiy::ComputedNode`, `buiy::*` exclusively. The parallel-stack rationale (foundation [README § 1.4](../../specs/2026-05-07-buiy-foundation/README.md)) — Buiy widgets cannot be reused on bevy_ui surfaces and vice-versa, which is the cost of owning the pipeline. |
| **Five different sources owning state machines (`Pressed` in bevy_ui, `Checked` in bevy_ui, focus split across `bevy_ui::focus` / `bevy_input_focus` / `bevy_ui::auto_directional_navigation`).** A new contributor reading the Slider code has to chase those breadcrumbs across crates. | [`architecture.md`](architecture.md) § "Substrate dependencies"; [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) (focus model split). | Buiy owns a single focus tree ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) with `:focus-visible`, traps, restoration, inert, roving tabindex, `aria-activedescendant`, sequential + spatial nav. State components live in `buiy_core`, not scattered across sub-crates. |
| **"External state, no escape hatches" forces 5× form code.** Only Checkbox has `checkbox_self_update`. Productivity apps with many fields pay disproportionately. | [`open-problems.md`](open-problems.md) § "Critique 11: External-state cost"; [`api.md`](api.md). | Buiy ships per-widget self-update observers by default (e.g. `radio_self_update`, `slider_self_update`, `text_input_self_update`). Apps that want the bevy_ui_widgets-style controlled mode opt-out by not registering them. |

## Borrow

Concrete primitives worth studying and adapting:

1. **The two-event vocabulary: `Activate` + `ValueChange<T>` with `is_final: bool`.** Adopt directly in Buiy's widget catalog. Simpler than per-widget event types, covers the activation + value-edit distinction cleanly, supports interim-vs-commit semantics. See [`api.md`](api.md) § "Event API."

2. **`#[require(...)]` chains for marker → state + a11y wiring.** Spawning `Slider` auto-inserts `AccessibilityNode + SliderDragState + SliderValue + SliderRange + SliderStep`. Buiy's widget markers should do the same — `buiy::Button` auto-inserts `A11yRole(Button) + Focusable + Hittable + ...`. The pattern is BSN-friendly (one component → many wired up) and follows the established Bevy idiom.

3. **`observe(...)` as a bundle-effect helper.** Buiy ships its analog in `buiy_core` so the API works the same way: `commands.spawn((buiy::Button, observe(|_: On<Activate>, ...| ...)))`. (Note the source-comment says the helper "probably doesn't belong in bevy_ui_widgets" — Buiy avoiding that misfile from the start.)

4. **Popover positioning as a primitive separate from any one widget.** `Popover { positions: Vec<PopoverPlacement>, window_margin }` is composed by Menu, would be composed by Tooltip, Combobox, Dialog, etc. The "candidate placement, pick the first that fits" pattern is a clean abstraction over CSS anchor positioning. Buiy's Popover spec ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md), [`visuals.md § 3.2`](../../specs/2026-05-07-buiy-foundation/visuals.md)) borrows this shape.

5. **External-state controlled mode + opt-in self-update observers.** The Checkbox pattern — `Checkbox` does not auto-toggle `Checked`; `checkbox_self_update` is an opt-in observer that does. Buiy ships this pattern with the inverse default (self-update is *on* by default), but the API shape is exactly the bevy_ui_widgets shape.

6. **`SetChecked` / `SetSliderValue` etc. command events** for driving widgets from external sources (gamepad, scripts, tests). These let a non-pointer/non-keyboard input flow (e.g. a gamepad mapping system) drive the widget without faking pointer events. Buiy should ship the same pattern: every widget that has state has a `Set<X> { entity, value }` event.

7. **`SliderThumb` / `ScrollbarThumb` as child markers** that the widget locates via a child query, instead of a fixed child index. Lets app authors put the thumb anywhere in the hierarchy. Buiy's widgets should use child-marker patterns the same way — flexible visual composition without hard-coded structure.

8. **`is_final: bool` discriminator on `ValueChange<T>`.** Avoids per-widget `DragStart` / `DragEnd` / `Change` events. Apps that care about interim updates listen for everything; apps that only care about commits filter on `is_final`. Adopt directly.

9. **`MenuFocusState::{Opening(NavAction), Open, Closed}` lifecycle.** Cleanly handles the "set focus on the popup's first item but the popup may not have spawned yet (BSN async)" problem with a state-machine field. The `Opening` state is the deferred-focus signal; a polling system picks it up once the children exist. Buiy's overlay / popover / dialog lifecycle can use the same pattern.

10. **`#[component(immutable)]` on `SliderValue` and `SliderRange`** signals that these are written-via-replace (insert), not written-via-mutate. Pairs with ECS change-detection cleanly. Buiy should use the same idiom for value-carrying state components.

11. **`TrackClick::{Drag, Step, Snap}` as a configurable interaction policy.** Three legitimate behaviors for "click the slider track" — the design didn't pick one; it exposed all three. Same shape applies to other widgets where multiple APG-conformant interpretations exist (e.g. Tabs auto-activate vs manual-activate).

12. **Per-widget reflection registration via the widget's own Plugin.** `RadioButton`'s `Plugin::build` does `app.register_type::<RadioButton>()`. Avoids the "did someone remember to register this?" problem. Buiy's per-widget plugin must do the same for every component the widget exposes.

13. **The `parley` dependency in `Cargo.toml`** — present but used only transitively via `bevy_text` on `main` (which migrated to parley). Reminder: bevy_ui_widgets's text-handler is parley-coupled post-0.19, but **Buiy commits to cosmic-text** ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)). Buiy's widget catalog cannot reuse `bevy_ui_widgets::text_input` even on a bevy_ui-window basis — the shaper is divergent. This compounds the parallel-stack rationale.

## How to use this file

When designing a Buiy widget:

1. **Find the row in `Avoid`** matching a pitfall close to your design. Read the linked file for the original incident.
2. **Find the entry in `Borrow`** matching a primitive close to what you're designing. Read the linked file to understand the bevy_ui_widgets shape, then adapt for Buiy's component model (no `bevy_ui::*` dependency, full WCAG / APG coverage as a CI gate, decomposed components, reflection-registered).
3. **Promote any decision into a Buiy spec** under `docs/specs/` — this file is for capturing what we learn from `bevy_ui_widgets`, not for encoding Buiy's own decisions.

## Sources

- Per-widget source files at `crates/bevy_ui_widgets/src/` (@ main, 2026-05-22)
- `examples/ui/widgets/standard_widgets.rs` (canonical custom-styling recipe)
- Buiy foundation specs — [`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)
- Sibling evidence: [`architecture.md`](architecture.md), [`widgets.md`](widgets.md), [`api.md`](api.md), [`apg-coverage.md`](apg-coverage.md), [`integration.md`](integration.md), [`history.md`](history.md), [`distribution.md`](distribution.md), [`open-problems.md`](open-problems.md), [`ecosystem.md`](ecosystem.md)
- Issue #17644 (megacomponent / BSN-hostility) — https://github.com/bevyengine/bevy/issues/17644
- Discussion #16900 (Standard Headless Widgets) — https://github.com/bevyengine/bevy/discussions/16900
- Bevy 0.17 / 0.18 announcements — https://bevy.org/news/
- Sibling prior-art: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../bevy-a11y/`](../bevy-a11y/), [`../bevy-feathers/`](../bevy-feathers/), [`../accesskit/`](../accesskit/), [`../bevy-picking/`](../bevy-picking/)
