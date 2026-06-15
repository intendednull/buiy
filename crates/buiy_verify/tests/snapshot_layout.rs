//! Task 2.2 self-tests for the Tier-1 layout dump. These are PLAIN `assert_eq!`
//! (not snapshots) so the meta-tests of the snapshot tooling cannot pass
//! vacuously (snapshots.md § Verification #1, #4).

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::components::Node;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_verify::snapshot::{LAYOUT_DUMP_VERSION, assert_layout_snapshot, layout_dump};

/// Build a minimal pure-CPU layout app: a 200x100 flex-row root with two 50x50
/// children, every entity `Name`-tagged. `spawn_order` flips the order the two
/// children are spawned so the determinism test can prove `Name`-keyed output
/// is invariant to ECS spawn / archetype order.
fn flex_row_app(reversed: bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let spawn_child = |app: &mut App, label: &str, w: f32| {
        app.world_mut()
            .spawn((
                Node,
                Name::new(label.to_string()),
                Style::default().width_px(w).height_px(50.0),
            ))
            .id()
    };

    // Flip spawn order to perturb Entity allocation; the dump must not change.
    let (a, b) = if reversed {
        let b = spawn_child(&mut app, "row.item[1]", 50.0);
        let a = spawn_child(&mut app, "row.item[0]", 50.0);
        (a, b)
    } else {
        let a = spawn_child(&mut app, "row.item[0]", 50.0);
        let b = spawn_child(&mut app, "row.item[1]", 50.0);
        (a, b)
    };

    let root = app
        .world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().flex_row().width_px(200.0).height_px(100.0),
        ))
        .id();
    app.world_mut().entity_mut(root).add_children(&[a, b]);
    app
}

#[test]
fn dump_is_entity_order_invariant() {
    // snapshots.md § Verification #1: the same fixture, spawned in two different
    // entity orders, must produce a BYTE-IDENTICAL dump — the property the
    // Name-keyed sibling sort exists to guarantee.
    let mut a = flex_row_app(false);
    let mut b = flex_row_app(true);
    a.update();
    b.update();
    let da = layout_dump(a.world());
    let db = layout_dump(b.world());
    assert_eq!(
        da, db,
        "layout dump must be invariant to entity spawn order"
    );
    // And it is non-empty / structured (guards a vacuous "" == "" pass).
    assert!(da.contains("root pos="), "dump names the root by Name");
    assert!(da.contains("row.item[0]"), "dump names the first child");
}

#[test]
fn layout_dump_has_version_header() {
    // snapshots.md § Verification #4: line 1 is the format-version constant, so
    // a formatter edit that should bump the version but didn't fails here.
    let mut app = flex_row_app(false);
    app.update();
    let dump = layout_dump(app.world());
    assert_eq!(
        dump.lines().next(),
        Some(LAYOUT_DUMP_VERSION),
        "first line must be the layout dump version header"
    );
}

#[test]
fn unnamed_entity_falls_back_to_entity_index() {
    // An entity with no `Name` renders as `entity#<index>` (flagged, since an
    // unnamed fixture is non-diff-stable). Proves the fallback path.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.world_mut()
        .spawn((Node, Style::default().width_px(10.0).height_px(10.0)));
    app.update();
    let dump = layout_dump(app.world());
    assert!(
        dump.contains("entity#"),
        "unnamed entity uses the entity#<index> fallback, got:\n{dump}"
    );
}

#[test]
fn flex_row_basic_layout_snapshot() {
    // The migration target from buiy_core's layout.rs:33 also runs here as a
    // buiy_verify self-test of the full `assert_layout_snapshot` path (insta
    // bridge + dump). `.snap` lands beside THIS file.
    let mut app = flex_row_app(false);
    assert_layout_snapshot(&mut app, "flex_row_basic_selftest");
}
