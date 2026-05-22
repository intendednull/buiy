**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — Architecture: plugin shape, module layout, layering on bevy_ui + bevy_ui_widgets

# Architecture

bevy_feathers is a workspace crate inside [bevyengine/bevy](https://github.com/bevyengine/bevy) at `crates/bevy_feathers/`. It is **a styled widget layer over `bevy_ui_widgets`** (the headless widget primitives) running on `bevy_ui` (the substrate). The crate's own `Cargo.toml` description: `"A collection of UI widgets for building editors and utilities in Bevy"`. The pre-amble framing matters: feathers is opinionated tooling-focused styling — not a general game-UI kit.

See [history.md](history.md) for the introduction PR (#19730, merged 2025-06-28, viridia) and the 0.17/0.18/0.19 timeline, and [governance.md](governance.md) for stewardship (ickshonpe is the active maintainer per recent commits).

## The layer cake

```
+-----------------------------------------------------------+
| bevy_feathers       — styled widgets, tokens, dark theme  |
+-----------------------------------------------------------+
| bevy_ui_widgets     — headless widget primitives          |
|                       (Activate / ValueChange / Checked / |
|                        InteractionDisabled / Slider /     |
|                        Checkbox / MenuItem / ...)         |
+-----------------------------------------------------------+
| bevy_ui             — Node, ComputedNode, Taffy bridge,   |
|                       focus, picking backend, render      |
+-----------------------------------------------------------+
| bevy_a11y / bevy_picking / bevy_input_focus / bevy_text   |
+-----------------------------------------------------------+
```

bevy_feathers does **not** define new core layout, render, or input primitives — it consumes `bevy_ui`. Its job is to (a) bind theme tokens to `BackgroundColor` / `BorderColor` / `TextColor` on the entities `bevy_ui_widgets` exposes, (b) ship a default dark theme, (c) ship a fixed set of widget scenes (button, slider, checkbox, etc.) that wire `bevy_ui_widgets` primitives + theme tokens + `RoundedCorners` + `FocusIndicator` into a coherent visual.

**bevy_feathers is NOT the only "official widget kit."** `bevy_ui_widgets` ships *headless* widget primitives (no styling, no theme, just behavior). bevy_feathers ships *styled* widgets on top. They are sibling crates with different goals: `bevy_ui_widgets` is the substrate any kit can build on; `bevy_feathers` is one specific opinionated kit for editors and utilities. See [comparisons.md](comparisons.md) (Agent B) for how this maps against alternatives like haalka, sickle_ui, iyes_ui.

## Plugin shape

The top-level export is `FeathersPlugins` (a `PluginGroup`), which bundles:

1. `bevy_input_focus::tab_navigation::TabNavigationPlugin`
2. `FeathersCorePlugin`

`FeathersCorePlugin` (the work-doer) does:

- `app.init_resource::<UiTheme>()` — installs the default dark theme.
- `embedded_asset!` for fonts (`FiraSans-{Regular,Italic,Bold,BoldItalic}.ttf`, `FiraMono-Medium.ttf`), icons (`chevron-{down,right}.png`, `x.png`), and shaders.
- `app.add_plugins((ControlsPlugin, CursorIconPlugin, ...))` — sub-plugins for each widget family.
- Schedules `propagate_text_fonts` in `PostUpdate` before `UiSystems::Content`.
- Adds `Observer`s for `ThemeBackgroundColor`, `ThemeBorderColor`, `ThemeTextColor`, `InheritableThemeTextColor` changes — when theme keys change on an entity, the matching `BackgroundColor` / `BorderColor` / `TextColor` is rewritten from the theme map.

Sub-plugin: `ControlsPlugin` adds one `Plugin` per control family: `AlphaPatternPlugin`, `ButtonPlugin`, `CheckboxPlugin`, `ColorPlanePlugin`, `ColorSliderPlugin`, `ColorSwatchPlugin`, `DisclosureTogglePlugin`, `MenuPlugin`, `RadioPlugin`, `SliderPlugin`, `TextInputPlugin`, `ToggleSwitchPlugin`. There is no `NumberInputPlugin` or `VirtualKeyboardPlugin` in the `ControlsPlugin` group (those modules exist but their plugin registration lives elsewhere or they're driven via scene components without per-family observers).

## Module layout — verified against `main` HEAD

`crates/bevy_feathers/src/`:

- **`lib.rs`** — `FeathersCorePlugin`, `FeathersPlugins`, asset embedding, theme observers.
- **`theme.rs`** — `UiTheme` resource, `ThemeProps`, `ThemeToken`, `ThemeBackgroundColor` / `ThemeBorderColor` / `ThemeTextColor` / `InheritableThemeTextColor` / `ThemedText` components, and the four `on_changed_*` observers that resolve tokens to colors.
- **`tokens.rs`** — string constants for every token name (e.g. `WINDOW_BG`, `BUTTON_BG_HOVER`, `SLIDER_BAR_PRESSED`, `FOCUS_RING`, `TEXT_INPUT_CURSOR`).
- **`dark_theme.rs`** — `create_dark_theme()` populates a `UiTheme` for ~100+ tokens. **There is no `light_theme.rs`.** See [theming.md](theming.md).
- **`palette.rs`** — base color constants (`GRAY_0` … `GRAY_3`, `LIGHT_GRAY_1`, etc.) the dark theme draws from.
- **`focus.rs`** — `FocusIndicator` / `FocusWithinIndicator` marker components + the `manage_focus_indicators` system, which sources color from `tokens::FOCUS_RING`, sets width 2px and offset 2px, and only applies the outline when `input_focus_visible.0` is true (the `:focus-visible` analogue).
- **`font_styles.rs`** — `InheritableFont` component → `Propagate<TextFont>` directive propagating font, size, weight down to `ThemedText`-marked descendants.
- **`rounded_corners.rs`** — `RoundedCorners` enum (`None`, `All`, `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Top`, `Right`, `Bottom`, `Left`) with `to_border_radius(r)` mapping to `BorderRadius`. Used for segmented button groups.
- **`cursor.rs`** — `CursorIconPlugin`, `EntityCursor` (system or custom cursor), `DefaultCursor`, `OverrideCursor`, `update_cursor` system in `PreUpdate`.
- **`alpha_pattern.rs`** — checkerboard background shader for alpha previews (used by color widgets).
- **`constants.rs`** — fixed numeric layout constants: `ROW_HEIGHT=24`, `CHECKBOX_SIZE=18`, `HEADER_HEIGHT=30`, `RADIO_SIZE=18`, `TOGGLE_WIDTH=32`, `TOGGLE_HEIGHT=18`, font sizes `MEDIUM_FONT=14`, `COMPACT_FONT=13`, `SMALL_FONT=12`, `EXTRA_SMALL_FONT=11`. These are not theme tokens; they are hardcoded.
- **`controls/`** — 14 per-widget files: `button`, `checkbox`, `color_plane`, `color_slider`, `color_swatch`, `disclosure_toggle`, `menu`, `number_input`, `radio`, `slider`, `text_input`, `toggle_switch`, `virtual_keyboard`, plus `mod.rs` with `ControlsPlugin`.
- **`containers/`** — `flex_spacer`, `group`, `pane`, `subpane` + `mod.rs`.
- **`display/`** — `icon`, `label` + `mod.rs`.
- **`assets/`** — embedded `fonts/`, `icons/`, `shaders/`.

## System ordering

Feathers does **not** introduce its own `SystemSet`s. It hangs systems off `bevy_ui`'s sets:

- `update_cursor` (PreUpdate) — cursor icon resolution from hovered entity.
- `propagate_text_fonts` (PostUpdate, before `UiSystems::Content`) — font inheritance.
- `manage_focus_indicators` (PostUpdate, in `UiSystems::Content`) — focus-ring outline application, change-detection-gated.
- Theme `Observer`s fire on insertion/change of the `ThemeBackgroundColor` / `ThemeBorderColor` / `ThemeTextColor` components.

Buiy by contrast owns named `SystemSet`s (`BuiySet::Layout` → `Style` → `Input` → `Animate` → `Picking` → `A11yUpdate` → `Render`); see foundation [architecture.md § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md).

## How feathers extends bevy_ui's component model

Feathers does **not** subclass or wrap `bevy_ui::Node`. It composes existing bevy_ui components by attaching its own theme components alongside them. A button entity, for example, ends up holding:

- From `bevy_ui`: `Node`, `BackgroundColor`, `BorderColor`, `BorderRadius`, `Visibility`, etc.
- From `bevy_ui_widgets`: `Button` marker, plus the activation plumbing (`Activate` event emitter).
- From `bevy_input_focus`: `TabIndex(0)`.
- From `bevy_feathers`: `ThemeBackgroundColor(BUTTON_BG)`, `ThemeBorderColor(...)`, `FocusIndicator`, `EntityCursor::System(Pointer)`, `RoundedCorners::All`-derived `BorderRadius`.

The theme observer then resolves `ThemeBackgroundColor(BUTTON_BG)` against `UiTheme` and writes the resulting `Color` into `BackgroundColor`. **All visual styling flows through this single resolution step** — every styled property is a token lookup, never a hardcoded color in widget code.

Hover/pressed/disabled/checked variants are handled by swapping the *token reference* on the entity (e.g. `ThemeBackgroundColor(BUTTON_BG)` → `ThemeBackgroundColor(BUTTON_BG_HOVER)`) and letting the observer rewrite the color. The widget itself owns the state-machine logic that decides which token to point at.

This is a clean design but constrains feathers to bevy_ui's authoring surface — there is no escape hatch for non-rectangular clipping, backdrop-filter, mix-blend-mode, etc. (see [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Renderer that caps web-parity features"). Buiy's parallel-stack choice deliberately avoids inheriting these caps; feathers, sitting on bevy_ui, inherits them by construction.

## Relationship to bevy_ui_widgets

`bevy_ui_widgets` is the **behavior**; bevy_feathers is the **style**. The split is intentional and follows the WAI-ARIA "headless" pattern:

- `bevy_ui_widgets::Slider` carries value/min/max, emits `ValueChange<f32>`, handles drag. It does not draw a track or thumb.
- `bevy_feathers::FeathersSlider` wraps that with a track entity (themed), a thumb entity (themed), tab index, focus indicator, and the rounded-corners + cursor visuals.

Any third-party widget kit (sickle_ui, haalka, a future Buiy migration adapter, etc.) can build on `bevy_ui_widgets` independently of feathers. See [comparisons.md](comparisons.md) (Agent B).

## Implications for Buiy

Buiy's foundation [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to a complete parallel stack — Buiy owns its render pipeline, component model, focus model, and a11y integration. **Buiy is not a widget kit on top of bevy_ui; it is the substrate.** Per-window coexistence with bevy_feathers (foundation [cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) means a host app may add `FeathersPlugins` and bevy_feathers will operate on its bevy_ui-owned windows, while Buiy operates on its own.

The architectural lesson worth borrowing (and adapting for Buiy's own widget catalog) is the **headless-behavior / styled-presentation split** that `bevy_ui_widgets` ↔ `bevy_feathers` exemplifies. Buiy's `buiy_widgets` crate should preserve the same split internally — `buiy_widgets::core` carrying behavior + APG keyboard contracts, with one or more theme-driven presentation layers (the default + room for downstream replacement).

## Sources

- Cargo manifest — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/Cargo.toml
- Source root — https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src
- `lib.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/lib.rs
- `theme.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/theme.rs
- `tokens.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/tokens.rs
- `focus.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/focus.rs
- `constants.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/constants.rs
- PR #19730 (introduction) — https://github.com/bevyengine/bevy/pull/19730
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation cross-cutting (coexistence rule) — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- bevy_ui lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
