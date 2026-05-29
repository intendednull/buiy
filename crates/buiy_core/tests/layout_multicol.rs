//! Phase 13 — multi-column packing integration tests (sub-pass 6c).
//! Harness: MinimalPlugins + LayoutPlugin (runs Taffy + the
//! PostTaffyOverrides chain headless). Spec: flex-and-grid.md § 3.
use bevy::prelude::*;
use buiy_core::layout::{
    ColumnCount, LayoutPlugin, MultiColumn, PostTaffyPositionOverrides, Style,
};
use buiy_core::{Node, ResolvedLayout};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

/// Spawn a multicol container of fixed content-box width/height with the
/// given `MultiColumn`, plus `n` fixed-size block children. Returns
/// (container, child entities in document order).
fn multicol_container(
    app: &mut App,
    width: f32,
    height: f32,
    mc: MultiColumn,
    child_sizes: &[(f32, f32)],
) -> (Entity, Vec<Entity>) {
    let container = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(width)
                .height_px(height)
                .multi_column(mc),
        ))
        .id();
    let mut kids = Vec::new();
    for &(w, h) in child_sizes {
        let c = app
            .world_mut()
            .spawn((Node, Style::default().width_px(w).height_px(h)))
            .id();
        app.world_mut().entity_mut(container).add_children(&[c]);
        kids.push(c);
    }
    (container, kids)
}

#[test]
fn two_column_count_packs_children_into_columns() {
    // 2 columns, gap 0, container 200x100. Three 100x40 children.
    // resolve_column_count(Count(2), None, 0, 200) → (2, 100).
    // Greedy with col_block_size = 100: col0 [c0@y0, c1@y40], col1 [c2@y0].
    // col0 x = 0, col1 x = 100.
    let mut app = app();
    let mc = MultiColumn {
        column_count: ColumnCount::Count(2),
        ..Default::default()
    };
    let (_container, kids) = multicol_container(
        &mut app,
        200.0,
        100.0,
        mc,
        &[(100.0, 40.0), (100.0, 40.0), (100.0, 40.0)],
    );
    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    // Container-content-relative offsets (plan D7).
    assert_eq!(
        overrides.by_entity.get(&kids[0]).copied(),
        Some(Vec2::new(0.0, 0.0))
    );
    assert_eq!(
        overrides.by_entity.get(&kids[1]).copied(),
        Some(Vec2::new(0.0, 40.0))
    );
    assert_eq!(
        overrides.by_entity.get(&kids[2]).copied(),
        Some(Vec2::new(100.0, 0.0))
    );
}

#[test]
fn packed_child_resolved_layout_is_container_relative() {
    // Guard for D7: the child's ResolvedLayout.position must be the
    // in-column offset (parent-relative), NOT double-counting the
    // container origin. Container at root → container origin (0,0), so
    // the child's ResolvedLayout.position equals its in-column offset.
    let mut app = app();
    let mc = MultiColumn {
        column_count: ColumnCount::Count(2),
        ..Default::default()
    };
    let (_container, kids) = multicol_container(
        &mut app,
        200.0,
        100.0,
        mc,
        &[(100.0, 40.0), (100.0, 40.0), (100.0, 40.0)],
    );
    app.update();
    let rl = app.world().get::<ResolvedLayout>(kids[2]).unwrap();
    assert_eq!(
        rl.position,
        Vec2::new(100.0, 0.0),
        "child 2 sits at col1 x=100, y=0"
    );
}

#[test]
fn no_multicol_writes_no_overrides() {
    // A plain block container with plain children writes nothing to the map.
    let mut app = app();
    let container = app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(100.0)))
        .id();
    let c = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(20.0)))
        .id();
    app.world_mut().entity_mut(container).add_children(&[c]);
    app.update();
    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        !overrides.by_entity.contains_key(&c),
        "non-multicol child untouched"
    );
}

use buiy_core::layout::{ColumnFill, LayoutWarnOnceKey, LayoutWarnedOnceSession};

#[test]
fn balanced_fill_with_oversized_child_warns_once() {
    // column_fill: Balance + a child taller than the resolved column
    // block-size → fragmentation would be needed; v1 greedy-packs and
    // warns once per session (plan D5).
    let mut app = app();
    let mc = MultiColumn {
        column_count: ColumnCount::Count(2),
        column_fill: ColumnFill::Balance,
        ..Default::default()
    };
    // Container content-box 200x100; one 100x250 child (250 > 100).
    let (_container, _kids) = multicol_container(&mut app, 200.0, 100.0, mc, &[(100.0, 250.0)]);
    app.update();
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert_eq!(
        warned
            .set
            .iter()
            .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolFragmentationDeferred))
            .count(),
        1,
        "oversized child under Balance warns once",
    );

    // A second frame does not re-warn (session-wide dedup).
    app.update();
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert_eq!(
        warned
            .set
            .iter()
            .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolFragmentationDeferred))
            .count(),
        1,
    );
}

#[test]
fn auto_fill_oversized_child_does_not_warn() {
    // column_fill: Auto does not promise balancing → no fragmentation
    // warn even for an oversized child.
    let mut app = app();
    let mc = MultiColumn {
        column_count: ColumnCount::Count(2),
        column_fill: ColumnFill::Auto,
        ..Default::default()
    };
    let (_container, _kids) = multicol_container(&mut app, 200.0, 100.0, mc, &[(100.0, 250.0)]);
    app.update();
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert_eq!(
        warned
            .set
            .iter()
            .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolFragmentationDeferred))
            .count(),
        0,
        "Auto fill does not warn",
    );
}
