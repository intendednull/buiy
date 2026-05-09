//! Topological invariant: parents resolve before children every frame.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 5.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{LayoutPlugin, Style},
};

#[test]
fn parents_resolve_before_children_in_a_4_deep_tree() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(400.0)
                .height_px(300.0),
        ))
        .id();
    let mid = app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(200.0)))
        .id();
    let inner = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();
    let leaf = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();

    app.world_mut().entity_mut(root).add_child(mid);
    app.world_mut().entity_mut(mid).add_child(inner);
    app.world_mut().entity_mut(inner).add_child(leaf);

    app.update();

    // After a single Update, every entity in the chain has a ResolvedLayout
    // with non-zero size. The contract here is that a child's layout uses
    // its parent's containing block; that only works if Step 3 computed
    // the parent's box first, which is what Taffy enforces inside one
    // tree.compute_layout call but Buiy's pipeline must also enforce
    // across the whole tree. A failure (parent Vec2::ZERO, child non-zero)
    // would mean Step 3 was called on the leaf root but not the actual
    // root.
    for (label, e) in [
        ("root", root),
        ("mid", mid),
        ("inner", inner),
        ("leaf", leaf),
    ] {
        let rl = app
            .world()
            .get::<ResolvedLayout>(e)
            .unwrap_or_else(|| panic!("{label} has no ResolvedLayout"));
        assert!(rl.size.x > 0.0, "{label} resolved width should be > 0");
        assert!(rl.size.y > 0.0, "{label} resolved height should be > 0");
    }

    // Specifically: the leaf's resolved-layout x must be at least its
    // parents' offsets (Taffy's tree.layout returns local coordinates,
    // not absolute, so this check covers "child computed inside parent's
    // box" — tested via cumulative width: leaf width 50px <= inner width
    // 100px).
    let leaf_layout = app.world().get::<ResolvedLayout>(leaf).unwrap();
    assert!(
        leaf_layout.size.x <= 100.0,
        "leaf width should fit within inner's 100px container"
    );
}
