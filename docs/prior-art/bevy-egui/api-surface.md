**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — the egui widget vocabulary, layout primitives, styling, and a11y surface

# API surface

bevy_egui doesn't define a widget vocabulary — it forwards egui's. This file documents what egui itself ships, since that's the API surface Buiy designers are comparing against. The bevy_egui side is the `EguiContexts` system-param and the per-camera `EguiContext` component; everything beyond that is upstream egui.

## Acquiring a `&mut egui::Ui`

The standard pattern in a Bevy system:

```rust
fn ui_system(mut contexts: EguiContexts) {
    let ctx = contexts.ctx_mut().unwrap();   // primary context
    egui::Window::new("Inspector").show(ctx, |ui| {
        // ui: &mut egui::Ui  — everything below is on this
    });
}
```

`egui::Ui` is the layout-and-emission cursor. Every widget call takes `&mut Ui` and returns a `Response`.

## Widget vocabulary

egui's built-ins, grouped by purpose. Type names live in `egui::` unless noted.

**Atoms.**
- `Label` — non-interactive text.
- `Hyperlink` — clickable text that opens a URL via `egui::output().open_url(...)`.
- `Image::new(texture_id, size)` — display an image. Texture ID comes from `EguiUserTextures` for app-supplied images, or `egui::include_image!(...)` for embedded.
- `Spinner` — indeterminate progress.
- `Separator` — horizontal/vertical rule.

**Buttons.**
- `Button::new(text)` — push button. `.show(ui)` returns `Response`; `response.clicked()`, `response.hovered()`, `response.double_clicked()`, `response.dragged()`.
- `ImageButton::new(image)` — button wrapping an `Image`.
- `Checkbox::new(&mut bool, text)` — two-way bound to a `bool` the caller holds.
- `RadioButton::new(selected, text)`; `ui.radio_value(&mut state, value, label)` is the common helper.
- `SelectableLabel` — toggle styled as a label.

**Numeric & text input.**
- `Slider::new(&mut f32, range)` — slider with optional logarithmic scaling and clamp behavior.
- `DragValue::new(&mut value)` — numeric field that scrubs on horizontal drag (the dev-tools idiom).
- `TextEdit::singleline(&mut String)` and `TextEdit::multiline(&mut String)` — text inputs. IME composition is handled internally by egui via `EguiInput::events::Ime`.
- `Color32::picker` via `ui.color_edit_button_*` family.

**Containers / layouts.**
- `Window::new(title).show(ctx, |ui| { ... })` — movable, resizable, optionally collapsible floating panel. Position survives frames via the `Id` system (see [immediate-mode-paradigm.md § id system](immediate-mode-paradigm.md)).
- `Area::new(id).show(ctx, |ui| { ... })` — floating region without window chrome.
- `SidePanel::left/right(id)`, `TopBottomPanel::top/bottom(id)`, `CentralPanel::default()` — dock-style panels.
- `ScrollArea::vertical()` / `horizontal()` / `both()` — scrollable region with virtualization helpers (`ScrollArea::show_rows` for known-height homogeneous lists).
- `Grid::new(id).num_columns(n).show(ui, |ui| { ... })` — aligned grid.
- `CollapsingHeader::new(title).show(ui, |ui| { ... })` — disclosure widget.
- `ComboBox::from_label(label).selected_text(...).show_ui(ui, |ui| { ... })` — single-select dropdown.

**Layout direction primitives.**
- `ui.horizontal(|ui| { ... })` / `ui.horizontal_top(...)` / `ui.horizontal_wrapped(...)`.
- `ui.vertical(|ui| { ... })` / `ui.vertical_centered(...)`.
- `ui.columns(n, |columns| { columns[0].label(...); columns[1].button(...); })`.
- `ui.allocate_ui_with_layout(size, layout, |ui| { ... })` — for custom alignment / cross-axis behavior via `egui::Layout`.

**Other.**
- `Tooltip` — attach via `response.on_hover_text(...)` / `.on_hover_ui(|ui| { ... })`.
- `Popup` — `egui::popup_below_widget`.
- `Menu` — `ui.menu_button(label, |ui| { ... })`.

## Composition pattern

Containers take a closure that receives a sub-`Ui`. The closure runs eagerly, emitting widgets into the container. There is no "build a tree, then `.show()` it" — `.show()` *is* the build.

```rust
egui::Window::new("Settings")
    .resizable(true)
    .collapsible(true)
    .show(ctx, |ui| {
        ui.heading("Audio");
        ui.horizontal(|ui| {
            ui.label("Volume");
            ui.add(egui::Slider::new(&mut volume, 0.0..=1.0));
        });
    });
```

The whole tree exists only inside the closure call. Next frame, it's emitted again.

## Styling: `Style`, `Visuals`, `Spacing`

egui's style is a single `egui::Style` struct (held on `Context`) containing:
- `visuals: Visuals` — colors, fills, strokes, shadows, separator color, hyperlink color, selection fill, `WidgetVisuals` per state (inactive, hovered, active, open, noninteractive).
- `spacing: Spacing` — item spacing, button padding, slider width, indent, scroll bar width, window margin.
- `text_styles: BTreeMap<TextStyle, FontId>` — font sizing by style (`Body`, `Heading`, `Monospace`, `Button`, `Small`, plus user-defined).
- `interaction`, `animation_time`, `wrap`, `explanation_tooltips`, etc.

Theming = mutate `ctx.style_mut()` once at startup or on toggle. Built-in `Visuals::dark()` and `Visuals::light()` are the two stock themes. There's no token system, no semantic colors — direct RGBA, occasionally with `Color32::from_rgba_unmultiplied(...)`.

## Themes: dark / light

The README ships both, with `Visuals::dark()` as the default. Switching is `ctx.set_visuals(egui::Visuals::light())`. There's no `prefers-color-scheme` OS integration in egui core; applications that want it wire it manually.

## Custom painting

`ui.painter()` returns a `Painter` that draws raw shapes — lines, rectangles, circles, convex polygons, antialiased strokes, text via `paint.text(pos, anchor, text, font_id, color)`. Used for plots, custom indicators, in-widget visualization. `egui_plot` (now a separate crate, was upstream) and other community packages build on this.

For ECS-side custom rendering (e.g. egui paints a marker that should map back to a Bevy entity), use a `egui::PaintCallback` carrying a custom shader — bevy_egui forwards these to a wgpu pass.

## Accessibility: AccessKit integration

egui ships an optional AccessKit integration (`accesskit` cargo feature on egui; `accesskit` feature on bevy_egui). When enabled, egui builds an `accesskit::TreeUpdate` from the widget tree during the frame and exposes it via `ctx.accesskit_update_if_active()`. bevy_egui's accesskit feature (re-enabled in 0.38.0 after a hiatus) routes this update through `bevy_a11y`.

Important caveats:
- **Off by default in bevy_egui.** The 0.38.0 changelog: "AccessKit support re-enabled (disabled by default, requires `accesskit` feature)."
- **Platform coverage.** egui's AccessKit covers Windows (UI Automation) and macOS (NSAccessibility). Linux AT-SPI is via `accesskit_unix` which depends on the upstream AccessKit Linux work; web is via the AccessKit web adapter.
- **Tree shape.** Because widgets don't persist, the AccessKit `NodeId`s are derived from egui's same call-site `Id` system. Stable across frames if your widget layout is stable; collide-and-confuse if you have dynamic content without `push_id`.
- **APG conformance** (WAI-ARIA Authoring Practices). egui's widgets implement keyboard contracts ad-hoc — they're "good enough" for dev tools but not a conformant APG widget set. Tab order works; complex roving-tabindex / `aria-activedescendant` patterns are not first-class.

Implications for Buiy: egui+AccessKit proves the cross-platform AT bridge works in a Bevy app today; what egui does *not* prove is that a comprehensive APG widget catalog ([media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) is achievable on top of immediate-mode. Buiy's per-widget APG contract goal needs persistent nodes — see [immediate-mode-paradigm.md § when retained-mode wins](immediate-mode-paradigm.md).

## Touch / gamepad / virtual keyboard

- **Touch.** egui receives Bevy `TouchInput` events translated into pointer events with `PointerType::Touch`. Multi-touch is partial — gestures (pinch, rotate) work via `egui::MultiTouchInfo` on canvas-style widgets that opt in. Most stock egui widgets ignore secondary touches.
- **Mobile virtual keyboard.** bevy_egui 0.35.0 added support, currently "rough around the edges" per the README. egui requests the keyboard via `egui::output().mutable_text_under_cursor`; on web this triggers the OS virtual keyboard, on native this works only if the platform layer surfaces it.
- **Gamepad.** No native support in egui core. Common pattern: convert Bevy `Gamepad` events into synthetic `KeyEvent` (DPad-up → `ArrowUp`, A-button → `Enter`) before they hit `EguiInput`. Spatial navigation across egui widgets is not first-class.

## Other API surfaces worth flagging

- **`egui_extras`** — companion crate from emilk with `Table`, `DatePickerButton`, `Image::from_uri` async loading, and other extensions that didn't fit upstream.
- **`egui_plot`** — formerly in-tree, now separate. 2D plotting (lines, points, bars, polygons) on `Plot::new(id).show(ui, |plot_ui| { ... })`.
- **Persistence.** `serde` feature on egui makes `Memory` serializable — apps that want window positions to survive process restart serialize the memory blob on shutdown and restore on startup. egui ships this; bevy_egui does not wrap it explicitly.

See [integration.md](integration.md) for setup mechanics, [use-cases.md](use-cases.md) for what this surface is good at, [architecture.md](architecture.md) for how Bevy and egui exchange data each frame.

## Sources

- egui crate — https://docs.rs/egui/latest/egui/
- egui README — https://github.com/emilk/egui
- bevy_egui CHANGELOG (0.35.0 — AccessKit re-enable, virtual-keyboard, 0.38.0 — AccessKit feature gate) — https://raw.githubusercontent.com/vladbat00/bevy_egui/main/CHANGELOG.md
- egui_extras — https://docs.rs/egui_extras/latest/egui_extras/
- egui_plot — https://docs.rs/egui_plot/latest/egui_plot/
- AccessKit — https://accesskit.dev
- Sibling files: [architecture.md](architecture.md), [integration.md](integration.md), [immediate-mode-paradigm.md](immediate-mode-paradigm.md), [use-cases.md](use-cases.md)
