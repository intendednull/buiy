//! # Buiy — an accessible, web-quality UI library for [Bevy](https://bevyengine.org)
//!
//! Buiy is a comprehensive, AccessKit-first UI toolkit built as a **parallel UI stack to
//! `bevy_ui`** — not on top of it: a CSS-subset layout engine over Taffy, complex text +
//! editing via cosmic-text, a custom wgpu render pipeline that runs as a system in Bevy's
//! `Core2d` schedule, and an Elm-style MVU state funnel — all behind decomposed, ECS-native
//! components.
//!
//! > **Pre-0.1, pre-alpha.** APIs are unstable and may break in any commit.
//!
//! ## Quick start
//!
//! Add [`BuiyPlugin`] after `DefaultPlugins`, then spawn widgets:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use buiy::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(BuiyPlugin)
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn(Camera2d);
//!     commands.spawn(Button::new("Save")); // focusable, accessible; emits `OnPress`
//!     commands.spawn(TextInput::single_line("Search…")); // caret, selection, clipboard, IME
//! }
//! ```
//!
//! ## Prelude
//!
//! `use buiy::prelude::*;` brings the common surface — components, plugins, the widget catalog —
//! plus the `bsn!` authoring macros into scope in one import (the flat crate root re-exports the
//! same set for `use buiy::*;`).
//!
//! ## State (MVU)
//!
//! Widget state flows through the Model-View-Update funnel in `buiy_core::mvu` — now re-exported
//! through this crate's prelude (`Model` / `Msg` / a pure reducer / `Cmd` / `mvu_model` /
//! `enqueue`). See the `hello_button` counter and `todomvc` examples and the [getting-started
//! guide](https://github.com/intendednull/buiy/blob/main/docs/guide/getting-started.md).
//!
//! ## Feature flags
//!
//! - **`default_font`** *(default)* — embed the Fira Sans latin subset as the fallback font;
//!   disable to ship zero font bytes and supply your own.
//! - **`clipboard-image`** — the image clipboard flavor (`ClipboardImage`, `get_image` /
//!   `set_image`).
//! - **`multi_threaded`** — stay correct under Bevy's `multi_threaded` executor (opt-in).
//!
//! ## Design docs
//!
//! Architecture, specs, plans, and prior art live under
//! [`docs/`](https://github.com/intendednull/buiy/blob/main/docs/README.md) (start at the
//! [foundation design](https://github.com/intendednull/buiy/blob/main/docs/specs/2026-05-07-buiy-foundation/README.md)).

// (Bevy's prelude is not glob-imported here; the curated `pub use
// bevy::prelude::{…}` block below brings the ECS essentials into scope for both
// this crate's own code and — via `pub` — every `use buiy::prelude::*;` author.)

pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{
        A11yDescription,
        A11yExpanded,
        A11yLabel,
        A11yRole,
        A11yScroll,
        A11yScrollView,
        A11yTextValue,
        A11yToggled,
        A11yTreeBuilder,
        A11yValue,
        AccessKitAdapterPlugin,
        // The widget state components an app author queries to read live widget
        // state (Track C / F1) — paired with the domain accessors
        // (`Checkbox::checked`, `Switch::on`, `Slider::value`, `Disclosure::expanded`,
        // `TextInput::value`). `accesskit::Toggled` is deliberately NOT preluded —
        // the accessors return plain `bool`/`f64`/`&str` so the foreign enum never
        // surfaces at a call site.
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
        FontStack, FontWeight, FontsGeneration, GenericFamily, IntrinsicWidths, LetterSpacing,
        LineHeight, ResolvedBaseline, SharedFontSystem, Text, TextAlign, TextBuffer,
        TextCommitReshapeCount, TextMeasureCallCount, TextStyleDefaults, TextSyncAppliedCount,
        TextWrap, WhiteSpace,
    },
    theme::{SetAccent, Theme, UserPreferences, default_light_theme},
};
// Foundation primitives promoted to the crate root (→ `buiy::prelude` below).
// Animation: the `Tween<T>` per-property model + `Easing` curve enum + the
// `Repeat` loop control (`Once`/`Loop`/`PingPong` — the blink/pulse status dots
// the values table marks `infinite` need it) + the per-property tween
// *components* a `bsn!` author attaches to a node
// (`TranslateTween`/`RotateTween`/`ScaleTween`/`OpacityTween`/
// `BackgroundColorTween`) and the `AnimatedBackgroundColor` color marker. These
// are everyday authoring primitives (widget-catalog parity § 3.3 / spec § 2
// REFINE prelude promotions), so they belong next to the other component surface.
pub use buiy_core::animation::{
    AnimatedBackgroundColor, BackgroundColorTween, Easing, OpacityTween, Repeat, RotateTween,
    ScaleTween, TranslateTween, Tween,
};
// The gradient + vector-icon render primitives the Widget-Catalog parity work
// added (spec § 2 REFINE prelude promotions): the layered-background fan
// (`BackgroundLayers`/`BackgroundLayer` + `LinearGradient`/`RadialGradient`/
// `ColorStop`) and the `Icon` vector glyph. The gallery is the first real
// consumer and proved these are everyday authoring primitives (the logo
// gradient, the dotted-grid canvas, the 25-icon catalog), so they join the
// component surface next to `Background`/`Border` rather than living buried in
// `buiy_core::render::components`.
pub use buiy_core::render::components::{
    BackgroundLayer, BackgroundLayers, ColorStop, Icon, LinearGradient, RadialGradient,
};
// The editor's seed/set channel is the existing `EditCommand` verbs (`Insert`,
// `SelectAll` + `Insert` — no `SetValue` variant; the `EditCommand` surface is
// agent-interface-owned). Apps and the controlled `TextField` drive them through
// `EditCommand`, so the type belongs in the prelude next to the widget surface;
// `TextChanged` pairs with it (the message the Bug-3 fix keeps honest). Audit § 4.
pub use buiy_core::text::edit::{EditCommand, EditSubmitted, TextChanged, TextEditState};
// MVU (Model-View-Update) — Buiy's PRIMARY state interface. The demos→MVU
// dogfooding (docs/reports/2026-06-30-demos-mvu-migration-journal.md) found the #1
// app-author wall was that NONE of the MVU surface was preluded: an app had to take
// a *second*, direct `buiy_core` dependency and hunt for `enqueue` (which is not
// even at the `buiy_core` root). Prelude the everyday app-author surface so
// `use buiy::prelude::*;` is enough to define a `Model` + reducer, register it
// (`mvu_model`/`add_reducer`), place systems in the `MvuSet` windows, and `enqueue`
// messages. `fold_one_inline` (the synchronous seam that makes an in-place applier
// migration tractable — the gallery router) and `LogicalId`/`Envelope` (replay +
// the direct-inbox test idiom) round out the surface. (Recommendation #1 of the
// dogfooding report.)
pub use buiy_core::mvu::{
    Cmd, Envelope, LogicalId, Model, MvuAppExt, MvuCorePlugin, MvuModelExt, MvuSet,
    MvuWorkCounters, enqueue, fold_one_inline,
};
pub use buiy_widgets::{
    Button, Checkbox, Dialog, Disclosure, LightDismiss, Menu, MenuButton, MenuItem, OnPress,
    Popover, PopoverAlign, PopoverPlacement, PopoverSide, ScrollArea, Slider, Switch, TextInput,
    TooltipTrigger, WidgetsPlugin, dialog_invoker,
};
// The general composite builders (widget-catalog parity § 2 REFINE — "promote the
// genuinely-general composites to `buiy_widgets`"): imperative `World`-spawning
// trees that compose the primitive widgets/render components into a recognizable
// higher-level control (a progress `meter`, a `table_row`/`table_header`, a
// `search_input`, a `kbd` chip, a `status_dot` + its `pulse_blink`). They are
// font-NEUTRAL (each text-bearing builder takes a `FontFamily` — the app owns its
// typeface), so they fold into the prelude next to the widget surface for any app,
// not just the gallery they were extracted from.
pub use buiy_widgets::composites::{
    CMD_GLYPH_ICON, MeterFill, RowSelBar, TableRow, TableRowData, kbd, kbd_content, meter,
    pulse_blink, search_input, set_meter, set_table_row_selected, status_dot, table_header,
    table_row,
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

// Bevy ECS authoring essentials — the curated subset an app author needs to
// define components and WRITE SYSTEMS from `use buiy::prelude::*;` ALONE, with no
// second `use bevy::prelude::*;` (Track C / spec § "Track C"). This closes the
// prototype's N1 wall (the Buiy prelude could not express a Bevy system, forcing
// the colliding bevy glob) AND resolves the `Text`/`Node` collision *by
// construction*: because the prelude is now self-sufficient, a downstream
// `DefaultPlugins` app never needs `bevy::prelude::*`, so Buiy's `Text`/`Node`
// (and `Style`) stay unambiguous. Curated — NOT a blanket `pub use
// bevy::prelude::*` — only the names the N=4 probes reached for plus the minimum
// to wire an MVU app (grouped below; each group notes its load-bearing / excluded
// members).
// App + scheduling.
pub use bevy::prelude::{
    App, FixedUpdate, IntoScheduleConfigs, Plugin, PostUpdate, PreUpdate, Startup, Update,
};
// Derive macros + reflection. `Reflect` + `ReflectComponent` are load-bearing for
// the PRIMARY state path: the `Model` trait bounds `Reflect`, and every model
// derives `#[derive(Reflect)] #[reflect(Component)]` (the latter expands to name
// `ReflectComponent`) — so without these two, authoring an MVU `Model` from the
// prelude alone is impossible, which is exactly the "wire an MVU app" the slice
// promises.
pub use bevy::prelude::{Bundle, Component, Event, Message, Reflect, ReflectComponent, Resource};
// System params.
pub use bevy::prelude::{
    Commands, Local, MessageReader, MessageWriter, Query, Res, ResMut, Single,
};
// Query filters.
pub use bevy::prelude::{Added, Changed, Or, With, Without};
// Everyday components / types + the `default()` helper. `Visibility` is
// deliberately EXCLUDED: Buiy nodes hide via `CssVisibility`, so shadowing that
// with bevy's render `Visibility` would be a silent-wrong.
pub use bevy::prelude::{
    Camera2d, Color, Entity, Name, Time, Timer, TimerMode, Transform, default,
};

// BSN authoring (docs/specs/2026-06-18-buiy-bsn-integration-design.md § 4.2).
// `buiy::bsn` is the named path to the authoring crate; the BSN prelude
// (`bsn!`, `bsn_list!`, the spawn extension traits) is folded into the
// `buiy` crate root so the existing `use buiy::*;` convention
// (`hello_button` / `hello_text`) brings `bsn!` into scope — and into the
// `buiy::prelude` module below for the explicit `use buiy::prelude::*;` form.
pub use buiy_bsn as bsn;
pub use buiy_bsn::prelude::*;

/// The **view-authoring** sub-prelude (`buiy_view`, "safer V"): the whole
/// app-author surface is `Model` + `enum Msg` + `fn update` + `fn view`, where
/// `view(&Model) -> Element<Msg>` is a declarative description a library
/// reconciler + router realize onto real widgets and route back through the MVU
/// funnel — no hand-written `Changed<Model>` bind, no `OnPress → Model` routing.
/// See the [`buiy_view`] crate docs and
/// `docs/specs/2026-07-01-buiy-view-authoring-design.md`.
///
/// ## Why a distinct import path (not the flat prelude)
///
/// `buiy::view` is a **separate module** from [`prelude`], not folded into it,
/// because the `Element`-returning view builders (`button` / `checkbox` /
/// `text_input`) collide **name-for-name** with the `Scene`-returning `bsn!`
/// scene-fns already re-exported at the crate root (and thus in [`prelude`]).
/// Two `button`s in one glob is a hard error, so the surfaces stay on distinct
/// paths: a view-function author reaches for `buiy::view::*`; a `bsn!` scene
/// author keeps the untouched [`prelude`] scene-fns. Pull the MVU + Bevy types a
/// view app also needs (`Model` / `Cmd` / `App`) by **name** from `buiy::prelude`
/// (an explicit import does not glob-collide with the view builders).
///
/// Two papercuts, each resolved by importing the offender **by name**:
/// - the `column!` macro shares its name with the `std::column!` built-in, so
///   under a glob it is ambiguous — `use buiy::view::column;` (`row!` / `text!`
///   have no such collision);
/// - the typed tokens `Color` and `Radius` collide with `bevy::prelude::Color`
///   and the render `Radius` under the `use bevy::prelude::*;` + `use buiy::view::*;`
///   dual glob — import those by name too (`use buiy::view::{Color, Radius};`).
///
/// The shipped `counter_view` / `todomvc_view` examples import the whole surface
/// by name for the same reason.
///
/// ```no_run
/// use bevy::prelude::*;
/// use buiy::prelude::{BuiyPlugin, Cmd, Model}; // by name — no glob collision
/// use buiy::view::*; // Element, the Element-returning builders, ui(), tokens
/// use buiy::view::column; // disambiguate `column!` from the `std::column!` built-in
///
/// #[derive(Component, Default, Clone, PartialEq, Reflect)]
/// #[reflect(Component)]
/// struct Counter {
///     count: i32,
/// }
/// impl Model for Counter {
///     type Msg = Msg;
/// }
///
/// #[derive(Clone, Debug, PartialEq, Reflect)]
/// enum Msg {
///     Inc,
///     Dec,
///     Reset,
/// }
///
/// fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
///     match m {
///         Msg::Inc => s.count += 1,
///         Msg::Dec => s.count -= 1,
///         Msg::Reset => s.count = 0,
///     }
///     Cmd::none()
/// }
///
/// fn view(s: &Counter) -> Element<Msg> {
///     column![
///         text!("Count: {}", s.count).size(48.0),
///         row![
///             button("-").on_press(Msg::Dec),
///             button("+").on_press(Msg::Inc),
///             button("Reset").on_press_maybe((s.count != 0).then_some(Msg::Reset)),
///         ]
///         .gap(Space::Sm),
///     ]
///     .gap(Space::Md)
///     .padding(Space::Xl)
///     .align_center()
/// }
///
/// fn main() {
///     App::new()
///         .add_plugins((DefaultPlugins, BuiyPlugin))
///         .ui(Counter::default(), update, view) // ← the whole install
///         .run();
/// }
/// ```
pub mod view {
    // The Element-returning app-author surface (spec §1). `column` / `row` /
    // `text` re-export the `#[macro_export]`ed `column!` / `row!` / `text!`
    // macros by path (a single `pub use buiy_view::text` carries both the
    // builder fn and the `text!` macro — distinct namespaces, one path). The
    // widget scene-fns collide with these names, so this surface is a distinct
    // module, NOT flattened into `buiy::prelude` (see the module doc).
    pub use buiy_view::{
        BuiyViewAppExt, Color, Element, Radius, Space, button, checkbox, column, keyed_column, row,
        text, text_input, when,
    };
}

/// The agent-facing **probe / drive** surface (spec §4 Track A) — the front
/// door to Buiy's headless feedback loop. A coding agent runs a scene under
/// [`BuiyProbePlugin`] (GPU-free, no window/adapter), then *reads* it with
/// [`snapshot_report`](crate::probe::snapshot_report) and *drives* it with
/// [`click`](crate::probe::click) / [`get_by_role`](crate::probe::get_by_role) /
/// [`wait_for`](crate::probe::wait_for) — the whole author → run → inspect loop
/// with no pixels.
///
/// This is a distinct module, not flattened into [`prelude`] — but on
/// **altitude** grounds, not name collision (unlike [`view`], whose builders do
/// collide name-for-name with the widget scene-fns; these verbs flatten without
/// ambiguity). The probe surface is ~18 dev/test-driver symbols an *app author*
/// should not inherit wholesale just by writing `use buiy::prelude::*;`; it is a
/// separate front door you opt into when you are inspecting/driving a scene.
/// Reach the loop through one import:
///
/// ```no_run
/// use buiy::prelude::*;
/// use buiy::probe::*;
///
/// # fn build(app: &mut bevy::app::App) {
/// app.add_plugins(BuiyProbePlugin);
/// // …spawn a scene, step frames…
/// # let world = app.world_mut();
/// let report = snapshot_report(world);      // read the semantic tree
/// let save = get_by_role(world, A11yRole::Button, Some("Save"), None).unwrap();
/// click(world, save).unwrap();              // drive it
/// # }
/// ```
///
/// Built on the shipped [`a11y::inprocess`](buiy_core::a11y::inprocess) driver —
/// this module only re-exports it behind the probe preset as one coherent
/// front door.
pub mod probe {
    #[doc(inline)]
    pub use crate::BuiyProbePlugin;
    #[doc(inline)]
    pub use buiy_core::a11y::inprocess::{
        NodeState, ScrollState, SemanticNode, SemanticTree, StateQuery, TreeView, click, expand,
        focus, get_by_role, hide_tooltip, increment, perform, set_value, show_tooltip, snapshot,
        wait_for,
    };
    #[doc(inline)]
    pub use buiy_core::a11y::snapshot_report;
    #[doc(inline)]
    pub use buiy_core::a11y::{ActionError, NotActionableReason};
}

/// The Buiy prelude. `use buiy::prelude::*;` brings the common Buiy surface —
/// components, plugins, widgets, the MVU state funnel — the BSN authoring macros
/// (`bsn!`, `bsn_list!`) + spawn ext traits, **and a curated set of Bevy ECS
/// authoring essentials** (`Component`/`Commands`/`Query`/`Res`/`MessageReader`/
/// `With`/`Camera2d`/`App`/`Startup`/`Update`/…) into scope in one import. Mirrors
/// the flat crate-root re-export the examples use via `use buiy::*;`.
///
/// It is **self-sufficient**: you can define components *and wire systems* from
/// this one import, with no second `use bevy::prelude::*;` — which is also how
/// the `Text`/`Node` name-collision with `bevy::prelude` is avoided (you never
/// need the bevy glob, so Buiy's `Text`/`Node`/`Style` stay unambiguous).
///
/// ```no_run
/// use buiy::prelude::*;
///
/// #[derive(Component)]
/// struct Score(u32);
///
/// fn setup(mut commands: Commands) {
///     commands.spawn(Camera2d);
///     commands.spawn(Button::new("Save"));
///     commands.spawn(Score(0));
/// }
///
/// // Reads the activation sink + a query — the exact shape an agent writes, but
/// // which `buiy::prelude::*` alone could not express before Track C.
/// fn count_presses(mut presses: MessageReader<OnPress>, mut scores: Query<&mut Score>) {
///     for _press in presses.read() {
///         for mut score in &mut scores {
///             score.0 += 1;
///         }
///     }
/// }
///
/// fn main() {
///     App::new()
///         .add_plugins(BuiyPlugin)
///         .add_systems(Startup, setup)
///         .add_systems(Update, count_presses)
///         .run();
/// }
/// ```
///
/// The **MVU model** path — Buiy's primary state interface — is expressible from
/// the prelude alone too. The `Model` trait bounds `Reflect`, and a model derives
/// `#[derive(Reflect)] #[reflect(Component)]`, so `Reflect`/`ReflectComponent` are
/// part of the curated set — authoring a model no longer forces the bevy glob:
///
/// ```no_run
/// use buiy::prelude::*;
///
/// #[derive(Component, Default, Clone, PartialEq, Reflect)]
/// #[reflect(Component)]
/// struct Counter {
///     value: i64,
/// }
///
/// #[derive(Clone, Debug, PartialEq, Reflect)]
/// enum CounterMsg {
///     Increment,
///     Reset,
/// }
///
/// impl Model for Counter {
///     type Msg = CounterMsg;
/// }
///
/// fn update(model: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
///     match msg {
///         CounterMsg::Increment => model.value += 1,
///         CounterMsg::Reset => model.value = 0,
///     }
///     Cmd::none()
/// }
///
/// fn main() {
///     App::new()
///         .add_plugins(BuiyPlugin)
///         .mvu_model(update) // register_type + add_model + add_reducer, one call
///         .app() // ModelWiring handle → &mut App
///         .run();
/// }
/// ```
///
/// Reading live **widget state** is a plain `bool`/`f64`/`&str` via the domain
/// accessors — the widget state components and the accessors are both in the
/// prelude, and the foreign `accesskit::Toggled` enum never appears:
///
/// ```no_run
/// use buiy::prelude::*;
///
/// // `A11yToggled` (the state) + `Checkbox` (the accessor namespace) are both
/// // preluded; `Checkbox::checked` returns `bool`, not `accesskit::Toggled`.
/// fn read_checkboxes(boxes: Query<&A11yToggled, With<Checkbox>>) {
///     let checked = boxes.iter().filter(|t| Checkbox::checked(t)).count();
///     let _ = checked;
/// }
/// ```
pub mod prelude {
    pub use crate::*;
}

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
/// panics when a `Res<T>` system param is missing, so the plugin
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
            // Tween registry (widget-catalog parity § 3.3 / § 8): the per-property
            // tween-update systems wired into the existing `BuiySet::Animate`.
            // Added before `WidgetsPlugin` since widgets spawn tweens.
            buiy_core::animation::AnimationPlugin,
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

/// The **headless render subset** of the Buiy stack: everything needed to lay out,
/// shape, and *paint* a UI tree to an offscreen target, with **no winit window and
/// no picking** (no live pointer input). It composes exactly the data-and-render
/// sub-plugins — `core` · `theme` · `a11y` · `focus` · `layout` · `text` · `widgets`
/// · `render` — and deliberately omits:
///
/// - **winit / OS bridges:** `PointerInputPlugin` (the winit cursor reader) and
///   `AccessKitAdapterPlugin` (the OS-accessibility bridge) — both need a real
///   window. The in-process a11y *tree* (`A11yPlugin`) is kept; only the OS adapter
///   is dropped.
/// - **picking:** `bevy::picking::PickingPlugin` + Buiy's `PickingPlugin` /
///   `BuiyPickingBackendPlugin` — a static capture forces widget state directly
///   instead of routing pointer hits.
/// - **runtime interaction:** `ScrollInputPlugin` and `AnimationPlugin` — not needed
///   to paint a single settled frame. A capture that wants tween motion (e.g. a
///   progress meter ramp) adds [`buiy_core::animation::AnimationPlugin`] on top.
///
/// This is the production replacement for the hand-rolled, drift-prone ~8-line
/// plugin list the offscreen capture bins used to maintain (widget-catalog parity
/// § 2 REDESIGN). The plugin order mirrors [`BuiyPlugin`]'s canonical order with the
/// excluded plugins removed.
///
/// # Required Bevy plugins
///
/// `BuiyHeadlessPlugin` is the *Buiy* subset only; the caller composes the headless
/// **Bevy** stack itself (and adds it BEFORE this plugin so the `RenderApp` exists
/// when `BuiyRenderPlugin::build` runs — the same "after the render plugin" contract
/// [`BuiyPlugin`] documents). A minimal offscreen harness adds, in order:
/// `MinimalPlugins`, a sized `WindowPlugin`, `AssetPlugin`, `ScenePlugin`,
/// `RenderPlugin`, `ImagePlugin`, `CameraPlugin`, `CorePipelinePlugin`, and
/// `bevy::input::InputPlugin` (the focus/keymap systems read
/// `Res<ButtonInput<KeyCode>>`), then `BuiyHeadlessPlugin`.
///
/// ```no_run
/// use bevy::prelude::*;
/// use buiy::BuiyHeadlessPlugin;
///
/// App::new()
///     .add_plugins(MinimalPlugins)
///     // … the offscreen Bevy render stack (RenderPlugin, CameraPlugin, …) …
///     .add_plugins(bevy::input::InputPlugin)
///     .add_plugins(BuiyHeadlessPlugin)
///     .run();
/// ```
pub struct BuiyHeadlessPlugin;

impl Plugin for BuiyHeadlessPlugin {
    fn build(&self, app: &mut App) {
        // The subset of `BuiyPlugin`'s composition that paints a frame, in the same
        // relative order, minus winit/picking/scroll/animation (see the type doc).
        app.add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            WidgetsPlugin,
            buiy_core::render::BuiyRenderPlugin,
        ));
    }
}

/// The **GPU-free probe preset** — the agent's "eyes." It composes only the
/// data-and-projection sub-plugins Buiy needs to lay out a scene and project its
/// **semantic tree** + **widget state**, with **no render, no adapter, no
/// window**: `core` · `theme` · `a11y` · `focus` · `layout` · `text` · `widgets`.
/// Layout + a11y + widget state are **pure ECS projections** — a Taffy solve, an
/// `A11yTreeBuilder` fold, and component reads — so the agent's
/// author → build → run → *inspect* feedback loop runs fully headless: spawn a
/// scene, step a few frames, and read it back with
/// [`snapshot`](buiy_core::a11y::inprocess::snapshot) /
/// [`snapshot_report`](buiy_core::a11y::snapshot_report) — no wgpu adapter, no
/// `RenderApp`, no OS accessibility bridge.
///
/// It differs from [`BuiyHeadlessPlugin`] on exactly one axis: the headless
/// preset keeps [`BuiyRenderPlugin`](buiy_core::render::BuiyRenderPlugin) (to
/// *paint* an offscreen frame, so it needs a `RenderApp` + a real wgpu adapter);
/// the probe **omits** it (nothing rasterizes, so no adapter is needed). Both
/// omit winit/picking (a probe forces widget state via the
/// [`perform`](buiy_core::a11y::inprocess::perform) action seam rather than
/// routing live pointer hits); the probe additionally leaves out scroll +
/// animation for the same "one settled inspection frame" reason
/// [`BuiyHeadlessPlugin`] documents (a probe that wants tween motion or scroll
/// geometry adds [`AnimationPlugin`](buiy_core::animation::AnimationPlugin) /
/// [`ScrollInputPlugin`] on top).
///
/// Realizes Track A of the agent-interface north-star spec (the probe / "eyes"):
/// packages the *shipped* `a11y::inprocess` driver behind a preset an agent's
/// build/test loop can stand up with no GPU.
///
/// # Required Bevy plugins
///
/// `BuiyProbePlugin` is the *Buiy* subset only; the caller supplies the small
/// **Bevy** substrate a headless-no-render app needs, added BEFORE this plugin:
/// `MinimalPlugins`, `AssetPlugin` (the text stack loads its fallback font as an
/// asset), and `bevy::input::InputPlugin` (the focus/keymap systems read
/// `Res<ButtonInput<KeyCode>>`). No `WindowPlugin`, no `RenderPlugin`, no
/// `CameraPlugin` — the probe never touches a surface.
///
/// ```no_run
/// use bevy::prelude::*;
/// use buiy::{BuiyProbePlugin, Button};
/// use buiy_core::a11y::inprocess::{snapshot, TreeView};
///
/// let mut app = App::new();
/// app.add_plugins(MinimalPlugins)
///     .add_plugins(bevy::asset::AssetPlugin::default())
///     .add_plugins(bevy::input::InputPlugin)
///     .add_plugins(BuiyProbePlugin);
/// app.world_mut().spawn(Button::new("Save"));
/// for _ in 0..8 {
///     app.update();
/// }
/// let tree = snapshot(app.world_mut(), TreeView::Unmerged);
/// assert!(tree.nodes.iter().any(|n| n.name == "Save"));
/// ```
pub struct BuiyProbePlugin;

impl Plugin for BuiyProbePlugin {
    fn build(&self, app: &mut App) {
        // `BuiyHeadlessPlugin`'s composition minus `BuiyRenderPlugin` — the
        // GPU-free subset (see the type doc). Same relative order.
        app.add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            WidgetsPlugin,
        ));
    }
}
