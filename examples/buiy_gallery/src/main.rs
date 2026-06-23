//! Buiy widget-gallery — **S1 (TodoMVC)**. `cargo run -p buiy_gallery` opens the
//! TodoMVC screen composed from the P1d widgets (single-line TextInput +
//! tri-state Checkbox + Button) with the live "N items left" Status region.
//!
//! Type the field + Enter to add a todo, click a checkbox to complete it, click
//! × to destroy a row, "Clear completed" to drop the done ones, and the
//! All/Active/Completed buttons to filter. Double-click a label to edit it in
//! place. The headless inspection-driver acceptance
//! (`crates/buiy_verify/tests/verify_headless/todomvc_c8a.rs`) drives the same
//! tree through the a11y driver + synthetic pointer/keyboard and is the gate.

use bevy::prelude::*;
use buiy::BuiyPlugin;
use buiy_gallery::{TodoMvcPlugin, setup};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_plugins(TodoMvcPlugin)
        .add_systems(Startup, setup)
        .run();
}
