//! Phase 0 contract test: spawning a `Button` attaches role + label +
//! focusable + a default style; clicking a button emits `OnPress` (C3c: via the
//! bevy_picking `Pointer<Click>` producer, not the retired `Hovered` poll).
//!
//! Bevy 0.18 renamed buffered `Event` → `Message` and `EventWriter`/`EventReader`
//! → `MessageWriter`/`MessageReader`. Phase 0 leans on the `Message` flavor for
//! `OnPress` so the test reaches into `Messages<OnPress>` (not `Events<OnPress>`).

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yLabel, A11yRole},
    components::Node,
    focus::Focusable,
    layout::{BoxModel, Display, FlexParams, Overflow, Position},
    render::components::{Background, Border},
};
use buiy_widgets::{Button, OnPress, WidgetsPlugin};

/// The `#[require]` contract: spawning the **bare** `Button` marker (the
/// `bsn! { Button }` path) materializes the full widget contract that
/// `Button::new()` assembles by hand — the layout-visible Style
/// decomposition (so `sync_styles` sees it) plus paint, focus, and a11y.
/// Without this, `bsn! { Button }` would spawn a marker with no companions
/// and be useless (spec § 4.1a).
#[test]
fn bare_button_marker_materializes_the_full_required_contract() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(Button).id();
    app.update();

    let world = app.world();
    // Layout marker + the non-optional `sync_styles` style decomposition.
    assert!(world.get::<Node>(entity).is_some(), "Node");
    assert!(world.get::<Display>(entity).is_some(), "Display");
    assert!(world.get::<Position>(entity).is_some(), "Position");
    assert!(world.get::<FlexParams>(entity).is_some(), "FlexParams");
    assert!(world.get::<Overflow>(entity).is_some(), "Overflow");
    let box_model = world.get::<BoxModel>(entity).expect("BoxModel");
    // The canonical button box — **content-width** (Auto) × 32, 8px padding —
    // shared with `Button::new()` via the `button_box_model()` initializer. A
    // fixed width oversized short labels and overflowed dense footers; a button
    // now sizes to its label (an author patches `BoxModel { width }` for fixed).
    use buiy_core::layout::{Edges, Length, Sizing};
    assert_eq!(box_model.width, Sizing::Auto, "content-width");
    assert_eq!(box_model.height, Sizing::Length(Length::Px(32.0)));
    assert_eq!(box_model.padding, Edges::all(8.0));
    // Paint + interaction + a11y companions.
    assert!(world.get::<Background>(entity).is_some(), "Background");
    assert!(world.get::<Border>(entity).is_some(), "Border");
    assert!(world.get::<Focusable>(entity).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(entity).copied(),
        Some(A11yRole::Button),
        "a11y role defaults to Button"
    );
    assert!(world.get::<A11yLabel>(entity).is_some(), "A11yLabel");
}

#[test]
fn spawning_a_button_attaches_role_label_focusable_and_default_style() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(Button::new("Save")).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Button>(entity).is_some());
    assert!(world.get::<Focusable>(entity).is_some());
    assert_eq!(
        world.get::<A11yRole>(entity).copied(),
        Some(A11yRole::Button)
    );
    let label = world.get::<A11yLabel>(entity).expect("a11y label");
    assert_eq!(label.0, "Save");
}

/// Clicking a button emits `OnPress` — C3c migrated this off the legacy
/// `Hovered` resource (hand-set + `just_pressed` poll) onto the bevy_picking
/// `Pointer<E>` layer. The driving mechanism changed (a synthetic
/// `PointerInput` press+release over the button's absolute box, through the real
/// picking pipeline → `Pointer<Click>` → C3b's `pointer_click_emits_on_press`
/// producer), but the asserted intent is identical: a click on the button emits
/// the shared `OnPress` activation message for that entity.
///
/// The full Buiy backend (bevy_picking's `PickingPlugin`, Buiy's `PickingPlugin`,
/// and `BuiyPickingBackendPlugin`) is added explicitly — `CorePlugin` does not
/// pull them (the meta-crate `BuiyPlugin` does), but the C3b activation producer
/// lives in Buiy's `PickingPlugin`. The button is given a `ResolvedLayout` and a
/// matching `GlobalTransform` (the absolute basis `emit_picks` reads) plus a
/// window and `Camera2d` so the backend resolves a real camera; the pointer is
/// injected via `PointerInput`, the lessons-sanctioned synthetic path (no winit).
#[test]
fn clicking_a_button_emits_on_press() {
    use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
    use bevy::picking::pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerLocation,
    };
    use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};
    use buiy_core::ResolvedLayout;
    use buiy_core::picking::{BuiyPickingBackendPlugin, PickingPlugin};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);
    app.add_plugins(WidgetsPlugin);

    // A synthetic primary window + a Camera2d targeting it — `emit_picks`
    // resolves the pointer's window → this camera (§3.1); without a matching
    // camera the backend emits no hits.
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

    // The button at an absolute box (0,0)..(120,32). The `Button` marker carries
    // `A11yRole::Button` via its `#[require]` contract, which the C3b producer
    // keys activation on. Hand-give it the absolute basis `emit_picks` reads
    // (ResolvedLayout + a matching GlobalTransform) so the synthetic pointer hits
    // it without standing up the full layout→bridge chain.
    let entity = app.world_mut().spawn(Button::new("Save")).id();
    app.world_mut().entity_mut(entity).insert((
        ResolvedLayout {
            position: Vec2::ZERO,
            size: Vec2::new(120.0, 32.0),
        },
        GlobalTransform::IDENTITY,
    ));

    // The synthetic pointer at the button's center.
    let target = WindowRef::Entity(window).normalize(Some(window)).unwrap();
    let location = Location {
        target: NormalizedRenderTarget::Window(target),
        position: Vec2::new(60.0, 16.0),
    };
    app.world_mut()
        .spawn((PointerId::Mouse, PointerLocation::new(location.clone())));
    app.update(); // let the backend emit a hit + the hover stage register Over.

    // Inject a primary press then release at the same location — bevy_picking
    // emits `Pointer<Click>` (press + release share the target), which C3b's
    // producer lowers to `OnPress`.
    for action in [
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        app.world_mut()
            .write_message(bevy::picking::pointer::PointerInput {
                pointer_id: PointerId::Mouse,
                location: location.clone(),
                action,
            });
        app.update();
    }

    let messages = app.world().resource::<Messages<OnPress>>();
    let mut cursor = messages.get_cursor();
    let mut found = false;
    for ev in cursor.read(messages) {
        if ev.0 == entity {
            found = true;
        }
    }
    assert!(
        found,
        "OnPress message for clicked button (via the C3b Pointer<Click> producer)"
    );
}
