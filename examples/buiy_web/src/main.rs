//! Buiy in the browser (WebGPU). The smallest end-to-end web target: a single
//! Button on a canvas-bound window. Build with `trunk build examples/buiy_web/index.html`;
//! the headless-browser WebGPU smoke gate (`tools/web-smoke`) loads it and
//! asserts it paints with no shader/pipeline errors.

use bevy::prelude::*;
use buiy::*;

fn main() {
    // Readable panics + logs in the browser console (no-op off-wasm).
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // Bind to <canvas id="buiy"> in index.html (bevy_winit reads this
                // selector on wasm; inert on native).
                canvas: Some("#buiy".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiyPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, log_press)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Button::new("Save"));
}

fn log_press(mut events: MessageReader<OnPress>) {
    for ev in events.read() {
        info!("button pressed: {:?}", ev.0);
    }
}
