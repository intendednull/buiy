//! `buiy_gallery` — the widget-catalog campaign's runnable exemplar app and the
//! `buiy_verify` screen-fixture source. **C8-a delivers S1 (TodoMVC); C8-b adds
//! S2 (scroll / long-list) + S3 (overlay / menu); C8-c adds S4 (modal + focus-
//! trap) + S5 (F-tier look showcase).**
//!
//! S1 composes the landed P1d widget bundles (single-line [`TextInput`], the
//! tri-state [`Checkbox`], [`Button`]) + the A11yLive `Status` live region into
//! the literal TodoMVC exemplar: type the "What needs to be done?" field + Enter
//! to add a row, toggle a row's checkbox to complete it, destroy a row, clear
//! completed, filter All/Active/Completed, and a "N items left" count that lives
//! in an `A11yRole::Status` aria-live region. Double-clicking a row's label edits
//! it in place (C3b `MultiClick`).
//!
//! **S2 ([`spawn_scroll_screen`]) — the entity-tree Virtual List (the scale
//! game).** The design's 1000-node entity-tree table: a heading (H1 + total +
//! search box) over a table card (sticky header → a [`ScrollArea`] viewport of
//! [`SCROLL_LIST_ROWS`] (~1000) `table_row`s → footer). The container owns
//! keyboard scroll (PageDown/End) and the SC-4 `A11yScroll` source the a11y fold
//! projects (off-screen rows ride the landed `ContentVisibility::Auto` skip — the
//! documented v1 windowing ceiling). [`ScrollListPlugin`] wires the live
//! search-filter + single-select row selection.
//!
//! **S3 ([`spawn_overlay_menu`]) — overlays.** A file **card** whose ⋮
//! [`MenuButton`] opens an anchored [`Menu`] of 5 file-action [`MenuItem`]s
//! (Open / Rename / Duplicate / Copy link / Delete — roving / activedescendant),
//! with a footer "last action" indicator; plus the catalog's other two overlay
//! primitives — a [`TooltipTrigger`] (show/hide its tip) and a standalone anchored
//! [`Popover`] (light-dismiss). The menu item activation flows through the shared
//! `OnPress` sink into an observable effect ([`MenuActivations`]) **and** rewrites
//! the footer value, so the driver can drive open → arrow-nav → Enter-activate →
//! Esc/outside-close and observe each step through the a11y tree.
//!
//! **S4 ([`spawn_modal`]) — modal + focus-trap.** A trigger Button (the invoker)
//! that `controls` a C5-d `Dialog` (title + body + a focusable `Switch` + a
//! `DialogClose` button) and a focusable background button OUTSIDE the dialog.
//! Activating the invoker opens the dialog (the `A11yModal` is in the snapshot +
//! focus moves inside), Tab traps + wraps inside the modal (never the background),
//! Escape closes + restores focus to the invoker, and the background is pruned from
//! the a11y tree (`A11yHidden`) while open + restored on close. The whole lifecycle
//! is the C5-d `WidgetsPlugin` overlay state machine; S4 is **pure composition**
//! (no app systems). The dialog is spawned imperatively (the invoker references the
//! dialog entity, which a scene cannot name), like S3's standalone popover.
//!
//! **S5 ([`spawn_showcase`]) — the Controls grid (parity Wave C3).** The design's
//! 2-column controls grid (`display:grid; grid-template-columns:1fr 1fr`): five
//! flat `surface.card` cards — three [`Switch`] toggle rows, a [`Slider`] driving a
//! live-radius gradient preview square, a Segmented + Stepper card, a Meter +
//! "Run build" card (the press animates the meter 0→100%), and a full-width
//! ([`GridLine::StartEnd`]`(1, -1)`) [`Disclosure`] accordion (three items). The
//! widgets keep their real a11y bundles but carry custom design pixels (the
//! `append_row` checkbox precedent), with [`ShowcasePlugin`] driving the visual from
//! the a11y state. The driver drives each widget (Switch toggles, Slider increments,
//! Disclosure expands) and the display-list acceptance asserts the cards emit border
//! bands + the slider preview emits its glow shadow + a keyboard-focused widget
//! emits the focus-ring Outline (`modal_showcase_c8c.rs`).
//!
//! **Pure composition (C8 contract).** The crate defines no widget bundle, no
//! a11y-state component, and no primitive. [`spawn_todomvc_screen`] builds the
//! static frame; [`TodoMvcPlugin`] registers the retained-mode app systems that run
//! `.after(BuiySet::Input)` (C8 §3.1). The binary boots the screen under
//! `BuiyPlugin + TodoMvcPlugin`; the `buiy_verify` fixture builds the same
//! [`spawn_todomvc_screen`] tree (the "example IS the fixture" discipline), and the
//! inspection-driver acceptance test adds [`TodoMvcPlugin`] to drive behavior.
//!
//! ## App-logic shape (the retained-mode pattern, C8 §3.4 KEEP)
//!
//! Activation/submit are `Message`s emitted in `BuiySet::Input`. The intent
//! systems (`collect_*`) are ordinary `MessageReader` systems that run
//! `.after(BuiySet::Input)` and stage their reads into [`TodoIntents`]; the
//! `apply_intents` exclusive system then performs the structural mutations
//! (append/despawn rows, clear/seed editors) over `&mut World` and clears the
//! staging. Splitting "read messages once" (each `Message` has exactly one
//! reader, so no double-buffer re-read) from "mutate the tree" keeps the
//! exclusive system free of `MessageReader` cursor pitfalls. The pure
//! change-detection systems (`apply_filter`, `update_count`, `restyle_completed`)
//! run last and need no exclusive access.
//!
//! Design: `docs/specs/2026-06-23-c8a-todomvc-s1-design.md`.

/// The unified IDE-style shell (parity Wave C1): `ScreenRouter`, the `shell_root`
/// scene (chrome / rail / viewport / inspector / status), and nav switching. The
/// binary boots [`shell::ScreenRouterPlugin`] to host all 5 screens in one window.
pub mod shell;

/// The composite widgets (parity Wave C2): `stepper` / `segmented` /
/// `search_input` / `meter` / `toast` / `badge` / `chip` / `kbd` / `status_dot` /
/// `stat_row` / `table_row` — the reusable scene-builders the design's screens are
/// composed from, each styled to exact parity against the values table.
pub mod composites;

/// The Inspector pane content + live accent theming (parity Wave C4): fills the
/// shell's inspector stub with the active screen's name/desc, "Composed of" chips,
/// "Live state" rows (refreshed every frame from the screens' ECS state), and the
/// 4 accent swatches (a press writes `SetAccent`, re-theming the whole app live).
pub mod inspector;

use bevy::prelude::{
    App, Camera2d, Changed, ChildOf, Children, Commands, Component, DetectChanges, Entity, Has,
    IntoScheduleConfigs, MessageReader, Messages, Name, On, Plugin, Quat, Query, Ref, Res, ResMut,
    Resource, Update, With, Without, World, children,
};
use buiy::prelude::*;
use buiy_core::BuiySet;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{
    A11yExpanded, A11yHidden, A11yLabel, A11yOrientation, A11yRelations, A11yRole, A11yToggled,
    A11yValue, Orientation, Toggled, set_value,
};
use buiy_core::focus::FocusedEntity;
use buiy_core::interaction::OnPress;
use buiy_core::mvu::{Envelope, ToggleLeafSet, ToggleMsg};
use buiy_core::render::components::{
    BackdropFilter, BackgroundLayer, BackgroundLayers, BorderSide, BoxShadow, ColorStop,
    CssVisibility, FilterFn, Icon, LineStyle, LinearGradient, Opacity, Shadow, TextColor,
};
use buiy_core::text::edit::{EditSubmitted, TextEditState};
use buiy_core::text::{
    DecorationLineStyle, DecorationLines, FamilyEntry, FontStack, LetterSpacing, Text,
    TextDecorations,
};
use buiy_widgets::checkbox::CheckboxMark;

/// The whole gallery as one plugin: the **dark theme** + the shell router + all
/// five screens' app plugins + the inspector pane + the shared toast lifecycle.
/// Both entry points add this — the native binary (`buiy_gallery`) and the WebGPU
/// example (`gallery_web`) — so the screen wiring lives in exactly one place and
/// the two `main`s differ only in their window setup.
///
/// Boots `default_dark_theme` so the design's dark tokens resolve (the framework
/// default theme is light — the gallery opts in here; Wave A reconciliation note).
pub struct GalleryPlugin;

impl Plugin for GalleryPlugin {
    fn build(&self, app: &mut App) {
        // The framework ships light by default; the gallery opts into dark.
        app.insert_resource(buiy_core::theme::default_dark_theme());
        // The shell router spawns the shell tree + all 5 screens at boot; the
        // per-screen plugins supply the retained-mode app logic each needs, and
        // `ToastPlugin` drives the shared toast lifecycle. The router toggles which
        // screen is laid out + a11y-visible, preserving per-screen state.
        app.add_plugins((
            shell::ScreenRouterPlugin,
            inspector::InspectorPlugin,
            TodoMvcPlugin,
            ScrollListPlugin,
            OverlayMenuPlugin,
            ModalPlugin,
            ShowcasePlugin,
            composites::ToastPlugin,
        ));
    }
}

// ===========================================================================
// App-state markers + resources (C8: composition-level, NOT a widget primitive)
// ===========================================================================

// The markers authored as bare `bsn!` idents (or via `template_value`) derive
// `Clone + Default` — the trait surface `bsn!` requires of an authorable
// component (mirrors `buiy_widgets`' `CheckboxMark`).

/// Tag on the `#TodoList` container — where rows are appended and walked.
#[derive(Component, Clone, Default)]
pub struct TodoList;

/// Tag on the add-field `TextInput` ("What needs to be done?").
#[derive(Component, Clone, Default)]
pub struct AddField;

/// One todo row (the checkbox/destroy/label children are found by a `Children`
/// walk + marker query — C8 §3.4 drops the denormalized child cache).
#[derive(Component, Clone, Default)]
pub struct TodoRow;

/// Marks the checkbox inside a row.
#[derive(Component, Clone, Default)]
pub struct RowCheckbox;

/// Marks the destroy button inside a row.
#[derive(Component, Clone, Default)]
pub struct RowDestroy;

/// Marks the visible label child inside a row (the double-click edit target).
#[derive(Component, Clone, Default)]
pub struct RowLabel;

/// Marks a row's round checkbox **box** (the 20×20 radius-99 paint surface, child
/// of the `RowCheckbox` widget). [`restyle_completed`] swaps its fill/border on
/// toggle (the design's `boxStyle`).
#[derive(Component, Clone, Default)]
pub struct RowCheckBoxMarker;

/// Marks the check-mark [`Icon`] inside a row's round checkbox box. The design's
/// `checkStyle` keeps the box but toggles the glyph's `opacity` (`done ? 1 : 0`);
/// [`restyle_completed`] drives this child's [`Opacity`] from the row's toggle
/// state, so the check appears/disappears without the box changing.
#[derive(Component, Clone, Default)]
pub struct RowCheck;

/// The "N items left" Status live-region root (its `A11yLabel` is the announced
/// utterance; the child `Text` carries the visible pixels — the footer count).
#[derive(Component, Clone, Default)]
pub struct ItemsLeft;

/// The visible `Text` inside the [`ItemsLeft`] region (the footer "N items").
#[derive(Component, Clone, Default)]
pub struct ItemsLeftText;

/// The header "remaining" badge `Text` ("N left" / "all clear", mono pill). Driven
/// by [`update_count`] from the same `remaining` recount as the footer count.
#[derive(Component, Clone, Default)]
pub struct RemainingBadge;

/// The toggle-all chevron button (marks every row done, or all undone if already
/// all done — the design's `toggleAll`). Carries the [`Button`] OnPress sink.
#[derive(Component, Clone, Default)]
pub struct ToggleAllButton;

/// The chevron [`Icon`] inside the toggle-all button. Its tint is `accent` when
/// every (non-empty) row is done, else `text.dim` — [`update_count`] drives it.
#[derive(Component, Clone, Default)]
pub struct ToggleAllChevron;

/// One filter button (All / Active / Completed).
#[derive(Component, Clone, Copy, Default)]
pub struct FilterButton(pub FilterMode);

/// The clear-completed button. [`update_count`] tints its label `text.muted` when
/// any row is done (enabled) else `text.dimmer` (the design's `clearStyle`).
#[derive(Component, Clone, Default)]
pub struct ClearCompleted;

/// The "Clear done" button's label `Text` (the tint-toggled child).
#[derive(Component, Clone, Default)]
pub struct ClearCompletedLabel;

/// The empty-state label `Text` shown when the active filter matches no rows.
/// [`apply_filter`] toggles its visibility + sets the per-filter message.
#[derive(Component, Clone, Default)]
pub struct EmptyLabel;

/// Marks an in-place editor; carries the row whose label it edits.
#[derive(Component, Clone, Copy)]
pub struct EditingInPlace {
    /// The row whose label is being edited.
    pub row: Entity,
}

/// Which rows the filter shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilterMode {
    #[default]
    All,
    Active,
    Completed,
}

/// The active filter (the single source of truth `apply_filter` reads).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filter(pub FilterMode);

impl Default for Filter {
    fn default() -> Self {
        Self(FilterMode::All)
    }
}

/// Staged interaction intents, read once per frame by the `collect_*` systems
/// and consumed by [`apply_intents`]. Empty between frames.
#[derive(Resource, Default)]
pub struct TodoIntents {
    /// A submitted add-field value (the trimmed text), if Enter fired this frame.
    pub add: Option<String>,
    /// Rows whose destroy button fired.
    pub destroy_rows: Vec<Entity>,
    /// Whether clear-completed fired.
    pub clear_completed: bool,
    /// Whether the toggle-all chevron fired (mark all done / all undone).
    pub toggle_all: bool,
    /// Labels requesting edit-in-place (double-clicked this frame).
    pub begin_edit: Vec<Entity>,
    /// In-place editors that were submitted (commit the edit).
    pub commit_edit: Vec<Entity>,
}

/// The demo rows the binary + fixture seed: `(label, completed)`.
pub const DEMO_SEEDS: &[(&str, bool)] = &[
    ("Taste BSN authoring", true),
    ("Compose the P1d widgets", false),
    ("Inspect through the a11y driver", false),
];

// ===========================================================================
// The screen tree (the static frame — the "example IS the fixture")
//
// The TodoMVC screen is an icon-heavy, exact-parity restyle (the design's
// bordered card, round SVG-checked checkboxes, accent filter pills, the toggle-
// all chevron, the `↵` kbd chip). Icon leaves are not authored in `bsn!` in this
// codebase — the shell / composites / modal / overlay screens all build their
// one-off styled, icon-bearing boxes imperatively over `&mut World`. The todo
// screen follows that idiom: [`spawn_todomvc_screen`] builds the empty frame and
// returns the `#TodoScreen` root, and the dynamic rows are appended by
// [`append_row`] (icon-bearing too), the way `spawn_modal` / `spawn_overlay_menu`
// build their screens. Every value (px / color / radius / font / letter-spacing)
// comes from `docs/specs/2026-06-25-widget-catalog-values.md` (the § 4 typography
// rows, § 2 shadows, § 3 radii, § 6 icons, § 7.2 Todo layout).
// ===========================================================================

/// The Geist sans font stack (the sans generic still resolves to Fira — Wave A
/// note — so author Geist by name, like the shell / composites do).
fn geist() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist".into())]))
}

/// The Geist Mono font stack (the design's monospace UI face).
fn geist_mono() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist Mono".into())]))
}

/// A solid 1px `BorderSide` of a [`ColorToken`] color.
fn solid_side(token: ColorToken) -> BorderSide {
    BorderSide {
        color: token,
        style: LineStyle::Solid,
    }
}

/// A uniform 1px `Border` of `token` with `radius` rounded corners.
fn border_all(token: ColorToken, radius: f32) -> Border {
    Border {
        top: solid_side(token),
        right: solid_side(token),
        bottom: solid_side(token),
        left: solid_side(token),
        radius: Corners::all(Radius::circular(radius)),
    }
}

/// A leaf text node: `Text` + font (`family`/`size`/`weight`/`color`), an optional
/// `LetterSpacing`, and `Pickable::IGNORE` (decorative pixels — clicks fall
/// through to the owning control/row). The family/size/weight/color/ls are the
/// values.md § 4 typography row for that label.
#[allow(clippy::too_many_arguments)]
fn text_leaf(
    world: &mut World,
    name: &str,
    s: &str,
    family: FontFamily,
    size: f32,
    weight: u16,
    color: ColorToken,
    letter_spacing: Option<f32>,
) -> Entity {
    let mut e = world.spawn((
        Node,
        Name::new(name.to_string()),
        Text(s.to_string()),
        FontSize(size),
        family,
        FontWeight(weight),
        TextColor(color),
        Pickable::IGNORE,
    ));
    if let Some(ls) = letter_spacing {
        e.insert(LetterSpacing(ls));
    }
    e.id()
}

/// A vector-icon box: a `size`×`size` node carrying an [`Icon`] (the SVG path
/// stroked to a coverage glyph, tinted `color`), `Pickable::IGNORE` so it doesn't
/// eat the owning control's clicks. `extra` lets the caller layer a marker/Opacity.
fn icon_box(
    world: &mut World,
    name: &str,
    path_d: &str,
    stroke_width: f32,
    size_px: u16,
    color: ColorToken,
) -> Entity {
    world
        .spawn((
            Node,
            Name::new(name.to_string()),
            Style::default()
                .width_px(size_px as f32)
                .height_px(size_px as f32),
            Icon {
                path_d: path_d.to_string(),
                stroke_width,
                size_px,
                fill: false,
                color,
            },
            Pickable::IGNORE,
        ))
        .id()
}

/// Build the empty S1 TodoMVC screen into `world` and return its `#TodoScreen`
/// root. The frame mirrors the design's todo screen (values.md § 7.2 Todo):
/// a centered `max-width:560` wrap holding a **heading row** ("todos" + the "N
/// left" badge), the bordered **card** (header draft row + the todo list + the
/// empty-state label + the footer count/filters/clear), and a **caption**. The
/// dynamic rows are appended by [`append_row`]; both the binary and the fixture
/// build this same frame (the "example IS the fixture" discipline).
pub fn spawn_todomvc_screen(world: &mut World) -> Entity {
    let heading = build_todo_heading(world);
    let card = build_todo_card(world);
    let caption = text_leaf(
        world,
        "#TodoCaption",
        "double-click semantics · keyboard add · live count",
        geist_mono(),
        11.0,
        400,
        ColorToken::TextDimmer,
        None,
    );
    world.entity_mut(caption).insert(BoxModel {
        margin: Edges {
            top: Length::px(14.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // The centered `max-width:560` wrap (the design's `margin:0 auto`): a column
    // of width 560 centered by `align_self:center` in the flex-column
    // `#ScreenContent` slot, padded `48px 24px 64px` (values.md § 7.2 Outer wrap).
    let wrap = world
        .spawn((
            Node,
            Name::new("#TodoWrap"),
            Style::default()
                .flex_column()
                .width_px(560.0)
                .padding_edges(Edges {
                    top: Length::px(48.0),
                    right: Length::px(24.0),
                    bottom: Length::px(64.0),
                    left: Length::px(24.0),
                }),
            FlexItem {
                align_self: Some(AlignItems::Center),
                ..Default::default()
            },
        ))
        .id();
    world
        .entity_mut(wrap)
        .add_children(&[heading, card, caption]);

    // The `#TodoScreen` root the router toggles: a flex-column carrying the wrap,
    // tagged so the shell's `spawn_todo_screen` can re-home it under
    // `#ScreenContent`. (The wrap is a child so the wrap's own padding + centering
    // resolve against the full-width screen root.)
    let root = world
        .spawn((
            Node,
            Name::new("#TodoScreen"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0))),
        ))
        .id();
    world.entity_mut(root).add_child(wrap);
    root
}

/// The heading row (values.md § 7.2 "Header row"): `align-items:flex-end;
/// space-between; margin-bottom:18px`. The "todos" H1 (Geist 30 / 600 / -.75px LS,
/// `text.primary`) + the "N left" remaining badge (Geist Mono 12 / 500
/// `text.muted`, 1px `border.default`, radius 99, pad `4px 9px`, bg
/// `surface.inset`).
fn build_todo_heading(world: &mut World) -> Entity {
    // The "todos" H1 with the design's `-.025em` tracking (= -0.75px @ 30px).
    // `LetterSpacing` is logical px and lowers correctly (px / font_size → em
    // for cosmic-text); the earlier C3 ×30 over-application was the framework
    // units bug, now fixed in `buiy_core::text::sync` (AuthoredStyle::spaced).
    let h1 = text_leaf(
        world,
        "#TodoH1",
        "todos",
        geist(),
        30.0,
        600,
        ColorToken::TextPrimary,
        Some(-0.75),
    );

    // The remaining badge: a mono pill whose text [`update_count`] drives to
    // "N left" / "all clear" (the design's `remainingLabel`). Seeded "all clear"
    // (0 rows at build); the first `update_count` reconciles it to the seeds.
    let badge_text = text_leaf(
        world,
        "#TodoRemaining",
        "all clear",
        geist_mono(),
        12.0,
        500,
        ColorToken::TextMuted,
        None,
    );
    world.entity_mut(badge_text).insert(RemainingBadge);
    let badge = world
        .spawn((
            Node,
            Name::new("#TodoRemainingBadge"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .padding_edges(Edges::axis(9.0, 4.0))
                .border(1.0),
            Background {
                color: ColorToken::SurfaceInset,
            },
            border_all(ColorToken::BorderDefault, 99.0),
        ))
        .add_child(badge_text)
        .id();

    let heading = world
        .spawn((
            Node,
            Name::new("#TodoHeading"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexEnd)
                .justify_content(JustifyContent::SpaceBetween)
                .margin_edges(Edges {
                    bottom: Length::px(18.0),
                    ..Default::default()
                }),
        ))
        .id();
    world.entity_mut(heading).add_children(&[h1, badge]);
    heading
}

/// The bordered card (values.md § 7.2 "Card"): 1px `border.strong`, radius 12, bg
/// `surface.card`, `overflow:hidden`, `shadow.card`. A flex-column of the header
/// draft row, the `#TodoList`, the empty-state label, and the footer.
fn build_todo_card(world: &mut World) -> Entity {
    let header = build_todo_header(world);
    let list = world
        .spawn((
            Node,
            TodoList,
            Name::new("#TodoList"),
            Style::default().flex_column(),
        ))
        .id();
    let empty = build_todo_empty(world);
    let footer = build_todo_footer(world);

    let card = world
        .spawn((
            Node,
            Name::new("#TodoCard"),
            Style::default().flex_column().overflow_hidden().border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderStrong, 12.0),
            // shadow.card — `0 12px 32px -16px rgba(0,0,0,.7)` (values.md § 2).
            BoxShadow(vec![Shadow {
                color: ColorToken::ShadowCard,
                offset_x: Length::px(0.0),
                offset_y: Length::px(12.0),
                blur: Length::px(32.0),
                spread: Length::px(-16.0),
                inset: false,
            }]),
        ))
        .id();
    world
        .entity_mut(card)
        .add_children(&[header, list, empty, footer]);
    card
}

/// The card header / draft row (values.md § 7.2 "Draft row"): `gap:12px;
/// padding:14px 16px`; border-bottom 1px `border.subtle`. A toggle-all chevron
/// button + the `What needs doing?` draft input (`AddField`, Geist 450 15px,
/// transparent, no border, `flex:1`) + the `↵` kbd chip.
fn build_todo_header(world: &mut World) -> Entity {
    // The toggle-all chevron button (22×22, `flex:none`): a bare `Button` (the
    // OnPress sink + a11y) holding the down-chevron Icon. Tint defaults `text.dim`
    // (empty list); [`update_count`] flips it to `accent` when all rows are done.
    let chevron = icon_box(
        world,
        "#ToggleAllChevron",
        "M6 9l6 6 6-6",
        2.0,
        16,
        ColorToken::TextDim,
    );
    world.entity_mut(chevron).insert(ToggleAllChevron);
    let toggle_all = world
        .spawn((
            buiy::prelude::Button,
            A11yLabel("Toggle all".to_string()),
            ToggleAllButton,
            Name::new("#ToggleAll"),
            Style::default()
                .width_px(22.0)
                .height_px(22.0)
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center),
        ))
        .add_child(chevron)
        .id();

    // The draft input: a real single-line field (focusable + editable) styled to
    // the design's transparent, borderless `flex:1` input (Geist 450 15px). The
    // `text_input_single_line` scene-fn supplies the editor + placeholder; the
    // overrides strip the default chrome and grow it.
    let field = world
        .spawn_scene(text_input_single_line("What needs doing?"))
        .expect("spawn the todo draft field")
        .id();
    world.entity_mut(field).insert((
        AddField,
        Name::new("#AddField"),
        FontSize(15.0),
        geist(),
        FontWeight(450),
        TextColor(ColorToken::TextPrimary),
        // Transparent bg + no border + no rounding + grow to fill the row.
        Background {
            color: ColorToken::Transparent,
        },
        Border::default(),
        // Strip the widget's default 200×32 + 8px padding chrome: the design's draft
        // input is borderless + content-height; the row supplies the padding. Auto
        // height lets it sit on the row's text baseline (the row is `14px` tall
        // padded); `flex:1` grows the width across the row.
        BoxModel {
            width: Sizing::Auto,
            height: Sizing::Auto,
            ..Default::default()
        },
        FlexItem {
            grow: 1.0,
            ..Default::default()
        },
    ));

    // The `↵` kbd chip (Geist Mono 10 / 500 `text.dim`, 1px `border.default`,
    // radius 5, pad `3px 6px`, bg `surface.inset` — values.md § 7.2 "Todo `↵` kbd").
    let kbd_text = text_leaf(
        world,
        "#TodoKbdGlyph",
        "↵",
        geist_mono(),
        10.0,
        500,
        ColorToken::TextDim,
        None,
    );
    let kbd = world
        .spawn((
            Node,
            Name::new("#TodoKbd"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .padding_edges(Edges::axis(6.0, 3.0))
                .border(1.0),
            Background {
                color: ColorToken::SurfaceInset,
            },
            border_all(ColorToken::BorderDefault, 5.0),
        ))
        .add_child(kbd_text)
        .id();

    let header = world
        .spawn((
            Node,
            Name::new("#TodoDraftRow"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(12.0)
                .padding_edges(Edges::axis(16.0, 14.0))
                .border_edges(Edges {
                    bottom: Length::px(1.0),
                    ..Default::default()
                }),
            Border {
                bottom: solid_side(ColorToken::BorderSubtle),
                ..Default::default()
            },
        ))
        .id();
    world
        .entity_mut(header)
        .add_children(&[toggle_all, field, kbd]);
    header
}

/// The empty-state label (values.md § 7.2 "Empty label"): `padding:36px 16px;
/// text-align:center`, Geist 450 13px `text.dim`. Hidden at rest (seeded rows
/// exist); [`apply_filter`] shows it + sets the per-filter message when the active
/// filter matches no rows.
fn build_todo_empty(world: &mut World) -> Entity {
    let label = text_leaf(
        world,
        "#TodoEmpty",
        EMPTY_ALL,
        geist(),
        13.0,
        450,
        ColorToken::TextDim,
        None,
    );
    world.entity_mut(label).insert((
        EmptyLabel,
        TextAlign::Center,
        CssVisibility::Hidden,
        // Starts `Display::None` (seeded rows exist, so nothing is empty at boot);
        // `apply_filter` flips it to `flex_row` + visible when no row matches.
        Style::default()
            .display(Display::None)
            .justify_content(JustifyContent::Center)
            .padding_edges(Edges::axis(16.0, 36.0)),
    ));
    label
}

/// The footer strip (values.md § 7.2 "Footer strip"): `gap:12px; padding:11px
/// 14px`; border-top 1px `border.subtle`; bg `surface.inset`; space-between. The
/// "N items" count (Status live region) + the filter pills (All/Active/Done) + the
/// "Clear done" button.
fn build_todo_footer(world: &mut World) -> Entity {
    // The "N items" footer count, in an `A11yRole::Status` live region (the
    // announced utterance is the region's `A11yLabel`; the child `Text` is the
    // visible pixels). Geist Mono 11.5 / 500 `text.muted`.
    let count_text = text_leaf(
        world,
        "#ItemsLeftCount",
        &items_left_text(0),
        geist_mono(),
        11.5,
        500,
        ColorToken::TextMuted,
        None,
    );
    world.entity_mut(count_text).insert(ItemsLeftText);
    let count = world
        .spawn((
            Node,
            ItemsLeft,
            A11yRole::Status,
            A11yLabel(items_left_utterance(0)),
            Name::new("#ItemsLeft"),
            Style::default().flex_row().align_items(AlignItems::Center),
        ))
        .add_child(count_text)
        .id();

    // The filter pills (gap 3): All / Active / Done. Each is a bare `Button`
    // (OnPress + a11y, no auto-label) carrying a Geist 11.5/500 label; All is the
    // active pill at boot (accent bg + on-accent label), the rest transparent +
    // muted. [`apply_filter`] restyles them on a filter change.
    let pills: Vec<Entity> = [
        (FilterMode::All, "All", "#FilterAll"),
        (FilterMode::Active, "Active", "#FilterActive"),
        (FilterMode::Completed, "Done", "#FilterCompleted"),
    ]
    .iter()
    .map(|&(mode, label, name)| build_filter_pill(world, mode, label, name))
    .collect();
    let filters = world
        .spawn((
            Node,
            Name::new("#TodoFilters"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(3.0),
        ))
        .id();
    world.entity_mut(filters).add_children(&pills);

    // The "Clear done" button: a bare `Button` carrying a Geist 11.5/500 label.
    // The label tint [`update_count`] drives to `text.muted` (enabled, any done)
    // or `text.dimmer` (disabled). Pad `5px 4px` (the design's `clearStyle`).
    let clear_text = text_leaf(
        world,
        "#ClearDoneLabel",
        "Clear done",
        geist(),
        11.5,
        500,
        ColorToken::TextDimmer,
        None,
    );
    world.entity_mut(clear_text).insert(ClearCompletedLabel);
    let clear = world
        .spawn((
            buiy::prelude::Button,
            A11yLabel("Clear done".to_string()),
            ClearCompleted,
            Name::new("#ClearButton"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .padding_edges(Edges::axis(4.0, 5.0)),
        ))
        .add_child(clear_text)
        .id();

    let footer = world
        .spawn((
            Node,
            Name::new("#TodoFooter"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::SpaceBetween)
                .gap_px(12.0)
                .padding_edges(Edges::axis(14.0, 11.0))
                .border_edges(Edges {
                    top: Length::px(1.0),
                    ..Default::default()
                }),
            Background {
                color: ColorToken::SurfaceInset,
            },
            Border {
                top: solid_side(ColorToken::BorderSubtle),
                ..Default::default()
            },
        ))
        .id();
    world
        .entity_mut(footer)
        .add_children(&[count, filters, clear]);
    footer
}

/// One filter pill (values.md § 7.2 "Filter button"): a bare `Button` (no auto-
/// label, the § 4.1c double-label gotcha) carrying a Geist 11.5/500 label, `pad:5px
/// 11px`, radius 6. `active` = accent bg + `text.on-accent` label; else transparent
/// + `text.muted` (the design's filter `style`). Carries [`FilterButton`].
fn build_filter_pill(world: &mut World, mode: FilterMode, label: &str, name: &str) -> Entity {
    let active = mode == FilterMode::default();
    let (bg, fg) = filter_pill_colors(active);
    let text = text_leaf(world, "#FilterLabel", label, geist(), 11.5, 500, fg, None);
    world
        .spawn((
            buiy::prelude::Button,
            A11yLabel(label.to_string()),
            FilterButton(mode),
            Name::new(name.to_string()),
            Style::default()
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .padding_edges(Edges::axis(11.0, 5.0)),
            Background { color: bg },
            Border {
                radius: Corners::all(Radius::circular(6.0)),
                ..Default::default()
            },
        ))
        .add_child(text)
        .id()
}

/// The `(bg, fg)` token pair for a filter pill in `active` state (design `style`):
/// active = accent bg + on-accent label; inactive = transparent + muted.
fn filter_pill_colors(active: bool) -> (ColorToken, ColorToken) {
    if active {
        (ColorToken::Accent, ColorToken::TextOnAccent)
    } else {
        (ColorToken::Transparent, ColorToken::TextMuted)
    }
}

/// The footer "N items" string (the design's `itemsLeftLabel` — `N item`/`N items`,
/// no trailing "left"; the live-region utterance keeps "N items left" for AT).
pub fn items_left_text(n: usize) -> String {
    if n == 1 {
        "1 item".to_string()
    } else {
        format!("{n} items")
    }
}

/// The `A11yRole::Status` live-region utterance (the announced phrase, distinct
/// from the terse visible footer count [`items_left_text`]): "N item(s) left". The
/// design's visible footer reads "N items"; the live region keeps the clearer
/// "N items left" for AT. `pub` so the inspection-driver acceptance asserts the
/// announced phrase through the a11y tree.
pub fn items_left_utterance(n: usize) -> String {
    if n == 1 {
        "1 item left".to_string()
    } else {
        format!("{n} items left")
    }
}

/// The empty-state message for each filter (the design's `emptyLabel`).
const EMPTY_ALL: &str = "Add your first todo above.";
const EMPTY_ACTIVE: &str = "No active items — nice.";
const EMPTY_COMPLETED: &str = "Nothing completed yet.";

/// The empty message for the active filter.
fn empty_label_for(filter: FilterMode) -> &'static str {
    match filter {
        FilterMode::All => EMPTY_ALL,
        FilterMode::Active => EMPTY_ACTIVE,
        FilterMode::Completed => EMPTY_COMPLETED,
    }
}

// ===========================================================================
// Setup (the legacy standalone-S1 startup; the shipped binary boots the shell)
// ===========================================================================

/// Startup system (legacy standalone S1 — the shipped binary boots the unified
/// shell): spawn a camera, the screen frame, then the [`DEMO_SEEDS`] rows.
pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        spawn_todomvc_screen(world);
        for &(label, completed) in DEMO_SEEDS {
            append_row(world, label, completed);
        }
    });
}

// ===========================================================================
// The TodoMvcPlugin — the retained-mode app logic (C8 §3.1)
// ===========================================================================

/// The TodoMVC app logic. Registers the intent collectors + the exclusive
/// applier + the pure change-detection systems, all `.after(BuiySet::Input)`
/// (C8 §2.5(1)), plus the [`MultiClick`] double-click observer.
pub struct TodoMvcPlugin;

impl Plugin for TodoMvcPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Filter>()
            .init_resource::<TodoIntents>()
            .add_observer(on_label_double_click)
            // PHASE 1 — collect intents + apply them (spawn/despawn rows, clear the input
            // editor on submit). Kept `.after(Input).before(A11yUpdate)`: the activation/submit
            // Messages are emitted in `BuiySet::Input` (C8 §2.5(1)), and `apply_intents` mutates
            // the input *editor* — which MUST settle before the layout `TextCommit` reshape, so
            // this phase stays in the early window (moving it past the toggle drain would leave
            // a just-cleared editor dirty-unshaped — the `commit.rs` coherence invariant).
            .add_systems(
                Update,
                (
                    collect_add_submit,
                    collect_button_press,
                    collect_edit_submit,
                    apply_intents,
                )
                    .chain()
                    .after(BuiySet::Input)
                    .before(BuiySet::A11yUpdate),
            )
            // PHASE 2 — recount from `A11yToggled` (count, filter, completed-restyle). With the
            // MVU toggle-leaf migration a checkbox press no longer flips `A11yToggled` in
            // `BuiySet::Input`; it ENQUEUES a `ToggleMsg` whose fold lands in the early
            // `ToggleLeafSet::Drain` (`.after(Picking).before(A11yUpdate)`). So the recounting
            // systems MUST run `.after(ToggleLeafSet::Drain)` (else "N items left" lags a frame)
            // AND `.after(apply_intents)` (to see rows added/removed this frame), still
            // `.before(A11yUpdate)` so the driver's very-next a11y snapshot reflects the new
            // `A11yLabel`/`A11yHidden`. (None of these three touch the input editor, so running
            // them late is coherence-safe.)
            .add_systems(
                Update,
                (apply_filter, update_count, restyle_completed)
                    .chain()
                    .after(apply_intents)
                    .after(ToggleLeafSet::Drain)
                    .before(BuiySet::A11yUpdate),
            );
    }
}

/// Append a new row to `#TodoList`, seeding its checkbox state. Returns the row.
/// Shared by [`setup`] and the runtime add-flow.
///
/// The row is the design's `[round checkbox · label · × delete]` (values.md § 7.2
/// "Todo row" + "Checkbox" + "Delete button"): `gap:12px; padding:12px 14px 12px
/// 16px`, border-bottom 1px `border.subtle`. The checkbox is a `Checkbox` widget
/// (the toggle a11y + OnPress) restyled to the design's **round** box — a 20×20
/// radius-99 box whose fill/border swap on toggle (`accent` + transparent border
/// when done; transparent + 1.5px `border.muted` when active) holding a check
/// `Icon` (path `M4 12.5 9 17.5 20 6.5`, `text.on-accent`) whose `opacity` is 1
/// when done else 0. NO `CheckboxMark` child → the widget's `✓` font-glyph mark
/// (the C1 caret/tofu artifact) never renders. The label is the `flex:1` todo text
/// (`RowLabel`, the double-click edit target); the × delete is a bare `Button`.
pub fn append_row(world: &mut World, label: &str, completed: bool) -> Entity {
    let Some(list) = find_single::<TodoList>(world) else {
        return Entity::PLACEHOLDER;
    };

    // The round checkbox box (20×20, radius 99). Authored at the resting (active /
    // unchecked) paint; `restyle_completed` swaps it to the done paint when the row
    // is completed. It carries NO `CheckboxMark` — so `update_checkbox_visual` is
    // inert for it (no font-glyph mark) — only the SVG check `Icon` child.
    let check = icon_box(
        world,
        "#RowCheckIcon",
        "M4 12.5 9 17.5 20 6.5",
        2.4,
        13,
        ColorToken::TextOnAccent,
    );
    world.entity_mut(check).insert((RowCheck, Opacity(0.0)));
    let mark = world
        .spawn((
            Node,
            RowCheckBoxMarker,
            Name::new("#RowCheckBox"),
            Style::default()
                .width_px(20.0)
                .height_px(20.0)
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .border(1.5),
            Background {
                color: ColorToken::Transparent,
            },
            Border {
                top: solid_side(ColorToken::BorderMuted),
                right: solid_side(ColorToken::BorderMuted),
                bottom: solid_side(ColorToken::BorderMuted),
                left: solid_side(ColorToken::BorderMuted),
                radius: Corners::all(Radius::circular(99.0)),
            },
            Pickable::IGNORE,
        ))
        .add_child(check)
        .id();

    // The visible label (Geist 450 14.5px; active = `text.bright`). `flex:1` so the
    // × delete button is pushed to the row's right edge. The double-click edit
    // target (`RowLabel`).
    let label_leaf = text_leaf(
        world,
        "#RowLabel",
        label,
        geist(),
        14.5,
        450,
        ColorToken::TextBright,
        None,
    );
    world.entity_mut(label_leaf).insert((
        RowLabel,
        // Carried (empty) so `restyle_completed` can flip the line-through on it.
        TextDecorations::default(),
        FlexItem {
            grow: 1.0,
            ..Default::default()
        },
    ));

    // The `Checkbox` widget root (the toggle a11y + OnPress). Authored bare (not the
    // `checkbox()` scene-fn) so its children are OUR round box + label, not the
    // widget's default square + `CheckboxMark`. The `#[require]` flex-row lays
    // `[box, label]` with the widget's own gap; override it to the design's 12.
    let checkbox = world
        .spawn((
            Checkbox,
            RowCheckbox,
            A11yLabel(label.to_string()),
            Name::new("#RowCheckbox"),
            FlexParams {
                direction: FlexAxis::Row,
                align_items: AlignItems::Center,
                gap: FlexGap {
                    row: Length::px(12.0),
                    column: Length::px(12.0),
                },
                ..Default::default()
            },
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
        ))
        .id();
    world.entity_mut(checkbox).add_children(&[mark, label_leaf]);

    // The × delete button (26×26, radius 6): a bare `Button` holding the close Icon
    // (`M6 6l12 12M18 6 6 18`, stroke 1.7, `text.dim`).
    let close = icon_box(
        world,
        "#RowDeleteIcon",
        "M6 6l12 12M18 6 6 18",
        1.7,
        14,
        ColorToken::TextDim,
    );
    let destroy = world
        .spawn((
            buiy::prelude::Button,
            A11yLabel("Delete".to_string()),
            RowDestroy,
            Name::new("#RowDestroy"),
            Style::default()
                .width_px(26.0)
                .height_px(26.0)
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center),
            Border {
                radius: Corners::all(Radius::circular(6.0)),
                ..Default::default()
            },
        ))
        .add_child(close)
        .id();

    // The row container (values.md § 7.2 "Todo row"): `gap:12px; padding:12px 14px
    // 12px 16px`, border-bottom 1px `border.subtle`.
    let row = world
        .spawn((
            Node,
            TodoRow,
            Name::new("#TodoRow"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(12.0)
                .padding_edges(Edges {
                    top: Length::px(12.0),
                    right: Length::px(14.0),
                    bottom: Length::px(12.0),
                    left: Length::px(16.0),
                })
                .border_edges(Edges {
                    bottom: Length::px(1.0),
                    ..Default::default()
                }),
            Border {
                bottom: solid_side(ColorToken::BorderSubtle),
                ..Default::default()
            },
        ))
        .add_children(&[checkbox, destroy])
        .id();
    world.entity_mut(list).add_child(row);

    // SEED (authored initial state, NOT a runtime writer): a row spawned `completed`
    // starts checked. This is a seed-scene initial condition set at spawn time, before
    // the toggle leaf's drain ever runs for this entity, so it stays a direct write —
    // the D3/D10 single-writer rule governs RUNTIME mutations of `A11yToggled`.
    if completed && let Some(mut t) = world.get_mut::<A11yToggled>(checkbox) {
        t.0 = Toggled::True;
    }
    row
}

// --- Intent collectors (ordinary MessageReader systems) --------------------

/// Stage the add-field submission (single-line Enter → `EditSubmitted`).
pub fn collect_add_submit(
    mut reader: MessageReader<EditSubmitted>,
    fields: Query<&TextEditState, With<AddField>>,
    mut intents: ResMut<TodoIntents>,
) {
    for EditSubmitted(e) in reader.read() {
        if let Ok(state) = fields.get(*e) {
            let text = state.value().trim().to_string();
            if !text.is_empty() {
                intents.add = Some(text);
            }
        }
    }
}

/// Stage destroy / clear-completed / toggle-all / filter presses. `kinds` reads the
/// pressed entity's button role in one query; `hierarchy` walks the row ancestry
/// for a destroy.
#[allow(clippy::type_complexity)]
pub fn collect_button_press(
    mut reader: MessageReader<OnPress>,
    kinds: Query<(
        Has<RowDestroy>,
        Has<ClearCompleted>,
        Has<ToggleAllButton>,
        Option<&FilterButton>,
    )>,
    hierarchy: Query<(Has<TodoRow>, Option<&ChildOf>)>,
    mut intents: ResMut<TodoIntents>,
    mut filter: ResMut<Filter>,
) {
    for OnPress(e) in reader.read() {
        let Ok((is_destroy, is_clear, is_toggle_all, fb)) = kinds.get(*e) else {
            continue;
        };
        if is_destroy {
            if let Some(row) = ancestor_row(*e, &hierarchy) {
                intents.destroy_rows.push(row);
            }
        } else if is_clear {
            intents.clear_completed = true;
        } else if is_toggle_all {
            intents.toggle_all = true;
        } else if let Some(fb) = fb {
            filter.0 = fb.0;
        }
    }
}

/// Stage in-place editor submissions (commit the edit).
pub fn collect_edit_submit(
    mut reader: MessageReader<EditSubmitted>,
    editors: Query<(), With<EditingInPlace>>,
    mut intents: ResMut<TodoIntents>,
) {
    for EditSubmitted(e) in reader.read() {
        if editors.get(*e).is_ok() {
            intents.commit_edit.push(*e);
        }
    }
}

/// Observer: a double-click (`MultiClick.count >= 2`) on a row label stages an
/// in-place edit. `MultiClick` auto-propagates, so a click on the label's own
/// pixels (or a descendant) bubbles to the `RowLabel` entity.
pub fn on_label_double_click(
    ev: On<MultiClick>,
    labels: Query<(), With<RowLabel>>,
    mut intents: ResMut<TodoIntents>,
) {
    if ev.count >= 2 && labels.get(ev.entity).is_ok() {
        intents.begin_edit.push(ev.entity);
    }
}

// --- The exclusive applier (structural mutations over &mut World) ----------

/// Consume the staged [`TodoIntents`]: append the new todo, despawn destroyed /
/// completed rows (restoring focus), begin/commit in-place edits. Clears the
/// staging at the end so each intent fires exactly once.
pub fn apply_intents(world: &mut World) {
    let intents = std::mem::take(&mut *world.resource_mut::<TodoIntents>());

    // 1. Add a todo + clear the add-field through the driver's text channel.
    if let Some(text) = intents.add {
        append_row(world, &text, false);
        if let Some(field) = find_single::<AddField>(world) {
            // W3 (MVU-as-core / H5): this `set_value` rides the a11y DRIVER text
            // channel, NOT `apply_keyboard_edits`/`apply_ime`, so the W3 record tap
            // does NOT capture it. It is seed-scene state — a derived consequence of
            // the Add intent — reproduced when replay rebuilds the UI from the seed
            // and RE-RUNS this app logic, never re-applied as a recorded EditCommand.
            let _ = set_value(world, node_id_for(field), "");
        }
    }

    // 2. Toggle-all: mark every row done, or all undone if already all done (the
    //    design's `toggleAll`). Runs before destroy/clear so a clear-after-toggle
    //    in the same frame sees the new state.
    if intents.toggle_all {
        toggle_all_rows(world);
    }

    // 3. Destroy rows (explicit ×) + clear-completed.
    let mut to_despawn = intents.destroy_rows;
    if intents.clear_completed {
        for row in completed_rows(world) {
            if !to_despawn.contains(&row) {
                to_despawn.push(row);
            }
        }
    }
    for row in to_despawn {
        despawn_row_restoring_focus(world, row);
    }

    // 4. Begin in-place edits: hide the static label, spawn + seed + focus an
    //    editor sibling tagged with the row.
    for label in intents.begin_edit {
        begin_one_edit(world, label);
    }

    // 5. Commit in-place edits: write the new text back to the label, restore
    //    its visibility, despawn the editor.
    for editor in intents.commit_edit {
        commit_one_edit(world, editor);
    }
}

/// Toggle every row's done state: if all rows are already done, mark them all
/// undone; otherwise mark them all done (the design's `toggleAll`).
fn toggle_all_rows(world: &mut World) {
    let checkboxes: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<RowCheckbox>>();
        q.iter(world).collect()
    };
    if checkboxes.is_empty() {
        return;
    }
    let all_done = checkboxes
        .iter()
        .all(|&cb| world.get::<A11yToggled>(cb).map(|t| t.0) == Some(Toggled::True));
    // D10 single-writer reroute: `toggleAll` is a RUNTIME mutator of `A11yToggled`,
    // so it must NOT write `t.0` directly (that races the toggle leaf's drain — the
    // multi-writer flicker W2 cures). Enqueue an absolute `ToggleMsg::Set(on)` per row;
    // the early `ToggleLeafSet::Drain` (the SOLE writer) folds it via `set_if_neq`. We
    // write the `Envelope` inbox directly because this is an exclusive `&mut World`
    // system (no `Commands`), mirroring the menu machine's world-level enqueue helper.
    let on = !all_done;
    if let Some(mut inbox) = world.get_resource_mut::<Messages<Envelope<A11yToggled>>>() {
        for cb in checkboxes {
            inbox.write(Envelope::user(cb, ToggleMsg::Set(on)));
        }
    }
}

fn begin_one_edit(world: &mut World, label: Entity) {
    let Some(text) = world.get::<Text>(label).map(|t| t.0.clone()) else {
        return;
    };
    let Some(row) = ancestor_row_world(world, label) else {
        return;
    };
    world.entity_mut(label).insert(CssVisibility::Hidden);
    let editor = world
        .spawn_scene(text_input_single_line(""))
        .expect("spawn edit-in-place editor")
        .id();
    world.entity_mut(editor).insert(EditingInPlace { row });
    world.entity_mut(row).add_child(editor);
    // W3 (MVU-as-core / H5): the freshly-spawned in-place editor's authored INITIAL
    // CONDITION (seeded with the row's existing text). Like the W2 toggle seeds, this
    // `set_value` is a seed-scene write on the a11y driver channel — NOT captured by
    // the W3 record tap (`apply_keyboard_edits`/`apply_ime`). Replay reproduces it by
    // rebuilding the editor from this seed, never by re-applying a recorded EditCommand.
    let _ = set_value(world, node_id_for(editor), &text);
    world.resource_mut::<FocusedEntity>().0 = Some(editor);
}

fn commit_one_edit(world: &mut World, editor: Entity) {
    let Some(EditingInPlace { row }) = world.get::<EditingInPlace>(editor).copied() else {
        return;
    };
    let new_text = world
        .get::<TextEditState>(editor)
        .map(|s| s.value())
        .unwrap_or_default();
    if let Some(checkbox) = child_with::<RowCheckbox>(world, row)
        && let Some(label) = checkbox_label_child(world, checkbox)
    {
        if let Some(mut t) = world.get_mut::<Text>(label) {
            t.0 = new_text.clone();
        }
        if let Some(mut name) = world.get_mut::<A11yLabel>(checkbox) {
            name.0 = new_text.clone();
        }
        world.entity_mut(label).insert(CssVisibility::Visible);
    }
    if world.resource::<FocusedEntity>().0 == Some(editor) {
        world.resource_mut::<FocusedEntity>().0 = None;
    }
    world.entity_mut(editor).despawn();
}

// --- Pure change-detection systems -----------------------------------------

/// Show only rows matching the active filter: add/remove `A11yHidden` (a11y
/// prune, C8 §3.4) + `Display::None` (collapse out of layout, so the card hugs the
/// visible rows). When NO row matches, reveal the empty-state label (with the
/// per-filter message) in the rows' place. One cheap walk/frame.
#[allow(clippy::type_complexity)]
pub fn apply_filter(
    filter: Res<Filter>,
    mut commands: Commands,
    rows: Query<(Entity, &Children), With<TodoRow>>,
    checkboxes: Query<&A11yToggled, With<RowCheckbox>>,
    mut empty: Query<(Entity, &mut Text), With<EmptyLabel>>,
) {
    let mut visible = 0usize;
    for (row, children) in &rows {
        let completed = row_completed(children, &checkboxes);
        let show = match filter.0 {
            FilterMode::All => true,
            FilterMode::Active => !completed,
            FilterMode::Completed => completed,
        };
        if show {
            visible += 1;
            commands
                .entity(row)
                .remove::<A11yHidden>()
                .insert(CssVisibility::Visible)
                .insert(Display::flex_row());
        } else {
            commands
                .entity(row)
                .insert(A11yHidden)
                .insert(Display::None);
        }
    }

    // The empty-state label: shown (with the per-filter message) iff nothing is
    // visible. `Display::None`/`flex` toggles it in/out of layout so it occupies
    // the rows' place only when needed.
    let message = empty_label_for(filter.0);
    for (entity, mut text) in &mut empty {
        if visible == 0 {
            if text.0 != message {
                text.0 = message.to_string();
            }
            commands
                .entity(entity)
                .remove::<A11yHidden>()
                .insert(CssVisibility::Visible)
                .insert(Display::flex_row());
        } else {
            commands
                .entity(entity)
                .insert(A11yHidden)
                .insert(Display::None);
        }
    }
}

/// Recount the rows and drive every count-derived chrome from `remaining` /
/// `done` / `total`: the footer "N items" count + its `ItemsLeft` Status region
/// utterance ("N items left"), the header "N left" / "all clear" badge, the
/// toggle-all chevron tint (`accent` when every non-empty row is done, else
/// `text.dim`), and the "Clear done" label tint (`text.muted` when any done, else
/// `text.dimmer`). The design recomputes all of these from the same recount.
#[allow(clippy::type_complexity)]
pub fn update_count(
    rows: Query<&Children, With<TodoRow>>,
    checkboxes: Query<&A11yToggled, With<RowCheckbox>>,
    mut region: Query<(&Children, &mut A11yLabel), With<ItemsLeft>>,
    mut texts: Query<&mut Text, With<ItemsLeftText>>,
    mut badges: Query<&mut Text, (With<RemainingBadge>, Without<ItemsLeftText>)>,
    mut chevrons: Query<&mut Icon, With<ToggleAllChevron>>,
    mut clear_labels: Query<&mut TextColor, With<ClearCompletedLabel>>,
) {
    let total = rows.iter().count();
    let remaining = rows
        .iter()
        .filter(|children| !row_completed(children, &checkboxes))
        .count();
    let done = total - remaining;

    // The footer visible count ("N items") + the live-region utterance.
    let count = items_left_text(remaining);
    let utterance = items_left_utterance(remaining);
    for (children, mut label) in &mut region {
        if label.0 != utterance {
            label.0 = utterance.clone();
        }
        for &child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != count
            {
                text.0 = count.clone();
            }
        }
    }

    // The header badge: "all clear" when nothing is left, else "N left".
    let badge = if remaining == 0 {
        "all clear".to_string()
    } else {
        format!("{remaining} left")
    };
    for mut text in &mut badges {
        if text.0 != badge {
            text.0 = badge.clone();
        }
    }

    // The toggle-all chevron tint: accent when every (non-empty) row is done.
    let all_done = total > 0 && remaining == 0;
    let chevron_tint = if all_done {
        ColorToken::Accent
    } else {
        ColorToken::TextDim
    };
    for mut icon in &mut chevrons {
        if icon.color != chevron_tint {
            icon.color = chevron_tint;
        }
    }

    // The "Clear done" label tint: muted (enabled) when any row is done.
    let clear_tint = if done > 0 {
        ColorToken::TextMuted
    } else {
        ColorToken::TextDimmer
    };
    for mut color in &mut clear_labels {
        if color.0 != clear_tint {
            color.0 = clear_tint;
        }
    }
}

/// Drive every row's completed visual (the design's `textStyle` + `boxStyle` +
/// `checkStyle`): the label tints `text.bright` (active) / `text.dim` (done) with a
/// `line-through` (decoration color `text.dimmer`) when done; the round box swaps
/// to `accent` fill + transparent border when done (else transparent fill + 1.5px
/// `border.muted`); and the check `Icon`'s opacity is 1 when done else 0.
#[allow(clippy::type_complexity)]
pub fn restyle_completed(
    rows: Query<&Children, With<TodoRow>>,
    checkboxes: Query<(&A11yToggled, &Children), With<RowCheckbox>>,
    mut labels: Query<&mut TextColor, With<RowLabel>>,
    mut decorations: Query<&mut TextDecorations, With<RowLabel>>,
    mut box_paint: Query<(&mut Background, &mut Border), With<RowCheckBoxMarker>>,
    mut checks: Query<&mut Opacity, With<RowCheck>>,
    box_children: Query<&Children>,
) {
    for row_children in &rows {
        for &child in row_children.iter() {
            let Ok((toggled, cb_children)) = checkboxes.get(child) else {
                continue;
            };
            let completed = toggled.0 == Toggled::True;
            for &cb_child in cb_children.iter() {
                // The label: tint + line-through.
                if let Ok(mut color) = labels.get_mut(cb_child) {
                    color.0 = if completed {
                        ColorToken::TextDim
                    } else {
                        ColorToken::TextBright
                    };
                }
                if let Ok(mut deco) = decorations.get_mut(cb_child) {
                    let want_line = if completed {
                        DecorationLines::LINE_THROUGH
                    } else {
                        DecorationLines::empty()
                    };
                    if deco.line != want_line {
                        deco.line = want_line;
                        deco.style = DecorationLineStyle::Solid;
                        deco.color = Some(ColorToken::TextDimmer);
                    }
                }
                // The round box: fill + border swap, and the check icon's opacity.
                if let Ok((mut bg, mut border)) = box_paint.get_mut(cb_child) {
                    restyle_check_box(&mut bg, &mut border, completed);
                    if let Ok(grandchildren) = box_children.get(cb_child) {
                        for &g in grandchildren.iter() {
                            if let Ok(mut opacity) = checks.get_mut(g) {
                                opacity.0 = if completed { 1.0 } else { 0.0 };
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Set a round checkbox box's fill + border for its `completed` state (the design's
/// `boxStyle`): done = `accent` fill + transparent border; active = transparent
/// fill + 1.5px `border.muted`.
fn restyle_check_box(bg: &mut Background, border: &mut Border, completed: bool) {
    let (fill, side) = if completed {
        (ColorToken::Accent, ColorToken::Transparent)
    } else {
        (ColorToken::Transparent, ColorToken::BorderMuted)
    };
    bg.color = fill;
    border.top = solid_side(side);
    border.right = solid_side(side);
    border.bottom = solid_side(side);
    border.left = solid_side(side);
}

// ===========================================================================
// Helpers (Children walks + marker queries — C8 §3.4 drops the denorm cache)
// ===========================================================================

/// The single entity carrying `T`, or `None` (0 or >1).
fn find_single<T: Component>(world: &mut World) -> Option<Entity> {
    let mut q = world.query_filtered::<Entity, With<T>>();
    let mut it = q.iter(world);
    let first = it.next()?;
    if it.next().is_some() {
        return None;
    }
    Some(first)
}

/// The first descendant of `root` (BFS) carrying a [`TextEditState`] — the
/// focusable editor field inside the `search_input` composite (so the scroll
/// heading can tag it with [`ScrollSearch`]).
fn descendant_with_edit_state(world: &World, root: Entity) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if world.get::<TextEditState>(e).is_some() {
            return Some(e);
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    None
}

/// The first child of `parent` carrying `T`.
fn child_with<T: Component>(world: &World, parent: Entity) -> Option<Entity> {
    let children = world.get::<Children>(parent)?;
    children
        .iter()
        .copied()
        .find(|&c| world.get::<T>(c).is_some())
}

/// The checkbox's visible label child (the non-mark `Text` child).
fn checkbox_label_child(world: &World, checkbox: Entity) -> Option<Entity> {
    let children = world.get::<Children>(checkbox)?;
    children
        .iter()
        .copied()
        .find(|&c| world.get::<Text>(c).is_some() && world.get::<CheckboxMark>(c).is_none())
}

/// Walk up from `e` to its owning `TodoRow` (query form, for the collector
/// system). `hierarchy` yields `(is_row, parent)` per entity.
fn ancestor_row(e: Entity, hierarchy: &Query<(Has<TodoRow>, Option<&ChildOf>)>) -> Option<Entity> {
    let mut cur = e;
    for _ in 0..8 {
        let (is_row, parent) = hierarchy.get(cur).ok()?;
        if is_row {
            return Some(cur);
        }
        cur = parent?.parent();
    }
    None
}

/// Walk up from `e` to its owning `TodoRow` ancestor (`&World` form).
fn ancestor_row_world(world: &World, e: Entity) -> Option<Entity> {
    let mut cur = e;
    for _ in 0..8 {
        if world.get::<TodoRow>(cur).is_some() {
            return Some(cur);
        }
        cur = world.get::<ChildOf>(cur)?.parent();
    }
    None
}

/// Whether a row (given its `Children`) is completed — its checkbox is `True`.
fn row_completed(children: &Children, checkboxes: &Query<&A11yToggled, With<RowCheckbox>>) -> bool {
    children
        .iter()
        .copied()
        .filter_map(|c| checkboxes.get(c).ok())
        .any(|t| t.0 == Toggled::True)
}

/// Every completed row (a `&mut World` walk for the exclusive clear-completed).
fn completed_rows(world: &mut World) -> Vec<Entity> {
    let rows: Vec<(Entity, Vec<Entity>)> = {
        let mut q = world.query_filtered::<(Entity, &Children), With<TodoRow>>();
        q.iter(world)
            .map(|(row, children)| (row, children.iter().copied().collect()))
            .collect()
    };
    rows.into_iter()
        .filter(|(_, children)| {
            children.iter().any(|&c| {
                world.get::<RowCheckbox>(c).is_some()
                    && world.get::<A11yToggled>(c).map(|t| t.0) == Some(Toggled::True)
            })
        })
        .map(|(row, _)| row)
        .collect()
}

/// Despawn a row + subtree, clearing focus if the subtree held it (C8 §2.5(4)).
fn despawn_row_restoring_focus(world: &mut World, row: Entity) {
    let focused = world.resource::<FocusedEntity>().0;
    let held = focused.is_some_and(|f| is_descendant(world, f, row));
    world.entity_mut(row).despawn();
    if held {
        world.resource_mut::<FocusedEntity>().0 = None;
    }
}

/// Whether `e` is `ancestor` or a descendant of it.
fn is_descendant(world: &World, e: Entity, ancestor: Entity) -> bool {
    let mut cur = e;
    for _ in 0..16 {
        if cur == ancestor {
            return true;
        }
        match world.get::<ChildOf>(cur) {
            Some(c) => cur = c.parent(),
            None => return false,
        }
    }
    false
}

// ###########################################################################
// S2 — Virtual List / entity-tree table (the scale-game). The design's
// 1000-node entity-tree (values.md § 7.2 Scroll): a heading row (H1 + total
// label + search box) over a bordered table card (sticky header → the
// `ScrollArea` viewport of `table_row`s → footer), with live search-filter +
// single-select row selection wired by [`ScrollListPlugin`].
//
// "Windowing" note: Buiy spawns ALL ~1000 rows + rides `ContentVisibility::Auto`
// (paint/layout-skip off-screen), NOT DOM-style windowed remount — the documented
// v1 ceiling (parity design). The footer reports the visible window from the
// `ScrollOffset`; there is no true row recycling. The off-screen rows cost 1000
// resident entities but only the on-screen window costs paint + shaping.
// ###########################################################################

// The general composites the scroll/menu screens build from were promoted to the
// framework (`buiy_widgets::composites`, Wave 5 refinement) and are font-NEUTRAL —
// the gallery threads its Geist faces (`geist()` / `geist_mono()`) into them.
use buiy_widgets::composites::{
    TableRowData, kbd_content, pulse_blink, search_input, set_table_row_selected, status_dot,
    table_header, table_row,
};

/// The entity-tree node count — the 1000× TodoMVC scale-game (C8 §3.2). One entity
/// per node (the retained-mode model); the off-screen rows ride the landed
/// `ContentVisibility::Auto` skip.
pub const SCROLL_LIST_ROWS: usize = 1000;

/// The S2 scroll-viewport height (logical px). The design's table panel is
/// `flex:1` (fills the canvas); in the unified shell the scroll body needs a
/// bounded height so the 1000-node content overflows + scrolls from the first
/// frame (and so the screen overflows standalone, in the headless tests). A fixed
/// viewport is the documented prototype approximation of the design's `flex:1`
/// panel (the footer still reports the visible window from the `ScrollOffset`).
const SCROLL_VIEWPORT_H: f32 = 360.0;

/// One entity-tree row's fixed height (logical px, values.md § 7.2 "Row height").
/// `content = ROWS × ROW_H` is the scroll extent the clamp reads.
const SCROLL_ROW_H: f32 = 34.0;

/// The frame-ms warn threshold (values.md § 7.2: ms cell is `status.warn` when
/// `> 1.4`, else `text.faint`).
const MS_WARN_THRESHOLD: f32 = 1.4;

/// Tag on the S2 `#ScrollList` container (the `ScrollArea`) — where the rows are
/// appended and the scroll/keyboard handlers target. (Kept named `ScrollList` —
/// the C8b/C8d driver acceptances query `With<ScrollList>` for the live area.)
#[derive(Component, Clone, Default)]
pub struct ScrollList;

/// Tag on the search field inside the scroll heading (the `TextChanged` filter
/// reads its [`TextEditState`]).
#[derive(Component, Clone, Default)]
pub struct ScrollSearch;

/// Tag on the heading "total" label (`#ScrollTotal`) + the footer (`#ScrollFooter`
/// labels) — driven by [`ScrollListPlugin`] from the filtered/visible counts.
#[derive(Component, Clone, Copy)]
pub enum ScrollCountField {
    /// The heading total label ("1,000 nodes · windowed" / "N of 1,000 nodes").
    Total,
    /// The footer left label ("rows X–Y mounted").
    FooterWindow,
    /// The footer right label ("selected #NNNN" / "no selection").
    FooterSelection,
}

/// One entity-tree row's app-state: its node index (so search can hide/show + the
/// footer reports the selection) and lower-cased searchable text (type + name,
/// precomputed so the filter is a cheap substring test). Carried on each
/// `table_row` (which also carries the composite [`TableRow`] marker).
#[derive(Component, Clone)]
pub struct ScrollNode {
    /// The node's 0-based index (the design's `#NNNN` selection id + sort key).
    pub index: usize,
    /// `"{type} {name}"` lower-cased — the search haystack (precomputed once).
    pub haystack: String,
}

/// Marks the currently-selected row (single-select; [`apply_scroll_intents`] keeps
/// at most one). The selected row gets the `accent.soft` bg + the inset-left bar
/// (via [`set_table_row_selected`]).
#[derive(Component, Clone, Copy)]
pub struct SelectedRow;

/// The synthetic node-type cycle (the design JS `TYPES`, values.md § 1.1 type-dot
/// palette + § 9 generator). Each entry is `(type label, dot color token)`.
const SCROLL_TYPES: &[(&str, ColorToken)] = &[
    ("Stack", ColorToken::AccentBlue),
    ("Row", ColorToken::AccentBlue),
    ("Grid", ColorToken::AccentBlue),
    ("Text", ColorToken::TextMuted),
    ("Button", ColorToken::StatusOk),
    ("Icon", ColorToken::StatusOk),
    ("Image", ColorToken::AccentViolet),
    ("Input", ColorToken::StatusWarn),
    ("Scroll", ColorToken::StatusError),
    ("Spacer", ColorToken::TextDim),
];

/// The synthetic node-name stems (the design JS `names`, values.md § 9 generator).
const SCROLL_NAMES: &[&str] = &[
    "root", "panel", "header", "list", "row", "cell", "label", "icon", "field", "track", "thumb",
    "badge", "chip", "card", "menu", "item", "sep", "tip", "modal", "frame",
];

/// One generated entity-tree node (the design JS generator, values.md § 9): the
/// type/dot (`TYPES[(i·7+3)%10]`), depth (`i==0?0:(i·13)%5`), frame-ms
/// (`((i·37)%180)/100+0.02`), state (`i%53==0?WARN : i%131==0?ERR : OK`), and name
/// (`names[(i·11)%20]+'_'+pad4(i)`). Pure — a test pins it.
struct GenNode {
    index: usize,
    node_type: &'static str,
    dot_color: ColorToken,
    depth: usize,
    ms: f32,
    state: &'static str,
    state_color: ColorToken,
    name: String,
}

/// Generate node `i` (`0`-based) exactly like the design JS generator (§ 9).
fn gen_node(i: usize) -> GenNode {
    let (node_type, dot_color) = SCROLL_TYPES[(i * 7 + 3) % SCROLL_TYPES.len()];
    let depth = if i == 0 { 0 } else { (i * 13) % 5 };
    let ms = ((i * 37) % 180) as f32 / 100.0 + 0.02;
    let (state, state_color) = if i.is_multiple_of(53) {
        ("WARN", ColorToken::StatusWarn)
    } else if i.is_multiple_of(131) {
        ("ERR", ColorToken::StatusError)
    } else {
        ("OK", ColorToken::StatusOk)
    };
    let name = format!("{}_{:04}", SCROLL_NAMES[(i * 11) % SCROLL_NAMES.len()], i);
    GenNode {
        index: i,
        node_type,
        dot_color,
        depth,
        ms,
        state,
        state_color,
        name,
    }
}

/// Build the S2 entity-tree screen into `world` and return its `#ScrollScreen`
/// root. The dynamic 1000 rows are appended by [`fill_scroll_list`] (the same
/// "example IS the fixture" idiom S1's todo rows use) — both the binary and the
/// fixture build this same frame then seed the rows.
///
/// Authored imperatively (a `World`-spawning builder, the shell / composites /
/// `spawn_todomvc_screen` idiom) — NOT a `bsn!` scene-fn — because the screen is
/// composite/icon-heavy: the `search_input` composite spawns a real focusable
/// field, and `table_header`/`table_row` build styled icon/dot boxes (this codebase
/// never authors `Icon` leaves in `bsn!`). This mirrors the C3 shift of the
/// icon-heavy S1 from `screen_todomvc(seeds) -> impl Scene` to
/// `spawn_todomvc_screen(world) -> Entity`.
pub fn spawn_scroll_screen(world: &mut World) -> Entity {
    let heading = build_scroll_heading(world);
    let card = build_scroll_card(world);

    // The outer wrap (values.md § 7.2 Scroll "Outer wrap"): `height:100%`, column,
    // `padding:18px 22px`, `min-height:0`. NO explicit width — as a flex-column
    // child of `#ScrollScreen` it `align-items:stretch`es to the full screen width,
    // with the padding INSIDE (so it never overflows the screen the way an explicit
    // `width:100%` + padding content-box would).
    let wrap = world
        .spawn((
            Node,
            Name::new("#ScrollWrap"),
            Style::default()
                .flex_column()
                .height(Sizing::Length(Length::percent(100.0)))
                .min_height(Sizing::Length(Length::px(0.0)))
                .padding_edges(Edges::axis(22.0, 18.0)),
        ))
        .add_children(&[heading, card])
        .id();

    world
        .spawn((
            Node,
            Name::new("#ScrollScreen"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0)))
                .min_height(Sizing::Length(Length::px(0.0))),
        ))
        .add_child(wrap)
        .id()
}

/// The heading row (values.md § 7.2 "Header row"): `gap:12px; margin-bottom:14px`.
/// The "Entity tree" H1 (Geist 18 / 600 / -0.18px LS `text.primary`) + the total
/// label (mono 11 / 500 `text.muted`) + a `flex:1` spacer + the 240px search box
/// (the `search_input` composite, placeholder "Filter nodes…").
fn build_scroll_heading(world: &mut World) -> Entity {
    let h1 = text_leaf(
        world,
        "#ScrollH1",
        "Entity tree",
        geist(),
        18.0,
        600,
        ColorToken::TextPrimary,
        Some(-0.18),
    );

    let total = text_leaf(
        world,
        "#ScrollTotal",
        &scroll_total_text(SCROLL_LIST_ROWS, SCROLL_LIST_ROWS),
        geist_mono(),
        11.0,
        500,
        ColorToken::TextMuted,
        None,
    );
    world.entity_mut(total).insert(ScrollCountField::Total);

    let heading_spacer = world
        .spawn((
            Node,
            Name::new("#ScrollHeadingSpacer"),
            Style::default(),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
        ))
        .id();

    // The 240px search box (the C2 `search_input` composite: magnifier + a real
    // focusable single-line field). Tag the field with `ScrollSearch` so the live
    // `TextChanged` filter reads it.
    let search = search_input(world, "Filter nodes…", geist(), 240.0);
    if let Some(field) = descendant_with_edit_state(world, search) {
        world.entity_mut(field).insert(ScrollSearch);
    }

    world
        .spawn((
            Node,
            Name::new("#ScrollHeading"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(12.0)
                .margin_edges(Edges {
                    bottom: Length::px(14.0),
                    ..Default::default()
                }),
        ))
        .add_children(&[h1, total, heading_spacer, search])
        .id()
}

/// The bordered table card (values.md § 7.2 "Panel"): 1px `border.default`, radius
/// 12, bg `surface.card`, column, `overflow:hidden`, `shadow.card`. A flex-column
/// of the sticky header, the `#ScrollList` `ScrollArea` viewport, and the footer.
fn build_scroll_card(world: &mut World) -> Entity {
    // The sticky header (the C2 `table_header` composite): Index 46 / Node flex /
    // Frame 66 / State 42 (values.md § 7.2 "Table header").
    let header = table_header(
        world,
        &[
            ("INDEX", Some(46.0)),
            ("NODE", None),
            ("FRAME", Some(66.0)),
            ("STATE", Some(42.0)),
        ],
        geist_mono(),
    );

    // The scroll viewport: the `ScrollArea` marker triggers the full C5-a contract
    // (scrollable `Overflow`, `ScrollOffset`/`ScrollExtent`, `Focusable`,
    // `A11yRole::Group`, the SC-4 `A11yScroll` source). The a11y label stays "Items"
    // (the C8b/C8d acceptances find the region by that name). Bounded height so the
    // 1000-row content overflows; the rows are appended by `fill_scroll_list`.
    let list = world
        .spawn_scene(bsn! {
            #ScrollList
            ScrollList
            scroll_area("Items")
            BoxModel {
                width: { Sizing::Length(Length::percent(100.0)) },
                height: { Sizing::Length(Length::Px(SCROLL_VIEWPORT_H)) },
            }
            FlexParams {
                direction: FlexAxis::Column,
            }
        })
        .expect("spawn the scroll viewport")
        .id();

    let footer = build_scroll_footer(world);

    world
        .spawn((
            Node,
            Name::new("#ScrollPanel"),
            Style::default()
                .flex_column()
                .overflow_hidden()
                .min_height(Sizing::Length(Length::px(0.0)))
                .border(1.0),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderDefault, 12.0),
            // shadow.card — `0 12px 32px -16px rgba(0,0,0,.7)` (values.md § 2).
            BoxShadow(vec![Shadow {
                color: ColorToken::ShadowCard,
                offset_x: Length::px(0.0),
                offset_y: Length::px(12.0),
                blur: Length::px(32.0),
                spread: Length::px(-16.0),
                inset: false,
            }]),
        ))
        .add_children(&[header, list, footer])
        .id()
}

/// The table footer (values.md § 7.2 "Footer"): `padding:7px 14px`, top 1px
/// `border.subtle`, bg `surface.inset`, space-between. The left "rows X–Y mounted"
/// window label + the right "selected #NNNN" / "no selection" label (mono 11 / 500;
/// left `text.dim`, right `text.muted`).
fn build_scroll_footer(world: &mut World) -> Entity {
    let window = text_leaf(
        world,
        "#ScrollFooterWindow",
        &scroll_window_text(0, 0),
        geist_mono(),
        11.0,
        500,
        ColorToken::TextDim,
        None,
    );
    world
        .entity_mut(window)
        .insert(ScrollCountField::FooterWindow);

    let selection = text_leaf(
        world,
        "#ScrollFooterSelection",
        SCROLL_NO_SELECTION,
        geist_mono(),
        11.0,
        500,
        ColorToken::TextMuted,
        None,
    );
    world
        .entity_mut(selection)
        .insert(ScrollCountField::FooterSelection);

    world
        .spawn((
            Node,
            Name::new("#ScrollFooter"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::SpaceBetween)
                .padding_edges(Edges::axis(14.0, 7.0))
                .border_edges(Edges {
                    top: Length::px(1.0),
                    ..Default::default()
                }),
            Background {
                color: ColorToken::SurfaceInset,
            },
            Border {
                top: solid_side(ColorToken::BorderSubtle),
                ..Default::default()
            },
        ))
        .add_children(&[window, selection])
        .id()
}

/// Seed `n` entity-tree rows into the `#ScrollList` (the binary + fixture both call
/// this; the rows are dynamic, like S1's todo rows). Each row is the C2 `table_row`
/// composite over a synthetically-generated node (the design's values.md § 9
/// generator), tagged with its [`ScrollNode`] app-state. Returns the row count
/// actually appended.
pub fn fill_scroll_list(world: &mut World, n: usize) -> usize {
    let Some(list) = find_single::<ScrollList>(world) else {
        return 0;
    };
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let node = gen_node(i);
        let idx = format!("{:04}", node.index);
        let ms = format!("{:.2}", node.ms);
        // The tree-indent: `depth·13px`, capped at depth 3 (values.md § 7.2).
        let indent_px = node.depth.min(3) as f32 * 13.0;
        let row = table_row(
            world,
            &TableRowData {
                idx: &idx,
                indent_px,
                dot_color: node.dot_color,
                node_type: node.node_type,
                name: &node.name,
                ms: &ms,
                ms_warn: node.ms > MS_WARN_THRESHOLD,
                state: node.state,
                state_color: node.state_color,
            },
            geist_mono(),
            false,
        );
        // The row carries its app-state: the index + the precomputed lower-cased
        // search haystack (type + name).
        world.entity_mut(row).insert((
            ScrollNode {
                index: node.index,
                haystack: format!("{} {}", node.node_type, node.name).to_lowercase(),
            },
            Name::new(format!("#ScrollRow{i}")),
        ));
        // Pin the row's `min_height` so the flex column cannot shrink the
        // overflowing content back to the viewport (the C5-a "keep it tall so the
        // container overflows" discipline). Patch the single `BoxModel` field — the
        // composite already authored the full row layout (height 34 / gap / padding
        // / border); we only add the shrink floor.
        if let Some(mut box_model) = world.get_mut::<BoxModel>(row) {
            box_model.min_height = Sizing::Length(Length::px(SCROLL_ROW_H));
        }
        rows.push(row);
    }
    world.entity_mut(list).add_children(&rows);
    n
}

/// Startup system for the S2 binary: a camera, the screen, then the rows.
pub fn setup_scroll_list(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        spawn_scroll_screen(world);
        fill_scroll_list(world, SCROLL_LIST_ROWS);
    });
}

/// The heading total label (the design's "1,000 nodes · windowed" at rest /
/// "N of 1,000 nodes" when filtered). `visible` is the post-filter row count;
/// `total` the full node count.
fn scroll_total_text(visible: usize, total: usize) -> String {
    if visible == total {
        format!("{} nodes · windowed", group_thousands(total))
    } else {
        format!(
            "{} of {} nodes",
            group_thousands(visible),
            group_thousands(total)
        )
    }
}

/// The footer window label ("rows X–Y mounted" / "no rows" when empty). `first`/
/// `last` are 1-based inclusive row positions in the filtered list (the visible
/// window the `ScrollOffset` projects).
fn scroll_window_text(first: usize, last: usize) -> String {
    if last == 0 {
        "no rows".to_string()
    } else {
        format!("rows {first}–{last} mounted")
    }
}

/// The footer "no selection" label.
const SCROLL_NO_SELECTION: &str = "no selection";

/// The footer selection label ("selected #NNNN") for node `index`.
fn scroll_selection_text(index: usize) -> String {
    format!("selected #{index:04}")
}

/// Group an integer with thousands separators ("1,000") — the design's `toLocale`.
fn group_thousands(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(b as char);
    }
    out
}

// ===========================================================================
// The ScrollListPlugin — the S2 retained-mode app logic (search + selection).
//
// Mirrors `TodoMvcPlugin`: a `Pointer<Click>` observer + a `TextChanged` intent
// collector stage into `ScrollIntents`, an exclusive `apply_scroll_intents`
// performs the structural mutations (filter via `Display::None`, single-select),
// and a change-detection `reflect_scroll_window` keeps the footer window label in
// sync with the `ScrollOffset`. All `.after(BuiySet::Input).before(A11yUpdate)`.
// ===========================================================================

/// Staged S2 interaction intents, read once per frame by the collectors and
/// consumed by [`apply_scroll_intents`]. Empty between frames.
#[derive(Resource, Default)]
pub struct ScrollIntents {
    /// A new search query (the lower-cased filter), staged when the search field's
    /// text changed this frame. `Some("")` clears the filter (all rows shown).
    pub search: Option<String>,
    /// A row whose body was clicked this frame (the new single-selection target).
    pub select: Option<Entity>,
}

/// The S2 app logic. Registers the click observer + the search-change collector +
/// the exclusive applier + the footer-window reflect, all `.after(BuiySet::Input)`
/// (so the same frame's `TextChanged` / `Pointer<Click>` are read) and
/// `.before(BuiySet::A11yUpdate)` (so any `A11yHidden` filter prune lands in the
/// same frame's tree rebuild — mirrors `TodoMvcPlugin`).
pub struct ScrollListPlugin;

impl Plugin for ScrollListPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScrollIntents>()
            .add_observer(on_scroll_row_click)
            .add_systems(
                Update,
                (
                    collect_scroll_search,
                    apply_scroll_intents,
                    reflect_scroll_window,
                )
                    .chain()
                    .after(BuiySet::Input)
                    .before(BuiySet::A11yUpdate),
            );
    }
}

/// Observer: a click on a row body (a `ScrollNode` row — its cells are
/// `Pickable::IGNORE` so the hit resolves to the row) stages it as the new
/// single-selection. `Pointer<Click>` bubbles, so gate on the original (leaf)
/// target — without this the click would also fire for each ancestor hop.
pub fn on_scroll_row_click(
    click: On<bevy::picking::events::Pointer<bevy::picking::events::Click>>,
    rows: Query<(), With<ScrollNode>>,
    mut intents: ResMut<ScrollIntents>,
) {
    if click.entity != click.original_event_target() {
        return;
    }
    let target = click.original_event_target();
    if rows.get(target).is_ok() {
        intents.select = Some(target);
    }
}

/// Stage the search field's new value whenever its `TextEditState` changes
/// (`Changed` change-detection — the single source of truth that fires for EVERY
/// edit path: keyboard typing, IME commit, the a11y driver's `set_value`, all of
/// which mutate `TextEditState`; the host-facing `TextChanged` message only fires
/// on the keyboard path). Reads the `ScrollSearch` field's value and stages the
/// lower-cased query (so [`apply_scroll_intents`] filters the rows).
pub fn collect_scroll_search(
    fields: Query<&TextEditState, (With<ScrollSearch>, Changed<TextEditState>)>,
    mut intents: ResMut<ScrollIntents>,
) {
    for state in &fields {
        intents.search = Some(state.value().trim().to_lowercase());
    }
}

/// Consume the staged [`ScrollIntents`]: apply the search filter (hide
/// non-matching rows via `Display::None` so the card hugs the matches + the
/// `ScrollExtent` shrinks) and the single-select row selection (clear the prior
/// selection, mark + restyle the new one), then drive the heading total + footer
/// selection labels. Clears the staging at the end.
pub fn apply_scroll_intents(world: &mut World) {
    let intents = std::mem::take(&mut *world.resource_mut::<ScrollIntents>());

    if let Some(query) = intents.search {
        apply_scroll_filter(world, &query);
    }

    if let Some(row) = intents.select {
        apply_scroll_selection(world, row);
    }
}

/// Filter the rows by the (already lower-cased, trimmed) `query`: a matching row
/// (substring of its `ScrollNode.haystack`, or any row when the query is empty) is
/// shown (`Display::flex` relative); a non-matching row is `Display::None` (pruned
/// from Taffy, so the visible rows pack + the `ScrollExtent` shrinks). Updates the
/// heading total label to the visible count.
fn apply_scroll_filter(world: &mut World, query: &str) {
    let rows: Vec<(Entity, bool)> = {
        let mut q = world.query::<(Entity, &ScrollNode)>();
        q.iter(world)
            .map(|(e, node)| (e, query.is_empty() || node.haystack.contains(query)))
            .collect()
    };
    let total = rows.len();
    let mut visible = 0usize;
    for (row, show) in rows {
        if show {
            visible += 1;
            world
                .entity_mut(row)
                .insert(Display::flex_row())
                .remove::<A11yHidden>();
        } else {
            world.entity_mut(row).insert((Display::None, A11yHidden));
        }
    }
    set_scroll_count(
        world,
        ScrollCountField::Total,
        scroll_total_text(visible, total),
    );
}

/// Apply a single-select row selection: clear the prior [`SelectedRow`] (restoring
/// its transparent bg + dropping the inset bar) and mark + restyle the clicked row
/// (`accent.soft` bg + the inset-left accent bar), then set the footer selection
/// label to "selected #NNNN".
fn apply_scroll_selection(world: &mut World, row: Entity) {
    // Clear the prior selection (if any, and not the same row).
    let prior: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<SelectedRow>>();
        q.iter(world).filter(|&e| e != row).collect()
    };
    for old in prior {
        world.entity_mut(old).remove::<SelectedRow>();
        set_table_row_selected(world, old, false);
    }

    world.entity_mut(row).insert(SelectedRow);
    set_table_row_selected(world, row, true);

    let label = match world.get::<ScrollNode>(row) {
        Some(node) => scroll_selection_text(node.index),
        None => SCROLL_NO_SELECTION.to_string(),
    };
    set_scroll_count(world, ScrollCountField::FooterSelection, label);
}

/// Rewrite the [`ScrollCountField`]-tagged label's `Text` (the heading total or a
/// footer label). A no-op when the field is absent or already correct.
fn set_scroll_count(world: &mut World, field: ScrollCountField, value: String) {
    let want = std::mem::discriminant(&field);
    let target = {
        let mut q = world.query::<(Entity, &ScrollCountField)>();
        q.iter(world)
            .find(|(_, f)| std::mem::discriminant(*f) == want)
            .map(|(e, _)| e)
    };
    if let Some(e) = target
        && let Some(mut text) = world.get_mut::<Text>(e)
        && text.0 != value
    {
        text.0 = value;
    }
}

/// Reflect the live scroll window into the footer "rows X–Y mounted" label: a
/// change-detection system that runs when the `#ScrollList` `ScrollOffset` changes
/// (a wheel / key scroll). Projects the offset + the viewport height onto the
/// VISIBLE (non-`Display::None`) rows to report the mounted window. (Buiy spawns
/// all rows; this reports the on-screen window the design's true windowing would
/// remount — the documented v1 ceiling.)
#[allow(clippy::type_complexity)]
pub fn reflect_scroll_window(
    lists: Query<(Ref<ScrollOffset>, &Children), With<ScrollList>>,
    rows: Query<&Display, With<ScrollNode>>,
    mut labels: Query<(&ScrollCountField, &mut Text)>,
) {
    let Ok((offset, children)) = lists.single() else {
        return;
    };
    if !offset.is_changed() {
        return;
    }
    // The count of currently-visible (non-collapsed) rows.
    let visible = children
        .iter()
        .copied()
        .filter(|&c| rows.get(c).map(|d| *d != Display::None).unwrap_or(false))
        .count();
    let (first, last) = scroll_window_range(offset.y, visible);

    for (field, mut text) in &mut labels {
        if matches!(field, ScrollCountField::FooterWindow) {
            let next = scroll_window_text(first, last);
            if text.0 != next {
                text.0 = next;
            }
        }
    }
}

/// The 1-based inclusive `(first, last)` row positions visible in the viewport at
/// scroll offset `offset_y` over `visible` total rows (each [`SCROLL_ROW_H`] tall,
/// the [`SCROLL_VIEWPORT_H`] viewport). Empty list → `(0, 0)`.
fn scroll_window_range(offset_y: f32, visible: usize) -> (usize, usize) {
    if visible == 0 {
        return (0, 0);
    }
    let first = (offset_y / SCROLL_ROW_H).floor().max(0.0) as usize;
    let rows_in_view = (SCROLL_VIEWPORT_H / SCROLL_ROW_H).ceil() as usize;
    let last = (first + rows_in_view).min(visible);
    // 1-based inclusive for display.
    (first + 1, last)
}

/// The number of rows in the current visible window — the design's inspState
/// `mounted` (`visRows.length`) and the footer's "rows X–Y mounted" span size,
/// from the SAME [`scroll_window_range`] math (single source of truth). `offset_y`
/// is the live `ScrollOffset.y`; `visible` the filtered (non-`Display::None`) row
/// count. Buiy mounts every matching row (real overflow, off-screen paint-skip),
/// so the literal mounted count is the filtered total — but the design's `mounted`
/// is the WINDOWED count, which is what both the footer and inspector report.
pub(crate) fn scroll_window_size(offset_y: f32, visible: usize) -> usize {
    let (first, last) = scroll_window_range(offset_y, visible);
    last.saturating_sub(first.saturating_sub(1))
}

// ###########################################################################
// S3 — overlay / menu. A file card whose ⋮ MenuButton opens an anchored dropdown
// of 5 actions, a footer "last action" indicator, plus a TooltipTrigger + a
// standalone Popover (the catalog's other two overlay primitives).
// ###########################################################################

/// Tag on each S3 menu item — carries the item's index so an activation records a
/// content-keyed effect ([`MenuActivations`]) the driver can observe **and** updates
/// the footer "last action" value ([`update_last_action`]).
#[derive(Component, Clone, Copy, Default)]
pub struct MenuAction(pub usize);

/// Tag on the footer "last action" value `Text` ([`MenuLastActionField`]) so
/// [`update_last_action`] can rewrite it from the activated item's label.
#[derive(Component, Clone, Copy, Default)]
pub struct MenuLastActionField;

/// The observable effect of a menu-item activation (the S3 grounding loop): the
/// labels of the items whose shared `OnPress` sink fired, in order. The driver
/// asserts an Enter on the active item appends to this — proving activation
/// reaches an app-level effect, not just the a11y state.
#[derive(Resource, Default)]
pub struct MenuActivations(pub Vec<String>);

/// The labels of the S3 menu's 5 file-action items, in document order (values.md
/// § 6 #15–#19 / the design's `MI` array). Index `i` is the item carrying
/// [`MenuAction`]`(i)`; both [`record_menu_activation`] (the effect log) and the
/// C8b acceptance resolve an activation through this single source.
pub const MENU_ITEM_LABELS: &[&str] = &["Open", "Rename", "Duplicate", "Copy link", "Delete"];

/// The footer "last action" value at rest (before any item fires) — the design's
/// `lastAction: '—'` initial state (values.md § 9).
pub const MENU_NO_ACTION: &str = "—";

/// One S3 menu-item's design data (values.md § 6 #15–#19 / the `MI` array): its
/// label, keyboard shortcut glyph, the 15px stroked icon path, and whether it is
/// the destructive "Delete" item (which tints the label `text.danger` + the kbd
/// `text.danger-dim`).
struct MenuItemSpec {
    /// The item label (Geist 450 13px, `text.bright` / danger `text.danger`).
    label: &'static str,
    /// The keyboard shortcut glyph (mono 10px, `text.dim` / danger `text.danger-dim`).
    kbd: &'static str,
    /// The 15px leading icon's SVG path (stroke 1.7, inherits the item color).
    icon: &'static str,
    /// Whether this is the destructive "Delete" item (danger tints).
    danger: bool,
}

/// The S3 menu's 5 items, in document order (values.md § 6 #15–#19).
const MENU_ITEMS: [MenuItemSpec; 5] = [
    MenuItemSpec {
        label: "Open",
        kbd: "↵",
        icon: "M14 4h6v6M20 4l-9 9M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6",
        danger: false,
    },
    MenuItemSpec {
        label: "Rename",
        kbd: "F2",
        icon: "M4 20h4L19 9a2 2 0 0 0-3-3L5 17zM14 7l3 3",
        danger: false,
    },
    MenuItemSpec {
        label: "Duplicate",
        kbd: "⌘D",
        icon: "M9 9h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V10a1 1 0 0 1 1-1M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1",
        danger: false,
    },
    MenuItemSpec {
        label: "Copy link",
        kbd: "⌘L",
        icon: "M9 15l6-6M10.5 6.5 12 5a4 4 0 0 1 6 6l-1.5 1.5M13.5 17.5 12 19a4 4 0 0 1-6-6l1.5-1.5",
        danger: false,
    },
    MenuItemSpec {
        label: "Delete",
        kbd: "⌫",
        icon: "M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13h10l1-13",
        danger: true,
    },
];

/// The menu folder-tile accent-folder icon (values.md § 6 #14, stroke 1.7).
const MENU_FOLDER_ICON: &str =
    "M3 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z";
/// The ⋮ menu-button icon (values.md § 6 #13, vert-dots, stroke 2.4).
const MENU_DOTS_ICON: &str = "M12 6h.01M12 12h.01M12 18h.01";

/// Spawn the S3 screen into `world`: the centered file **card** (header icon-tile +
/// name/path column + the ⋮ [`MenuButton`] whose controlled [`Menu`] holds the 5
/// file-action [`MenuItem`]s, + a footer blink-dot/"last action" strip), a caption,
/// plus the catalog's other two overlay primitives — a [`TooltipTrigger`] ("?") and
/// a standalone anchored [`Popover`] (both still wired so the C8b acceptance's
/// tooltip-show + popover-dismiss drivers have their targets). Returns the screen
/// root (`#MenuScreen`). Shared by the binary ([`setup_overlay_menu`]), the shell
/// mount, and the fixtures so all three render the same tree.
///
/// The ⋮ button + menu + items are the bare `buiy_widgets` markers ([`MenuButton`]/
/// [`Menu`]/[`MenuItem`]) with the design box/paint **explicitly** inserted (the
/// `#[require]` defaults only fill absent components, so the explicit boxes win) and
/// the design's icon/label/kbd row authored as each item's own children — the same
/// imperative idiom S1/S2 use. Authoring the markers directly (rather than the
/// `menu_button`/`menu_item` scene-fns, whose baked-in centered-text children fight
/// the icon-row layout — the § 4.1c suppression gotcha) keeps the full a11y
/// machinery (`wire_menu_button` controls/anchor edges, the `MenuModel` funnel
/// (`route_menu_press`/`menu_reducer`/`bind_menu_model`) for open/close, roving
/// `menu_keyboard_nav`, the `auto` `LightDismiss` outside-click close) while
/// the rows match the design pixel-for-pixel. Each item carries its [`MenuAction`]
/// index so an activation logs to [`MenuActivations`] + updates the footer.
pub fn spawn_overlay_menu(world: &mut World) -> Entity {
    let card = build_menu_card(world);

    // The caption below the card (values.md § 4 "Menu — body paragraph": Geist 12 /
    // 400 `text.dim`, line-height 1.6). `margin:16px 4px 0` per the design.
    //
    // The design authors the literal `⋮` (U+22EE) here and leans on the browser's
    // font fallback. Buiy's registered-only font system (Geist / Geist Mono /
    // Fira) carries no `⋮`, so the literal tofus (the same M4 class as `⌘`/`∅`).
    // An inline vector `⋮` icon (the M4 `kbd_with_cmd` route) is not clean inside
    // a flowing, wrapped paragraph — each text run would become an atomic flex
    // item and break natural word-wrapping. So the caption references the trigger
    // by name ("menu button"), consistent with its own word-convention for keys
    // ("Esc to dismiss", not the `⎋` glyph) — preserving the meaning, no tofu.
    let caption = text_leaf(
        world,
        "#MenuCaption",
        "Anchored overlay with arrow-key roving focus, Esc to dismiss, and an outside-click scrim. Click the menu button to open.",
        geist(),
        12.0,
        400,
        ColorToken::TextDim,
        None,
    );
    world.entity_mut(caption).insert((
        Style::default()
            .width(Sizing::Length(Length::px(420.0)))
            .margin_edges(Edges {
                top: Length::px(16.0),
                right: Length::px(4.0),
                left: Length::px(4.0),
                ..Default::default()
            }),
        LineHeight::Scale(1.6),
    ));

    // The card-wrap column (`width:420px`) holding the card + the caption.
    let card_wrap = world
        .spawn((
            Node,
            Name::new("#MenuCardWrap"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::px(420.0))),
        ))
        .add_children(&[card, caption])
        .id();

    // The catalog's other two overlay primitives, kept so the C8b acceptance's
    // tooltip-show + popover-dismiss drivers have their live targets. The
    // `tooltip_trigger` "?" stays addressable by role Generic + name "?"; the
    // standalone `Popover` anchors to it. Both start hidden — the design's menu
    // screen does not surface them, so they sit in a zero-size, `Display::None`
    // holder that never paints but keeps the wiring + a11y nodes alive.
    let tooltip = world
        .spawn_scene(tooltip_trigger("?", "More info here"))
        .expect("spawn the tooltip trigger")
        .id();
    world.entity_mut(tooltip).insert(Name::new("InfoTip"));
    let popover = world
        .spawn((
            buiy_widgets::Popover {
                anchor: Some(tooltip),
                ..Default::default()
            },
            Name::new("InfoPopover"),
            CssVisibility::Hidden,
            Style::default().width_px(160.0).height_px(80.0),
            children![(
                Name::new("InfoPopoverText"),
                Text("Anchored panel".to_string()),
                FontSize(14.0),
                Pickable::IGNORE,
            )],
        ))
        .id();
    // A `Display::None` holder so the two primitives stay in the screen subtree
    // (riding the router toggle + a11y prune with the rest of S3) without painting.
    let overlay_holder = world
        .spawn((
            Node,
            Name::new("#MenuOverlayPrimitives"),
            Style::default().display(Display::None),
        ))
        .add_children(&[tooltip, popover])
        .id();

    // The screen root: the design's centering wrap (`min-height:100%; center;
    // padding:40px`) holding the card-wrap + the (non-painting) overlay holder.
    world
        .spawn((
            Node,
            Name::new("#MenuScreen"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0)))
                .min_height(Sizing::Length(Length::px(0.0)))
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .padding(40.0),
        ))
        .add_children(&[card_wrap, overlay_holder])
        .id()
}

/// Build the design's file **card** (values.md § 7.2 Menu "Card"): 1px
/// `border.default`, radius 12, bg `surface.card`, `shadow.card`, a column of the
/// header row + the footer strip. Returns the `#OverlayCard` root.
fn build_menu_card(world: &mut World) -> Entity {
    let header = build_menu_header(world);
    let footer = build_menu_footer(world);
    world
        .spawn((
            Node,
            Name::new("#OverlayCard"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::px(420.0)))
                .border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderDefault, 12.0),
            // shadow.card — `0 12px 32px -16px rgba(0,0,0,.7)` (values.md § 2).
            BoxShadow(vec![Shadow {
                color: ColorToken::ShadowCard,
                offset_x: Length::px(0.0),
                offset_y: Length::px(12.0),
                blur: Length::px(32.0),
                spread: Length::px(-16.0),
                inset: false,
            }]),
        ))
        .add_children(&[header, footer])
        .id()
}

/// The card **header** row (values.md § 7.2 Menu "Header": `gap:12px; padding:14px
/// 16px`, bottom 1px `border.subtle`): a 34×34 accent folder-icon tile + the
/// name/path column + a flex spacer + the ⋮ [`MenuButton`].
fn build_menu_header(world: &mut World) -> Entity {
    // The 34×34 icon tile (radius 8, bg `surface.raised-alt`) holding the accent
    // folder Icon (17px, stroke 1.7, `color.accent`).
    let folder = icon_box(
        world,
        "#MenuFolderIcon",
        MENU_FOLDER_ICON,
        1.7,
        17,
        ColorToken::Accent,
    );
    let tile = world
        .spawn((
            Node,
            Name::new("#MenuFileTile"),
            Style::default()
                .width_px(34.0)
                .height_px(34.0)
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center),
            FlexItem {
                shrink: 0.0,
                ..Default::default()
            },
            Background {
                color: ColorToken::SurfaceRaisedAlt,
            },
            Border {
                radius: Corners::all(Radius::circular(8.0)),
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .add_child(folder)
        .id();

    // The name/path column ("primary_button.bsn" Geist 14 / 500 `text.primary` +
    // "crates/buiy_widgets · 1.2 KB" mono 11.5 / 400 `text.faint`), `gap:2px`.
    let name = text_leaf(
        world,
        "#MenuFileName",
        "primary_button.bsn",
        geist(),
        14.0,
        500,
        ColorToken::TextPrimary,
        None,
    );
    let path = text_leaf(
        world,
        "#MenuFilePath",
        "crates/buiy_widgets · 1.2 KB",
        geist_mono(),
        11.5,
        400,
        ColorToken::TextFaint,
        None,
    );
    let name_col = world
        .spawn((
            Node,
            Name::new("#MenuNameCol"),
            Style::default()
                .flex_column()
                .gap_px(2.0)
                .min_width(Sizing::Length(Length::px(0.0))),
        ))
        .add_children(&[name, path])
        .id();

    let spacer = world
        .spawn((
            Node,
            Name::new("#MenuHeaderSpacer"),
            Style::default(),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .id();

    // The ⋮ trigger + its controlled dropdown menu. The menu is a SIBLING of the
    // button (both header children), NOT a button child: the popover anchor override
    // (`anchor_resolution`) computes the menu's position in the button's-PARENT
    // frame (the header), and the render transform bridge composes a node's position
    // through its ECS parent — so the menu must share the button's parent for the
    // two frames to agree. Authoring the menu inside the button (the `MenuButton::new`
    // default) double-counts the button's offset and flings the dropdown off-screen
    // (a real buiy popover-as-anchor-child positioning bug this restyle surfaced —
    // see the campaign journal). As a header sibling the dropdown lands directly
    // below the ⋮ button. The `controls`/`Popover.anchor` edges are wired manually
    // (`wire_menu_button` only auto-wires a menu found among the button's children).
    let (menu_button, menu) = build_menu_button(world);

    let header = world
        .spawn((
            Node,
            Name::new("#MenuHeader"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(12.0)
                .padding_edges(Edges::axis(16.0, 14.0))
                .border_edges(Edges {
                    bottom: Length::px(1.0),
                    ..Default::default()
                }),
            Border {
                bottom: solid_side(ColorToken::BorderSubtle),
                ..Default::default()
            },
        ))
        .add_children(&[tile, name_col, spacer, menu_button, menu])
        .id();

    // Wire the button↔menu edges manually (the menu is a sibling, so the
    // `Added<Children>`-gated `wire_menu_button` would not find it): the button
    // `controls` the menu (the edge `route_menu_press` and `bind_menu_model` follow
    // between the button and its `MenuModel`-bearing menu), and the menu's
    // `Popover.anchor` points at the button (so `position_popover` places it below the ⋮).
    world.entity_mut(menu_button).insert(A11yRelations {
        controls: vec![menu],
        ..Default::default()
    });
    if let Some(mut popover) = world.get_mut::<buiy_widgets::Popover>(menu) {
        popover.anchor = Some(menu_button);
    }
    header
}

/// Build the ⋮ [`MenuButton`] (32×32, radius 8, border `border.default`, bg
/// `surface.inset`; values.md § 7.2 Menu "Menu ⋮ button" / the `menuBtnStyle`) and
/// its controlled [`Menu`] dropdown (the 5 file-action items). Returns
/// `(button, menu)` — the caller adds **both** as siblings under the header and
/// wires the `controls`/`anchor` edges (see [`build_menu_header`] for why the menu
/// is a sibling, not a button child).
///
/// Built from the bare markers + explicit design boxes (the `#[require]` defaults
/// only fill absent components, so these win): a 17px ⋮ Icon (centered in a
/// flex wrapper) gives the button its glyph. `A11yLabel` names the trigger for the
/// AT / tab traversal. The menu starts hidden (`CssVisibility::Hidden`).
fn build_menu_button(world: &mut World) -> (Entity, Entity) {
    // The ⋮ glyph in a flex-centered wrapper that fills the button. The wrapper
    // (not the button) is the flex container, so the in-flow `Menu` sibling stays
    // OUT of the centering row — authoring the menu directly on a centered button
    // row would squeeze it (and collapse the icon to 0), the flex-squeeze the
    // original `menu_button` dodged by being 120px wide + `Display::Block`.
    let dots = icon_box(
        world,
        "#MenuDotsIcon",
        MENU_DOTS_ICON,
        2.4,
        17,
        ColorToken::TextMuted,
    );
    let dots_center = world
        .spawn((
            Node,
            Name::new("#MenuDotsCenter"),
            Style::default()
                .width(Sizing::Length(Length::percent(100.0)))
                .height(Sizing::Length(Length::percent(100.0)))
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center),
            Pickable::IGNORE,
        ))
        .add_child(dots)
        .id();
    let menu = build_menu_dropdown(world);
    // 32×32 + 1px border = 34×34 outer (content-box), the design's rendered ⋮
    // button. The button holds ONLY the centered-icon wrapper; the menu is added as
    // a header sibling by the caller (the popover frame-match — see
    // `build_menu_header`).
    let button = world
        .spawn((
            buiy_widgets::MenuButton,
            Name::new("#MenuTrigger"),
            A11yLabel("File actions".to_string()),
            Style::default().width_px(32.0).height_px(32.0).border(1.0),
            FlexItem {
                shrink: 0.0,
                ..Default::default()
            },
            Background {
                color: ColorToken::SurfaceInset,
            },
            border_all(ColorToken::BorderDefault, 8.0),
        ))
        .add_child(dots_center)
        .id();
    (button, menu)
}

/// Build the [`Menu`] dropdown panel (values.md § 7.2 Menu "Dropdown": width 218,
/// `padding:5px`, 1px `border.strong`, radius 10, bg `surface.raised`,
/// `shadow.menu`) holding the 5 file-action [`MenuItem`] rows. Returns the menu.
///
/// The bare `Menu` marker carries the popover-positioning + roving-focus
/// machinery via its `#[require]`; the explicit box/paint here override the
/// require defaults to the design dropdown. Starts `CssVisibility::Hidden` (a button
/// press enqueues `MenuMsg` → `menu_reducer` flips `MenuModel.open` → `bind_menu_model`
/// projects it onto `CssVisibility`).
fn build_menu_dropdown(world: &mut World) -> Entity {
    let items: Vec<Entity> = MENU_ITEMS
        .iter()
        .enumerate()
        .map(|(i, spec)| build_menu_item(world, i, spec))
        .collect();
    let menu = world
        .spawn((
            buiy_widgets::Menu,
            Name::new("#MenuDropdown"),
            CssVisibility::Hidden,
            // `position:absolute` (the design's dropdown) so the 218px menu does NOT
            // sit in the header's flex row (which would widen the header / shove the
            // chrome). As a HEADER child its absolute containing block + the popover
            // anchor override share the header frame, so the override composes
            // correctly and the dropdown lands below the ⋮ button. `flex-column` lays
            // the 5 item rows top-to-bottom.
            Style::default()
                .flex_column()
                .absolute()
                .width_px(218.0)
                .padding(5.0)
                .border(1.0),
            Background {
                color: ColorToken::SurfaceRaised,
            },
            border_all(ColorToken::BorderStrong, 10.0),
            // shadow.menu — `0 16px 40px -12px rgba(0,0,0,.8)` (values.md § 2).
            BoxShadow(vec![Shadow {
                color: ColorToken::ShadowMenu,
                offset_x: Length::px(0.0),
                offset_y: Length::px(16.0),
                blur: Length::px(40.0),
                spread: Length::px(-12.0),
                inset: false,
            }]),
        ))
        .id();
    world.entity_mut(menu).add_children(&items);
    menu
}

/// Build one file-action [`MenuItem`] row (values.md § 7.2 Menu "Menu item":
/// `gap:10px; padding:8px 9px`, radius 7): a 15px leading Icon + the label (Geist
/// 450 13px `text.bright`, danger `text.danger`) + a flex spacer + the kbd glyph
/// (mono 10px `text.dim`, danger `text.danger-dim`). Carries [`MenuAction`]`(idx)`.
fn build_menu_item(world: &mut World, idx: usize, spec: &MenuItemSpec) -> Entity {
    let (label_color, kbd_color) = if spec.danger {
        (ColorToken::TextDanger, ColorToken::TextDangerDim)
    } else {
        (ColorToken::TextBright, ColorToken::TextDim)
    };
    let icon = icon_box(world, "#MenuItemIcon", spec.icon, 1.7, 15, label_color);
    let label = text_leaf(
        world,
        "#MenuItemLabel",
        spec.label,
        geist(),
        13.0,
        450,
        label_color,
        None,
    );
    let label_spacer = world
        .spawn((
            Node,
            Name::new("#MenuItemSpacer"),
            Style::default(),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let kbd_chip = kbd_content(world, "#MenuItemKbd", spec.kbd, geist_mono(), kbd_color);
    // A per-item name (the label slug) so the layout-dump can order the items
    // deterministically even when the screen is INACTIVE in the shell (all items
    // collapse to pos/size 0 under `Display::None`, where same-named siblings are
    // ambiguous to the snapshot sorter).
    let item_name = format!("#MenuItem-{}", spec.label.replace(' ', ""));
    world
        .spawn((
            buiy_widgets::MenuItem,
            MenuAction(idx),
            A11yLabel(spec.label.to_string()),
            Name::new(item_name),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(10.0)
                .padding_edges(Edges::axis(9.0, 8.0)),
            Background {
                color: ColorToken::Transparent,
            },
            Border {
                radius: Corners::all(Radius::circular(7.0)),
                ..Default::default()
            },
        ))
        .add_children(&[icon, label, label_spacer, kbd_chip])
        .id()
}

/// The card **footer** strip (values.md § 7.2 Menu "Footer": `padding:14px 16px;
/// gap:10px`, bg `surface.inset`, bottom corners radius 11): the accent blink dot +
/// "last action" label + the last-action value (mono 12px `text.secondary`, "—"
/// until an item fires; carries [`MenuLastActionField`]).
fn build_menu_footer(world: &mut World) -> Entity {
    // The 8×8 accent blink dot (values.md § 6 "Menu blink dot": bg `color.accent`,
    // `box-shadow:0 0 0 4px color.accent.soft` ring). The C2 `status_dot` composite
    // builds the dot + the ring at 7×7; the menu dot is 8×8, so resize it.
    //
    // The design's `animation:blink 1.6s infinite` (opacity 1→.25→1) is driven by
    // `pulse_blink` — an infinite ping-pong `OpacityTween` (finding M3 added the
    // looping/`Repeat` capability). `Opacity < 1` auto-forms an effect group, so the
    // dot composites + pulses; reduced motion snaps it to a steady-lit 1.0. (Invisible
    // in the single-frame capture, but the live app pulses.)
    let dot = status_dot(world, ColorToken::Accent, ColorToken::AccentSoft, 0.0, 4.0);
    world.entity_mut(dot).insert((
        Name::new("#MenuBlinkDot"),
        Style::default().width_px(8.0).height_px(8.0),
        FlexItem {
            shrink: 0.0,
            ..Default::default()
        },
    ));
    pulse_blink(world, dot);

    let label = text_leaf(
        world,
        "#MenuLastActionLabel",
        "last action",
        geist_mono(),
        11.5,
        500,
        ColorToken::TextMuted,
        None,
    );
    let value = text_leaf(
        world,
        "#MenuLastAction",
        MENU_NO_ACTION,
        geist_mono(),
        12.0,
        500,
        ColorToken::TextSecondary,
        None,
    );
    world.entity_mut(value).insert(MenuLastActionField);

    world
        .spawn((
            Node,
            Name::new("#MenuFooter"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(10.0)
                .padding_edges(Edges::axis(16.0, 14.0)),
            Background {
                color: ColorToken::SurfaceInset,
            },
            // Bottom corners radius 11 (the design's `border-radius:0 0 11px 11px`);
            // the top is flush against the header divider.
            Border {
                radius: Corners {
                    bottom_left: Radius::circular(11.0),
                    bottom_right: Radius::circular(11.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .add_children(&[dot, label, value])
        .id()
}

/// Startup system for the S3 binary: a camera, then the overlay screen.
pub fn setup_overlay_menu(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        spawn_overlay_menu(world);
    });
}

/// The S3 app logic: record an observable effect + update the footer when a menu
/// item is activated. A menu item's activation rides the shared [`OnPress`] sink
/// (written by the menu's Enter/Space keyboard nav, or the pointer path); these
/// systems read it `.after(BuiySet::Input)` (the C8 §2.5(1) ordering) and (1)
/// append the activated item's label to [`MenuActivations`] — the grounding-loop
/// effect the driver asserts — and (2) rewrite the footer "last action" value.
pub struct OverlayMenuPlugin;

impl Plugin for OverlayMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuActivations>().add_systems(
            Update,
            (record_menu_activation, update_last_action).after(BuiySet::Input),
        );
    }
}

/// Append each activated menu item's label to [`MenuActivations`]. Reads the
/// shared `OnPress` sink (the same sink the menu keyboard nav writes for the
/// active item) and resolves the pressed entity's [`MenuAction`] → its label.
pub fn record_menu_activation(
    mut reader: MessageReader<OnPress>,
    items: Query<&MenuAction>,
    mut log: ResMut<MenuActivations>,
) {
    for OnPress(e) in reader.read() {
        if let Ok(action) = items.get(*e)
            && let Some(label) = MENU_ITEM_LABELS.get(action.0)
        {
            log.0.push((*label).to_string());
        }
    }
}

/// Rewrite the footer "last action" value [`Text`] ([`MenuLastActionField`]) from
/// the most-recently-activated menu item. Reads the same [`OnPress`] sink as
/// [`record_menu_activation`]; the last item pressed this frame wins (one footer
/// value). A no-op when nothing fired.
pub fn update_last_action(
    mut reader: MessageReader<OnPress>,
    items: Query<&MenuAction>,
    mut field: Query<&mut Text, With<MenuLastActionField>>,
) {
    let mut latest = None;
    for OnPress(e) in reader.read() {
        if let Ok(action) = items.get(*e)
            && let Some(label) = MENU_ITEM_LABELS.get(action.0)
        {
            latest = Some(*label);
        }
    }
    if let Some(label) = latest
        && let Ok(mut text) = field.single_mut()
    {
        text.0 = label.to_string();
    }
}

// ###########################################################################
// S4 — modal + focus-trap (parity Wave C3). Two trigger buttons invoke a C5-d
// Dialog restyled to the design's create/delete modal (header → create/delete
// body → footer). Reuses the C2 `segmented`/`kbd` composites + the buiy_widgets
// `Switch` + the BackdropFilter scrim. The whole open/close/focus-trap/Esc/restore
// + inert-background lifecycle is the C5-d `WidgetsPlugin` overlay state machine;
// S4 is pure composition over it, plus a thin `ModalPlugin` that swaps the
// create/delete body when a trigger fires (the design's `modalMode`).
//
// Every value (px / color / radius / font / letter-spacing) comes from
// `docs/specs/2026-06-25-widget-catalog-values.md` (§ 7.2 Modal, § 4 typography,
// § 2 shadows, § 3 radii, § 6 icons).
// ###########################################################################

/// The create-mode dialog title (the design's `modalTitle` when create — JS 712).
pub const MODAL_TITLE: &str = "New widget";
/// The create-mode dialog subtitle (the design's `modalSub` when create).
pub const MODAL_SUB: &str = "Scaffold a primitive into the registry";
/// The delete-mode dialog title.
pub const MODAL_TITLE_DELETE: &str = "Delete widget";
/// The delete-mode dialog subtitle.
pub const MODAL_SUB_DELETE: &str = "Confirm destructive action";
/// The delete-body paragraph (the design's delete warning — HTML 289).
pub const MODAL_BODY: &str =
    "This permanently removes primary_button.bsn and its 3 dependents. This cannot be undone.";

/// The "New widget" create-trigger button's accessible name (the create invoker;
/// the driver activates it to open the dialog — the `MODAL_INVOKER` constant the
/// modal acceptance tests address by name).
pub const MODAL_INVOKER: &str = "New widget";
/// The "Delete" trigger button's accessible name. A second invoker that opens the
/// dialog in delete mode; it is also the focusable OUTSIDE the open dialog the
/// inert-prune acceptance proves (it is pruned while the modal is open + restored
/// on close — the `MODAL_BG_BUTTON` role the tests address by name).
pub const MODAL_BG_BUTTON: &str = "Delete";

/// Which body the modal shows (the design's `modalMode`). Drives the title/sub,
/// the create-vs-delete body visibility, and the confirm button face.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModalMode {
    /// Scaffold a new widget — the Name/Kind/Register form.
    #[default]
    Create,
    /// Confirm a destructive delete — the warning tile + body.
    Delete,
}

/// Tag on a trigger button (the create / delete invokers), carrying the mode it
/// opens the dialog in. [`apply_modal_mode`] reads the pressed trigger's mode and
/// swaps the dialog body before the C5-d open lifecycle runs.
#[derive(Component, Clone, Copy)]
pub struct ModalInvoker(pub ModalMode);

/// Tag on the S4 dialog (so the binary/fixture/driver can find it among roots).
#[derive(Component, Clone, Default)]
pub struct ModalDialog;

/// Tag on the dialog's title `Text` ([`apply_modal_mode`] rewrites it per mode).
#[derive(Component, Clone, Default)]
pub struct ModalTitleField;

/// Tag on the dialog's subtitle `Text` ([`apply_modal_mode`] rewrites it).
#[derive(Component, Clone, Default)]
pub struct ModalSubField;

/// Tag on the create-body container (shown in [`ModalMode::Create`], hidden in
/// delete). [`apply_modal_mode`] toggles its `Display` + `A11yHidden`.
#[derive(Component, Clone, Default)]
pub struct ModalCreateBody;

/// Tag on the delete-body container (shown in [`ModalMode::Delete`]).
#[derive(Component, Clone, Default)]
pub struct ModalDeleteBody;

/// Tag on the create-body Kind segmented track (so [`select_modal_kind`] restyles
/// only the modal's Kind group on a `SegmentedOption` press — the design's `mKind`
/// selection). The C2 `segmented` composite already carries the per-option
/// [`SegmentedOption`](crate::composites::SegmentedOption) index + the
/// [`set_segmented`](crate::composites::set_segmented) restyle.
#[derive(Component, Clone, Default)]
pub struct ModalKindTrack;

/// Tag on the confirm button (Create / Delete). [`apply_modal_mode`] swaps its bg +
/// glow + label tint + label text to match the mode (the design's `confirmStyle`).
#[derive(Component, Clone, Default)]
pub struct ModalConfirm;

/// Tag on the confirm button's label `Text` ([`apply_modal_mode`] rewrites it).
#[derive(Component, Clone, Default)]
pub struct ModalConfirmLabel;

/// Spawn the S4 modal screen into `world`: the **trigger group** (the two centered
/// trigger buttons "New widget" / "Delete" + the caption) under a window-sized
/// `#ModalRoot`, and a closed C5-d [`Dialog`] restyled to the design's modal card
/// (header → create body [Name input + Kind segmented + Register switch] / delete
/// body [warning tile + body] → footer [Esc kbd + Cancel + confirm]). The dialog is
/// a separate top-layer root; the two triggers `controls` it. Returns
/// `(create_invoker, dialog, delete_invoker)`.
///
/// The dialog is spawned imperatively (not a `screen_*` scene-fn) because the
/// invokers reference the dialog **entity**, which a scene cannot name until it is
/// spawned — the `spawn_overlay_menu` standalone-popover constraint. `WidgetsPlugin`
/// owns the open/close/focus-trap/Esc/restore + inert-background lifecycle (C5-d
/// `dialog.rs`); [`ModalPlugin`] adds only the create/delete body swap. The dialog
/// opens in [`ModalMode::Create`] by default (the design's `modalMode:'create'`).
pub fn spawn_modal(world: &mut World) -> (Entity, Entity, Entity) {
    // The styled dialog card (closed at rest). Spawned first so the triggers can
    // reference it.
    let dialog = build_modal_dialog(world);

    // The two trigger buttons (the create + delete invokers) + the caption, in a
    // centered column (the design's `min-height:100%; center; gap:20px` wrap).
    let create_trigger = build_modal_trigger(world, ModalMode::Create, dialog);
    let delete_trigger = build_modal_trigger(world, ModalMode::Delete, dialog);
    let triggers = world
        .spawn((
            Node,
            Name::new("#ModalTriggers"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .gap_px(12.0),
        ))
        .add_children(&[create_trigger, delete_trigger])
        .id();
    let caption = text_leaf(
        world,
        "#ModalCaption",
        "Dialogs trap focus, restore it on close, dim the backdrop, and respond to Esc.",
        geist(),
        12.0,
        400,
        ColorToken::TextDim,
        None,
    );
    world.entity_mut(caption).insert((
        TextAlign::Center,
        Style::default()
            .max_width(Sizing::Length(Length::px(340.0)))
            .flex_row()
            .justify_content(JustifyContent::Center),
    ));

    // The `#ModalRoot` (the router toggles it): a window-filling centered column
    // holding the trigger group + caption (the design's centering wrap, `gap:20px`,
    // `padding:40px`). Window-sized so the centering resolves against the viewport.
    let root = world
        .spawn((
            Node,
            Name::new("#ModalRoot"),
            Style::default()
                .flex_column()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .gap_px(20.0)
                .padding(40.0)
                .width(Sizing::Length(Length::percent(100.0)))
                .height(Sizing::Length(Length::percent(100.0))),
        ))
        .add_children(&[triggers, caption])
        .id();
    let _ = root;
    (create_trigger, dialog, delete_trigger)
}

/// One centered trigger button (the design's "New widget" / "Delete" — HTML 232,
/// values.md § 7.2 "Trigger buttons" height 40, radius 9, gap 8, padding `0 16px`).
/// The create trigger is accent-filled (`accent` bg, `text.on-accent`, a `+` icon,
/// `shadow.accent-button`); the delete trigger is danger-soft (`surface.danger-soft`
/// bg, 1px `border.danger`, `text.danger`, a trash icon). Both are real `Button`s
/// carrying `controls = [dialog]` (a `dialog_invoker`) + the [`ModalInvoker`] mode
/// tag, so a press opens the dialog (C5-d) after [`apply_modal_mode`] sets the body.
fn build_modal_trigger(world: &mut World, mode: ModalMode, dialog: Entity) -> Entity {
    use buiy_widgets::dialog::dialog_invoker;

    let (label, name, icon_d, icon_stroke, label_tok, bg_tok) = match mode {
        ModalMode::Create => (
            MODAL_INVOKER,
            "#ModalCreateTrigger",
            "M12 5v14M5 12h14",
            2.2,
            ColorToken::TextOnAccent,
            ColorToken::Accent,
        ),
        ModalMode::Delete => (
            MODAL_BG_BUTTON,
            "#ModalDeleteTrigger",
            "M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13h10l1-13",
            1.7,
            ColorToken::TextDanger,
            ColorToken::SurfaceDangerSoft,
        ),
    };

    let icon = icon_box(
        world,
        "#ModalTriggerIcon",
        icon_d,
        icon_stroke,
        16,
        label_tok,
    );
    let text = text_leaf(
        world,
        "#ModalTriggerLabel",
        label,
        geist(),
        13.0,
        600,
        label_tok,
        None,
    );

    // `dialog_invoker` supplies the full Button contract + `controls=[dialog]` + an
    // auto-label child; spawn a BARE controls button instead (no auto-label — the
    // § 4.1c double-label gotcha) and layer our own icon + label. The invoker's
    // `A11yLabel` is the accessible name (`label`).
    let mut e = world.spawn((
        dialog_invoker(label, dialog),
        ModalInvoker(mode),
        Name::new(name.to_string()),
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .justify_content(JustifyContent::Center)
            .gap_px(8.0)
            .height_px(40.0)
            .padding_edges(Edges::axis(16.0, 0.0)),
        Background { color: bg_tok },
    ));
    // `dialog_invoker` is `Button::new(label)` which injects a default-styled label
    // child; strip it so only our icon + token-tinted label render (the gallery
    // bare-button idiom). Re-author the border (accent trigger has none; delete
    // trigger has a 1px `border.danger`).
    match mode {
        ModalMode::Create => {
            e.insert((
                border_all_radius_only(9.0),
                // shadow.accent-button — `0 8px 20px -8px color.accent.glow`.
                BoxShadow(vec![Shadow {
                    color: ColorToken::AccentGlow,
                    offset_x: Length::px(0.0),
                    offset_y: Length::px(8.0),
                    blur: Length::px(20.0),
                    spread: Length::px(-8.0),
                    inset: false,
                }]),
            ));
        }
        ModalMode::Delete => {
            e.insert(border_all(ColorToken::BorderDanger, 9.0));
        }
    }
    let trigger = e.id();
    // Drop the `Button::new` auto-label child so only our icon + label show.
    strip_button_auto_label(world, trigger);
    world.entity_mut(trigger).add_children(&[icon, text]);
    trigger
}

/// Build the closed, parity-styled dialog card and return it. The `Dialog`
/// `#[require]` carries the modal a11y + `TopLayer::Modal` + `FocusScope::trap` +
/// `FocusReturn` + `CssVisibility::Hidden`; we override its `BoxModel`/`Background`/
/// `Border` so the dialog is a **full-window overlay** (transparent, centered),
/// holding the scrim + the 440px card (header / create+delete bodies / footer). All
/// focusables live inside the card → inside the dialog subtree → the C5-d trap
/// confines Tab to them.
fn build_modal_dialog(world: &mut World) -> Entity {
    use buiy_widgets::dialog::Dialog;

    // The 440px card pieces.
    let header = build_modal_header(world);
    let create_body = build_modal_create_body(world);
    let delete_body = build_modal_delete_body(world);
    let footer = build_modal_footer(world);

    let card = world
        .spawn((
            Node,
            Name::new("#ModalCard"),
            Style::default()
                .flex_column()
                .relative()
                .overflow_hidden()
                .width_px(440.0)
                .border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderStrong, 14.0),
            // shadow.modal — `0 30px 70px -20px rgba(0,0,0,.85)` (values.md § 2).
            BoxShadow(vec![Shadow {
                color: ColorToken::ShadowModal,
                offset_x: Length::px(0.0),
                offset_y: Length::px(30.0),
                blur: Length::px(70.0),
                spread: Length::px(-20.0),
                inset: false,
            }]),
        ))
        .add_children(&[header, create_body, delete_body, footer])
        .id();

    // The backdrop scrim: absolute inset:0, the `color.scrim` dim + a blur(2px)
    // backdrop-filter (B4 — the scrim is window-parent, so the full-screen blur
    // qualifies). `Pickable::IGNORE` so it never eats the card's clicks.
    let scrim = world
        .spawn((
            Node,
            Name::new("#ModalScrim"),
            Style::default().absolute().inset(Inset {
                top: Sizing::Length(Length::px(0.0)),
                right: Sizing::Length(Length::px(0.0)),
                bottom: Sizing::Length(Length::px(0.0)),
                left: Sizing::Length(Length::px(0.0)),
            }),
            Background {
                color: ColorToken::Scrim,
            },
            BackdropFilter(vec![FilterFn::Blur(Length::px(2.0))]),
            Pickable::IGNORE,
        ))
        .id();

    // The dialog root = the full-window overlay (transparent, centered). Override
    // the `#[require]` panel box so it fills the viewport + centers its children.
    // The overlay layout: full-window, centered flex, padded 24 (the design's
    // overlay inset). We override ONLY the three layout-input components
    // (`Display`/`FlexParams`/`BoxModel`) — NOT the whole `Style` bundle, which also
    // carries `Stacking` and would clobber the Dialog `#[require]`'s
    // `Stacking = TopLayer::Modal` (dropping the dialog out of the top-layer
    // activation deque the Escape/focus-trap handlers consult).
    let overlay = Style::default()
        .flex_row()
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .padding(24.0)
        .width(Sizing::Length(Length::percent(100.0)))
        .height(Sizing::Length(Length::percent(100.0)));
    world
        .spawn((
            Dialog,
            ModalDialog,
            Name::new("ModalDialog"),
            overlay.display,
            overlay.flex_params,
            overlay.box_model,
            // Transparent overlay (the scrim child supplies the dim) + no border.
            Background {
                color: ColorToken::Transparent,
            },
            Border::default(),
        ))
        .add_children(&[scrim, card])
        .id()
}

/// The dialog header (values.md § 7.2 Modal "Header"): `padding:18px 20px 14px`,
/// border-bottom 1px `border.subtle`, `gap:12px`, `align-items:flex-start`. A
/// title/sub column (`flex:1`) + a 28×28 close button (× icon, 1px `border.default`,
/// radius 7, bg `surface.inset`).
fn build_modal_header(world: &mut World) -> Entity {
    // Title (Geist 16 / 600 / -.16px LS, `text.primary`) — the `DialogTitle` label
    // source so the dialog's `labelled_by` resolves to it.
    let title = text_leaf(
        world,
        "#ModalTitle",
        MODAL_TITLE,
        geist(),
        16.0,
        600,
        ColorToken::TextPrimary,
        Some(-0.16),
    );
    world.entity_mut(title).insert((
        buiy_widgets::dialog::DialogTitle,
        ModalTitleField,
        A11yLabel(MODAL_TITLE.to_string()),
    ));
    // Subtitle (Geist 12.5 / 400 `text.muted`).
    let sub = text_leaf(
        world,
        "#ModalSub",
        MODAL_SUB,
        geist(),
        12.5,
        400,
        ColorToken::TextMuted,
        None,
    );
    world.entity_mut(sub).insert(ModalSubField);
    let title_col = world
        .spawn((
            Node,
            Name::new("#ModalTitleCol"),
            Style::default().flex_column().gap_px(3.0),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
        ))
        .add_children(&[title, sub])
        .id();

    // The 28×28 close button (× icon, the design's header `closeModal`). A
    // `DialogClose` button so its press closes + restores focus (C5-d).
    let close = build_modal_close_button(
        world,
        "#ModalCloseX",
        28.0,
        7.0,
        ColorToken::BorderDefault,
        "Close",
        Some(("M6 6l12 12M18 6 6 18", 1.7, 14, ColorToken::TextMuted)),
        None,
    );

    world
        .spawn((
            Node,
            Name::new("#ModalHeader"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .gap_px(12.0)
                .padding_edges(Edges {
                    top: Length::px(18.0),
                    right: Length::px(20.0),
                    bottom: Length::px(14.0),
                    left: Length::px(20.0),
                })
                .border_edges(Edges {
                    bottom: Length::px(1.0),
                    ..Default::default()
                }),
            Border {
                bottom: solid_side(ColorToken::BorderSubtle),
                ..Default::default()
            },
        ))
        .add_children(&[title_col, close])
        .id()
}

/// The CREATE body (values.md § 7.2 Modal "Create body"): `padding:18px 20px;
/// gap:16px`. A Name field (mono label + 38px input) + a Kind field (mono label +
/// the C2 `segmented` Button/Layout/Input) + a "Register globally" row (label col +
/// a buiy_widgets `Switch`). Tagged [`ModalCreateBody`] so [`apply_modal_mode`]
/// toggles its visibility. Shown by default (the design's `modalMode:'create'`).
fn build_modal_create_body(world: &mut World) -> Entity {
    let name_field = build_modal_name_field(world);
    let kind_field = build_modal_kind_field(world);
    let register_row = build_modal_register_row(world);

    world
        .spawn((
            Node,
            ModalCreateBody,
            Name::new("#ModalCreateBody"),
            Style::default()
                .flex_column()
                .gap_px(16.0)
                .padding_edges(Edges::axis(20.0, 18.0)),
        ))
        .add_children(&[name_field, kind_field, register_row])
        .id()
}

/// The Name field: a `gap:7px` column of the uppercase mono "NAME" label (Geist
/// Mono 11 / 500 / .88px LS `text.muted`) over a 38px single-line input (1px
/// `border.strong`, radius 8, bg `surface.inset`, mono 13.5 placeholder "my_widget").
fn build_modal_name_field(world: &mut World) -> Entity {
    let label = text_leaf(
        world,
        "#ModalNameLabel",
        "NAME",
        geist_mono(),
        11.0,
        500,
        ColorToken::TextMuted,
        Some(0.88),
    );
    // A real single-line field (focusable + editable), styled to the design's input.
    // Spawned via the `TextInput::single_line` BUNDLE constructor (not the
    // `spawn_scene` scene-fn) so `spawn_modal` stays ScenePlugin-free — the C5-d
    // modal acceptance harnesses (`PointerHarness` / `modal_a11y_app`) build it
    // without a `ScenePlugin`, the same constraint the old `children!`-authored
    // dialog satisfied.
    let field = world
        .spawn(buiy_widgets::TextInput::single_line("my_widget"))
        .id();
    world.entity_mut(field).insert((
        Name::new("#ModalNameInput"),
        A11yLabel("Name".to_string()),
        FontSize(13.5),
        geist_mono(),
        FontWeight(450),
        TextColor(ColorToken::TextPrimary),
        Background {
            color: ColorToken::SurfaceInset,
        },
        border_all(ColorToken::BorderStrong, 8.0),
        BoxModel {
            height: Sizing::Length(Length::px(38.0)),
            width: Sizing::Auto,
            padding: Edges::axis(12.0, 0.0),
            border: Edges::all(1.0),
            ..Default::default()
        },
    ));
    world
        .spawn((
            Node,
            Name::new("#ModalNameField"),
            Style::default().flex_column().gap_px(7.0),
        ))
        .add_children(&[label, field])
        .id()
}

/// The Kind field: the uppercase mono "KIND" label over the C2 `segmented`
/// composite (Button / Layout / Input, the design's `kinds`), `gap:7px`.
fn build_modal_kind_field(world: &mut World) -> Entity {
    let label = text_leaf(
        world,
        "#ModalKindLabel",
        "KIND",
        geist_mono(),
        11.0,
        500,
        ColorToken::TextMuted,
        Some(0.88),
    );
    let segmented = crate::composites::segmented(world, &["Button", "Layout", "Input"], 0);
    world
        .entity_mut(segmented)
        .insert((Name::new("#ModalKind"), ModalKindTrack));
    world
        .spawn((
            Node,
            Name::new("#ModalKindField"),
            Style::default().flex_column().gap_px(7.0),
        ))
        .add_children(&[label, segmented])
        .id()
}

/// The "Register globally" row (values.md § 7.2): `padding:12px 14px`, 1px
/// `border.default`, radius 9, bg `surface.inset`, space-between. A label column
/// ("Register globally" Geist 13/500 + "Expose in the widget registry" Geist
/// 11.5/400 `text.faint`) + a buiy_widgets `Switch` on the right.
fn build_modal_register_row(world: &mut World) -> Entity {
    let title = text_leaf(
        world,
        "#ModalRegisterTitle",
        "Register globally",
        geist(),
        13.0,
        500,
        ColorToken::TextPrimary,
        None,
    );
    let sub = text_leaf(
        world,
        "#ModalRegisterSub",
        "Expose in the widget registry",
        geist(),
        11.5,
        400,
        ColorToken::TextFaint,
        None,
    );
    let label_col = world
        .spawn((
            Node,
            Name::new("#ModalRegisterCol"),
            Style::default().flex_column().gap_px(2.0),
        ))
        .add_children(&[title, sub])
        .id();

    // The register switch — a buiy_widgets `Switch` (the focusable toggle inside the
    // trap). Default ON (the design's `mPublic:true`). `Switch::new` adds a visible
    // label `Text` beside the track; the design's switch is label-LESS (the
    // "Register globally" text is the row's own left column), so strip the widget's
    // visible label — the A11yLabel (accessible name) stays on the root.
    let switch = world
        .spawn((
            Switch::new("Register globally"),
            Name::new("#ModalRegisterSwitch"),
        ))
        .id();
    strip_switch_label(world, switch);
    // SEED (authored initial state, NOT a runtime writer): the modal's "Register
    // globally" switch defaults ON (the design's `mPublic:true`), set at spawn time
    // before the toggle leaf's drain runs — a seed-scene initial condition, so it
    // stays a direct write (D3/D10 govern RUNTIME writes only).
    if let Some(mut t) = world.get_mut::<A11yToggled>(switch) {
        t.0 = Toggled::True;
    }

    world
        .spawn((
            Node,
            Name::new("#ModalRegisterRow"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::SpaceBetween)
                .padding_edges(Edges::axis(14.0, 12.0))
                .border(1.0),
            Background {
                color: ColorToken::SurfaceInset,
            },
            border_all(ColorToken::BorderDefault, 9.0),
        ))
        .add_children(&[label_col, switch])
        .id()
}

/// The DELETE body (values.md § 7.2 Modal "Delete body"): `padding:20px; gap:14px;
/// align-items:flex-start`. A 38×38 warning tile (radius 9, bg `surface.danger`,
/// `text.danger`, a triangle-warning icon) + the body text (Geist 13.5 / 450
/// `text.secondary`, line-height 1.55). Tagged [`ModalDeleteBody`]; hidden by
/// default (the create body shows first).
fn build_modal_delete_body(world: &mut World) -> Entity {
    let warn_icon = icon_box(
        world,
        "#ModalWarnIcon",
        "M12 3 2 20h20zM12 10v4M12 17h.01",
        1.8,
        19,
        ColorToken::TextDanger,
    );
    let tile = world
        .spawn((
            Node,
            Name::new("#ModalWarnTile"),
            Style::default()
                .width_px(38.0)
                .height_px(38.0)
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center),
            FlexItem {
                shrink: 0.0,
                ..Default::default()
            },
            Background {
                color: ColorToken::SurfaceDanger,
            },
            Border {
                radius: Corners::all(Radius::circular(9.0)),
                ..Default::default()
            },
        ))
        .add_children(&[warn_icon])
        .id();

    let body = text_leaf(
        world,
        "#ModalDeleteText",
        MODAL_BODY,
        geist(),
        13.5,
        450,
        ColorToken::TextSecondary,
        None,
    );
    world.entity_mut(body).insert((
        buiy_widgets::dialog::DialogBody,
        A11yLabel(MODAL_BODY.to_string()),
        FlexItem {
            grow: 1.0,
            ..Default::default()
        },
    ));

    let delete_body = world
        .spawn((
            Node,
            ModalDeleteBody,
            Name::new("#ModalDeleteBody"),
            // The SHOWN layout (a flex-row warning + body). The body is hidden at
            // rest (create shows first); `set_body_visible` toggles it. We author
            // the shown layout here, then overwrite `Display` with `None` below —
            // authoring `.display(None).flex_row()` would have `.flex_row()` reset
            // display back to Flex (the `Style` builder sets `display` on each call).
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .gap_px(14.0)
                .padding(20.0),
            A11yHidden,
        ))
        .add_children(&[tile, body])
        .id();
    // Hidden at rest — overwrite the just-inserted `Display::Flex(Row)` with `None`.
    // `Display::None` zero-sizes the whole subtree (icon box included), so the
    // warning `Icon` no longer strays (finding M5 is fixed at the producer: a
    // zero-area box emits no coverage). `CssVisibility::Hidden` stays to paint-skip
    // the body TEXT — a text leaf's glyphs are positioned at their shaped layout,
    // not zeroed by a zero box, so the paint-skip marker is the correct hide for it.
    world
        .entity_mut(delete_body)
        .insert((Display::None, CssVisibility::Hidden));
    delete_body
}

/// The footer (values.md § 7.2 Modal "Footer"): `padding:13px 20px`, border-top 1px
/// `border.subtle`, bg `surface.inset`, bottom-corner radius `0 0 11px 11px`,
/// `justify-content:flex-end; gap:10px`. Holds an "Esc to close" kbd
/// (`margin-right:auto`), a Cancel button (`DialogClose`), and the confirm button
/// (Create accent / Delete danger — [`ModalConfirm`]).
fn build_modal_footer(world: &mut World) -> Entity {
    // The "Esc to close" kbd (Geist Mono 10 / 500 `text.dim`, 1px `border.default`,
    // radius 6, pad `4px 7px`). `margin-right:auto` pushes the buttons right.
    let esc_text = text_leaf(
        world,
        "#ModalEscGlyph",
        "Esc to close",
        geist_mono(),
        10.0,
        500,
        ColorToken::TextDim,
        None,
    );
    let esc_kbd = world
        .spawn((
            Node,
            Name::new("#ModalEscKbd"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .padding_edges(Edges::axis(7.0, 4.0))
                .border(1.0),
            FlexItem {
                shrink: 0.0,
                ..Default::default()
            },
            Background {
                color: ColorToken::SurfaceInset,
            },
            border_all(ColorToken::BorderDefault, 6.0),
            Pickable::IGNORE,
        ))
        .add_children(&[esc_text])
        .id();

    // The design's `margin-right:auto` on the kbd pushes the buttons to the right
    // edge; a `flex:1` spacer between the kbd and the buttons is the equivalent
    // (Buiy's `Length` has no `auto` margin keyword — the shell `spacer` idiom).
    let spacer = world
        .spawn((
            Node,
            Name::new("#ModalFooterSpacer"),
            Style::default(),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .id();

    // The Cancel button (height 36, 1px `border.strong`, radius 8, bg
    // `surface.card`, Geist 12.5/600 `text.secondary`). A `DialogClose` so it closes.
    let cancel = build_modal_close_button(
        world,
        "#ModalCancel",
        36.0,
        8.0,
        ColorToken::BorderStrong,
        "Cancel",
        None,
        Some(("Cancel", ColorToken::TextSecondary)),
    );
    world.entity_mut(cancel).insert(Background {
        color: ColorToken::SurfaceCard,
    });

    // The confirm button (height 36, radius 8, pad `0 16px`). Create = accent +
    // on-accent + glow; the mode swap restyles it for delete. A `DialogClose` so it
    // closes (the design's `confirmModal` also closes the modal).
    let confirm = build_modal_confirm_button(world);

    world
        .spawn((
            Node,
            Name::new("#ModalFooter"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(10.0)
                .padding_edges(Edges::axis(20.0, 13.0))
                .border_edges(Edges {
                    top: Length::px(1.0),
                    ..Default::default()
                }),
            Background {
                color: ColorToken::SurfaceInset,
            },
            Border {
                top: solid_side(ColorToken::BorderSubtle),
                // The asymmetric bottom-only radius `0 0 11px 11px` (values.md § 3):
                // the footer's top edge is flush against the divider, the bottom two
                // corners round to 11 (inside the card's 14 outer radius).
                radius: Corners {
                    top_left: Radius::ZERO,
                    top_right: Radius::ZERO,
                    bottom_right: Radius::circular(11.0),
                    bottom_left: Radius::circular(11.0),
                },
                ..Default::default()
            },
        ))
        .add_children(&[esc_kbd, spacer, cancel, confirm])
        .id()
}

/// The confirm button (the design's `confirmModal`): height 36, pad `0 16px`,
/// radius 8, create = `accent` bg + `text.on-accent` label + `shadow.accent-button`.
/// A bare `Button` + [`DialogClose`] (so a press closes the dialog) + [`ModalConfirm`]
/// (so `apply_modal_mode` swaps it for the delete face). The label leaf carries
/// [`ModalConfirmLabel`].
fn build_modal_confirm_button(world: &mut World) -> Entity {
    use buiy_widgets::dialog::DialogClose;
    let label = text_leaf(
        world,
        "#ModalConfirmLabel",
        "Create",
        geist(),
        12.5,
        600,
        ColorToken::TextOnAccent,
        None,
    );
    world.entity_mut(label).insert(ModalConfirmLabel);
    world
        .spawn((
            buiy::prelude::Button,
            DialogClose,
            ModalConfirm,
            A11yLabel("Create".to_string()),
            Name::new("#ModalConfirm"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .height_px(36.0)
                .padding_edges(Edges::axis(16.0, 0.0)),
            Background {
                color: ColorToken::Accent,
            },
            border_all_radius_only(8.0),
            BoxShadow(vec![Shadow {
                color: ColorToken::AccentGlow,
                offset_x: Length::px(0.0),
                offset_y: Length::px(8.0),
                blur: Length::px(20.0),
                spread: Length::px(-8.0),
                inset: false,
            }]),
        ))
        .add_children(&[label])
        .id()
}

/// A `DialogClose`-marked button used for the header × (a fixed square holding an
/// icon) and the footer Cancel (a fixed-height label button). `height` sizes both;
/// the header × is also `width = height` (a square) via `icon`; `radius` rounds it;
/// `border_tok` is the 1px border color. Exactly one of `icon`
/// (`(path, stroke, px, tok)` — a centered glyph) or `text` (`(label, tint_tok)` — a
/// centered Geist 12.5/600 label) is supplied. The button bg is `surface.inset`
/// (the caller overrides it for the Cancel card-bg). Carries `DialogClose` + an
/// explicit `A11yLabel`. `FlexItem.shrink = 0` so it never squishes in the row.
#[allow(clippy::too_many_arguments)]
fn build_modal_close_button(
    world: &mut World,
    name: &str,
    height: f32,
    radius: f32,
    border_tok: ColorToken,
    a11y_label: &str,
    icon: Option<(&str, f32, u16, ColorToken)>,
    text: Option<(&str, ColorToken)>,
) -> Entity {
    use buiy_widgets::dialog::DialogClose;

    // The header × is a square (width == height); the footer Cancel is a height-only
    // label button with horizontal padding (`0 14px`, the design's Cancel).
    let mut style = Style::default()
        .flex_row()
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .height_px(height)
        .border(1.0);
    let child = if let Some((path_d, stroke, px, color_tok)) = icon {
        style = style.width_px(height);
        icon_box(world, "#ModalCloseIcon", path_d, stroke, px, color_tok)
    } else {
        let (label, color_tok) = text.expect("a close button has either an icon or a text label");
        style = style.padding_edges(Edges::axis(14.0, 0.0));
        text_leaf(
            world,
            "#ModalCloseLabel",
            label,
            geist(),
            12.5,
            600,
            color_tok,
            None,
        )
    };

    world
        .spawn((
            buiy::prelude::Button,
            DialogClose,
            A11yLabel(a11y_label.to_string()),
            Name::new(name.to_string()),
            style,
            Background {
                color: ColorToken::SurfaceInset,
            },
            Border {
                top: solid_side(border_tok),
                right: solid_side(border_tok),
                bottom: solid_side(border_tok),
                left: solid_side(border_tok),
                radius: Corners::all(Radius::circular(radius)),
            },
            FlexItem {
                shrink: 0.0,
                ..Default::default()
            },
        ))
        .add_children(&[child])
        .id()
}

/// A radius-only `Border` (no painted sides) — the accent triggers/confirm have a
/// rounded edge but no 1px stroke.
fn border_all_radius_only(radius: f32) -> Border {
    Border {
        radius: Corners::all(Radius::circular(radius)),
        ..Default::default()
    }
}

/// Drop a `Button::new`-injected default-styled auto-label child from `button` (the
/// gallery bare-button idiom — the § 4.1c double-label gotcha): the trigger/confirm
/// buttons carry our own icon + token-tinted label, so the widget's default label
/// child must not also render. Despawns every `Text`-bearing child the button
/// auto-spawned (the buttons have no other authored children at this point).
fn strip_button_auto_label(world: &mut World, button: Entity) {
    let label_children: Vec<Entity> = world
        .get::<Children>(button)
        .into_iter()
        .flat_map(|c| c.iter().copied().collect::<Vec<Entity>>())
        .filter(|&c| world.get::<Text>(c).is_some())
        .collect();
    for child in label_children {
        world.entity_mut(child).despawn();
    }
}

/// Drop a `Switch::new`-injected visible label `Text` child from `switch` (its
/// direct `Text`-bearing child — the track is a `SwitchTrack` with no `Text`). The
/// design's register-row switch is label-LESS (the row's left column already labels
/// it), so only the track + thumb should render; the accessible name stays on the
/// root's `A11yLabel`.
fn strip_switch_label(world: &mut World, switch: Entity) {
    let label_children: Vec<Entity> = world
        .get::<Children>(switch)
        .into_iter()
        .flat_map(|c| c.iter().copied().collect::<Vec<Entity>>())
        .filter(|&c| world.get::<Text>(c).is_some())
        .collect();
    for child in label_children {
        world.entity_mut(child).despawn();
    }
}

/// Startup system for the S4 binary: a camera, then the modal screen.
pub fn setup_modal(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        spawn_modal(world);
    });
}

// --- ModalPlugin — the create/delete body swap (the design's `modalMode`) -----

/// Staged modal-mode request: the mode the most-recently-pressed trigger this frame
/// opens the dialog in. [`collect_modal_mode`] writes it (an ordinary
/// `MessageReader<OnPress>` system); [`apply_modal_mode`] consumes + clears it. The
/// collect/apply split is the C8 intent pattern (read `OnPress` once in a normal
/// system, mutate the tree in the exclusive applier).
#[derive(Resource, Default)]
pub struct PendingModalMode(pub Option<ModalMode>);

/// Staged Kind selection: the `SegmentedOption` button pressed inside the modal's
/// Kind track this frame, if any (the design's `mKind` choice). [`collect_modal_mode`]
/// stages it; [`select_modal_kind`] consumes it + restyles the track. Same
/// collect/apply split (read `OnPress` once; `set_segmented` is `&mut World`).
#[derive(Resource, Default)]
pub struct PendingModalKind(pub Option<Entity>);

/// The S4 app logic: swap the dialog body (create ↔ delete) when a trigger fires.
/// The trigger's `OnPress` ALSO rides the C5-d `open_dialog_on_invoker_press`
/// consumer (both triggers `controls` the dialog), so the dialog opens; this plugin
/// sets WHICH body shows + the title/sub/confirm face for the pressed trigger's
/// mode. Runs `.after(BuiySet::Input)` (the C8 intent ordering) `.before(A11yUpdate)`
/// so the body visibility + the inert-prune land in the same a11y rebuild.
pub struct ModalPlugin;

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingModalMode>()
            .init_resource::<PendingModalKind>()
            .add_systems(
                Update,
                (collect_modal_mode, apply_modal_mode, select_modal_kind)
                    .chain()
                    .after(BuiySet::Input)
                    .before(BuiySet::A11yUpdate),
            );
    }
}

/// Stage the trigger mode + the Kind option pressed this frame (last wins). An
/// ordinary `MessageReader<OnPress>` system — its own cursor sees every `OnPress`,
/// so the C5-d open consumer + the segmented composite read the same messages
/// independently. A pressed `SegmentedOption` is staged only when it is a child of
/// the modal's [`ModalKindTrack`] (the create-form's Kind group, not some other
/// segmented).
pub fn collect_modal_mode(
    mut reader: MessageReader<OnPress>,
    invokers: Query<&ModalInvoker>,
    kind_options: Query<&ChildOf, With<crate::composites::SegmentedOption>>,
    kind_tracks: Query<(), With<ModalKindTrack>>,
    mut pending: ResMut<PendingModalMode>,
    mut pending_kind: ResMut<PendingModalKind>,
) {
    for OnPress(e) in reader.read() {
        if let Ok(inv) = invokers.get(*e) {
            pending.0 = Some(inv.0);
        }
        // A Kind option press: stage it only when its parent track is the modal's.
        if let Ok(parent) = kind_options.get(*e)
            && kind_tracks.get(parent.parent()).is_ok()
        {
            pending_kind.0 = Some(*e);
        }
    }
}

/// Consume the staged [`PendingModalMode`]: reconfigure the dialog for that mode
/// (title/sub, create/delete body visibility, confirm face) and clear the staging.
/// An exclusive system (it walks several markers across the dialog subtree).
pub fn apply_modal_mode(world: &mut World) {
    let Some(mode) = world.resource_mut::<PendingModalMode>().0.take() else {
        return;
    };
    set_modal_mode(world, mode);
}

/// Consume the staged [`PendingModalKind`]: restyle the modal's Kind segmented
/// group so the pressed option is the new selected pill (the design's `mKind`).
/// Resolves the pressed option's `SegmentedOption` index + its parent track, then
/// rides the C2 [`set_segmented`](crate::composites::set_segmented) restyle. An
/// exclusive system (`set_segmented` is `&mut World`).
pub fn select_modal_kind(world: &mut World) {
    let Some(option) = world.resource_mut::<PendingModalKind>().0.take() else {
        return;
    };
    let Some(idx) = world
        .get::<crate::composites::SegmentedOption>(option)
        .map(|o| o.0)
    else {
        return;
    };
    let Some(track) = world
        .get::<ChildOf>(option)
        .map(|c| c.parent())
        .filter(|&t| world.get::<ModalKindTrack>(t).is_some())
    else {
        return;
    };
    crate::composites::set_segmented(world, track, idx);
}

/// Apply [`ModalMode`] to the dialog subtree: title/sub text, create/delete body
/// visibility, and the confirm button face. Idempotent — `pub` so the capture/test
/// paths can force a mode without a trigger press.
pub fn set_modal_mode(world: &mut World, mode: ModalMode) {
    let is_create = mode == ModalMode::Create;
    let (title, sub) = if is_create {
        (MODAL_TITLE, MODAL_SUB)
    } else {
        (MODAL_TITLE_DELETE, MODAL_SUB_DELETE)
    };

    // Title + sub text.
    if let Some(e) = find_single::<ModalTitleField>(world) {
        if let Some(mut t) = world.get_mut::<Text>(e) {
            t.0 = title.to_string();
        }
        if let Some(mut a) = world.get_mut::<A11yLabel>(e) {
            a.0 = title.to_string();
        }
    }
    if let Some(e) = find_single::<ModalSubField>(world)
        && let Some(mut t) = world.get_mut::<Text>(e)
    {
        t.0 = sub.to_string();
    }

    // Body visibility — the shown body is `flex` + a11y-visible; the hidden one is
    // `Display::None` + `A11yHidden` (so its (empty/absent) focus surface + nodes
    // drop from the trap + the tree).
    let create_body = find_single::<ModalCreateBody>(world);
    let delete_body = find_single::<ModalDeleteBody>(world);
    if let Some(cb) = create_body {
        set_body_visible(world, cb, is_create, Display::flex_column());
    }
    if let Some(db) = delete_body {
        set_body_visible(world, db, !is_create, Display::flex_row());
    }

    // Confirm button face (the design's `confirmStyle` / `confirmLabel`).
    if let Some(confirm) = find_single::<ModalConfirm>(world) {
        let (bg, glow, label_tok, label) = if is_create {
            (
                ColorToken::Accent,
                ColorToken::AccentGlow,
                ColorToken::TextOnAccent,
                "Create",
            )
        } else {
            (
                ColorToken::SurfaceDangerStrong,
                ColorToken::ShadowDangerButton,
                ColorToken::White,
                "Delete",
            )
        };
        if let Some(mut b) = world.get_mut::<Background>(confirm) {
            b.color = bg;
        }
        if let Some(mut s) = world.get_mut::<BoxShadow>(confirm)
            && let Some(term) = s.0.first_mut()
        {
            term.color = glow;
        }
        if let Some(mut a) = world.get_mut::<A11yLabel>(confirm) {
            a.0 = label.to_string();
        }
        if let Some(lbl) = find_single::<ModalConfirmLabel>(world) {
            if let Some(mut t) = world.get_mut::<Text>(lbl) {
                t.0 = label.to_string();
            }
            if let Some(mut c) = world.get_mut::<TextColor>(lbl) {
                c.0 = label_tok;
            }
        }
    }
}

/// Show/hide a modal body: `visible` → restore its `Display` + clear `A11yHidden` +
/// `CssVisibility::Visible`; else `Display::None` + `A11yHidden` +
/// `CssVisibility::Hidden`. The `Display::None` collapses it out of layout (the whole
/// subtree zero-sizes, icon box included) + the `A11yHidden` prunes it from the a11y
/// tree. `Icon` glyphs no longer stray on a collapsed body (finding M5 fixed at the
/// producer — a zero-area box emits no coverage); the `CssVisibility::Hidden` stays to
/// paint-skip the body TEXT, whose glyphs are positioned at their shaped layout and
/// are not zeroed by a zero box.
fn set_body_visible(world: &mut World, body: Entity, visible: bool, shown_display: Display) {
    let mut e = world.entity_mut(body);
    if visible {
        e.insert(shown_display)
            .insert(CssVisibility::Visible)
            .remove::<A11yHidden>();
    } else {
        e.insert(Display::None)
            .insert(CssVisibility::Hidden)
            .insert(A11yHidden);
    }
}

// ###########################################################################
// S5 — Controls showcase (parity Wave C3). The design's 2-column controls grid
// (HTML 303–395): a `max-width:880` wrap holding a `1fr 1fr` CSS grid of five
// cards — Switch (3 toggles), Slider+radius preview, Segmented+Stepper,
// Meter+Run-build, and a full-width (`grid-column:1/-1`) Disclosure accordion.
//
// The widgets keep their real a11y bundles (the bare `Switch`/`Slider`/
// `Disclosure` markers materialize the full `#[require]` contract — role, value/
// toggle/expand state, focus, name) but carry CUSTOM children (the design's exact
// track/thumb/chevron pixels) instead of the widget defaults — the `append_row`
// todo-checkbox precedent (a bare `Checkbox` with our round-box children, no
// `CheckboxMark`). Showcase-local systems ([`ShowcasePlugin`]) drive the visual
// from the a11y state (`Changed<A11yToggled>`/`Changed<A11yValue>`/
// `Changed<A11yExpanded>`), so pointer / keyboard / AT all converge on the one
// state the widget owns and these systems repaint. Every value comes from
// `docs/specs/2026-06-25-widget-catalog-values.md` (§ 4 type, § 2 shadows, § 3
// radii, § 7.2 Showcase, § 8 gradients).
// ###########################################################################

/// The S5 slider's accessible name (the driver addresses it by role+name). The
/// design's slider drives a corner radius, so it is named "Radius".
pub const SHOWCASE_SLIDER: &str = "Radius";
/// An S5 switch's accessible name the driver addresses by role+name. The three
/// switches share roles, so each carries a distinct name (the design's `Wireframe
/// mode` / `Snap to grid` / `Reduced motion`). This points at `Snap to grid`, which
/// starts **off** (`sw.snap:false`, HTML 643) — so the driver's `click` acceptance
/// observes a real off→on toggle (the first switch starts on).
pub const SHOWCASE_SWITCH: &str = "Snap to grid";
/// The S5 disclosure's accessible name the driver addresses by role+name. The
/// design's three items each carry a distinct name; this points at the SECOND item
/// (`Theme tokens`), which starts **collapsed** (`disc.b:false`, HTML 640) — so the
/// driver's `expand` acceptance observes a real collapsed→open transition (the
/// first item starts open).
pub const SHOWCASE_DISCLOSURE: &str = "Theme tokens";

/// The S5 slider's initial value / range / step. The design's `radius` starts at
/// 14 over `[0, 40]`, stepping by 1 (HTML 499; the slider drives the preview
/// square's corner radius `{radius}px`).
pub const SHOWCASE_SLIDER_NOW: f64 = 14.0;
/// The S5 slider's minimum.
pub const SHOWCASE_SLIDER_MIN: f64 = 0.0;
/// The S5 slider's maximum (the design caps the preview radius at 40px).
pub const SHOWCASE_SLIDER_MAX: f64 = 40.0;
/// The S5 slider's step.
pub const SHOWCASE_SLIDER_STEP: f64 = 1.0;

/// The three switch rows (the design's `SW` array, JS 631): `(name, description,
/// initial-on)`. The default-on `Wireframe mode` mirrors `sw.wireframe:true`
/// (HTML 642); the other two start off.
const SHOWCASE_SWITCHES: &[(&str, &str, bool)] = &[
    ("Wireframe mode", "Outline every node", true),
    ("Snap to grid", "8px layout grid", false),
    ("Reduced motion", "Disable transitions", false),
];

/// The segmented options (the design's `SEG`, JS 643) + the default selection
/// (`compact`, index 1). The C2 [`segmented`](crate::composites::segmented)
/// composite renders + restyles them.
const SHOWCASE_SEGMENTS: &[&str] = &["Cozy", "Compact", "Dense"];
/// The default-selected segment index (`compact`).
const SHOWCASE_SEG_DEFAULT: usize = 1;

/// The stepper's initial count (the design's `count:3`, HTML 499 → "03").
const SHOWCASE_STEPPER_NOW: i32 = 3;

/// The meter's initial fill fraction (the design's `progress:64` → 64%, HTML 499).
const SHOWCASE_METER_NOW: f32 = 0.64;

/// The three disclosure accordion items (the design's `DISC`, JS 646): `(title,
/// tag, body, initial-open)`. The first starts open (`disc.a:true`, HTML 640).
const SHOWCASE_DISCLOSURES: &[(&str, &str, &str, bool)] = &[
    (
        "Layout & flex",
        "4 props",
        "Direction, gap, padding, align and justify map straight onto buiy's Stack and Row primitives — no flexbox ceremony.",
        true,
    ),
    (
        "Theme tokens",
        "6 props",
        "Surfaces, ink ladder, accent, radius, border and shadow resolve from a single Theme resource, swappable at runtime.",
        false,
    ),
    (
        "Accessibility",
        "a11y",
        "Roles, names and focus order derive from the widget tree; the focus-trap and roving tabindex ship in core.",
        false,
    ),
];

// ---------------------------------------------------------------------------
// Showcase markers (composition-level — NOT widget primitives)
// ---------------------------------------------------------------------------

/// Tag on the controls **grid** container (the `1fr 1fr` CSS grid). The single
/// entity the display-list acceptance + the screen tree query by; the per-card
/// border bands ride its children.
#[derive(Component, Clone, Default)]
pub struct ShowcaseCard;

/// Marks the slider's **preview square** (the 88×88 gradient box whose corner
/// radius the slider value drives live — `radius = now px`, HTML 331). Carries the
/// `shadow.slider-preview` glow. [`drive_showcase_slider`] rewrites its
/// `Border.radius` from the slider's `A11yValue.now`.
#[derive(Component, Clone, Default)]
pub struct ShowcasePreview;

/// Marks the "Slider · radius" live value `Text` ("Npx", accent). Driven by
/// [`drive_showcase_slider`] from the slider value.
#[derive(Component, Clone, Default)]
pub struct ShowcaseRadiusLabel;

/// Marks a showcase switch's custom **track** box (the 40×23 pill). Its fill is
/// swapped (accent on / `surface.raised-alt` off) on toggle by
/// [`drive_showcase_switches`].
#[derive(Component, Clone, Default)]
pub struct ShowcaseSwitchTrack;

/// Marks a showcase switch's custom **thumb** box (the 17×17 white knob whose
/// `Translate` x [`drive_showcase_switches`] slides `3px ↔ 20px`).
#[derive(Component, Clone, Default)]
pub struct ShowcaseSwitchThumb;

/// Marks the showcase slider's custom **rail** (the 6px track). The driver finds
/// the fill + thumb among its descendants.
#[derive(Component, Clone, Default)]
pub struct ShowcaseSliderRail;

/// Marks the showcase slider's filled portion (the accent bar whose width
/// [`drive_showcase_slider`] resizes from `A11yValue.now`).
#[derive(Component, Clone, Default)]
pub struct ShowcaseSliderFill;

/// Marks the showcase slider's custom **thumb** (the 15px white knob whose
/// `Translate` x [`drive_showcase_slider`] slides along the rail).
#[derive(Component, Clone, Default)]
pub struct ShowcaseSliderThumb;

/// Tag on the showcase stepper composite root, carrying its live count (so the
/// `−`/`+` app logic can recompute + restyle via [`set_stepper`](crate::composites::set_stepper)).
#[derive(Component, Clone, Copy, Default)]
pub struct ShowcaseStepper {
    /// The current count (the design's `count`, rendered "0N").
    pub count: i32,
}

/// Tags the showcase **density** segmented track (Cozy/Compact/Dense), so the
/// inspector's `density` cell reads THIS segmented and not the modal screen's
/// KIND segmented (Button/Layout/Input) — both use the same `SegmentedOption`
/// marker, so an unscoped query could return the modal's selected "Button".
#[derive(Component, Clone, Default)]
pub struct ShowcaseDensitySegmented;

/// Marks the meter's live "N%" value `Text` (mono, `text.muted`). Driven by
/// [`tick_showcase_build`] as the build animates.
#[derive(Component, Clone, Default)]
pub struct ShowcaseMeterLabel;

/// Marks the "Run build" button. [`ShowcasePlugin`] animates the meter `0→100%`
/// when it fires.
#[derive(Component, Clone, Default)]
pub struct ShowcaseRunBuild;

/// Marks a disclosure's custom **chevron** icon box (rotated 90° on expand by
/// [`drive_showcase_disclosures`]).
#[derive(Component, Clone, Default)]
pub struct ShowcaseChevron;

/// Marks a disclosure's expandable **body** `Text` (`Display::None` collapsed →
/// `flex` expanded, by [`drive_showcase_disclosures`]).
#[derive(Component, Clone, Default)]
pub struct ShowcaseDiscBody;

/// The live meter handle (the fill entity [`set_meter`] re-targets) + the running
/// animation state. `run_showcase_build` sets it; the
/// per-frame [`tick_showcase_build`] advances the "N%" label as the tween runs.
#[derive(Resource, Default)]
pub struct ShowcaseBuild {
    /// The meter fill entity (the `set_meter` target), `None` until the screen
    /// mounts.
    pub fill: Option<Entity>,
    /// The build progress fraction `[0, 1]` the label reflects. While
    /// `building`, [`tick_showcase_build`] ramps it to 1.0.
    pub progress: f32,
    /// Whether a build is in flight (the "Run build" button started it). The label
    /// ramp runs while true; it clears (and shows a toast) on reach-100%.
    pub building: bool,
}

/// The build-ramp duration (matches the meter fill tween, ~0.3s — values.md § 5.1
/// `width .3s`; the label counts up over the same window).
const SHOWCASE_BUILD_SECS: f32 = 0.3;

// ---------------------------------------------------------------------------
// Shared paint sources (the test ↔ screen agreement — the display-list +
// GPU acceptances spell the SAME shadow/border the screen authors)
// ---------------------------------------------------------------------------

/// The slider **preview square**'s drop shadow (the ONLY box-shadow on the design's
/// flat controls screen): `shadow.slider-preview` `0 10px 26px -10px acglow`
/// (values.md § 2; HTML 331). The single source the screen-fn + the display-list /
/// GPU acceptances share (the term-count + accent-glow guarantee).
pub fn showcase_preview_shadow() -> buiy_core::render::components::BoxShadow {
    use buiy_core::render::components::{BoxShadow, Shadow};
    BoxShadow(vec![Shadow {
        color: ColorToken::AccentGlow,
        offset_x: Length::px(0.0),
        offset_y: Length::px(10.0),
        blur: Length::px(26.0),
        spread: Length::px(-10.0),
        inset: false,
    }])
}

/// The showcase **card** border PAINT (the four flat-card sides + radius 12): 1px
/// `border.default`, radius 12 (values.md § 7.2 "Card"). The single source the
/// screen-fn + the display-list / GPU acceptances share (the card emits a border
/// band; the design's controls cards are flat — bordered, NOT shadowed).
pub fn showcase_card_border() -> Border {
    border_all(ColorToken::BorderDefault, 12.0)
}

// ---------------------------------------------------------------------------
// Shared local helpers (the design's slider/switch geometry)
// ---------------------------------------------------------------------------

/// The slider track width (logical px) the custom showcase rail spans. The thumb's
/// left edge travels `[0, WIDTH − THUMB]`; the fill width is the same fraction.
const SHOWCASE_SLIDER_WIDTH: f32 = 248.0;
/// The custom showcase slider thumb diameter (values.md § 7.2: 15×15).
const SHOWCASE_SLIDER_THUMB: f32 = 15.0;
/// The switch thumb's off / on left inset (values.md § 7.2: `left 3px ↔ 20px`).
const SHOWCASE_SWITCH_OFF_X: f32 = 3.0;
const SHOWCASE_SWITCH_ON_X: f32 = 20.0;

/// The slider fill width (px) for value `now` over `[min, max]`: the filled
/// fraction of the rail (the design's `radiusPct` width). Clamped to `[0, WIDTH]`.
fn showcase_slider_fill_px(now: f64, min: f64, max: f64) -> f32 {
    let span = max - min;
    if span <= 0.0 {
        return 0.0;
    }
    let frac = ((now - min) / span).clamp(0.0, 1.0) as f32;
    frac * SHOWCASE_SLIDER_WIDTH
}

/// The slider thumb's left-edge offset (px) for value `now`: the fraction mapped
/// onto `[0, WIDTH − THUMB]` so the 15px knob stays inside the rail at both ends.
fn showcase_slider_thumb_px(now: f64, min: f64, max: f64) -> f32 {
    let span = max - min;
    if span <= 0.0 {
        return 0.0;
    }
    let frac = ((now - min) / span).clamp(0.0, 1.0) as f32;
    frac * (SHOWCASE_SLIDER_WIDTH - SHOWCASE_SLIDER_THUMB)
}

/// One uppercase mono **section label** (values.md § 4 "Showcase — card section
/// labels": Geist Mono 10 / 500 / .12em (= 1.20px @ 10px) `text.dim`). `mb` is the
/// design's section-label margin-bottom (14px switch/slider, 12px else).
fn showcase_section_label(world: &mut World, name: &str, text: &str, mb: f32) -> Entity {
    let label = text_leaf(
        world,
        name,
        text,
        geist_mono(),
        10.0,
        500,
        ColorToken::TextDim,
        Some(1.20),
    );
    world.entity_mut(label).insert(BoxModel {
        margin: Edges {
            bottom: Length::px(mb),
            ..Default::default()
        },
        ..Default::default()
    });
    label
}

/// One showcase **card** (values.md § 7.2 "Card"): 1px `border.default`, radius 12,
/// bg `surface.card`, `padding:16`, a flex-column. `children` are added after.
fn showcase_card_box(world: &mut World, name: &str) -> Entity {
    world
        .spawn((
            Node,
            Name::new(name.to_string()),
            Style::default().flex_column().padding(16.0).border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderDefault, 12.0),
        ))
        .id()
}

// ---------------------------------------------------------------------------
// The screen tree (the design's 2-column controls grid)
// ---------------------------------------------------------------------------

/// Build the S5 Controls showcase into `world` and return the `#ShowcaseScreen`
/// root. The design's controls grid (values.md § 7.2 Showcase, HTML 303–395): a
/// centered `max-width:880` wrap holding a `1fr 1fr` CSS grid of five cards
/// (Switch / Slider+preview / Segmented+Stepper / Meter+Run-build / full-width
/// Disclosure). The meter fill entity is recorded in [`ShowcaseBuild`] so the
/// "Run build" button can animate it; both the binary and the fixtures build this
/// same frame (the "example IS the fixture" discipline). [`ShowcasePlugin`] wires
/// the behavior.
pub fn spawn_showcase(world: &mut World) -> Entity {
    let switch_card = build_showcase_switch_card(world);
    let slider_card = build_showcase_slider_card(world);
    let seg_step_card = build_showcase_seg_stepper_card(world);
    let meter_card = build_showcase_meter_card(world);
    let disc_card = build_showcase_disclosure_card(world);

    // The `1fr 1fr` CSS grid (values.md § 7.2 "Grid": `grid-template-columns:1fr
    // 1fr; gap:16`). The disclosure card spans both columns (`grid-column:1/-1`).
    let grid = world
        .spawn((
            Node,
            ShowcaseCard,
            Name::new("#ShowcaseGrid"),
            Style::default()
                .grid()
                .grid_template_columns(vec![
                    TrackSize::Length(Length::Fr(1.0)),
                    TrackSize::Length(Length::Fr(1.0)),
                ])
                .grid_gap_px(16.0)
                .width(Sizing::Length(Length::percent(100.0))),
        ))
        .id();
    world
        .entity_mut(grid)
        .add_children(&[switch_card, slider_card, seg_step_card, meter_card]);
    // The disclosure card spans the full grid width (`grid-column:1 / -1`).
    world.entity_mut(disc_card).insert(GridItem {
        column: GridLine::StartEnd(1, -1),
        ..Default::default()
    });
    world.entity_mut(grid).add_child(disc_card);

    // The centered `max-width:880` wrap (the design's `margin:0 auto;
    // padding:28px 24px 56px`): `width:100%` so it FILLS a narrower content slot
    // (the unified shell's viewport is `≈752px` between the rail + inspector — the
    // 880 grid would overflow a fixed width), capped at 880 (`max-width`) and
    // centered (`align_self:center`) when the slot is wider — the CSS `max-width` +
    // `margin:0 auto` behavior, faithful at any container width. The `1fr 1fr` cards
    // shrink to fit, so the controls stay fully on-screen.
    let wrap = world
        .spawn((
            Node,
            Name::new("#ShowcaseWrap"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0)))
                .max_width(Sizing::Length(Length::px(880.0)))
                .padding_edges(Edges {
                    top: Length::px(28.0),
                    right: Length::px(24.0),
                    bottom: Length::px(56.0),
                    left: Length::px(24.0),
                }),
            FlexItem {
                align_self: Some(AlignItems::Center),
                ..Default::default()
            },
        ))
        .add_child(grid)
        .id();

    world
        .spawn((
            Node,
            Name::new("#ShowcaseScreen"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0))),
        ))
        .add_child(wrap)
        .id()
}

/// The Switch card (values.md § 7.2 Showcase, HTML 308–322): a "SWITCH" section
/// label over a `gap:13` column of three switch rows. Each row is `[label-col,
/// switch]` space-between: the label col is `(name Geist 13 `text.primary`, desc
/// Geist 11 #6f7783)`, the switch is a real focusable `Switch` restyled to the
/// design's 40×23 accent-on track + 17×17 white thumb.
fn build_showcase_switch_card(world: &mut World) -> Entity {
    let card = showcase_card_box(world, "#ShowcaseSwitchCard");
    let label = showcase_section_label(world, "#ShowcaseSwitchLabel", "SWITCH", 14.0);

    let rows: Vec<Entity> = SHOWCASE_SWITCHES
        .iter()
        .enumerate()
        .map(|(i, &(name, desc, on))| build_showcase_switch_row(world, i, name, desc, on))
        .collect();
    let list = world
        .spawn((
            Node,
            Name::new("#ShowcaseSwitchList"),
            Style::default().flex_column().gap_px(13.0),
        ))
        .id();
    world.entity_mut(list).add_children(&rows);

    world.entity_mut(card).add_children(&[label, list]);
    card
}

/// One switch row: `[label-col, switch]` space-between. The label col stacks the
/// `name` (Geist 13 / 500 `text.primary`) + `desc` (Geist 11 / 400 #6f7783 =
/// `text.faint`-ish; the design uses a slightly cooler tint — author `text.faint`).
/// The switch is a bare `Switch` widget (the focusable toggle + a11y) restyled to
/// the design's pixels; [`drive_showcase_switches`] recolors the track + slides the
/// thumb on toggle.
fn build_showcase_switch_row(
    world: &mut World,
    idx: usize,
    name: &str,
    desc: &str,
    on: bool,
) -> Entity {
    let title = text_leaf(
        world,
        "#ShowcaseSwitchName",
        name,
        geist(),
        13.0,
        500,
        ColorToken::TextPrimary,
        None,
    );
    let sub = text_leaf(
        world,
        "#ShowcaseSwitchDesc",
        desc,
        geist(),
        11.0,
        400,
        ColorToken::TextFaint,
        None,
    );
    let label_col = world
        .spawn((
            Node,
            Name::new("#ShowcaseSwitchCol"),
            Style::default().flex_column().gap_px(1.0),
        ))
        .add_children(&[title, sub])
        .id();

    let switch = build_showcase_switch(world, name, on);

    world
        .spawn((
            Node,
            // Indexed so the layout dump disambiguates the three rows when the screen
            // collapses to zero-boxes (a `Display::None` hidden screen).
            Name::new(format!("#ShowcaseSwitchRow-{idx}")),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::SpaceBetween)
                .gap_px(12.0),
        ))
        .add_children(&[label_col, switch])
        .id()
}

/// A design-styled switch: a bare `Switch` marker (the full a11y bundle — role,
/// binary toggle, focus, name) carrying CUSTOM children (our 40×23 track + 17×17
/// thumb, NO `SwitchTrack`/`SwitchThumb` markers so the widget's own visual is
/// inert), the `append_row` checkbox precedent. [`drive_showcase_switches`] reads
/// `Changed<A11yToggled>` and recolors the track (accent on / #2a2f37 off) +
/// slides the thumb (`left 3px ↔ 20px`). Seeded ON when `on`.
fn build_showcase_switch(world: &mut World, name: &str, on: bool) -> Entity {
    // The 17×17 white thumb (radius 99, `shadow.switch-thumb`), authored at the OFF
    // position (x = 3px via Translate); the driver slides it. `Pickable::IGNORE` so
    // a hit resolves to the switch root.
    let thumb = world
        .spawn((
            Node,
            ShowcaseSwitchThumb,
            Name::new("#ShowcaseSwitchThumb"),
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(3.0)),
                    ..Default::default()
                })
                .width_px(17.0)
                .height_px(17.0),
            // The design's white thumb (`#fff` — `color.misc.white`).
            Background {
                color: ColorToken::White,
            },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            // shadow.switch-thumb — `0 1px 3px rgba(0,0,0,.4)` (values.md § 2).
            BoxShadow(vec![Shadow {
                color: ColorToken::ShadowSwitchThumb,
                offset_x: Length::px(0.0),
                offset_y: Length::px(1.0),
                blur: Length::px(3.0),
                spread: Length::px(0.0),
                inset: false,
            }]),
            Translate(
                Length::px(if on {
                    SHOWCASE_SWITCH_ON_X
                } else {
                    SHOWCASE_SWITCH_OFF_X
                }),
                Length::px(0.0),
                Length::px(0.0),
            ),
            Pickable::IGNORE,
        ))
        .id();

    // The 40×23 track pill. `relative` so the absolute thumb anchors to it; the
    // thumb is centered vertically by the track's flex + slid by Translate x. The
    // fill is accent (on) / #2a2f37 (off, ≈ `surface.raised-alt`). Seeded for `on`.
    let track = world
        .spawn((
            Node,
            ShowcaseSwitchTrack,
            Name::new("#ShowcaseSwitchTrack"),
            Style::default()
                .relative()
                .width_px(40.0)
                .height_px(23.0)
                .flex_row()
                .align_items(AlignItems::Center),
            Background {
                color: if on {
                    ColorToken::Accent
                } else {
                    ColorToken::SurfaceRaisedAlt
                },
            },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .add_child(thumb)
        .id();

    let switch = world
        .spawn((
            Switch,
            A11yLabel(name.to_string()),
            Name::new("#ShowcaseSwitch"),
            Style::default().flex_row().align_items(AlignItems::Center),
        ))
        .add_child(track)
        .id();
    // SEED (authored initial state, NOT a runtime writer): a showcase switch authored
    // `on` starts checked, set at spawn time before the toggle leaf's drain runs — a
    // seed-scene initial condition, so it stays a direct write (D3/D10 govern RUNTIME
    // writes only).
    if on && let Some(mut t) = world.get_mut::<A11yToggled>(switch) {
        t.0 = Toggled::True;
    }
    switch
}

/// The Slider card (values.md § 7.2 Showcase, HTML 324–337): a "SLIDER · radius"
/// section label + a live "Npx" value (accent), a centered 88×88 gradient preview
/// square (radius = the slider value, `shadow.slider-preview`), and a custom 6px
/// slider track (fill `accent`, 15px white thumb). The slider value (0..40) drives
/// the preview square's corner radius LIVE.
fn build_showcase_slider_card(world: &mut World) -> Entity {
    let card = world
        .spawn((
            Node,
            Name::new("#ShowcaseSliderCard"),
            Style::default().flex_column().padding(16.0).border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderDefault, 12.0),
        ))
        .id();

    // The header row: "SLIDER · radius" label + a live "Npx" accent value.
    let label = text_leaf(
        world,
        "#ShowcaseSliderLabel",
        "SLIDER · RADIUS",
        geist_mono(),
        10.0,
        500,
        ColorToken::TextDim,
        Some(1.20),
    );
    let value = text_leaf(
        world,
        "#ShowcaseRadiusValue",
        &showcase_radius_text(SHOWCASE_SLIDER_NOW),
        geist_mono(),
        12.0,
        500,
        ColorToken::Accent,
        None,
    );
    world.entity_mut(value).insert(ShowcaseRadiusLabel);
    let header = world
        .spawn((
            Node,
            Name::new("#ShowcaseSliderHeader"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::SpaceBetween)
                .margin_edges(Edges {
                    bottom: Length::px(14.0),
                    ..Default::default()
                }),
        ))
        .add_children(&[label, value])
        .id();

    // The centered preview square (88×88, the 150deg accent gradient, radius =
    // the slider value, `shadow.slider-preview` glow). `drive_showcase_slider`
    // rewrites its `Border.radius` from the value.
    let preview = world
        .spawn((
            Node,
            ShowcasePreview,
            Name::new("#ShowcasePreview"),
            Style::default().width_px(88.0).height_px(88.0),
            // The 150deg accent gradient (`gradient.accent-150`, values.md § 8).
            BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                angle_deg: 150.0,
                stops: vec![
                    ColorStop {
                        color: ColorToken::Accent,
                        position: 0.0,
                    },
                    ColorStop {
                        color: ColorToken::AccentLighter,
                        position: 1.0,
                    },
                ],
            })]),
            Border {
                radius: Corners::all(Radius::circular(SHOWCASE_SLIDER_NOW as f32)),
                ..Default::default()
            },
            // shadow.slider-preview — the screen's only box-shadow (shared source).
            showcase_preview_shadow(),
            Pickable::IGNORE,
        ))
        .id();
    let preview_wrap = world
        .spawn((
            Node,
            Name::new("#ShowcasePreviewWrap"),
            Style::default()
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .padding_edges(Edges {
                    top: Length::px(6.0),
                    bottom: Length::px(16.0),
                    ..Default::default()
                }),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
        ))
        .add_child(preview)
        .id();

    let slider = build_showcase_slider(world);

    world
        .entity_mut(card)
        .add_children(&[header, preview_wrap, slider]);
    card
}

/// A design-styled slider: a bare `Slider` marker (the full a11y bundle — role,
/// valued range, orientation, focus, name) carrying CUSTOM children (our 6px
/// track + accent fill + 15px white thumb, NO `SliderTrack`/`SliderThumb` markers).
/// [`drive_showcase_slider`] reads `Changed<A11yValue>` and resizes the fill +
/// slides the thumb + drives the preview-square radius + the "Npx" label.
fn build_showcase_slider(world: &mut World) -> Entity {
    // The filled portion (accent, radius 99, height 6), authored at the resting
    // value's width. `relative`/`absolute` not needed — it is the first track child
    // and the thumb sits absolutely over it.
    let fill = world
        .spawn((
            Node,
            ShowcaseSliderFill,
            Name::new("#ShowcaseSliderFill"),
            Style::default()
                .absolute()
                .inset(Inset {
                    left: Sizing::Length(Length::px(0.0)),
                    top: Sizing::Length(Length::px(0.0)),
                    ..Default::default()
                })
                .width_px(showcase_slider_fill_px(
                    SHOWCASE_SLIDER_NOW,
                    SHOWCASE_SLIDER_MIN,
                    SHOWCASE_SLIDER_MAX,
                ))
                .height_px(6.0),
            Background {
                color: ColorToken::Accent,
            },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .id();

    // The 15px white thumb (radius 99, `shadow.slider-thumb`), authored at the
    // resting value's left offset (absolute over the rail, vertically centered).
    let thumb = world
        .spawn((
            Node,
            ShowcaseSliderThumb,
            Name::new("#ShowcaseSliderThumb"),
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(-4.5)),
                    ..Default::default()
                })
                .width_px(SHOWCASE_SLIDER_THUMB)
                .height_px(SHOWCASE_SLIDER_THUMB),
            // The design's `#f1f3f6` thumb (`color.text.primary` resolves to it).
            Background {
                color: ColorToken::TextPrimary,
            },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            // shadow.slider-thumb — `0 2px 6px rgba(0,0,0,.5)` (values.md § 2).
            BoxShadow(vec![Shadow {
                color: ColorToken::ShadowSliderThumb,
                offset_x: Length::px(0.0),
                offset_y: Length::px(2.0),
                blur: Length::px(6.0),
                spread: Length::px(0.0),
                inset: false,
            }]),
            Translate(
                Length::px(showcase_slider_thumb_px(
                    SHOWCASE_SLIDER_NOW,
                    SHOWCASE_SLIDER_MIN,
                    SHOWCASE_SLIDER_MAX,
                )),
                Length::px(0.0),
                Length::px(0.0),
            ),
            Pickable::IGNORE,
        ))
        .id();

    // The 6px rail (radius 99, bg `surface.raised-alt`/#1e2127), `relative` so the
    // absolute fill + thumb anchor to it. `Pickable::IGNORE` so a hit resolves to
    // the slider root.
    let track = world
        .spawn((
            Node,
            ShowcaseSliderRail,
            Name::new("#ShowcaseSliderTrack"),
            Style::default()
                .relative()
                .width_px(SHOWCASE_SLIDER_WIDTH)
                .height_px(6.0),
            Background {
                color: ColorToken::SurfaceRaisedAlt,
            },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .add_children(&[fill, thumb])
        .id();

    world
        .spawn((
            Slider,
            A11yLabel(SHOWCASE_SLIDER.to_string()),
            A11yValue {
                now: SHOWCASE_SLIDER_NOW,
                min: SHOWCASE_SLIDER_MIN,
                max: SHOWCASE_SLIDER_MAX,
                step: Some(SHOWCASE_SLIDER_STEP),
                jump: None,
                text: None,
            },
            A11yOrientation(Orientation::Horizontal),
            Name::new("#ShowcaseSlider"),
            Style::default().flex_row().align_items(AlignItems::Center),
        ))
        .add_child(track)
        .id()
}

/// The Segmented + Stepper card (values.md § 7.2 Showcase, HTML 339–361): a
/// `gap:16` column of two sections — "SEGMENTED" over the C2 segmented composite
/// (Cozy/Compact/Dense), and "STEPPER" over the C2 stepper composite (− count +).
fn build_showcase_seg_stepper_card(world: &mut World) -> Entity {
    let card = world
        .spawn((
            Node,
            Name::new("#ShowcaseSegStepCard"),
            Style::default()
                .flex_column()
                .gap_px(16.0)
                .padding(16.0)
                .border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderDefault, 12.0),
        ))
        .id();

    // Segmented section.
    let seg_label = showcase_section_label(world, "#ShowcaseSegLabel", "SEGMENTED", 12.0);
    let seg = crate::composites::segmented(world, SHOWCASE_SEGMENTS, SHOWCASE_SEG_DEFAULT);
    // Tag the track so the inspector's `density` cell reads THIS segmented (not
    // the modal's KIND segmented, which shares the `SegmentedOption` marker).
    world.entity_mut(seg).insert(ShowcaseDensitySegmented);
    let seg_section = world
        .spawn((
            Node,
            Name::new("#ShowcaseSegSection"),
            Style::default().flex_column(),
        ))
        .add_children(&[seg_label, seg])
        .id();

    // Stepper section.
    let step_label = showcase_section_label(world, "#ShowcaseStepLabel", "STEPPER", 12.0);
    let step = crate::composites::stepper(world, SHOWCASE_STEPPER_NOW);
    world.entity_mut(step).insert(ShowcaseStepper {
        count: SHOWCASE_STEPPER_NOW,
    });
    let step_section = world
        .spawn((
            Node,
            Name::new("#ShowcaseStepSection"),
            Style::default().flex_column(),
        ))
        .add_children(&[step_label, step])
        .id();

    world
        .entity_mut(card)
        .add_children(&[seg_section, step_section]);
    card
}

/// The Meter card (values.md § 7.2 Showcase, HTML 363–375): a "METER · build"
/// section label + a live "N%" value, the C2 meter composite (gradient fill), and a
/// full-width "Run build" button that animates the meter `0→100%`.
fn build_showcase_meter_card(world: &mut World) -> Entity {
    let card = world
        .spawn((
            Node,
            Name::new("#ShowcaseMeterCard"),
            Style::default().flex_column().padding(16.0).border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderDefault, 12.0),
        ))
        .id();

    let label = text_leaf(
        world,
        "#ShowcaseMeterLabel",
        "METER · BUILD",
        geist_mono(),
        10.0,
        500,
        ColorToken::TextDim,
        Some(1.20),
    );
    let value = text_leaf(
        world,
        "#ShowcaseMeterValue",
        &showcase_pct_text(SHOWCASE_METER_NOW),
        geist_mono(),
        12.0,
        500,
        ColorToken::TextMuted,
        None,
    );
    world.entity_mut(value).insert(ShowcaseMeterLabel);
    let header = world
        .spawn((
            Node,
            Name::new("#ShowcaseMeterHeader"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::SpaceBetween)
                .margin_edges(Edges {
                    bottom: Length::px(12.0),
                    ..Default::default()
                }),
        ))
        .add_children(&[label, value])
        .id();

    let (meter_track, meter_fill) =
        buiy_widgets::composites::meter(world, 280.0, SHOWCASE_METER_NOW);
    world.insert_resource(ShowcaseBuild {
        fill: Some(meter_fill),
        progress: SHOWCASE_METER_NOW,
        building: false,
    });

    // The "Run build" button (full-width, height 34, 1px border, radius 8, bg
    // `surface.raised-alt`, label Geist 12/600 `text.secondary`). A bare `Button`
    // (OnPress + a11y) holding a Geist label.
    let run_label = text_leaf(
        world,
        "#ShowcaseRunBuildLabel",
        "Run build",
        geist(),
        12.0,
        600,
        ColorToken::TextSecondary,
        None,
    );
    let run = world
        .spawn((
            buiy::prelude::Button,
            A11yLabel("Run build".to_string()),
            ShowcaseRunBuild,
            Name::new("#ShowcaseRunBuild"),
            Style::default()
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .width(Sizing::Length(Length::percent(100.0)))
                .height_px(34.0)
                .border(1.0)
                .margin_edges(Edges {
                    top: Length::px(12.0),
                    ..Default::default()
                }),
            Background {
                color: ColorToken::SurfaceRaisedAlt,
            },
            border_all(ColorToken::BorderStrong, 8.0),
        ))
        .add_child(run_label)
        .id();

    world
        .entity_mut(card)
        .add_children(&[header, meter_track, run]);
    card
}

/// The Disclosure card (values.md § 7.2 Showcase, HTML 377–391): a full-width
/// (`grid-column:1/-1`) card of three accordion items, each a real focusable
/// `Disclosure` (the role-Button + Expand capability + a11y) restyled to the
/// design's `[chevron, title, tag]` header + expandable body, with a 1px divider
/// between items (none on the last).
fn build_showcase_disclosure_card(world: &mut World) -> Entity {
    // The card uses the design's `padding:6px 16px` (HTML 378) — a tighter vertical
    // pad so the item rows supply their own `13px 0`.
    let card = world
        .spawn((
            Node,
            Name::new("#ShowcaseDiscCard"),
            Style::default()
                .flex_column()
                .padding_edges(Edges::axis(16.0, 6.0))
                .border(1.0),
            Background {
                color: ColorToken::SurfaceCard,
            },
            border_all(ColorToken::BorderDefault, 12.0),
        ))
        .id();

    let last = SHOWCASE_DISCLOSURES.len() - 1;
    let items: Vec<Entity> = SHOWCASE_DISCLOSURES
        .iter()
        .enumerate()
        .map(|(i, &(title, tag, body, open))| {
            build_showcase_disclosure_item(world, i, title, tag, body, open, i != last)
        })
        .collect();
    world.entity_mut(card).add_children(&items);
    card
}

/// One disclosure accordion item: a real `Disclosure` (a bare marker → the full
/// role-Button + Expand + focus + name contract) carrying CUSTOM children (the
/// `[chevron, title, tag]` header + the expandable body), NOT the widget's default
/// caret/label/panel. [`drive_showcase_disclosures`] rotates the chevron + toggles
/// the body on `Changed<A11yExpanded>`. `divider` adds the design's 1px bottom rule
/// (none on the last item).
fn build_showcase_disclosure_item(
    world: &mut World,
    idx: usize,
    title: &str,
    tag: &str,
    body: &str,
    open: bool,
    divider: bool,
) -> Entity {
    // The chevron icon (a right-pointing `>`; rotated 90° down when expanded). Tint
    // accent when open, `text.muted` when collapsed — seeded for `open`.
    let chevron = icon_box(
        world,
        "#ShowcaseChevron",
        "M9 5l7 7-7 7",
        1.9,
        16,
        if open {
            ColorToken::Accent
        } else {
            ColorToken::TextMuted
        },
    );
    world.entity_mut(chevron).insert((
        ShowcaseChevron,
        if open {
            Rotate(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
        } else {
            Rotate(Quat::IDENTITY)
        },
    ));

    let title_leaf = text_leaf(
        world,
        "#ShowcaseDiscTitle",
        title,
        geist(),
        13.5,
        500,
        ColorToken::TextPrimary,
        None,
    );
    world.entity_mut(title_leaf).insert(FlexItem {
        grow: 1.0,
        ..Default::default()
    });
    let tag_leaf = text_leaf(
        world,
        "#ShowcaseDiscTag",
        tag,
        geist_mono(),
        10.5,
        500,
        ColorToken::TextDim,
        None,
    );

    // The `Disclosure` header trigger: a bare `Disclosure` marker (the full
    // contract) holding our `[chevron, title, tag]` row. Overriding the widget's
    // `#[require]` box to the design's `width:100%; padding:13px 0`. Seeded expanded
    // when `open`.
    let header = world
        .spawn((
            Disclosure,
            A11yLabel(title.to_string()),
            Name::new("#ShowcaseDisclosure"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(10.0)
                .width(Sizing::Length(Length::percent(100.0)))
                .padding_edges(Edges::axis(0.0, 13.0)),
            Background {
                color: ColorToken::Transparent,
            },
            Border::default(),
        ))
        .add_children(&[chevron, title_leaf, tag_leaf])
        .id();
    if open && let Some(mut e) = world.get_mut::<A11yExpanded>(header) {
        e.0 = true;
    }

    // The expandable body (`padding:0 0 14px 26px`, Geist 12.5 / 400 `text.muted`,
    // line-height 1.6). Hidden (`Display::None`) when collapsed; the driver toggles
    // it. Seeded shown for `open`.
    let body_leaf = text_leaf(
        world,
        "#ShowcaseDiscBody",
        body,
        geist(),
        12.5,
        400,
        ColorToken::TextMuted,
        None,
    );
    world.entity_mut(body_leaf).insert((
        ShowcaseDiscBody,
        LineHeight::Scale(1.6),
        Style::default()
            .display(if open {
                Display::flex_column()
            } else {
                Display::None
            })
            .padding_edges(Edges {
                bottom: Length::px(14.0),
                left: Length::px(26.0),
                ..Default::default()
            }),
    ));

    // The item wrapper (a column of `[header, body]`) with the design's 1px bottom
    // divider (`#1c1f24` = `border.subtle-2`) between items.
    let mut style = Style::default().flex_column();
    if divider {
        style = style.border_edges(Edges {
            bottom: Length::px(1.0),
            ..Default::default()
        });
    }
    let mut item = world.spawn((
        Node,
        // Indexed so the layout dump disambiguates the three accordion items when the
        // screen collapses to zero-boxes (a `Display::None` hidden screen).
        Name::new(format!("#ShowcaseDiscItem-{idx}")),
        style,
    ));
    if divider {
        item.insert(Border {
            bottom: solid_side(ColorToken::BorderSubtle2),
            ..Default::default()
        });
    }
    let item = item.id();
    world.entity_mut(item).add_children(&[header, body_leaf]);
    item
}

/// The "Npx" radius value string (the design's `radiusLabel`).
fn showcase_radius_text(radius: f64) -> String {
    format!("{}px", radius.round() as i64)
}

/// The "N%" meter value string (the design's `progressLabel`).
fn showcase_pct_text(frac: f32) -> String {
    format!("{}%", (frac.clamp(0.0, 1.0) * 100.0).round() as i64)
}

/// Startup system for the S5 binary (legacy standalone): a camera, then the screen.
pub fn setup_showcase(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        spawn_showcase(world);
    });
}

// ---------------------------------------------------------------------------
// ShowcasePlugin — the S5 controls behavior (the retained-mode app logic)
// ---------------------------------------------------------------------------

/// Staged S5 interaction intents, read once per frame by the collectors and
/// consumed by [`apply_showcase_intents`]. Empty between frames.
#[derive(Resource, Default)]
pub struct ShowcaseIntents {
    /// A segmented option pressed this frame (the new selection target).
    pub select_segment: Option<Entity>,
    /// Whether the stepper `−` (`false`) / `+` (`true`) fired this frame.
    pub step: Option<bool>,
    /// Whether "Run build" fired this frame.
    pub run_build: bool,
}

/// The S5 controls app logic. Registers the visual drivers (switch / slider /
/// disclosure restyle from the a11y state) + the press collector + the exclusive
/// applier + the build-ramp tick. The state→visual drivers run
/// `.after(BuiySet::Input)` (so the same frame's toggle / value / expand lands) and
/// the build tick rides `BuiySet::Animate` (alongside the meter tween).
pub struct ShowcasePlugin;

impl Plugin for ShowcasePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShowcaseIntents>()
            .add_systems(
                Update,
                (
                    collect_showcase_press,
                    apply_showcase_intents,
                    drive_showcase_switches,
                    drive_showcase_slider,
                    drive_showcase_disclosures,
                )
                    .chain()
                    .after(BuiySet::Input)
                    .before(BuiySet::A11yUpdate),
            )
            .add_systems(Update, tick_showcase_build.in_set(BuiySet::Animate));
    }
}

/// Stage the segmented / stepper / run-build presses (an ordinary
/// `MessageReader<OnPress>` system — the C8 intent pattern: read `OnPress` once
/// here, mutate the tree in the exclusive applier). The segmented/stepper are C2
/// composites whose buttons carry [`SegmentedOption`](crate::composites::SegmentedOption)
/// / [`StepperButton`](crate::composites::StepperButton).
#[allow(clippy::type_complexity)]
pub fn collect_showcase_press(
    mut reader: MessageReader<OnPress>,
    kinds: Query<(
        Option<&crate::composites::SegmentedOption>,
        Option<&crate::composites::StepperButton>,
        Has<ShowcaseRunBuild>,
    )>,
    mut intents: ResMut<ShowcaseIntents>,
) {
    for OnPress(e) in reader.read() {
        let Ok((seg, step, is_run)) = kinds.get(*e) else {
            continue;
        };
        if seg.is_some() {
            intents.select_segment = Some(*e);
        } else if let Some(step) = step {
            intents.step = Some(*step == crate::composites::StepperButton::Increment);
        } else if is_run {
            intents.run_build = true;
        }
    }
}

/// Consume the staged [`ShowcaseIntents`]: restyle the segmented selection, apply
/// the stepper `±` (clamped `[0, 99]`) + rewrite the count, and kick off the build
/// animation. The collect/apply split (read `OnPress` once; the `set_*` paths are
/// `&mut World`) mirrors `ScrollListPlugin` / `ModalPlugin`.
pub fn apply_showcase_intents(world: &mut World) {
    let intents = std::mem::take(&mut *world.resource_mut::<ShowcaseIntents>());

    if let Some(option) = intents.select_segment {
        select_showcase_segment(world, option);
    }
    if let Some(increment) = intents.step {
        step_showcase(world, increment);
    }
    if intents.run_build {
        run_showcase_build(world);
    }
}

/// Restyle the segmented track to reflect the pressed option (the C2
/// [`set_segmented`](crate::composites::set_segmented) restyle): read the pressed
/// button's index + walk to its track, then re-tint.
fn select_showcase_segment(world: &mut World, option: Entity) {
    let Some(idx) = world
        .get::<crate::composites::SegmentedOption>(option)
        .map(|o| o.0)
    else {
        return;
    };
    let Some(track) = world.get::<ChildOf>(option).map(|c| c.parent()) else {
        return;
    };
    crate::composites::set_segmented(world, track, idx);
}

/// Apply a stepper `±` (clamped `[0, 99]`): read the [`ShowcaseStepper`] count off
/// the stepper root, adjust, write it back, and rewrite the visible count via the
/// C2 [`set_stepper`](crate::composites::set_stepper).
fn step_showcase(world: &mut World, increment: bool) {
    let Some(stepper) = find_single::<ShowcaseStepper>(world) else {
        return;
    };
    let Some(mut state) = world.get_mut::<ShowcaseStepper>(stepper) else {
        return;
    };
    let next = (state.count + if increment { 1 } else { -1 }).clamp(0, 99);
    if next == state.count {
        return;
    }
    state.count = next;
    crate::composites::set_stepper(world, stepper, next);
}

/// Kick off the build animation: animate the meter `0 → 100%` (the C2
/// [`set_meter`] tween) and arm the
/// [`ShowcaseBuild`] ramp so [`tick_showcase_build`] counts the "N%" label up. A
/// no-op when a build is already in flight (the design disables the button while
/// `building`).
fn run_showcase_build(world: &mut World) {
    let already = world.resource::<ShowcaseBuild>().building;
    if already {
        return;
    }
    let fill = world.resource::<ShowcaseBuild>().fill;
    if let Some(fill) = fill {
        // Reset to 0 then animate to full (the design's `runBuild` restart).
        buiy_widgets::composites::set_meter(world, fill, 0.0);
        buiy_widgets::composites::set_meter(world, fill, 1.0);
    }
    let mut build = world.resource_mut::<ShowcaseBuild>();
    build.building = true;
    build.progress = 0.0;
}

/// Tick the build ramp (in [`BuiySet::Animate`]): while `building`, advance the
/// [`ShowcaseBuild`] progress to 1.0 over `SHOWCASE_BUILD_SECS` (matching the
/// meter fill tween), rewrite the "N%" label, and on reach-100% clear `building`
/// and show a "Build finished" toast.
pub fn tick_showcase_build(world: &mut World) {
    if !world.resource::<ShowcaseBuild>().building {
        return;
    }
    let dt = world.resource::<bevy::time::Time>().delta_secs();
    let (progress, finished) = {
        let mut build = world.resource_mut::<ShowcaseBuild>();
        build.progress = (build.progress + dt / SHOWCASE_BUILD_SECS).min(1.0);
        let finished = build.progress >= 1.0;
        if finished {
            build.building = false;
        }
        (build.progress, finished)
    };
    set_showcase_meter_label(world, progress);
    // On reach-100%, show the "Build finished" toast — but only when the toast
    // lifecycle is wired (`ToastPlugin` seeds the `Toast` resource). A headless
    // app that drives the build without `ToastPlugin` skips the toast rather than
    // panicking on the missing resource.
    if finished && world.get_resource::<crate::composites::Toast>().is_some() {
        crate::composites::show_toast(world, "Build finished");
    }
}

/// Rewrite the meter's "N%" value label from a fraction `[0, 1]`.
fn set_showcase_meter_label(world: &mut World, frac: f32) {
    let Some(label) = find_single::<ShowcaseMeterLabel>(world) else {
        return;
    };
    let text = showcase_pct_text(frac);
    if let Some(mut t) = world.get_mut::<Text>(label)
        && t.0 != text
    {
        t.0 = text;
    }
}

/// Restyle every showcase switch whose `A11yToggled` changed this frame: recolor
/// its track (accent on / `surface.raised-alt` off) + slide its thumb (`left 3px ↔
/// 20px` via `Translate` x). Reads the widget-owned `A11yToggled` so a toggle from
/// ANY modality (pointer / Space+Enter / AT `Click`) — all of which the `Switch`
/// contract funnels into this one state — repaints here.
#[allow(clippy::type_complexity)]
pub fn drive_showcase_switches(
    changed: Query<(&A11yToggled, &Children), (With<Switch>, Changed<A11yToggled>)>,
    mut tracks: Query<(&mut Background, &Children), With<ShowcaseSwitchTrack>>,
    mut thumbs: Query<&mut Translate, With<ShowcaseSwitchThumb>>,
) {
    for (toggled, children) in &changed {
        let on = toggled.0 == Toggled::True;
        for &child in children {
            let Ok((mut bg, track_children)) = tracks.get_mut(child) else {
                continue;
            };
            bg.color = if on {
                ColorToken::Accent
            } else {
                ColorToken::SurfaceRaisedAlt
            };
            for &grandchild in track_children {
                if let Ok(mut translate) = thumbs.get_mut(grandchild) {
                    translate.0 = Length::px(if on {
                        SHOWCASE_SWITCH_ON_X
                    } else {
                        SHOWCASE_SWITCH_OFF_X
                    });
                }
            }
        }
    }
}

/// Drive the slider's custom visual + the live preview-square radius from the
/// slider's `A11yValue` (gated on `Changed<A11yValue>` so it fires once per change):
/// resize the fill bar, slide the 15px thumb, rewrite the "Npx" label, and set the
/// preview square's corner `Border.radius` to the value — the design's live
/// radius-from-slider link. A value change from any modality (keyboard arrows /
/// AT `Increment`/`SetValue`) lands in `A11yValue` and repaints here.
#[allow(clippy::type_complexity)]
pub fn drive_showcase_slider(
    sliders: Query<(&A11yValue, &Children), (With<Slider>, Changed<A11yValue>)>,
    rails: Query<&Children, With<ShowcaseSliderRail>>,
    mut fills: Query<&mut BoxModel, (With<ShowcaseSliderFill>, Without<ShowcaseSliderThumb>)>,
    mut thumbs: Query<&mut Translate, With<ShowcaseSliderThumb>>,
    mut preview: Query<&mut Border, With<ShowcasePreview>>,
    mut radius_label: Query<&mut Text, With<ShowcaseRadiusLabel>>,
) {
    for (value, children) in &sliders {
        let fill_px = showcase_slider_fill_px(value.now, value.min, value.max);
        let thumb_px = showcase_slider_thumb_px(value.now, value.min, value.max);

        // Walk Slider → rail → [fill, thumb].
        for &child in children {
            let Ok(rail_children) = rails.get(child) else {
                continue;
            };
            for &gc in rail_children {
                if let Ok(mut bm) = fills.get_mut(gc) {
                    bm.width = Sizing::Length(Length::px(fill_px));
                }
                if let Ok(mut translate) = thumbs.get_mut(gc) {
                    translate.0 = Length::px(thumb_px);
                }
            }
        }

        // The live preview-square radius (the design's `border-radius:{radius}px`).
        for mut border in &mut preview {
            border.radius = Corners::all(Radius::circular(value.now as f32));
        }
        // The "Npx" accent value.
        let text = showcase_radius_text(value.now);
        for mut t in &mut radius_label {
            if t.0 != text {
                t.0 = text.clone();
            }
        }
    }
}

/// Restyle every showcase disclosure whose `A11yExpanded` changed this frame:
/// rotate its chevron (90° down when open, identity when collapsed) + re-tint it
/// (accent open / `text.muted` collapsed) + toggle its body `Display` (flex open /
/// none collapsed). Reads the widget-owned `A11yExpanded` so an expand from any
/// modality (pointer / keyboard / AT `Click` / AT `Expand`) repaints here.
#[allow(clippy::type_complexity)]
pub fn drive_showcase_disclosures(
    changed: Query<(&A11yExpanded, &ChildOf, &Children), (With<Disclosure>, Changed<A11yExpanded>)>,
    items: Query<&Children>,
    mut chevrons: Query<(&mut Rotate, &mut Icon), With<ShowcaseChevron>>,
    mut bodies: Query<&mut Display, With<ShowcaseDiscBody>>,
) {
    for (expanded, parent, header_children) in &changed {
        let open = expanded.0;
        // The chevron is a child of the trigger header.
        for &child in header_children {
            if let Ok((mut rotate, mut icon)) = chevrons.get_mut(child) {
                *rotate = if open {
                    Rotate(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                } else {
                    Rotate(Quat::IDENTITY)
                };
                icon.color = if open {
                    ColorToken::Accent
                } else {
                    ColorToken::TextMuted
                };
            }
        }
        // The body is a SIBLING of the trigger (both children of the item wrapper).
        if let Ok(siblings) = items.get(parent.parent()) {
            for &sib in siblings {
                if let Ok(mut display) = bodies.get_mut(sib) {
                    *display = if open {
                        Display::flex_column()
                    } else {
                        Display::None
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The entity-tree node generator matches the design JS (values.md § 9):
    /// `type=TYPES[(i·7+3)%10]`, `depth=i==0?0:(i·13)%5`, `ms=((i·37)%180)/100+.02`,
    /// `st=i%53==0?WARN : i%131==0?ERR : OK`, `name=names[(i·11)%20]+'_'+pad4(i)`.
    #[test]
    fn gen_node_matches_the_design_generator() {
        // i=0: type TYPES[3]=Text, depth 0 (the i==0 special-case), ms 0.02,
        // state OK (0%53==0 → WARN actually: 0%53==0 is true → WARN),
        // name names[0]_0000 = root_0000.
        let n0 = gen_node(0);
        assert_eq!(n0.node_type, "Text"); // (0·7+3)%10 = 3 → Text
        assert_eq!(n0.depth, 0);
        assert_eq!(n0.name, "root_0000");
        assert_eq!(n0.state, "WARN"); // 0 % 53 == 0
        assert!((n0.ms - 0.02).abs() < 1e-6);

        // i=5: type TYPES[(38)%10=8]=Scroll, depth (65)%5=0, ms (185%180=5)/100+.02
        // = 0.07, state OK, name names[(55)%20=15]=item → item_0005.
        let n5 = gen_node(5);
        assert_eq!(n5.node_type, "Scroll");
        assert_eq!(n5.dot_color, ColorToken::StatusError);
        assert_eq!(n5.name, "item_0005");
        assert_eq!(n5.state, "OK");

        // i=1: depth (13)%5 = 3 → the deepest indent (cap 3).
        assert_eq!(gen_node(1).depth, 3);

        // i=131: 131%53 != 0 but 131%131 == 0 → ERR (the rarer error state).
        assert_eq!(gen_node(131).state, "ERR");
    }

    /// The thousands grouping matches the design's locale formatting.
    #[test]
    fn group_thousands_inserts_separators() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(42), "42");
        assert_eq!(group_thousands(1000), "1,000");
        assert_eq!(group_thousands(1234567), "1,234,567");
    }

    /// The heading total label switches between the windowed + filtered forms.
    #[test]
    fn scroll_total_text_picks_windowed_vs_filtered() {
        assert_eq!(scroll_total_text(1000, 1000), "1,000 nodes · windowed");
        assert_eq!(scroll_total_text(40, 1000), "40 of 1,000 nodes");
    }

    /// The footer window range projects the scroll offset onto the visible rows.
    #[test]
    fn scroll_window_range_projects_the_offset() {
        // Empty list → (0, 0).
        assert_eq!(scroll_window_range(0.0, 0), (0, 0));
        // At the top: first row is #1; the window spans ceil(360/34)+1 = 12 rows.
        let (first, last) = scroll_window_range(0.0, 1000);
        assert_eq!(first, 1);
        assert_eq!(last, (SCROLL_VIEWPORT_H / SCROLL_ROW_H).ceil() as usize);
        // Scrolled down 5 rows (5·34 = 170px): the window starts at row 6.
        assert_eq!(scroll_window_range(170.0, 1000).0, 6);
    }
}
