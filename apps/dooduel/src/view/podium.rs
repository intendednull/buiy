//! Podium screen (final results) — a viewport-centered column: eyebrow + "{winner}
//! wins!" + subtitle, the three-place podium (2nd | 1st | 3rd left-to-right, the
//! winner's pedestal tallest in the center), the rest-of-field rows, and Play-again
//! / Home. The podium confetti is the decoupled `ConfettiPlugin` side effect.

use buiy::view::{Color, Element, Radius, Space, column, row, text};
use buiy_view::LineStyle;

use crate::game;
use crate::theme::{FONT_BODY, FONT_DISPLAY, WHITE};
use crate::view::widgets::{
    avatar_el, card_w, eyebrow, primary_button, quiet_button, screen_root, title,
};
use crate::{Dooduel, Msg};

/// The pedestal heights (logical px) by place. The design's `PODIUM_H` array is
/// indexed by rank so the *rendered* design gives 2nd the tallest block (an apparent
/// height/order indexing quirk in the design code); the FINAL renders the obvious
/// intent — the winner's center pedestal is the tallest (an intentional, documented
/// deviation, grounded by the real skribbl.io podium; spec §5.g, pending designer
/// ratification).
const PEDESTAL_H: [f32; 3] = [124.0, 92.0, 72.0];

pub fn podium(s: &Dooduel) -> Element<Msg> {
    let g = &s.game;
    let p = s.palette();
    let standings = g.standings();
    let winner = standings
        .first()
        .map(|(_, pl)| pl.name.clone())
        .unwrap_or_default();
    let rounds = g.config.total_rounds;
    let rounds_label = if rounds == 1 {
        "1 round".to_string()
    } else {
        format!("{rounds} rounds")
    };

    // The three pedestals in visual order: 2nd (left), 1st (center), 3rd (right).
    let mut cols: Vec<Element<Msg>> = Vec::new();
    if standings.len() > 1 {
        cols.push(podium_column(s, 2, &standings[1]));
    }
    if let Some(first) = standings.first() {
        cols.push(podium_column(s, 1, first));
    }
    if standings.len() > 2 {
        cols.push(podium_column(s, 3, &standings[2]));
    }
    let podium_row = Element::row(cols).gap(Space::Sm).align_center();

    // Everyone past the podium (rank 4+), as a simple standings list.
    let rest: Vec<Element<Msg>> = standings
        .iter()
        .enumerate()
        .skip(3)
        .map(|(i, entry)| rest_standing_row(s, i + 1, entry))
        .collect();

    let mut children = vec![
        eyebrow("Game over"),
        title(&format!("{winner} wins!"), 56.0, p),
        text!("Final scores after {rounds_label}")
            .size(16.0)
            .color(p.ink_2)
            .font(FONT_BODY),
        podium_row,
    ];
    if !rest.is_empty() {
        children.push(Element::column(rest).gap(Space::Sm).width(card_w(s, 420.0)));
    }
    children.push(
        row![
            primary_button("Play again", Msg::PlayAgain, p),
            quiet_button("Back to home", Msg::Back, p),
        ]
        .gap(Space::Md)
        .align_center(),
    );

    let content = Element::column(children)
        .gap(Space::Lg)
        .align_center()
        .width(card_w(s, 560.0));
    screen_root(content, p)
}

/// One podium pedestal column: the stair-step top spacer, the finisher's avatar /
/// name / score, and the colored rank pedestal (accent for 1st, else surface-2).
fn podium_column(s: &Dooduel, place: usize, entry: &(usize, game::Player)) -> Element<Msg> {
    let (orig_idx, p) = (entry.0, &entry.1);
    let pal = s.palette();
    let h = PEDESTAL_H[(place - 1).min(2)];
    let spacer = PEDESTAL_H[0] - h;
    let (bg, fg) = if place == 1 {
        (Color::Accent, WHITE)
    } else {
        (pal.surface_2, pal.ink_2)
    };
    // The pedestal: rounded top corners, square bottom (design `14px 18px 0 0`), ink
    // outline + `--sh-sm`.
    let pedestal = column![text!("{place}").size(28.0).color(fg).font(FONT_DISPLAY)]
        .width(112.0)
        .height(h)
        .align_center()
        .padding(Space::Xs)
        .background(bg)
        .border(2.5, pal.ink, LineStyle::Solid)
        .radius_corners(14.0, 18.0, 0.0, 0.0)
        .shadow(0.0, 2.0, 0.0, 0.0, pal.shadow_hard)
        .shadow(0.0, 1.0, 2.0, 0.0, pal.shadow_soft);

    column![
        // The stair-step: a taller pedestal lifts this whole column (equal total
        // heights ⇒ bottoms line up, tops stagger — the podium silhouette).
        Element::column(vec![]).height(spacer),
        avatar_el(s, orig_idx == 0, &p.name, 46.0),
        text(p.name.as_str())
            .size(15.0)
            .color(pal.ink)
            .font(FONT_BODY),
        text!("{}", p.score)
            .size(22.0)
            .color(pal.ink)
            .font(FONT_DISPLAY),
        pedestal,
    ]
    .width(112.0)
    .gap(Space::Xs)
    .align_center()
}

/// One rank-4+ standings row (rank / avatar / name / score).
fn rest_standing_row(s: &Dooduel, rank: usize, entry: &(usize, game::Player)) -> Element<Msg> {
    let (orig_idx, p) = (entry.0, &entry.1);
    let pal = s.palette();
    row![
        text!("{rank}")
            .width(20.0)
            .size(14.0)
            .color(pal.muted)
            .font(FONT_BODY),
        avatar_el(s, orig_idx == 0, &p.name, 34.0),
        text(p.name.as_str())
            .size(15.0)
            .color(pal.ink)
            .font(FONT_BODY)
            .grow(),
        text!("{}", p.score)
            .size(19.0)
            .color(pal.ink)
            .font(FONT_DISPLAY),
    ]
    .gap(Space::Sm)
    .align_center()
    .padding(Space::Sm)
    .background(pal.surface_2)
    .border(2.0, pal.ink, LineStyle::Solid)
    .radius(Radius::Md)
}
