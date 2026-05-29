**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Critiques and open problems

# Critiques + open problems

Honest read of `bevy_ui_widgets` as of 0.19.0-rc.2 / 2026-05-22. Surfaces what the crate doesn't yet solve, where the design has cost, and what a Buiy spec author needs to know before committing to anything downstream.

## Critiques

### 1. Scope choice — five widgets is a starter set, not a kit

The crate ships ~5 widgets covered fully (Button, Checkbox, Radio, Slider, Menu) plus Scrollbar (deliberately not-a11y), Popover (positioning primitive), and EditableText (input handler driving bevy_text). That's a fraction of the ~30 APG widget patterns. Critical absences for any production UI: **Tabs, Dialog, AlertDialog, Listbox, Combobox, Tooltip, Progressbar, Disclosure, Accordion, Switch (as its own widget), Toggle Button, Treeview**.

The shipping cadence (0.17 → 0.18 → 0.19 = 8 months, added 3 widgets) suggests that closing the APG gap is a multi-year effort. Apps needing those widgets either:
- Use `bevy_feathers` (when its widgets cover the gap; Feathers does ship `ColorPlane` and a styled gallery but its scope is editor-shaped, not app-shaped).
- Compose third-party (`sickle_ui` etc., which weren't designed against `bevy_ui_widgets`).
- Build their own.

This is not unique to Bevy — most "headless" libraries (Radix, React Aria) took years to fill out their catalogs. But it does mean **bevy_ui_widgets as of 2026-05-22 is not the "you can build a UI from this" crate**; it's a "you can build the simplest UI from this" crate.

### 2. BSN-friendliness: decomposed, but the decomposition is shallow

The widget components are largely BSN-friendly by the issue-#17644 standard (small, public-fielded, observable, reflection-registered). `Slider`'s required-component chain (`SliderDragState + SliderValue + SliderRange + SliderStep`) is exactly the decomposition #17644 demanded.

But the bar is not uniformly held:

- **`Scrollbar` carries `target: Entity, orientation: ControlOrientation, min_thumb_length: f32` as one component** — three fields, one of them an entity reference. A BSN template would have to set the `target` programmatically (entity ids aren't BSN-serializable in any obvious way). This is the canonical "entity-reference field" challenge.
- **`Popover` carries `positions: Vec<PopoverPlacement>`** — a vec of structs. BSN-templatable (PR #23924 added `FromTemplate`), but verbose.
- **`Slider` carries `track_click: TrackClick + orientation: SliderOrientation`** — two related fields in one component. Could be decomposed into `TrackClick` and `SliderOrientation` standalone components per the issue-#17644 maxim.
- **`MenuPopup` carries `layout: MenuLayout`** — single field, but: APG has Column / Row / Grid as conceptually distinct widgets (menu vs menubar vs menu-grid), and `MenuLayout::Grid` is explicitly *"keys not mapped, you'll need to write your own observer"*. Bundling them in one enum is a smell.

PR #23924 (FromTemplate derive, merged 2026-04-22) was added late and covers *"most"* components, not all. The crate is still iterating toward BSN-readiness on a per-component basis.

### 3. Discoverability vs `bevy_feathers`

Apps ask: "do I use `bevy_ui_widgets` or `bevy_feathers`?" The answer is non-obvious without reading both:

- **`bevy_feathers`** is the styled kit — ergonomic for editor-flavored UI; opinionated visuals (dark theme tokens, atlas icons, fixed sizing); ships widgets like `feathers_counter` and a `feathers_gallery` example.
- **`bevy_ui_widgets`** is the headless brain — flexible; requires writing all visual code yourself; not "use this for your app" in any direct sense.

The naming doesn't help: nothing in the names "ui_widgets" vs "feathers" signals which is the substrate and which is the styled kit. A first-time user might reasonably assume the named widget crate is the user-facing one and the bird-named crate is internal. New users hitting `bevy::ui_widgets` are likely to spawn a `Button` and be surprised it has no visuals.

### 4. The `Switch` role workaround signals a design tension

From `checkbox.rs`: *"If you are going to do a toggle switch, you should override the `AccessibilityNode` component with the `Switch` role instead of the `Checkbox` role."*

This is asking users to monkey-patch the a11y tree to reuse the Checkbox observer. The right design is a separate `Switch` marker that requires `Role::Switch` and shares the Checkbox observer internally. The current shape elides the divergence between Checkbox and Switch (Switch has no tri-state, no group semantics, different visual conventions) into "use Checkbox + override role" — a workaround, not a primitive.

### 5. `observe(...)` is admittedly misplaced

From `observe.rs`: *"TODO: This probably doesn't belong in bevy_ui_widgets, but I am not sure where it should go. It is certainly a useful thing to have."* The helper is a `bevy_ecs` primitive (declarative observer attachment) that landed in the widget crate by accident. It uses `unsafe` code and is generally useful — but it's an open question whether it stays here.

### 6. Scrollbar is not in the a11y tree (deliberately) — but the workaround isn't sketched

The scrollbar.rs design rationale: *"Scrollbars don't have an `AccessibilityNode` component, nor can they have keyboard focus. This is because scrollbars are usually used in conjunction with a scrollable container, which is itself accessible and focusable."*

This is APG-conformant (the scrollable container does own the contract). But `bevy_ui_widgets` does not ship a "scrollable container" widget — only the scrollbar. So apps need to wire keyboard scrolling, ARIA `aria-controls`, and focus into their own scroll container. The piece that owns the a11y is the one the crate doesn't ship.

### 7. The "experimental" labeling is stale

`lib.rs` still says: *"This crate is currently experimental and under active development. The API is likely to change substantially: be prepared to migrate your code."* Yet [PR #22934](https://github.com/bevyengine/bevy/pull/22934) (2026-02-18) removed the `experimental` cargo-feature flag and the crate now compiles in `bevy::ui_widgets` by default. The signal is mixed — "experimental but compiled by default" — and downstream consumers have a hard time gauging stability.

The lib.rs warning is honest about API churn (the 0.17 → 0.19 evolution confirms it). But the cargo-feature removal made the crate de-facto stable from a "should I use it?" perspective. The doc and the feature flag are out of sync.

## Open problems

### 1. APG coverage breadth

The single biggest open problem. ~25 APG patterns are uncovered (see [`apg-coverage.md`](apg-coverage.md)). The two most-asked-for-by-apps:

- **Dialog (modal + non-modal)** — every app needs this. No work in flight visible in the issue tracker.
- **Tabs (auto-activate + manual-activate)** — second most-asked. Also not in flight.

Sub-problems:
- **Combobox + Listbox** — neither shipped. Combobox is the textbox + popup-listbox hybrid, requires both popup positioning (now available via `Popover`) and Listbox (not shipped). A Combobox PR would necessarily ship Listbox first.
- **Tree + Grid + Treegrid** — complex data-display widgets. Likely deferred indefinitely.
- **Tooltip** — Popover positioning exists, but no Tooltip role/lifecycle wrapper (hover + focus trigger, dismissable per WCAG 1.4.13).

### 2. Drag-and-drop widgets

No drag-and-drop primitives at all. `Pointer<DragStart>` / `Pointer<Drag>` / `Pointer<DragEnd>` exist in `bevy_picking` (the Slider uses them), but no headless DnD widget pattern (draggable list reorder, drop target, drag-source handle) ships. App teams reinvent.

### 3. Keyboard navigation maturity

- **First-letter type-ahead** missing from Menu and Radio (both APG-recommend it).
- **PageUp / PageDown larger-step** missing from Slider.
- **Submenu open/close** missing from Menu (in-source TODO).
- **`aria-activedescendant` pattern** not modeled — widgets like Combobox / Listbox use it to keep focus on the input while moving "virtual focus" through a list.

### 4. Touch / gamepad handling

`Pointer<*>` events handle touch via `bevy_picking`'s pointer abstraction, but:

- No widget specializes for touch (no minimum hit-target enforcement, no swipe gestures, no touch-and-hold-to-edit).
- Gamepad support requires app-supplied event-to-action mapping; widgets accept `SetChecked` / `SetSliderValue` / etc. commands so gamepad → widget routing is *possible*, but no shipping integration ([`bevy_input_focus`](../bevy-ui/text-and-input.md) has `AutoDirectionalNavigation` since 0.18, but the widget-side integration is app-side).

### 5. WCAG 2.2 SC coverage gaps

WCAG 2.2 success criteria the crate doesn't currently address:

- **2.5.7 Dragging Movements (Level AA)** — every drag interaction must have a non-drag alternative. Slider's drag-only interaction lacks an obvious keyboard alternative beyond arrow-step.
- **2.5.8 Target Size (Minimum) (Level AA)** — no minimum hit target enforced (≥24×24 CSS px). Apps must enforce this themselves.
- **2.4.11 Focus Not Obscured (Minimum) (Level AA)** — no focus-tracking-in-viewport logic; menus/popovers may render over the focused element.
- **2.4.13 Focus Appearance (Level AAA)** — no focus-ring visuals shipped; users build their own.
- **3.3.7 Redundant Entry (Level A)**, **3.3.8 / 3.3.9 Accessible Authentication** — text input lacks paste-friendly defaults (it does support Cmd+V), no remember-me hooks, no password-field special treatment.
- **1.4.13 Content on Hover or Focus** — Tooltip pattern not shipped; the standard dismissable/hoverable/persistent contract therefore not enforced.

### 6. Themability beyond `bevy_feathers`

The headless crate has no theme abstraction. `bevy_feathers` ships a tokens / theme system, but apps that don't use Feathers must reinvent. There is no canonical "this is how a third-party widget kit should consume bevy_ui_widgets state" template.

### 7. Performance at scale

No published benchmarks. The observer-heavy design has costs:

- Every key event fires a `FocusedInput<KeyboardInput>` observer chain across every focused widget. With 1000+ widgets in a scene (e.g. a large editor inspector), the observer dispatch cost compounds.
- `position_popover` is a `PostUpdate` system that iterates all `Popover`-marked entities every frame regardless of whether their anchor moved. No dirty-tracking.
- Sliders fire `ValueChange<f32>` on every drag tick — if the app has expensive observers, drag latency rises.

No 1000-widget stress test ships in `examples/`. The 10-widget `standard_widgets.rs` example is the largest demo. This mirrors the gap [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md) flags for bevy_ui itself: *"a few hundred microseconds per frame at moderate counts."*

### 8. Localization / BiDi handling

- **Slider orientation is geometric**, not logical — there is no concept of "start" vs "end" relative to writing direction. In an RTL layout, a "horizontal" Slider goes left-to-right regardless of `:dir(rtl)`.
- **Radio arrow-key wraparound is geometric** — ArrowRight always means "next index," not "logical next." In RTL, this is backwards.
- **Menu Row layout** has the same RTL issue.
- **EditableText**: the `cosmic-text` / `parley` substrate handles BiDi at the shaping level, but the widget-level caret motion is logical (left arrow moves the caret one position backward in logical order; in RTL this is visually-rightward). The interaction is correct; the **labeling in keyboard contracts is inconsistent** — `KeyCode::ArrowLeft` doesn't mean "visual left" everywhere.

No `:dir(rtl)` integration is built into widgets. Apps wanting RTL-correct widgets currently must override.

### 9. WASM / mobile target maturity

- **WASM:** Bevy's WASM story is partial (no AccessKit web adapter yet — `accesskit_web` is the missing piece). `bevy_ui_widgets` runs on WASM but the AccessibilityNode components don't reach a screen reader because there's no adapter.
- **iOS:** AccessKit iOS adapter is in-progress upstream. Until it ships, widgets are not accessible on iOS.
- **Android:** AccessKit Android (TalkBack) adapter exists but isn't well-tested with `bevy_ui_widgets`.
- **Console:** No console-specific Bevy support (Bevy doesn't ship console adapters publicly); apps target consoles via third-party Bevy forks.

The widget code is platform-neutral, but the **a11y reaches only desktop screen readers** in practice (NVDA on Windows, VoiceOver on macOS, Orca on Linux).

### 10. Documentation drift

- `lib.rs` says "Warning: Experimental" but the cargo-feature flag is gone.
- The `observe(...)` helper has an in-source TODO that's been there since 0.17.
- The Bevy 0.18 announcement names "Popover" and "MenuPopup" but doesn't name "RadioButton/RadioGroup improvements" explicitly (the improvements were real per PR list, but the announcement is sparse).
- No `crates/bevy_ui_widgets/README.md` exists (the `crates.io` description is the only top-level doc; the lib.rs doc-comment is the only API-level overview).

### 11. The "external state, no two-way binding" stance has real cost

The deliberate choice in `lib.rs`: *"the primary motivation for this is to avoid two-way data binding in scenarios where the user interface is showing a live view of dynamic data."* Correct for games. But for productivity-app forms with 50 fields, requiring the app to write one observer per field plus one polling system per visual variant **is a lot of code**. The form will be 5× the size of a React `<input value={x} onChange={setX}>`. `checkbox_self_update` is the only escape hatch shipped, and only for Checkbox.

## Implications for Buiy

- **Buiy commits to a much wider scope** ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md), ~50 widgets at F+C). The Bevy "ship 5 widgets per release" cadence calibrates expectations: 50 widgets is multi-quarter work, not multi-week. The verification harness ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) is the load-bearing artifact that makes catalog-scale work tractable in CI.
- **Buiy's widget catalog must avoid the BSN-friendliness shallow-decomposition trap.** Apply the issue-#17644 maxim ruthlessly: `track_click + orientation` on `Slider` is two components, not one; `target + orientation + min_thumb_length` on `Scrollbar` is three; etc. Document the decomposition heuristic in [`/docs/specs/2026-05-08-buiy-component-decomposition-design.md`](../../specs/) once it's authored.
- **Buiy's "external state" stance should ship escape hatches by default**, not as opt-in. `checkbox_self_update`-equivalent observers per widget are cheap to write and avoid the 5× form-code problem. Two-way-binding-via-signals is out of scope per Buiy's reactivity choice ([architecture.md § 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md)), but per-widget self-update observers are not.
- **`Switch` is its own Buiy widget** ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md): "Switch. F"), not a Checkbox-with-role-override.
- **Tooltip with WCAG 1.4.13 lifecycle** is Buiy F-tier, with full hover/focus dismiss state machine — closing the bevy_ui_widgets gap.
- **Dialog modal+non-modal + AlertDialog with `closedby`** is Buiy F-tier — closing another bevy_ui_widgets gap.
- **Buiy's WCAG 2.2 SC coverage is gated in CI** (verification gates 3, 4, 7). 2.5.7 (drag alternative), 2.5.8 (hit target ≥24×24), 1.4.13 (tooltip dismiss), 3.3.7 (redundant entry), 2.4.11 (focus not obscured) all become per-widget contract.
- **Performance at scale must be verified, not assumed.** Buiy's harness explicitly enumerates 1000+-node productivity-app fixtures ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)); bevy_ui_widgets has no such gate.

## Sources

- Per-widget sources at `crates/bevy_ui_widgets/src/` (@ main, 2026-05-22)
- PR #22934 (remove experimental flag) — https://github.com/bevyengine/bevy/pull/22934
- PR #23924 (FromTemplate derives) — https://github.com/bevyengine/bevy/pull/23924
- Issue #17644 (megacomponent / BSN-hostility) — https://github.com/bevyengine/bevy/issues/17644
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- Sibling: [`apg-coverage.md`](apg-coverage.md), [`widgets.md`](widgets.md), [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md), [`../bevy-ui/open-problems.md`](../bevy-ui/open-problems.md)
- Buiy widget catalog — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy verification — [`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
