//! The **unified headless driver** end-to-end: a texture-presenting canvas driven
//! from a real synthetic drag all the way to ink in its CPU pixel buffer, with no
//! GPU.
//!
//! This is the general-purpose form of the recipe the multi-agent playtest host
//! needs. A single App composes the real picking stack (bevy's `PickingPlugin`,
//! Buiy's `PickingPlugin` + backend, a `Camera2d` + synthetic window + pointer)
//! plus the layout stack, and — the one non-obvious recipe piece —
//! `app.init_asset::<Image>()`, because a GPU-free host has no
//! `ImagePlugin`/`RenderPlugin` to register `Assets<Image>` (without it, the first
//! `Assets<Image>` access is an opaque "Resource does not exist" panic under a
//! release-profile `debug=0` build). On that App a plain pickable canvas node owns
//! an `Image`; a `Pointer<Drag>` observer maps each drag point to a pixel and paints
//! it; [`drive_stroke`] strokes across the laid-out rect; and we assert INK landed in
//! the buffer along the stroke — the full input → funnel → paint-observer → buffer
//! path.
//!
//! The playtest host stacks the GPU-free probe preset (`buiy::BuiyProbePlugin` — the
//! agent's "eyes") on top of exactly this picking stack; the probe preset omits
//! picking, so re-adding it conflicts with nothing. This test exercises the
//! *driver + canvas* half (the half that lives in `buiy_verify`, dep-free of the
//! `buiy` umbrella); the eyes half is exercised where a full MVU app is booted.

use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::image::Image;
use bevy::picking::events::{Drag, Pointer};
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};

use buiy_core::{Node, layout::Style};
use buiy_verify::pointer::drive_stroke;

const CANVAS_W: usize = 200;
const CANVAS_H: usize = 120;
const PAPER: [u8; 4] = [245, 245, 240, 255];
const INK: [u8; 4] = [20, 20, 20, 255];

/// The canvas node's `Image` handle, so the paint observer can find the buffer to
/// write into given only the drag's target entity.
#[derive(Component)]
struct CanvasImage(Handle<Image>);

/// Paint the drag segment onto the target's canvas image — the shape a real
/// freehand brush produces. Uses the `Pointer<Drag>.delta` to reconstruct the
/// previous point (`pos - delta`) and rasterizes a contiguous ink line from there
/// to the current point, mapping window-space into node-local pixels via the node's
/// `GlobalTransform` top-left (which the production layout → bridge chain produced).
fn paint_on_drag(
    ev: On<Pointer<Drag>>,
    nodes: Query<(&CanvasImage, &GlobalTransform)>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok((canvas, gt)) = nodes.get(ev.entity) else {
        return;
    };
    let top_left = gt.translation().truncate();
    let to = ev.pointer_location.position - top_left;
    let from = to - ev.event.delta;
    let Some(mut image) = images.get_mut(&canvas.0) else {
        return;
    };
    let Some(data) = image.data.as_mut() else {
        return;
    };
    // Sample the segment densely enough that consecutive dabs overlap.
    let steps = (from.distance(to).ceil() as usize).max(1);
    for s in 0..=steps {
        let p = from.lerp(to, s as f32 / steps as f32);
        dab(data, p.x as i32, p.y as i32);
    }
}

/// A 3×3 ink dab centered on `(cx, cy)`, clipped to the canvas bounds.
fn dab(data: &mut [u8], cx: i32, cy: i32) {
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= CANVAS_W as i32 || y >= CANVAS_H as i32 {
                continue;
            }
            let i = (y as usize * CANVAS_W + x as usize) * 4;
            data[i..i + 4].copy_from_slice(&INK);
        }
    }
}

/// Build the unified headless driver: the real picking stack + the layout stack + a
/// synthetic window/camera/pointer + `init_asset::<Image>()`. Returns
/// `(app, window, pointer)`.
fn driver() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        // The core bevy_picking infrastructure (PointerInput::receive + hit
        // scheduling + Messages<PointerHits>).
        .add_plugins(bevy::picking::PickingPlugin)
        // The Buiy data + layout stack (the reconciler-free subset: components,
        // the layout solve, and the transform-propagation bridge).
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        // Buiy's picking half: the InteractionPlugin hover stage (the Pointer<E>
        // taxonomy, incl. the drag machine) + the Buiy hit-test backend.
        .add_plugins(buiy_core::picking::PickingPlugin)
        .add_plugins(buiy_core::picking::BuiyPickingBackendPlugin);
    // The `Image` asset type: a GPU-free host has no ImagePlugin/RenderPlugin to
    // register `Assets<Image>`, so the canvas host must register it itself — the
    // missing piece of the unified-driver recipe.
    app.init_asset::<Image>();

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

    (app, window, pointer)
}

/// Spawn a paper-filled canvas image + a pickable canvas node presenting it, under
/// a root that centers it in the window, settle the layout, and return the canvas
/// entity + its window-space top-left.
fn spawn_canvas(app: &mut App) -> (Entity, Vec2) {
    let pixels: Vec<u8> = PAPER
        .iter()
        .copied()
        .cycle()
        .take(CANVAS_W * CANVAS_H * 4)
        .collect();
    let image = Image::new(
        Extent3d {
            width: CANVAS_W as u32,
            height: CANVAS_H as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let canvas = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(CANVAS_W as f32)
                .height_px(CANVAS_H as f32),
            CanvasImage(handle),
            Name::new("canvas"),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(800.0)
                .height_px(600.0)
                .padding(120.0),
        ))
        .add_child(canvas);
    app.add_observer(paint_on_drag);

    // Settle: layout → bridge → propagation → picking produce a GlobalTransform.
    for _ in 0..6 {
        app.update();
    }
    let top_left = app
        .world()
        .get::<GlobalTransform>(canvas)
        .expect("canvas has a GlobalTransform")
        .translation()
        .truncate();
    (canvas, top_left)
}

fn pixel(app: &App, handle: &Handle<Image>, x: usize, y: usize) -> [u8; 4] {
    let image = app.world().resource::<Assets<Image>>().get(handle).unwrap();
    let data = image.data.as_ref().unwrap();
    let i = (y * CANVAS_W + x) * 4;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

fn canvas_handle(app: &App, canvas: Entity) -> Handle<Image> {
    app.world().get::<CanvasImage>(canvas).unwrap().0.clone()
}

/// The end-to-end: drive a real horizontal stroke across the canvas node and assert
/// ink landed in the CPU buffer along it — proving a synthetic pointer drives the
/// production drag machine into an app's own paint observer, all headless.
#[test]
fn stroke_across_canvas_paints_ink_into_the_buffer() {
    let (mut app, window, pointer) = driver();
    let (canvas, top_left) = spawn_canvas(&mut app);
    let handle = canvas_handle(&app, canvas);

    // Before: the mid-row is untouched paper.
    let mid_y = CANVAS_H / 2;
    assert_eq!(
        pixel(&app, &handle, CANVAS_W / 2, mid_y),
        PAPER,
        "canvas starts as blank paper"
    );

    // A horizontal stroke across the middle of the canvas, in window space.
    let y = top_left.y + mid_y as f32;
    let from = Vec2::new(top_left.x + 20.0, y);
    let to = Vec2::new(top_left.x + (CANVAS_W as f32 - 20.0), y);
    drive_stroke(&mut app, window, pointer, &{
        let steps = 30usize;
        (0..=steps)
            .map(|i| from.lerp(to, i as f32 / steps as f32))
            .collect::<Vec<_>>()
    });

    // After: the mid-row is inked where the stroke passed.
    assert_eq!(
        pixel(&app, &handle, CANVAS_W / 2, mid_y),
        INK,
        "the stroke inked the mid-canvas pixel"
    );
    let inked = (0..CANVAS_W)
        .filter(|&x| pixel(&app, &handle, x, mid_y) == INK)
        .count();
    assert!(
        inked > CANVAS_W / 2,
        "the horizontal stroke inked most of the mid-row (got {inked} of {CANVAS_W})"
    );

    // Untouched rows near the top stay paper — the stroke did not flood the buffer.
    assert_eq!(
        pixel(&app, &handle, CANVAS_W / 2, 5),
        PAPER,
        "rows away from the stroke stay blank"
    );
}
