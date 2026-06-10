//! Buiy — comprehensive UI library for Bevy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/README.md.

use bevy::prelude::*;

pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder, AccessKitAdapterPlugin},
    components::{Node, ResolvedLayout, ResolvedTransform, StackingContext},
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
    render::color::ColorToken,
    render::components::{
        Background, Border, BorderSide, Corners, CssVisibility, Opacity, Radius, TextColor,
    },
    text::{
        BuiyTextPlugin, ComputedTextLayout, ComputedTextLine, FamilyEntry, FontFamily, FontSize,
        FontStack, FontWeight, FontsGeneration, GenericFamily, IntrinsicWidths, LineHeight,
        ResolvedBaseline, SharedFontSystem, Text, TextAlign, TextBuffer, TextCommitReshapeCount,
        TextMeasureCallCount, TextStyleDefaults, TextSyncAppliedCount, TextWrap, WhiteSpace,
    },
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
/// `BuiyPlugin` does **not** itself require `bevy::transform::TransformPlugin`
/// for the bridge to produce a correct `GlobalTransform` (clip-and-transform.md
/// § B): `CorePlugin` schedules `write_buiy_transform` plus a distinct `Update`
/// copy of Bevy's `mark_dirty_trees → propagate_parent_transforms →
/// sync_simple_transforms`, and seeds their `StaticTransformOptimizations`
/// resource itself, so `GlobalTransform` is already final before
/// `BuiySet::Picking` and extract with no `TransformPlugin` present (§ B.2.1's
/// escape hatch — a UI-only app pays propagation exactly once, in `Update`).
///
/// Add `TransformPlugin` when you want Bevy's *canonical* late propagation pass:
/// `TransformPlugin` re-runs the same chain in `PostUpdate` (and `PostStartup`),
/// reconciling any `Transform` an app system mutates *after* the `Update` window
/// has closed — without it, such a late mutation is not reflected in
/// `GlobalTransform` until the next frame's `Update` chain. `DefaultPlugins`
/// includes `TransformPlugin`; on `MinimalPlugins` add it explicitly if your app
/// edits `Transform` outside the bridge's `Update` window.
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
        // bevy_picking::PickingPlugin registers the PickingSystems system sets
        // and the Messages<PointerHits> message resource that both of Buiy's
        // picking plugins depend on, so it must be added before them. Bevy 0.18's
        // `DefaultPlugins` ALREADY includes it — adding it unconditionally
        // panicked every real app ("plugin was already added", hit by
        // `cargo run -p hello_button`) — so guard it: a library plugin supplies a
        // dependency only when the app hasn't (the headless MinimalPlugins tests).
        if !app.is_plugin_added::<bevy::picking::PickingPlugin>() {
            app.add_plugins(bevy::picking::PickingPlugin);
        }
        app.add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::a11y::AccessKitAdapterPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::picking::PickingPlugin,
            buiy_core::picking::BuiyPickingBackendPlugin,
            // Text engine foundation (buiy-text-rendering-design T1): the
            // shared FontSystem + the FontsGeneration reshape trigger.
            // System-font scan stays opt-in/off in the composed default.
            buiy_core::text::BuiyTextPlugin::default(),
            WidgetsPlugin,
            // The render plugin is added in `build`, NOT `finish`: Bevy's
            // `App::finish` iterates `0..plugin_registry.len()` with the length
            // captured BEFORE the loop, so a plugin added DURING another
            // plugin's `finish` never gets its own `finish()` called — and
            // `BuiyRenderPlugin::finish` is where the device-dependent
            // `BuiyPipeline` / `AtlasGpu` register. The old finish-time add left
            // them unregistered in every real app (`prepare_atlas_textures`
            // panicked "Resource does not exist" on frame 1). Adding here is
            // the standard ecosystem convention: `BuiyPlugin` is documented to
            // come AFTER `DefaultPlugins`, so the `RenderApp` already exists;
            // without one (headless tests) the plugin's own guard no-ops its
            // render half.
            buiy_core::render::BuiyRenderPlugin,
        ));
    }
}
