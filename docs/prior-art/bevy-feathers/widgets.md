**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — Widget catalog: shipped controls, contracts, gaps relative to WAI-ARIA APG

# Widget catalog

What ships in `crates/bevy_feathers/src/{controls,containers,display}/` on `main` HEAD (matching the 0.19.0-rc.1 line; 0.18.1 has a subset — `color_plane` is the 0.18 addition, see [history.md](history.md)).

For each widget: spawn API, key state/value, keyboard contract, AccessKit role wiring. **A theme of this catalog: many widgets ship without explicit AccessKit role wiring on the styled entity.** The headless behavior lives in `bevy_ui_widgets`; the AccessKit role may be set there, here, or nowhere — verify per widget. See [accessibility.md](accessibility.md) for the consolidated a11y gap analysis.

## Spawning convention

Two conventions coexist:

1. **Scene-component** (modern): `FeathersFoo` is a `Component` whose `Scene` impl produces the entity hierarchy. Optional `FeathersFooProps` carries config. Spawned by listing the scene component on an entity; the scene fans out children.
2. **Bundle function** (deprecated): `foo_bundle()` returns a `Bundle` for direct `commands.spawn`. Every widget has a deprecated `_bundle()` form retained for back-compat.

Recent PRs are migrating to the scene-component form; the deprecation warnings are still suppressed module-wide.

## Controls — `src/controls/`

### Button — `button.rs`
- **Scene:** `FeathersButton` + `FeathersButtonProps { caption, variant, corners }`. Also `FeathersToolButton` for toolbar variant.
- **Variant enum:** `ButtonVariant::{Normal, Primary, Plain}`. Primary = call-to-action, Plain = background only on hover/press.
- **Event:** emits `bevy_ui_widgets::Activate` on pointer release while hovered, ENTER, or SPACE with focus.
- **Keyboard contract:** TAB to focus (`TabIndex(0)`); SPACE / ENTER to activate. Matches WAI-ARIA APG button pattern.
- **A11y role:** **not explicitly set** in `button.rs` (no `accesskit::Role::Button` literal). Whatever role surfaces depends on `bevy_ui_widgets::Button` upstream; verify per Bevy release.
- **Disabled:** `InteractionDisabled` component suppresses `Activate`; theme token swaps to `BUTTON_BG_DISABLED` / `BUTTON_TEXT_DISABLED`.

### Checkbox — `checkbox.rs`
- **Scene:** `checkbox()` function (modern); `checkbox_bundle()` deprecated.
- **State:** `Checked` component (boolean only; no tri-state).
- **Event:** `ValueChange<bool>`.
- **Keyboard:** `TabIndex(0)`; SPACE toggles (per `bevy_ui_widgets::Checkbox`).
- **A11y role:** no explicit role set in feathers source. Buiy's foundation widget catalog ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) requires tri-state via `aria-checked="mixed"`; feathers does not provide this.

### Toggle switch — `toggle_switch.rs`
- **Scene:** `toggle_switch()` (modern); `toggle_switch_bundle()` deprecated.
- **State:** `Checked`.
- **Event:** `ValueChange<bool>`.
- **Keyboard:** `TabIndex(0)`.
- **A11y role:** **explicitly sets `AccessibilityNode(accesskit::Node::new(Role::Switch))`** on the entity — the only feathers widget verified to wire its role directly in the styled layer.

### Radio — `radio.rs`
- **Scene:** `FeathersRadio` + `FeathersRadioProps { caption }`.
- **State:** `RadioButton` marker + `Checked` component.
- **Events:** `ValueChange<bool>` (per-radio) and `ValueChange<Entity>` (which radio in the group).
- **Keyboard:** `TabIndex(0)`. Arrow-key cycling within a radio group is a `bevy_ui_widgets` responsibility; verify per Bevy version.
- **A11y role:** not explicitly set in `radio.rs`.

### Slider — `slider.rs`
- **Scene:** `FeathersSlider` + `FeathersSliderProps { value: f32, min: f32, max: f32 }` (defaults 0, 0, 1).
- **Event:** `ValueChange<f32>`.
- **Keyboard:** `TabIndex(0)`. **No evidence of arrow-key value adjustment in feathers source**; if the contract is implemented, it's upstream in `bevy_ui_widgets`. WAI-ARIA APG slider pattern requires `←/→/↑/↓` (small step), `PageUp/PageDown` (large step), `Home/End` (min/max). Verify.
- **A11y role:** not explicitly set in `slider.rs`.
- **Single-thumb only**; no multi-thumb / range slider in feathers.

### Color slider — `color_slider.rs`
- **Channels:** RGB (Red / Green / Blue, 0–1), HSL (Hue 0–360°, Saturation 0–1, Lightness 0–1), Alpha (0–1).
- **Scene:** `FeathersColorSlider` + props selecting the channel and base color.
- **Event:** `ValueChange<f32>`.
- **Use case:** color-picker primitive — one channel at a time.

### Color plane — `color_plane.rs`
- **Modes:** `RedGreen`, `RedBlue`, `GreenBlue`, `HueSaturation`, `HueLightness`.
- **Event:** `ValueChange<Vec2>` (normalized 0..1).
- **Note:** uses `Vec3` input where `z` controls the background gradient; avoids internal color-space conversion to dodge gimbal-lock at cylindrical-space poles.
- **Added in 0.18** (Bevy 0.18 release notes — see [history.md](history.md)).

### Color swatch — `color_swatch.rs`
- Small color-display element, used inside color pickers.

### Disclosure toggle — `disclosure_toggle.rs`
- **Purpose:** the chevron-rotating control inside collapsible headers (12×12 px).
- **State:** `Checked` (expanded = checked).
- **Behavior:** chevron icon rotates 90° (`Rot2::turn_fraction(0.25)`) when checked.
- **Keyboard:** `TabIndex(0)`. The underlying `Checkbox` component provides toggle semantics.
- **A11y role:** **not a `Role::Button` with `aria-expanded`** — it inherits checkbox semantics. WAI-ARIA APG disclosure pattern expects `button` + `aria-expanded`; this is a mismatch (see [accessibility.md](accessibility.md)).

### Menu — `menu.rs`
- **Components:** `FeathersMenu` (container), `FeathersMenuButton` (trigger), `FeathersMenuPopup` (popup), `FeathersMenuItem` (item), `FeathersMenuDivider` (rule).
- **Popup:** absolutely positioned at z-index 100; placement bottom-start (or top-start fallback) via a popover system.
- **Events:** `MenuEvent::{Open, Toggle, CloseAll}`.
- **Keyboard:** menu items carry `TabIndex(0)`; `NavAction` directional input drives navigation.
- **A11y:** uses `bevy_ui_widgets::MenuButton` / `MenuItem` for semantic identification. Role wiring lives upstream.
- **Gap:** no menubar, no submenu nesting verified in the file listing; verify against the APG menu pattern before claiming AA conformance.

### Text input — `text_input.rs`
- **Structure:** `FeathersTextInputContainer` (parent for styling) + `FeathersTextInput` (child for editing).
- **Props:** `visible_width`, `max_characters`.
- **Constraint:** `TextLayout::LineBreak::NoWrap` — **single-line only**. No multi-line / textarea.
- **Cursor/selection:** themed via `TextCursorStyle` (cursor + selection colors, both focused and unfocused variants).
- **IME:** **no evidence of IME composition in feathers source**. If present, it's upstream in `bevy_ui_widgets` or `bevy_input` — verify before claiming CJK support.
- **A11y role:** not explicitly set in `text_input.rs`.

### Number input — `number_input.rs`
- **Type-generic:** `F32`, `F64`, `I32`, `I64` per `number_format`.
- **Filter:** `EditableTextFilter` accepts digits + `.`, `-`, `+`, `e`, `E`.
- **Visual quirks:** colored sigil strip on the left edge (typically used as 3D-editor X/Y/Z axis indicator); optional label text.
- **Events:** `ValueChange<T>` while focused, listens for `UpdateNumberInput<T>` when unfocused (host app pushes external updates).
- **Keyboard:** ENTER finalizes. **No arrow-key increment/decrement, no wheel-scroll** — feathers number_input is not a WAI-ARIA `spinbutton` per APG (which requires ↑/↓ stepping).

### Virtual keyboard — `virtual_keyboard.rs`
- **Purpose:** on-screen software keyboard.
- **API:** `VirtualKeyboard<T>` scene + `VirtualKeyboardProps<T>` carrying `Vec<Vec<T>>` (keys-by-row).
- **Event:** `VirtualKeyPressed<T>` entity event.
- **Use case:** consoles, kiosks, gamepad-driven text entry. Not a substitute for OS-IME.

## Containers — `src/containers/`

### Pane — `pane.rs`
- Flex column container with themed header + body. Header is a `JustifyContent::SpaceBetween` row with padding + border; body is a column with gap, padding, and rounded bottom corners. `pane_header_divider()` helper inserts vertical separators.
- Intended use: editor side panels / inspector sections.

### Subpane — `subpane.rs`
- Lighter-weight nested variant of `Pane`.

### Group — `group.rs`
- Generic header + body container — visually grouped but less heavy than `Pane`. The source does not assert "toolbar" or "button group"; it's a flexbox-styled container.

### Flex spacer — `flex_spacer.rs`
- Layout helper: stretches to fill remaining flex space.

## Display — `src/display/`

### Icon — `icon.rs`
- `ImageNode`-based template, default height 14px. Feathers icons are **PNG raster** (`assets/icons/chevron-{down,right}.png`, `x.png`), not vector. Tinting via theme is not visible from the icon template alone — verify per call site.

### Label — `label.rs`
- Themed text label using `ThemedText` + `InheritableFont`.

## Widgets that are conspicuously absent

bevy_feathers's "intended for editors and utilities" framing shows in its omissions. Compared to Buiy's foundation widget catalog ([media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)):

- **No Dialog / Modal / AlertDialog** — modals are a common editor need; absence is notable.
- **No Tooltip widget** — `tooltip` is a foundation-tier APG pattern Buiy commits to.
- **No Tabs / TabList / TabPanel.**
- **No Tree / TreeGrid** — editors typically need scene-graph trees; absent here.
- **No Listbox / Combobox.**
- **No Accordion / Collapsible** (only the disclosure-toggle primitive).
- **No Toolbar widget** (despite `FeathersToolButton` existing).
- **No ProgressBar / Meter / Spinner.**
- **No Date / Time / File picker.**
- **No Alert / Status / Log / Timer live-region widgets.**
- **No Toast / Snackbar.**
- **No Table / Grid.**
- **No Breadcrumb.**
- **No multi-line / rich text editing surface.**
- **No Carousel / Feed.**
- **No Window splitter.**

The kit is consistent with its stated scope (editor utilities). It is **not** a substitute for a WCAG 2.2 AA APG widget set. Buiy's `buiy_widgets` crate targets full APG; feathers does not.

## Implications for Buiy

The shipped catalog tells us what Bevy's official kit currently commits to building. A few takeaways:

- **Per-window coexistence is the policy** ([cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)). A host app that wants both an editor pane (feathers) and a game UI (Buiy) gets them on separate windows. Buiy does not need to provide migration adapters from feathers widgets in v1.
- **Headless / styled split** — `bevy_ui_widgets` ↔ `bevy_feathers` validates Buiy's intent to keep behavior + presentation cleanly split inside `buiy_widgets`.
- **A11y wiring is the load-bearing gap.** Feathers's incomplete role wiring (most widgets don't set `AccessibilityNode` directly) shows what happens when a11y is treated as someone-else's-crate's-job. Buiy's AccessKit-first model ([foundation architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) makes the role + name + states explicit on every widget by construction — see [accessibility.md](accessibility.md).
- **Coverage discipline matters more than count.** Feathers has ~14 controls; Buiy's APG catalog has ~50+ widgets. The difference is not "more" but "the WCAG 2.2 AA-conformant set." Don't compare like-for-like.

## Sources

- `controls/` index — https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src/controls
- `containers/` — https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src/containers
- `display/` — https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src/display
- Individual control source files (button, checkbox, toggle_switch, slider, radio, color_slider, color_plane, disclosure_toggle, menu, text_input, number_input, virtual_keyboard) on `main`.
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- Buiy foundation widget catalog — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Cross-link: [accessibility.md](accessibility.md), [theming.md](theming.md), [integration.md](integration.md)
