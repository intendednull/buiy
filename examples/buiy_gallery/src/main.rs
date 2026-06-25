//! Buiy widget-gallery — runnable exemplar. `cargo run -p buiy_gallery` opens a
//! screen composed from the P1d widgets + this campaign's containers.
//!
//! Pick the screen with `BUIY_GALLERY_SCREEN` (default `todomvc`):
//!
//! - `todomvc` (**S1**) — the TodoMVC exemplar (single-line TextInput + tri-state
//!   Checkbox + Button + the live "N items left" Status region). Type the field +
//!   Enter to add, click a checkbox to complete, × to destroy, "Clear completed",
//!   All/Active/Completed to filter, double-click a label to edit in place.
//! - `scroll` (**S2**) — the long-list scale-game: a `ScrollArea` over ~1000 rows
//!   (the off-screen rows ride `ContentVisibility::Auto`). Wheel / PageDown / End
//!   scroll the clamped offset.
//! - `overlay` (**S3**) — overlays: an "Edit" `MenuButton` → `Menu` (Cut/Copy/
//!   Paste, arrow-nav + Enter), a "?" tooltip trigger, and an anchored popover.
//! - `modal` (**S4**) — modal + focus-trap: a trigger Button invokes a `Dialog`
//!   (title + body + a Switch + a Close button); Tab traps inside, Escape closes
//!   + restores focus, the background is pruned while open.
//! - `showcase` (**S5**) — the F-tier look: a `Switch` + `Slider` + `Disclosure`
//!   on a card with a `BoxShadow` elevation + per-side `Border` + the focus ring.
//!
//! Each screen is `pub fn screen_*` (or `spawn_*` where it needs imperative
//! entity-referencing wiring) in `buiy_gallery`, so the same tree the binary
//! renders is the tree the `buiy_verify` fixtures + the headless inspection-driver
//! acceptance tests (`crates/buiy_verify/tests/verify_headless/todomvc_c8a.rs`,
//! `scroll_overlay_c8b.rs`, `modal_showcase_c8c.rs`) drive — the "example IS the
//! fixture" discipline.

use bevy::prelude::*;
use buiy::BuiyPlugin;
use buiy_gallery::{
    OverlayMenuPlugin, TodoMvcPlugin, setup, setup_modal, setup_overlay_menu, setup_scroll_list,
    setup_showcase,
};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(BuiyPlugin);

    match std::env::var("BUIY_GALLERY_SCREEN").as_deref() {
        Ok("scroll") => {
            app.add_systems(Startup, setup_scroll_list);
        }
        Ok("overlay") => {
            app.add_plugins(OverlayMenuPlugin)
                .add_systems(Startup, setup_overlay_menu);
        }
        // S4 (modal) + S5 (showcase) are pure composition over the C5-d Dialog
        // lifecycle + the C6 styling — both fully owned by `WidgetsPlugin` (inside
        // `BuiyPlugin`), so they need no gallery app plugin.
        Ok("modal") => {
            app.add_systems(Startup, setup_modal);
        }
        Ok("showcase") => {
            app.add_systems(Startup, setup_showcase);
        }
        // `todomvc` (S1) is the default screen.
        _ => {
            app.add_plugins(TodoMvcPlugin).add_systems(Startup, setup);
        }
    }

    app.run();
}
