//! Headless (no-adapter) tests for the raster (textured-quad) primitive — the
//! two lowest-tier observations the F1 spec calls for:
//!
//!  * a **layout snapshot** — a [`RasterImage`] node lands in the tree at its
//!    fixed size (the raster component does not perturb layout);
//!  * a **display-list snapshot** — the real `extract_buiy_rasters` SYSTEM mirrors
//!    that one node into `ExtractedRasters` as a single [`RasterInstance`] carrying
//!    its resolved size, with the image id in the parallel `images` vec.
//!
//! The per-node record mapping (`raster_instance_for`) is unit-tested inside
//! `render/raster.rs`; this file proves the *system wiring* — the `With<Node>`
//! query matches a real spawned node — device-free via the MainWorld swap the
//! render extract step performs (bevy_render `lib.rs`), the `extract_harness`
//! idiom minus the renderer.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use buiy_core::components::Node;
use buiy_core::layout::Style;
use buiy_core::render::raster::{ExtractedRasters, RasterImage, extract_buiy_rasters};

/// The Dooduel game-canvas size — a concrete, recognizable fixed rect.
const CANVAS_W: f32 = 720.0;
const CANVAS_H: f32 = 450.0;

#[test]
fn raster_node_resolves_to_its_fixed_size() {
    let mut app = crate::support::headless_layout_app();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(CANVAS_W).height_px(CANVAS_H),
            RasterImage(Handle::<Image>::default()),
        ))
        .id();
    crate::support::settle(&mut app);

    let layout = app
        .world()
        .entity(e)
        .get::<buiy_core::ResolvedLayout>()
        .expect("the raster node resolved a layout");
    assert_eq!(layout.size, Vec2::new(CANVAS_W, CANVAS_H));
}

#[test]
fn extract_produces_one_raster_instance_for_the_node() {
    let mut app = crate::support::headless_layout_app();
    let handle = Handle::<Image>::default();
    app.world_mut().spawn((
        Node,
        Style::default().width_px(CANVAS_W).height_px(CANVAS_H),
        RasterImage(handle.clone()),
    ));
    crate::support::settle(&mut app);

    // A bare render world with only the carrier + the `MainWorld` swap slot, plus
    // a one-system `ExtractSchedule` — the minimal harness that runs the real
    // extract system without a wgpu adapter.
    let mut render = World::new();
    render.init_resource::<ExtractedRasters>();
    render.init_resource::<MainWorld>();
    let mut schedule = Schedule::new(ExtractSchedule);
    schedule.add_systems(extract_buiy_rasters);

    swap_main(&mut render, &mut app);
    schedule.run(&mut render);
    swap_main(&mut render, &mut app);

    let extracted = render.resource::<ExtractedRasters>();
    assert_eq!(
        extracted.instances.len(),
        1,
        "one RasterImage node extracts to exactly one instance"
    );
    assert_eq!(extracted.instances[0].rect_size, [CANVAS_W, CANVAS_H]);
    assert_eq!(
        extracted.images,
        vec![handle.id()],
        "the parallel images vec carries the node's image id"
    );
}

/// Swap the live main world into (or back out of) the render world's `MainWorld`
/// slot — bevy_render's own extract dance, run manually so the extract step sees
/// the real spawned node.
fn swap_main(render: &mut World, app: &mut App) {
    let mut main = render.resource_mut::<MainWorld>();
    core::mem::swap(&mut **main, app.world_mut());
}
