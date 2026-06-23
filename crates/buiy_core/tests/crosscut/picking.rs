//! C1: the free `hit_test` fn reads absolute position via GlobalTransform.
//!
//! The offset integration assertion (`hit_test_returns_entity_under_offset_widget`)
//! is re-homed onto the C7-owned PointerHarness (crates/buiy_verify/src/pointer.rs),
//! which drives the real layout → bridge → GlobalTransform chain with the root
//! placed at an explicit window offset — so it OBSERVES Bug 1, unlike the prior
//! hand-written single-node ResolvedLayout test. The harness is C7-owned (do not
//! recreate it here); the offset RED proof is
//! crates/buiy_verify/tests/verify_headless/pointer_offset_regression.rs.
//!
//! The remaining tests pin behaviors ORTHOGONAL to Bug 1 (the smallest-area
//! tiebreak — audit #4 — and the inclusive AABB edge contract — audit #38).
//! They hand-spawn ResolvedLayout + a matching GlobalTransform (the absolute
//! basis C1 now reads) so the geometry under test is unchanged while the entity
//! is visible to the new `&GlobalTransform` query. A bare ResolvedLayout (no
//! GlobalTransform) would be dropped by the C1 query (the no-fallback contract,
//! D2), which is itself pinned by `node_without_global_transform_is_not_picked`.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout,
    layout::Style,
    picking::{PickingPlugin, hit_test},
};
use buiy_verify::pointer::PointerHarness;

/// Spawn a Node carrying a hand-written `ResolvedLayout` AND a `GlobalTransform`
/// whose translation matches `position` (the absolute basis C1 reads). This is
/// the minimal bridge-free fixture for the Bug-1-orthogonal geometry tests
/// (smallest-area, edge semantics) — the entity is visible to the C1
/// `(ResolvedLayout, GlobalTransform)` query without standing up the full
/// layout → bridge chain.
fn spawn_node(app: &mut App, position: Vec2, size: Vec2) -> Entity {
    app.world_mut()
        .spawn((
            Node,
            ResolvedLayout { position, size },
            GlobalTransform::from_translation(position.extend(0.0)),
        ))
        .id()
}

fn picking_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app
}

#[test]
fn hit_test_returns_entity_under_offset_widget() {
    // The harness places the root at window offset (70,90); the target is the
    // offset root's content, so its ResolvedLayout.position is parent-local but
    // its GlobalTransform.translation is the accumulated absolute. A pre-C1
    // hit_test (reading ResolvedLayout.position) would look at the origin box
    // and MISS the absolute box; the fixed one HITS it.
    let mut h = PointerHarness::new();
    let target = h.spawn_offset_tree(
        Vec2::new(70.0, 90.0),
        (Node, Style::default().width_px(100.0).height_px(50.0)),
    );

    // Sanity: the target is genuinely offset (parent-local position != absolute).
    let rl = h
        .world_mut()
        .get::<ResolvedLayout>(target)
        .cloned()
        .unwrap();
    let gt = h
        .world_mut()
        .get::<GlobalTransform>(target)
        .unwrap()
        .translation()
        .truncate();
    assert_ne!(
        gt, rl.position,
        "absolute != parent-local (the offset is real)"
    );

    // hit_test lands at the ABSOLUTE box (the target's global center), not the
    // origin box where the buggy code looked.
    let center = h.global_center(target);
    assert_eq!(
        hit_test(h.world_mut(), center),
        Some(target),
        "a point at the target's GLOBAL center hits it"
    );
    assert_eq!(
        hit_test(h.world_mut(), Vec2::new(10.0, 10.0)),
        None,
        "a point in the ORIGIN box (where pre-C1 code looked) misses"
    );
}

/// A node carrying ResolvedLayout but NO GlobalTransform (hand-spawned,
/// detached, never bridged) must be ABSENT from hit_test — not silently placed
/// at ResolvedLayout.position (D2: no `unwrap_or` fallback). Guards against a
/// future change quietly re-adding the position fallback foot-gun.
#[test]
fn node_without_global_transform_is_not_picked() {
    let mut app = picking_app();
    let bare = app
        .world_mut()
        .spawn((
            Node,
            ResolvedLayout {
                position: Vec2::new(10.0, 10.0),
                size: Vec2::new(100.0, 50.0),
            },
        ))
        .id();
    // Node's #[require] graph does not pull in Transform/GlobalTransform, so a
    // hand-spawned, detached Node has neither; strip GlobalTransform anyway as
    // belt-and-suspenders against a require-graph change, and pin its absence
    // immediately before the hit_test call.
    app.world_mut().entity_mut(bare).remove::<GlobalTransform>();
    assert!(
        app.world().get::<GlobalTransform>(bare).is_none(),
        "the bare node has no GlobalTransform before hit_test"
    );
    // A point inside the hand-set ResolvedLayout box must NOT hit — the node has
    // no GlobalTransform, so it is dropped from the query.
    assert_eq!(hit_test(app.world(), Vec2::new(50.0, 30.0)), None);
}

/// Audit #4 (T2.16): top-most / z-resolution. Two overlapping nodes — a small
/// one sitting *atop* a large one — both contain the probe point. `hit_test`
/// resolves "top-most" by smallest area, so it must return the SMALLER node. A
/// flipped comparator (`area < a` -> `>`) returns the large node and reddens
/// this. (Each node carries a GlobalTransform matching its position so it is
/// visible to the C1 absolute-basis query; the geometry is unchanged.)
#[test]
fn hit_test_returns_smaller_area_node_when_overlapping() {
    let mut app = picking_app();

    // Large background panel: 200x200 at origin (area 40000).
    let large = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    // Small node on top: 40x40 at (80,80) (area 1600), fully inside `large`.
    // Spawned AFTER `large` so a "last/first-wins" bug (rather than the real
    // area comparator) would be caught regardless of iteration order.
    let small = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));

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

/// Audit #38 (T4.6): AABB edge semantics. `point_in_aabb` is **fully inclusive**
/// on all four edges — `point >= min && point <= max`, closed interval `[min,
/// max]` on both axes — so a point lying exactly ON an edge or corner is a HIT,
/// and a point one ulp OUTSIDE is a MISS. A regression that flipped an edge
/// comparator to strict (`>`/`<`) would drop the on-edge hit and redden this;
/// one that widened the box would let the just-outside probe hit. (The node
/// carries a GlobalTransform matching its position, the basis C1 reads.)
#[test]
fn hit_test_aabb_edges_are_inclusive_and_just_outside_is_a_miss() {
    let mut app = picking_app();

    // Box spanning [10,10] .. [110,60] (min inclusive, max inclusive).
    let entity = spawn_node(&mut app, Vec2::new(10.0, 10.0), Vec2::new(100.0, 50.0));
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
