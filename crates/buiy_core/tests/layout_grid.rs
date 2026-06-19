//! Integration tests for grid through the full LayoutPlugin pipeline.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    FlexWrap, GridAreas, GridItem, GridLine, LayoutPlugin, Length, RepeatCount, Style, TrackSize,
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

// --- Track-size functions through the full pipeline (audit #22, T2.2) ------
//
// `layout_grid.rs` previously exercised only `Length(Fr/Px)` +
// `Repeat(AutoFill, [Px])`. `minmax`, `fit-content`, and `repeat(auto-fit, …)`
// have live arms in translate.rs but ZERO integration coverage; AutoFit in
// particular differs from AutoFill (it collapses empty tracks). These drive
// each through `sync_styles → Taffy → write_resolved_layout` and assert the
// observable resolved geometry.

#[test]
fn grid_minmax_100px_1fr_single_column_fills_400px() {
    // One column `minmax(100px, 1fr)` in a 400px grid: the 1fr max lets the
    // track grow to fill the whole 400px (>= the 100px floor). The single
    // child therefore resolves to width 400.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(400.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::MinMax(vec![
            TrackSize::Length(Length::Px(100.0)),
            TrackSize::Length(Length::Fr(1.0)),
        ])]);
    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let child = app
        .world_mut()
        .spawn((Style::default().height_px(100.0), Node))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);
    app.update();

    let rl = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("ResolvedLayout written");
    assert!(
        (rl.size.x - 400.0).abs() < 0.5,
        "minmax(100px,1fr) track grows to fill 400px; child width = {}",
        rl.size.x
    );
}

#[test]
fn grid_minmax_floor_applies_when_below_min() {
    // `minmax(150px, 1fr)` in a grid only 100px wide: the 150px FLOOR wins
    // over the available 100px, so the track (and child) resolve to 150 —
    // overflowing the container. This pins the MIN slot specifically (a track
    // that ignored the floor would resolve to 100).
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(100.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::MinMax(vec![
            TrackSize::Length(Length::Px(150.0)),
            TrackSize::Length(Length::Fr(1.0)),
        ])]);
    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let child = app
        .world_mut()
        .spawn((Style::default().height_px(100.0), Node))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);
    app.update();

    let rl = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("ResolvedLayout written");
    assert!(
        (rl.size.x - 150.0).abs() < 0.5,
        "minmax(150px,1fr) floor pins track at 150px; child width = {}",
        rl.size.x
    );
}

#[test]
fn grid_fit_content_track_is_content_sized_not_fixed() {
    // `fit-content(120px)` resolves to `max(min-content, min(max-content,
    // 120px))`. To make the 120px LIMIT load-bearing (audit #22 nit), the
    // track content must be WIDER than 120 so the `min(max-content, 120)` clamp
    // actually bites. The fit-content child is a flex-row WRAP box of three
    // 50px items: min-content = 50 (one item per line), max-content = 150 (all
    // on one line). So the track resolves to:
    //
    //     max(50, min(150, 120)) = max(50, 120) = 120
    //
    // The trailing 1fr track soaks up the remaining 400 − 120 = 280px. This is
    // NON-vacuous in the limit: at limit 999 the clamp would not bite and the
    // track would resolve to 150 (max-content), pushing the 1fr track to x≈150.
    // Raising the fixture's 120 to 999 therefore changes the asserted width —
    // the 120 is load-bearing. (Separately, mutating production track_to_max's
    // fit-content limit is caught by `translate_fit_content_inner_max_keeps_limit`.)
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(400.0)
        .height_px(100.0)
        .grid_template_columns(vec![
            TrackSize::FitContent(Length::Px(120.0)),
            // A trailing 1fr track soaks up the remaining width so the
            // fit-content track is NOT stretched by leftover free space.
            TrackSize::Length(Length::Fr(1.0)),
        ]);
    let parent = app.world_mut().spawn((parent_style, Node)).id();
    // fit-content child: a wrap flex-row whose max-content (150px, all three
    // 50px items on one line) exceeds the 120px limit, while its min-content
    // (50px, one item per line) is below it — so the clamp resolves to 120.
    let child = app
        .world_mut()
        .spawn((
            Style::default()
                .flex_row()
                .flex_wrap(FlexWrap::Wrap)
                .height_px(50.0),
            Node,
        ))
        .id();
    let mut subitems: Vec<Entity> = Vec::new();
    for _ in 0..3 {
        subitems.push(
            app.world_mut()
                .spawn((Style::default().width_px(50.0).height_px(20.0), Node))
                .id(),
        );
    }
    app.world_mut().entity_mut(child).add_children(&subitems);
    let filler = app
        .world_mut()
        .spawn((Style::default().height_px(50.0), Node))
        .id();
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[child, filler]);
    app.update();

    let child_rl = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("child ResolvedLayout");
    let filler_rl = app
        .world()
        .get::<ResolvedLayout>(filler)
        .expect("filler ResolvedLayout");
    // The fit-content track clamps at its 120px limit (NOT the 150px
    // max-content, and NOT a fixed reservation): the wrap box is exactly 120.
    assert!(
        (child_rl.size.x - 120.0).abs() < 0.5,
        "fit-content(120) clamps the 150px-max-content track to 120; got width = {}",
        child_rl.size.x
    );
    // The 1fr track starts right after the clamped 120px track …
    assert!(
        (filler_rl.position.x - 120.0).abs() < 0.5,
        "1fr track starts after the 120px fit-content track; got x = {}",
        filler_rl.position.x
    );
    // … and soaks up the remaining 400 − 120 = 280px.
    assert!(
        (filler_rl.size.x - 280.0).abs() < 0.5,
        "1fr track fills the residual 280px; got width = {}",
        filler_rl.size.x
    );
}

#[test]
fn grid_repeat_auto_fit_places_two_children_in_first_two_tracks() {
    // Plain auto-placement smoke for `repeat(auto-fit, [100px])`: two children
    // land in the first two 100px tracks (x = 0, 100). NOTE: this geometry is
    // IDENTICAL under auto-fill and auto-fit — it does NOT distinguish the two
    // (the collapse only shows once an EMPTY track would otherwise be kept).
    // The auto-fit-vs-auto-fill COLLAPSE distinction is pinned separately by
    // `grid_repeat_auto_fit_collapses_empty_tracks` below.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(350.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::Repeat(
            RepeatCount::AutoFit,
            vec![TrackSize::Length(Length::Px(100.0))],
        )]);
    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let c0 = app
        .world_mut()
        .spawn((Style::default().height_px(100.0), Node))
        .id();
    let c1 = app
        .world_mut()
        .spawn((Style::default().height_px(100.0), Node))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[c0, c1]);
    app.update();

    let layouts = capture_layouts(app.world(), &[c0, c1]);
    // Two occupied 100px tracks: x = 0 and 100.
    assert!(
        (layouts[0].0.x - 0.0).abs() < 0.5,
        "auto-fit child 0 x = {}",
        layouts[0].0.x
    );
    assert!(
        (layouts[0].1.x - 100.0).abs() < 0.5,
        "auto-fit child 0 width = {}",
        layouts[0].1.x
    );
    assert!(
        (layouts[1].0.x - 100.0).abs() < 0.5,
        "auto-fit child 1 x = {}",
        layouts[1].0.x
    );
}

#[test]
fn grid_repeat_auto_fit_collapses_empty_tracks() {
    // auto-FIT collapses empty tracks; auto-FILL keeps them. Observe the
    // distinction via an EXPLICIT placement at grid-column line 3:
    //
    //   `repeat(auto-fit, [100px])` in a 350px grid generates 3 candidate
    //   100px tracks (lines 1..=4). The ONLY child is pinned to
    //   `grid-column-start: 3` (1-indexed), so tracks 1 and 2 hold no item.
    //
    //   - Under auto-FILL the 2 leading tracks are KEPT at 100px each, so line
    //     3 sits at x≈200 and the explicit child lands there.
    //   - Under auto-FIT the 2 empty leading tracks COLLAPSE to zero, so line 3
    //     no longer sits 200px in; the explicit child falls back to x≈0 (and
    //     occupies the one non-collapsed 100px track).
    //
    // We assert the auto-FIT position (x≈0, width≈100). This FAILS if
    // `map_repeat_count` maps AutoFit→AutoFill (the proven-vacuous mutation),
    // because then the leading tracks survive and the child lands at x≈200.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(350.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::Repeat(
            RepeatCount::AutoFit,
            vec![TrackSize::Length(Length::Px(100.0))],
        )]);
    let parent = app.world_mut().spawn((parent_style, Node)).id();
    // The sole child is pinned to grid-column line 3, leaving tracks 1 and 2
    // empty (and therefore collapsible under auto-fit).
    let line3_child = app
        .world_mut()
        .spawn((
            Style::default().height_px(100.0),
            GridItem {
                column: GridLine::Start(3),
                ..Default::default()
            },
            Node,
        ))
        .id();
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[line3_child]);
    app.update();

    let line3_rl = app
        .world()
        .get::<ResolvedLayout>(line3_child)
        .expect("line3 child ResolvedLayout");
    // Under auto-FIT the empty leading tracks 1–2 collapse, so line 3 lands at
    // x≈0. Under auto-FILL this would be ≈200 — that divergence is what makes
    // the assertion non-vacuous (AutoFit→AutoFill flips it to ≈200).
    assert!(
        line3_rl.position.x.abs() < 0.5,
        "auto-fit collapses empty leading tracks so grid-column line 3 lands at \
         x≈0 (auto-fill would keep them, giving x≈200); got x = {}",
        line3_rl.position.x
    );
    // The non-collapsed track is still a real 100px track.
    assert!(
        (line3_rl.size.x - 100.0).abs() < 0.5,
        "the surviving (occupied) track keeps its 100px width; got {}",
        line3_rl.size.x
    );
}
