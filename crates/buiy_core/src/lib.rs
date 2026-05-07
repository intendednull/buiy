//! Buiy core: components, plugin scaffolding, system sets.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8 for
//! sub-plugin order and SystemSet definitions.

use bevy::prelude::*;

/// Top-level system sets for Buiy. Order: Layout → Style → Input → Animate
/// → Picking → A11yUpdate → Render.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiySet {
    Layout,
    Style,
    Input,
    Animate,
    Picking,
    A11yUpdate,
    Render,
}

/// Core Buiy plugin: registers types, configures system sets.
/// Composed into `BuiyPlugin` from the meta-crate; not consumed directly
/// by end users.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                BuiySet::Layout,
                BuiySet::Style,
                BuiySet::Input,
                BuiySet::Animate,
                BuiySet::Picking,
                BuiySet::A11yUpdate,
                BuiySet::Render,
            )
                .chain(),
        );
    }
}
