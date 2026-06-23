//! Buiy — comprehensive UI library for Bevy.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/README.md.

use bevy::prelude::*;

pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{
        A11yDescription, A11yLabel, A11yRole, A11yScroll, A11yScrollView, A11yTreeBuilder,
        AccessKitAdapterPlugin,
    },
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
    picking::{BuiyPickingBackendPlugin, MultiClick},
    render::color::ColorToken,
    render::components::{
        Background, Border, BorderSide, Corners, CssVisibility, Opacity, Radius, TextColor,
    },
    scroll::{ScrollExtent, ScrollInputPlugin},
    text::{
        BuiyTextPlugin, ComputedTextLayout, ComputedTextLine, FamilyEntry, FontFamily, FontSize,
        FontStack, FontWeight, FontsGeneration, GenericFamily, IntrinsicWidths, LineHeight,
        ResolvedBaseline, SharedFontSystem, Text, TextAlign, TextBuffer, TextCommitReshapeCount,
        TextMeasureCallCount, TextStyleDefaults, TextSyncAppliedCount, TextWrap, WhiteSpace,
    },
    theme::{Theme, UserPreferences, default_light_theme},
};
// The editor's seed/set channel is the existing `EditCommand` verbs (`Insert`,
// `SelectAll` + `Insert` — no `SetValue` variant; the `EditCommand` surface is
// agent-interface-owned). Apps and the controlled `TextField` drive them through
// `EditCommand`, so the type belongs in the prelude next to the widget surface;
// `TextChanged` pairs with it (the message the Bug-3 fix keeps honest). Audit § 4.
pub use buiy_core::text::edit::{EditCommand, TextChanged};
pub use buiy_widgets::{
    Button, Checkbox, Dialog, Disclosure, LightDismiss, Menu, MenuButton, MenuItem, OnPress,
    Popover, PopoverAlign, PopoverPlacement, PopoverSide, ScrollArea, Slider, Switch, TextInput,
    TooltipTrigger, WidgetsPlugin, dialog_invoker,
};
// Widget BSN scene-fns (the mergeable styled-authoring path): `button(label)`,
// `checkbox(label)`, `switch(label)`, `slider(label, now, min, max, step)`,
// `disclosure(label)`, `dialog(title, body)`, `tooltip_trigger(label, tip)`,
// `text_input_single_line(placeholder)`, `text_input_multi_line(placeholder)`.
// They live in `buiy_widgets` (so they reuse the `#[require]` initializer fns —
// one source of truth) and surface here, where the widget + BSN surfaces
// converge, so `use buiy::prelude::*;` brings them in next to `bsn!`. (They are
// NOT re-exported through `buiy_bsn`, which stays widget-agnostic per spec § 4.2
// — it must not take a `buiy_widgets` dependency.) The Wave-3 widget scene-fns
// are aliased (`checkbox_scene` / `switch_scene` / `slider_scene` /
// `disclosure_scene` / `dialog_scene`) inside `buiy_widgets` to avoid colliding
// with the `Checkbox` / `Switch` / `Slider` / `Disclosure` / `Dialog` markers; the
// prelude renames them back to `checkbox` / `switch` / `slider` / `disclosure` /
// `dialog`. (`tooltip_trigger` does not collide — the marker is `TooltipTrigger`.)
pub use buiy_widgets::scene::{
    button, checkbox, dialog, disclosure, menu, menu_button, menu_item, popover, scroll_area,
    slider, switch, text_input_multi_line, text_input_single_line, tooltip_trigger,
};

// bevy_picking surface (input-event-model.md § 2.9): re-export every
// bevy_picking type Buiy users touch through `buiy` (and the prelude below)
// so a pre-1.0 upstream rename touches this one file. `Pickable` is the
// widget-internal pick-through convention (`Pickable::IGNORE` on decorative
// children); the `Pointer<E>` family is the C3 event taxonomy widgets observe;
// `PointerButton` is carried by `MultiClick`. C3c deleted Buiy's own `Hovered`
// resource, so the name collision the staged migration guarded against is gone;
// bevy's hover *components* (`Hovered`/`DirectlyHovered`) are now reachable as
// the canonical hover surface via `bevy::picking` for any "is this hovered"
// query (re-exporting them under the Buiy prelude is a clean additive follow-up,
// not part of the C3c consumer migration).
pub use bevy::picking::Pickable;
pub use bevy::picking::events::{
    Cancel, Click, Drag, DragDrop, DragEnd, DragEnter, DragLeave, DragOver, DragStart, Move, Out,
    Over, Pointer, Press, Release,
};
pub use bevy::picking::pointer::PointerButton;
// The wheel event `bevy::picking::events::Scroll` is NOT flattened here: the
// name collides with the layout `Scroll` overflow component, which owns the
// flat prelude name. The wheel entry (§2.6) is the `Pointer<E>` event reached as
// `buiy::events::Scroll` (the picking events module, re-exported below); a
// `ScrollArea` (C5) observes `Pointer<buiy::events::Scroll>`.
pub use bevy::picking::events;

// BSN authoring (docs/specs/2026-06-18-buiy-bsn-integration-design.md § 4.2).
// `buiy::bsn` is the named path to the authoring crate; the BSN prelude
// (`bsn!`, `bsn_list!`, the spawn extension traits) is folded into the
// `buiy` crate root so the existing `use buiy::*;` convention
// (`hello_button` / `hello_text`) brings `bsn!` into scope — and into the
// `buiy::prelude` module below for the explicit `use buiy::prelude::*;` form.
pub use buiy_bsn as bsn;
pub use buiy_bsn::prelude::*;

/// The Buiy prelude. `use buiy::prelude::*;` brings the common Buiy surface —
/// components, plugins, widgets — and the BSN authoring macros (`bsn!`,
/// `bsn_list!`) + spawn extension traits into scope in one import. Mirrors the
/// flat crate-root re-export the examples use via `use buiy::*;`.
pub mod prelude {
    pub use crate::*;
}

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
/// ```no_run
/// use bevy::prelude::*;
/// use buiy::BuiyPlugin;
///
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
        // C3b §2.1: the winit-cursor reader that gathers raw pointer input
        // (cursor move / button / wheel) into `PointerInput` and updates
        // `PointerLocation`/`PointerPress`. Buiy's `PickingPlugin` adds the hover
        // stage (`InteractionPlugin`) that turns the resulting `PointerHits` into
        // the `Pointer<E>` taxonomy; this plugin feeds it the real input. Guarded
        // like the core plugin above — `DefaultPlugins` includes it via
        // `DefaultPickingPlugins`, `MinimalPlugins` does not. (The headless test
        // harness injects `PointerInput` directly and does NOT add this — adding
        // it would spawn a duplicate `PointerId::Mouse`.)
        // Gate on `WindowPlugin`: the winit reader's systems read
        // `MessageReader<WindowEvent>` (registered by `WindowPlugin`), so adding it
        // to a headless `MinimalPlugins` app (no `WindowPlugin`) panics with
        // "Message not initialized" on the first frame. Real windowed apps have it
        // via `DefaultPlugins`; the test harness injects `PointerInput` directly
        // and needs neither this plugin nor a window.
        if app.is_plugin_added::<bevy::window::WindowPlugin>()
            && !app.is_plugin_added::<bevy::picking::input::PointerInputPlugin>()
        {
            app.add_plugins(bevy::picking::input::PointerInputPlugin);
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
            // C5-a (scroll-overlay-modal.md §A): the scroll input pipeline —
            // `Pointer<Scroll>` → clamped `ScrollOffset`, keyboard scroll, the
            // `ScrollExtent` cache, and the SC-4 `A11yScroll` source sync.
            buiy_core::scroll::ScrollInputPlugin,
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
