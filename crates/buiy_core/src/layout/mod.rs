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
    Anchor, BoxModel, Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive,
    Display, FlexItem, FlexParams, GridItem, GridParams, LayoutAnchorBroken, MultiColumn, Overflow,
    Position, Scroll, ScrollOffset, ScrollSnapItem, WritingMode, WritingModeResolved,
};
pub use pipeline::BuiyLayoutStep;
pub use style::{LogicalBoxModel, LogicalInset, Style};
pub use systems::{
    AnchorNameRegistry, LayoutAnchorWarnedThisFrame, LayoutTaffyComputeCount,
    LayoutWarnedOnceSession, PostTaffyPositionOverrides, SyncStylesIterCount,
};
pub use tree::LayoutTree;
pub use types::{
    AlignContent, AlignItems, AnchorErrorKind, AnchorName, AnchorRef, AspectRatio, BoxSizing,
    BreakAfter, BreakBefore, BreakInside, ColumnCount, ColumnFill, ColumnRule, ColumnRuleStyle,
    ColumnSpan, ContainerType, Direction, Edges, FlexAxis, FlexGap, FlexWrap, GridAreas,
    GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems, LayoutWarnOnceKey, Length,
    LogicalEdges, NamedArea, Orientation, OverflowMode, OverscrollBehavior, PositionKind,
    PositionTry, QueryCondition, RepeatCount, ScrollBehavior, ScrollbarColor, ScrollbarGutter,
    ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType, TextOrientation, TrackSize,
    TryCondition, UnicodeBidi, WritingModeKind,
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

        // Phase 6/7 — anchor-positioning + shared override-map resources.
        // `AnchorNameRegistry` is maintained by the observers below;
        // `PostTaffyPositionOverrides` is cleared by
        // `clear_post_taffy_overrides` (Phase 7) and populated by every
        // sub-pass of `BuiyLayoutStep::PostTaffyOverrides`;
        // `LayoutAnchorWarnedThisFrame` is cleared + populated by
        // `anchor_resolution` each frame (anchor-specific);
        // `LayoutWarnedOnceSession` is the canonical per-session
        // warn-dedup HashSet used by sticky/table/multicol sub-passes
        // (spec § 6). It starts empty on every `App::new()`; the
        // matching `clear_warned_once_on_exit` system is defined but
        // not yet wired because `buiy_core` has no `BuiyState` /
        // `BuiyExit` lifecycle states (plan D7).
        app.init_resource::<systems::AnchorNameRegistry>();
        app.init_resource::<systems::PostTaffyPositionOverrides>();
        app.init_resource::<systems::LayoutAnchorWarnedThisFrame>();
        app.init_resource::<systems::LayoutWarnedOnceSession>();

        // Phase 6 — observers register as closures per Decision D12:
        // `On<'w, 't, E, B>` carries two lifetimes without defaults and
        // named-fn signatures don't elide them cleanly. Closures inherit
        // lifetimes from `add_observer`'s `IntoObserverSystem` impl.
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Insert, Anchor>,
             q: Query<&Anchor>,
             mut reg: ResMut<systems::AnchorNameRegistry>| {
                systems::handle_anchor_insert(trigger.event().entity, &q, &mut reg);
            },
        );
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Replace, Anchor>,
             mut reg: ResMut<systems::AnchorNameRegistry>| {
                reg.remove(trigger.event().entity);
            },
        );
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Remove, Anchor>,
             mut reg: ResMut<systems::AnchorNameRegistry>| {
                reg.remove(trigger.event().entity);
            },
        );

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
            .register_type::<QueryCondition>()
            // Phase 6 — anchor positioning.
            .register_type::<Anchor>()
            .register_type::<LayoutAnchorBroken>()
            .register_type::<AnchorName>()
            .register_type::<AnchorRef>()
            .register_type::<PositionTry>()
            .register_type::<TryCondition>()
            .register_type::<AnchorErrorKind>()
            // Phase 7 — multi-column + warn-once key (Tasks 3, 4, 7).
            .register_type::<MultiColumn>()
            .register_type::<ColumnCount>()
            .register_type::<ColumnRule>()
            .register_type::<ColumnRuleStyle>()
            .register_type::<ColumnSpan>()
            .register_type::<ColumnFill>()
            .register_type::<BreakInside>()
            .register_type::<BreakBefore>()
            .register_type::<BreakAfter>()
            .register_type::<LayoutWarnOnceKey>();

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
                // Phase 7 — PostTaffyOverrides chain: clear → sticky 6a →
                // table 6b → multicol 6c → anchor 6d. All four sub-passes
                // share `PostTaffyPositionOverrides`; the clear runs first
                // so each pass writes into an empty map (architecture.md
                // § 3, plan Task 8 + D2). `.chain()` over the tuple gives
                // the explicit deterministic order Phase 7's review
                // demanded; in-set membership lets external systems
                // hook between Taffy and write_resolved_layout without
                // depending on individual sub-pass labels.
                (
                    systems::clear_post_taffy_overrides,
                    systems::sticky_offset,
                    systems::table_layout,
                    systems::multicol_pack,
                    systems::anchor_resolution,
                )
                    .chain()
                    .in_set(BuiyLayoutStep::PostTaffyOverrides),
                systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
            ),
        );
    }
}
