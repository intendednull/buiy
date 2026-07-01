//! Windowed `buiy_view` TodoMVC (needs a display):
//! `cargo run -p todomvc_view --bin todomvc_view`.

use bevy::prelude::*;
use buiy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(BuiyPlugin);
    todomvc_view::install(&mut app);
    app.run();
}
