//! Dooduel in the browser (WebGPU / WebGL2). Reuses the whole `dooduel` lib (the
//! MVU app, the drawing canvas, theming, localStorage persistence, the responsive
//! mobile shell) via [`dooduel::install_runtime`], the SAME plugin set the native
//! `dooduel` bin uses. Only the window is canvas-bound (and a wasm panic hook is
//! installed) here.
//!
//! Build both backends + the auto-selecting loader with `tools/build-web.sh
//! apps/dooduel/dooduel_web`, then serve `apps/dooduel/dooduel_web/dist-web/`.
use bevy::prelude::*;
use buiy::BuiyPlugin;

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        // Bind the bevy window to the page's `<canvas id="buiy">`, size it to the
        // parent, and let bevy swallow the browser's default handling of the events
        // it consumes (scroll, context menu, touch). `fit_canvas_to_parent` keeps
        // the logical window size in sync with the viewport, which feeds the
        // responsive shell (`ViewportPlugin` → mobile layout).
        primary_window: Some(Window {
            canvas: Some("#buiy".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }));
    app.add_plugins(BuiyPlugin);
    dooduel::install_runtime(&mut app);
    app.run();
}
