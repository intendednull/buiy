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
    layout::Style,
    picking::Hovered,
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius},
};
use std::borrow::Cow;

#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Button;

#[derive(Message, Debug, Clone, Copy)]
pub struct OnPress(pub Entity);

impl Button {
    /// Spawn-ready bundle for a labelled button. Returns `impl Bundle`
    /// (not `Self`) so callers get the full Phase 0 button contract —
    /// marker + node + style + focusable + a11y role + a11y label —
    /// without having to assemble it themselves.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            Button,
            Node,
            // TODO(buiy-widget-catalog-design): replace hardcoded sizes
            // with size tokens (space.button.width, space.2). Hit target
            // 120x32 already meets WCAG 2.5.8 (>=24x24).
            Style::default()
                .width_px(120.0)
                .height_px(32.0)
                .padding(8.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
            },
            Border {
                radius: Corners::all(Radius::circular(6.0)), // matches "radius.md"
                ..Default::default()
            },
            Focusable::default(),
            A11yRole::Button,
            A11yLabel(label),
        )
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
