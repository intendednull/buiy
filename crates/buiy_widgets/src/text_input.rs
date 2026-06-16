//! `TextInput` widget (editing-and-ime § 2.3). Composes the `buiy_core` editor
//! mechanism (`TextEditState` + markers + the display `Text` carrier) with
//! widget policy: catalog sizes/tokens, focusable + a11y, submit-on-Enter (the
//! `SingleLine` marker drives `EditCommand::Submit`), and focus-on-click.
//!
//! `buiy_widgets` names NO cosmic type — `TextEditState::for_font_size` is the
//! seam (the facade boundary the campaign guards). Mirrors `Button::new`
//! (`button.rs`).

use bevy::prelude::*;
use buiy_core::FocusedEntity;
use buiy_core::{
    a11y::{A11yLabel, A11yRole},
    components::Node,
    focus::Focusable,
    layout::Style,
    picking::Hovered,
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius, TextColor},
    text::edit::{Placeholder, SingleLine, TextEditState},
    text::{FontSize, Text},
};
use std::borrow::Cow;

/// Marker for a text-input widget (the `Button` precedent). Carried so
/// `focus_on_click` and a11y can identify the widget.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct TextInput;

/// The catalog font size for a text input (logical px). Matches the `Button`
/// hardcoded-size convention (TODO: size tokens — buiy-widget-catalog-design).
const TEXT_INPUT_FONT_SIZE: f32 = 16.0;

impl TextInput {
    /// A single-line text input with a placeholder (Enter ⇒ Submit, `Wrap::None`
    /// — the `SingleLine` policy). Returns `impl Bundle` (the `Button::new`
    /// precedent), so callers get the full editor contract without assembling
    /// it.
    pub fn single_line(placeholder: impl Into<String>) -> impl Bundle {
        (base_bundle(placeholder), SingleLine)
    }

    /// A multi-line text input with a placeholder (Enter inserts a newline).
    pub fn multi_line(placeholder: impl Into<String>) -> impl Bundle {
        base_bundle(placeholder)
    }
}

/// The shared composition: editor + display carrier + node/style + a11y +
/// focusable + tokens. `single_line` adds `SingleLine` on top.
fn base_bundle(placeholder: impl Into<String>) -> impl Bundle {
    let placeholder = placeholder.into();
    (
        TextInput,
        Node,
        // TODO(buiy-widget-catalog-design): size tokens. 200x32 is a typical
        // single-line input; >=24x24 meets WCAG 2.5.8. Overflow-hidden so the
        // content clips (and the auto-scroll ScrollOffset has a scroll
        // container to pan — § 9).
        Style::default()
            .width_px(200.0)
            .height_px(32.0)
            .padding(8.0)
            .overflow_hidden(),
        Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
        },
        Border {
            radius: Corners::all(Radius::circular(6.0)),
            ..Default::default()
        },
        // The editor mechanism + its display carrier. `Text("")` is the
        // required display `TextBuffer` carrier (editor-optional /
        // buffer-required — the editor-owned buffer is authoritative, but the
        // entity still needs a `Text` so TextSync runs and the node measures).
        Text(String::new()),
        FontSize(TEXT_INPUT_FONT_SIZE),
        TextColor::default(),
        TextEditState::for_font_size(TEXT_INPUT_FONT_SIZE),
        Placeholder(placeholder),
        Focusable::default(),
        // The Phase-0 A11yRole taxonomy stops at `Text` (no `TextInput`/
        // `TextField` variant yet — verified: a11y/mod.rs). Use `Text`; the
        // full role taxonomy is buiy-accessibility-design's, and a `TextInput`
        // role is a clean additive follow-up there.
        A11yRole::Text,
        A11yLabel(String::new()),
    )
}

/// Widget-side focus-on-click (editing-and-ime § 2.3 / Borrow #7 — focus is
/// WIDGET policy, never core auto-focus). On a left mouse-down over a hovered
/// `TextInput`, set `FocusedEntity`. Mirrors `emit_on_press_on_click`
/// (`button.rs`): `Option` params so a partial harness no-ops.
pub fn focus_on_click(
    hovered: Option<Res<Hovered>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    inputs: Query<(), With<TextInput>>,
    focused: Option<ResMut<FocusedEntity>>,
) {
    let (Some(hovered), Some(mouse), Some(mut focused)) = (hovered, mouse, focused) else {
        return;
    };
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(entity) = hovered.0 else { return };
    if inputs.get(entity).is_ok() {
        focused.0 = Some(entity);
    }
}
