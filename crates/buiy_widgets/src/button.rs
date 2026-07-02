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

/// Builder-only **trigger** for the [`Button`] children observer (Track C / C4).
/// Inserted ONLY by [`Button::new`] (never by the bare `Button` marker, the
/// `(Button, A11yLabel)` icon-button path, or the `button()` scene-fn), so the
/// `On<Add, ButtonParts>` observer attaches the label child ONLY to a
/// `Button::new(..)` root — bare/icon/scene spawns stay label-less. Deliberately
/// **not** `Reflect`/`register_type`'d: a transient authoring signal that must
/// not round-trip through a scene / hot-reload / MVU-replay respawn (so the
/// observer never re-fires on a reconstructed entity).
#[derive(Component)]
pub struct ButtonParts;

/// Marker on the button's visible label `Text` child — the idempotency signature
/// the `attach_button_children` observer checks so it never double-attaches.
#[derive(Component)]
pub struct ButtonLabel;

impl Button {
    /// The canonical labelled button. Returns a named [`ButtonBuilder`] (a
    /// `Bundle`) spawned directly with `commands.spawn(Button::new("Save"))`.
    ///
    /// The `Button` marker's `#[require(...)]` materializes the node/style/paint/
    /// focus/a11y companions; the builder adds the accessible name (`A11yLabel`)
    /// and the [`ButtonParts`] trigger, and an `On<Add, ButtonParts>` observer
    /// attaches the centered, pick-through visible **label `Text`** child. The
    /// bare marker (`spawn(Button)`) or `(Button, A11yLabel)` stays a label-less
    /// box (an icon button's canvas) — no `ButtonParts`, so no label child.
    #[allow(clippy::new_ret_no_self)] // builder pattern: `new` returns the named builder
    pub fn new(label: impl Into<String>) -> ButtonBuilder {
        ButtonBuilder {
            marker: Button,
            a11y_label: A11yLabel(label.into()),
            parts: ButtonParts,
        }
    }
}

/// The named `Bundle` [`Button::new`] returns: the `Button` marker (fires the
/// `#[require]` contract) + the accessible name + the [`ButtonParts`] trigger.
#[derive(Bundle)]
pub struct ButtonBuilder {
    marker: Button,
    a11y_label: A11yLabel,
    parts: ButtonParts,
}

/// `On<Add, ButtonParts>` observer body (Track C / C4): attach the visible label
/// `Text` child to a `Button::new(..)` root, reading the accessible name off the
/// root's `A11yLabel` (single source — no duplicate label string). Idempotent:
/// early-returns if a [`ButtonLabel`] child already exists, so a re-added trigger
/// (a defensive belt against any respawn path) never double-attaches. The child
/// tuple is byte-identical to the pre-C4 hand-wired `children![…]`.
pub(crate) fn attach_button_children(
    root: Entity,
    labels: &Query<&A11yLabel>,
    children: &Query<&Children>,
    is_label: &Query<(), With<ButtonLabel>>,
    commands: &mut Commands,
) {
    if let Ok(existing) = children.get(root)
        && existing.iter().any(|c| is_label.get(c).is_ok())
    {
        return; // label already attached — idempotent no-op
    }
    let Ok(A11yLabel(text)) = labels.get(root) else {
        return;
    };
    commands.entity(root).with_child((
        ButtonLabel,
        Text(text.clone()),
        FontSize(BUTTON_LABEL_FONT_SIZE),
        TextColor::default(),
        TextAlign::Center,
        Pickable::IGNORE,
    ));
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
