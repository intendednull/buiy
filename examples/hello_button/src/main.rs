//! Buiy Phase 0 hello-world: spawn one Button. The end-to-end verification
//! test (`tests/hello_button_e2e.rs`) drives the same scene and asserts
//! visual regression + AccessKit tree snapshot + focus / click behavior.
//!
//! Bevy 0.18 split buffered events into `Message`. `OnPress` is therefore a
//! `Message`, not an `Event` — read with `MessageReader` instead of
//! `EventReader`. See `buiy_widgets::button` for the producer side.

use bevy::prelude::*;
use buiy::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
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
