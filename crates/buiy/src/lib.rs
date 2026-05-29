//! Buiy — comprehensive UI library for Bevy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/README.md.

use bevy::prelude::*;

pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder, AccessKitAdapterPlugin},
    components::{Node, ResolvedLayout, ResolvedTransform, StackingContext, Visual},
    focus::{FocusVisible, Focusable, FocusedEntity},
    layout::{
        AlignContent, AlignItems, Anchor, AnchorErrorKind, AnchorName, AnchorRef, AspectRatio,
        BackfaceVisibility, BoxModel, BoxSizing, BreakAfter, BreakBefore, BreakInside,
        BuiyLayoutStep, ColumnCount, ColumnFill, ColumnRule, ColumnRuleStyle, ColumnSpan,
        ContainFlags, ContainIntrinsicSize, Containment, ContentVisibility,
        ContentVisibilityMargin, Direction, Display, Edges, FlexAxis, FlexGap, FlexItem,
        FlexParams, FlexWrap, GridAreas, GridAutoFlow, GridItem, GridLine, GridParams, Inset,
        Isolation, JustifyContent, JustifyItems, LayoutAnchorBroken, LayoutPlugin,
        LayoutWarnOnceKey, LayoutWarnedOnceSession, Length, LogicalBoxModel, LogicalEdges,
        LogicalInset, MultiColumn, NamedArea, Overflow, OverflowMode, OverscrollBehavior, Position,
        PositionKind, PositionTry, PostTaffyPositionOverrides, RepeatCount, Rotate, Scale, Scroll,
        ScrollBehavior, ScrollOffset, ScrollSnapItem, ScrollbarColor, ScrollbarGutter,
        ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType, Stacking, Style, TextOrientation,
        TopLayer, TopLayerActivation, TrackSize, TransformMatrix, TransformOrigin, TransformStyle,
        Translate, TryCondition, UiTransform, UnicodeBidi, WillChange, WillChangeProperty,
        WritingMode, WritingModeKind, WritingModeResolved, ZIndex,
    },
    picking::{BuiyPickingBackendPlugin, Hovered},
    theme::{Theme, UserPreferences, default_light_theme},
};
pub use buiy_widgets::{Button, OnPress, WidgetsPlugin};

// `buiy_core::render::ExtractedDraws` is intentionally NOT re-exported at
// the crate root: it is a render-world resource only, populated during the
// extract phase. Main-world consumers reading it would see an empty Vec.
// Render-world plugin authors who need it can reach `buiy::buiy_core::render`
// (or depend on `buiy_core` directly) without crate-root surface pollution.

/// Top-level Buiy plugin. Composes sub-plugins in the documented order:
/// core → theme → a11y → focus → input → widgets. Render registration
/// happens in `Plugin::finish` so RenderApp exists when we reach it.
///
/// See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.8.
///
/// # Required Bevy plugins
///
/// `BuiyPlugin` requires `bevy::input::InputPlugin`. `DefaultPlugins`
/// includes it; if you build your app with `MinimalPlugins`, add it
/// explicitly:
///
/// ```ignore
/// App::new()
///     .add_plugins(MinimalPlugins)
///     .add_plugins(bevy::input::InputPlugin)
///     .add_plugins(BuiyPlugin)
///     .run();
/// ```
///
/// `FocusPlugin::handle_tab` reads `Res<ButtonInput<KeyCode>>` and the
/// `Button` click handler reads `Res<ButtonInput<MouseButton>>`. Bevy
/// 0.18 panics when a `Res<T>` system param is missing, so the plugin
/// must be present.
///
/// `BuiyPlugin` also composes `bevy::picking::PickingPlugin` (the core
/// bevy_picking infrastructure), so you do not need to add it separately.
/// If you are using `DefaultPlugins`, bevy_picking is not included by
/// default; `BuiyPlugin` adds it for you.
///
/// # Plugin order
///
/// Add `BuiyPlugin` **after** Bevy's render plugin (i.e., after
/// `DefaultPlugins`). `BuiyPlugin::finish` registers `BuiyRenderPlugin`,
/// whose `build` reads `PipelineCache` — a resource that
/// `RenderPlugin::finish` inserts. Plugin `finish` runs in registration
/// order, so adding `BuiyPlugin` before `DefaultPlugins` flips the order
/// and panics when `BuiyRenderPlugin` reaches for the missing
/// `PipelineCache`.
pub struct BuiyPlugin;

impl Plugin for BuiyPlugin {
    fn build(&self, app: &mut App) {
        // Sub-plugin order matches architecture.md § 2.8: core → theme → a11y →
        // focus → input → text → widgets. Phase 0 omits text/animation/forms/
        // devtools. LayoutPlugin, bevy::picking::PickingPlugin, Buiy's
        // PickingPlugin, and BuiyPickingBackendPlugin slot between Focus and
        // Widgets so widgets see resolved layout + hit-test results in the same
        // frame. bevy_picking must come first because it registers PickingSystems
        // sets + Messages<PointerHits>; the two Buiy plugins consume both.
        app.add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::a11y::AccessKitAdapterPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            // bevy_picking::PickingPlugin registers PickingSystems system sets
            // and the Messages<PointerHits> message resource that both
            // PickingPlugin and BuiyPickingBackendPlugin depend on. Must come
            // before the two Buiy picking plugins.
            bevy::picking::PickingPlugin,
            buiy_core::picking::PickingPlugin,
            buiy_core::picking::BuiyPickingBackendPlugin,
            WidgetsPlugin,
        ));
    }

    fn finish(&self, app: &mut App) {
        // RenderApp is guaranteed to exist by `finish` time.
        app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    }
}
