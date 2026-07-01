//! `buiy_gallery::shell` — the **unified IDE-style shell** (parity Wave C1) that
//! hosts + switches + themes the 5 screens (`spawn_todomvc_screen` / `spawn_scroll_screen`
//! / `spawn_overlay_menu` / `spawn_modal` / `spawn_showcase`).
//!
//! `cargo run -p buiy_gallery` opens the **whole shell**, not one screen: a top
//! chrome bar (gradient logo + `buiy` wordmark + a "widget catalog" badge + the
//! `$ cargo run -p buiy_gallery` mono chip + a "dark" theme chip + a GitHub icon),
//! a left **Screens** rail (5 nav buttons — each an active-state accent left-bar +
//! index + icon + name + sub-desc — over a **Stats** block), a center viewport (a
//! 42px backdrop-blurred header strip + a dotted radial-grid canvas hosting the
//! active screen), a right **Inspector** pane (the active screen's name/desc +
//! "Composed of" chips + live-state + accent swatches, filled by
//! [`inspector`](crate::inspector) — Wave C4), and a 28px status bar. Clicking a
//! rail button switches the viewport to that screen.
//!
//! ## Switch mechanism — Candidate A (spawn-all-once + `Display::None`)
//!
//! All 5 screen subtrees are spawned **once** at boot as children of
//! `#ScreenContent`; [`ScreenRouter`](crate::shell::ScreenRouter) selects the
//! active one and [`apply_screen_router`](crate::shell::apply_screen_router)
//! toggles `Display::None` + `A11yHidden` on the inactive
//! roots (NOT `CssVisibility::Hidden` — that is paint-skip only and keeps the box
//! in layout; `Display::None` prunes the whole subtree from Taffy, so a hidden
//! 1000-row scroll screen costs zero layout). State isolation (per-screen draft /
//! scroll-pos / menu-open) is preserved by construction — the subtree is never
//! despawned — and every screen has unique markers so the screen plugins'
//! `find_single::<…>` queries always resolve their one instance. The applier runs
//! `.after(BuiySet::Input).before(BuiySet::A11yUpdate)` so the `A11yHidden` toggle
//! lands in the same frame's a11y-tree rebuild (and `handle_tab` skips the inert
//! subtrees). Co-design recorded in the prototype journal (2026-06-25 — Wave C1).
//!
//! Design: `docs/specs/2026-06-25-widget-catalog-parity-design.md` § 3.7;
//! exact values: `docs/specs/2026-06-25-widget-catalog-values.md` § 1, § 6, § 7.

use bevy::prelude::{
    App, Camera2d, Commands, Component, DetectChanges, Entity, IntoScheduleConfigs, Message,
    MessageReader, MessageWriter, Name, Plugin, Query, Reflect, ReflectComponent, ReflectDefault,
    Res, Resource, Startup, Update, With, World,
};
use buiy::prelude::*;
use buiy_core::BuiySet;
use buiy_core::a11y::A11yHidden;
use buiy_core::interaction::OnPress;
// W3 (MVU migration): the screen router folds through an MVU model. `Cmd`,
// `Model`, `MvuAppExt`, and `fold_one_inline` now come from `buiy::prelude::*`
// (the preluded MVU surface), so no `buiy_core::mvu` import is needed.
// `BackgroundLayers`/`BackgroundLayer`/`LinearGradient`/`RadialGradient`/
// `ColorStop`/`Icon` now reach us through `buiy::prelude::*` (spec § 2 REFINE
// promotions); only the not-yet-promoted render primitives are imported here.
use buiy_core::render::components::{BackdropFilter, BoxShadow, FilterFn, LineStyle, Shadow};
use buiy_core::text::{FamilyEntry, FontStack, LetterSpacing};

use crate::{
    DEMO_SEEDS, SCROLL_LIST_ROWS, append_row, fill_scroll_list, spawn_modal, spawn_overlay_menu,
    spawn_scroll_screen, spawn_showcase, spawn_todomvc_screen,
};

// ===========================================================================
// Screen identity + the router
// ===========================================================================

/// One of the five gallery screens hosted in the viewport.
///
/// W3 (prototype MVU migration): `Reflect` added so it can be a field of the
/// `NavModel` MVU model + its `NavMsg` (the record log serializes via `Reflect`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
pub enum Screen {
    /// S1 — the TodoMVC exemplar (the default screen, mirroring the design's
    /// initial `screen: 'todo'`).
    #[default]
    Todo,
    /// S2 — the 1000-row virtual list.
    Scroll,
    /// S3 — the anchored overlay menu.
    Menu,
    /// S4 — the focus-trapped modal.
    Modal,
    /// S5 — the F-tier controls showcase.
    Showcase,
}

impl Screen {
    /// The five screens in rail order (`01`..`05`).
    pub const ALL: [Screen; 5] = [
        Screen::Todo,
        Screen::Scroll,
        Screen::Menu,
        Screen::Modal,
        Screen::Showcase,
    ];

    /// The rail index label (`"01"`..`"05"`).
    pub fn idx(self) -> &'static str {
        match self {
            Screen::Todo => "01",
            Screen::Scroll => "02",
            Screen::Menu => "03",
            Screen::Modal => "04",
            Screen::Showcase => "05",
        }
    }

    /// The rail nav name (values.md § META).
    pub fn name(self) -> &'static str {
        match self {
            Screen::Todo => "TodoMVC",
            Screen::Scroll => "Virtual List",
            Screen::Menu => "Overlay Menu",
            Screen::Modal => "Modal Dialog",
            Screen::Showcase => "Controls",
        }
    }

    /// The rail nav sub-description (values.md § SCR).
    pub fn desc(self) -> &'static str {
        match self {
            Screen::Todo => "list · input · filters",
            Screen::Scroll => "1,000 windowed rows",
            Screen::Menu => "anchored popover",
            Screen::Modal => "focus-trap + backdrop",
            Screen::Showcase => "switch · slider · more",
        }
    }

    /// The viewport-header source path (values.md § META `path`).
    pub fn path(self) -> &'static str {
        match self {
            Screen::Todo => "gallery::screens::todo",
            Screen::Scroll => "gallery::screens::scroll",
            Screen::Menu => "gallery::screens::menu",
            Screen::Modal => "gallery::screens::modal",
            Screen::Showcase => "gallery::screens::showcase",
        }
    }

    /// The viewport-header size badge (values.md § META `size`).
    pub fn size_badge(self) -> &'static str {
        match self {
            Screen::Todo => "560 × auto",
            Screen::Scroll => "fill",
            Screen::Menu => "420 × auto",
            Screen::Modal => "440 × auto",
            Screen::Showcase => "880 × auto",
        }
    }

    /// The rail nav SVG icon path `d` (values.md § 6 #4–#8).
    fn icon_path(self) -> &'static str {
        match self {
            Screen::Todo => "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5",
            Screen::Scroll => "M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01",
            Screen::Menu => "M12 6h.01M12 12h.01M12 18h.01",
            Screen::Modal => "M4 5h16v14H4zM4 9h16",
            Screen::Showcase => "M4 8h10M18 8h2M4 16h2M10 16h10M14 5v6M8 13v6",
        }
    }
}

/// The active-screen selector (default [`Screen::Todo`], mirroring the design's
/// `screen: 'todo'` initial state). [`apply_screen_router`] reads this to toggle
/// which screen subtree is laid out + a11y-visible.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenRouter(pub Screen);

/// Request to switch the viewport to a screen — written by a rail nav button's
/// `OnPress` ([`route_nav_press`]) and consumed by [`apply_screen_router`]. Carried
/// as a `Message` (the C8 intent pattern) so the press → switch flows through the
/// same `.after(Input)` ordering as every other gallery interaction.
#[derive(Message, Clone, Copy, Debug)]
pub struct SwitchScreen(pub Screen);

// ---------------------------------------------------------------------------
// W3 (prototype): the screen router migrated onto the MVU substrate.
//
// The active screen is now an MVU **model** (`NavModel`) — the single, recorded
// source of truth — folded by a pure `nav_reducer`. `apply_screen_router` folds a
// `SwitchScreen` request through it via the synchronous `fold_one_inline` seam and
// then PROJECTS the folded screen onto the legacy `ScreenRouter` resource, so the
// existing chrome-reflect + inspector readers (`reflect_active_screen`,
// `reflect_rail_active_state`, `inspector::rebuild_inspector_on_switch`) are
// untouched. This is the "strangler" migration: the model owns the truth, the
// resource becomes a bind-derived projection kept for compatibility.
// ---------------------------------------------------------------------------

/// The MVU model for the active screen (W3). The single recorded source of truth;
/// `nav_reducer` is its sole (inline) writer. Projected onto [`ScreenRouter`].
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug, Default)]
#[reflect(Component, Default)]
pub struct NavModel(pub Screen);

impl Model for NavModel {
    type Msg = NavMsg;
}

/// Messages folded into [`NavModel`]. Absolute (`Switch(screen)`), never a toggle,
/// so the inline fold is deterministic.
#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum NavMsg {
    /// Switch the active screen to this one.
    Switch(Screen),
}

/// The pure [`NavModel`] reducer (W3). `set_if_neq` in the fold makes a switch to
/// the already-active screen an idempotent no-op.
pub fn nav_reducer(model: &mut NavModel, msg: NavMsg) -> Cmd<NavMsg> {
    match msg {
        NavMsg::Switch(screen) => model.0 = screen,
    }
    Cmd::none()
}

/// Re-themes the whole app to a new accent — re-exported here under the gallery's
/// own name so the C4 inspector accent swatches can `write` it. (The framework
/// type is `buiy_core::theme::SetAccent`; the apply system lives in `ThemePlugin`.)
/// Produced by [`inspector::route_accent_press`](crate::inspector::route_accent_press)
/// when a swatch is pressed; `apply_set_accent` re-seeds the accent ramp so the
/// `theme.is_changed()` re-extract re-resolves every accent-bearing paint.
pub use buiy_core::theme::SetAccent;

/// Marks the `#ScreenContent` slot in the viewport canvas — the parent the 5
/// screen subtrees are re-homed under at boot.
#[derive(Component, Clone, Default)]
pub struct ScreenContent;

/// Marks the `#Inspector` panel (the right pane). The C4 inspector content
/// (`inspector::build_inspector_content`) finds this panel to fill it with the
/// name/desc + composed-of + live-state + accent sections.
#[derive(Component, Clone, Default)]
pub struct Inspector;

/// The root entity of one hosted screen subtree, tagged with which [`Screen`] it
/// is, so [`apply_screen_router`] can toggle the matching root. Each of the 5
/// screen subtrees gets exactly one of these on its outermost entity.
#[derive(Component, Clone, Copy)]
pub struct ScreenRoot(pub Screen);

/// The `Display` a screen root was authored with, captured at mount so the router
/// can restore it on re-activation (the `Display::None` hide is a non-destructive
/// overwrite; this is the value to put back).
#[derive(Component, Clone, Copy)]
pub struct ScreenAuthoredDisplay(pub Display);

/// Marks a rail nav button with the screen it selects ([`route_nav_press`] reads
/// the pressed entity's `ScreenNav` → writes `SwitchScreen`). Mirrors the todo
/// `FilterButton` marker pattern.
#[derive(Component, Clone, Copy)]
pub struct ScreenNav(pub Screen);

/// Marks one styled part of a rail nav button so [`reflect_rail_active_state`] can
/// restyle it on the active-screen change (the C4 rail-active-state fix — C1/C3
/// hard-coded the active visual to Todo; now it follows the [`ScreenRouter`]).
/// Each part's active/inactive paint is the design's `SCR` map (JS 561–564).
#[derive(Component, Clone, Copy)]
pub enum NavPart {
    /// The accent left-bar (`accent` when active, transparent when not).
    Bar,
    /// The index span (`accent` active / `text.dim` not).
    Idx,
    /// The screen icon (`text.primary` active / `text.muted` not).
    Icon,
    /// The name leaf (`text.primary` active / `text.secondary` not).
    Name,
}

/// Marks the viewport-header text entities the [`reflect_active_screen`] system
/// rewrites when the active screen changes: the screen name, the source path, and
/// the size badge.
#[derive(Component, Clone, Copy)]
pub enum ViewportHeaderField {
    /// The `#VpScreenName` mono label (e.g. "TodoMVC").
    Name,
    /// The `#VpScreenPath` dim mono label (e.g. "gallery::screens::todo").
    Path,
    /// The `#VpSizeBadge` muted chip (e.g. "560 × auto").
    Size,
}

// ===========================================================================
// Authoring helpers (the shell is a large static tree of one-off styled boxes;
// imperative `Style`-builder spawning + small composable helpers reads cleaner
// than one giant `bsn!` block — the `examples/capture` + `spawn_modal` idiom).
// ===========================================================================

/// A `ColorToken::Token` from a `&str` key — the shell authors every paint as a
/// named dark token (never a literal), keeping the forced-colors gate enforceable.
fn tok(key: &str) -> ColorToken {
    ColorToken::Token(key.to_string().into())
}

/// The Geist sans font stack (`FontFamily([Named("Geist")])`). The sans generic
/// still resolves to Fira (Wave A note), so the shell authors Geist **by name**.
fn geist() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist".into())]))
}

/// The Geist Mono font stack (the design's monospace UI face).
fn geist_mono() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist Mono".into())]))
}

/// The typographic spec of one shell text leaf — size, family, weight, tint, and
/// an optional letter-spacing. Built via [`TextSpec::sans`] / [`TextSpec::mono`]
/// then `.ls(...)`, so call sites read as the design's `font:<weight> <size>
/// <family>` shorthand without a 7-arg helper.
#[derive(Clone)]
struct TextSpec {
    size: f32,
    family: FontFamily,
    weight: u16,
    color: ColorToken,
    letter_spacing: Option<f32>,
}

impl TextSpec {
    /// A Geist (sans) leaf at `size`/`weight`, tinted `color`.
    fn sans(size: f32, weight: u16, color: ColorToken) -> Self {
        Self {
            size,
            family: geist(),
            weight,
            color,
            letter_spacing: None,
        }
    }

    /// A Geist Mono leaf at `size`/`weight`, tinted `color`.
    fn mono(size: f32, weight: u16, color: ColorToken) -> Self {
        Self {
            size,
            family: geist_mono(),
            weight,
            color,
            letter_spacing: None,
        }
    }

    /// Add a letter-spacing (the design's `letter-spacing` in px, values.md § 4).
    fn ls(mut self, px: f32) -> Self {
        self.letter_spacing = Some(px);
        self
    }
}

/// A leaf text node from a [`TextSpec`]. Carries `Pickable::IGNORE` so clicks fall
/// through to the owning button/box (the shell's labels are decorative pixels).
fn text_leaf(world: &mut World, name: &str, s: &str, spec: TextSpec) -> Entity {
    let mut e = world.spawn((
        Node,
        Name::new(name.to_string()),
        Text(s.to_string()),
        FontSize(spec.size),
        spec.family,
        FontWeight(spec.weight),
        TextColor(spec.color),
        Pickable::IGNORE,
    ));
    if let Some(ls) = spec.letter_spacing {
        e.insert(LetterSpacing(ls));
    }
    e.id()
}

/// A vector-icon box: a `size`×`size` node carrying an [`Icon`] (the SVG path
/// stroked to a coverage glyph, tinted by `color`). `Pickable::IGNORE` so it
/// doesn't eat the owning button's clicks.
fn icon_box(
    world: &mut World,
    name: &str,
    path_d: &str,
    stroke_width: f32,
    size_px: u16,
    fill: bool,
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
                fill,
                color,
            },
            Pickable::IGNORE,
        ))
        .id()
}

/// A bordered, token-filled chrome chip (header `$` chip / "dark" chip / size
/// badge): a flex-row box with `padding`, a 1px border (`border_color`), `radius`,
/// and a token `bg`. Children are added by the caller.
fn chip(
    world: &mut World,
    name: &str,
    padding: Edges,
    radius: f32,
    bg: &str,
    border_color: &str,
    gap: f32,
) -> Entity {
    world
        .spawn((
            Node,
            Name::new(name.to_string()),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(gap)
                .padding_edges(padding)
                .border(1.0),
            Background { color: tok(bg) },
            Border {
                top: solid_side(border_color),
                right: solid_side(border_color),
                bottom: solid_side(border_color),
                left: solid_side(border_color),
                radius: Corners::all(Radius::circular(radius)),
            },
        ))
        .id()
}

/// A solid 1px `BorderSide` of a token color.
fn solid_side(token: &str) -> BorderSide {
    BorderSide {
        color: tok(token),
        style: LineStyle::Solid,
    }
}

/// A flex container box (no fill) — a layout-only grouping node. `axis` picks
/// row/column; `gap` is the flex gap.
fn flex_box(world: &mut World, name: &str, style: Style) -> Entity {
    world.spawn((Node, Name::new(name.to_string()), style)).id()
}

/// A flex spacer (`flex: 1`) that eats free space — the chrome `<div flex:1>` and
/// rail `<div flex:1>` gap fillers. `FlexItem.grow` is the decomposed-only flex
/// growth input (the `Style` builder does not carry flex-item props).
fn spacer(world: &mut World, name: &str) -> Entity {
    world
        .spawn((
            Node,
            Name::new(name.to_string()),
            Style::default(),
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
        ))
        .id()
}

/// A `flex: 1` content node — fills its parent's free space along the main axis
/// (the viewport `<main>`, the canvas, the rail nav-list growth). Carries the
/// given `Style` plus `FlexItem.grow = 1.0`.
fn grow_box(world: &mut World, name: &str, style: Style) -> Entity {
    world
        .spawn((
            Node,
            Name::new(name.to_string()),
            style,
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
        ))
        .id()
}

// ===========================================================================
// The shell tree (5 fixed panes: chrome → body[rail | viewport | inspector] →
// status). Sizes are the exact values.md § 7.1 chrome dimensions.
// ===========================================================================

/// Build the whole shell tree into `world` and return its `#ShellRoot` entity.
/// Lays out a full-window flex-column: a 52px chrome row, a `flex:1` body row
/// (`[rail 248px | viewport flex:1 | inspector 280px]`), and a 28px status row.
/// The `#ScreenContent` slot inside the viewport canvas is where the 5 screen
/// subtrees are re-homed by [`mount_screens`].
pub fn build_shell(world: &mut World) -> Entity {
    let chrome = build_chrome(world);
    let rail = build_rail(world);
    let viewport = build_viewport(world);
    let inspector = build_inspector(world);
    let status = build_status_bar(world);

    // The body row: rail | viewport | inspector. `flex:1`, `min-height:0` so the
    // panes' own `overflow` governs (HTML 56 `flex:1;min-height:0`).
    let body = grow_box(
        world,
        "#Body",
        Style::default()
            .flex_row()
            .min_height(Sizing::Length(Length::px(0.0))),
    );
    world
        .entity_mut(body)
        .add_children(&[rail, viewport, inspector]);

    // The window-filling root flex-column (HTML 31: `100vh × 100%`, column,
    // `overflow:hidden`, bg `color.surface.app`). 100vh → the preview height; the
    // capture/run paths size the window to 1280×800, so author the root at that.
    let root = world
        .spawn((
            Node,
            Name::new("#ShellRoot"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0)))
                .height(Sizing::Length(Length::percent(100.0)))
                .overflow_hidden(),
            Background {
                color: tok("color.surface.app"),
            },
        ))
        .id();
    world.entity_mut(root).add_children(&[chrome, body, status]);
    root
}

/// The top chrome bar (HTML 34, values.md § 7.1): height 52px, `flex:none`,
/// `padding:0 16px 0 18px`, `gap:18px`, a bottom 1px `color.border.subtle`
/// divider, bg `color.surface.chrome`. Holds the gradient logo cluster, a `flex:1`
/// spacer, the `$ cargo run` mono chip, the "dark" theme chip, and the GitHub icon.
fn build_chrome(world: &mut World) -> Entity {
    // Logo tile: 24×24, radius 6, the 150deg accent gradient, holding the 13×13
    // logo-bars glyph stroked in `color.text.on-accent` (values.md § 6 #1; § 8).
    let logo_glyph = icon_box(
        world,
        "#LogoGlyph",
        "M5 4h7a4 4 0 0 1 0 8H5zM5 12h8a4 4 0 0 1 0 8H5z",
        2.4,
        13,
        false,
        tok("color.text.on-accent"),
    );
    let logo = world
        .spawn((
            Node,
            Name::new("#LogoTile"),
            Style::default()
                .width_px(24.0)
                .height_px(24.0)
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center),
            // The 150deg accent gradient (`--ac → --ac2`), opaque stops (§ 8).
            BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                angle_deg: 150.0,
                stops: vec![
                    ColorStop {
                        color: tok("color.accent"),
                        position: 0.0,
                    },
                    ColorStop {
                        color: tok("color.accent.lighter"),
                        position: 1.0,
                    },
                ],
            })]),
            Border {
                radius: Corners::all(Radius::circular(6.0)),
                ..Default::default()
            },
        ))
        .add_children(&[logo_glyph])
        .id();

    // "buiy" wordmark (Geist Mono 15 / 600 / -.15px LS, text.primary).
    let wordmark = text_leaf(
        world,
        "#Wordmark",
        "buiy",
        TextSpec::mono(15.0, 600, tok("color.text.primary")).ls(-0.15),
    );

    // "widget catalog" accent badge (Geist Mono 11 / 500 / .22px, accent tint).
    let badge = {
        let label = text_leaf(
            world,
            "#CatalogBadgeText",
            "widget catalog",
            TextSpec::mono(11.0, 500, tok("color.accent")).ls(0.22),
        );
        let b = chip(
            world,
            "#CatalogBadge",
            Edges::axis(7.0, 3.0),
            5.0,
            "color.surface.inset",
            "color.border.default",
            0.0,
        );
        world.entity_mut(b).add_children(&[label]);
        b
    };

    // The logo cluster: logo tile + wordmark + badge, `gap:10px` (HTML 35).
    let logo_cluster = flex_box(
        world,
        "#LogoCluster",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .gap_px(10.0),
    );
    world
        .entity_mut(logo_cluster)
        .add_children(&[logo, wordmark, badge]);

    let chrome_spacer = spacer(world, "#ChromeSpacer");

    // `$ cargo run -p buiy_gallery` mono chip — the `$ ` prefix is `text.dim`, the
    // command `text.muted` (values.md typography table). Authored as two spans.
    let cargo_chip = {
        let prefix = text_leaf(
            world,
            "#CargoPrefix",
            "$ ",
            TextSpec::mono(11.5, 500, tok("color.text.dim")),
        );
        let cmd = text_leaf(
            world,
            "#CargoCmd",
            "cargo run -p buiy_gallery",
            TextSpec::mono(11.5, 500, tok("color.text.muted")),
        );
        let c = chip(
            world,
            "#CargoChip",
            Edges::axis(9.0, 5.0),
            6.0,
            "color.surface.inset",
            "color.border.default",
            0.0,
        );
        world.entity_mut(c).add_children(&[prefix, cmd]);
        c
    };

    // "dark" theme chip: a 13×13 moon icon + the "dark" label (values.md § 6 #2).
    let theme_chip = {
        let moon = icon_box(
            world,
            "#ThemeMoon",
            "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z",
            1.7,
            13,
            false,
            tok("color.text.muted"),
        );
        let label = text_leaf(
            world,
            "#ThemeLabel",
            "dark",
            TextSpec::mono(11.0, 500, tok("color.text.muted")),
        );
        let c = chip(
            world,
            "#ThemeChip",
            Edges::axis(9.0, 5.0),
            6.0,
            "color.surface.inset",
            "color.border.default",
            6.0,
        );
        world.entity_mut(c).add_children(&[moon, label]);
        c
    };

    // GitHub icon button: 30×30, radius 6, the filled octocat mark (values.md
    // § 6 #3 — `fill: true`, tinted `text.secondary` the link color).
    let github = {
        let mark = icon_box(
            world,
            "#GithubMark",
            "M12 2C6.48 2 2 6.58 2 12.25c0 4.53 2.87 8.37 6.84 9.73.5.1.68-.22.68-.49v-1.7c-2.78.62-3.37-1.22-3.37-1.22-.46-1.18-1.11-1.5-1.11-1.5-.91-.64.07-.62.07-.62 1 .07 1.53 1.05 1.53 1.05.9 1.57 2.36 1.12 2.94.86.09-.67.35-1.12.63-1.38-2.22-.26-4.55-1.14-4.55-5.06 0-1.12.39-2.03 1.03-2.75-.1-.26-.45-1.3.1-2.7 0 0 .84-.28 2.75 1.05a9.3 9.3 0 0 1 5 0c1.91-1.33 2.75-1.05 2.75-1.05.55 1.4.2 2.44.1 2.7.64.72 1.03 1.63 1.03 2.75 0 3.93-2.34 4.8-4.57 5.05.36.32.68.94.68 1.9v2.82c0 .27.18.6.69.49A10.02 10.02 0 0 0 22 12.25C22 6.58 17.52 2 12 2Z",
            0.0,
            15,
            true,
            tok("color.text.secondary"),
        );
        world
            .spawn((
                Node,
                Name::new("#GithubButton"),
                Style::default()
                    .width_px(30.0)
                    .height_px(30.0)
                    .flex_row()
                    .justify_content(JustifyContent::Center)
                    .align_items(AlignItems::Center)
                    .border(1.0),
                Background {
                    color: tok("color.surface.inset"),
                },
                Border {
                    top: solid_side("color.border.default"),
                    right: solid_side("color.border.default"),
                    bottom: solid_side("color.border.default"),
                    left: solid_side("color.border.default"),
                    radius: Corners::all(Radius::circular(6.0)),
                },
            ))
            .add_children(&[mark])
            .id()
    };

    // The chrome row itself: 52px, padding 0/16/0/18, gap 18, bottom divider, bg.
    let chrome = world
        .spawn((
            Node,
            Name::new("#TopChrome"),
            Style::default()
                .flex_row()
                .height_px(52.0)
                .align_items(AlignItems::Center)
                .gap_px(18.0)
                .padding_edges(Edges {
                    top: Length::px(0.0),
                    right: Length::px(16.0),
                    bottom: Length::px(0.0),
                    left: Length::px(18.0),
                })
                .border_edges(Edges {
                    top: Length::px(0.0),
                    right: Length::px(0.0),
                    bottom: Length::px(1.0),
                    left: Length::px(0.0),
                }),
            Background {
                color: tok("color.surface.chrome"),
            },
            Border {
                bottom: solid_side("color.border.subtle"),
                ..Default::default()
            },
        ))
        .id();
    world.entity_mut(chrome).add_children(&[
        logo_cluster,
        chrome_spacer,
        cargo_chip,
        theme_chip,
        github,
    ]);
    chrome
}

/// The left Screens rail (HTML 57, values.md § 7.1): width 248px, `flex:none`,
/// column, a right 1px `color.border.subtle` divider, bg `color.surface.chrome`.
/// Holds the "Screens" section label, the 5 nav buttons, a `flex:1` spacer, and
/// the bordered Stats block.
fn build_rail(world: &mut World) -> Entity {
    // "Screens" uppercase section label (Geist Mono 10 / 500 / 1.40px LS, dim).
    // padding:16px 14px 8px (HTML 58) — set via a `BoxModel` patch so the rest of
    // the text node's default layout (from `Node`'s required Style) is untouched.
    let screens_label = section_label(world, "#ScreensLabel", "SCREENS");
    world.entity_mut(screens_label).insert(BoxModel {
        padding: Edges {
            top: Length::px(16.0),
            right: Length::px(14.0),
            bottom: Length::px(8.0),
            left: Length::px(14.0),
        },
        ..Default::default()
    });

    // The nav list: padding 0/8, gap 2 (HTML 59), holding the 5 nav buttons. The
    // default screen (Todo) is active at boot — the bar/idx/icon/name reflect that.
    let nav_buttons: Vec<Entity> = Screen::ALL
        .iter()
        .map(|&s| build_nav_button(world, s, s == Screen::default()))
        .collect();
    let nav_list = flex_box(
        world,
        "#NavList",
        Style::default()
            .flex_column()
            .gap_px(2.0)
            .padding_edges(Edges::axis(8.0, 0.0)),
    );
    world.entity_mut(nav_list).add_children(&nav_buttons);

    let rail_spacer = spacer(world, "#RailSpacer");
    let stats = build_stats(world);

    let rail = world
        .spawn((
            Node,
            Name::new("#ScreenRail"),
            Style::default()
                .flex_column()
                .width_px(248.0)
                .min_height(Sizing::Length(Length::px(0.0)))
                .border_edges(Edges {
                    top: Length::px(0.0),
                    right: Length::px(1.0),
                    bottom: Length::px(0.0),
                    left: Length::px(0.0),
                }),
            Background {
                color: tok("color.surface.chrome"),
            },
            Border {
                right: solid_side("color.border.subtle"),
                ..Default::default()
            },
        ))
        .id();
    world
        .entity_mut(rail)
        .add_children(&[screens_label, nav_list, rail_spacer, stats]);
    rail
}

/// One uppercase mono section label (Geist Mono 10 / 500, `text.dim`, 1.40px LS) —
/// the "SCREENS" / "STATS" rail headings + the inspector section headings.
fn section_label(world: &mut World, name: &str, s: &str) -> Entity {
    text_leaf(
        world,
        name,
        s,
        TextSpec::mono(10.0, 500, tok("color.text.dim")).ls(1.40),
    )
}

/// One rail nav button (HTML 60–70, JS 560–566): a full-width flex-row button —
/// an active-state accent left-bar (absolute, 2.5px, radius 99), the index span
/// (16px), the screen icon (17×17), and a name/desc column. Carries a
/// [`ScreenNav`] marker so a press writes `SwitchScreen`; the `Button` widget
/// supplies the `OnPress` sink + the `A11yRole::Button` + the keymap. `active`
/// seeds the boot-time active styling (the default screen).
fn build_nav_button(world: &mut World, screen: Screen, active: bool) -> Entity {
    let bar_color = if active {
        tok("color.accent")
    } else {
        tok("color.surface.transparent")
    };
    let idx_color = if active {
        tok("color.accent")
    } else {
        tok("color.text.dim")
    };
    let icon_color = if active {
        tok("color.text.primary")
    } else {
        tok("color.text.muted")
    };
    let name_color = if active {
        tok("color.text.primary")
    } else {
        tok("color.text.secondary")
    };
    let bg = if active {
        tok("color.surface.card")
    } else {
        tok("color.surface.transparent")
    };

    // The active accent left-bar: absolute, left:0, top/bottom:8px, width 2.5px,
    // radius 99 (JS 561). Rendered as a thin rounded box.
    let bar = world
        .spawn((
            Node,
            Name::new("#NavBar"),
            Style::default()
                .absolute()
                .inset(Inset {
                    left: Sizing::Length(Length::px(0.0)),
                    top: Sizing::Length(Length::px(8.0)),
                    bottom: Sizing::Length(Length::px(8.0)),
                    ..Default::default()
                })
                .width_px(2.5),
            Background { color: bar_color },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            NavPart::Bar,
            Pickable::IGNORE,
        ))
        .id();

    // The index span (Geist Mono 10.5 / 500, 16px wide, left-aligned). The fixed
    // 16px width is a `BoxModel` patch (leaves the rest of the default layout).
    let idx = text_leaf(
        world,
        "#NavIdx",
        screen.idx(),
        TextSpec::mono(10.5, 500, idx_color),
    );
    world.entity_mut(idx).insert((
        NavPart::Idx,
        BoxModel {
            width: Sizing::Length(Length::px(16.0)),
            ..Default::default()
        },
    ));

    // The 17×17 screen icon (values.md § 6 #4–#8, stroke 1.7).
    let icon = icon_box(
        world,
        "#NavIcon",
        screen.icon_path(),
        1.7,
        17,
        false,
        icon_color,
    );
    world.entity_mut(icon).insert(NavPart::Icon);

    // The name/desc column (gap 1).
    let name = text_leaf(
        world,
        "#NavName",
        screen.name(),
        TextSpec::sans(13.0, 500, name_color),
    );
    world.entity_mut(name).insert(NavPart::Name);
    let desc = text_leaf(
        world,
        "#NavDesc",
        screen.desc(),
        TextSpec::sans(11.0, 400, tok("color.text.faint")),
    );
    let label_col = flex_box(
        world,
        "#NavLabelCol",
        Style::default().flex_column().gap_px(1.0),
    );
    world.entity_mut(label_col).add_children(&[name, desc]);

    // The button itself: a `Button` widget (the OnPress sink + Button a11y/keymap)
    // styled as the nav row. `padding:9px 10px 9px 12px`, gap 10, radius 8,
    // `position:relative` so the absolute bar anchors to it.
    let btn = world
        .spawn((
            buiy::prelude::Button::new(screen.name()),
            ScreenNav(screen),
            Name::new("#NavButton"),
            Style::default()
                .relative()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(10.0)
                .padding_edges(Edges {
                    top: Length::px(9.0),
                    right: Length::px(10.0),
                    bottom: Length::px(9.0),
                    left: Length::px(12.0),
                })
                .width(Sizing::Length(Length::percent(100.0))),
            Background { color: bg },
            Border {
                radius: Corners::all(Radius::circular(8.0)),
                ..Default::default()
            },
        ))
        .id();
    world
        .entity_mut(btn)
        .add_children(&[bar, idx, icon, label_col]);
    btn
}

/// The rail Stats block (HTML 73, values.md § 7.1): `padding:12px 14px`, a top 1px
/// divider, `gap:9px`. A "Stats" section label + 3 key/value rows.
fn build_stats(world: &mut World) -> Entity {
    let label = section_label(world, "#StatsLabel", "STATS");
    let rows: Vec<Entity> = [("screens", "5"), ("primitives", "12"), ("crates", "5")]
        .iter()
        .map(|&(k, v)| stat_row(world, k, v))
        .collect();

    let stats = world
        .spawn((
            Node,
            Name::new("#StatsBlock"),
            Style::default()
                .flex_column()
                .gap_px(9.0)
                .padding_edges(Edges::axis(14.0, 12.0))
                .border_edges(Edges {
                    top: Length::px(1.0),
                    right: Length::px(0.0),
                    bottom: Length::px(0.0),
                    left: Length::px(0.0),
                }),
            Border {
                top: solid_side("color.border.subtle"),
                ..Default::default()
            },
        ))
        .id();
    let mut children = vec![label];
    children.extend(rows);
    world.entity_mut(stats).add_children(&children);
    stats
}

/// One stat key/value row: a baseline space-between row — the key (Geist 11.5/400,
/// muted) left, the value (Geist Mono 11.5/500, secondary) right.
fn stat_row(world: &mut World, k: &str, v: &str) -> Entity {
    let key = text_leaf(
        world,
        "#StatKey",
        k,
        TextSpec::sans(11.5, 400, tok("color.text.muted")),
    );
    let val = text_leaf(
        world,
        "#StatVal",
        v,
        TextSpec::mono(11.5, 500, tok("color.text.secondary")),
    );
    let row = flex_box(
        world,
        "#StatRow",
        Style::default()
            .flex_row()
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Baseline),
    );
    world.entity_mut(row).add_children(&[key, val]);
    row
}

/// The center viewport (HTML 85, values.md § 7.1): `flex:1`, column, bg
/// `color.surface.app` + the **dotted radial-grid** (`RadialGradient::dot_grid`,
/// 1px dot / 22px tile). Holds a 42px **backdrop-blurred** header strip + the
/// `flex:1` canvas whose `#ScreenContent` slot hosts the active screen.
fn build_viewport(world: &mut World) -> Entity {
    let header = build_viewport_header(world);

    // The `#ScreenContent` slot — the parent the 5 screen subtrees re-home under.
    // A flex-column so a screen's own `max-width`/`margin:auto` wrap centers; the
    // active screen fills it.
    let screen_content = world
        .spawn((
            Node,
            ScreenContent,
            Name::new("#ScreenContent"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0))),
        ))
        .id();

    // The viewport canvas (HTML 95): `flex:1`, `overflow:auto`, `position:relative`.
    let canvas = grow_box(
        world,
        "#Canvas",
        Style::default()
            .flex_column()
            .relative()
            .min_height(Sizing::Length(Length::px(0.0)))
            .overflow(OverflowMode::Auto, OverflowMode::Auto),
    );
    world.entity_mut(canvas).add_children(&[screen_content]);

    // The viewport `<main>`: `flex:1`, `min-width:0`, column, bg app + dotted grid.
    let viewport = grow_box(
        world,
        "#Viewport",
        Style::default()
            .flex_column()
            .min_width(Sizing::Length(Length::px(0.0))),
    );
    world.entity_mut(viewport).insert((
        Background {
            color: tok("color.surface.app"),
        },
        // The dotted radial-grid: a 1px `color.misc.dot-bg` dot centered in every
        // 22px cell, transparent between (values.md § 7.3; B2 `dot_grid`).
        BackgroundLayers(vec![BackgroundLayer::Radial(RadialGradient::dot_grid(
            tok("color.misc.dot-bg"),
            1.0,
            22.0,
        ))]),
    ));
    world.entity_mut(viewport).add_children(&[header, canvas]);
    viewport
}

/// The 42px viewport header strip (HTML 87, values.md § 7.1): `flex:none`, gap 12,
/// padding 0/16, a bottom 1px divider, bg `color.surface.chrome-translucent` +
/// **`backdrop-filter:blur(6px)`** (B4). Holds the active screen name + path + a
/// `flex:1` spacer + the size badge; the three labels carry
/// [`ViewportHeaderField`] so [`reflect_viewport_header`] rewrites them on switch.
fn build_viewport_header(world: &mut World) -> Entity {
    let initial = Screen::default();

    let name = text_leaf(
        world,
        "#VpScreenName",
        initial.name(),
        TextSpec::mono(12.5, 500, tok("color.text.secondary")),
    );
    world.entity_mut(name).insert(ViewportHeaderField::Name);

    let path = text_leaf(
        world,
        "#VpScreenPath",
        initial.path(),
        TextSpec::mono(11.0, 400, tok("color.text.dim")),
    );
    world.entity_mut(path).insert(ViewportHeaderField::Path);

    let header_spacer = spacer(world, "#VpHeaderSpacer");

    // The size badge chip (Geist Mono 11 / 500, muted), padding 3px 8px, radius 5.
    let size_text = text_leaf(
        world,
        "#VpSizeBadge",
        initial.size_badge(),
        TextSpec::mono(11.0, 500, tok("color.text.muted")),
    );
    world
        .entity_mut(size_text)
        .insert(ViewportHeaderField::Size);
    let size_badge = chip(
        world,
        "#VpSizeChip",
        Edges::axis(8.0, 3.0),
        5.0,
        "color.surface.inset",
        "color.border.default",
        0.0,
    );
    world.entity_mut(size_badge).add_children(&[size_text]);

    let header = world
        .spawn((
            Node,
            Name::new("#ViewportHeader"),
            Style::default()
                .flex_row()
                .height_px(42.0)
                .align_items(AlignItems::Center)
                .gap_px(12.0)
                .padding_edges(Edges::axis(16.0, 0.0))
                .border_edges(Edges {
                    top: Length::px(0.0),
                    right: Length::px(0.0),
                    bottom: Length::px(1.0),
                    left: Length::px(0.0),
                }),
            Background {
                color: tok("color.surface.chrome-translucent"),
            },
            Border {
                bottom: solid_side("color.border.subtle"),
                ..Default::default()
            },
            // backdrop-filter: blur(6px) — the translucent header blurs the dotted
            // canvas scrolling beneath it (B4 dual-Kawase; window-parent path).
            BackdropFilter(vec![FilterFn::Blur(Length::px(6.0))]),
        ))
        .id();
    world
        .entity_mut(header)
        .add_children(&[name, path, header_spacer, size_badge]);
    header
}

/// The right Inspector pane **panel + header** (the gear + "INSPECTOR" label).
/// The four content sections (name/desc, "Composed of" chips, "Live state" rows,
/// accent swatches) are filled by
/// [`inspector::build_inspector_content`](crate::inspector::build_inspector_content)
/// (Wave C4), called right after `build_shell`. Width 280px, `flex:none`, column,
/// a left 1px divider, bg `color.surface.chrome` (HTML 401, values.md § 7.1).
fn build_inspector(world: &mut World) -> Entity {
    // The gear icon (values.md § 6 #22, stroke 1.5, accent-tinted) + the
    // "INSPECTOR" uppercase mono label (Geist Mono 10 / 500 / 1.40px, muted).
    let gear = icon_box(
        world,
        "#InspectorGear",
        "M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7M19.4 12a7.6 7.6 0 0 0-.1-1.3l2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2.2-1.3l-.4-2.5H10l-.4 2.5a7.6 7.6 0 0 0-2.2 1.3l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0 0 2.6l-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2.2 1.3l.4 2.5h4l.4-2.5a7.6 7.6 0 0 0 2.2-1.3l2.4 1 2-3.4-2-1.6c.07-.43.1-.86.1-1.3",
        1.5,
        15,
        false,
        tok("color.accent"),
    );
    let label = text_leaf(
        world,
        "#InspectorLabel",
        "INSPECTOR",
        TextSpec::mono(10.0, 500, tok("color.text.muted")).ls(1.40),
    );
    let header = flex_box(
        world,
        "#InspectorHeader",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .gap_px(8.0)
            .padding_edges(Edges {
                top: Length::px(16.0),
                right: Length::px(16.0),
                bottom: Length::px(10.0),
                left: Length::px(16.0),
            }),
    );
    world.entity_mut(header).add_children(&[gear, label]);

    let inspector = world
        .spawn((
            Node,
            Inspector,
            Name::new("#Inspector"),
            Style::default()
                .flex_column()
                .width_px(280.0)
                .min_height(Sizing::Length(Length::px(0.0)))
                .overflow_y(OverflowMode::Auto)
                .border_edges(Edges {
                    top: Length::px(0.0),
                    right: Length::px(0.0),
                    bottom: Length::px(0.0),
                    left: Length::px(1.0),
                }),
            Background {
                color: tok("color.surface.chrome"),
            },
            Border {
                left: solid_side("color.border.subtle"),
                ..Default::default()
            },
        ))
        .id();
    world.entity_mut(inspector).add_children(&[header]);
    inspector
}

/// The 28px status bar (HTML 449, values.md § 7.1): `flex:none`, gap 16, padding
/// 0/14, a top 1px divider, bg `color.surface.chrome`. Holds the green "ready"
/// dot + label, a `|` separator, the active screen path, a `flex:1` spacer, a
/// right status note, another `|`, and the version. (C4 wires the live values;
/// C1 ships the static frame with the default screen's path.)
fn build_status_bar(world: &mut World) -> Entity {
    let initial = Screen::default();

    // The green "ready" dot (7×7, radius 99, glow via BoxShadow) + label.
    let dot = world
        .spawn((
            Node,
            Name::new("#ReadyDot"),
            Style::default().width_px(7.0).height_px(7.0),
            Background {
                color: tok("color.status.ok"),
            },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            BoxShadow(vec![Shadow {
                color: tok("color.status.ok"),
                offset_x: Length::px(0.0),
                offset_y: Length::px(0.0),
                blur: Length::px(6.0),
                spread: Length::px(0.0),
                inset: false,
            }]),
            Pickable::IGNORE,
        ))
        .id();
    let ready_label = text_leaf(
        world,
        "#ReadyLabel",
        "ready",
        TextSpec::mono(11.0, 500, tok("color.status.ok")),
    );
    let ready = flex_box(
        world,
        "#ReadyGroup",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .gap_px(6.0),
    );
    world.entity_mut(ready).add_children(&[dot, ready_label]);

    let sep1 = status_sep(world);
    let path = text_leaf(
        world,
        "#StatusPath",
        initial.path(),
        TextSpec::mono(11.0, 500, tok("color.text.muted")),
    );
    world.entity_mut(path).insert(ViewportHeaderField::Path);
    let status_spacer = spacer(world, "#StatusSpacer");
    let right = text_leaf(
        world,
        "#StatusRight",
        "inspector on",
        TextSpec::mono(11.0, 500, tok("color.text.dim")),
    );
    let sep2 = status_sep(world);
    let version = text_leaf(
        world,
        "#StatusVersion",
        "buiy 0.3.0",
        TextSpec::mono(11.0, 500, tok("color.text.muted")),
    );

    let status = world
        .spawn((
            Node,
            Name::new("#StatusBar"),
            Style::default()
                .flex_row()
                .height_px(28.0)
                .align_items(AlignItems::Center)
                .gap_px(16.0)
                .padding_edges(Edges::axis(14.0, 0.0))
                .border_edges(Edges {
                    top: Length::px(1.0),
                    right: Length::px(0.0),
                    bottom: Length::px(0.0),
                    left: Length::px(0.0),
                }),
            Background {
                color: tok("color.surface.chrome"),
            },
            Border {
                top: solid_side("color.border.subtle"),
                ..Default::default()
            },
        ))
        .id();
    world.entity_mut(status).add_children(&[
        ready,
        sep1,
        path,
        status_spacer,
        right,
        sep2,
        version,
    ]);
    status
}

/// A `|` status-bar separator (Geist Mono 11 / 500, `text.dimmer`).
fn status_sep(world: &mut World) -> Entity {
    text_leaf(
        world,
        "#StatusSep",
        "|",
        TextSpec::mono(11.0, 500, tok("color.text.dimmer")),
    )
}

// ===========================================================================
// Mounting the 5 screens (spawn-all-once: Candidate A)
// ===========================================================================

/// Spawn all 5 screen subtrees ONCE, tag each root with a [`ScreenRoot`], re-home
/// them under `#ScreenContent`, and apply the initial router state (everything but
/// the default screen is `Display::None` + `A11yHidden`). Called at boot, after
/// [`build_shell`]; seeds the full [`SCROLL_LIST_ROWS`] (1000) scroll rows. Mirrors
/// the existing per-screen `setup_*` seeding but for the unified shell.
pub fn mount_screens(world: &mut World) {
    mount_screens_with(world, SCROLL_LIST_ROWS);
}

/// [`mount_screens`] with the S2 scroll-row count parameterized — the layout
/// snapshot test seeds a small set (the structure is what is pinned; the 1000-row
/// scale-game has its own driver acceptance), the binary seeds the full 1000.
pub fn mount_screens_with(world: &mut World, scroll_rows: usize) {
    let Some(slot) = find_named_screen_content(world) else {
        return;
    };

    let roots = [
        (Screen::Todo, spawn_todo_screen(world)),
        (Screen::Scroll, mount_scroll_screen(world, scroll_rows)),
        (Screen::Menu, spawn_menu_screen(world)),
        (Screen::Modal, spawn_modal_screen(world)),
        (Screen::Showcase, spawn_showcase_screen(world)),
    ];
    for (screen, root) in roots {
        // Capture the screen's authored `Display` so the router can restore it when
        // re-activating (the `Display::None` hide is a non-destructive overwrite;
        // the captured value is the truth to put back).
        let authored = world.get::<Display>(root).copied().unwrap_or_default();
        world
            .entity_mut(root)
            .insert((ScreenRoot(screen), ScreenAuthoredDisplay(authored)));
        world.entity_mut(slot).add_child(root);
    }

    // Seed the initial router state so the inactive 4 are `Display::None` from the
    // first frame (no flash of all-5-stacked).
    let active = world.resource::<ScreenRouter>().0;
    set_active_screen(world, active);
}

/// The single `#ScreenContent` slot entity.
fn find_named_screen_content(world: &mut World) -> Option<Entity> {
    let mut q = world.query_filtered::<Entity, With<ScreenContent>>();
    q.iter(world).next()
}

/// Spawn S1 (TodoMVC) + seed its demo rows; return the screen root (`#TodoScreen`).
fn spawn_todo_screen(world: &mut World) -> Entity {
    let root = spawn_todomvc_screen(world);
    for &(label, completed) in DEMO_SEEDS {
        append_row(world, label, completed);
    }
    root
}

/// Spawn S2 (the entity-tree Virtual List) + seed `rows` rows; return the screen
/// root (`#ScrollScreen`). The screen is built imperatively (composite/icon-heavy
/// — `spawn_scroll_screen`), then the dynamic rows are seeded by `fill_scroll_list`.
fn mount_scroll_screen(world: &mut World, rows: usize) -> Entity {
    let root = spawn_scroll_screen(world);
    fill_scroll_list(world, rows);
    root
}

/// Spawn S3 (overlay menu); return the screen root (`#MenuScreen`).
/// [`spawn_overlay_menu`] builds the design's file card (the ⋮ `MenuButton` + its
/// 5-item dropdown + the footer last-action strip) under a centering wrap, plus the
/// catalog's other two overlay primitives (the `TooltipTrigger` + the standalone
/// `Popover`) in a non-painting holder so the C8b acceptance's drivers keep their
/// targets. The whole tree rides the router toggle from the returned root.
fn spawn_menu_screen(world: &mut World) -> Entity {
    spawn_overlay_menu(world)
}

/// Spawn S4 (modal). `spawn_modal` builds the invoker+background under a
/// window-sized `#ModalRoot` (the dialog is a separate top-layer root). The router
/// toggles `#ModalRoot`; the dialog rides `WidgetsPlugin`'s open/close lifecycle
/// (it starts `CssVisibility::Hidden` and is only shown when the invoker fires).
fn spawn_modal_screen(world: &mut World) -> Entity {
    let (invoker, _dialog, _bg) = spawn_modal(world);
    // The `#ModalRoot` is the invoker's parent (the window-sized container
    // `spawn_modal` builds). Walk up to it so the router toggles the whole pane.
    ancestor_root(world, invoker)
}

/// Spawn S5 (the Controls showcase); return the screen root (`#ShowcaseScreen`).
/// [`spawn_showcase`] builds the design's 2-column controls grid imperatively
/// (icon-heavy — like the other C-screens), recording the meter fill in
/// [`ShowcaseBuild`](crate::ShowcaseBuild) so the "Run build" button can animate it.
fn spawn_showcase_screen(world: &mut World) -> Entity {
    spawn_showcase(world)
}

/// Walk up from `e` to its top-most ancestor (the screen-root container).
fn ancestor_root(world: &World, e: Entity) -> Entity {
    let mut cur = e;
    for _ in 0..16 {
        match world.get::<bevy::prelude::ChildOf>(cur) {
            Some(c) => cur = c.parent(),
            None => break,
        }
    }
    cur
}

// ===========================================================================
// The router applier + nav wiring + viewport-header reflect
// ===========================================================================

/// Apply the active screen to every [`ScreenRoot`]: the active root restores its
/// authored `Display` and clears `A11yHidden`; the inactive ones get both
/// `Display::None` and `A11yHidden`. `Display::None` prunes the whole subtree from
/// Taffy (zero layout cost for hidden screens); `A11yHidden` prunes them from the
/// a11y tree and makes their focusables inert (Tab skips them). Idempotent — safe
/// to call every switch.
fn set_active_screen(world: &mut World, active: Screen) {
    let roots: Vec<(Entity, Screen, Display)> = {
        let mut q = world.query::<(Entity, &ScreenRoot, &ScreenAuthoredDisplay)>();
        q.iter(world).map(|(e, r, d)| (e, r.0, d.0)).collect()
    };
    for (root, screen, authored) in roots {
        if screen == active {
            world
                .entity_mut(root)
                .remove::<A11yHidden>()
                .insert(authored);
        } else {
            world.entity_mut(root).insert((A11yHidden, Display::None));
        }
    }
}

/// The exclusive router applier (C8 §2.5(1) ordering): drain [`SwitchScreen`]
/// messages, update [`ScreenRouter`], and (if it changed) re-toggle the screen
/// roots via `set_active_screen`. Runs `.after(BuiySet::Input).before(
/// BuiySet::A11yUpdate)` so a nav press THIS frame switches the screen + the
/// `A11yHidden` toggle lands in the SAME frame's a11y-tree rebuild (no stale
/// frame where a hidden screen lingers in the tree).
pub fn apply_screen_router(world: &mut World) {
    // Read the last requested switch (the design's nav is single-select; if two
    // arrive in one frame the last wins — mirrors `apply_set_accent`).
    let requested = world
        .resource_mut::<bevy::ecs::message::Messages<SwitchScreen>>()
        .drain()
        .map(|SwitchScreen(s)| s)
        .last();
    let Some(target) = requested else {
        return;
    };
    // W3: fold the switch THROUGH the MVU `NavModel` (the recorded source of truth)
    // via the synchronous inline seam, then project onto the legacy `ScreenRouter`.
    // Lazy-spawn the model seeded to the current router so every harness driving
    // this applier gets it without extra wiring (tests don't add ScreenRouterPlugin).
    let nav = {
        let mut q = world.query_filtered::<Entity, With<NavModel>>();
        match q.iter(world).next() {
            Some(e) => e,
            None => {
                let cur = world.resource::<ScreenRouter>().0;
                world.spawn((NavModel(cur), Name::new("#NavModel"))).id()
            }
        }
    };
    // `set_if_neq` in the fold ⇒ `changed` is false when target == current (the
    // original early-return, now expressed through the model).
    let changed = fold_one_inline::<NavModel>(world, nav, NavMsg::Switch(target), nav_reducer);
    if !changed {
        return;
    }
    let screen = world.get::<NavModel>(nav).map(|m| m.0).unwrap_or(target);
    world.resource_mut::<ScreenRouter>().0 = screen; // the projection consumers read
    set_active_screen(world, screen);
}

/// Map a rail nav button's `OnPress` to a [`SwitchScreen`] request: read the
/// pressed entity's [`ScreenNav`] and write the switch. Ordinary `.after(Input)`
/// `MessageReader` system (the C8 intent-collector pattern); the exclusive
/// [`apply_screen_router`] consumes the request.
pub fn route_nav_press(
    mut reader: MessageReader<OnPress>,
    navs: Query<&ScreenNav>,
    mut writer: MessageWriter<SwitchScreen>,
) {
    for OnPress(e) in reader.read() {
        if let Ok(ScreenNav(screen)) = navs.get(*e) {
            writer.write(SwitchScreen(*screen));
        }
    }
}

/// Reflect the active [`ScreenRouter`] into the viewport-header + status-bar
/// labels (the screen name / source path / size badge). A change-detection system
/// (runs only when the router changed) so it is free at rest. Keeps the header
/// content in sync with whatever set the router (a nav press, or a future
/// programmatic switch). The rail nav-button **active state** (the accent bar/idx/
/// icon/name + card bg) is the sibling [`reflect_rail_active_state`]; the screen
/// subtree toggle is `set_active_screen`.
pub fn reflect_active_screen(
    router: Res<ScreenRouter>,
    mut fields: Query<(&ViewportHeaderField, &mut Text)>,
) {
    if !router.is_changed() {
        return;
    }
    let s = router.0;
    for (field, mut text) in &mut fields {
        let next = match field {
            ViewportHeaderField::Name => s.name().to_string(),
            ViewportHeaderField::Path => s.path().to_string(),
            ViewportHeaderField::Size => s.size_badge().to_string(),
        };
        if text.0 != next {
            text.0 = next;
        }
    }
}

/// Reflect the active [`ScreenRouter`] into the **rail nav-button active state**
/// (the C4 fix — C1/C3 hard-coded the active visual to the boot screen; this
/// follows the router on every switch). For each nav button, the active one gets
/// the `surface.card` bg + the accent bar/idx + bright icon/name; the inactive
/// ones get the transparent bg + dim bar/idx + muted icon/name (the design's
/// `SCR` map, JS 560–564). An exclusive change-detection system — free at rest,
/// a handful of paint writes per switch. Walks each `ScreenNav` button → its
/// `NavPart` children, so the parts need no per-screen tag of their own.
pub fn reflect_rail_active_state(world: &mut World) {
    if !world.is_resource_changed::<ScreenRouter>() {
        return;
    }
    let active = world.resource::<ScreenRouter>().0;

    // Each nav button + its children (the bar/idx/icon/name parts).
    let buttons: Vec<(Entity, Screen, Vec<Entity>)> = {
        let mut q = world.query::<(Entity, &ScreenNav, &bevy::prelude::Children)>();
        q.iter(world)
            .map(|(e, nav, ch)| (e, nav.0, ch.iter().copied().collect()))
            .collect()
    };

    for (button, screen, children) in buttons {
        let is_active = screen == active;
        // The button card bg (`surface.card` active / transparent not).
        let bg = if is_active {
            tok("color.surface.card")
        } else {
            tok("color.surface.transparent")
        };
        if let Some(mut b) = world.get_mut::<Background>(button)
            && b.color != bg
        {
            b.color = bg;
        }
        // Each tagged part's active/inactive paint.
        for child in children {
            let Some(part) = world.get::<NavPart>(child).copied() else {
                continue;
            };
            match part {
                NavPart::Bar => {
                    let color = if is_active {
                        tok("color.accent")
                    } else {
                        tok("color.surface.transparent")
                    };
                    if let Some(mut b) = world.get_mut::<Background>(child)
                        && b.color != color
                    {
                        b.color = color;
                    }
                }
                NavPart::Idx => set_text_color(
                    world,
                    child,
                    if is_active {
                        "color.accent"
                    } else {
                        "color.text.dim"
                    },
                ),
                NavPart::Icon => {
                    // The icon's tint lives on its `Icon.color` (a coverage glyph).
                    let color = if is_active {
                        tok("color.text.primary")
                    } else {
                        tok("color.text.muted")
                    };
                    if let Some(mut icon) = world.get_mut::<Icon>(child)
                        && icon.color != color
                    {
                        icon.color = color;
                    }
                }
                NavPart::Name => set_text_color(
                    world,
                    child,
                    if is_active {
                        "color.text.primary"
                    } else {
                        "color.text.secondary"
                    },
                ),
            }
        }
    }
}

/// Set a text leaf's `TextColor` to a token (only on change).
fn set_text_color(world: &mut World, leaf: Entity, token: &str) {
    let want = tok(token);
    if let Some(mut c) = world.get_mut::<buiy_core::render::components::TextColor>(leaf)
        && c.0 != want
    {
        c.0 = want;
    }
}

// ===========================================================================
// The boot setup + the ScreenRouterPlugin
// ===========================================================================

/// The startup system the binary + the capture/test paths call: a camera, the
/// dark theme, the shell tree, the 5 screens mounted under `#ScreenContent`, and
/// the initial router state. (The dark-theme insert + the screen plugins are added
/// by the binary/test harness; this only builds the world tree.)
pub fn setup_shell(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.queue(|world: &mut World| {
        build_shell(world);
        mount_screens(world);
        // Fill the C1 inspector stub with the C4 content (name/desc + composed-of
        // + live-state + accent swatches) for the boot screen.
        crate::inspector::build_inspector_content(world);
    });
}

/// The shell router app logic: the [`ScreenRouter`] resource, the [`SwitchScreen`]
/// message, the nav-press collector, the exclusive applier, and the chrome-reflect
/// system. The applier is `.after(BuiySet::Input).before(BuiySet::A11yUpdate)` so a
/// switch THIS frame lands in the SAME frame's a11y rebuild. The per-screen app
/// plugins (`TodoMvcPlugin` / `OverlayMenuPlugin`) are added by the binary
/// alongside this — they run globally and no-op on the input-starved hidden screens.
pub struct ScreenRouterPlugin;

impl Plugin for ScreenRouterPlugin {
    fn build(&self, app: &mut App) {
        // W3: register the MVU nav model (record/replay-readiness). The applier
        // lazy-spawns the `NavModel` entity, so no Startup spawn is needed here.
        app.register_type::<Screen>()
            .register_type::<NavModel>()
            .add_model::<NavModel>();
        app.init_resource::<ScreenRouter>()
            .add_message::<SwitchScreen>()
            .add_systems(Startup, setup_shell)
            .add_systems(
                Update,
                (
                    route_nav_press,
                    apply_screen_router,
                    reflect_active_screen,
                    reflect_rail_active_state,
                )
                    .chain()
                    .after(BuiySet::Input)
                    .before(BuiySet::A11yUpdate),
            );
    }
}
