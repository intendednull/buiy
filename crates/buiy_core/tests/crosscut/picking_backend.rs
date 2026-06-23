//! C1: the bevy_picking backend (`emit_picks`) hit-tests in absolute space.
//!
//! The offset assertion (`pointer_over_offset_buiy_node_emits_hit`) is re-homed
//! onto the C7-owned PointerHarness, which spawns the target off the origin via
//! the real layout → bridge chain and injects a synthetic pointer through the
//! sanctioned bevy_picking path — so it observes Bug 1 (a hand-written
//! single-node ResolvedLayout cannot — spec §1). The harness is C7-owned; do not
//! recreate the injection machinery here.
//!
//! The remaining tests pin behaviors ORTHOGONAL to Bug 1 (the smallest-area
//! depth ranking — audit #4 — and the Hovered consumer chain — audit #21).
//! They hand-spawn ResolvedLayout + a matching GlobalTransform (the absolute
//! basis C1 reads) so the geometry is unchanged while the node is visible to the
//! new `&GlobalTransform` query. NOTE: do NOT trust the hand-spawned fixtures as
//! the coordinate-correctness gate — that is the harness offset test.
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
    CorePlugin, Node, ResolvedLayout,
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

/// Audit #4 (T2.16): the backend's top-most / z-resolution. Two overlapping
/// nodes both contain the cursor; `emit_picks` sorts by area ascending and
/// assigns `HitData.depth` = area-rank. So `picks[0]` must be the SMALLER node
/// and the depths must ascend (0.0, 1.0). A flipped sort or a dropped
/// rank-as-depth assignment reddens this. (Each node carries a GlobalTransform
/// matching its position so it is visible to the C1 absolute-basis query.)
#[test]
fn overlapping_nodes_emit_picks_smallest_first_with_ascending_depths() {
    let mut app = backend_app();

    // Large panel (area 40000) spawned first; small node on top (area 1600)
    // spawned second so a naive iteration-order bug can't accidentally pass.
    let large = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    let small = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));

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

    // picks[0] is the top-most (smallest area).
    assert_eq!(
        hit.picks[0].0, small,
        "picks[0] must be the smaller-area (top-most) node"
    );
    assert_eq!(
        hit.picks[1].0, large,
        "picks[1] must be the larger-area node beneath"
    );
    // Depths are the area rank: ascending 0.0, 1.0.
    assert_eq!(hit.picks[0].1.depth, 0.0, "top-most node has depth rank 0");
    assert_eq!(hit.picks[1].1.depth, 1.0, "node beneath has depth rank 1");
    assert!(
        hit.picks[0].1.depth < hit.picks[1].1.depth,
        "HitData depths must ascend by area rank"
    );
}

/// Audit #21 (T2.19): the `Hovered` consumer chain end-to-end. `emit_picks`
/// (PreUpdate) writes `PointerHits`; `update_hovered` (Update, `BuiySet::Picking`,
/// the only writer of `Hovered`) reads `picks.first()` and stores it. After one
/// `app.update()` the `Hovered` resource must equal the entity under the cursor.
/// With two overlapping nodes this simultaneously pins the top-most rule:
/// `Hovered` must be the SMALLER node (`picks[0]`), not the one beneath. (Each
/// node carries a GlobalTransform matching its position, the basis C1 reads.)
#[test]
fn hovered_resource_tracks_top_most_node_after_backend_emit() {
    let mut app = backend_app();

    // Nothing hovered before any pointer is processed.
    assert_eq!(
        app.world().resource::<Hovered>().0,
        None,
        "Hovered starts empty"
    );

    let large = spawn_node(&mut app, Vec2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
    let small = spawn_node(&mut app, Vec2::new(80.0, 80.0), Vec2::new(40.0, 40.0));

    spawn_pointer(&mut app, Vec2::new(90.0, 90.0));

    app.update();

    assert_eq!(
        app.world().resource::<Hovered>().0,
        Some(small),
        "Hovered must track the top-most (smaller-area) node under the cursor, not {large:?}"
    );
}
