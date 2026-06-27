//! Buiy widget-gallery — the runnable **unified shell** (parity Wave C1).
//! `cargo run -p buiy_gallery` opens the WHOLE IDE-style shell (not one screen):
//! a top chrome bar, a left **Screens** rail (5 nav buttons over a Stats block), a
//! center viewport (a backdrop-blurred header + a dotted radial-grid canvas hosting
//! the active screen), a right **Inspector** pane (the active screen's name/desc +
//! "Composed of" chips + live-state + accent swatches — Wave C4), and a status bar.
//! Clicking a rail button switches the viewport to that screen; an accent swatch
//! re-themes the whole app live.
//!
//! The five hosted screens are still authored once each in `buiy_gallery`
//! (`spawn_todomvc_screen` / `spawn_scroll_screen` / `spawn_overlay_menu` / `spawn_modal`
//! / `spawn_showcase`) — the "example IS the fixture" discipline. The shell
//! (`buiy_gallery::shell`) spawns all five ONCE under `#ScreenContent` and the
//! `ScreenRouter` toggles which is laid out + a11y-visible (`Display::None` +
//! `A11yHidden`), preserving per-screen state across switches.
//!
//! The app boots the **dark theme** (`default_dark_theme`) so the design's dark
//! tokens resolve (Wave A note: the framework default theme is light; the gallery
//! opts in here). The per-screen app plugins (`TodoMvcPlugin` / `OverlayMenuPlugin`)
//! run globally alongside the router and no-op on the input-starved hidden screens.

use bevy::prelude::*;
use buiy::BuiyPlugin;
use buiy_gallery::GalleryPlugin;

fn main() {
    // The whole gallery — dark theme + shell router + all 5 screens' app plugins +
    // inspector + toast — is `GalleryPlugin`, shared verbatim with the `gallery_web`
    // WebGPU example so the screen wiring lives in exactly one place. The native
    // binary and the web example differ only in their window setup.
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_plugins(GalleryPlugin)
        .run();
}
