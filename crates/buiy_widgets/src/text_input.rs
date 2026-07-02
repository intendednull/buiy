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
    a11y::{A11yLabel, A11yPlaceholder, A11yRole, A11yTextValue},
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
    // The role split IS the multiline distinction (widget-contracts.md §5):
    // `TextInput` → `Role::TextInput` (single-line) vs `MultilineTextInput` →
    // `Role::MultilineTextInput`. The BARE marker carries NO `SingleLine`, so it
    // is multi-line — hence the `#[require]` default is `MultilineTextInput`;
    // `single_line()` / the `text_input_single_line` scene-fn layer the
    // single-line role (`A11yRole::TextInput`) on top alongside the `SingleLine`
    // policy marker. (Retires the old `A11yRole::Text` stopgap — the role
    // taxonomy now has the two text-input variants.)
    A11yRole = A11yRole::MultilineTextInput,
    A11yLabel,
    // Synced state: `A11yTextValue` mirrors the editor's live value (the
    // `sync_text_input_a11y` system) and `A11yPlaceholder` mirrors the
    // `Placeholder` string — so the a11y tree + the inbound driver observe the
    // text (widget-contracts.md §5). Both default empty here; the sync system
    // fills them each frame the source changes.
    A11yTextValue,
    A11yPlaceholder,
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
        color: ColorToken::SurfaceSecondary,
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
    /// companions; this layers the placeholder string, the `SingleLine` policy
    /// marker, AND the single-line **role** override (`A11yRole::TextInput`) on
    /// top — the role split IS the multiline distinction (widget-contracts.md
    /// §5), so single-line carries the single-line role rather than the bare
    /// marker's default `MultilineTextInput`.
    pub fn single_line(placeholder: impl Into<String>) -> impl Bundle {
        (
            TextInput,
            Placeholder(placeholder.into()),
            SingleLine,
            A11yRole::TextInput,
        )
    }

    /// A multi-line text input with a placeholder (Enter inserts a newline).
    /// The bare `TextInput` contract is already multi-line — no `SingleLine`,
    /// and the `#[require]` default role is `A11yRole::MultilineTextInput` — so
    /// this only layers the placeholder string.
    pub fn multi_line(placeholder: impl Into<String>) -> impl Bundle {
        (TextInput, Placeholder(placeholder.into()))
    }
}

/// Sync the agent-interface text state onto each `TextInput` root (P1d,
/// widget-contracts.md §5): mirror the editor's live value into [`A11yTextValue`]
/// and the [`Placeholder`] string into [`A11yPlaceholder`], so the a11y tree (and
/// the inbound in-process driver) observe the live text + prompt.
///
/// `A11yTextValue` is the single-line text value the outbound fold projects into
/// the view (`build_tree` → `text_value` → `set_value`); the editor-owned buffer
/// (`TextEditState::value()`) is authoritative, so this is the one-directional
/// projection from editor → a11y. It runs every frame the source changed and
/// **writes through only on a real difference** (the `*v != value` guard) so it
/// does not spuriously tick `Changed<A11yTextValue>` (which the outbound fold
/// keys off) when the text is unchanged — including the empty steady state.
///
/// `Changed<TextEditState>` would be the obvious gate, but the editor mutates its
/// buffer behind a `&` accessor in some paths (it is machinery state, not always
/// `DerefMut`-ticked on a value change), so the value is re-read each frame and
/// the difference guard provides the change-suppression instead.
pub fn sync_text_input_a11y(
    mut inputs: Query<
        (
            &TextEditState,
            &Placeholder,
            &mut A11yTextValue,
            &mut A11yPlaceholder,
        ),
        With<TextInput>,
    >,
) {
    for (editor, placeholder, mut text_value, mut a11y_placeholder) in &mut inputs {
        let value = editor.value();
        if text_value.0 != value {
            text_value.0 = value;
        }
        if a11y_placeholder.0 != placeholder.0 {
            a11y_placeholder.0 = placeholder.0.clone();
        }
    }
}
