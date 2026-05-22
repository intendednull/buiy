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

pub use components::{
    BoxModel, Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive, Display,
    FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll, ScrollOffset,
    ScrollSnapItem, WritingMode, WritingModeResolved,
};
pub use pipeline::BuiyLayoutStep;
pub use style::{LogicalBoxModel, LogicalInset, Style};
pub use systems::{LayoutTaffyComputeCount, SyncStylesIterCount};
pub use tree::LayoutTree;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, ContainerType, Direction, Edges, FlexAxis,
    FlexGap, FlexWrap, GridAreas, GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems,
    Length, LogicalEdges, NamedArea, Orientation, OverflowMode, OverscrollBehavior, PositionKind,
    QueryCondition, RepeatCount, ScrollBehavior, ScrollbarColor, ScrollbarGutter, ScrollbarWidth,
    Sizing, SnapAlign, SnapStop, SnapType, TextOrientation, TrackSize, UnicodeBidi,
    WritingModeKind,
};

use bevy::prelude::*;

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<LayoutTree>();
        // Phase 5 Task 8: re-run flag set by step 4 and observed/cleared
        // by step 5. `LayoutTaffyComputeCount` is a per-frame instrument
        // used by tests to assert the "cap at 2× Taffy per frame"
        // architecture invariant (architecture.md § 3.2).
        // `SyncStylesIterCount` (Task 9) is the per-frame iter-count
        // instrument used by the Phase 2 O(0) steady-state assertion in
        // `tests/layout_container_queries.rs`.
        app.init_resource::<systems::CqReRunRequested>();
        app.init_resource::<systems::LayoutTaffyComputeCount>();
        app.init_resource::<systems::SyncStylesIterCount>();

        // Register decomposed components for reflection / BSN / inspectors.
        // Grouped by phase / feature area: Phase 1-2 layout primitives,
        // then Phase 3 grid types, then Phase 4 writing-mode types, then
        // Phase 5 container-query types (Container/ContainerQuery markers
        // alongside ContainerType/Orientation/QueryCondition enums).
        app.register_type::<BoxModel>()
            .register_type::<Display>()
            .register_type::<Position>()
            .register_type::<FlexParams>()
            .register_type::<FlexItem>()
            .register_type::<Overflow>()
            .register_type::<Scroll>()
            .register_type::<ScrollOffset>()
            .register_type::<ScrollSnapItem>()
            .register_type::<GridParams>()
            .register_type::<GridItem>()
            .register_type::<WritingMode>()
            .register_type::<WritingModeResolved>()
            .register_type::<Edges>()
            .register_type::<Sizing>()
            .register_type::<Length>()
            .register_type::<AspectRatio>()
            .register_type::<Inset>()
            .register_type::<TrackSize>()
            .register_type::<RepeatCount>()
            .register_type::<GridLine>()
            .register_type::<GridAreas>()
            .register_type::<NamedArea>()
            .register_type::<GridAutoFlow>()
            .register_type::<JustifyItems>()
            .register_type::<WritingModeKind>()
            .register_type::<Direction>()
            .register_type::<TextOrientation>()
            .register_type::<UnicodeBidi>()
            .register_type::<LogicalEdges>()
            // Phase 5 — container queries.
            .register_type::<Container>()
            .register_type::<ContainerQuery>()
            .register_type::<ContainerQueryActive>()
            .register_type::<ContainerQueryInactive>()
            .register_type::<ContainerType>()
            .register_type::<Orientation>()
            .register_type::<QueryCondition>();

        pipeline::configure_pipeline(app);

        app.add_systems(
            Update,
            (
                systems::gc_removed_nodes.in_set(BuiyLayoutStep::RemovedNodesGc),
                systems::inherit_writing_mode.in_set(BuiyLayoutStep::WritingModeInherit),
                systems::sync_styles.in_set(BuiyLayoutStep::SyncStyles),
                systems::cq_activate.in_set(BuiyLayoutStep::CqActivate),
                systems::taffy_compute.in_set(BuiyLayoutStep::TaffyCompute),
                systems::cq_flip_check.in_set(BuiyLayoutStep::CqFlipCheck),
                systems::cq_flip_rerun.in_set(BuiyLayoutStep::CqFlipReRun),
                systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
            ),
        );
    }
}
