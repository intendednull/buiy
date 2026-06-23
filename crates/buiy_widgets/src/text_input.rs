//! `TextInput` widget (editing-and-ime § 2.3). Composes the `buiy_core` editor
//! mechanism (`TextEditState` + markers + the display `Text` carrier) with
//! widget policy: catalog sizes/tokens, focusable + a11y, submit-on-Enter (the
//! `SingleLine` marker drives `EditCommand::Submit`).
//!
//! Focus-on-click is NOT a widget-specific observer here: C3d
//! (input-event-model.md § 2.7) consolidated focus-on-click into one
//! widget-agnostic `buiy_core::focus::focus_on_click` observer over every
//! `Focusable`. The `TextInput` `#[require]`s `Focusable`, so a primary press
//! focuses it through that shared path — the widget carries no focus observer of
//! its own.
//!
//! `buiy_widgets` names NO cosmic type — `TextEditState::for_font_size` is the
//! seam (the facade boundary the campaign guards). Mirrors `Button::new`
//! (`button.rs`).

use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yRole},
    components::Node,
    focus::Focusable,
    layout::{BoxModel, Overflow, Style},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius, TextColor},
    text::edit::{Placeholder, SingleLine, TextEditState},
    text::{FontSize, Text},
};
use std::borrow::Cow;

/// The catalog font size for a text input (logical px). Matches the `Button`
/// hardcoded-size convention (TODO: size tokens — buiy-widget-catalog-design).
/// `pub(crate)` so the `scene` module's text-input scene-fns spell the same
/// font size as the `#[require]` initializer.
pub(crate) const TEXT_INPUT_FONT_SIZE: f32 = 16.0;

/// Marker for a text-input widget (the `Button` precedent). Carried so a11y
/// can identify the widget; focus-on-click is the shared
/// `buiy_core::focus::focus_on_click` over the `#[require]`'d `Focusable`.
///
/// The `#[require(...)]` contract makes the bare marker
/// (`world.spawn(TextInput)` / `bsn! { TextInput }`) materialize the editor
/// mechanism + the display `Text` carrier + the layout-visible `Style`
/// decomposition + paint + focus + a11y — everything the constructors used to
/// assemble by hand (spec § 4.1a; the `Button` precedent). `single_line()`
/// layers the `SingleLine` policy marker on top; `SingleLine` is therefore NOT
/// required by the base marker.
///
/// `Node` (required) pulls the full `Style` decomposition, so the input is
/// layout-visible without re-spelling those; the two style **overrides** —
/// `BoxModel` (the 200×32 / 8px box) and `Overflow` (`overflow: hidden`, so the
/// content clips and auto-scroll has a scroll container) — are direct requires.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    // The two Style overrides (the rest of the decomposition rides `Node`).
    BoxModel = text_input_box_model(),
    Overflow = text_input_overflow(),
    // Paint companions.
    Background = text_input_background(),
    Border = text_input_border(),
    // Editor mechanism + display carrier. `Text("")` is the required display
    // `TextBuffer` carrier (editor-optional / buffer-required: the
    // editor-owned buffer is authoritative, but the entity still needs a
    // `Text` so TextSync runs and the node measures).
    Text,
    FontSize = FontSize(TEXT_INPUT_FONT_SIZE),
    TextColor,
    TextEditState = TextEditState::for_font_size(TEXT_INPUT_FONT_SIZE),
    Placeholder,
    Focusable,
    // The Phase-0 A11yRole taxonomy stops at `Text` (no `TextInput`/
    // `TextField` variant yet). Use `Text`; the full role taxonomy is
    // buiy-accessibility-design's, and a `TextInput` role is a clean additive
    // follow-up there.
    A11yRole = A11yRole::Text,
    A11yLabel,
)]
pub struct TextInput;

// The initializer fns below are `pub(crate)` so the `scene` module's
// text-input scene-fns spell the SAME canonical values as `bsn!` field-patches
// — one source of truth shared with the `#[require]` initializers.

/// The canonical text-input box: 200×32 logical px, 8px padding. >=24×24 meets
/// WCAG 2.5.8.
// TODO(buiy-widget-catalog-design): size tokens.
pub(crate) fn text_input_box_model() -> BoxModel {
    Style::default()
        .width_px(200.0)
        .height_px(32.0)
        .padding(8.0)
        .box_model
}

/// `overflow: hidden` so the content clips and the auto-scroll `ScrollOffset`
/// has a scroll container to pan (§ 9).
pub(crate) fn text_input_overflow() -> Overflow {
    Style::default().overflow_hidden().overflow
}

/// The default input surface fill (`color.surface.secondary`).
pub(crate) fn text_input_background() -> Background {
    Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
    }
}

/// The default input border: rounded corners, no painted line.
pub(crate) fn text_input_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(6.0)),
        ..Default::default()
    }
}

impl TextInput {
    /// A single-line text input with a placeholder (Enter ⇒ Submit, `Wrap::None`
    /// — the `SingleLine` policy). Returns `impl Bundle` (the `Button::new`
    /// precedent). The `#[require]` contract supplies the editor/style/a11y
    /// companions; this layers the placeholder string and the `SingleLine`
    /// policy marker on top.
    pub fn single_line(placeholder: impl Into<String>) -> impl Bundle {
        (TextInput, Placeholder(placeholder.into()), SingleLine)
    }

    /// A multi-line text input with a placeholder (Enter inserts a newline).
    /// The bare `TextInput` contract is already multi-line (no `SingleLine`),
    /// so this only layers the placeholder string.
    pub fn multi_line(placeholder: impl Into<String>) -> impl Bundle {
        (TextInput, Placeholder(placeholder.into()))
    }
}
