//! Buiy widgets. Phase 0 ships a single `Button` to validate the
//! foundation. Full APG widget catalog lives in `buiy-widget-catalog-design`.

use bevy::prelude::*;

pub mod button;
pub mod scene;
pub mod text_input;
pub use button::Button;
// `OnPress` relocated to `buiy_core` (co-drive SC-1) so the in-core P1c action
// router and C3 pointer layer can write the same activation sink. Re-exported
// here for source-compat: `buiy_widgets::OnPress` and the `buiy` prelude keep
// resolving unchanged.
pub use buiy_core::interaction::OnPress;
pub use scene::{button, text_input_multi_line, text_input_single_line};
pub use text_input::{TextInput, focus_on_click};

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        // `Messages<OnPress>` is registered by `CorePlugin`
        // (`InteractionPlugin`, co-drive SC-1), not here — the shared
        // activation sink lives in `buiy_core` so in-core producers can write
        // it. `WidgetsPlugin` is always composed after `CorePlugin`.
        app.register_type::<Button>()
            .register_type::<text_input::TextInput>()
            .add_systems(
                Update,
                (button::emit_on_press_on_click, text_input::focus_on_click)
                    .in_set(buiy_core::BuiySet::Input),
            );
    }
}
