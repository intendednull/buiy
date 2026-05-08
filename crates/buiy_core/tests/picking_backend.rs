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
    components::{Node, ResolvedLayout, Style},
    picking::{BuiyPickingBackendPlugin, PickingPlugin},
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
            Style::default(),
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
