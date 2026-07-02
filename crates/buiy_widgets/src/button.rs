//! Button widget. Phase 0 contract: `Focusable + A11yRole::Button + A11yLabel +
//! Node + Style with theme tokens + click-emits-OnPress`. Per-widget detail
//! (toggle button via aria-pressed, keyboard contract, full APG behavior)
//! lives in `buiy-widget-catalog-design`.
//!
//! The activation sink `OnPress` is the shared `buiy_core::interaction::OnPress`
//! (co-drive SC-1) — relocated to core so the P1c action router and the C3
//! pointer layer can write it too. The click handler uses
//! `MessageWriter::write`.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yRole},
    components::Node,
    focus::Focusable,
    layout::{AlignItems, BoxModel, Display, FlexAxis, FlexParams, JustifyContent, Style},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius, TextColor},
    text::{FontSize, Text, TextAlign},
};

/// The catalog font size for a button label (logical px).
pub(crate) const BUTTON_LABEL_FONT_SIZE: f32 = 16.0;

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
    Display = Display::flex_row(),
    FlexParams = button_center_flex(),
    Focusable,
    A11yRole = A11yRole::Button,
    A11yLabel,
)]
pub struct Button;

/// The button's content flex: center the label `Text` child in the box on both
/// axes (the canonical centered-label button).
pub(crate) fn button_center_flex() -> FlexParams {
    FlexParams {
        direction: FlexAxis::Row,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..Default::default()
    }
}

// The three initializer fns below are `pub(crate)` so the `scene` module's
// `button()` scene-fn can spell the SAME canonical values as `bsn!`
// field-patches — one source of truth shared between the `#[require]`
// initializers here and the mergeable scene-fn there.

/// The canonical button box: **content-width** (sizes to the label) × 32 logical
/// px tall, with 8px padding so the label has breathing room and the hit target
/// meets WCAG 2.5.8 (≥24×24). A content-width button is the natural shape — a
/// fixed width made "All" and "×" the same oversized box and overflowed dense
/// footers; an author who wants a fixed width patches `BoxModel { width }` on top.
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size
// tokens (space.2).
pub(crate) fn button_box_model() -> BoxModel {
    // Build through the `Style` fluent builder (the canonical authoring path
    // for the box), then extract the one decomposed component the require
    // needs. No width ⇒ auto (content-sized).
    Style::default().height_px(32.0).padding(8.0).box_model
}

/// The default button surface fill (the `color.surface.secondary` token).
pub(crate) fn button_background() -> Background {
    Background {
        color: ColorToken::SurfaceSecondary,
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
    /// paint, focus, and a11y companions; this constructor layers the a11y label
    /// (the accessible name) PLUS a centered, pick-through **label `Text`** child
    /// — the visible button text. The bare marker (`spawn(Button)`) is a
    /// label-less box (an icon button's canvas); `new`/`button()` give it text.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            Button,
            A11yLabel(label.clone()),
            children![(
                Text(label),
                FontSize(BUTTON_LABEL_FONT_SIZE),
                TextColor::default(),
                TextAlign::Center,
                Pickable::IGNORE,
            )],
        )
    }
}

// Pointer activation: C3c retired the Phase-0 `emit_on_press_on_click` poll
// (which fired `OnPress` on mouse-down by reading the legacy `Hovered` resource
// + `just_pressed`). Activation now lowers through C3b's pointer producer
// `buiy_core::picking::pointer_click_emits_on_press`, a `Pointer<Click>` observer
// that writes `OnPress` for any `A11yRole::Button` root (the `Button` marker
// carries that role via its `#[require]` contract). bevy_picking's
// `Pointer<Click>` fires only when press + release share a target, so the
// press-on-target → release-on-target = activate / release-off-target = cancel
// (drag-cancel) semantics — input-event-model.md § 2.5 / gate #8 — fall out for
// free, and the same `OnPress` sink is fed by the agent-interface campaign's
// keyboard (Enter/Space) and AT (`Action::Click`) producers (Phase 1c, the
// shared SC-1 convergence). The Button widget needs no activation system of its
// own.
