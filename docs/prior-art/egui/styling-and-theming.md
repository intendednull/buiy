**Date:** 2026-05-22
**Status:** active
**Subject:** egui — visuals, dark/light themes, the Style API, and the limits of its theming model

# Styling and theming

egui's theming model is one of its most-visible structural limits. This file inventories what the model offers, what it doesn't, and how the "egui look" arises from those decisions. The architectural foundation is in [`architecture.md`](architecture.md); the widget vocabulary that consumes styling is in [`api-surface.md`](api-surface.md).

## The two-level model: `Style` and `Visuals`

Styling configuration lives on `egui::Context` and is structured as:

- **`egui::Style`** — the top-level container. Holds `Visuals` plus non-color settings: `spacing`, `interaction`, `text_styles`, `wrap_mode`, `animation_time`, debug overlays.
- **`egui::Visuals`** — the color/treatment subset. Holds the dark/light split, button backgrounds, text colors, hyperlink colors, selection colors, hover/active states, window frame styling.

The split is:

- `Style` answers "how big is the spacing, what's the indent width, what's the slider grab radius."
- `Visuals` answers "what color is the button, what's the hyperlink hue, what's the focus ring."

Both can be replaced wholesale (`ctx.set_style(style)`) or mutated incrementally (`ctx.style_mut(|s| s.spacing.item_spacing = ...)`)). Scoped overrides via `ui.scope(|ui| { ui.style_mut().visuals.override_text_color = ...; })` apply only to widgets emitted inside the closure.

## The dark/light split

`Visuals::dark()` and `Visuals::light()` are the two preset themes egui ships. They're not parameterized variants of a single token system — they're two hand-crafted color palettes with hardcoded color choices for each visual slot:

```rust
pub struct Visuals {
    pub dark_mode: bool,
    pub override_text_color: Option<Color32>,
    pub widgets: Widgets,           // per-state widget visuals
    pub selection: Selection,       // selected text / item
    pub hyperlink_color: Color32,
    pub faint_bg_color: Color32,
    pub extreme_bg_color: Color32,
    pub code_bg_color: Color32,
    pub warn_fg_color: Color32,
    pub error_fg_color: Color32,
    pub window_rounding: Rounding,
    pub window_shadow: Shadow,
    pub window_fill: Color32,
    pub window_stroke: Stroke,
    pub panel_fill: Color32,
    pub popup_shadow: Shadow,
    pub resize_corner_size: f32,
    pub text_cursor: TextCursorStyle,
    pub clip_rect_margin: f32,
    pub button_frame: bool,
    pub collapsing_header_frame: bool,
    pub indent_has_left_vline: bool,
    pub striped: bool,
    pub slider_trailing_fill: bool,
    pub handle_shape: HandleShape,
    pub interact_cursor: Option<CursorIcon>,
    pub image_loading_spinners: bool,
    pub numeric_color_space: NumericColorSpace,
}
```

(Field set varies slightly across versions; the shape is representative as of 0.34.x.)

The `Widgets` substructure defines per-state visuals: `noninteractive`, `inactive`, `hovered`, `active`, `open`. Each carries `WidgetVisuals { bg_fill, weak_bg_fill, bg_stroke, fg_stroke, expansion, rounding }`. This is egui's analogue to CSS `:hover` / `:active` / `:focus-visible` — a per-state visual override applied automatically based on the widget's `Response` flags.

`Visuals::dark()` is the default. Switching: `ctx.set_visuals(egui::Visuals::light())`.

## Custom themes

Users build custom themes by mutating `Visuals` fields:

```rust
let mut visuals = egui::Visuals::dark();
visuals.window_fill = egui::Color32::from_rgb(20, 20, 30);
visuals.panel_fill = egui::Color32::from_rgb(15, 15, 25);
visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(25, 25, 35);
visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(40, 40, 60);
visuals.hyperlink_color = egui::Color32::from_rgb(120, 180, 255);
ctx.set_visuals(visuals);
```

This works, and many shipping egui apps do it (`bevy-inspector-egui` ships several themes, Rerun has its own customized palette). But it's also exactly as ad-hoc as it looks — there is no token layer, no semantic palette, no cascade.

## What's missing vs a modern token system

A modern token system (the model Buiy's foundation spec targets, [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5) provides:

1. **Semantic tokens** — `color.surface.primary`, `color.surface.secondary`, `color.text.default`, `color.text.muted`, `space.4`, `radius.md`, `motion.fast`. Components reference tokens by name; never raw values.
2. **Palette + scales + variant composition** — a theme is a palette × scale × variant tuple. Variants for dark/light/high-contrast compose with the same scale.
3. **OS-pref binding** — `prefers-color-scheme`, `prefers-contrast`, `prefers-reduced-motion`, `forced-colors` surface as theme-variant selectors automatically.
4. **Cascade / inheritance** — a subtree can override theme; descendants see the override.
5. **Animation tokens** — `motion.fast`, `motion.medium`, `motion.slow` referenced by animation systems.
6. **Contrast linting** — automated validation that token-defined color pairs meet WCAG 2.2 AA contrast ratios.

egui has **none of this**. Specifically:

- **No tokens.** Widget code references `Color32::RED` directly or hits a `Visuals` field. No layer of indirection where "this widget paints in `color.surface.primary`."
- **No scales.** Spacing values are raw f32 with no semantic name (`Spacing::item_spacing = vec2(8.0, 3.0)`).
- **No variants beyond dark/light.** High-contrast support would be a third hand-crafted `Visuals` instance the user wires up themselves.
- **No OS-pref binding.** egui doesn't read `prefers-color-scheme`, `prefers-contrast`, `prefers-reduced-motion`, or `forced-colors`. The host backend (eframe, bevy_egui) doesn't surface these as theme-variant selectors. Apps that want dark/light to follow OS need to wire that themselves.
- **No cascade.** A subtree override via `ui.scope(|ui| ui.style_mut()...)` applies *inside* the scope but doesn't compose with parent overrides — the closure's `Style` is a copy that's discarded at scope exit.
- **No animation tokens.** `Style::animation_time` is a single global f32 (default 1/12 sec). All animations share it.
- **No contrast linting.** Custom themes that fail WCAG 2.2 AA contrast ratios will silently fail — there's no built-in check.

## Why egui apps tend to "look like egui"

Without a token system, the path of least resistance is to use `Visuals::dark()` as-is or with minor tweaks. The result is that the vast majority of egui apps share visual fingerprint:

- A specific rounded-rectangle button style with subtle hover lift.
- A specific muted dark-gray panel background.
- A specific blue accent color.
- A specific monospace-looking font (`Ubuntu Mono`-derived built-in).
- A specific text size (13 px after the 0.33.0 increase from 12.5; "Body" `TextStyle`).

This is consistent with the README non-goal: **"Native looking interface"** is explicitly excluded. egui has its own visual identity, and the styling primitives are calibrated to make small variations on that identity easy while making fundamental visual reinvention hard. For dev tools this is appropriate — "this looks like a tool, not like the game" is correct affordance signaling. For production game UI or polished consumer apps, it's a structural limit.

The community has produced multiple "themes" (catppuccin, nord-style, solarized-style) as third-party crates that ship pre-configured `Visuals` instances. These shift palette but don't change the fundamental egui aesthetic — there's no way to make egui look like, say, Material Design 3 or iOS 18 without rewriting the widget vocabulary.

## Text styles and fonts

`Style::text_styles: BTreeMap<TextStyle, FontId>` maps named styles (`Body`, `Heading`, `Monospace`, `Button`, `Small`, `Name(String)`) to `FontId { size, family }`. `FontFamily::Proportional` and `FontFamily::Monospace` are the built-in families; users register more via `FontDefinitions` (see [`text-rendering.md`](text-rendering.md)).

A custom-font setup:

```rust
let mut fonts = egui::FontDefinitions::default();
fonts.font_data.insert(
    "my_font".into(),
    std::sync::Arc::new(egui::FontData::from_static(include_bytes!("MyFont.ttf"))),
);
fonts.families
    .get_mut(&egui::FontFamily::Proportional)
    .unwrap()
    .insert(0, "my_font".into());
ctx.set_fonts(fonts);
```

`Style::text_styles` is set per `Context` and per `Ui` scope. There is no per-paragraph or per-span font override outside of `RichText` (which lets you build a `LayoutJob` with mixed-style runs for a single text emission). Multi-font text in a single paragraph works; rich-text editing (mixed-format buffer the user can edit) does not.

## Per-widget style override

Inline override via scope:

```rust
ui.scope(|ui| {
    ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::DARK_RED;
    if ui.button("Danger").clicked() { /* ... */ }
});
```

The scope closure receives a `Ui` with a cloned `Style`; mutations to `ui.style_mut()` apply only to that scope. This works for one-off overrides; it doesn't compose into a reusable styling system.

## Second-party styling extensions

- **`egui_extras`** — official extras crate. Adds `Table` (more flexible than `Grid`), `DatePickerButton`, image loaders. Carries some visual styling specific to these widgets but doesn't redefine the broader theming model.
- **`egui_plot`** — separate-repo plotting crate. Has its own `PlotConfig` for grid colors, axis styles, legend placement. Plot styling is decoupled from `Visuals`.
- **Community themes** — `egui_catppuccin`, `egui-themes` (various), `egui_dock` (docking panes with its own theming concerns), `egui_extras::syntax_highlighting` for code blocks.

None of these introduce a token layer. They all work by setting `Visuals` fields with curated values.

## OS preferences: explicitly not handled

egui does not read OS UI preferences. The host backend can plumb `prefers-color-scheme` etc. into the egui context, but there's no built-in path. eframe has a `Theme::FollowSystem` option (added in eframe 0.27+) that switches between `Visuals::dark()` and `Visuals::light()` based on the OS-reported theme — that's the extent of OS-pref support. `prefers-contrast`, `prefers-reduced-motion`, `forced-colors` are not surfaced at all.

For Buiy: the foundation spec ([`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5) commits to all of these as `UserPreferences` resource bound to theme variants automatically. This is a real gap to fill, and it's not just a matter of plumbing — the token system itself has to exist before OS-pref binding is meaningful.

## Implications for Buiy

What this folder documents about egui's theming model translates directly to Buiy design lessons:

- **Validates the token-layer commitment.** egui's lack of tokens is the proximate cause of "all egui apps look like egui." A token layer is the necessary substrate for visual identity beyond a single library aesthetic.
- **Validates OS-pref binding as foundation work.** Bolting OS-prefs onto a non-token theme system means hand-wiring each preference per app. Bolting them onto a token system means defining how variants compose, once.
- **Validates contrast linting at theme-load time.** egui has no contrast linter; custom themes silently fail WCAG without warning. Buiy's foundation spec calls for both load-time and CI-time contrast linting.
- **Validates per-state visual decomposition.** egui's `Widgets { noninteractive, inactive, hovered, active, open }` is a reasonable per-state structure that Buiy should mirror in its own component design — naming aside, the per-state breakdown is the right shape.
- **Cautionary about animation tokens.** `Style::animation_time` as a single global is too coarse. Buiy needs separate motion tokens for `fast` / `medium` / `slow` plus reduced-motion-mode mappings.

## See also

- [`architecture.md`](architecture.md) — how `Style` and `Visuals` live on `Context`.
- [`api-surface.md`](api-surface.md) — which widgets consume which Style/Visuals fields.
- [`text-rendering.md`](text-rendering.md) — `TextStyle` and `FontDefinitions` in depth.
- [`../bevy-egui/api-surface.md`](../bevy-egui/api-surface.md) — how Buiy adjacent stacks handle theming (cross-stack styling comparison; see § Styling: `Style`, `Visuals`, `Spacing`).

## Sources

- `egui::Style` rustdoc — https://docs.rs/egui/latest/egui/struct.Style.html
- `egui::Visuals` rustdoc — https://docs.rs/egui/latest/egui/style/struct.Visuals.html
- `egui::TextStyle` rustdoc — https://docs.rs/egui/latest/egui/enum.TextStyle.html
- egui CHANGELOG — https://raw.githubusercontent.com/emilk/egui/master/CHANGELOG.md
- eframe `Theme::FollowSystem` — https://docs.rs/eframe/latest/eframe/enum.Theme.html
- Community theme crates (catppuccin etc.) — https://crates.io/search?q=egui+theme
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
