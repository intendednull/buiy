//! Buiy core: components, plugin scaffolding, system sets.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8 for
//! sub-plugin order and SystemSet definitions.

use bevy::prelude::*;

pub mod a11y;
pub mod components;
pub mod focus;
pub mod layout;
pub mod picking;
pub mod render;
pub mod theme;

pub use a11y::{A11yDescription, A11yLabel, A11yNodeView, A11yPlugin, A11yRole, A11yTreeBuilder};
pub use components::{Node, ResolvedLayout, ResolvedTransform, Visual};
pub use focus::{FocusPlugin, FocusVisible, Focusable, FocusedEntity};
pub use layout::{
    AlignContent, AlignItems, Anchor, AnchorErrorKind, AnchorName, AnchorRef, AspectRatio,
    BackfaceVisibility, BoxModel, BoxSizing, BreakAfter, BreakBefore, BreakInside, BuiyLayoutStep,
    ColumnCount, ColumnFill, ColumnRule, ColumnRuleStyle, ColumnSpan, ContainFlags, Container,
    ContainerQuery, ContainerQueryActive, ContainerQueryInactive, ContainerType, Containment,
    ContentVisibility, Direction, Display, Edges, FlexAxis, FlexGap, FlexItem, FlexParams,
    FlexWrap, GridAreas, GridAutoFlow, GridItem, GridLine, GridParams, Inset, JustifyContent,
    JustifyItems, LayoutAnchorBroken, LayoutPlugin, LayoutTree, LayoutWarnOnceKey,
    LayoutWarnedOnceSession, Length, LogicalBoxModel, LogicalEdges, LogicalInset, MultiColumn,
    NamedArea, Orientation, Overflow, OverflowMode, OverscrollBehavior, Position, PositionKind,
    PositionTry, PostTaffyPositionOverrides, QueryCondition, RepeatCount, Rotate, Scale, Scroll,
    ScrollBehavior, ScrollOffset, ScrollSnapItem, ScrollbarColor, ScrollbarGutter, ScrollbarWidth,
    Sizing, SnapAlign, SnapStop, SnapType, Style, TextOrientation, TrackSize, TransformMatrix,
    TransformOrigin, TransformStyle, Translate, TryCondition, UiTransform, UnicodeBidi, WillChange,
    WillChangeProperty, WritingMode, WritingModeKind, WritingModeResolved,
};
pub use picking::{BuiyPickingBackendPlugin, Hovered, PickingPlugin, hit_test};

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
        app.register_type::<Node>()
            .register_type::<ResolvedLayout>()
            .register_type::<Visual>()
            .configure_sets(
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
