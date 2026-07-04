//! Join screen — the room-code field + join CTA (reached from Home's "Join a room").

use buiy::view::{Element, Radius, Space, text, text_input};
use buiy_view::LineStyle;

use crate::theme::FONT_BODY;
use crate::view::widgets::{
    card, card_w, eyebrow, primary_button, quiet_button, screen_root, title,
};
use crate::{Dooduel, Msg};

pub fn join(s: &Dooduel) -> Element<Msg> {
    let p = s.palette();

    // The code field sits in a dashed ink box (design `2.5px dashed var(--ink)`).
    // The view `text_input` element does not lower `.border()`, so the dashed frame
    // is the wrapping container; the input fills it.
    let code_field = Element::column(vec![
        text_input(s.join_code.clone())
            .placeholder("7XQ2KP")
            .on_input(Msg::SetJoinCode)
            .on_submit(Msg::SubmitJoin)
            .fill_width(),
    ])
    .background(p.surface_2)
    .radius(Radius::Md)
    .border(2.5, p.ink, LineStyle::Dashed)
    .padding(Space::Sm);

    let inner = card(
        card_w(s, 400.0),
        vec![
            quiet_button("‹ Back", Msg::Back, p),
            eyebrow("Join a room"),
            title("Enter a room code", 38.0, p),
            text("Ask whoever's hosting for their invite code or link.")
                .size(15.0)
                .color(p.ink_2)
                .font(FONT_BODY),
            code_field,
            primary_button("Join room", Msg::SubmitJoin, p),
            text("Solo demo — any code works, you'll drop into a mocked room.")
                .size(13.0)
                .color(p.muted)
                .font(FONT_BODY),
        ],
        p,
    );
    screen_root(inner, p)
}
