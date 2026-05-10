//! Integration tests for grid through the full LayoutPlugin pipeline.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    GridAreas, GridItem, GridLine, LayoutPlugin, Length, RepeatCount, Style, TrackSize,
};

fn capture_layouts(world: &World, entities: &[Entity]) -> Vec<(Vec2, Vec2)> {
    entities
        .iter()
        .map(|e| {
            let rl = world
                .get::<ResolvedLayout>(*e)
                .expect("ResolvedLayout written");
            (rl.position, rl.size)
        })
        .collect()
}

#[test]
fn grid_template_1fr_2fr_1fr_in_400px_row_lays_out_100_200_100() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(400.0)
        .height_px(100.0)
        .grid_template_columns(vec![
            TrackSize::Length(Length::Fr(1.0)),
            TrackSize::Length(Length::Fr(2.0)),
            TrackSize::Length(Length::Fr(1.0)),
        ]);

    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let mut children: Vec<Entity> = Vec::new();
    for _ in 0..3 {
        let c = app
            .world_mut()
            .spawn((Style::default().height_px(100.0), Node))
            .id();
        children.push(c);
    }
    app.world_mut().entity_mut(parent).add_children(&children);

    app.update();

    let layouts = capture_layouts(app.world(), &children);

    // 1fr / 2fr / 1fr in 400 px → 100 / 200 / 100 widths.
    assert!(
        (layouts[0].1.x - 100.0).abs() < 0.5,
        "child 0 width = {}",
        layouts[0].1.x
    );
    assert!(
        (layouts[1].1.x - 200.0).abs() < 0.5,
        "child 1 width = {}",
        layouts[1].1.x
    );
    assert!(
        (layouts[2].1.x - 100.0).abs() < 0.5,
        "child 2 width = {}",
        layouts[2].1.x
    );
    // Positions: 0 / 100 / 300.
    assert!(
        (layouts[0].0.x - 0.0).abs() < 0.5,
        "child 0 x = {}",
        layouts[0].0.x
    );
    assert!(
        (layouts[1].0.x - 100.0).abs() < 0.5,
        "child 1 x = {}",
        layouts[1].0.x
    );
    assert!(
        (layouts[2].0.x - 300.0).abs() < 0.5,
        "child 2 x = {}",
        layouts[2].0.x
    );
}

#[test]
fn grid_named_areas_resolve_child_to_correct_cell() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(200.0)
        .height_px(100.0)
        .grid_template_columns(vec![
            TrackSize::Length(Length::Fr(1.0)),
            TrackSize::Length(Length::Fr(1.0)),
        ])
        .grid_template_rows(vec![
            TrackSize::Length(Length::Px(50.0)),
            TrackSize::Length(Length::Px(50.0)),
        ])
        .grid_template_areas(GridAreas::from_lines(&["a a", "b ."]));

    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let area_a_child = app
        .world_mut()
        .spawn((
            Style::default(),
            GridItem {
                column: GridLine::Area("a".to_string()),
                row: GridLine::Area("a".to_string()),
                ..Default::default()
            },
            Node,
        ))
        .id();
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[area_a_child]);

    app.update();

    let rl = app
        .world()
        .get::<ResolvedLayout>(area_a_child)
        .expect("ResolvedLayout written");
    // Area "a" spans columns 0..2 of a 2-column 200 px grid → x=0, width=200.
    // Area "a" spans rows 0..1 → y=0, height=50.
    assert!(
        (rl.position.x - 0.0).abs() < 0.5,
        "area a x = {}",
        rl.position.x
    );
    assert!(
        (rl.size.x - 200.0).abs() < 0.5,
        "area a width = {}",
        rl.size.x
    );
    assert!(
        (rl.size.y - 50.0).abs() < 0.5,
        "area a height = {}",
        rl.size.y
    );
}

#[test]
fn grid_repeat_auto_fill_in_350px_produces_three_columns() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(350.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::Repeat(
            RepeatCount::AutoFill,
            vec![TrackSize::Length(Length::Px(100.0))],
        )]);

    let parent = app.world_mut().spawn((parent_style, Node)).id();
    // Three children placed implicitly into the auto-fill columns.
    let mut children: Vec<Entity> = Vec::new();
    for _ in 0..3 {
        let c = app
            .world_mut()
            .spawn((Style::default().height_px(100.0), Node))
            .id();
        children.push(c);
    }
    app.world_mut().entity_mut(parent).add_children(&children);

    app.update();

    let layouts = capture_layouts(app.world(), &children);

    // 3 columns of 100 px each = 300 px, with 50 px slack.
    assert!(
        (layouts[0].0.x - 0.0).abs() < 0.5,
        "child 0 x = {}",
        layouts[0].0.x
    );
    assert!(
        (layouts[0].1.x - 100.0).abs() < 0.5,
        "child 0 width = {}",
        layouts[0].1.x
    );
    assert!(
        (layouts[1].0.x - 100.0).abs() < 0.5,
        "child 1 x = {}",
        layouts[1].0.x
    );
    assert!(
        (layouts[2].0.x - 200.0).abs() < 0.5,
        "child 2 x = {}",
        layouts[2].0.x
    );
}

#[test]
fn grid_cell_hosts_flex_row_with_two_children() {
    // Mixed flex-in-grid: a grid parent with one cell whose child is a
    // flex-row container that has two flex children of its own. Pins
    // spec § 5 "Mixed flex-in-grid" composition.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(200.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::Length(Length::Fr(1.0))])
        .grid_template_rows(vec![TrackSize::Length(Length::Px(100.0))]);

    let parent = app.world_mut().spawn((parent_style, Node)).id();

    // Inner: flex-row container (auto-placed into the only grid cell).
    let flex_inner = app
        .world_mut()
        .spawn((
            Style::default().flex_row().width_px(200.0).height_px(100.0),
            Node,
        ))
        .id();
    // Two flex children at width 50px each.
    let f1 = app
        .world_mut()
        .spawn((Style::default().width_px(50.0).height_px(100.0), Node))
        .id();
    let f2 = app
        .world_mut()
        .spawn((Style::default().width_px(50.0).height_px(100.0), Node))
        .id();
    app.world_mut()
        .entity_mut(flex_inner)
        .add_children(&[f1, f2]);
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[flex_inner]);

    app.update();

    let r1 = app.world().get::<ResolvedLayout>(f1).expect("f1 layout");
    let r2 = app.world().get::<ResolvedLayout>(f2).expect("f2 layout");

    // Within the flex-row's local origin, child 1 starts at x=0 and
    // child 2 at x=50. Their global x is identical because the grid
    // cell hosts the flex-row at x=0.
    assert!(
        (r1.position.x - 0.0).abs() < 0.5,
        "flex child 1 x = {}",
        r1.position.x
    );
    assert!(
        (r2.position.x - 50.0).abs() < 0.5,
        "flex child 2 x = {}",
        r2.position.x
    );
}
