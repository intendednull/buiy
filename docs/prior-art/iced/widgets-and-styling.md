**Date:** 2026-05-22
**Status:** active
**Subject:** Iced — built-in widget catalog, custom widgets, function-based styling, themes

# Iced widgets and styling

Iced ships a substantial built-in widget set as of 0.14 — broader than egui, narrower than GTK/Qt. Styling is function-dispatch on `&Theme`, not CSS. Themes are `struct Theme` values, not asset files.

Sibling files: [`architecture.md`](architecture.md), [`elm-architecture.md`](elm-architecture.md), [`layout-engine.md`](layout-engine.md).

## Built-in widget catalog (Iced 0.14)

From `iced::widget` ([docs.rs/iced/0.14.0/iced/widget](https://docs.rs/iced/0.14.0/iced/widget/)):

**Containers / structure:**

- `Column`, `Row` — main-axis stacking; flex distribution
- `Container` — single-child wrapper with padding / alignment / background
- `Scrollable` — viewport with scrollbars (0.14 adds smart scrollbars + autoscroll)
- `Stack` — z-stacked children, all sharing the same rectangle
- `Space` — explicit empty area
- `Rule` — visual divider (horizontal or vertical)
- `PaneGrid` — splittable / draggable pane layout

**Primitive inputs:**

- `Button` — pressable, generic over child content
- `Checkbox`, `Radio`, `Toggler` — boolean inputs
- `Slider`, `VerticalSlider` — bounded numeric input
- `TextInput` — single-line text edit
- `TextEditor` — multi-line text edit (added 0.12, mature in 0.13)
- `PickList` — dropdown selector
- `ComboBox` — searchable / filterable picker (combobox-style)
- `ProgressBar` — visual progress
- `QRCode` — QR rendering

**Text + media:**

- `Text` — styled text run
- `Image` — raster image
- `Svg` — vector image via `resvg`
- `Markdown` — incremental markdown widget (heavy 0.14 upgrades: image support, tasklists, quotes)
- `RichText` — multi-attr text spans (added 0.13)
- `Canvas` — programmatic 2D drawing via `lyon` path API
- `Shader` — custom wgsl shader on a rectangle (wgpu only)

**Behavior / layout helpers:**

- `Tooltip` — hover overlay (0.14: delay support)
- `MouseArea` — invisible pointer-event collector
- `Opaque` — opt out of pointer-passthrough
- `Themer` — scope a different `Theme` to a subtree
- `Lazy` — memoize subtree against a hash key
- `Responsive` — children built once layout limits known
- `Keyed::column` / `Keyed::row` — stable-identity child reconciliation

**New in 0.14:**

- `Table`, `Grid` — tabular layouts (not Excel-grade — bounded column-count + row-count; not virtualized for unbounded data)
- `Pin` — absolutely-positioned overlay anchored to a parent
- `Float` — non-DOM-equivalent: a child rendered separately from layout flow (used for floating menus, dialogs)
- `Sensor` — invisible widget that emits messages on resize / mount / scroll, for app-level layout observation

**Not in 0.14:**

- No data-grid / virtualized list — `Column` + `Scrollable` is the recommended pattern (perf depends on `Lazy`).
- No tree-view widget.
- No tab-bar widget (apps build their own from `Button` + `Container`).
- No modal-dialog primitive (apps compose `Stack` + `MouseArea` + `Opaque`; libcosmic ships a recipe).
- No date/time picker.
- No accordion / collapsible.
- No drag-and-drop (winit-level events exposed, but no DnD widget contract).
- No menubar / context-menu primitive (libcosmic ships its own, not in upstream Iced).

## Custom widgets via the `Widget` trait

Apps with novel UI implement `iced::advanced::widget::Widget<Message, Theme, Renderer>` directly. The required surface:

```rust
trait Widget<Message, Theme, Renderer: iced::advanced::Renderer> {
    fn size(&self) -> Size<Length>;
    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node;
    fn draw(&self, tree: &Tree, renderer: &mut Renderer, theme: &Theme,
            style: &Style, layout: Layout, cursor: Cursor, viewport: &Rectangle);
    // optional:
    fn state(&self) -> tree::State;        // widget-local state shape
    fn tag(&self) -> tree::Tag;            // identity for state reconciliation
    fn on_event(&mut self, ...) -> Status; // message dispatch
    fn mouse_interaction(...) -> Interaction;
    fn overlay(...) -> Option<overlay::Element>;
    // accessibility hooks — not implemented (no AccessKit)
}
```

`tree::State` + `tree::Tag` is the persistent-state-across-rebuilds mechanism — every widget that needs state (a button's pressed-flag, a text-input's cursor) declares its state shape, and the runtime keeps it in the parallel `Tree` keyed by position + tag.

Custom widgets are not rare in production Iced — libcosmic ships dozens, Halloy ships several, Modrinth-launcher ships several. The pattern is robust but verbose.

## Styling — function dispatch on `&Theme`

Iced 0.13 introduced "functional widget styling" ([PR #2312](https://github.com/iced-rs/iced/pull/2312)) — every styleable widget takes a closure `Box<dyn Fn(&Theme, Status) -> Style>`:

```rust
button("Save")
    .style(|theme: &Theme, status: button::Status| button::Style {
        background: Some(theme.palette().primary.into()),
        text_color: theme.palette().background,
        border: border::rounded(4),
        ..button::Style::default()
    })
```

Per-widget `Status` enums capture interactive state (`button::Status::{Active, Hovered, Pressed, Disabled}`), so the style closure can switch on the state. This is Iced's equivalent of CSS `:hover` / `:active` / `:disabled` — but it's *application code*, not a stylesheet.

The `Catalog` trait standardizes the style API: each widget defines `Catalog::default_style(&Theme) -> Style` plus optional named variants (`button::primary`, `button::secondary`, `button::text`, `button::success`, `button::danger`). Theme implementors supply the variants; application code references them via name:

```rust
button("Save").style(button::primary)   // built-in named style
button("Cancel").style(button::text)
```

Or apps write their own closures for one-offs.

## Themes

`Theme` is an enum with built-in variants:

- `Light`, `Dark` — the originals
- `Dracula`, `Nord`, `SolarizedLight`, `SolarizedDark`, `GruvboxLight`, `GruvboxDark`
- `TokyoNight`, `TokyoNightStorm`, `TokyoNightLight`
- `KanagawaWave`, `KanagawaDragon`, `KanagawaLotus`
- `Moonfly`, `Nightfly`, `Oxocarbon`, `Ferra` (Ferra was Iced 0.13's blessed light/dark pair)
- `Custom(Box<custom::Custom>)` — application-defined

Each `Theme` resolves to a `Palette { background, text, primary, success, warning, danger }`. The 0.14 release added a *warning* color and overhauled palette derivation using the **Oklch** color space (perceptually-uniform deltas across the palette).

OS-pref binding (`prefers-color-scheme`) is in 0.14 via "System theme reactions" — `Theme::default()` reads the OS theme on launch; apps can subscribe to OS-theme-change events. No support for `prefers-contrast`, `forced-colors`, or `prefers-reduced-motion` as first-class concerns.

## No CSS-style cascade

A subtree does **not** inherit styles from its parent the way CSS does. Each widget's style is computed in isolation from `&Theme` + its own props. Apps wrapping a subtree in a new theme use `widget::Themer`, which is functionally inheritance-via-scoping but explicit, not implicit.

No `color: inherit`, no `font-family: inherit`, no `--custom-prop: ...` variables. The substitute is: the app passes shared values into widgets at construction time, or theme variants encode them.

## No design-token system

Iced has no equivalent of `color.surface.primary` / `space.4` / `radius.md` / `motion.fast` tokens. Style closures read from `theme.palette()` (which exposes ~6 semantic colors) and write raw values (pixel paddings, hex colors, `border::rounded(4)`).

For Iced apps that want a token discipline, libcosmic adds one above Iced — `cosmic_theme::Theme` exposes a much wider semantic surface (accent, success, warning, destructive, neutrals 0-10, palette ramps). Apps consuming libcosmic see tokens; apps consuming raw Iced see palette + ad-hoc constants.

## Animation primitives

Pre-0.14 Iced had only `Subscription::frame()` (a 60Hz tick) — apps animated by mutating state on each tick. 0.14 added a dedicated **Animation API**:

```rust
// state holds an Animation<Message>
struct State { sidebar_offset: Animation<f32> }
// view interpolates current value
sidebar_offset.interpolate(0.0, 200.0, now)
// update advances on tick or messages
```

Spring-style and tweened animations, both reduced-motion-honoring via the OS-pref system if the app surfaces it (not automatic). Compared to bevy_animation or CSS transitions, this is a thin layer — no keyframes, no easing curve library, no layout transitions (animating between layout solutions).

## Implications for Buiy

1. **The widget catalog is the right shape to plan against.** Iced's 40+ widgets cover roughly the WAI-ARIA APG patterns Buiy commits to ([media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) — minus the productivity-app pieces (tree, virtualized list, true modal, drag-and-drop). Buiy's widget-catalog sub-spec should compare per-widget keyboard contracts against Iced's; Iced is the proxy for "what shipping retained-mode Rust GUI provides."
2. **Function-based styling is not a token system.** Iced's `style: fn(&Theme, Status) -> Style` is type-safe and ergonomic for *application* code, but a 50-widget app accumulates style closures linearly. Buiy's commitment to **semantic tokens** ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the right answer for design-system-driven UIs; Iced's pattern is too low-level for a comprehensive UI library. libcosmic's add-on layer above Iced demonstrates the gap.
3. **Iced has no CSS cascade.** Buiy's commitment to no-cascade-but-tokens ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) is validated by Iced: production apps work fine without a cascade if the alternative (tokens + explicit composition) is well-designed.
4. **No AccessKit means no a11y.** Iced 0.14 has no `aria-label`, no role, no AccessKit tree, no screen-reader contract. This is the structural reason Iced is not a viable productivity-app library — Halloy, Icebreaker, COSMIC apps all ship without screen-reader support. Buiy's AccessKit-first stance ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the right gap to close, and Iced is the cautionary tale, not the model. Cross-link: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Avoid.
5. **The widget-state reconciliation mechanism (`Tree` + `tree::Tag`) is novel.** Buiy doesn't need it (entities persist), but it's worth understanding for the brainstorming-skill — it's what every retained-mode-with-rebuild GUI ends up with.
6. **Animation primitives are weak.** Buiy plans dedicated animation primitives ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md), `buiy-animation-design`); Iced's animation API is in-flight (0.14) and not as expressive as e.g. CSS animations + transitions. Don't model Buiy animation on Iced.

## Sources

- iced::widget docs — https://docs.rs/iced/0.14.0/iced/widget/index.html
- Iced 0.14.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.14.0
- Iced 0.13.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.13.0
- PR #2312 (Functional widget styling) — https://github.com/iced-rs/iced/pull/2312
- PR #2350 (Class-based theming) — https://github.com/iced-rs/iced/pull/2350
- iced::advanced::Widget — https://docs.rs/iced/0.14.0/iced/advanced/widget/trait.Widget.html
- libcosmic theme — https://github.com/pop-os/libcosmic/tree/master/cosmic-theme
- Buiy foundation media-and-widgets — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
