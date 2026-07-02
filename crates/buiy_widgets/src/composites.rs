//! `buiy_widgets::composites` — the **general composite builders**: small,
//! reusable trees of styled boxes that compose the primitive widgets/render
//! components into a recognizable higher-level control. Promoted out of the
//! parity gallery (Wave 5 refinement) once they proved genuinely general — a
//! caller anywhere can drop a [`meter`], a [`table_row`], a [`search_input`], a
//! [`kbd`] chip, a [`status_dot`], or attach a [`pulse_blink`].
//!
//! These are imperative `World`-spawning builders (`fn(world: &mut World, …) ->
//! Entity`), not declarative `bsn!` scene-fns: a composite is a small static tree
//! of one-off styled boxes that reads cleaner spawned with the [`Style`] builder +
//! the module-local helpers than as one giant `bsn!` block. (The marker-driven
//! widgets in this crate stay declarative; this module is the complementary
//! imperative-composite layer.)
//!
//! ## Font-neutral by contract
//!
//! Like the widget scene-fns, the text-bearing composites here never hard-code a
//! font *family* — that is a brand/registration choice the app owns. Each takes a
//! [`FontFamily`] argument (the mono/sans face to render with) and threads it into
//! its leaves; the font *size*/*weight* are the composite's own design constants.
//! A general UI framework must not force a typeface on every app, the same way it
//! must not force a theme — so the caller supplies it.
//!
//! ## Two builders return more than an entity
//!
//! - [`meter`] returns `(track, fill)`: a progress fill animates its width, and
//!   the rule is **never animate a Taffy-owned length per frame** — so the fill is
//!   authored at the **full track width** and a left-anchored [`ScaleTween`] on its
//!   X axis grows it `0 → pct` (transform-only, masked by the track's
//!   `overflow:hidden`). [`set_meter`] re-targets the scale from the current value.
//!   A decomposed [`Scale`] pivots about the box top-left
//!   corner, so the full-width fill grows from the LEFT edge for free.
//! - [`table_row`] returns the row entity and, when `selected`, parents a
//!   [`RowSelBar`] accent bar; [`set_table_row_selected`] adds/removes it.

use bevy::picking::Pickable;
use bevy::prelude::{Children, Component, Entity, Name, Vec3, World};
use bevy::scene::WorldSceneExt;

use buiy_core::animation::{Easing, OpacityTween, Repeat, ScaleTween, Tween};
use buiy_core::components::Node;
use buiy_core::layout::{
    AlignItems, Edges, FlexItem, Inset, JustifyContent, Length, Scale, Sizing, Style,
};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{
    Background, BackgroundLayer, BackgroundLayers, Border, BorderSide, BoxShadow, ColorStop,
    Corners, Icon, LineStyle, LinearGradient, Radius, Shadow, TextColor,
};
use buiy_core::text::{FontFamily, FontSize, FontWeight, LetterSpacing, Text, TextAlign};

use crate::scene::text_input_single_line;

// ===========================================================================
// Shared authoring helpers (module-local: the imperative composite-builder
// vocabulary these composites are spelled in). Font-NEUTRAL — `text_leaf` takes
// the family as an argument so this module never hard-codes a typeface.
// ===========================================================================

/// A `ColorToken::Token` from a `&str` key (every paint is a named theme token,
/// so the forced-colors discipline stays enforceable).
fn tok(key: &str) -> ColorToken {
    ColorToken::Token(key.to_string().into())
}

/// A solid 1px [`BorderSide`] of a token color.
fn solid_side(token: &str) -> BorderSide {
    BorderSide {
        color: tok(token),
        style: LineStyle::Solid,
    }
}

/// A uniform 1px [`Border`] of `token` with `radius` rounded corners.
fn border_all(token: &str, radius: f32) -> Border {
    Border {
        top: solid_side(token),
        right: solid_side(token),
        bottom: solid_side(token),
        left: solid_side(token),
        radius: Corners::all(Radius::circular(radius)),
    }
}

/// A leaf text node: `Text` + `FontSize`/`FontFamily`/`FontWeight`/`TextColor`,
/// an optional [`LetterSpacing`], and `Pickable::IGNORE` (composite labels are
/// decorative pixels — clicks fall through to the owning control). `family` is the
/// caller-supplied typeface (the module never hard-codes a font).
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
/// stroked/filled to a coverage glyph, tinted `color`), `Pickable::IGNORE`.
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

/// A token-filled box with a uniform 1px border + radius + flex layout. The
/// generic "card / chip / track" body the composites share. Children are added by
/// the caller.
fn box_node(world: &mut World, name: &str, style: Style, bg: &str, border: Border) -> Entity {
    world
        .spawn((
            Node,
            Name::new(name.to_string()),
            style,
            Background { color: tok(bg) },
            border,
        ))
        .id()
}

// ===========================================================================
// meter(width, pct) — an animated left-anchored progress fill
// ===========================================================================

/// Marks a meter's animated fill (so [`set_meter`] can re-target its scale).
#[derive(Component, Clone, Copy)]
pub struct MeterFill;

/// A progress meter: a track (height 8, radius 99, bg `surface.raised-alt`,
/// `overflow:hidden`) + a fill (the 90deg accent gradient `accent → accent.lighter`,
/// radius 99). The fill is authored at the **full track width** and grown from the
/// LEFT via an X [`ScaleTween`] `0 → pct` with the design easing
/// (`cubic-bezier(.2,.8,.2,1)`, ~0.3s); this is **transform-only** (never a
/// per-frame Taffy width). The track's `overflow:hidden` masks the un-filled
/// remainder. `width` is the track width; `pct` is the initial fill fraction in
/// `[0, 1]`. Returns `(track, fill)`; pass `fill` to [`set_meter`] to animate to a
/// new fraction.
pub fn meter(world: &mut World, width: f32, pct: f32) -> (Entity, Entity) {
    let pct = pct.clamp(0.0, 1.0);
    // The fill spans the full track width; the X scale is the fraction shown.
    let fill = world
        .spawn((
            Node,
            MeterFill,
            Name::new("#MeterFill"),
            Style::default()
                .width(Sizing::Length(Length::percent(100.0)))
                .height(Sizing::Length(Length::percent(100.0))),
            // The 90deg accent gradient (opaque stops).
            BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                angle_deg: 90.0,
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
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            // The resting X scale = the initial fill fraction. A decomposed `Scale`
            // pivots about the box-LOCAL ORIGIN (the top-left corner), so
            // `Scale(pct, 1, 1)` grows the full-width fill from the LEFT edge:
            // exactly a left-anchored progress fill, with NO transform-origin
            // needed (`compose_transform` is `T·R·S·M`, origin-free, and the corner
            // pivot is what we want here).
            Scale(pct, 1.0, 1.0),
            Pickable::IGNORE,
        ))
        .id();

    let track = box_node(
        world,
        "#MeterTrack",
        Style::default()
            .width_px(width)
            .height_px(8.0)
            .overflow_hidden()
            .relative(),
        "color.surface.raised-alt",
        Border {
            radius: Corners::all(Radius::circular(99.0)),
            ..Default::default()
        },
    );
    world.entity_mut(track).add_child(fill);
    (track, fill)
}

/// The meter's fill animation duration (`width .3s`).
const METER_TWEEN_SECS: f32 = 0.3;

/// Animate a meter's fill to a new fraction `pct` (in `[0, 1]`) with the design
/// easing ([`Easing::DESIGN`], ~0.3s). Reads the fill's current X scale as the
/// tween start (so re-targeting mid-flight is continuous), and attaches a
/// left-anchored [`ScaleTween`]. Transform-only — no Taffy relayout.
pub fn set_meter(world: &mut World, fill: Entity, pct: f32) {
    let pct = pct.clamp(0.0, 1.0);
    let from = world.get::<Scale>(fill).map(|s| s.0).unwrap_or(0.0);
    world.entity_mut(fill).insert(ScaleTween(Tween::secs(
        Vec3::new(from, 1.0, 1.0),
        Vec3::new(pct, 1.0, 1.0),
        METER_TWEEN_SECS,
        Easing::DESIGN,
    )));
}

// ===========================================================================
// search_input(placeholder, font, width) — a TextInput + leading search icon
// ===========================================================================

/// The search FIELD's content-box height inside the 34px search row. The row is
/// 34px tall (1px border → ~32px inner); a 22px field comfortably fits a 13px sans
/// line, and `align-items:center` on the row vertically centers it (replacing the
/// `TextInput` default 32px+8px-padding box that would overflow the row by 6px).
const SEARCH_FIELD_H: f32 = 22.0;

/// A 34px-tall search row: `padding:0 11px`, `gap:8px`, 1px `border.default`,
/// radius 8, bg `surface.card`. A leading search [`Icon`]
/// (`M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M20 20l-4-4`, stroke 1.7, size 15,
/// `text.dim`) + a single-line `text_input`-style field rendered in `font`
/// (the caller's sans face, 13/450). The field is a real
/// [`text_input_single_line`] so it is
/// focusable + editable; the row supplies the chrome. Returns the row root.
/// `width` sizes the row.
pub fn search_input(world: &mut World, placeholder: &str, font: FontFamily, width: f32) -> Entity {
    let glyph = icon_box(
        world,
        "#SearchIcon",
        "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M20 20l-4-4",
        1.7,
        15,
        ColorToken::TextDim,
    );
    // A real single-line text field (focusable + editable), grown to fill the row.
    // The `TextInput` `#[require]` box model is 200×32 + 8px padding → a 48px
    // border-box that overflows this 34px row by 6px. Override it to a slim box
    // that fits: the row owns the chrome + the 11px side padding, so the field
    // carries NO vertical padding and a fixed `SEARCH_FIELD_H` height that, with
    // `align-items:center` on the row, sits centered inside the 34px field with
    // text baseline-centered. Horizontal padding stays 0 (the row's `padding:0 11px`
    // + the icon/gap already inset the text).
    let field = world
        .spawn_scene(text_input_single_line(placeholder))
        .expect("spawn search field")
        .id();
    world.entity_mut(field).insert((
        Name::new("#SearchField"),
        FontSize(13.0),
        font,
        FontWeight(450),
        Style::default()
            .height_px(SEARCH_FIELD_H)
            .padding_edges(Edges::axis(0.0, 0.0))
            .box_model,
        FlexItem {
            grow: 1.0,
            ..Default::default()
        },
    ));

    let row = box_node(
        world,
        "#SearchInput",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .gap_px(8.0)
            .width_px(width)
            .height_px(34.0)
            .padding_edges(Edges::axis(11.0, 0.0))
            .border(1.0),
        "color.surface.card",
        border_all("color.border.default", 8.0),
    );
    world.entity_mut(row).add_children(&[glyph, field]);
    row
}

// ===========================================================================
// kbd(key, font) — a keyboard-shortcut chip (⌘ as a vector icon)
// ===========================================================================

/// The macOS Command symbol (⌘, U+2318 "Place of Interest Sign") as a stroke
/// vector path — the four corner loops + connecting square (the Lucide `command`
/// glyph). A `<kbd>` that authors the literal `⌘` leans on the BROWSER's system
/// font fallback; a registered-only font system (Geist / Geist Mono / Fira, NONE
/// of which carry U+2318) would tofu it, so rendering ⌘ as a real [`Icon`] is the
/// faithful, font-independent fix.
pub const CMD_GLYPH_ICON: &str = "M18 3a3 3 0 0 0-3 3v12a3 3 0 0 0 3 3 3 3 0 0 0 3-3 3 3 0 0 0-3-3H6a3 3 0 0 0-3 3 3 3 0 0 0 3 3 3 3 0 0 0 3-3V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3 3 3 0 0 0 3 3h12a3 3 0 0 0 3-3 3 3 0 0 0-3-3";

/// Build the CONTENT of a keyboard-shortcut chip (`mono` 10 / 500, `color`).
/// A shortcut beginning with the macOS Command symbol (`⌘D`, `⌘K`, …) would tofu
/// in a registered-only font system, so its content becomes a tight flex-row of a
/// vector ⌘ [`Icon`] ([`CMD_GLYPH_ICON`]) + the remaining text as `mono`. A plain
/// shortcut (`↵`, `F2`, `⌫`) stays a single mono leaf. Shared by the [`kbd`]
/// composite AND any menu-item kbd so ⌘ renders identically everywhere. Returns
/// the content root (caller parents it into the chip / row).
pub fn kbd_content(
    world: &mut World,
    name: &str,
    key: &str,
    mono: FontFamily,
    color: ColorToken,
) -> Entity {
    let Some(rest) = key.strip_prefix('⌘') else {
        return text_leaf(world, name, key, mono, 10.0, 500, color, None);
    };
    // ⌘ + text: a flex-row [vector ⌘ icon][text]. The 11px icon box matches the
    // 10px cap-height; a 1px gap keeps the symbol and text visually adjacent.
    let cmd = icon_box(
        world,
        "#KbdCmdGlyph",
        CMD_GLYPH_ICON,
        1.5,
        11,
        color.clone(),
    );
    let text = text_leaf(world, "#KbdCmdText", rest, mono, 10.0, 500, color, None);
    world
        .spawn((
            Node,
            Name::new(name.to_string()),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(1.0),
            Pickable::IGNORE,
        ))
        .add_children(&[cmd, text])
        .id()
}

/// A keyboard-key chip: `mono` 10 / 500 `text.dim`, 1px `border.default`, radius 5,
/// `padding:3px 6px`, bg `surface.inset`. A `⌘`-prefixed key renders the Command
/// symbol as a vector icon. `mono` is the caller's monospace face. Returns the kbd
/// root.
pub fn kbd(world: &mut World, key: &str, mono: FontFamily) -> Entity {
    let text = kbd_content(world, "#KbdGlyph", key, mono, ColorToken::TextDim);
    let k = box_node(
        world,
        "#Kbd",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .padding_edges(Edges::axis(6.0, 3.0))
            .border(1.0),
        "color.surface.inset",
        border_all("color.border.default", 5.0),
    );
    world.entity_mut(k).add_children(&[text]);
    k
}

// ===========================================================================
// status_dot(color, glow, blur, spread) — a glowing status indicator
// ===========================================================================

/// A 7×7 glowing status dot (radius 99): fill `color`, with a glow via a
/// [`BoxShadow`] (`0 0 <blur>px <glow_color>`). `spread_px` is a ring spread (a
/// "ready" dot uses 0; a blink dot uses a 4px soft ring); `blur_px` is the glow
/// blur (a steady glow = 6, a ring = 0). Returns the dot root.
pub fn status_dot(
    world: &mut World,
    color: &str,
    glow_color: &str,
    blur_px: f32,
    spread_px: f32,
) -> Entity {
    world
        .spawn((
            Node,
            Name::new("#StatusDot"),
            Style::default().width_px(7.0).height_px(7.0),
            Background { color: tok(color) },
            Border {
                radius: Corners::all(Radius::circular(99.0)),
                ..Default::default()
            },
            BoxShadow(vec![Shadow {
                color: tok(glow_color),
                offset_x: Length::px(0.0),
                offset_y: Length::px(0.0),
                blur: Length::px(blur_px),
                spread: Length::px(spread_px),
                inset: false,
            }]),
            Pickable::IGNORE,
        ))
        .id()
}

/// One half-cycle of an `blink 1.6s infinite` pulse (opacity `1`→`.25`): 1.6 s full
/// cycle ⇒ 0.8 s each one-way pass.
const BLINK_HALF_SECS: f32 = 0.8;
/// The dimmed opacity the blink pulse reaches at its low point (CSS `blink` to
/// `opacity:.25`). `Opacity < 1` auto-forms an `EffectGroup` (render
/// `effect.rs::effect_reason_for`), so the dot composites + dims with no manual
/// group authoring — and ping-pongs back to a steady-lit `1.0`.
const BLINK_LOW_OPACITY: f32 = 0.25;

/// Attach an infinite blink/pulse to a status dot: an [`OpacityTween`] that
/// ping-pongs opacity `1`→`.25`→`1` forever ([`Repeat::PingPong`] with no count).
/// Under reduced motion the tween snaps to the steady bright rest state (`1.0`) and
/// removes itself — a pulse never oscillates (the per-target system reads
/// `prefers_reduced_motion`). Single-frame captures don't show the pulse, but the
/// live app pulses.
pub fn pulse_blink(world: &mut World, dot: Entity) {
    world.entity_mut(dot).insert(OpacityTween(
        Tween::secs(1.0, BLINK_LOW_OPACITY, BLINK_HALF_SECS, Easing::EASE)
            .with_repeat(Repeat::PingPong { count: None }),
    ));
}

// ===========================================================================
// table_row(data, font, selected) + table_header(cols, font) — a columned row
// ===========================================================================

/// One table row's column data: the index, the type-dot color token, the node
/// type, the node name, the frame ms, and the state (OK/WARN/ERR). Mirrors a
/// per-row generated record.
#[derive(Clone)]
pub struct TableRowData<'a> {
    /// The row index label (mono, `text.dim`).
    pub idx: &'a str,
    /// The tree-indent width in px (`depth·13px`), rendered as a fixed-width spacer
    /// between the index and the dot so nested nodes step right.
    pub indent_px: f32,
    /// The type-dot color token.
    pub dot_color: &'a str,
    /// The node type cell (mono, `text.secondary`).
    pub node_type: &'a str,
    /// The node name cell (mono, `text.faint`, ellipsis — `flex:1`).
    pub name: &'a str,
    /// The frame-ms cell (mono, right-aligned; `text.faint` normal / `status.warn`
    /// when over threshold — the caller picks the token).
    pub ms: &'a str,
    /// Whether the ms cell is over the warn threshold (picks the ms token).
    pub ms_warn: bool,
    /// The state cell (OK/WARN/ERR) + its color token.
    pub state: &'a str,
    /// The state color token.
    pub state_color: &'a str,
}

/// Marks a table row (so [`set_table_row_selected`] can find + restyle it).
#[derive(Component, Clone, Copy)]
pub struct TableRow;

/// One columned table row rendered in `font` (the caller's monospace face): a
/// flex-row of column cells (idx 46 / dot 7×7 r2 / type / name `flex:1` ellipsis /
/// ms 66 right / state 42 right) at `height:34px`, `gap:8px`, `padding:0 12px`, a
/// bottom 1px `border.subtle-2`. When `selected`, bg `accent.soft` + a 2.5px accent
/// **inset-left bar** (rendered as an absolutely-positioned left bar). Returns the
/// row; carries [`TableRow`].
pub fn table_row(
    world: &mut World,
    data: &TableRowData,
    font: FontFamily,
    selected: bool,
) -> Entity {
    let idx = text_leaf(
        world,
        "#RowIdx",
        data.idx,
        font.clone(),
        11.0,
        500,
        ColorToken::TextDim,
        None,
    );
    world
        .entity_mut(idx)
        .insert(Style::default().width_px(46.0));

    // The tree-indent spacer (`depth·13px`): a fixed-width `flex:none` box (no fill)
    // so nested nodes step right. `Pickable::IGNORE` so a click resolves to the row.
    let indent = world
        .spawn((
            Node,
            Name::new("#RowIndent"),
            Style::default().width_px(data.indent_px.max(0.0)),
            FlexItem {
                shrink: 0.0,
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .id();

    let dot = world
        .spawn((
            Node,
            Name::new("#RowDot"),
            Style::default().width_px(7.0).height_px(7.0),
            Background {
                color: tok(data.dot_color),
            },
            Border {
                radius: Corners::all(Radius::circular(2.0)),
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .id();

    let node_type = text_leaf(
        world,
        "#RowType",
        data.node_type,
        font.clone(),
        12.5,
        500,
        ColorToken::TextSecondary,
        None,
    );

    let name = text_leaf(
        world,
        "#RowName",
        data.name,
        font.clone(),
        12.5,
        400,
        ColorToken::TextFaint,
        None,
    );
    world.entity_mut(name).insert((
        Style::default().overflow_hidden(),
        FlexItem {
            grow: 1.0,
            ..Default::default()
        },
    ));

    let ms_color = if data.ms_warn {
        ColorToken::StatusWarn
    } else {
        ColorToken::TextFaint
    };
    let ms = text_leaf(
        world,
        "#RowMs",
        data.ms,
        font.clone(),
        11.5,
        500,
        ms_color,
        None,
    );
    world.entity_mut(ms).insert((
        Style::default()
            .width_px(66.0)
            .flex_row()
            .justify_content(JustifyContent::FlexEnd),
        TextAlign::End,
    ));

    let state = text_leaf(
        world,
        "#RowState",
        data.state,
        font,
        10.0,
        500,
        tok(data.state_color),
        None,
    );
    world.entity_mut(state).insert((
        Style::default()
            .width_px(42.0)
            .flex_row()
            .justify_content(JustifyContent::FlexEnd),
        LetterSpacing(0.40),
        TextAlign::End,
    ));

    let bg = if selected {
        "color.accent.soft"
    } else {
        "color.surface.transparent"
    };
    let row = world
        .spawn((
            Node,
            TableRow,
            Name::new("#TableRow"),
            Style::default()
                .relative()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(8.0)
                .height_px(34.0)
                .padding_edges(Edges::axis(12.0, 0.0))
                .border_edges(Edges {
                    top: Length::px(0.0),
                    right: Length::px(0.0),
                    bottom: Length::px(1.0),
                    left: Length::px(0.0),
                }),
            Background { color: tok(bg) },
            Border {
                bottom: solid_side("color.border.subtle-2"),
                ..Default::default()
            },
        ))
        .add_children(&[idx, indent, dot, node_type, name, ms, state])
        .id();

    // The selected inset-left accent bar (`inset 2.5px 0 0 --ac`): an
    // absolutely-positioned 2.5px-wide accent bar on the left edge.
    if selected {
        let bar = spawn_row_sel_bar(world);
        world.entity_mut(row).add_child(bar);
    }
    row
}

/// Marks the absolutely-positioned 2.5px accent **inset-left bar** child of a
/// selected [`table_row`]. Kept as a marker so [`set_table_row_selected`] can find +
/// despawn it when a row deselects.
#[derive(Component, Clone, Copy)]
pub struct RowSelBar;

/// Spawn a row's selected inset-left accent bar (an absolutely-positioned 2.5px
/// `accent` box on the left edge, `Pickable::IGNORE` so a hit resolves to the row).
fn spawn_row_sel_bar(world: &mut World) -> Entity {
    world
        .spawn((
            Node,
            RowSelBar,
            Name::new("#RowSelBar"),
            Style::default()
                .absolute()
                .inset(Inset {
                    left: Sizing::Length(Length::px(0.0)),
                    top: Sizing::Length(Length::px(0.0)),
                    bottom: Sizing::Length(Length::px(0.0)),
                    ..Default::default()
                })
                .width_px(2.5),
            Background {
                color: ColorToken::Accent,
            },
            Pickable::IGNORE,
        ))
        .id()
}

/// The sticky table header row rendered in `font`: `gap:12px`, `padding:8px 14px`,
/// bottom 1px `border.subtle`, bg `surface.inset`; the column labels are `font`
/// 10 / 500 / 1.00px LS uppercase `text.dim`. `cols` are `(label, width_px_or_flex)`
/// — pass a `Some(px)` fixed width or `None` for the `flex:1` column. Returns the
/// header root.
pub fn table_header(world: &mut World, cols: &[(&str, Option<f32>)], font: FontFamily) -> Entity {
    let cells: Vec<Entity> = cols
        .iter()
        .map(|&(label, width)| {
            // Name each cell by its column label (`#HeaderCell-INDEX`, …) so a
            // layout dump stays spawn-order-independent even when the whole table
            // collapses to zero-boxes (a hidden screen) — identically-named cells
            // at (0,0) would be ambiguous siblings.
            let cell = text_leaf(
                world,
                &format!("#HeaderCell-{label}"),
                label,
                font.clone(),
                10.0,
                500,
                ColorToken::TextDim,
                Some(1.00),
            );
            match width {
                Some(px) => {
                    world.entity_mut(cell).insert(Style::default().width_px(px));
                }
                None => {
                    world.entity_mut(cell).insert(FlexItem {
                        grow: 1.0,
                        ..Default::default()
                    });
                }
            }
            cell
        })
        .collect();

    let header = box_node(
        world,
        "#TableHeader",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .gap_px(12.0)
            .padding_edges(Edges::axis(14.0, 8.0))
            .border_edges(Edges {
                top: Length::px(0.0),
                right: Length::px(0.0),
                bottom: Length::px(1.0),
                left: Length::px(0.0),
            }),
        "color.surface.inset",
        Border {
            bottom: solid_side("color.border.subtle"),
            ..Default::default()
        },
    );
    world.entity_mut(header).add_children(&cells);
    header
}

/// Re-style a table row's selected state: swap the row `Background` (`accent.soft`
/// selected / transparent not) AND add/remove the 2.5px accent **inset-left bar**
/// ([`RowSelBar`]) child — the full selected representation, so a caller flips
/// selection with one call. Idempotent (never spawns a duplicate bar; a no-op when
/// already in the requested state).
pub fn set_table_row_selected(world: &mut World, row: Entity, selected: bool) {
    let bg = if selected {
        "color.accent.soft"
    } else {
        "color.surface.transparent"
    };
    if let Some(mut b) = world.get_mut::<Background>(row) {
        b.color = tok(bg);
    }

    let existing_bar: Option<Entity> = world
        .get::<Children>(row)
        .into_iter()
        .flat_map(|c| c.iter().copied())
        .find(|&c| world.get::<RowSelBar>(c).is_some());
    match (selected, existing_bar) {
        (true, None) => {
            let bar = spawn_row_sel_bar(world);
            world.entity_mut(row).add_child(bar);
        }
        (false, Some(bar)) => {
            world.entity_mut(bar).despawn();
        }
        _ => {}
    }
}
