//! GPU lane: the `reftest!` macro generates an `#[ignore]` test per pairing.
//! Uses the same self-match scene as the engine test to prove the macro wires
//! through to a passing run. reftests.md § "The reftest! macro".

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::Background;

fn solid_box(app: &mut App) {
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(8.0)),
                    left: Sizing::Length(Length::px(8.0)),
                    ..default()
                })
                .width_px(40.0)
                .height_px(40.0),
            Background {
                color: ColorToken::Custom(Color::srgb(0.90, 0.10, 0.10)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[e]);
}

buiy_verify::reftest!(match, macro_self_match, solid_box, solid_box);
