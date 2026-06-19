//! Button widget. Phase 0 contract: `Focusable + A11yRole::Button + A11yLabel +
//! Node + Style with theme tokens + click-emits-OnPress`. Per-widget detail
//! (toggle button via aria-pressed, keyboard contract, full APG behavior)
//! lives in `buiy-widget-catalog-design`.
//!
//! Bevy 0.18 split buffered events into `Message`. `OnPress` is therefore a
//! `Message` (not an `Event` — `Event` is now reserved for observer-style
//! triggers in 0.18). The click handler uses `MessageWriter::write`.

use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yRole},
    components::Node,
    focus::Focusable,
    layout::{BoxModel, Style},
    picking::Hovered,
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius},
};
use std::borrow::Cow;

/// Button widget marker. The `#[require(...)]` contract is the single source
/// of the Phase-0 button shape — required-components are "the architectural
/// prerequisite for BSN" (bevy-ui prior-art; spec § 4.1a): the bare marker —
/// `world.spawn(Button)` or `bsn! { Button }` — materializes the full
/// layout-visible + paintable + focusable + accessible entity.
/// `Button::new(label)` layers only the a11y label on top.
///
/// The require list reproduces what `Button::new()` used to assemble by hand:
/// - `Node` — the layout marker, which itself `#[require]`s the full `Style`
///   decomposition (`Display`/`Position`/`FlexParams`/…), so a button is
///   layout-visible without the widget re-spelling those here.
/// - `BoxModel = button_box_model()` — the one style override (the canonical
///   120×32 / 8px-padding box); a *direct* `#[require]` initializer wins over
///   `Node`'s transitive `BoxModel` default.
/// - `Background` / `Border` — the paint companions.
/// - `Focusable` + `A11yRole::Button` + `A11yLabel` — interaction + a11y.
///
/// The initializer fns below are the shared canonical defaults, so the
/// constructor and the `#[require]` path can never diverge.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = button_box_model(),
    Background = button_background(),
    Border = button_border(),
    Focusable,
    A11yRole = A11yRole::Button,
    A11yLabel,
)]
pub struct Button;

#[derive(Message, Debug, Clone, Copy)]
pub struct OnPress(pub Entity);

// The three initializer fns below are `pub(crate)` so the `scene` module's
// `button()` scene-fn can spell the SAME canonical values as `bsn!`
// field-patches — one source of truth shared between the `#[require]`
// initializers here and the mergeable scene-fn there.

/// The canonical button box: 120×32 logical px, 8px padding. The hit target
/// already meets WCAG 2.5.8 (≥24×24).
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size
// tokens (space.button.width, space.2).
pub(crate) fn button_box_model() -> BoxModel {
    // Build through the `Style` fluent builder (the canonical authoring path
    // for the box), then extract the one decomposed component the require
    // needs. Keeps the numbers expressed once, the way an author would write
    // them.
    Style::default()
        .width_px(120.0)
        .height_px(32.0)
        .padding(8.0)
        .box_model
}

/// The default button surface fill (the `color.surface.secondary` token).
pub(crate) fn button_background() -> Background {
    Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
    }
}

/// The default button border: rounded corners ("radius.md"), no painted line.
pub(crate) fn button_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(6.0)),
        ..Default::default()
    }
}

impl Button {
    /// Spawn-ready bundle for a labelled button. Returns `impl Bundle`
    /// (not `Self`) so callers get the full Phase 0 button contract.
    ///
    /// The `Button` marker's `#[require(...)]` materializes the node, style,
    /// paint, focus, and a11y companions; this constructor only layers the
    /// a11y label on top. The bare-marker path (`spawn(Button)` /
    /// `bsn! { Button }`) and this path therefore produce the same entity,
    /// differing only in the label string.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        (Button, A11yLabel(label.into()))
    }
}

// TODO(buiy-widget-catalog-design): Phase 0 fires OnPress on mouse-down
// (`just_pressed`). WAI-ARIA APG and the web platform fire on mouse-up
// after press-on-target so users can drag-cancel. Switch to press-down
// → set "armed" state → release-on-target = OnPress, release-off-target
// = cancel, in the widget catalog spec.
//
// TODO(buiy-widget-catalog-design): Phase 0 ships only mouse activation.
// APG button contract requires Enter and Space (key down) to also fire
// OnPress when the button is focused. Wire keyboard activation alongside
// the full APG keyboard contract in the widget catalog spec.
pub(crate) fn emit_on_press_on_click(
    // Both `Hovered` (from `PickingPlugin`) and `ButtonInput<MouseButton>`
    // (from bevy_input's `InputPlugin`) are owned by sibling plugins that
    // are not pulled in by every test setup — `MinimalPlugins + CorePlugin`
    // includes neither. Treat their absence as "nothing to do this frame"
    // so the system is robust under partial harnesses.
    hovered: Option<Res<Hovered>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    buttons: Query<(), With<Button>>,
    mut writer: MessageWriter<OnPress>,
) {
    let (Some(hovered), Some(mouse)) = (hovered, mouse) else {
        return;
    };
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(entity) = hovered.0 else {
        return;
    };
    if buttons.get(entity).is_ok() {
        writer.write(OnPress(entity));
    }
}
