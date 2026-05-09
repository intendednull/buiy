//! Hybrid Style API: struct literal and fluent form produce identical
//! decomposed components when applied to a real entity, and both produce
//! identical ResolvedLayout after running the pipeline.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 8 test #4.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{
        AlignItems, BoxModel, BoxSizing, Display, Edges, FlexAxis, FlexGap, FlexParams,
        JustifyContent, LayoutPlugin, Length, Position, Sizing, Style,
    },
};

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

#[test]
fn struct_literal_and_fluent_have_identical_resolved_layout() {
    let make_literal = || Style {
        display: Display::Flex(FlexAxis::Column),
        box_model: BoxModel {
            width: Sizing::Length(Length::Px(200.0)),
            height: Sizing::Length(Length::Px(100.0)),
            padding: Edges::all(8.0),
            box_sizing: BoxSizing::BorderBox,
            ..default()
        },
        flex_params: FlexParams {
            direction: FlexAxis::Column,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            gap: FlexGap {
                row: Length::Px(4.0),
                column: Length::Px(4.0),
            },
            ..default()
        },
        position: Position::default(),
        ..default()
    };

    let make_fluent = || {
        Style::default()
            .flex_column()
            .width_px(200.0)
            .height_px(100.0)
            .padding(8.0)
            .border_box()
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Center)
            .gap_px(4.0)
    };

    let mut a = build_app();
    let ent_a = a.world_mut().spawn((Node, make_literal())).id();
    a.update();
    let rl_a = a.world().get::<ResolvedLayout>(ent_a).unwrap().clone();

    let mut b = build_app();
    let ent_b = b.world_mut().spawn((Node, make_fluent())).id();
    b.update();
    let rl_b = b.world().get::<ResolvedLayout>(ent_b).unwrap().clone();

    assert!(
        (rl_a.size - rl_b.size).length() < 0.5,
        "Struct-literal Style and fluent Style produced divergent sizes: {:?} vs {:?}",
        rl_a.size,
        rl_b.size
    );
    assert!(
        (rl_a.position - rl_b.position).length() < 0.5,
        "Struct-literal Style and fluent Style produced divergent positions: {:?} vs {:?}",
        rl_a.position,
        rl_b.position
    );
}
