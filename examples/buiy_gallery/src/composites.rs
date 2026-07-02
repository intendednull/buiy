//! `buiy_gallery::composites` — the **screen-specific composite widgets** (parity
//! Wave C2) the design's screens are built from but which are NOT general enough to
//! belong in the framework: the controls-showcase composites (stepper / segmented),
//! and the chrome composites (toast / badge / chip / stat-row).
//!
//! ## Promoted vs. gallery-local (Wave 5 refinement)
//!
//! The genuinely-general composites — `meter`, `table_row`/`table_header`,
//! `search_input`, `kbd`, `status_dot`/`pulse_blink` — were **promoted to
//! [`buiy_widgets::composites`]** (spec § 2 REFINE) and are reached here through
//! `buiy::prelude::*` (font-NEUTRAL: each text-bearing one takes the typeface as an
//! argument, so this module threads the gallery's Geist faces into them). What stays
//! gallery-local is what is screen-specific or design-bespoke: the stepper/segmented
//! controls, the toast lifecycle, the header `badge`/`chip`, and the rail/inspector
//! `stat_row`. There is ONE definition of each promoted composite (in the framework)
//! — this module only *uses* them, it does not redefine them.
//!
//! These are authored as imperative `World`-spawning builders (the same idiom as
//! [`crate::shell`] and `examples/capture`): a composite is a small static tree of
//! one-off styled boxes, which reads cleaner spawned with the `Style`-builder + small
//! helpers than as one giant `bsn!` block. Each is built to EXACT parity against
//! `docs/specs/2026-06-25-widget-catalog-values.md` (every px / color / radius / font
//! / letter-spacing comes from the values table, never re-derived) — see the
//! per-builder doc for its values.md anchor.
//!
//! ## The toast builder returns more than an entity
//!
//! [`toast`](crate::composites::toast) is spawned by
//! [`show_toast`](crate::composites::show_toast) into a top-layer, fixed,
//! bottom-center slot with an entrance
//! ([`OpacityTween`](buiy_core::animation::OpacityTween) +
//! [`TranslateTween`](buiy_core::animation::TranslateTween), values.md § 5.2
//! `toast-in` ~180ms ease-out) and an auto-dismiss [`bevy::time::Timer`] (~2.2s) the
//! [`ToastPlugin`](crate::composites::ToastPlugin) runs in
//! [`BuiySet::Animate`](buiy_core::BuiySet::Animate).
//!
//! Design: `docs/specs/2026-06-25-widget-catalog-parity-design.md` § 3.7;
//! exact values: `docs/specs/2026-06-25-widget-catalog-values.md` § 2–§ 7.

use bevy::prelude::{
    App, Children, Component, Entity, IntoScheduleConfigs, Name, Plugin, Resource, Update, Vec3,
    World,
};
use bevy::time::{Time, Timer, TimerMode};
// `buiy::prelude::*` brings the render/layout/text component surface AND the promoted
// composites (`meter` / `table_row` / `table_header` / `search_input` / `kbd` /
// `status_dot` / `pulse_blink` / `TableRowData`) now living in `buiy_widgets`.
use buiy::prelude::*;
use buiy_core::BuiySet;
use buiy_core::animation::{Easing, OpacityTween, TranslateTween, Tween};
use buiy_core::layout::{Stacking, TopLayer, Translate};
// `Icon` reaches us through `buiy::prelude::*` (spec § 2 REFINE promotion).
use buiy_core::render::components::{BoxShadow, LineStyle, Opacity, Shadow};
use buiy_core::text::{FamilyEntry, FontStack, LetterSpacing};

// ===========================================================================
// Shared authoring helpers (mirroring `shell.rs`; kept module-local so the two
// modules stay independently buildable — `composites` does not depend on private
// `shell` helpers and vice versa).
// ===========================================================================

/// A `ColorToken::Token` from a `&str` key (every paint is a named dark token —
/// the forced-colors gate stays enforceable; the shell uses the same `tok`).
fn tok(key: &str) -> ColorToken {
    ColorToken::Token(key.to_string().into())
}

/// The Geist sans font stack (the sans generic still resolves to Fira — Wave A
/// note — so author Geist by name, like the shell does).
fn geist() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist".into())]))
}

/// The Geist Mono font stack (the design's monospace UI face).
fn geist_mono() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist Mono".into())]))
}

/// A solid 1px `BorderSide` of a token color.
fn solid_side(token: &str) -> BorderSide {
    BorderSide {
        color: tok(token),
        style: LineStyle::Solid,
    }
}

/// A uniform 1px `Border` of `token` with `radius` rounded corners.
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
/// an optional `LetterSpacing`, and `Pickable::IGNORE` (composite labels are
/// decorative pixels — clicks fall through to the owning control). The font
/// family/size/weight/color/ls are the values.md § 4 typography row.
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

/// A Geist Mono leaf (size / weight / color, no letter-spacing).
fn mono_leaf(
    world: &mut World,
    name: &str,
    s: &str,
    size: f32,
    weight: u16,
    color: ColorToken,
) -> Entity {
    text_leaf(world, name, s, geist_mono(), size, weight, color, None)
}

/// A Geist sans leaf (size / weight / color, no letter-spacing).
fn sans_leaf(
    world: &mut World,
    name: &str,
    s: &str,
    size: f32,
    weight: u16,
    color: ColorToken,
) -> Entity {
    text_leaf(world, name, s, geist(), size, weight, color, None)
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

/// A centered flex-row (icon-button body): `width`×`height`, center-aligned.
fn square_button_style(size: f32) -> Style {
    Style::default()
        .width_px(size)
        .height_px(size)
        .flex_row()
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .border(1.0)
}

// ===========================================================================
// 1. stepper(count) — values.md § 7.2 Showcase (stepper), § 6 #20/#23, § 4
// ===========================================================================

/// Marks a stepper's count `Text` leaf (so [`set_stepper`] can rewrite it).
#[derive(Component, Clone, Copy)]
pub struct StepperCount;

/// Marks a stepper's `−` / `+` button (the app logic reads which one fired).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum StepperButton {
    /// The `−` (decrement) button.
    Decrement,
    /// The `+` (increment) button.
    Increment,
}

/// A `[− btn][count][+ btn]` stepper. The two 34×34 icon buttons (border
/// `border.strong`, radius 8, bg `surface.inset`) carry a `−`/`+` [`Icon`]
/// (values.md § 6 #23 minus `M5 12h14` / #20 plus `M12 5v14M5 12h14`, stroke 2,
/// size 15, `text.secondary`); the count is Geist Mono 20 / 600 `text.primary`,
/// `min-width:44px` centered (values.md § 4 "Showcase — stepper count").
/// `gap:10px` between the three (the design lays the showcase steppers with a
/// flex-row gap). Returns the stepper root; the buttons carry [`StepperButton`]
/// and the count carries [`StepperCount`].
pub fn stepper(world: &mut World, count: i32) -> Entity {
    let dec = stepper_button(world, StepperButton::Decrement, "M5 12h14");
    let count_leaf = mono_leaf(
        world,
        "#StepperCount",
        &format!("{count:02}"),
        20.0,
        600,
        tok("color.text.primary"),
    );
    // `min-width:44px; text-align:center` (values.md § 7.2). A centered flex-box
    // wrapper gives the count its min-width without disturbing the leaf's own
    // intrinsic text box.
    world.entity_mut(count_leaf).insert((
        // The marker `set_stepper` walks to rewrite the visible count. Without it
        // `descendant_with::<StepperCount>` finds nothing and `set_stepper` is a
        // silent no-op — the `Name("#StepperCount")` above is only a debug label,
        // not this component.
        StepperCount,
        Style::default()
            .min_width(Sizing::Length(Length::px(44.0)))
            .flex_row()
            .justify_content(JustifyContent::Center),
        TextAlign::Center,
    ));
    let inc = stepper_button(world, StepperButton::Increment, "M12 5v14M5 12h14");

    let root = world
        .spawn((
            Node,
            Name::new("#Stepper"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(10.0),
        ))
        .id();
    world.entity_mut(root).add_children(&[dec, count_leaf, inc]);
    root
}

/// One 34×34 stepper icon button (the `−` or `+`). Uses the **bare `Button`
/// marker** (an "icon button's canvas" — button.rs doc) so it carries the OnPress
/// sink + keymap + a11y role with NO auto-label child; the only child is the
/// `−`/`+` icon. The accessible name is layered explicitly via `A11yLabel`. The
/// [`StepperButton`] marker is the app handle.
fn stepper_button(world: &mut World, which: StepperButton, path_d: &str) -> Entity {
    let label = match which {
        StepperButton::Decrement => "Decrement",
        StepperButton::Increment => "Increment",
    };
    // Name the icon + button by their role (`#StepperIcon-Decrement`, …) so the
    // layout dump stays spawn-order-independent even when the whole stepper
    // collapses to zero-boxes (a `Display::None` hidden screen) — identically-named
    // siblings at (0,0) would be ambiguous (the `#SegmentedOption-N` precedent).
    let glyph = icon_box(
        world,
        &format!("#StepperIcon-{label}"),
        path_d,
        2.0,
        15,
        tok("color.text.secondary"),
    );
    world
        .spawn((
            buiy::prelude::Button,
            A11yLabel(label.to_string()),
            which,
            Name::new(format!("#StepperButton-{label}")),
            square_button_style(34.0),
            Background {
                color: tok("color.surface.inset"),
            },
            border_all("color.border.strong", 8.0),
        ))
        .add_children(&[glyph])
        .id()
}

/// Rewrite a stepper's visible count (the app logic calls this after applying a
/// `−`/`+` press). Walks the stepper's [`StepperCount`] leaf and rewrites its
/// `Text` to the zero-padded two-digit count (the design's `pad("03")` idiom).
pub fn set_stepper(world: &mut World, stepper: Entity, count: i32) {
    let Some(leaf) = descendant_with::<StepperCount>(world, stepper) else {
        return;
    };
    if let Some(mut t) = world.get_mut::<Text>(leaf) {
        t.0 = format!("{count:02}");
    }
}

// ===========================================================================
// 2. segmented(options, selected) — values.md § 7.2 Showcase (segmented), § 4
// ===========================================================================

/// Marks one segmented option button, carrying its index (the app logic reads
/// which fired; [`set_segmented`] restyles by comparing to the selected index).
#[derive(Component, Clone, Copy)]
pub struct SegmentedOption(pub usize);

/// An exclusive pill group in a `surface.inset` track (border `border.default`,
/// radius 9, `gap:4px`, `padding:3px` — values.md § 7.2 "Segmented track"). Each
/// option is a button (`flex:1; padding:7px 0`, radius 6 — values.md § 7.2
/// "Segmented button"); the selected one is `accent` bg + `text.on-accent`, the
/// rest are transparent + `text.muted`; labels Geist 12 / 500 (values.md § 4
/// "Showcase — segmented buttons"). Returns the track root; each option carries
/// its [`SegmentedOption`] index.
pub fn segmented(world: &mut World, options: &[&str], selected: usize) -> Entity {
    let buttons: Vec<Entity> = options
        .iter()
        .enumerate()
        .map(|(i, &label)| segmented_option(world, i, label, i == selected))
        .collect();

    let track = box_node(
        world,
        "#Segmented",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .gap_px(4.0)
            .padding(3.0)
            .border(1.0),
        "color.surface.inset",
        border_all("color.border.default", 9.0),
    );
    world.entity_mut(track).add_children(&buttons);
    track
}

/// One segmented option button (`flex:1`, the selected/unselected paint). Uses
/// the **bare `Button` marker** (not `Button::new`) so it carries the OnPress sink,
/// keymap, and a11y role WITHOUT the auto-injected default-styled label child — the
/// option's label is our own Geist-12/500 token-tinted leaf (the § 4.1c double-
/// label gotcha: `Button::new` would add a second, default-styled label). The
/// accessible name is layered explicitly via `A11yLabel`.
fn segmented_option(world: &mut World, idx: usize, label: &str, selected: bool) -> Entity {
    let (bg, fg) = segmented_colors(selected);
    // Name the label + option by index (`#SegmentedLabel-N`, `#SegmentedOption-N`)
    // so the layout dump stays spawn-order-independent even when the whole track
    // collapses to zero-boxes (a `Display::None` hidden screen) — identically-named
    // siblings at (0,0) would be ambiguous (snapshots.md § Tier 1; the
    // `table_header` `#HeaderCell-{label}` precedent).
    let text = sans_leaf(
        world,
        &format!("#SegmentedLabel-{idx}"),
        label,
        12.0,
        500,
        fg,
    );
    world
        .spawn((
            buiy::prelude::Button,
            A11yLabel(label.to_string()),
            SegmentedOption(idx),
            Name::new(format!("#SegmentedOption-{idx}")),
            Style::default()
                .flex_row()
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .padding_edges(Edges::axis(0.0, 7.0)),
            Background { color: tok(bg) },
            Border {
                radius: Corners::all(Radius::circular(6.0)),
                ..Default::default()
            },
            FlexItem {
                grow: 1.0,
                ..Default::default()
            },
        ))
        .add_children(&[text])
        .id()
}

/// The `(bg, fg)` token pair for a segmented option in `selected` state.
fn segmented_colors(selected: bool) -> (&'static str, ColorToken) {
    if selected {
        ("color.accent", tok("color.text.on-accent"))
    } else {
        ("color.surface.transparent", tok("color.text.muted"))
    }
}

/// Re-style a segmented group's options to reflect a new selected index (the app
/// logic calls this after a press): the newly-selected option gets the accent
/// fill + on-accent label, the rest go transparent + muted.
pub fn set_segmented(world: &mut World, track: Entity, selected: usize) {
    let options: Vec<(Entity, usize)> = child_options(world, track);
    for (btn, idx) in options {
        let (bg, fg) = segmented_colors(idx == selected);
        if let Some(mut b) = world.get_mut::<Background>(btn) {
            b.color = tok(bg);
        }
        // Re-tint the option's label leaf.
        if let Some(label) = first_text_child(world, btn)
            && let Some(mut c) = world.get_mut::<TextColor>(label)
        {
            c.0 = fg;
        }
    }
}

/// The `(button, index)` pairs of a segmented track's option children.
fn child_options(world: &World, track: Entity) -> Vec<(Entity, usize)> {
    let Some(children) = world.get::<Children>(track) else {
        return Vec::new();
    };
    children
        .iter()
        .filter_map(|&c| world.get::<SegmentedOption>(c).map(|o| (c, o.0)))
        .collect()
}

// ===========================================================================
// 5. toast(msg) — values.md § 7 (toast), § 2 shadow.menu, § 5.2 toast-in, § 6 #25
// ===========================================================================

/// The toast lifecycle resource: the live toast entity (if shown) + its
/// auto-dismiss timer. [`show_toast`] sets it; [`ToastPlugin`] ticks the timer and
/// despawns on expiry.
#[derive(Resource, Default)]
pub struct Toast {
    /// The live toast entity, or `None` when no toast is showing.
    pub entity: Option<Entity>,
    /// The auto-dismiss countdown (set on show; ticks each frame in
    /// `BuiySet::Animate`).
    pub timer: Option<Timer>,
}

/// The toast auto-dismiss lifetime (values.md "lifecycle ~2.2s").
const TOAST_LIFETIME_SECS: f32 = 2.2;
/// The toast entrance duration (values.md § 5.2 `toast-in` ~180ms ease-out).
const TOAST_IN_SECS: f32 = 0.18;
/// The toast entrance translateY (values.md § 5.2 `toast-in` `translateY(8px)`).
const TOAST_IN_DY: f32 = 8.0;

/// Build a toast card (NOT spawned into a slot — [`show_toast`] does that): a
/// `surface.raised` card (border `border.strong`, radius 10, `shadow.menu`
/// `0 16px 40px -12px rgba(0,0,0,.8)`, `padding:11px 16px`, `gap:10px` — values.md
/// § 7 toast row, § 2 `shadow.menu`) holding a check [`Icon`] (values.md § 6 #25
/// `M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5`, stroke 1.8, size
/// 16, accent) + the message (Geist 12.5 / 500 `text.primary` — values.md § 4
/// "Toast — message"). Starts at `Opacity(0)` + translated down (the entrance
/// start); [`show_toast`] attaches the entrance tweens. Returns the card.
pub fn toast(world: &mut World, msg: &str) -> Entity {
    let check = icon_box(
        world,
        "#ToastIcon",
        "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5",
        1.8,
        16,
        tok("color.accent"),
    );
    let label = sans_leaf(
        world,
        "#ToastMessage",
        msg,
        12.5,
        500,
        tok("color.text.primary"),
    );

    let card = world
        .spawn((
            Node,
            Name::new("#Toast"),
            Style::default()
                .flex_row()
                .align_items(AlignItems::Center)
                .gap_px(10.0)
                .padding_edges(Edges::axis(16.0, 11.0))
                .border(1.0),
            Background {
                color: tok("color.surface.raised"),
            },
            border_all("color.border.strong", 10.0),
            // shadow.menu — `0 16px 40px -12px rgba(0,0,0,.8)` (values.md § 2). The
            // spread is the design's negative `-12px`.
            BoxShadow(vec![Shadow {
                color: tok("color.shadow.menu"),
                offset_x: Length::px(0.0),
                offset_y: Length::px(16.0),
                blur: Length::px(40.0),
                spread: Length::px(-12.0),
                inset: false,
            }]),
            Pickable::IGNORE,
        ))
        .add_children(&[check, label])
        .id();
    // The entrance start state, inserted separately to keep the spawn tuple small
    // (the tweens animate these to rest).
    world.entity_mut(card).insert((
        Opacity(0.0),
        Translate(Length::px(0.0), Length::px(TOAST_IN_DY), Length::px(0.0)),
    ));
    card
}

/// The toast's bottom inset from the viewport floor (values.md / design HTML 461:
/// `position:fixed; bottom:44px`).
const TOAST_BOTTOM_PX: f32 = 44.0;

/// Show a transient toast bottom-center (fixed / top-layer): despawn any prior
/// toast, build a fresh card, place it in a fixed full-width bottom-anchored
/// **centering wrapper**, attach the card's entrance tweens (opacity `0→1` +
/// translateY `8px→0`, ~180ms ease-out — values.md § 5.2), and arm the ~2.2s
/// auto-dismiss timer (ticked by [`ToastPlugin`]).
///
/// **Centering (C4 fix).** The design centers the toast with
/// `left:50%; transform:translateX(-50%)` (HTML 461). A static `Translate`
/// would collide with the card's translateY entrance tween, and a measure-then-
/// shift needs a frame of layout. Instead we wrap the content-sized card in a
/// fixed, viewport-width, bottom-anchored row that `justify-content:center`s its
/// single child — the same measure-free centering the `#ModalRoot` overlay uses
/// (`align/justify: Center` against a window-sized box). The card stays
/// content-sized; the wrapper does the horizontal centering; the card's
/// translateY entrance rides on top untouched.
pub fn show_toast(world: &mut World, msg: &str) {
    // Despawn any existing toast (one at a time — the design shows a single toast).
    // The tracked entity is the WRAPPER, so despawning it removes the card too.
    if let Some(prev) = world.resource::<Toast>().entity
        && world.get_entity(prev).is_ok()
    {
        world.entity_mut(prev).despawn();
    }

    let card = toast(world, msg);
    // The card's entrance: opacity 0→1 + translateY 8px→0, ~180ms, ease-out
    // (DESIGN is the catalog's ease-out cubic). Applied to the card so the
    // wrapper's centering layout is undisturbed.
    world.entity_mut(card).insert((
        OpacityTween(Tween::secs(0.0, 1.0, TOAST_IN_SECS, Easing::DESIGN)),
        TranslateTween(Tween::secs(
            Vec3::new(0.0, TOAST_IN_DY, 0.0),
            Vec3::ZERO,
            TOAST_IN_SECS,
            Easing::DESIGN,
        )),
    ));

    // The fixed, full-VIEWPORT, bottom-center wrapper (the design's
    // `position:fixed; bottom:44px; left:50%; translateX(-50%)` rendered as a
    // viewport-filling column that horizontally centers + bottom-anchors its single
    // child — measure-free, the proven top-layer-overlay idiom the `ModalDialog`
    // overlay uses: a `TopLayer` member sized `width/height:100%` fills the viewport
    // (the `fixed_root` attach), and `align_items:Center` + `justify_content:FlexEnd`
    // place the card bottom-center; the `padding-bottom:44px` is the design's
    // bottom offset). `width/height:100%` (NOT `inset left:0;right:0`, which only
    // OFFSETS a content-sized fixed box, leaving it left-aligned).
    let wrapper = world
        .spawn((
            Node,
            Name::new("#ToastWrapper"),
            Style::default()
                .fixed()
                .flex_column()
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::FlexEnd)
                .width(Sizing::Length(Length::percent(100.0)))
                .height(Sizing::Length(Length::percent(100.0)))
                .padding_edges(Edges {
                    bottom: Length::px(TOAST_BOTTOM_PX),
                    ..Default::default()
                })
                .stacking(Stacking {
                    top_layer: TopLayer::Tooltip,
                    ..Default::default()
                }),
            // The wrapper itself paints nothing + ignores hits (the card is the
            // only visible/interactive content).
            Pickable::IGNORE,
        ))
        .add_child(card)
        .id();

    let mut toast_res = world.resource_mut::<Toast>();
    toast_res.entity = Some(wrapper);
    toast_res.timer = Some(Timer::from_seconds(TOAST_LIFETIME_SECS, TimerMode::Once));
}

/// Tick the toast auto-dismiss timer; when it expires, despawn the toast and clear
/// the [`Toast`] resource. Runs in [`BuiySet::Animate`] (next to the tweens).
pub fn tick_toast(world: &mut World) {
    let delta = world.resource::<Time>().delta();
    let expired = {
        let mut toast = world.resource_mut::<Toast>();
        match toast.timer.as_mut() {
            Some(timer) => timer.tick(delta).just_finished(),
            None => false,
        }
    };
    if expired {
        let entity = world.resource_mut::<Toast>().entity.take();
        world.resource_mut::<Toast>().timer = None;
        if let Some(e) = entity
            && world.get_entity(e).is_ok()
        {
            world.entity_mut(e).despawn();
        }
    }
}

/// The toast lifecycle plugin: the [`Toast`] resource + the auto-dismiss tick in
/// [`BuiySet::Animate`] (so it ticks alongside the entrance tweens). [`show_toast`]
/// is the imperative entry the app logic calls.
pub struct ToastPlugin;

impl Plugin for ToastPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Toast>()
            .add_systems(Update, tick_toast.in_set(BuiySet::Animate));
    }
}

// ===========================================================================
// 6. badge(label) / chip(label, dot) — values.md § 7.1 (chips), § 3 radii, § 4
// ===========================================================================

/// A rounded pill badge: Geist Mono label, 1px `border.default`, radius (`radius`),
/// `padding:5px 9px`, bg `surface.inset` (the header "widget catalog" badge idiom,
/// values.md § 7.1 header chips; § 3 radius.sm/pill). `size`/`weight`/`color` are
/// the label's Geist Mono typography. Returns the badge root.
pub fn badge(world: &mut World, label: &str, radius: f32) -> Entity {
    let text = mono_leaf(
        world,
        "#BadgeLabel",
        label,
        11.0,
        500,
        tok("color.text.muted"),
    );
    let b = box_node(
        world,
        "#Badge",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .padding_edges(Edges::axis(9.0, 5.0))
            .border(1.0),
        "color.surface.inset",
        border_all("color.border.default", radius),
    );
    world.entity_mut(b).add_children(&[text]);
    b
}

/// A `[dot 6×6 radius2][mono label]` chip — the inspector "Composed of" chip
/// (values.md § 7.1 "composed chips" `padding:4px 9px`, § 6 inspector chip dot
/// 6×6 radius 2). The dot is `dot_color`; the label is Geist Mono 11 / 500
/// `text.secondary` (values.md § 4 "Inspector — Composed of chips"); 1px
/// `border.default`, radius 6, `gap:6px`. Returns the chip root.
pub fn chip(world: &mut World, label: &str, dot_color: &str) -> Entity {
    let dot = world
        .spawn((
            Node,
            Name::new("#ChipDot"),
            Style::default().width_px(6.0).height_px(6.0),
            Background {
                color: tok(dot_color),
            },
            Border {
                radius: Corners::all(Radius::circular(2.0)),
                ..Default::default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let text = mono_leaf(
        world,
        "#ChipLabel",
        label,
        11.0,
        500,
        tok("color.text.secondary"),
    );
    let c = box_node(
        world,
        "#Chip",
        Style::default()
            .flex_row()
            .align_items(AlignItems::Center)
            .gap_px(6.0)
            .padding_edges(Edges::axis(9.0, 4.0))
            .border(1.0),
        "color.surface.card",
        border_all("color.border.default", 6.0),
    );
    world.entity_mut(c).add_children(&[dot, text]);
    c
}

// ===========================================================================
// 9. stat_row(key, value) — values.md § 7.1 (stats), § 4 (Rail — stat key/value)
// ===========================================================================

/// A `[key][spacer][value]` baseline-aligned stat row: the key Geist 11.5 / 400
/// `text.muted` left, the value Geist Mono 11.5 / 500 `text.secondary` right
/// (values.md § 4 "Rail — stat key/value"; § 7.1 stats `gap:9px` baseline
/// space-between). Returns the row. (The rail's own stats are built in `shell.rs`;
/// this is the reusable extraction the inspector live-state + showcase reuse.)
pub fn stat_row(world: &mut World, key: &str, value: &str) -> Entity {
    let key_leaf = sans_leaf(world, "#StatKey", key, 11.5, 400, tok("color.text.muted"));
    let val_leaf = mono_leaf(
        world,
        "#StatVal",
        value,
        11.5,
        500,
        tok("color.text.secondary"),
    );
    world
        .spawn((
            Node,
            Name::new("#StatRow"),
            Style::default()
                .flex_row()
                .justify_content(JustifyContent::SpaceBetween)
                .align_items(AlignItems::Baseline),
        ))
        .add_children(&[key_leaf, val_leaf])
        .id()
}

// ===========================================================================
// The composites showcase — one of each composite in a labeled grid. Shared by
// the `composites_layout` snapshot gate AND the `capture_composites` artifact bin
// (the "example IS the fixture" discipline: the test pins the same tree the
// eyeball artifact renders). NOT part of any screen — a verification scaffold.
// ===========================================================================

/// One labeled showcase cell: an uppercase mono caption over the composite, in a
/// gap-8 column. The caption is Geist Mono 10 / 500 / 1.20px uppercase `text.dim`
/// (the design's showcase section-label idiom, values.md § 4).
fn showcase_cell(world: &mut World, caption: &str, content: Entity) -> Entity {
    let label = text_leaf(
        world,
        "#CellLabel",
        caption,
        geist_mono(),
        10.0,
        500,
        tok("color.text.dim"),
        Some(1.20),
    );
    world
        .spawn((
            Node,
            Name::new("#ShowcaseCell"),
            Style::default().flex_column().gap_px(8.0),
        ))
        .add_children(&[label, content])
        .id()
}

/// Build a labeled showcase grid laying out **one of each composite** (stepper /
/// segmented / search / meter / badge / chip / kbd / status-dots / stat-row /
/// table header + rows), centered on a `surface.app` page, and return the page
/// root. The `meter` fill is returned to `set_meter` for the capture's animated
/// frame; the toast is shown via `show_toast` (it spawns top-layer, not in the
/// grid). Returns `(page_root, meter_fill)`.
pub fn composites_showcase(world: &mut World) -> (Entity, Entity) {
    let stepper_cell = {
        let s = stepper(world, 3);
        showcase_cell(world, "STEPPER", s)
    };
    let segmented_cell = {
        let s = segmented(world, &["Compact", "Cozy", "Roomy"], 0);
        showcase_cell(world, "SEGMENTED", s)
    };
    let search_cell = {
        // The promoted `search_input` is font-neutral — thread the gallery's Geist
        // sans face (the design's search field is Geist 13/450).
        let s = search_input(world, "Filter nodes…", geist(), 240.0);
        showcase_cell(world, "SEARCH", s)
    };
    let (meter_track, meter_fill) = meter(world, 240.0, 0.64);
    let meter_cell = showcase_cell(world, "METER", meter_track);
    let badge_cell = {
        let b = badge(world, "v0.3.0", 5.0);
        showcase_cell(world, "BADGE", b)
    };
    let chip_cell = {
        let c = chip(world, "Button", "color.status.ok");
        showcase_cell(world, "CHIP", c)
    };
    let kbd_cell = {
        // The promoted `kbd` is font-neutral — thread the gallery's Geist Mono face.
        let k = kbd(world, "⌘K", geist_mono());
        showcase_cell(world, "KBD", k)
    };
    let dots_cell = {
        let row = world
            .spawn((
                Node,
                Name::new("#StatusDots"),
                Style::default()
                    .flex_row()
                    .align_items(AlignItems::Center)
                    .gap_px(14.0),
            ))
            .id();
        // The "ready" dot (status.ok, 6px glow) + the blink dot (accent, 4px soft
        // ring) — values.md § 6 CSS dots / § 2 shadow.ready-dot/blink-dot.
        let ready = status_dot(world, "color.status.ok", "color.status.ok", 6.0, 0.0);
        let blink = status_dot(world, "color.accent", "color.accent.soft", 0.0, 4.0);
        // The design's `blink 1.6s infinite` pulse (opacity 1→.25→1).
        pulse_blink(world, blink);
        world.entity_mut(row).add_children(&[ready, blink]);
        showcase_cell(world, "STATUS DOTS", row)
    };
    let stat_cell = {
        let wrap = world
            .spawn((
                Node,
                Name::new("#StatList"),
                Style::default().flex_column().gap_px(6.0).width_px(180.0),
            ))
            .id();
        let r1 = stat_row(world, "entities", "1,284");
        let r2 = stat_row(world, "frame", "2.4 ms");
        world.entity_mut(wrap).add_children(&[r1, r2]);
        showcase_cell(world, "STAT ROW", wrap)
    };
    let table_cell = {
        let panel = world
            .spawn((
                Node,
                Name::new("#TablePanel"),
                Style::default().flex_column().width_px(360.0),
                Background {
                    color: tok("color.surface.card"),
                },
                border_all("color.border.default", 8.0),
            ))
            .id();
        // The promoted `table_header`/`table_row` are font-neutral — thread the
        // gallery's Geist Mono face (the design's entity-tree table is monospace).
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
        let r0 = table_row(
            world,
            &TableRowData {
                idx: "00",
                indent_px: 0.0,
                dot_color: "color.accent.blue",
                node_type: "Stack",
                name: "root_0000",
                ms: "0.42",
                ms_warn: false,
                state: "OK",
                state_color: "color.status.ok",
            },
            geist_mono(),
            false,
        );
        let r1 = table_row(
            world,
            &TableRowData {
                idx: "01",
                indent_px: 13.0,
                dot_color: "color.status.ok",
                node_type: "Button",
                name: "primary_0001",
                ms: "1.62",
                ms_warn: true,
                state: "WARN",
                state_color: "color.status.warn",
            },
            geist_mono(),
            true,
        );
        world.entity_mut(panel).add_children(&[header, r0, r1]);
        showcase_cell(world, "TABLE ROW (+selected)", panel)
    };

    // The grid: a wrapping flex-row of cells, gap 28, padded.
    let grid = world
        .spawn((
            Node,
            Name::new("#CompositesGrid"),
            Style::default()
                .flex_row()
                .flex_wrap(FlexWrap::Wrap)
                .align_items(AlignItems::FlexStart)
                .gap_px(28.0)
                .padding(32.0)
                .width(Sizing::Length(Length::percent(100.0))),
        ))
        .add_children(&[
            stepper_cell,
            segmented_cell,
            search_cell,
            meter_cell,
            badge_cell,
            chip_cell,
            kbd_cell,
            dots_cell,
            stat_cell,
            table_cell,
        ])
        .id();

    let page = world
        .spawn((
            Node,
            Name::new("#CompositesPage"),
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::percent(100.0)))
                .height(Sizing::Length(Length::percent(100.0))),
            Background {
                color: tok("color.surface.app"),
            },
        ))
        .add_children(&[grid])
        .id();
    (page, meter_fill)
}

// ===========================================================================
// Small `&World` walk helpers (the composites' marker lookups)
// ===========================================================================

/// The first descendant of `root` (BFS) carrying marker component `T`. Used by
/// [`set_stepper`] to find the count leaf among the stepper subtree.
fn descendant_with<T: Component>(world: &World, root: Entity) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if world.get::<T>(e).is_some() {
            return Some(e);
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    None
}

/// The first direct `Text`-bearing child of `parent` (a segmented option's label
/// leaf).
fn first_text_child(world: &World, parent: Entity) -> Option<Entity> {
    let children = world.get::<Children>(parent)?;
    children
        .iter()
        .copied()
        .find(|&c| world.get::<Text>(c).is_some())
}
