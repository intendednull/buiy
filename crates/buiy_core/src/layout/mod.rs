//! Buiy layout subsystem.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/.

mod components;
mod pipeline;
mod style;
mod systems;
pub(crate) mod translate;
mod tree;
mod types;

pub use components::{BoxModel, Display, FlexItem, FlexParams, Position};
pub use pipeline::BuiyLayoutStep;
pub use style::Style;
pub use tree::LayoutTree;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};

use bevy::prelude::*;

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<LayoutTree>();

        // Register decomposed components for reflection / BSN / inspectors.
        app.register_type::<BoxModel>()
            .register_type::<Display>()
            .register_type::<Position>()
            .register_type::<FlexParams>()
            .register_type::<FlexItem>()
            .register_type::<Edges>()
            .register_type::<Sizing>()
            .register_type::<Length>()
            .register_type::<AspectRatio>()
            .register_type::<Inset>();

        pipeline::configure_pipeline(app);

        app.add_systems(
            Update,
            (
                systems::gc_removed_nodes.in_set(BuiyLayoutStep::RemovedNodesGc),
                systems::sync_styles.in_set(BuiyLayoutStep::SyncStyles),
                systems::taffy_compute.in_set(BuiyLayoutStep::TaffyCompute),
                systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
            ),
        );
    }
}
