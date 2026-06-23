//! C1: the bevy_picking backend (`emit_picks`) hit-tests in absolute space.
//!
//! The offset assertion (`pointer_over_offset_buiy_node_emits_hit`) is re-homed
//! onto the C7-owned PointerHarness, which spawns the target off the origin via
//! the real layout → bridge chain and injects a synthetic pointer through the
//! sanctioned bevy_picking path — so it observes Bug 1 (a hand-written
//! single-node ResolvedLayout cannot — spec §1). The harness is C7-owned; do not
//! recreate the injection machinery here.
//!
//! The remaining tests pin behaviors ORTHOGONAL to Bug 1 (the depth rule — now
//! paint-order, audit #4 updated by C3a — and the Hovered consumer chain — audit
//! #21). They hand-spawn ResolvedLayout + a matching GlobalTransform (the absolute
//! basis C1 reads) plus a root StackingContext to fix the paint order, so the
//! geometry is unchanged while the node is visible to the new `&GlobalTransform`
//! query. NOTE: do NOT trust the hand-spawned fixtures as the coordinate-
//! correctness gate — that is the harness offset test.
//!
//! API deviations from plan (Bevy 0.19 vs plan's 0.18 assumptions):
//! - `PointerHits` is a `Message`, not an `Event`; accessed via
//!   `Messages<PointerHits>` + `MessageCursor`, not `Events<PointerHits>`.
//! - `Location.target` is `NormalizedRenderTarget`, not `PointerTarget`.
//!   Constructed via `WindowRef::Entity(e).normalize(Some(e)).unwrap()`.
//! - `PickSet::Backend` is `PickingSystems::Backend`.
//! - Bevy's `PickingPlugin` is `bevy::picking::PickingPlugin`.

use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::message::Messages;
use bevy::picking::backend::PointerHits;
use bevy::picking::pointer::Location;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::window::WindowRef;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout, StackingContext,
    layout::Style,
    picking::{BuiyPickingBackendPlugin, Hovered, PickingPlugin},
};
use buiy_verify::pointer::PointerHarness;

/// Spawn a Node carrying a hand-written `ResolvedLayout` AND a `GlobalTransform`
/// whose translation matches `position` (the absolute basis C1 reads). The
/// bridge-free fixture for the Bug-1-orthogonal backend tests (depth ranking,
/// Hovered) — the node is visible to the C1 `(ResolvedLayout, GlobalTransform)`
/// query without the full layout → bridge chain.
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
/// forward paint order (`entities[0]` = bottom-most, `entities.last()` = topmost)
/// — the convention `global_paint_order` consumes. The root carries no
/// `ResolvedLayout`, so it is never a pick candidate; it only gives the listed
/// entities a deterministic stacking order (C3a depth = paint-order, replacing the
/// Phase-0 smallest-area rank).
fn spawn_paint_order(app: &mut App, entities: &[Entity]) {
    app.world_mut().spawn(StackingContext {
        painters_z: entities.to_vec(),
    });
}

fn backend_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // bevy_picking::PickingPlugin registers PickingSystems sets and the
    // Messages<PointerHits> message resource.
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);
    app
}

fn spawn_pointer(app: &mut App, position: Vec2) {
    let window_entity = Entity::PLACEHOLDER;
    let target = WindowRef::Entity(window_entity)
        .normalize(Some(window_entity))
        .unwrap();
    app.world_mut().spawn((
        PointerId::Mouse,
        PointerLocation::new(Location {
            target: NormalizedRenderTarget::Window(target),
            position,
        }),
    ));
}

#[test]
fn pointer_over_offset_buiy_node_emits_hit() {
    let mut h = PointerHarness::new();
    // Target placed at window offset (70,90): absolute box (70,90)..(170,140).
    let target = h.spawn_offset_tree(
        Vec2::new(70.0, 90.0),
        (Node, Style::default().width_px(100.0).height_px(50.0)),
    );
    // Aim the synthetic pointer at the target's GLOBAL center; the backend must
    // emit a hit for it. On pre-C1 code the origin-anchored rect is at
    // (0,0)..(100,50), the global center is outside it, and no hit fires.
    let center = h.global_center(target);
    h.move_to(center);
    assert_eq!(
        h.top_hit(),
        Some(target),
        "the backend must emit a hit for the OFFSET target at its absolute box \
         (Bug-1 regression; the harness drives the real layout→bridge chain)"
    );
}

/// Audit #4 (T2.16), C3a-updated: the backend's top-most / z-resolution is now
/// **paint-order**, not smallest-area. `emit_picks` sorts the geometric hits
/// top-most-painted-first and assigns `HitData.depth = paint_len - 1 -
/// paint_index`, so the topmost-painted node lands at the smallest depth (what
/// bevy_picking's ascending-depth hover sort wants). Intent preserved (overlapping
/// hits resolve to the topmost node, ascending depths); the discriminator is the
/// shared `global_paint_order`, not area. Here the LARGE node is painted ABOVE the
/// small one — the inverse of the old fixture — so the area rule (which would put
/// the SMALL node first) cannot pass this: it is a direct paint-order proof.
#[test]
fn overlapping_nodes_emit_picks_top_painted_first_with_ascending_depths() {
    let mut app = backend_app();

    // Small node below; large panel painted ON TOP of it.
    let small = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));
    let large = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    // Paint order: small bottom, large TOP. The default Pickable on `large`
    // (unmarked node) blocks lower — but here we want both reported, so make
    // `large` a non-blocking pass-through surface so `small` survives the
    // truncation and the two-pick ordering/depth contract is observable.
    app.world_mut()
        .entity_mut(large)
        .insert(bevy::picking::Pickable {
            should_block_lower: false,
            is_hoverable: true,
        });
    spawn_paint_order(&mut app, &[small, large]);

    // Cursor at (90,90): inside BOTH AABBs.
    spawn_pointer(&mut app, Vec2::new(90.0, 90.0));

    app.update();

    let world = app.world_mut();
    let messages = world.resource::<Messages<PointerHits>>();
    let mut cursor = messages.get_cursor();
    let hit = cursor
        .read(messages)
        .find(|h| h.picks.len() == 2)
        .expect("a PointerHits with both overlapping nodes should be emitted");

    // picks[0] is the top-most PAINTED node (large), not the smaller one.
    assert_eq!(
        hit.picks[0].0, large,
        "picks[0] must be the top-PAINTED node, not the smaller one"
    );
    assert_eq!(
        hit.picks[1].0, small,
        "picks[1] must be the node painted beneath"
    );
    // Depths ascend (topmost-painted is nearest). The paint order is
    // [root, small, large] (paint_len 3): large at index 2 → depth 0, small at
    // index 1 → depth 1.
    assert_eq!(
        hit.picks[0].1.depth, 0.0,
        "top-painted node has the nearest depth"
    );
    assert_eq!(
        hit.picks[1].1.depth, 1.0,
        "node beneath has a farther depth"
    );
    assert!(
        hit.picks[0].1.depth < hit.picks[1].1.depth,
        "HitData depths must ascend top-to-bottom by paint order"
    );
}

/// Audit #21 (T2.19), C3a-updated: the `Hovered` consumer chain end-to-end.
/// `emit_picks` (PreUpdate) writes `PointerHits`; `update_hovered` (Update,
/// `BuiySet::Picking`, the only writer of `Hovered`) reads `picks.first()` (the
/// topmost) and stores it. After one `app.update()` the `Hovered` resource must
/// equal the entity under the cursor. With two overlapping nodes this pins the
/// top-most rule, now **paint-order** not smallest-area: the LARGE node is painted
/// on top, so `Hovered` must be the larger one (`picks[0]`), the inverse of the
/// pre-C3a area rule. (`update_hovered` is unchanged by C3a — only the depth rule
/// feeding it changed.)
#[test]
fn hovered_resource_tracks_top_painted_node_after_backend_emit() {
    let mut app = backend_app();

    // Nothing hovered before any pointer is processed.
    assert_eq!(
        app.world().resource::<Hovered>().0,
        None,
        "Hovered starts empty"
    );

    let small = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));
    let large = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    // Paint order: small bottom, large TOP. `large` is the default-pickable
    // occluder, so it is the sole survivor and the tracked hover.
    spawn_paint_order(&mut app, &[small, large]);

    spawn_pointer(&mut app, Vec2::new(90.0, 90.0));

    app.update();

    assert_eq!(
        app.world().resource::<Hovered>().0,
        Some(large),
        "Hovered must track the top-PAINTED node under the cursor, not {small:?}"
    );
}
