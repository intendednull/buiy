//! Integration tests for writing-mode through the full LayoutPlugin pipeline.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    LayoutPlugin, Length, LogicalBoxModel, Sizing, Style, WritingMode, WritingModeKind,
    WritingModeResolved,
};

#[test]
fn direction_rtl_flips_flex_row_children() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .flex_row()
        .width_px(300.0)
        .height_px(50.0)
        .rtl();
    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let mut children: Vec<Entity> = Vec::new();
    for _ in 0..3 {
        let c = app
            .world_mut()
            .spawn((Style::default().width_px(100.0).height_px(50.0), Node))
            .id();
        children.push(c);
    }
    app.world_mut().entity_mut(parent).add_children(&children);
    app.update();

    let r0 = app
        .world()
        .get::<ResolvedLayout>(children[0])
        .expect("c0")
        .position;
    let r2 = app
        .world()
        .get::<ResolvedLayout>(children[2])
        .expect("c2")
        .position;
    // Under RTL, the first child sits at the right edge, the last at the left.
    assert!(
        r0.x > r2.x,
        "rtl should put child 0 right of child 2 (got {} vs {})",
        r0.x,
        r2.x
    );
}

#[test]
fn vertical_rl_swaps_inline_block_via_logical_box_model() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let wm = WritingMode {
        mode: WritingModeKind::VerticalRl,
        ..Default::default()
    };
    let bm = LogicalBoxModel {
        inline_size: Sizing::Length(Length::Px(100.0)),
        block_size: Sizing::Length(Length::Px(50.0)),
        ..Default::default()
    }
    .to_box_model(&wm);

    let entity = app
        .world_mut()
        .spawn((
            Style {
                box_model: bm,
                writing_mode: wm,
                ..Default::default()
            },
            Node,
        ))
        .id();

    app.update();

    let rl = app.world().get::<ResolvedLayout>(entity).expect("layout");
    // inline-size 100, block-size 50 under vertical-rl → height = 100, width = 50.
    assert!((rl.size.x - 50.0).abs() < 0.5, "width = {}", rl.size.x);
    assert!((rl.size.y - 100.0).abs() < 0.5, "height = {}", rl.size.y);
}

#[test]
fn inheritance_propagates_writing_mode_to_descendant() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent = app
        .world_mut()
        .spawn((
            Style::default().writing_mode(WritingMode {
                mode: WritingModeKind::VerticalRl,
                ..Default::default()
            }),
            Node,
        ))
        .id();
    let child = app.world_mut().spawn((Style::default(), Node)).id();
    app.world_mut().entity_mut(parent).add_children(&[child]);
    app.update();

    let resolved = app
        .world()
        .get::<WritingModeResolved>(child)
        .expect("child should have WritingModeResolved after inherit pass");
    assert_eq!(resolved.mode, WritingModeKind::VerticalRl);
}

#[test]
fn sideways_rl_falls_back_to_vertical_rl_layout() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let wm = WritingMode {
        mode: WritingModeKind::SidewaysRl,
        ..Default::default()
    };
    let bm = LogicalBoxModel {
        inline_size: Sizing::Length(Length::Px(100.0)),
        block_size: Sizing::Length(Length::Px(50.0)),
        ..Default::default()
    }
    .to_box_model(&wm);

    let entity = app
        .world_mut()
        .spawn((
            Style {
                box_model: bm,
                writing_mode: wm,
                ..Default::default()
            },
            Node,
        ))
        .id();
    app.update();

    let rl = app.world().get::<ResolvedLayout>(entity).expect("layout");
    // sideways-rl falls back to vertical-rl layout: inline 100 → height, block 50 → width.
    assert!(
        (rl.size.x - 50.0).abs() < 0.5,
        "sideways-rl width = {}",
        rl.size.x
    );
    assert!(
        (rl.size.y - 100.0).abs() < 0.5,
        "sideways-rl height = {}",
        rl.size.y
    );
}
