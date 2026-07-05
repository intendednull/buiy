//! Lobby screen — the LIVE private room (M1 W4.5): the server-issued invite code, the
//! live roster fed by `RoomState`/`Roster`, and a host-gated Start. Before `Welcome`
//! lands it shows a "Connecting…" state; a rejected join surfaces its toast here.

use buiy::view::{Color, Element, Radius, Space, row, text};
use buiy_view::LineStyle;

use crate::avatar::{doodle_avatar, doodle_avatar_forced};
use crate::theme::{FONT_BODY, FONT_DISPLAY, POS};
use crate::view::widgets::{
    avatar_el, badge, card, card_w, eyebrow, primary_button, quiet_button, screen_root, title,
};
use crate::{Dooduel, Msg, NetState, ReplicaPlayer, WireAvatar};

pub fn lobby(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();

    // Before `Welcome` (no seat/code yet) the lobby is a connecting spinner-card.
    let connected = matches!(s.net, NetState::Connected { .. }) && !s.replica.room_code.is_empty();
    if !connected {
        return connecting(s);
    }

    let is_host = s.is_host_seat();
    let lobby_title = if is_host {
        "Invite your friends!"
    } else {
        "You're in!"
    };

    // The invite-code box (design `2.5px dashed var(--ink)`) — the SERVER-issued code.
    let code_box = row![
        text(s.replica.room_code.as_str())
            .size(24.0)
            .color(p.ink)
            .font(FONT_DISPLAY),
        text("Share this code")
            .size(14.0)
            .color(Color::Accent)
            .font(FONT_BODY),
    ]
    .justify_between()
    .align_center()
    .background(p.surface_2)
    .radius(Radius::Md)
    .border(2.5, p.ink, LineStyle::Dashed)
    .padding(Space::Sm);

    // The LIVE roster from the replica: one row per seat, host + connection state badges.
    let rows: Vec<Element<Msg>> = s
        .replica
        .players
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let (btxt, bbg, bfg) = if i == s.replica.host {
                ("Host", p.accent_tint, Color::Accent)
            } else if !pl.connected {
                ("Away", p.hair, p.muted)
            } else {
                ("Ready", p.pos_tint, POS)
            };
            row![
                row![
                    roster_avatar(s, i, pl, 38.0),
                    text(pl.name.as_str())
                        .size(16.0)
                        .color(p.ink)
                        .font(FONT_BODY),
                ]
                .gap(Space::Sm)
                .align_center(),
                badge(btxt, bbg, bfg),
            ]
            .justify_between()
            .align_center()
        })
        .collect();

    // Host-gated Start: the host gets an enabled Start (the server also host-gates the
    // intent); a guest sees a waiting line instead.
    let start: Element<Msg> = if is_host {
        primary_button("▶ Start game", Msg::StartMatch, p)
    } else {
        text("Waiting for the host to start…")
            .size(15.0)
            .color(p.muted)
            .font(FONT_BODY)
    };

    let mut children = vec![
        eyebrow("Private room"),
        title(lobby_title, 38.0, p),
        code_box,
        Element::column(rows).gap(Space::Sm),
        start,
    ];
    if let Some(msg) = &s.toast {
        children.push(toast_line(msg, p.muted));
    }
    children.push(quiet_button("Leave room", Msg::Back, p));

    screen_root(card(card_w(s, 440.0), children, p), p)
}

/// The pre-`Welcome` connecting card (spec §4.2 — the `Joining` state), with a Cancel
/// back to Home and any error toast (a failed connect reverts here first).
fn connecting(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();
    let mut children = vec![
        eyebrow("Private room"),
        title("Connecting…", 38.0, p),
        text("Reaching the server…")
            .size(15.0)
            .color(p.ink_2)
            .font(FONT_BODY),
    ];
    if let Some(msg) = &s.toast {
        children.push(toast_line(msg, Color::Accent));
    }
    children.push(quiet_button("Cancel", Msg::Back, p));
    screen_root(card(card_w(s, 440.0), children, p), p)
}

/// A toast / status line under the lobby content.
fn toast_line(msg: &str, color: Color) -> Element<Msg> {
    text(msg).size(14.0).color(color).font(FONT_BODY)
}

/// The roster avatar for seat `i`: this client's own local avatar when it's us, else the
/// other player's WIRE avatar (a `Preset` renders its icon/tint; otherwise the
/// name-hashed doodle — the M1 custom-avatar-over-wire gap, see `net::wire_avatar`).
fn roster_avatar(s: &Dooduel, i: usize, pl: &ReplicaPlayer, px: f32) -> Element<Msg> {
    if i == s.replica.my_seat {
        avatar_el(s, true, &pl.name, px)
    } else {
        match pl.avatar {
            WireAvatar::Preset { icon, tint } => doodle_avatar_forced(icon, tint, px),
            _ => doodle_avatar(&pl.name, px),
        }
    }
}
