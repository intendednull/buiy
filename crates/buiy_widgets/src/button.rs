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
    components::{Node, Style},
    focus::Focusable,
    picking::Hovered,
};

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
            Style {
                width: 120.0,
                height: 32.0,
                padding: 8.0,
                border_radius: 6.0, // matches "radius.md"
                background_token: "color.surface.secondary".into(),
                foreground_token: "color.text.primary".into(),
                ..default()
            },
            Focusable::default(),
            A11yRole::Button,
            A11yLabel(label),
        )
    }
}

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
