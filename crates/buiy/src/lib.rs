//! Buiy — comprehensive UI library for Bevy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/README.md.

use bevy::prelude::*;

pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder},
    components::{FlexDirection, Node, ResolvedLayout, Style},
    focus::{FocusVisible, Focusable, FocusedEntity},
    picking::Hovered,
    theme::{Theme, UserPreferences, default_light_theme},
};
pub use buiy_widgets::{Button, OnPress, WidgetsPlugin};

// `buiy_core::render::ExtractedDraws` is intentionally NOT re-exported at
// the crate root: it is a render-world resource only, populated during the
// extract phase. Main-world consumers reading it would see an empty Vec.
// Render-world plugin authors who need it can reach `buiy::buiy_core::render`
// (or depend on `buiy_core` directly) without crate-root surface pollution.

/// Top-level Buiy plugin. Composes sub-plugins in the documented order:
/// core → theme → a11y → focus → input → widgets. Render registration
/// happens in `Plugin::finish` so RenderApp exists when we reach it.
///
/// See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8.
///
/// # Required Bevy plugins
///
/// `BuiyPlugin` requires `bevy::input::InputPlugin`. `DefaultPlugins`
/// includes it; if you build your app with `MinimalPlugins`, add it
/// explicitly:
///
/// ```ignore
/// App::new()
///     .add_plugins(MinimalPlugins)
///     .add_plugins(bevy::input::InputPlugin)
///     .add_plugins(BuiyPlugin)
///     .run();
/// ```
///
/// `FocusPlugin::handle_tab` reads `Res<ButtonInput<KeyCode>>` and the
/// `Button` click handler reads `Res<ButtonInput<MouseButton>>`. Bevy
/// 0.18 panics when a `Res<T>` system param is missing, so the plugin
/// must be present.
pub struct BuiyPlugin;

impl Plugin for BuiyPlugin {
    fn build(&self, app: &mut App) {
        // Sub-plugin order matches architecture.md § 2.8 documented order:
        // core → theme → a11y → focus → input → text → widgets → ...
        // Phase 0 omits text / animation / forms / devtools; LayoutPlugin
        // and PickingPlugin (which aren't enumerated as sub-plugins in § 2.8
        // because their work lives in BuiySet::Layout and BuiySet::Picking)
        // are slotted between Focus and Widgets so widgets see resolved
        // layout + hit-test results when they run in the same frame.
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
