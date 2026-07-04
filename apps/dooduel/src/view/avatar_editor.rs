//! Avatar editor — a full in-flow SCREEN (reached from Home's pencil badge).
//!
//! The design draws this as a centered modal over Home, and the FINAL spec's F4a
//! re-decision assumed a top-layer modal would work. **RUNNING it disproved that:**
//! a top-layer modal panel cannot occlude the base screen's TEXT. Glyphs draw in a
//! single global tier AFTER all quads (`node.rs`: "shadow < quad < gradient <
//! glyph"), with no per-top-layer glyph partition, so Home's text bleeds through any
//! top-layer panel over it (F4a fixed RASTER interleave, not glyph-over-quad
//! occlusion). So the editor renders as a full in-flow screen (the prototype's
//! proven approach): when it is open the router shows THIS instead of the underlying
//! screen, so there is no base content behind it to bleed. The design's scrim look
//! is the one accepted deviation (also the prototype's). See the App-2 hand-off /
//! framework follow-up: "top-layer modals cannot occlude base-layer text".
//!
//! Two tabs: the stock-doodle GALLERY (pick one of 22) and DRAW-YOUR-OWN (the 2nd
//! `raster()` canvas + swatches / sizes / eraser / undo / clear / save). The draw
//! canvas is the avatar editor's separate paint surface (`paint::CanvasKind::Avatar`);
//! in-flow it paints cleanly like the in-game canvas.

use buiy::view::{Color, Element, Radius, Space, button, column, row};
use buiy_view::{LineStyle, raster};

use crate::paint;
use crate::theme::{CLEAR, FONT_BODY, FONT_DISPLAY, WHITE, WOBBLE_PANEL};
use crate::view::widgets::{card_w, eyebrow, quiet_button, screen_root, title};
use crate::{AvatarState, AvatarTab, Dooduel, HumanAvatar, Msg, avatar};

/// The avatar editor as a full in-flow screen (the editor panel centered on the app
/// canvas background — no base content behind it, so no glyph bleed-through).
pub fn avatar_editor_screen(s: &Dooduel) -> Element<Msg> {
    screen_root(avatar_panel(s), s.palette())
}

/// The editor panel (in-flow: the draw-canvas raster inside it paints cleanly).
fn avatar_panel(s: &Dooduel) -> Element<Msg> {
    let a = &s.avatar;
    let p = s.palette();
    let header = row![
        eyebrow("Your profile pic"),
        quiet_button("Close", Msg::CloseAvatarEditor, p),
    ]
    .justify_between()
    .align_center();

    let tabs = row![
        avatar_tab_btn("Pick a doodle", AvatarTab::Gallery, a.tab, p),
        avatar_tab_btn("Draw your own", AvatarTab::Draw, a.tab, p),
    ]
    .gap(Space::Xs);

    let body = match a.tab {
        AvatarTab::Gallery => avatar_gallery(s),
        AvatarTab::Draw => avatar_draw(s),
    };

    let mut children = vec![header, title("Doodle yourself!", 28.0, p), tabs, body];
    // Offer a reset only when a custom avatar is set (design `hasCustomAvatarSet`).
    if a.kind != HumanAvatar::Default {
        children.push(quiet_button(
            "Reset to a random doodle",
            Msg::ResetAvatar,
            p,
        ));
    }
    Element::column(children)
        .gap(Space::Md)
        .width(card_w(s, 460.0))
        .background(p.surface)
        .radius_corners(
            WOBBLE_PANEL[0],
            WOBBLE_PANEL[1],
            WOBBLE_PANEL[2],
            WOBBLE_PANEL[3],
        )
        .border(2.5, p.ink, LineStyle::Solid)
        .shadow(0.0, 10.0, 0.0, 0.0, p.shadow_hard)
        .shadow(0.0, 18.0, 40.0, -10.0, p.shadow_soft)
        .padding(Space::Xl)
}

/// One segmented tab of the avatar editor (accent-filled when active).
fn avatar_tab_btn(
    label: &str,
    tab: AvatarTab,
    current: AvatarTab,
    p: crate::theme::Palette,
) -> Element<Msg> {
    let active = tab == current;
    let (bg, fg) = if active {
        (Color::Accent, WHITE)
    } else {
        (p.surface_2, p.ink_2)
    };
    button(label)
        .on_press(Msg::SetAvatarTab(tab))
        .background(bg)
        .color(fg)
        .size(15.0)
        .font(FONT_BODY)
        .radius(Radius::Md)
        .height(36.0)
        .grow()
}

/// The GALLERY tab: the 22 stock doodles in a 5-wide grid; the selected preset gets
/// an accent-tinted pad.
fn avatar_gallery(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();
    let selected = if let HumanAvatar::Preset { icon, .. } = s.avatar.kind {
        Some(icon)
    } else {
        None
    };
    let mut rows: Vec<Element<Msg>> = Vec::new();
    let mut cur: Vec<Element<Msg>> = Vec::new();
    for i in 0..avatar::ICON_COUNT {
        let tint = i % avatar::TINT_COUNT;
        // The press route (F5) is wired for containers, not for a bare `icon()`
        // (the doodle badge), so the pressable is the CELL container — the click on
        // the doodle child bubbles to it.
        let cell = column![avatar::doodle_avatar_forced::<Msg>(i, tint, 44.0)]
            .on_press(Msg::PickGalleryIcon(i))
            .label(format!("Doodle {i}"))
            .padding(Space::Xs)
            .radius(Radius::Lg)
            .background(if selected == Some(i) {
                p.accent_tint
            } else {
                CLEAR
            });
        cur.push(cell);
        if cur.len() == 5 {
            rows.push(Element::row(std::mem::take(&mut cur)).gap(Space::Xs));
        }
    }
    if !cur.is_empty() {
        rows.push(Element::row(cur).gap(Space::Xs));
    }
    Element::column(rows).gap(Space::Xs).align_center()
}

/// The DRAW-YOUR-OWN tab: the 220×220 avatar canvas + swatches / sizes / tools / save.
fn avatar_draw(s: &Dooduel) -> Element<Msg> {
    let a = &s.avatar;
    let p = s.palette();
    let canvas = column![raster(s.avatar_canvas.clone(), 220.0, 220.0)]
        .width(228.0)
        .background(WHITE)
        .radius(Radius::Lg)
        .border(3.0, p.ink, LineStyle::Solid)
        .align_center()
        .justify_center();

    // 16 swatches in two rows of 8 (the design's editor grid).
    let mut swatch_rows: Vec<Element<Msg>> = Vec::new();
    let mut cur: Vec<Element<Msg>> = Vec::new();
    for i in 0..paint::PALETTE.len() {
        cur.push(avatar_swatch(i, a, p));
        if cur.len() == 8 {
            swatch_rows.push(Element::row(std::mem::take(&mut cur)).gap(Space::Xs));
        }
    }
    if !cur.is_empty() {
        swatch_rows.push(Element::row(cur).gap(Space::Xs));
    }
    let swatches = Element::column(swatch_rows).gap(Space::Xs).align_center();

    let sizes: Vec<Element<Msg>> = (0..paint::BRUSH_SIZES.len())
        .map(|i| avatar_size_dot(i, a, p))
        .collect();
    let tools = row![
        Element::row(sizes).gap(Space::Xs),
        avatar_tool_btn("Eraser", Msg::ToggleAvatarEraser, a.draft_eraser, p),
        avatar_tool_btn("Undo", Msg::AvatarUndo, false, p),
        avatar_tool_btn("Clear", Msg::AvatarClear, false, p),
    ]
    .gap(Space::Sm)
    .align_center();

    let save = button("Use this doodle")
        .on_press(Msg::SaveAvatar)
        .background(Color::Accent)
        .color(WHITE)
        .radius(Radius::Full)
        .height(50.0)
        .size(22.0)
        .font(FONT_DISPLAY);

    column![column![canvas].align_center(), swatches, tools, save]
        .gap(Space::Md)
        .align_center()
}

/// One avatar-editor color swatch (a 26px circle; the selected one gets an accent
/// ring). Selection ignores the eraser (a swatch pick also clears the eraser).
fn avatar_swatch(i: usize, a: &AvatarState, p: crate::theme::Palette) -> Element<Msg> {
    let c = paint::PALETTE[i];
    let fill = Color::rgb(c[0], c[1], c[2]);
    let selected = !a.draft_eraser && a.draft_color_idx == i;
    let ring = if selected { Color::Accent } else { p.ink };
    Element::column(vec![])
        .width(26.0)
        .height(26.0)
        .background(fill)
        .border(2.0, ring, LineStyle::Solid)
        .radius(Radius::Full)
        .on_press(Msg::SelectAvatarColor(i))
        .label(format!("Color {i}"))
}

/// One avatar-editor brush-size dot (previews the current draw color; muted for the
/// eraser).
fn avatar_size_dot(i: usize, a: &AvatarState, p: crate::theme::Palette) -> Element<Msg> {
    let active = a.draft_size_idx == i;
    let dia = paint::BRUSH_SIZES[i] as f32;
    let dot = (dia * 0.85).max(3.0);
    let (bg, ring) = if active {
        (p.accent_tint, Color::Accent)
    } else {
        (p.surface, p.ink)
    };
    let dot_color = if a.draft_eraser {
        p.muted
    } else {
        let c = paint::PALETTE[a.draft_color_idx.min(paint::PALETTE.len() - 1)];
        Color::rgb(c[0], c[1], c[2])
    };
    column![
        Element::column(vec![])
            .width(dot)
            .height(dot)
            .background(dot_color)
            .radius(Radius::Full),
    ]
    .width(34.0)
    .height(34.0)
    .align_center()
    .justify_center()
    .background(bg)
    .border(2.0, ring, LineStyle::Solid)
    .radius(Radius::Full)
    .on_press(Msg::SelectAvatarSize(i))
    .label(format!("Brush size {}", paint::BRUSH_SIZES[i]))
}

/// A small avatar-editor tool button (Eraser toggle / Undo / Clear). Accent-tinted
/// when `active` (the eraser's on-state).
fn avatar_tool_btn(label: &str, msg: Msg, active: bool, p: crate::theme::Palette) -> Element<Msg> {
    let (bg, fg) = if active {
        (p.accent_tint, Color::Accent)
    } else {
        (p.surface, p.ink)
    };
    button(label)
        .on_press(msg)
        .background(bg)
        .color(fg)
        .size(13.0)
        .font(FONT_BODY)
        .border(2.0, p.ink, LineStyle::Solid)
        .radius(Radius::Full)
        .height(34.0)
}
