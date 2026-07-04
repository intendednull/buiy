//! Lobby screen — the invite-code box + roster + host-gated Start (create ⇒ host,
//! join ⇒ guest).

use buiy::view::{Color, Element, Radius, Space, row, text};
use buiy_view::LineStyle;

use crate::game;
use crate::theme::{FONT_BODY, FONT_DISPLAY, POS};
use crate::view::widgets::{
    avatar_el, badge, card, card_w, eyebrow, preview_name, primary_button, quiet_button,
    screen_root, title,
};
use crate::{Dooduel, Msg};

pub fn lobby(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();
    let preview = preview_name(s);
    let lobby_title = if s.is_host {
        "Invite your friends!"
    } else {
        "You're in!"
    };

    // The invite-code box (design `2.5px dashed var(--ink)`). The copy-link control
    // is inert — clipboard is deferred (arboard native / async web).
    let code_box = row![
        text(s.room_code.as_str())
            .size(24.0)
            .color(p.ink)
            .font(FONT_DISPLAY),
        text("Copy link")
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

    // The roster: host sees itself as host + the bots joining; a joiner sees the host
    // + itself. Roster tuple: (name, badge text, badge bg, badge fg, is_the_human).
    let roster: Vec<(String, &str, Color, Color, bool)> = if s.is_host {
        let mut v = vec![(preview.clone(), "Host", p.accent_tint, Color::Accent, true)];
        for n in game::PRESET_NAMES {
            v.push((n.to_string(), "Joined", p.pos_tint, POS, false));
        }
        v
    } else {
        vec![
            ("Priya".to_string(), "Host", p.accent_tint, Color::Accent, false),
            (preview.clone(), "You", p.pos_tint, POS, true),
            ("Theo".to_string(), "Joined", p.pos_tint, POS, false),
            ("Sam".to_string(), "Joined", p.pos_tint, POS, false),
        ]
    };
    let rows: Vec<Element<Msg>> = roster
        .iter()
        .map(|(name, btxt, bbg, bfg, is_me)| {
            row![
                row![
                    avatar_el(s, *is_me, name, 38.0),
                    text(name.as_str()).size(16.0).color(p.ink).font(FONT_BODY),
                ]
                .gap(Space::Sm)
                .align_center(),
                badge(btxt, *bbg, *bfg),
            ]
            .justify_between()
            .align_center()
        })
        .collect();

    let inner = card(
        card_w(s, 440.0),
        vec![
            eyebrow("Private room"),
            title(lobby_title, 38.0, p),
            code_box,
            Element::column(rows).gap(Space::Sm),
            primary_button("▶ Start game", Msg::StartMatch, p),
            text("Solo demo — this room is mocked, but the flow is real.")
                .size(13.0)
                .color(p.muted)
                .font(FONT_BODY),
            quiet_button("Leave room", Msg::Back, p),
        ],
        p,
    );
    screen_root(inner, p)
}
