//! Windowed `buiy_view` Counter (needs a display):
//! `cargo run -p counter_view --bin counter_view`.

use bevy::prelude::*;
use buiy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(BuiyPlugin);
    counter_view::install(&mut app);
    app.run();
}
