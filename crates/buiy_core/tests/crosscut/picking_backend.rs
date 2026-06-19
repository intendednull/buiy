//! `bevy_picking` backend integration test. Drives a fake `PointerLocation`
//! and asserts a `PointerHits` message fires for the entity under the pointer.
//!
//! API deviations from plan (Bevy 0.18.1 vs plan's 0.18 assumptions):
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
    CorePlugin,
    components::{Node, ResolvedLayout},
    picking::{BuiyPickingBackendPlugin, Hovered, PickingPlugin},
};

#[test]
fn pointer_over_buiy_node_emits_hit() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // bevy_picking::PickingPlugin registers PickingSystems sets and the
    // Messages<PointerHits> message resource.
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);

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

    // Build a NormalizedRenderTarget for the pointer location. The backend
    // reads only `loc.position`, so the target entity just needs to be
    // constructible — it need not correspond to a real window.
    let window_entity = Entity::PLACEHOLDER;
    let target = WindowRef::Entity(window_entity)
        .normalize(Some(window_entity))
        .unwrap();

    // Spawn a pointer at (50, 30). Real apps source this from winit; the
    // backend reads `PointerLocation` regardless of source.
    app.world_mut().spawn((
        PointerId::Mouse,
        PointerLocation::new(Location {
            target: NormalizedRenderTarget::Window(target),
            position: Vec2::new(50.0, 30.0),
        }),
    ));

    app.update();

    // PointerHits is a Message in Bevy 0.18, not an Event.
    // Read via Messages<PointerHits> resource + a fresh cursor.
    let world = app.world_mut();
    let messages = world.resource::<Messages<PointerHits>>();
    let mut cursor = messages.get_cursor();
    let any_hit = cursor
        .read(messages)
        .any(|h| h.picks.iter().any(|(e, _)| *e == entity));
    assert!(
        any_hit,
        "Buiy backend should emit a PointerHits message for the entity under the cursor"
    );
}

/// Audit #4 (T2.16): the backend's top-most / z-resolution. Two overlapping
/// nodes both contain the cursor; `emit_picks` sorts by area ascending and
/// assigns `HitData.depth` = area-rank (`backend.rs:53-60`). So `picks[0]` must
/// be the SMALLER node and the depths must ascend (0.0, 1.0). A flipped sort or
/// a dropped rank-as-depth assignment reddens this; the pre-#4 single-node test
/// could not observe either.
#[test]
fn overlapping_nodes_emit_picks_smallest_first_with_ascending_depths() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);

    // Large panel (area 40000) spawned first; small node on top (area 1600)
    // spawned second so a naive iteration-order bug can't accidentally pass.
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

    let window_entity = Entity::PLACEHOLDER;
    let target = WindowRef::Entity(window_entity)
        .normalize(Some(window_entity))
        .unwrap();

    // Cursor at (90,90): inside BOTH AABBs.
    app.world_mut().spawn((
        PointerId::Mouse,
        PointerLocation::new(Location {
            target: NormalizedRenderTarget::Window(target),
            position: Vec2::new(90.0, 90.0),
        }),
    ));

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
/// `Hovered` must be the SMALLER node (`picks[0]`), not the one beneath. No
/// existing test reads `Hovered` after a backend emit.
#[test]
fn hovered_resource_tracks_top_most_node_after_backend_emit() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);

    // Nothing hovered before any pointer is processed.
    assert_eq!(
        app.world().resource::<Hovered>().0,
        None,
        "Hovered starts empty"
    );

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

    let window_entity = Entity::PLACEHOLDER;
    let target = WindowRef::Entity(window_entity)
        .normalize(Some(window_entity))
        .unwrap();

    app.world_mut().spawn((
        PointerId::Mouse,
        PointerLocation::new(Location {
            target: NormalizedRenderTarget::Window(target),
            position: Vec2::new(90.0, 90.0),
        }),
    ));

    app.update();

    assert_eq!(
        app.world().resource::<Hovered>().0,
        Some(small),
        "Hovered must track the top-most (smaller-area) node under the cursor, not {large:?}"
    );
}
