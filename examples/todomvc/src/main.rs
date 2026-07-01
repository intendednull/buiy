//! TodoMVC on Buiy MVU (prototype) — `cargo run -p todomvc`.
//!
//! The feature lives in [`todomvc::TodoPlugin`]; this binary just hosts it.

use bevy::prelude::*;
use buiy::*;
use todomvc::TodoPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_plugins(TodoPlugin)
        .run();
}
