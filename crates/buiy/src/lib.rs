//! Buiy — comprehensive UI library for Bevy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/README.md.

use bevy::prelude::*;

pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder},
    components::{Node, ResolvedLayout, Style},
    focus::{FocusVisible, Focusable, FocusedEntity},
    picking::Hovered,
    render::ExtractedDraws,
    theme::{Theme, UserPreferences, default_light_theme},
};
pub use buiy_widgets::{Button, OnPress, WidgetsPlugin};

/// Top-level Buiy plugin. Composes sub-plugins in the documented order:
/// core → theme → a11y → focus → input → widgets. Render registration
/// happens in `Plugin::finish` so RenderApp exists when we reach it.
///
/// See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8.
pub struct BuiyPlugin;

impl Plugin for BuiyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::picking::PickingPlugin,
            WidgetsPlugin,
        ));
    }

    fn finish(&self, app: &mut App) {
        // RenderApp is guaranteed to exist by `finish` time.
        app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    }
}
