//! In-game screen — the desktop 3-pane (scoreboard | canvas + toolbar | chat) and
//! the phone single-column layout, both under the fixed dark top bar, plus the
//! word-pick / turn-reveal / waiting overlays (fixed + top-layer scrims).
//!
//! Every datum reads the [`RoomReplica`] (M1 W3): the local `game::Game` is gone,
//! so the drawer/word/roster/countdown come from the authoritative mirror the
//! `Msg::Net` fold maintains — the same code path solo and (W4) networked.

use buiy::view::{
    Color, Element, Radius, Space, button, column, keyed_column, row, text, text_input, when,
};
use buiy_view::{ICON_VIEWBOX, LineStyle, icon, raster};

use crate::game::{ChatKind, ChatMsg, Phase};
use crate::paint;
use crate::theme::{
    CLEAR, DANGER, FONT_BODY, FONT_DISPLAY, INK_PANEL, INK_PANEL_ON, POS, Palette, WHITE,
    WOBBLE_PANEL,
};
use crate::view::widgets::{avatar_el, badge, eyebrow, panel, scrim, sketchy_panel, title};
use crate::{Dooduel, MOBILE_BREAKPOINT, Msg, NetState, RoomReplica, ToolState};

// The drawing canvas's on-screen display size (logical px). Smaller than the
// `CANVAS_W×CANVAS_H` image (720×450) — the raster samples the texture scaled to fit
// the center pane; `to_pixel` maps pointer→texel by the ratio, so input stays
// correct. Kept at the image's 1.6 aspect.
const CANVAS_DISP_W: f32 = 600.0;
const CANVAS_DISP_H: f32 = 375.0;
/// The canvas wrapper (canvas + a 3px ink frame) + the toolbar share this width so
/// they stack flush in the center pane.
const CENTER_W: f32 = 606.0;

/// The phone layout's side margin, matching the in-game body's `Space::Md` padding
/// so the single-column content (`viewport_w − 2·MOBILE_MARGIN`) fits exactly.
const MOBILE_MARGIN: f32 = 16.0;
/// The phone canvas display height (logical px). Shorter than the desktop 375 so the
/// header + scoreboard strip + toolbar + chat all fit a phone viewport. The raster
/// stretches to this box; `to_pixel` maps pointer→texel per-axis.
const MOBILE_CANVAS_H: f32 = 240.0;

pub fn in_game(s: &Dooduel) -> Element<Msg> {
    if s.is_mobile() {
        in_game_mobile(s)
    } else {
        in_game_desktop(s)
    }
}

/// The word-pick / turn-reveal / waiting overlays (fixed + top-layer, so out of flow
/// and painting over the canvas). Shared by the desktop + mobile layouts; `max_w`
/// clamps the modal panel to the phone width (desktop passes `f32::MAX`).
fn game_overlays(s: &Dooduel, max_w: f32) -> Vec<Element<Msg>> {
    let p = s.palette();
    let picking = s.replica.phase == Phase::Picking;
    vec![
        when(picking && s.is_drawer(), pick_overlay(s, p, max_w)),
        when(picking && !s.is_drawer(), waiting_overlay(s, p, max_w)),
        when(
            s.replica.phase == Phase::Reveal,
            reveal_overlay(s, p, max_w),
        ),
    ]
}

fn in_game_desktop(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();
    let mut children = vec![
        top_bar(s),
        Element::column(vec![header_card(s), three_pane(s)])
            .gap(Space::Md)
            .padding(Space::Lg)
            .grow(),
    ];
    children.extend(game_overlays(s, f32::MAX));
    Element::column(children).fill().background(p.canvas)
}

/// The phone in-game layout: a single column — dark top bar, then header card →
/// horizontal scoreboard strip → canvas → toolbar → chat. Sizes derive from the
/// measured viewport width. The overlays are the same `.fixed().top_layer()` scrims.
fn in_game_mobile(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();
    // Content width inside the phone side margins, capped at the design's ≤430px.
    let content_w = (s.viewport_w - 2.0 * MOBILE_MARGIN).clamp(280.0, MOBILE_BREAKPOINT);

    let body = Element::column(vec![
        header_card_mobile(s, content_w),
        scoreboard_strip(s, content_w),
        canvas_mobile(s, content_w),
        toolbar_mobile(s, content_w),
        chat_pane(s, content_w, 200.0),
    ])
    .gap(Space::Sm)
    .padding(Space::Md)
    .align_center()
    .grow();

    let mut children = vec![top_bar(s), body];
    children.extend(game_overlays(s, content_w));
    Element::column(children).fill().background(p.canvas)
}

/// The phone header card: Round + timer on one row, the word-slot row centered
/// below, the role badge centered under that.
fn header_card_mobile(s: &Dooduel, width: f32) -> Element<Msg> {
    let p = s.palette();
    let (role_text, role_bg, role_fg) = role_badge_parts(s, p);
    // The phone header renders the round as "Round r of t" (word — finding #20).
    let top = row![
        text!("Round {} of {}", s.replica.round, s.replica.total_rounds)
            .size(14.0)
            .color(p.ink_2)
            .font(FONT_BODY),
        timer_view(s),
    ]
    .justify_between()
    .align_center();

    let slots: Vec<Element<Msg>> = s
        .replica
        .word_slots()
        .into_iter()
        .map(|slot| word_slot(slot, p))
        .collect();
    let word_row = Element::row(slots).gap(Space::Xs).justify_center();
    let progress = drawer_progress_text(s);
    let role = column![
        badge(role_text, role_bg, role_fg),
        when(
            progress.is_some(),
            text(progress.unwrap_or_default())
                .size(12.0)
                .color(POS)
                .font(FONT_BODY),
        ),
    ]
    .gap(Space::Xs)
    .align_center();

    sketchy_panel(
        column![top, word_row, role]
            .gap(Space::Sm)
            .width(width)
            .background(p.surface)
            .padding(Space::Md),
        p,
    )
}

/// The phone scoreboard: a horizontally-scrolling strip of compact per-player cards
/// (avatar + name + score). F2's `.scroll_x()` matches the design's overflow-x strip;
/// this client's seat is accent-tinted.
fn scoreboard_strip(s: &Dooduel, width: f32) -> Element<Msg> {
    let p = s.palette();
    let cards: Vec<Element<Msg>> = s
        .standings()
        .into_iter()
        .map(|(i, pl)| {
            let bg = if i == s.replica.my_seat {
                p.accent_tint
            } else {
                p.surface_2
            };
            column![
                avatar_el(s, i == s.replica.my_seat, &pl.name, 34.0),
                text(pl.name.as_str())
                    .size(12.0)
                    .color(p.ink)
                    .font(FONT_BODY),
                text!("{}", pl.score)
                    .size(15.0)
                    .color(p.ink)
                    .font(FONT_DISPLAY),
            ]
            .gap(Space::Xs)
            .align_center()
            .padding(Space::Xs)
            .background(bg)
            .radius(Radius::Md)
            .width(84.0)
        })
        .collect();
    sketchy_panel(
        Element::row(cards)
            .gap(Space::Xs)
            .scroll_x()
            .width(width)
            .background(p.surface)
            .padding(Space::Sm),
        p,
    )
}

/// The phone drawing canvas: the framed `raster(...)` at full content width and a
/// fixed [`MOBILE_CANVAS_H`]. Same 3px ink frame as desktop; the pointer/touch
/// observers ride the reconciler-owned node exactly as on desktop.
fn canvas_mobile(s: &Dooduel, width: f32) -> Element<Msg> {
    let p = s.palette();
    column![raster(s.canvas.clone(), width - 6.0, MOBILE_CANVAS_H)]
        .width(width)
        .background(WHITE)
        .radius(Radius::Lg)
        .border(3.0, p.ink, LineStyle::Solid)
        .align_center()
        .justify_center()
}

/// The phone toolbar: the same brush/fill/eraser + sizes + 16 swatches + undo/clear
/// as desktop, stacked into narrow rows. The 16 swatches use F2 `.wrap()` (the
/// design's wrapping swatch grid). Dimmed when you are not the drawer.
fn toolbar_mobile(s: &Dooduel, width: f32) -> Element<Msg> {
    let t = &s.tools;
    let p = s.palette();
    let seg = row![
        tool_seg("Brush", paint::Tool::Brush, t.tool, p),
        tool_seg("Fill", paint::Tool::Bucket, t.tool, p),
        tool_seg("Eraser", paint::Tool::Eraser, t.tool, p),
    ]
    .gap(Space::Xs);
    let sizes: Vec<Element<Msg>> = (0..paint::BRUSH_SIZES.len())
        .map(|i| brush_dot(i, t, p))
        .collect();
    let swatches: Vec<Element<Msg>> = (0..paint::PALETTE.len()).map(|i| swatch(i, t, p)).collect();

    let controls = column![
        row![seg, Element::row(sizes).gap(Space::Xs)]
            .gap(Space::Sm)
            .align_center(),
        Element::row(swatches).gap(Space::Xs).wrap(),
        row![
            tool_btn("Undo", Msg::UndoStroke, p),
            tool_btn("Clear", Msg::ClearCanvas, p),
        ]
        .gap(Space::Xs),
    ]
    .gap(Space::Xs);

    column![controls]
        .width(width)
        .background(p.surface)
        .radius_corners(
            WOBBLE_PANEL[0],
            WOBBLE_PANEL[1],
            WOBBLE_PANEL[2],
            WOBBLE_PANEL[3],
        )
        .border(2.5, p.ink, LineStyle::Solid)
        .padding(Space::Sm)
        .disabled(s.replica.phase != Phase::Drawing || !s.is_drawer())
}

/// The fixed dark top bar: the session caption + the roster avatar badges (inert now
/// the hot-seat switcher is gone — this client's seat renders larger) + Leave.
fn top_bar(s: &Dooduel) -> Element<Msg> {
    // The session caption. Solo shows the demo label; a networked room shows its code.
    let caption_text = match s.net {
        NetState::Solo => "Solo demo".to_string(),
        _ => s.replica.room_code.clone(),
    };
    let caption = row![
        text(caption_text)
            .size(if s.is_mobile() { 14.0 } else { 15.0 })
            .color(INK_PANEL_ON)
            .font(FONT_BODY),
    ]
    .gap(Space::Sm)
    .align_center();

    // Roster badges: each player's doodle avatar; this client's own seat renders
    // larger (the design's highlight). Inert now — seat switching is removed.
    let mut chips: Vec<Element<Msg>> = s
        .replica
        .players
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let me = i == s.replica.my_seat;
            let px = if me { 40.0 } else { 32.0 };
            column![avatar_el(s, me, &pl.name, px)].label(&pl.name)
        })
        .collect();
    chips.push(
        button("Leave")
            .on_press(Msg::Back)
            .background(CLEAR)
            .color(INK_PANEL_ON)
            .size(15.0)
            .font(FONT_BODY),
    );
    let right = Element::row(chips).gap(Space::Sm).align_center();

    row![caption, right]
        .justify_between()
        .align_center()
        .background(INK_PANEL)
        .padding(Space::Md)
}

/// The header card: round + role badge (left), the word-slot row (center), the
/// countdown timer ring + number (right).
fn header_card(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();
    let (role_text, role_bg, role_fg) = role_badge_parts(s, p);
    // The desktop header renders the round as "Round r / t" (slash — finding #20).
    let progress = drawer_progress_text(s);
    let left = column![
        text!("Round {} / {}", s.replica.round, s.replica.total_rounds)
            .size(14.0)
            .color(p.ink_2)
            .font(FONT_BODY),
        badge(role_text, role_bg, role_fg),
        when(
            progress.is_some(),
            text(progress.clone().unwrap_or_default())
                .size(13.0)
                .color(POS)
                .font(FONT_BODY),
        ),
    ]
    .gap(Space::Xs)
    .width(180.0);

    let slots: Vec<Element<Msg>> = s
        .replica
        .word_slots()
        .into_iter()
        .map(|slot| word_slot(slot, p))
        .collect();
    let word_row = Element::row(slots).gap(Space::Xs).grow().justify_center();

    panel(
        row![left, word_row, timer_view(s)]
            .gap(Space::Lg)
            .align_center(),
        p,
    )
}

/// One underlined letter slot of the word row (blank when unrevealed). The replica's
/// [`RoomReplica::word_slots`] yields `(char, revealed)` — a `'_'` char is a blank.
fn word_slot(slot: (char, bool), p: Palette) -> Element<Msg> {
    let (ch, revealed) = slot;
    let ch_str = if ch == '_' {
        " ".to_string()
    } else {
        ch.to_string()
    };
    // The design underlines each slot (accent once revealed, ink while blank); a thin
    // colored bar stands in for `border-bottom` (no per-side border surface).
    let underline = if revealed { Color::Accent } else { p.ink };
    column![
        text(ch_str).size(30.0).color(p.ink).font(FONT_DISPLAY),
        Element::column(vec![])
            .width(26.0)
            .height(4.0)
            .background(underline),
    ]
    .gap(Space::Xs)
    .align_center()
    .width(34.0)
}

/// The countdown timer: a progress ring (an `icon` arc regenerated per displayed
/// second — a per-frame arc would churn the icon atlas) beside the seconds number,
/// danger-red under 10s. The countdown is the monotonic-anchored [`crate::Countdown`]
/// (spec §4.3), so the ring + number derive from the same whole-second value.
fn timer_view(s: &Dooduel) -> Element<Msg> {
    let secs = s.countdown.secs();
    let frac = s.countdown.fraction();
    let tcolor = if s.replica.phase == Phase::Drawing && secs <= 10 {
        DANGER
    } else {
        Color::Accent
    };
    row![
        icon::<Msg>(ring_path(frac), 60, 2.2, ICON_VIEWBOX)
            .color(tcolor)
            .width(60.0)
            .height(60.0),
        text!("{}", secs)
            .size(32.0)
            .color(tcolor)
            .font(FONT_DISPLAY),
    ]
    .gap(Space::Sm)
    .align_center()
    .width(140.0)
    .justify_center()
}

/// The progress-ring arc as an SVG `d` on the 24×24 icon viewBox: `frac` of a
/// circle, starting at 12 o'clock, sweeping clockwise. Built from short line
/// segments (round-capped by the icon stroke → a smooth arc). Regenerated once per
/// displayed second, so the content-addressed icon atlas re-rasters at most once a
/// second.
fn ring_path(frac: f32) -> String {
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 0.0 {
        return String::new();
    }
    let (cx, cy, r) = (12.0_f32, 12.0_f32, 10.0_f32);
    let segs = ((frac * 48.0).round() as i32).max(1);
    let mut d = String::with_capacity(segs as usize * 12);
    for i in 0..=segs {
        let theta = (i as f32 / segs as f32) * frac * std::f32::consts::TAU;
        let x = cx + r * theta.sin();
        let y = cy - r * theta.cos();
        if i == 0 {
            d.push_str(&format!("M{x:.2} {y:.2}"));
        } else {
            d.push_str(&format!(" L{x:.2} {y:.2}"));
        }
    }
    d
}

/// The 3-pane body: scoreboard (240) | canvas + toolbar (grow) | chat (300),
/// top-aligned (`align_start`, so a short scoreboard is not stretched tall).
fn three_pane(s: &Dooduel) -> Element<Msg> {
    row![
        scoreboard_pane(s),
        center_pane(s),
        chat_pane(s, 300.0, 556.0)
    ]
    .gap(Space::Md)
    .align_start()
}

/// The scoreboard pane: rank / avatar / name + role pill / score, sorted high→low,
/// this client's seat tinted.
fn scoreboard_pane(s: &Dooduel) -> Element<Msg> {
    let pal = s.palette();
    let mut rows: Vec<Element<Msg>> = vec![
        text("Scoreboard")
            .size(14.0)
            .color(pal.ink_2)
            .font(FONT_BODY),
    ];
    for (rank, (i, pl)) in s.standings().into_iter().enumerate() {
        let (rtext, rbg, rfg) = player_role(&s.replica, i, pal);
        let bg = if i == s.replica.my_seat {
            pal.accent_tint
        } else {
            CLEAR
        };
        rows.push(
            row![
                text!("{}", rank + 1)
                    .width(16.0)
                    .size(13.0)
                    .color(pal.muted)
                    .font(FONT_BODY),
                avatar_el(s, i == s.replica.my_seat, &pl.name, 34.0),
                column![
                    text(pl.name.as_str())
                        .size(14.0)
                        .color(pal.ink)
                        .font(FONT_BODY),
                    badge(rtext, rbg, rfg),
                ]
                .gap(Space::Xs)
                .grow(),
                text!("{}", pl.score)
                    .size(19.0)
                    .color(pal.ink)
                    .font(FONT_DISPLAY),
            ]
            .gap(Space::Sm)
            .align_center()
            .padding(Space::Xs)
            .background(bg)
            .radius(Radius::Md),
        );
    }
    Element::column(rows)
        .gap(Space::Xs)
        .width(240.0)
        .background(pal.surface)
        .radius_corners(
            WOBBLE_PANEL[0],
            WOBBLE_PANEL[1],
            WOBBLE_PANEL[2],
            WOBBLE_PANEL[3],
        )
        .border(2.5, pal.ink, LineStyle::Solid)
        .padding(Space::Md)
}

/// The center pane: the framed drawing canvas + the toolbar, both `CENTER_W` wide
/// and centered.
fn center_pane(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();
    let canvas = column![raster(s.canvas.clone(), CANVAS_DISP_W, CANVAS_DISP_H)]
        .width(CENTER_W)
        .background(WHITE)
        .radius(Radius::Lg)
        .border(3.0, p.ink, LineStyle::Solid)
        .align_center()
        .justify_center();
    column![canvas, toolbar_view(s)]
        .gap(Space::Md)
        .grow()
        .align_center()
}

/// The toolbar: brush/fill/eraser segmented control + brush sizes + the 16-color
/// swatch grid + undo/clear. The swatches use F2 `.wrap()` (the design's wrapping
/// swatch grid — one row that wraps as the width shrinks). Dimmed when not drawing.
fn toolbar_view(s: &Dooduel) -> Element<Msg> {
    let t = &s.tools;
    let p = s.palette();
    let seg = row![
        tool_seg("Brush", paint::Tool::Brush, t.tool, p),
        tool_seg("Fill", paint::Tool::Bucket, t.tool, p),
        tool_seg("Eraser", paint::Tool::Eraser, t.tool, p),
    ]
    .gap(Space::Xs);

    let sizes: Vec<Element<Msg>> = (0..paint::BRUSH_SIZES.len())
        .map(|i| brush_dot(i, t, p))
        .collect();
    let swatches: Vec<Element<Msg>> = (0..paint::PALETTE.len()).map(|i| swatch(i, t, p)).collect();

    let controls = column![
        row![
            seg,
            Element::row(sizes).gap(Space::Xs),
            row![
                tool_btn("Undo", Msg::UndoStroke, p),
                tool_btn("Clear", Msg::ClearCanvas, p),
            ]
            .gap(Space::Xs),
        ]
        .gap(Space::Md)
        .align_center(),
        Element::row(swatches).gap(Space::Xs).wrap(),
    ]
    .gap(Space::Sm);

    column![controls]
        .width(CENTER_W)
        .background(p.surface)
        .radius_corners(
            WOBBLE_PANEL[0],
            WOBBLE_PANEL[1],
            WOBBLE_PANEL[2],
            WOBBLE_PANEL[3],
        )
        .border(2.5, p.ink, LineStyle::Solid)
        .padding(Space::Md)
        // Dim the whole toolbar when you cannot draw (parity: tools disabled for
        // guessers). `.disabled(true)` on a container dims via `Opacity`.
        .disabled(s.replica.phase != Phase::Drawing || !s.is_drawer())
}

/// One segment of the brush/fill/eraser control (an accent-filled pill when active).
fn tool_seg(label: &str, tool: paint::Tool, current: paint::Tool, p: Palette) -> Element<Msg> {
    let active = tool == current;
    let (bg, fg) = if active {
        (Color::Accent, WHITE)
    } else {
        (p.surface_2, p.ink_2)
    };
    button(label)
        .on_press(Msg::SelectTool(tool))
        .background(bg)
        .color(fg)
        .size(14.0)
        .font(FONT_BODY)
        .radius(Radius::Md)
        .height(34.0)
}

/// One brush-size dot: a round button whose inner dot grows with the size, active =
/// accent-tinted.
fn brush_dot(i: usize, t: &ToolState, p: Palette) -> Element<Msg> {
    let active = t.size_idx == i;
    let dia = paint::BRUSH_SIZES[i] as f32;
    let dot = (dia * 0.85).max(3.0);
    let (bg, ring) = if active {
        (p.accent_tint, Color::Accent)
    } else {
        (p.surface, p.ink)
    };
    // The dot previews the current draw color (muted for the eraser).
    let dot_color = if t.tool == paint::Tool::Eraser {
        p.muted
    } else {
        let c = paint::PALETTE[t.color_idx.min(paint::PALETTE.len() - 1)];
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
    .on_press(Msg::SelectSize(i))
    .label(format!("Brush size {}", paint::BRUSH_SIZES[i]))
}

/// One color swatch (a 26px circle; the selected one gets an accent ring).
fn swatch(i: usize, t: &ToolState, p: Palette) -> Element<Msg> {
    let c = paint::PALETTE[i];
    let fill = Color::rgb(c[0], c[1], c[2]);
    let ring = if t.color_idx == i {
        Color::Accent
    } else {
        p.ink
    };
    Element::column(vec![])
        .width(26.0)
        .height(26.0)
        .background(fill)
        .border(2.0, ring, LineStyle::Solid)
        .radius(Radius::Full)
        .on_press(Msg::SelectColor(i))
        .label(format!("Color {i}"))
}

/// A small ghost toolbar button (Undo / Clear).
fn tool_btn(label: &str, msg: Msg, p: Palette) -> Element<Msg> {
    button(label)
        .on_press(msg)
        .background(p.surface)
        .color(p.ink)
        .size(13.0)
        .font(FONT_BODY)
        .border(2.0, p.ink, LineStyle::Solid)
        .radius(Radius::Full)
        .height(34.0)
}

/// The chat pane: title + the guess/chat log (F2 `.stick_to_bottom()` scroll — the
/// design's auto-scroll-to-bottom) + the guess input. Sized by the caller so the
/// desktop (300×556) and phone (full-width, shorter) layouts share it. The log is
/// already per-recipient filtered by the server ([`RoomReplica::chat`]).
fn chat_pane(s: &Dooduel, width: f32, height: f32) -> Element<Msg> {
    let p = s.palette();
    // The log AS THIS SEAT SEES IT (bug #4): the server addressed the private near-
    // miss nudges only to this seat, so the replica's chat is already the honest view.
    let lines = keyed_column(
        s.replica.chat.iter().cloned(),
        |m| m.seq,
        |m| chat_line(m, p),
    )
    .gap(Space::Xs)
    .grow()
    .stick_to_bottom();

    let guessed = s
        .replica
        .players
        .get(s.replica.my_seat)
        .is_some_and(|pl| pl.guessed);
    let placeholder = match s.replica.phase {
        Phase::Drawing if s.is_drawer() => "You're drawing — guessing is off",
        Phase::Drawing if guessed => "You already guessed it!",
        Phase::Drawing => "Type your guess…",
        Phase::Picking => "Waiting for the word…",
        Phase::Reveal => "Round over — see results",
        _ => "",
    };
    let input = row![
        text_input(s.chat_input.clone())
            .placeholder(placeholder)
            .on_input(Msg::SetChatInput)
            .on_submit(Msg::SubmitGuess)
            .grow(),
        button("Send")
            .on_press(Msg::SubmitGuess)
            .background(Color::Accent)
            .color(WHITE)
            .size(15.0)
            .font(FONT_DISPLAY)
            .radius(Radius::Full)
            .height(40.0),
    ]
    .gap(Space::Sm)
    .align_center();

    column![
        text("Guess the word")
            .size(14.0)
            .color(p.ink_2)
            .font(FONT_BODY),
        lines,
        input,
    ]
    .gap(Space::Sm)
    .width(width)
    .height(height)
    .background(p.surface)
    .radius_corners(
        WOBBLE_PANEL[0],
        WOBBLE_PANEL[1],
        WOBBLE_PANEL[2],
        WOBBLE_PANEL[3],
    )
    .border(2.5, p.ink, LineStyle::Solid)
    .shadow(0.0, 6.0, 0.0, 0.0, p.shadow_hard)
    .shadow(0.0, 12.0, 26.0, -8.0, p.shadow_soft)
    .padding(Space::Md)
}

/// One chat line, styled by kind (system centered+muted, correct green-tinted,
/// guesses in a neutral bubble). The design's emoji are stripped for display — the
/// bundled Latin fonts have no color-emoji glyphs (they'd render as tofu); the copy
/// still reads without them.
fn chat_line(m: &ChatMsg, p: Palette) -> Element<Msg> {
    let txt = strip_emoji(&m.text);
    match m.kind {
        ChatKind::System => {
            column![text(txt).size(13.0).color(p.muted).font(FONT_BODY)].align_center()
        }
        ChatKind::Correct => column![text(txt).size(15.0).color(POS).font(FONT_BODY)]
            .background(p.pos_tint)
            .radius(Radius::Md)
            .padding(Space::Xs),
        // A private near-miss nudge — an accent-tinted, centered pill so it reads as
        // the "So close!" toast (bug #4; per-seat filtered, only the guesser sees it).
        ChatKind::Close => column![text(txt).size(14.0).color(Color::Accent).font(FONT_BODY)]
            .align_center()
            .background(p.accent_tint)
            .radius(Radius::Full)
            .padding(Space::Xs),
        // An ordinary wrong guess: a neutral bubble (broadcast to everyone).
        ChatKind::Guess => column![text(txt).size(15.0).color(p.ink_2).font(FONT_BODY)]
            .background(p.surface_2)
            .radius(Radius::Md)
            .padding(Space::Xs),
    }
}

/// Drop emoji / pictographs from a chat string and tidy the whitespace — the Latin
/// font stack cannot render color emoji (they tofu), so the copy shows without them.
/// Keeps typographic punctuation the fonts DO have (em-dash, ellipsis, quotes — all
/// below the U+2300 symbol/emoji wall).
fn strip_emoji(s: &str) -> String {
    let cleaned: String = s.chars().filter(|c| (*c as u32) < 0x2300).collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The drawer's live "who has guessed" progress line (bug #3) — shown to the drawer
/// during the draw phase so they are never blind to how many guessers already have
/// the word. `None` for guessers / outside the draw phase.
fn drawer_progress_text(s: &Dooduel) -> Option<String> {
    if s.replica.phase == Phase::Drawing && s.is_drawer() {
        let total = s.replica.players.len().saturating_sub(1);
        let guessed = s.replica.players.iter().filter(|p| p.guessed).count();
        Some(format!("{guessed} of {total} guessed"))
    } else {
        None
    }
}

/// The role badge text + tones for the header.
fn role_badge_parts(s: &Dooduel, p: Palette) -> (&'static str, Color, Color) {
    let drawer = s.is_drawer();
    let guessed = s
        .replica
        .players
        .get(s.replica.my_seat)
        .is_some_and(|pl| pl.guessed);
    match s.replica.phase {
        Phase::Picking if drawer => ("Choosing a word", p.accent_tint, Color::Accent),
        Phase::Picking => ("Waiting for the drawer", p.hair, p.muted),
        Phase::Drawing if drawer => ("You're drawing", p.accent_tint, Color::Accent),
        Phase::Drawing if guessed => ("You guessed it!", p.pos_tint, POS),
        Phase::Drawing => ("Guess the word", p.hair, p.muted),
        Phase::Reveal => ("Round over", p.hair, p.muted),
        _ => ("", p.hair, p.muted),
    }
}

/// The per-player scoreboard role pill text + tones. The drawer tag is gated on the
/// active drawer + phase (bug #2) so a finished-match seat is never mis-tagged
/// "Drawing".
fn player_role(r: &RoomReplica, i: usize, p: Palette) -> (&'static str, Color, Color) {
    let is_drawer = r.drawer == Some(i);
    let guessed = r.players.get(i).is_some_and(|pl| pl.guessed);
    match (is_drawer, r.phase) {
        (true, Phase::Picking) => ("Picking", p.accent_tint, Color::Accent),
        (true, Phase::Drawing) => ("Drawing", p.accent_tint, Color::Accent),
        _ if guessed => ("Guessed", p.pos_tint, POS),
        _ => ("Guessing", p.hair, p.muted),
    }
}

/// The word-pick overlay (drawer, Picking): the eyebrow + a big word-choice list.
fn pick_overlay(s: &Dooduel, p: Palette, max_w: f32) -> Element<Msg> {
    let choices: Vec<Element<Msg>> = s
        .replica
        .word_choices
        .iter()
        .enumerate()
        .map(|(i, w)| {
            button(w.to_uppercase())
                .on_press(Msg::ChooseWord(i))
                .background(p.surface_2)
                .color(p.ink)
                .size(26.0)
                .font(FONT_DISPLAY)
                .radius(Radius::Lg)
                .height(64.0)
        })
        .collect();
    let panel = column![
        row![
            eyebrow("Your turn to draw"),
            text!("{}s", s.countdown.secs())
                .size(22.0)
                .color(DANGER)
                .font(FONT_DISPLAY),
        ]
        .justify_between()
        .align_center(),
        title("Pick a word!", 34.0, p),
        Element::column(choices).gap(Space::Md),
    ]
    .gap(Space::Md)
    .width(460.0_f32.min(max_w))
    .background(p.surface)
    .radius_corners(
        WOBBLE_PANEL[0],
        WOBBLE_PANEL[1],
        WOBBLE_PANEL[2],
        WOBBLE_PANEL[3],
    )
    .border(2.5, p.ink, LineStyle::Solid)
    .shadow(0.0, 6.0, 0.0, 0.0, p.shadow_hard)
    .shadow(0.0, 12.0, 26.0, -8.0, p.shadow_soft)
    .padding(Space::Xl);
    scrim(panel)
}

/// The turn-end reveal overlay: the word + per-player deltas + Continue.
fn reveal_overlay(s: &Dooduel, p: Palette, max_w: f32) -> Element<Msg> {
    let rows: Vec<Element<Msg>> = s
        .replica
        .turn_results
        .iter()
        .map(|r| {
            let (dtxt, dbg, dfg) = if r.delta > 0 {
                (format!("+{}", r.delta), p.pos_tint, POS)
            } else {
                (format!("{}", r.delta), p.hair, p.muted)
            };
            row![
                text(r.name.as_str())
                    .size(15.0)
                    .color(p.ink)
                    .font(FONT_BODY)
                    .grow(),
                badge(&dtxt, dbg, dfg),
                text!("{}", r.total)
                    .size(18.0)
                    .color(p.ink)
                    .font(FONT_DISPLAY),
            ]
            .gap(Space::Sm)
            .align_center()
            .padding(Space::Xs)
            .background(p.surface_2)
            .radius(Radius::Md)
        })
        .collect();
    // The revealed word: TurnEnded set `word_display` to the full, space-joined row,
    // so every slot is a real letter — collect them back into the word.
    let word: String = s.replica.word_slots().iter().map(|(c, _)| *c).collect();
    let panel = column![
        row![
            eyebrow("Turn over"),
            text!("next in {}s", s.countdown.secs())
                .size(14.0)
                .color(p.muted)
                .font(FONT_BODY),
        ]
        .justify_between()
        .align_center(),
        title(&format!("The word was {word}"), 30.0, p),
        Element::column(rows).gap(Space::Xs),
        button("Continue")
            .on_press(Msg::Continue)
            .background(Color::Accent)
            .color(WHITE)
            .size(22.0)
            .font(FONT_DISPLAY)
            .radius(Radius::Full)
            .height(52.0),
    ]
    .gap(Space::Md)
    .width(440.0_f32.min(max_w))
    .background(p.surface)
    .radius_corners(
        WOBBLE_PANEL[0],
        WOBBLE_PANEL[1],
        WOBBLE_PANEL[2],
        WOBBLE_PANEL[3],
    )
    .border(2.5, p.ink, LineStyle::Solid)
    .shadow(0.0, 6.0, 0.0, 0.0, p.shadow_hard)
    .shadow(0.0, 12.0, 26.0, -8.0, p.shadow_soft)
    .padding(Space::Xl);
    scrim(panel)
}

/// The "waiting for the drawer to pick" overlay (a guesser during Picking). The
/// hot-seat "Switch to {drawer}" button is gone (the switcher is removed).
fn waiting_overlay(s: &Dooduel, p: Palette, max_w: f32) -> Element<Msg> {
    let drawer = s
        .replica
        .drawer
        .and_then(|d| s.replica.players.get(d))
        .map(|pl| pl.name.clone())
        .unwrap_or_default();
    let panel = column![
        title("Hang tight!", 30.0, p),
        text!("Waiting for {} to pick a word.", drawer)
            .size(15.0)
            .color(p.ink_2)
            .font(FONT_BODY),
    ]
    .gap(Space::Md)
    .width(420.0_f32.min(max_w))
    .align_center()
    .background(p.surface)
    .radius_corners(
        WOBBLE_PANEL[0],
        WOBBLE_PANEL[1],
        WOBBLE_PANEL[2],
        WOBBLE_PANEL[3],
    )
    .border(2.5, p.ink, LineStyle::Solid)
    .shadow(0.0, 6.0, 0.0, 0.0, p.shadow_hard)
    .shadow(0.0, 12.0, 26.0, -8.0, p.shadow_soft)
    .padding(Space::Xl);
    scrim(panel)
}
