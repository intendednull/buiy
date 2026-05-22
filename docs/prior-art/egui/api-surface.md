**Date:** 2026-05-22
**Status:** active
**Subject:** egui — the widget vocabulary, layout primitives, custom-widget hooks, and input affordances

# API surface

This file inventories what egui actually offers a user code to call. The architectural foundations are in [`architecture.md`](architecture.md); the conceptual story is in [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md). This file is the catalog.

## Widget vocabulary

egui's built-in widget set is intentionally compact — Emil's design philosophy ([`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md)) is "minimal, easy to write your own." The shipping widgets:

**Atomic interactive widgets:**

- `Button` — click target with label and optional icon. Returns `Response`; check `.clicked()` / `.double_clicked()` / `.secondary_clicked()` / `.hovered()`.
- `ImageButton` — button with an image instead of a text label.
- `Checkbox` — boolean toggle. Constructed via `ui.checkbox(&mut state, "Label")`.
- `RadioButton` — single-value radio. Use `ui.radio_value(&mut state, Value::A, "A")` for a group.
- `Slider` — numeric slider. `ui.add(egui::Slider::new(&mut value, 0.0..=1.0).text("Volume"))`.
- `DragValue` — number-input-by-dragging plus type-to-edit. `ui.add(egui::DragValue::new(&mut value))`.
- `ComboBox` — dropdown. `egui::ComboBox::from_label("Choice").selected_text(...).show_ui(ui, |ui| { ... })`.
- `ProgressBar` — non-interactive 0..1 progress display.
- `Spinner` — animated busy indicator.
- `Hyperlink` — clickable URL; emits a `Hyperlink` action in `PlatformOutput.open_url`.
- `Label` — non-interactive text. `ui.label("text")` is the common shorthand.
- `Image` — display a texture. Takes an `egui::ImageSource` (URL, raw bytes via `egui_extras::image::*`, or a managed `TextureHandle`).
- `SelectableLabel` — clickable label that can be highlighted (the building block for tab strips, list selection, etc.).
- `Separator` — visual divider line.

**Text input:**

- `TextEdit::singleline(&mut string)` — single-line text input.
- `TextEdit::multiline(&mut string)` — multi-line text area.
- Both support: IME composition, undo/redo, selection, clipboard cut/copy/paste, password mode (`.password(true)`), font/style override, hint text, character filter, desired width/rows. See [`text-rendering.md`](text-rendering.md) for the underlying text-shaping pipeline.

**Plot widget** (in the separate `egui_plot` crate, not core egui):

- `egui_plot::Plot` — interactive 2D plot with lines, points, bars, polygons, text, hover tooltips, pan/zoom. The de facto standard for "I need a graph in my Rust app."
- 6,765,876 total downloads at last check; lives in its own repository `emilk/egui_plot`.

## Container widgets

Containers create a new `Ui` scope. The closure pattern is uniform: `container.show(ui, |ui| { ... })`.

- `Window` — floating, draggable, resizable window. Persistent position/size stored in `Memory`. Multi-window via `Window::new("Title")`.
- `Area` — bare positioned region without window chrome. Building block for tooltips, popups, drag overlays.
- `Panel` — the unified panel API (0.34.0 consolidated the older `SidePanel` / `TopBottomPanel` / `CentralPanel` into a single `Panel` type with directional config; the old types still work as aliases).
- Prior to 0.34.0: `SidePanel::left("id")`, `SidePanel::right("id")`, `TopBottomPanel::top("id")`, `TopBottomPanel::bottom("id")`, `CentralPanel::default()`. All four still present in 0.34.x as legacy aliases.
- `ScrollArea` — clipped scrollable region. `ScrollArea::vertical()` / `::horizontal()` / `::both()`. Supports virtualization via `.show_rows(...)` (the API egui uses internally for the demo's huge tables).
- `CollapsingHeader` — disclosure triangle + collapsible content. Open state persisted in `Memory`.
- `Grid::new("id").show(ui, |ui| { ... })` — fixed-column grid layout. Useful for label/value pairs.
- `Frame` — visual decoration (background, border, shadow, padding) around its content. Composes with any layout.

## Layout

Layout is closure-based. Inside a closure, the passed-in `&mut Ui` is a new layout cursor with bounded extent.

- `ui.horizontal(|ui| { ... })` — left-to-right.
- `ui.vertical(|ui| { ... })` — top-to-bottom (this is also the default within a `Ui`).
- `ui.horizontal_wrapped(|ui| { ... })` — like horizontal but wraps to next line when out of room.
- `ui.vertical_centered(|ui| { ... })` — vertical with horizontally-centered children.
- `ui.columns(N, |cols: &mut [Ui]| { ... })` — split into N equal-width columns; receive an array of child UIs.
- `ui.allocate_ui_with_layout(size, layout, |ui| { ... })` — full layout control via `egui::Layout` (main_dir, cross_dir, main_align, main_justify, cross_align, etc.).
- `ui.allocate_ui_at_rect(rect, |ui| { ... })` — explicit pixel rectangle (escape hatch for custom positioning).
- `ui.add_space(pixels)` — insert empty space.
- `ui.indent(id, |ui| { ... })` — visual indentation block.

Layout primitives are simple by design. There is no Flexbox, no CSS Grid, no `aspect-ratio`, no `min-content` / `max-content` / `fit-content`. The "layout language" is horizontal / vertical / columns. For game-tool UIs this is sufficient; for production app UIs it's a structural limit.

## Styling

Styling configuration lives on `Context`:

- `ctx.set_style(style)` — wholesale replacement of the visual config.
- `ctx.style_mut(|s| { ... })` — incremental mutation.
- `egui::Style` — the master config. Contains `Visuals`, `spacing`, `interaction`, `text_styles`, `wrap_mode`, `animation_time`.
- `egui::Visuals` — color palette + visual treatment. `Visuals::dark()` / `Visuals::light()` are the two shipped presets.
- `egui::TextStyle` — named text styles (`Body`, `Heading`, `Monospace`, `Button`, `Small`, `Name(custom)`). Each maps to a `FontId` (size + family).

Per-widget style override via `Style` mutation in a scope:

```rust
ui.scope(|ui| {
    ui.style_mut().visuals.override_text_color = Some(egui::Color32::RED);
    ui.label("This is red");
});
```

See [`styling-and-theming.md`](styling-and-theming.md) for the full theming model and its limits.

## Painting

`ui.painter()` returns a `Painter` for raw shape drawing:

- `painter.rect_filled(rect, rounding, color)`
- `painter.rect_stroke(rect, rounding, stroke)`
- `painter.circle_filled(center, radius, color)`
- `painter.line_segment([a, b], stroke)`
- `painter.text(pos, anchor, text, font_id, color)`
- `painter.image(texture_id, rect, uv, tint)`
- `painter.add(shape)` — for arbitrary `epaint::Shape`.

The painter outputs directly into the current frame's `Vec<ClippedShape>`. This is the escape hatch for custom widgets that need arbitrary shape drawing — graphs, diagrams, in-place mini-renderers, debug visualizations.

## Custom widgets

Two patterns:

**Pattern 1: implement `Widget` trait**

```rust
impl egui::Widget for MyWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let size = egui::vec2(80.0, 20.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, 4.0, egui::Color32::DARK_BLUE);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, &self.label,
                              egui::FontId::default(), egui::Color32::WHITE);
        }
        response
    }
}

// Use:
ui.add(MyWidget { label: "Hi".to_owned() });
```

**Pattern 2: free function returning `Response`**

```rust
fn my_widget(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    let response = ui.add(egui::Slider::new(value, 0.0..=1.0));
    // ... do additional things, paint extra decoration, etc.
    response
}
```

The `Widget` trait + `Response` return-value convention is the entire custom-widget API. Compared to a retained-mode framework where custom widgets need lifecycle management, state coordination, accessibility tree contribution, and theme integration, this is dramatically simpler — and the lack of those concerns is the immediate-mode trade.

## Response, Sense, and interaction

Every widget call returns `egui::Response`:

```rust
pub struct Response {
    pub ctx: Context,
    pub id: Id,
    pub rect: Rect,
    pub sense: Sense,
    // (private fields: clicked, hovered, dragged, etc.)
}
```

`Response` has accessor methods: `.clicked()`, `.double_clicked()`, `.triple_clicked()`, `.secondary_clicked()`, `.middle_clicked()`, `.hovered()`, `.dragged()`, `.drag_delta()`, `.drag_released()`, `.has_focus()`, `.lost_focus()`, `.changed()`, `.context_menu(|ui| { ... })`.

A widget allocates `Sense::click()`, `Sense::drag()`, `Sense::hover()`, or `Sense::click_and_drag()` at `allocate_exact_size` time, and egui matches input events against the widget's rect + sense to set the `Response` flags.

`Response::total_drag_delta` and `PointerState::total_drag_delta` (0.33.2) give absolute cumulative drag distance, useful for "did the user drag at all" vs "instantaneous frame delta."

## Touch / gamepad

egui's input model is primarily mouse + keyboard. Touch is supported (`egui::TouchPhase` events, multi-touch via `PointerState::primary_touch`), but the widget vocabulary doesn't have first-class touch-only patterns (no native swipe-to-dismiss, no native pull-to-refresh).

**Gamepad support is essentially absent.** egui has no built-in gamepad input pipeline. Apps that need gamepad navigation either:

1. Translate gamepad input into synthetic keyboard arrow events (the cheapest hack).
2. Build a custom focus-walking system on top of egui's `Memory::focus`.
3. Use a retained-mode UI for the gamepad-driven parts and reserve egui for the mouse-driven dev panels.

For Buiy: gamepad spatial navigation is in scope ([`../../specs/2026-05-07-buiy-foundation/interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)) — Buiy will not inherit this gap from egui.

Mobile virtual-keyboard support exists in bevy_egui 0.35.0+ but is documented as "rough around the edges." On-device mobile usage of egui is a real but minority workflow.

## Text input details

`TextEdit` is the workhorse:

```rust
let response = ui.add(
    egui::TextEdit::multiline(&mut self.text)
        .hint_text("Type here…")
        .desired_rows(10)
        .desired_width(f32::INFINITY)
        .lock_focus(true)
        .password(false)
        .font(egui::TextStyle::Monospace)
);
if response.changed() {
    // text edit handler
}
```

What it supports:

- Single-line and multi-line text input.
- Selection (mouse drag + keyboard shift-arrows).
- Cut/copy/paste via `PlatformOutput.copied_text` + `RawInput.events::Paste`.
- Undo/redo with bounded history.
- IME composition — preedit underlining + commit. The IME story shipped iterative fixes in 0.33 / 0.34 (backspace bug on macOS + Safari fixed in 0.34.0).
- Password mode (renders dots; clipboard read suppressed).
- Character filter (`.char_limit(N)`).

What it does not support:

- Rich text editing (bold, italic, inline formatting embedded in the editable buffer). egui's text is single-style per `TextEdit`.
- Full IME conformance for complex scripts (Arabic shaping, Indic conjuncts). See [`text-rendering.md`](text-rendering.md).
- Spell-check. No OS spell-checker integration.
- BiDi caret movement following UAX #9 fully. Basic LTR/RTL works for the common cases.
- Suggestion popups (e.g. autocomplete, address completion). Users build these manually with a positioned `Area`.

For Buiy: cosmic-text + custom editor surface ([`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)) deliberately exceeds egui's text-editing capability.

## Platform output

The platform side-effects egui requests at end-of-frame, packaged into `PlatformOutput`:

- `cursor_icon: CursorIcon` — what cursor the OS should show.
- `open_url: Option<OpenUrl>` — a hyperlink the host should open.
- `copied_text: String` — text to write to the clipboard.
- `events: Vec<OutputEvent>` — accessibility events (focus moved, value changed, click triggered). Consumed by AccessKit.
- `mutable_text_under_cursor: bool` — hint for OS-IME mode.
- `ime: Option<IMEOutput>` — IME positioning info (caret rect for the OS-IME composition window).
- `num_completed_passes` — info for the host about whether multi-pass settled.
- `request_discard_reasons` — debug info on why a pass was discarded.

The host backend applies these. eframe handles them all; bevy_egui handles most (it sets cursor, writes clipboard, opens URLs, forwards IME positioning to Bevy's IME plumbing).

## What egui's API is missing

Vs the web platform (the Buiy target):

- **No `aria-*` attributes** at the widget level. Accessibility role/name/state is inferred from widget type; finer-grained ARIA control needs to drop into `accesskit::Node` construction manually.
- **No CSS-style cascade.** Styling is per-`Ui`-scope or global; no inheritance, no `:hover` / `:focus-visible` selector mechanism (`Visuals` has `widgets.hovered`, `widgets.active`, etc. but these are global, not selector-driven).
- **No animations as first-class.** `animate_bool` and `animate_value_with_time` cover basic interpolation; complex animations (curves, springs, keyframes, layout transitions) are not provided.
- **No virtualization at scale.** `ScrollArea::show_rows` virtualizes rows but the rest of the UI rebuilds every frame regardless.
- **No declarative layout language** comparable to Flexbox/Grid. The `Layout` struct configures `main_dir` and alignment; complex layouts compose by nesting closures.
- **No form-validation model.** Constraint validation, `:invalid` / `:user-invalid` analogues, error message regions — all hand-rolled per app.
- **No drag-and-drop UI primitives** at the level of HTML5 drag-drop. Building a drag-drop list works via `Response::dragged()` + manual positioning.
- **No date/time picker** in core (one exists in `egui_extras::DatePickerButton`).
- **No combo-box with search / autocomplete** in core. Common community pattern is to roll your own with `TextEdit` + a manually-positioned `Area` of `SelectableLabel`s.

These omissions are coherent with the README's non-goals ("Become the most powerful GUI library"). They are also why egui is the wrong substrate for Buiy's targets.

## See also

- [`architecture.md`](architecture.md) — how `Response`, `Ui`, `Context`, and `FullOutput` fit together internally.
- [`immediate-mode-deep-dive.md`](immediate-mode-deep-dive.md) — why the API is shaped this way.
- [`styling-and-theming.md`](styling-and-theming.md) — visuals, themes, customization limits.
- [`text-rendering.md`](text-rendering.md) — the text pipeline behind `Label`, `TextEdit`, `painter.text`.
- [`../bevy-egui/api-surface.md`](../bevy-egui/api-surface.md) — how the API surface is consumed through Bevy.

## Sources

- egui rustdoc — https://docs.rs/egui/latest/egui/
- `egui::Widget` rustdoc — https://docs.rs/egui/latest/egui/trait.Widget.html
- `egui::Response` rustdoc — https://docs.rs/egui/latest/egui/struct.Response.html
- `egui::TextEdit` rustdoc — https://docs.rs/egui/latest/egui/widgets/struct.TextEdit.html
- `egui::ScrollArea` rustdoc — https://docs.rs/egui/latest/egui/containers/scroll_area/struct.ScrollArea.html
- egui CHANGELOG — https://raw.githubusercontent.com/emilk/egui/master/CHANGELOG.md
- egui_plot crate — https://crates.io/crates/egui_plot
- egui_extras crate — https://crates.io/crates/egui_extras
- Buiy foundation interaction spec — [`../../specs/2026-05-07-buiy-foundation/interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)
- Buiy foundation text spec — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
