//! Buiy hello-world via **BSN**: author a real widget tree declaratively with
//! the `bsn!` macro (see [`hello_bsn::hello_bsn_scene`]) and render it under
//! `BuiyPlugin`. The headless layout-snapshot gate (`tests/layout_snapshot.rs`)
//! drives the same scene and pins the resolved tree.
//!
//! `cargo run -p hello_bsn` opens a window; the snapshot test is the gate.

use bevy::prelude::*;
use buiy::BuiyPlugin;
use hello_bsn::setup;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_systems(Startup, setup)
        .run();
}
