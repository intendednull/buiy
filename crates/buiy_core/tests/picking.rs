use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    picking::{PickingPlugin, hit_test},
};

#[test]
fn hit_test_returns_entity_under_point() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            ResolvedLayout {
                position: Vec2::new(10.0, 10.0),
                size: Vec2::new(100.0, 50.0),
            },
        ))
        .id();

    let world = app.world();
    let hit = hit_test(world, Vec2::new(50.0, 30.0));
    assert_eq!(hit, Some(entity));
    let miss = hit_test(world, Vec2::new(500.0, 500.0));
    assert_eq!(miss, None);
}

/// Audit #4 (T2.16): top-most / z-resolution. Two overlapping nodes — a small
/// one sitting *atop* a large one — both contain the probe point. `hit_test`
/// resolves "top-most" by smallest area (`picking/mod.rs:42-44`), so it must
/// return the SMALLER node. A flipped comparator (`area < a` -> `>`) returns
/// the large node and reddens this; with one node per test (the pre-#4 state)
/// this behavior is unobservable.
#[test]
fn hit_test_returns_smaller_area_node_when_overlapping() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);

    // Large background panel: 200x200 at origin (area 40000).
    let large = app
        .world_mut()
        .spawn((
            Node,
            ResolvedLayout {
                position: Vec2::new(0.0, 0.0),
                size: Vec2::new(200.0, 200.0),
            },
        ))
        .id();
    // Small node on top: 40x40 at (80,80) (area 1600), fully inside `large`.
    // Spawned AFTER `large` so a "last/first-wins" bug (rather than the real
    // area comparator) would be caught regardless of iteration order.
    let small = app
        .world_mut()
        .spawn((
            Node,
            ResolvedLayout {
                position: Vec2::new(80.0, 80.0),
                size: Vec2::new(40.0, 40.0),
            },
        ))
        .id();

    let world = app.world();

    // (90,90) is inside BOTH AABBs — the only discriminator is area.
    let hit = hit_test(world, Vec2::new(90.0, 90.0));
    assert_eq!(
        hit,
        Some(small),
        "overlapping hit must resolve to the smaller-area (top-most) node, not {large:?}"
    );

    // A point inside only the large node still resolves to it — sanity that the
    // large node is genuinely present and pickable.
    let only_large = hit_test(world, Vec2::new(10.0, 10.0));
    assert_eq!(
        only_large,
        Some(large),
        "a point outside the small node falls through to the large one"
    );
}

/// Audit #38 (T4.6): AABB edge semantics. `point_in_aabb` (`picking/mod.rs:51`)
/// is **fully inclusive** on all four edges — `point >= min && point <= max`,
/// closed interval `[min, max]` on both axes — so a point lying exactly ON an
/// edge or corner is a HIT, and a point one ulp OUTSIDE is a MISS. This pins
/// that intended boundary contract (it was previously unexercised: every prior
/// hit-test used a point strictly interior or far outside). A regression that
/// flipped an edge comparator to strict (`>`/`<`) would drop the on-edge hit and
/// redden this; one that widened the box would let the just-outside probe hit.
#[test]
fn hit_test_aabb_edges_are_inclusive_and_just_outside_is_a_miss() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);

    // Box spanning [10,10] .. [110,60] (min inclusive, max inclusive).
    let entity = app
        .world_mut()
        .spawn((
            Node,
            ResolvedLayout {
                position: Vec2::new(10.0, 10.0),
                size: Vec2::new(100.0, 50.0),
            },
        ))
        .id();
    let world = app.world();

    // Exactly on each of the four edges: a hit (the box owns its boundary).
    let min = Vec2::new(10.0, 10.0);
    let max = Vec2::new(110.0, 60.0);
    for (label, on_edge) in [
        ("top-left corner (min,min)", min),
        ("bottom-right corner (max,max)", max),
        ("left edge x==min", Vec2::new(min.x, 35.0)),
        ("right edge x==max", Vec2::new(max.x, 35.0)),
        ("top edge y==min", Vec2::new(60.0, min.y)),
        ("bottom edge y==max", Vec2::new(60.0, max.y)),
    ] {
        assert_eq!(
            hit_test(world, on_edge),
            Some(entity),
            "a point exactly on the {label} must be an inclusive-edge hit"
        );
    }

    // One ulp PAST each edge (in the exterior direction): a miss. `next_*` walks
    // to the adjacent representable f32 so the probe is the closest possible
    // outside point — the tightest witness that the interval is closed, not open.
    for (label, just_outside) in [
        (
            "left of x==min",
            Vec2::new(f32::from_bits(min.x.to_bits() - 1), 35.0),
        ),
        (
            "right of x==max",
            Vec2::new(f32::from_bits(max.x.to_bits() + 1), 35.0),
        ),
        (
            "above y==min",
            Vec2::new(60.0, f32::from_bits(min.y.to_bits() - 1)),
        ),
        (
            "below y==max",
            Vec2::new(60.0, f32::from_bits(max.y.to_bits() + 1)),
        ),
    ] {
        assert_eq!(
            hit_test(world, just_outside),
            None,
            "a point one ulp {label} must miss (the interval is closed, not wider)"
        );
    }
}
