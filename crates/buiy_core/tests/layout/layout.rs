use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{LayoutPlugin, LayoutTree, Style},
};
use buiy_verify::snapshot::assert_layout_snapshot;

#[test]
fn layout_resolves_a_simple_flex_row() {
    // Migrated to the shared 2-plugin headless layout stack (#35): this test
    // reads only `ResolvedLayout` via the snapshot, never `GlobalTransform`, so
    // the bare (no-TransformPlugin) builder is the right one.
    let mut app = crate::support::bare_layout_app();

    // A 200x100 flex-row root with two 50x50 children. `Name`-tagging is what
    // makes the Tier-1 layout snapshot diff-stable (entity-by-Name, never raw
    // Entity bits). The trailing per-field `(size.x - 50.0).abs() < 0.5` pair
    // is now one holistic `assert_layout_snapshot` — the .snap pins EVERY box's
    // position+size (root + both children), strictly more than the old child-
    // only width/height tolerance asserts (snapshots.md § Tier 1).
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().flex_row().width_px(200.0).height_px(100.0),
        ))
        .id();

    let child0 = app
        .world_mut()
        .spawn((
            Node,
            Name::new("row.item[0]"),
            Style::default().width_px(50.0).height_px(50.0),
        ))
        .id();
    let child1 = app
        .world_mut()
        .spawn((
            Node,
            Name::new("row.item[1]"),
            Style::default().width_px(50.0).height_px(50.0),
        ))
        .id();

    app.world_mut()
        .entity_mut(parent)
        .add_children(&[child0, child1]);

    assert_layout_snapshot(&mut app, "flex_row_basic");
}

#[test]
fn layout_tree_garbage_collects_despawned_entities() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();

    app.update();
    assert_eq!(
        app.world().non_send::<LayoutTree>().len(),
        1,
        "spawned entity registered in LayoutTree after first update",
    );

    app.world_mut().entity_mut(entity).despawn();
    app.update();

    assert!(
        app.world().non_send::<LayoutTree>().is_empty(),
        "despawned entity dropped from LayoutTree by gc system",
    );
}

#[test]
fn layout_tree_garbage_collects_within_a_single_tick() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let first = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();

    app.update();
    assert_eq!(
        app.world().non_send::<LayoutTree>().len(),
        1,
        "first entity registered after first update",
    );

    app.world_mut().entity_mut(first).despawn();
    let second = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();

    app.update();

    let tree = app.world().non_send::<LayoutTree>();
    assert_eq!(
        tree.len(),
        1,
        "exactly one entity remains after same-tick despawn+respawn",
    );
    assert!(
        app.world().get::<ResolvedLayout>(second).is_some(),
        "the surviving entity is the new one (proves it was synced after gc)",
    );
}
