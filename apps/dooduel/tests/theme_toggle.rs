//! The floating light/dark theme toggle must sit FULLY INSIDE the bottom-right
//! corner of the viewport (design `position:fixed; bottom:20px; right:20px`) — an
//! earlier `.fill()+.justify_end().align_end().padding()` form let the pill overflow
//! past the corner and get clipped. And it must stay CLICKABLE through a real
//! synthetic pointer click (the historical click-swallow guard — the pill's content-
//! sized `.ignore_picking()` top-layer container must not swallow its own click).
//!
//! Uses the unified headless driver (the GPU-free probe preset + the real picking
//! stack + a synthetic window/camera/pointer), so both the layout rect and the real
//! pointer route are exercised headless.

use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};

use buiy_core::ResolvedLayout;
use buiy_core::a11y::A11yRole;
use buiy_core::a11y::translate::entity_for_node_id;
use buiy_verify::pointer::drive_stroke;
use dooduel::theme::ThemePref;
use dooduel::{Dooduel, Screen};

const VW: f32 = 1200.0;
const VH: f32 = 760.0;

fn driver() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(bevy::picking::PickingPlugin)
        .add_plugins(buiy::BuiyProbePlugin)
        .add_plugins(buiy_core::picking::PickingPlugin)
        .add_plugins(buiy_core::picking::BuiyPickingBackendPlugin);
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(VW as u32, VH as u32),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));
    let target = WindowRef::Entity(window)
        .normalize(Some(window))
        .expect("normalize window target");
    let pointer = app
        .world_mut()
        .spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target: NormalizedRenderTarget::Window(target),
                position: Vec2::ZERO,
            }),
        ))
        .id();
    app.init_asset::<Image>();
    dooduel::install(&mut app);
    (app, window, pointer)
}

fn settle(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn model(app: &mut App) -> Dooduel {
    app.world_mut()
        .query::<&Dooduel>()
        .iter(app.world())
        .next()
        .cloned()
        .expect("model exists")
}

/// The toggle pill's window-space top-left + size.
fn pill_rect(app: &mut App) -> (Vec2, Vec2) {
    let node = buiy::probe::get_by_role(app.world_mut(), A11yRole::Button, Some("Light"), None)
        .expect("the theme toggle is a locatable button labelled 'Light'");
    let e = entity_for_node_id(node).expect("the toggle node maps to an entity");
    let world = app.world();
    let tl = world
        .get::<GlobalTransform>(e)
        .expect("pill has GlobalTransform")
        .translation()
        .truncate();
    let size = world
        .get::<ResolvedLayout>(e)
        .expect("pill has ResolvedLayout")
        .size;
    (tl, size)
}

#[test]
fn theme_toggle_pill_sits_fully_inside_the_bottom_right_corner() {
    let (mut app, _window, _pointer) = driver();
    settle(&mut app, 12);
    assert_eq!(model(&mut app).screen, Screen::Home, "starts on Home");

    let (tl, size) = pill_rect(&mut app);
    // Fully inside the viewport on all four edges.
    assert!(tl.x >= 0.0, "pill left {} < 0 (off the left edge)", tl.x);
    assert!(tl.y >= 0.0, "pill top {} < 0 (off the top edge)", tl.y);
    assert!(
        tl.x + size.x <= VW,
        "pill right {} > viewport width {VW} (clipped at the right edge)",
        tl.x + size.x
    );
    assert!(
        tl.y + size.y <= VH,
        "pill bottom {} > viewport height {VH} (clipped at the bottom edge)",
        tl.y + size.y
    );
    // And actually parked in the bottom-right corner with ~20px design margin
    // (right/bottom insets), not floating elsewhere.
    assert!(
        tl.x + size.x >= VW - 40.0,
        "pill is not near the right edge (right {} vs vw {VW})",
        tl.x + size.x
    );
    assert!(
        tl.y + size.y >= VH - 40.0,
        "pill is not near the bottom edge (bottom {} vs vh {VH})",
        tl.y + size.y
    );
}

#[test]
fn theme_toggle_stays_clickable_through_a_real_pointer_click() {
    let (mut app, window, pointer) = driver();
    settle(&mut app, 12);
    assert_eq!(
        model(&mut app).theme,
        ThemePref::Light,
        "starts in the light theme"
    );

    // A real synthetic pointer click at the pill's center: press → tiny move →
    // release on the pill target (the content-sized ignore_picking container must
    // not swallow it). `bevy_picking` derives the click on the press target.
    let (tl, size) = pill_rect(&mut app);
    let center = tl + size * 0.5;
    let path = [center, center + Vec2::new(1.0, 0.0)];
    drive_stroke(&mut app, window, pointer, &path);
    settle(&mut app, 6);

    assert_eq!(
        model(&mut app).theme,
        ThemePref::Dark,
        "a real pointer click on the toggle folded SetTheme(Dark) — the pill is not \
         occluded by its own container"
    );
}
