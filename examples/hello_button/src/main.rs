//! Buiy MVU demo (prototype): a **counter** — run with `cargo run -p hello_button`.
//!
//! The whole feature lives in [`hello_button::CounterPlugin`] (see `lib.rs`) so
//! the windowed binary, the headless logic test (`tests/counter_mvu.rs`), and the
//! GPU capture bin (`src/bin/capture_counter.rs`) all drive the same wiring.

use bevy::prelude::*;
use buiy::*;
use hello_button::CounterPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_plugins(CounterPlugin)
        .run();
}
