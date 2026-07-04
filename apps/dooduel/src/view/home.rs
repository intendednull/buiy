//! Home screen — the wordmark + editable avatar + name field + the primary
//! Play / Create / Join CTAs + the mocked "You'll play with" roster preview.

use buiy::view::{Color, Element, Radius, Space, column, row, text, text_input};
use buiy_view::{LineStyle, icon};

use crate::avatar::{PENCIL_D, PENCIL_VIEWBOX};
use crate::game;
use crate::theme::{FONT_BODY, WHITE};
use crate::view::widgets::{
    avatar_el, card, card_w, opponent_chip, preview_name, primary_button, screen_root, soft_button,
    title,
};
use crate::{Dooduel, Msg};

pub fn home(s: &Dooduel) -> Element<Msg> {
    let preview = preview_name(s);
    let p = s.palette();

    // Logo mark: a wobbly accent circle with an ink outline (the design's hand-drawn
    // blob `border-radius:50% 50% 45% 55%/…`) beside the wordmark. The design's -8°
    // tilt is available via `.rotate()` (F4b) but the pencil glyph on it is a color
    // emoji the font stack can't render, so the mark stays a plain accent blob.
    let logo = row![
        Element::column(vec![])
            .width(46.0)
            .height(46.0)
            .background(Color::Accent)
            .radius_corners(23.0, 23.0, 21.0, 25.0)
            .border(2.5, p.ink, LineStyle::Solid),
        title("Dooduel", 54.0, p),
    ]
    .gap(Space::Sm)
    .align_center();

    // Editable avatar (the human's) + the pencil "edit" affordance beside it (the
    // design's corner ✏️ badge), then the name field. The pencil is a stroked Icon
    // (the color emoji can't render); the badge opens the avatar-editor modal.
    let edit_badge = icon::<Msg>(PENCIL_D, 16, 2.2, PENCIL_VIEWBOX)
        .width(30.0)
        .height(30.0)
        .background(Color::Accent)
        .radius(Radius::Full)
        .color(WHITE)
        .on_press(Msg::OpenAvatarEditor)
        .label("Edit your avatar");
    let name_row = row![
        row![avatar_el(s, true, &preview, 56.0), edit_badge]
            .gap(Space::Xs)
            .align_center(),
        column![
            text("Your name").size(15.0).color(p.muted).font(FONT_BODY),
            text_input(s.player_name.clone())
                .placeholder("Type a display name")
                .on_input(Msg::SetName),
        ]
        .gap(Space::Xs)
        .grow(),
    ]
    .gap(Space::Md)
    .align_center();

    // "You'll play with" preview: three mocked opponents.
    let chips: Vec<Element<Msg>> = game::PRESET_NAMES
        .iter()
        .map(|n| opponent_chip(n, p))
        .collect();
    let play_with = column![
        text("You'll play with")
            .size(13.0)
            .color(p.muted)
            .font(FONT_BODY),
        Element::row(chips).gap(Space::Md),
        text("No real opponents yet — this is a solo demo. Switch seats in-game to play everyone.")
            .size(13.0)
            .color(p.muted)
            .font(FONT_BODY),
    ]
    .gap(Space::Sm);

    let inner = card(
        card_w(s, 440.0),
        vec![
            logo,
            text("Draw it. Guess it. Repeat!")
                .size(18.0)
                .color(p.ink_2)
                .font(FONT_BODY),
            name_row,
            column![
                primary_button("▶ Play", Msg::Play, p),
                row![
                    soft_button("Create a room", Msg::CreateRoom, p),
                    soft_button("Join a room", Msg::GoJoin, p),
                ]
                .gap(Space::Sm),
            ]
            .gap(Space::Sm),
            play_with,
        ],
        p,
    );
    screen_root(inner, p)
}
