//! C1: the bevy_picking backend (`emit_picks`) hit-tests in absolute space.
//!
//! The offset assertion (`pointer_over_offset_buiy_node_emits_hit`) is re-homed
//! onto the C7-owned PointerHarness, which spawns the target off the origin via
//! the real layout → bridge chain and injects a synthetic pointer through the
//! sanctioned bevy_picking path — so it observes Bug 1 (a hand-written
//! single-node ResolvedLayout cannot — spec §1). The harness is C7-owned; do not
//! recreate the injection machinery here.
//!
//! The remaining test pins a behavior ORTHOGONAL to Bug 1 (the depth rule — now
//! paint-order, audit #4 updated by C3a). It hand-spawns ResolvedLayout + a
//! matching GlobalTransform (the absolute basis C1 reads) plus a root
//! StackingContext to fix the paint order, so the geometry is unchanged while the
//! node is visible to the new `&GlobalTransform` query. NOTE: do NOT trust the
//! hand-spawned fixtures as the coordinate-correctness gate — that is the harness
//! offset test.
//!
//! C3c retired the legacy `Hovered` resource (input-event-model.md § 2.10), so
//! the `Hovered`-consumer-chain test (audit #21) is removed: the topmost-painted
//! resolution it asserted is now pinned directly on the backend's `PointerHits`
//! output by `overlapping_nodes_emit_picks_top_painted_first_with_ascending_depths`
//! (`picks[0]` is the top-painted node), and the live hover signal is
//! bevy_picking's own `Pointer<Over>`/`DirectlyHovered`, exercised on the C7
//! PointerHarness (`pointer_events_c3b.rs`).
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
    picking::{BuiyPickingBackendPlugin, PickingPlugin},
};
use buiy_verify::pointer::PointerHarness;

/// Spawn a Node carrying a hand-written `ResolvedLayout` AND a `GlobalTransform`
/// whose translation matches `position` (the absolute basis C1 reads). The
/// bridge-free fixture for the Bug-1-orthogonal backend test (depth ranking) —
/// the node is visible to the C1 `(ResolvedLayout, GlobalTransform)` query
/// without the full layout → bridge chain.
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
        ..Default::default()
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

/// Spawn a window + a `Camera2d` targeting it, returning the window entity.
/// C3b's `emit_picks` resolves the pointer's target window → this camera (the
/// real-camera fix, §3.1); without a matching camera the backend emits no hits.
/// These hand-spawned fixtures must therefore stand up a window + camera.
fn spawn_window_and_camera(app: &mut App) -> Entity {
    use bevy::camera::{Camera2d, RenderTarget};
    use bevy::window::{PrimaryWindow, Window, WindowResolution};
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));
    window
}

fn spawn_pointer_in(app: &mut App, window: Entity, position: Vec2) {
    let target = WindowRef::Entity(window).normalize(Some(window)).unwrap();
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
    let window = spawn_window_and_camera(&mut app);

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
    spawn_pointer_in(&mut app, window, Vec2::new(90.0, 90.0));

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

    // C3b camera ref + order (§3.1 / gate #7): every HitData carries the REAL
    // camera entity (not Entity::PLACEHOLDER), and the PointerHits order is
    // `camera.order + 0.5` (camera.order defaults to 0).
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<bevy::camera::Camera2d>>()
        .single(app.world())
        .expect("the one Camera2d");
    let world = app.world_mut();
    let messages = world.resource::<Messages<PointerHits>>();
    let mut cursor = messages.get_cursor();
    let hit = cursor
        .read(messages)
        .find(|h| h.picks.len() == 2)
        .expect("a PointerHits with both overlapping nodes should be emitted");
    assert_ne!(
        hit.picks[0].1.camera,
        Entity::PLACEHOLDER,
        "HitData.camera must be the real camera, not the placeholder"
    );
    assert_eq!(
        hit.picks[0].1.camera, camera,
        "HitData.camera resolves to the window's Camera2d"
    );
    assert_eq!(hit.order, 0.5, "PointerHits order == camera.order + 0.5");
}

/// **Regression — the gallery-interactivity bug (2026-06-26,
/// `docs/reports/2026-06-26-gallery-interactivity-rootcause.md`).** A node the
/// renderer skips — one carrying [`ComputedPaintSkip`] (stamped by
/// `write_paint_skip` on a `Display::None` / `CssVisibility::Hidden` / off-screen
/// subtree) — must NEVER be a pick candidate, even when it is the topmost-painted
/// box directly over an activatable node. The shipped bug: a CLOSED top-layer modal
/// `Dialog` (`CssVisibility::Hidden`, full-window, topmost in the stack) kept its
/// full layout box and absorbed EVERY click while painting nothing, so the whole
/// composed app read as non-interactive. The fix is `emit_picks` skipping any
/// `ComputedPaintSkip` node (pick-set == paint-set, the visibility analogue of the
/// pick-order == paint-order co-drive).
///
/// This pins the invariant two ways: the emitted `PointerHits` excludes the hidden
/// overlay (and resolves to the target beneath it), and a full click on the target
/// still lowers to `OnPress` — i.e. the hidden overlay neither receives nor
/// occludes the activation.
#[test]
fn paint_skipped_overlay_never_absorbs_the_click_beneath_it() {
    use bevy::picking::pointer::{PointerAction, PointerButton, PointerInput};
    use buiy_core::a11y::A11yRole;
    use buiy_core::interaction::OnPress;
    use buiy_core::render::components::{ComputedPaintSkip, SkipReason};

    let mut app = backend_app();
    // The C3b `Pointer<Click>` → `OnPress` producer lives in Buiy's `PickingPlugin`
    // (already added by `backend_app`); the a11y role gate it keys on needs no extra
    // plugin. Register the `A11yRole` type-less path is unnecessary — the role is a
    // plain component read.
    let window = spawn_window_and_camera(&mut app);

    // The activatable target — a full-window box at (0,0)..(200,200), role Button so
    // a `Pointer<Click>` on it lowers to `OnPress` (the same gate the real buttons
    // use). Default `Pickable` (blocks lower).
    let target = spawn_node(&mut app, Vec2::ZERO, Vec2::new(200.0, 200.0));
    app.world_mut().entity_mut(target).insert(A11yRole::Button);

    // A HIDDEN full-window overlay painted ON TOP of the target (the closed-modal
    // analogue): same box, higher paint order, default `Pickable` (would block), but
    // carrying `ComputedPaintSkip`. Without the fix it occludes `target` and eats the
    // click; with the fix it is excluded from the candidate set entirely.
    let hidden = spawn_node(&mut app, Vec2::ZERO, Vec2::new(200.0, 200.0));
    app.world_mut()
        .entity_mut(hidden)
        .insert(ComputedPaintSkip {
            reason: SkipReason::CssHidden,
        });
    // Paint order: target bottom, hidden TOP (the topmost-painted box).
    spawn_paint_order(&mut app, &[target, hidden]);

    // Pointer at (100,100) — inside BOTH boxes.
    let center = Vec2::new(100.0, 100.0);
    let location = {
        let t = WindowRef::Entity(window).normalize(Some(window)).unwrap();
        Location {
            target: NormalizedRenderTarget::Window(t),
            position: center,
        }
    };
    app.world_mut()
        .spawn((PointerId::Mouse, PointerLocation::new(location.clone())));
    app.update();

    // (1) Pick-set == paint-set: the emitted hits are EXACTLY the target — the
    //     paint-skipped overlay is neither hit nor an occluder.
    let picked: Vec<Entity> = {
        let world = app.world_mut();
        let messages = world.resource::<Messages<PointerHits>>();
        let mut cursor = messages.get_cursor();
        cursor
            .read(messages)
            .last()
            .expect("a PointerHits is emitted every frame the pointer targets the window")
            .picks
            .iter()
            .map(|(e, _)| *e)
            .collect()
    };
    assert!(
        !picked.contains(&hidden),
        "the paint-skipped (hidden) overlay must NEVER be a pick candidate, got {picked:?}"
    );
    assert_eq!(
        picked,
        vec![target],
        "the target beneath the hidden overlay is the sole pick (the overlay neither hits nor occludes)"
    );

    // (2) Behavioral: a full primary click on the target lowers to `OnPress`
    //     (proving the hidden overlay did not swallow the activation).
    for action in [
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        app.world_mut().write_message(PointerInput {
            pointer_id: PointerId::Mouse,
            location: location.clone(),
            action,
        });
        app.update();
    }
    let world = app.world();
    let messages = world.resource::<Messages<OnPress>>();
    let mut cursor = messages.get_cursor();
    let fired = cursor.read(messages).any(|OnPress(e)| *e == target);
    assert!(
        fired,
        "a click on the target still fires OnPress — the hidden top-layer overlay did not absorb it"
    );
}
