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
use buiy_core::theme::default_dark_theme;
use buiy_gallery::ModalPlugin;
use buiy_gallery::OverlayMenuPlugin;
use buiy_gallery::ScrollListPlugin;
use buiy_gallery::ShowcasePlugin;
use buiy_gallery::TodoMvcPlugin;
use buiy_gallery::composites::ToastPlugin;
use buiy_gallery::inspector::InspectorPlugin;
use buiy_gallery::shell::ScreenRouterPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(BuiyPlugin);

    // Opt into the dark theme so the design's dark tokens resolve (the framework
    // ships light by default — Wave A reconciliation note).
    app.insert_resource(default_dark_theme());

    // The shell router + the per-screen app plugins. The router spawns the shell
    // tree + all 5 screens at boot (`setup_shell` in `ScreenRouterPlugin`'s
    // Startup); `TodoMvcPlugin` (S1) + `ScrollListPlugin` (S2 search/selection) +
    // `OverlayMenuPlugin` (S3) + `ModalPlugin` (S4 create/delete body swap) +
    // `ShowcasePlugin` (S5 switch/slider/segmented/stepper/meter/disclosure) supply
    // the retained-mode app logic those screens need. `ToastPlugin` runs the S5
    // "Build finished" toast lifecycle (shared with the modal/menu toasts).
    app.add_plugins(ScreenRouterPlugin)
        .add_plugins(InspectorPlugin)
        .add_plugins(TodoMvcPlugin)
        .add_plugins(ScrollListPlugin)
        .add_plugins(OverlayMenuPlugin)
        .add_plugins(ModalPlugin)
        .add_plugins(ShowcasePlugin)
        .add_plugins(ToastPlugin);

    app.run();
}
