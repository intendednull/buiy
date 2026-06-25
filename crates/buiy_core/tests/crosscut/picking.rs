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

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    ClipRect, CorePlugin, Node, ResolvedLayout, StackingContext,
    layout::Style,
    picking::{PickingPlugin, hit_test},
};
use buiy_verify::pointer::PointerHarness;

/// Spawn a Node carrying a hand-written `ResolvedLayout` AND a `GlobalTransform`
/// whose translation matches `position` (the absolute basis C1 reads). This is
/// the minimal bridge-free fixture for the Bug-1-orthogonal geometry tests
/// (paint-order resolution, edge semantics) — the entity is visible to the C1
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

/// Hand-build a root `StackingContext` whose `painters_z` lists `entities` in
/// forward paint order (`entities[0]` = bottom-most, `entities.last()` =
/// topmost), the convention `global_paint_order` consumes. The root carries no
/// `ResolvedLayout`, so it is never itself a pick candidate — it exists only to
/// give the listed entities a real, deterministic stacking order (C3a paint-order
/// depth replaces the Phase-0 smallest-area tiebreak). Returns the root.
fn spawn_paint_order(app: &mut App, entities: &[Entity]) -> Entity {
    app.world_mut()
        .spawn(StackingContext {
            painters_z: entities.to_vec(),
        })
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

/// Audit #4 (T2.16), C3a-updated: top-most / z-resolution is now **paint-order**,
/// not smallest-area. This preserves the audit's INTENT (overlapping hits resolve
/// to the visually-topmost node) while replacing the wrong discriminator: under
/// C3a the topmost-PAINTED node wins regardless of its area. The fixture flips
/// the old one on its head — the SMALL node is painted on top — so a regression
/// to the old smallest-area rule (which would still pick small here) is NOT what
/// proves correctness; the load-bearing proof is `overlay_above_wins_over_larger
/// _element_below`, where the LARGER element is on top. Here both orderings agree
/// (small painted topmost), pinning that paint-order resolution returns the
/// topmost-painted node when small is genuinely on top.
#[test]
fn hit_test_returns_top_painted_node_when_overlapping() {
    let mut app = picking_app();

    // Large background panel; small node fully inside it.
    let large = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    let small = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));
    // Paint order: large bottom, small TOP (small.last() == topmost).
    spawn_paint_order(&mut app, &[large, small]);

    let world = app.world();

    // (90,90) is inside BOTH AABBs — the discriminator is paint order.
    let hit = hit_test(world, Vec2::new(90.0, 90.0));
    assert_eq!(
        hit,
        Some(small),
        "overlapping hit must resolve to the TOP-PAINTED node ({small:?}), not {large:?}"
    );

    // A point inside only the large node still resolves to it.
    let only_large = hit_test(world, Vec2::new(10.0, 10.0));
    assert_eq!(
        only_large,
        Some(large),
        "a point outside the small node falls through to the large one"
    );
}

/// C3a stacking proof (a): an overlay painted ABOVE a LARGER element below wins
/// the pick — the exact case the Phase-0 smallest-area rule got WRONG (it would
/// return the smaller `below`). RED on pre-C3a behavior: smallest-area picks the
/// smaller `below` node; paint-order picks the top-painted `overlay`.
#[test]
fn overlay_above_wins_over_larger_element_below() {
    let mut app = picking_app();

    // A SMALL element below (area 1600) and a LARGE overlay on top (area 40000),
    // overlapping at (90,90). Smallest-area would pick `below`; paint-order picks
    // `overlay` because it is painted last (topmost).
    let below = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));
    let overlay = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    // Paint order: below bottom, overlay TOP.
    spawn_paint_order(&mut app, &[below, overlay]);

    let hit = hit_test(app.world(), Vec2::new(90.0, 90.0));
    assert_eq!(
        hit,
        Some(overlay),
        "the larger element painted ABOVE must win over the smaller one below \
         (pre-C3a smallest-area wrongly returns {below:?})"
    );
}

/// C3a stacking proof (b): a `should_block_lower` occluder painted over a lower
/// interactive node hides it — the lower node is NOT the hit. RED pre-C3a:
/// `Pickable` was never read, so the smaller lower node would win by area.
#[test]
fn should_block_lower_occludes_what_is_beneath() {
    let mut app = picking_app();

    // A small interactive node below; a default-pickable (should_block_lower)
    // occluder painted on top of it. Default Pickable blocks lower entities.
    let below = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));
    let occluder = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    app.world_mut()
        .entity_mut(occluder)
        .insert(Pickable::default());
    spawn_paint_order(&mut app, &[below, occluder]);

    let hit = hit_test(app.world(), Vec2::new(90.0, 90.0));
    assert_eq!(
        hit,
        Some(occluder),
        "the should_block_lower occluder hides {below:?} beneath it"
    );
}

/// C3a stacking proof (c): a `Pickable::IGNORE` decorative child painted over a
/// default-pickable widget root passes the hit THROUGH to the root. RED pre-C3a:
/// `Pickable` was never read, so the topmost (here the IGNORE child) would be
/// returned. The IGNORE label is invisible to picking; the click resolves to the
/// widget root beneath.
#[test]
fn ignore_child_passes_hit_through_to_widget_root() {
    let mut app = picking_app();

    // The widget root (default pickable, interactive) and a decorative label
    // painted ON TOP of it carrying Pickable::IGNORE.
    let root = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(100.0, 50.0));
    let label = spawn_node(&mut app, Vec2::new(10.0, 10.0), Vec2::new(80.0, 30.0));
    app.world_mut().entity_mut(label).insert(Pickable::IGNORE);
    // Paint order: root bottom, label TOP (label is visually on top).
    spawn_paint_order(&mut app, &[root, label]);

    let hit = hit_test(app.world(), Vec2::new(40.0, 25.0));
    assert_eq!(
        hit,
        Some(root),
        "a Pickable::IGNORE label ({label:?}) passes the hit through to its \
         widget root, not stolen by the decorative child"
    );
}

/// C3a stacking proof (d): a point inside a node's own AABB but OUTSIDE its
/// computed `ClipRect` (own-box ∩ ancestor clips) is NOT a hit — clip bounds are
/// honored. RED pre-C3a: `ClipRect` was never read, so the point inside the raw
/// AABB would hit regardless of the clip.
#[test]
fn point_outside_ancestor_clip_is_a_miss() {
    let mut app = picking_app();

    // A node whose AABB spans (0,0)..(200,200) but whose ancestor clip crops it
    // to (0,0)..(100,100). A point at (150,150) is inside the AABB but OUTSIDE
    // the clip → must miss.
    let clipped = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    app.world_mut().entity_mut(clipped).insert(ClipRect {
        min: Vec2::new(0.0, 0.0),
        max: Vec2::new(100.0, 100.0),
    });
    spawn_paint_order(&mut app, &[clipped]);

    // Inside the clip: a hit.
    assert_eq!(
        hit_test(app.world(), Vec2::new(50.0, 50.0)),
        Some(clipped),
        "a point inside both the AABB and the clip hits"
    );
    // Inside the AABB but outside the clip: a miss.
    assert_eq!(
        hit_test(app.world(), Vec2::new(150.0, 150.0)),
        None,
        "a point inside the raw AABB but OUTSIDE the ancestor clip must miss"
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

/// SC-3 no-divergence guarantee (input-event-model.md § 6): `global_paint_order`
/// — the picking-side global paint order — must equal render's own flatten
/// (`render::extract::context_tree_paint_order` per root) over the same
/// `StackingContext` set. This is the property that makes "pick-order ==
/// paint-order" structurally true: both walk the SAME derivation. A nested
/// context is exercised so the atomic-descent path (not just a flat root list) is
/// covered. The test captures `global_paint_order`'s output via a one-shot system
/// (the only way to obtain the live `Query`) and compares it to the hand-applied
/// render walk.
#[test]
fn global_paint_order_equals_render_context_tree_flatten() {
    use bevy::ecs::system::RunSystemOnce;
    use buiy_core::picking::global_paint_order;
    use buiy_core::render::extract::context_tree_paint_order;
    use std::collections::HashMap;

    let mut app = picking_app();

    // Tree: root R = [a, nested, b]; nested = [c, d]. Render flattens this to
    // [R, a, nested, c, d, b] (nested descended atomically at its position).
    let world = app.world_mut();
    let a = world.spawn(Node).id();
    let b = world.spawn(Node).id();
    let c = world.spawn(Node).id();
    let d = world.spawn(Node).id();
    let nested = world
        .spawn(StackingContext {
            painters_z: vec![c, d],
        })
        .id();
    let root = world
        .spawn(StackingContext {
            painters_z: vec![a, nested, b],
        })
        .id();

    // The expected order from render's OWN walk on the same map (independent
    // derivation — not re-using global_paint_order's output).
    let mut map: HashMap<Entity, Vec<Entity>> = HashMap::new();
    map.insert(root, vec![a, nested, b]);
    map.insert(nested, vec![c, d]);
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { map.get(&e).map(Vec::as_slice) };
    let mut expected = Vec::new();
    context_tree_paint_order(root, &painters_z_of, &mut expected);
    assert_eq!(
        expected,
        vec![root, a, nested, c, d, b],
        "sanity: render's flatten descends the nested context atomically"
    );

    // Capture global_paint_order's live output via a one-shot system.
    let got = app
        .world_mut()
        .run_system_once(|contexts: Query<(Entity, &StackingContext)>| {
            global_paint_order(&contexts)
        })
        .expect("one-shot global_paint_order");

    assert_eq!(
        got, expected,
        "picking's global_paint_order must equal render's context_tree_paint_order \
         flatten on the same StackingContext set (SC-3 no-divergence)"
    );
}
