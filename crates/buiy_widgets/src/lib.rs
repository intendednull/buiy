//! Buiy widgets. Phase 0 ships a single `Button` to validate the
//! foundation. Full APG widget catalog lives in `buiy-widget-catalog-design`.

use bevy::prelude::*;

pub mod button;
pub use button::{Button, OnPress};

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        // Bevy 0.18 split buffered events into `Message`; `add_event` was
        // renamed to `add_message`. `OnPress` is a `Message` so it lives in
        // `Messages<OnPress>` and is read with `MessageReader` / a cursor.
        app.register_type::<Button>()
            .add_message::<OnPress>()
            .add_systems(
                Update,
                button::emit_on_press_on_click.in_set(buiy_core::BuiySet::Input),
            );
    }
}
