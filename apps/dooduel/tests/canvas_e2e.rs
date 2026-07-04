//! W8 §D1 (part 2) — the drawing canvas driven END-TO-END through the REAL
//! pointer pipeline, headless. This is the "unified headless driver" answer: a
//! single App runs the **GPU-free probe preset** (`BuiyProbePlugin` — the MVU
//! funnel + reconciler + a11y projection) AND the **picking stack** (bevy's
//! `PickingPlugin`, Buiy's `PickingPlugin` + backend, a `Camera2d` + synthetic
//! window) — they compose because the probe preset simply *omits* picking, so
//! adding it back conflicts with nothing. On that App we boot Dooduel + the
//! drawing canvas, navigate into the Drawing phase as the drawer, and drag a real
//! synthetic pointer across the canvas node's laid-out rect. `pointer_events`
//! derives `Pointer<Press>` → `Drag` → `Release`, the app's own paint observers
//! (`on_canvas_press`/`on_canvas_drag`) map each to canvas pixels, and we assert
//! INK landed in the `PaintCanvases` CPU buffer at the stroked location — the full
//! input → funnel → canvas path, with no GPU.

use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};

use buiy_core::components::ResolvedLayout;
use buiy_core::mvu::Envelope;
use buiy_verify::pointer::drive_stroke;
use dooduel::paint::{CANVAS_H, CANVAS_W, CanvasKind, PAPER, PaintCanvases};
use dooduel::{Dooduel, Msg};

/// Build the unified headless driver: the GPU-free probe preset + the real
/// picking stack + a synthetic window/camera/pointer, then Dooduel + the canvas.
/// Returns `(app, window, pointer)`.
fn unified_driver() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        // The core bevy_picking infrastructure (PointerInput::receive + hit
        // scheduling + Messages<PointerHits>).
        .add_plugins(bevy::picking::PickingPlugin)
        // The GPU-free Buiy stack: MVU funnel + reconciler + layout + a11y + text
        // + widgets. It OMITS picking — which is exactly why we can add it back.
        .add_plugins(buiy::BuiyProbePlugin)
        // Buiy's picking half: the InteractionPlugin hover stage (Pointer<E>
        // taxonomy) + the Buiy hit-test backend. (Scroll/animation are NOT needed
        // — the recipe is deliberately minimal, confirmed by removing them.)
        .add_plugins(buiy_core::picking::PickingPlugin)
        .add_plugins(buiy_core::picking::BuiyPickingBackendPlugin);

    // A synthetic primary window so the layout solver has a viewport and the
    // pointer has a target (desktop 3-pane in-game layout).
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(1280, 800),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    // A Camera2d targeting the window so `emit_picks` resolves a camera (else the
    // backend drops every hit).
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));
    // The synthetic mouse pointer (drive_stroke rewrites its PointerLocation).
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

    // The `Image` asset type: `CanvasPlugin` creates + mirrors the canvas image,
    // but the GPU-free probe preset never adds `ImagePlugin`/`RenderPlugin` (which
    // normally `init_asset::<Image>()`), so a headless canvas host must register it
    // itself — the missing piece of the unified-driver recipe.
    app.init_asset::<Image>();

    // Dooduel MVU + the drawing canvas (CPU paint surfaces + the paint observers).
    dooduel::install(&mut app);
    app.add_plugins(dooduel::paint::CanvasPlugin);

    (app, window, pointer)
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn enqueue(app: &mut App, msg: Msg) {
    let e = app
        .world_mut()
        .query_filtered::<Entity, With<Dooduel>>()
        .iter(app.world())
        .next()
        .expect("model entity exists");
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<Envelope<Dooduel>>>()
        .write(Envelope::user(e, msg));
}

/// The Game drawing canvas node's window-space top-left + size (set by the
/// reconciler + wire_canvas_node once the in-game screen is up).
fn canvas_rect(app: &mut App) -> (Vec2, Vec2) {
    app.world_mut()
        .query::<(&CanvasKind, &GlobalTransform, &ResolvedLayout)>()
        .iter(app.world())
        .find(|(k, ..)| **k == CanvasKind::Game)
        .map(|(_, gt, layout)| (gt.translation().truncate(), layout.size))
        .expect("the Game canvas node is laid out in the in-game screen")
}

fn game_pixel(app: &mut App, x: usize, y: usize) -> [u8; 4] {
    let canvases = app.world().resource::<PaintCanvases>();
    let s = canvases.surface(CanvasKind::Game);
    let i = (y * s.width + x) * 4;
    [
        s.pixels[i],
        s.pixels[i + 1],
        s.pixels[i + 2],
        s.pixels[i + 3],
    ]
}

#[test]
fn dragging_the_canvas_lands_ink_in_the_paint_buffer() {
    let (mut app, window, pointer) = unified_driver();
    // Boot: Startup creates the canvases; announce_canvases folds the handles;
    // the reconciler settles.
    settle(&mut app, 12);

    // Into the Drawing phase as the drawer (seat 0 is the human drawer, and the
    // human is auto-viewing the drawer — so `enabled` becomes true after sync).
    enqueue(&mut app, Msg::StartMatch);
    settle(&mut app, 8);
    enqueue(&mut app, Msg::ChooseWord(0));
    settle(&mut app, 12);

    // The canvas node is laid out with a real 720×450 rect (shrink(false) keeps it
    // at its natural size, so window px → canvas texel is 1:1).
    let (tl, size) = canvas_rect(&mut app);
    assert!(
        size.x > 100.0 && size.y > 100.0,
        "the canvas has a real rect: {size}"
    );

    // The buffer starts blank (the per-turn auto-clear on the Picking→Drawing edge).
    let mid = (CANVAS_W / 2, CANVAS_H / 2);
    assert_eq!(
        game_pixel(&mut app, mid.0, mid.1),
        PAPER,
        "the canvas is blank paper before we draw"
    );

    // Drag a horizontal line across the middle third of the canvas rect.
    let y = tl.y + size.y * 0.5;
    let from = Vec2::new(tl.x + size.x * 0.3, y);
    let to = Vec2::new(tl.x + size.x * 0.7, y);
    let path: Vec<Vec2> = (0..=6).map(|i| from.lerp(to, i as f32 / 6.0)).collect();
    drive_stroke(&mut app, window, pointer, &path);

    // INK landed: the center pixel (on the stroked line) is no longer paper, and a
    // meaningful band of pixels was painted (the brush stamped + interpolated).
    assert_ne!(
        game_pixel(&mut app, mid.0, mid.1),
        PAPER,
        "the stroke inked the canvas center pixel"
    );
    let inked = {
        let canvases = app.world().resource::<PaintCanvases>();
        let s = canvases.surface(CanvasKind::Game);
        s.pixels.chunks_exact(4).filter(|p| *p != PAPER).count()
    };
    assert!(
        inked > 500,
        "the stroke painted a visible line (inked pixels = {inked})"
    );
}
