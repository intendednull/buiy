//! Buiy core: components, plugin scaffolding, system sets.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8 for
//! sub-plugin order and SystemSet definitions.

use bevy::prelude::*;
use bevy::transform::systems::{
    StaticTransformOptimizations, mark_dirty_trees, propagate_parent_transforms,
    sync_simple_transforms,
};

pub mod a11y;
pub mod animation;
pub mod components;
pub mod focus;
pub mod interaction;
pub mod layout;
/// MVU (Model-View-Update) substrate (`docs/specs/2026-06-29-mvu-as-core-design.md`).
/// Opt-in via [`mvu::MvuCorePlugin`] (NOT composed into `CorePlugin`). See the module header.
pub mod mvu;
pub mod picking;
pub mod render;
/// Whole-UI record/replay (spec §7). Merges the
/// widget-fold log ([`mvu::MsgLog`]) + the editor-command log
/// ([`text::edit::EditLog`]) under one global sequence and replays a recorded session
/// byte-identically into a fresh app. See the module header.
pub mod replay;
pub mod scroll;
pub mod text;
pub mod theme;

pub use a11y::{A11yDescription, A11yLabel, A11yNodeView, A11yPlugin, A11yRole, A11yTreeBuilder};
pub use animation::{
    AnimatedBackgroundColor, AnimationPlugin, BackgroundColorTween, Easing, Lerp, OnComplete,
    OpacityTween, RotateTween, ScaleTween, TranslateTween, Tween,
};
pub use components::{Node, ResolvedLayout, ResolvedTransform, StackingContext};
pub use focus::{
    FocusPlugin, FocusReturn, FocusRingMarker, FocusScope, FocusScopeMode, FocusVisible, Focusable,
    FocusedEntity,
};
pub use interaction::{InteractionPlugin, OnPress};
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
    Sizing, SnapAlign, SnapStop, SnapType, Stacking, Style, TextOrientation, TrackSize,
    TransformMatrix, TransformOrigin, TransformStyle, Translate, TryCondition, UiTransform,
    UnicodeBidi, WillChange, WillChangeProperty, WritingMode, WritingModeKind, WritingModeResolved,
};
// MVU substrate. Opt-in via `MvuCorePlugin`; the full surface
// (`Envelope`, `MsgLog`, `RecordMode`, `LogicalId`, `enqueue`, `PureEnv`, …) lives
// under `buiy_core::mvu`.
pub use mvu::{Cmd, Model, MvuAppExt, MvuCorePlugin, MvuModelExt, MvuSet, MvuWorkCounters};
pub use picking::{
    BuiyPickingBackendPlugin, MultiClick, PickingPlugin, global_paint_order, hit_test,
};
pub use render::color::{ColorToken, SystemColorKeyword};
pub use render::components::{
    AncestorClip, BackdropFilter, Background, Border, BorderSide, BoxShadow, ClipRadius, ClipRect,
    ComputedPaintSkip, Corners, CssVisibility, EffectGroup, EffectReason, Filter, FilterFn,
    LineStyle, MixBlendMode, Opacity, Outline, Radius, Shadow, SkipReason, TextColor,
};
pub use render::forced_colors::{PrePreferenceTheme, apply_forced_colors_theme};
pub use render::forced_colors_analyzer::{
    CatalogPaint, ForcedColorsViolation, analyze_forced_colors, analyze_shadow_only,
};
#[allow(deprecated)]
pub use render::golden::{GoldenConfig, perceptual_diff};
pub use scroll::{ScrollExtent, ScrollInputPlugin};
pub use text::{
    BuiyTextPlugin, ComputedTextLayout, FontFamily, FontSize, FontWeight, FontsGeneration,
    LineHeight, ResolvedBaseline, SharedFontSystem, Text, TextAlign, TextBuffer,
    TextCommitReshapeCount, TextMeasureCallCount, TextStyleDefaults, TextSyncAppliedCount,
    TextWrap, WhiteSpace,
};
// `OffscreenAuto` is intentionally NOT root-exported: it is a layout-written
// marker (layout owns its registration), reachable via `render::components`.

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
        // The shared `OnPress` activation sink (co-drive SC-1). Registered in
        // core (not `WidgetsPlugin`) so `Messages<OnPress>` exists for the
        // in-core producers (the P1c action router, the C3 pointer layer)
        // regardless of whether `buiy_widgets` is present.
        app.add_plugins(crate::interaction::InteractionPlugin);

        app.register_type::<Node>()
            .register_type::<ResolvedLayout>()
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

        app.init_resource::<crate::render::bridge::ScrollDirty>();
        // `mark_dirty_trees` / `propagate_parent_transforms` take
        // `Res<StaticTransformOptimizations>`, a resource `TransformPlugin`
        // normally inserts. The bridge schedules those two systems in `Update`
        // independently of `TransformPlugin`, so `CorePlugin` must supply the
        // resource or the standalone `Update` copies panic on missing-resource
        // param validation. `init_resource` is idempotent: when
        // `TransformPlugin` is also present (the harness, `BuiyPlugin`,
        // `DefaultPlugins`) the single shared resource is reused.
        app.init_resource::<StaticTransformOptimizations>();
        app.add_systems(
            Update,
            (
                crate::render::bridge::seed_scroll_dirty,
                crate::render::bridge::write_buiy_transform,
                // Bevy's three public propagation systems, chained in
                // dependency order (clip-and-transform.md § B.2.1). A DISTINCT
                // Update instance — NOT PostUpdate's TransformSystems::Propagate
                // set — so GlobalTransform is final before Picking + extract.
                // These run even without TransformPlugin (CorePlugin supplies
                // their StaticTransformOptimizations resource above; they are
                // otherwise inert until an entity carries a Transform, which the
                // bridge inserts). With TransformPlugin also present, its
                // PostUpdate chain re-propagates — an accepted cost (§ B.2.1).
                mark_dirty_trees,
                propagate_parent_transforms,
                sync_simple_transforms,
            )
                .chain()
                .after(BuiySet::Animate)
                .before(BuiySet::Picking),
        );
    }
}
