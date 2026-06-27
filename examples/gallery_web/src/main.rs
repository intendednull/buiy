//! The Buiy widget gallery, in the browser (WebGPU). Shares `GalleryPlugin` with
//! the native `buiy_gallery` binary — the five screens, the shell router, the
//! inspector, and the dark theme are all reused verbatim; only the window is
//! canvas-bound (and a wasm panic hook is installed) here.
use bevy::prelude::*;
use buiy::BuiyPlugin;
use buiy_gallery::GalleryPlugin;

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            // Bind the bevy window to the page's `<canvas id="buiy">`, size it to
            // the parent, and let bevy swallow the browser's default handling of
            // the events it consumes (scroll, context menu, …).
            primary_window: Some(Window {
                canvas: Some("#buiy".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiyPlugin)
        .add_plugins(GalleryPlugin)
        .run();
}
