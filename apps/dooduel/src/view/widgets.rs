//! Shared view helpers — the sketchy card/panel treatments, the button family, the
//! eyebrow/title/badge text bits, the avatar element, the overlay scrim, and the
//! floating theme toggle. Used across every screen module.

use buiy::view::{Color, Element, Radius, Space, button, text};
use buiy_view::LineStyle;

use crate::avatar::{self, doodle_avatar};
use crate::theme::{
    self, CLEAR, FONT_BODY, FONT_DISPLAY, INK_PANEL, INK_PANEL_ON, Palette, SCRIM, WHITE,
    WOBBLE_CARD, WOBBLE_PANEL,
};
use crate::{Dooduel, HumanAvatar, Msg};

/// The name to preview before the player types one (design default "Mara").
pub fn preview_name(s: &Dooduel) -> String {
    let t = s.player_name.trim();
    if t.is_empty() {
        "Mara".to_string()
    } else {
        t.to_string()
    }
}

/// The viewport-centered screen shell over the canvas background.
pub fn screen_root(inner: Element<Msg>, p: Palette) -> Element<Msg> {
    column_of(vec![inner])
        .fill()
        .justify_center()
        .align_center()
        .background(p.canvas)
        .padding(Space::Lg)
}

/// A `column` from a `Vec` (a thin alias so the helpers read cleanly — the `column!`
/// macro is for literal child lists).
pub fn column_of(children: Vec<Element<Msg>>) -> Element<Msg> {
    Element::column(children)
}

/// The sketchy CARD treatment: the hand-drawn per-corner wobble + a chunky 3px ink
/// outline + the soft `--sh-md` ambient drop shadow. The bordered fill rounds to the
/// band's inner radius (F4b ears fix), so the wobble reads clean with no square
/// corner "ears". Wraps an already-built column.
pub fn sketchy_card(el: Element<Msg>, p: Palette) -> Element<Msg> {
    el.radius_corners(WOBBLE_CARD[0], WOBBLE_CARD[1], WOBBLE_CARD[2], WOBBLE_CARD[3])
        .border(3.0, p.ink, LineStyle::Solid)
        .shadow(0.0, 4.0, 0.0, 0.0, p.shadow_hard)
        .shadow(0.0, 3.0, 12.0, 0.0, p.shadow_soft)
}

/// The sketchy PANEL treatment (in-game surfaces): a smaller per-corner wobble + a
/// 2.5px ink outline + the tighter `--sh-sm` ambient shadow.
pub fn sketchy_panel(el: Element<Msg>, p: Palette) -> Element<Msg> {
    el.radius_corners(WOBBLE_PANEL[0], WOBBLE_PANEL[1], WOBBLE_PANEL[2], WOBBLE_PANEL[3])
        .border(2.5, p.ink, LineStyle::Solid)
        .shadow(0.0, 2.0, 0.0, 0.0, p.shadow_hard)
        .shadow(0.0, 1.0, 2.0, 0.0, p.shadow_soft)
}

/// The card width for the current viewport: the design width on desktop, clamped to
/// fit the phone's side margins on mobile so a 440px card never clips a ~390px
/// screen. The pre-game screens stay centered single-column at both sizes.
pub fn card_w(s: &Dooduel, design_w: f32) -> f32 {
    if s.is_mobile() {
        // The card sits inside `screen_root`'s `Space::Lg` (24px) padding, so it
        // must fit `viewport − 2·24` or it overflows (clipping the right edge).
        (s.viewport_w - 48.0).clamp(240.0, design_w)
    } else {
        design_w
    }
}

/// A fixed-width surface card (the pre-game panel). Content stretches to the card
/// width (no `align_center`), so buttons + inputs fill it. The ink outline is
/// Dooduel's defining edge; [`sketchy_card`] adds the wobble + drop shadow.
pub fn card(width: f32, children: Vec<Element<Msg>>, p: Palette) -> Element<Msg> {
    sketchy_card(
        Element::column(children)
            .width(width)
            .background(p.surface)
            .padding(Space::Xl)
            .gap(Space::Md),
        p,
    )
}

/// The big purple pill CTA (Play / Start / Join). Full width via the card stretch.
/// The 3D-press underside is a hard `0 5px 0 ink` offset shadow behind the pill (the
/// chunky "pressable" edge — the F4b rounded-shadow instance keeps the edge crisp)
/// plus a soft ambient. The press-DOWN dip is the F5 `PressEffect` (automatic).
pub fn primary_button(label: &str, msg: Msg, p: Palette) -> Element<Msg> {
    button(label)
        .on_press(msg)
        .background(Color::Accent)
        .color(WHITE)
        .radius(Radius::Full)
        .border(2.5, p.ink, LineStyle::Solid)
        .height(56.0)
        .size(24.0)
        .font(FONT_DISPLAY)
        .shadow(0.0, 5.0, 0.0, 0.0, p.ink)
        .shadow(0.0, 8.0, 18.0, -6.0, Color::Custom(0, 0, 0, 64))
}

/// An accent-tinted "soft" pill (secondary Create/Join). `.grow()` so a row of them
/// splits the width evenly.
pub fn soft_button(label: &str, msg: Msg, p: Palette) -> Element<Msg> {
    button(label)
        .on_press(msg)
        .background(p.accent_tint)
        .color(Color::Accent)
        .radius(Radius::Full)
        .height(46.0)
        .size(17.0)
        .font(FONT_DISPLAY)
        .grow()
}

/// A chromeless (transparent) text button (Back / Leave room).
pub fn quiet_button(label: &str, msg: Msg, p: Palette) -> Element<Msg> {
    button(label)
        .on_press(msg)
        .background(CLEAR)
        .color(p.muted)
        .size(15.0)
        .font(FONT_BODY)
}

/// A small uppercase accent eyebrow (`Private room`).
pub fn eyebrow(label: &str) -> Element<Msg> {
    text(label).size(13.0).color(Color::Accent).font(FONT_BODY)
}

/// A Caveat display heading in ink.
pub fn title(label: &str, size: f32, p: Palette) -> Element<Msg> {
    text(label).size(size).color(p.ink).font(FONT_DISPLAY)
}

/// A tinted status pill (the lobby roster badges). Borderless-rounded so the tint
/// fill pills cleanly.
pub fn badge(label: &str, bg: Color, fg: Color) -> Element<Msg> {
    Element::column(vec![text(label).size(12.0).color(fg).font(FONT_BODY)])
        .background(bg)
        .radius(Radius::Full)
        .padding(Space::Xs)
}

/// The avatar element for a seat: the human's chosen avatar (a gallery preset or the
/// drawn custom image) when `is_me`, else the name-hashed doodle. Callers may chain
/// `.on_press`/`.label` — the F5 raster press route now makes even the custom-image
/// `raster` case activatable.
pub fn avatar_el(s: &Dooduel, is_me: bool, name: &str, px: f32) -> Element<Msg> {
    if is_me {
        match s.avatar.kind {
            HumanAvatar::Custom => avatar::custom_avatar(s.saved_avatar.clone(), px),
            HumanAvatar::Preset { icon, tint } => avatar::doodle_avatar_forced(icon, tint, px),
            HumanAvatar::Default => doodle_avatar(name, px),
        }
    } else {
        doodle_avatar(name, px)
    }
}

/// The in-game surface panel: a sketchy-outlined surface card with `Md` padding.
pub fn panel(inner: Element<Msg>, p: Palette) -> Element<Msg> {
    sketchy_panel(
        Element::column(vec![inner]).background(p.surface).padding(Space::Md),
        p,
    )
}

/// Wrap an overlay panel in a centered, full-viewport, top-layer scrim (the design's
/// dark translucent modal backdrop). The scrim carries a translucent BACKGROUND
/// color (never an `Opacity` component), so it does not form an effect group — the
/// F4a boundary that keeps a nested raster visible in an opaque panel over it.
pub fn scrim(overlay_panel: Element<Msg>) -> Element<Msg> {
    Element::column(vec![overlay_panel])
        .fill()
        .fixed()
        .top_layer()
        .justify_center()
        .align_center()
        .background(SCRIM)
}

/// The floating light/dark theme toggle (design: a fixed bottom-right pill on every
/// screen). A pressable pill on the always-dark chrome; pressing folds
/// `SetTheme(toggled)` through the funnel. Placed via `.fixed().fill()` +
/// `.justify_end().align_end()` so it lands bottom-right of the viewport,
/// `.top_layer()` so it floats over content.
///
/// The transparent container that `.fill()`s the viewport would otherwise sit
/// topmost in the pick order and swallow every click across the whole app. F6 now
/// AUTO-applies `Pickable::IGNORE` to a transparent (`Color::NONE`) top-layer
/// container, so the bug is unwritable — but we keep the explicit `.ignore_picking()`
/// as a belt-and-suspenders statement of intent (a no-op when auto-IGNORE fires).
pub fn theme_toggle(s: &Dooduel) -> Element<Msg> {
    let label = match s.theme {
        theme::ThemePref::Dark => "Dark",
        theme::ThemePref::Light => "Light",
    };
    let pill = button(label)
        .on_press(Msg::SetTheme(s.theme.toggled()))
        .background(INK_PANEL)
        .color(INK_PANEL_ON)
        .size(15.0)
        .font(FONT_BODY)
        .radius(Radius::Full)
        .height(34.0)
        .width(72.0);
    Element::column(vec![pill])
        .fill()
        .fixed()
        .top_layer()
        .ignore_picking()
        .justify_end()
        .align_end()
        .padding(Space::Lg)
}

/// The mocked opponents preview chips ("You'll play with"). Shared by Home + Lobby.
pub fn opponent_chip(name: &str, p: Palette) -> Element<Msg> {
    Element::column(vec![
        doodle_avatar(name, 38.0),
        text(name).size(13.0).color(p.muted).font(FONT_BODY),
    ])
    .gap(Space::Xs)
    .align_center()
}

