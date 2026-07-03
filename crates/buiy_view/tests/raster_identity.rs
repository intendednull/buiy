//! Headless **raster identity-patch** verification (spec §2.2, no GPU). The
//! `raster()` element patches its `Handle<Image>` **by identity**: an unrelated
//! model fold never touches the texture and the canvas ENTITY is preserved (so a
//! re-render elsewhere never drops the GPU texture), while a real handle change
//! patches in place on the SAME entity.

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Envelope, Model};
use buiy_core::render::RasterImage;
use buiy_view::{BuiyViewAppExt, Element, Kind, column, find_kind, raster, text};

#[derive(Component, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Canvas {
    tick: u32,
    /// The live canvas handle (what `raster()` samples).
    handle: Handle<Image>,
    /// A second handle the `Swap` message moves `handle` to.
    alt: Handle<Image>,
}
impl Model for Canvas {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    /// An UNRELATED fold (bumps a counter shown in a sibling text) — the canvas
    /// handle is untouched.
    Tick,
    /// Move the live handle to the alternate — a real handle change.
    Swap,
}

fn update(s: &mut Canvas, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Tick => s.tick += 1,
        Msg::Swap => s.handle = s.alt.clone(),
    }
    Cmd::none()
}

fn view(s: &Canvas) -> Element<Msg> {
    column![
        text!("tick {}", s.tick),
        raster(s.handle.clone(), 100.0, 100.0),
    ]
}

fn raster_app() -> (App, Handle<Image>, Handle<Image>) {
    let mut app = common::logic_app();
    // A headless app has no `ImagePlugin`/`RenderPlugin` to register
    // `Assets<Image>`, so register it explicitly (the F8 headless-canvas gotcha).
    app.init_asset::<Image>();
    let (h0, h1) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    app.ui(
        Canvas {
            tick: 0,
            handle: h0.clone(),
            alt: h1.clone(),
        },
        update,
        view,
    );
    common::settle(&mut app);
    (app, h0, h1)
}

fn model_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<Canvas>>();
    q.iter(world).next().expect("the model entity exists")
}

fn send(app: &mut App, m: Msg) {
    let model = model_entity(app);
    app.world_mut()
        .resource_mut::<Messages<Envelope<Canvas>>>()
        .write(Envelope::user(model, m));
    app.update(); // fold
    app.update(); // reconcile (before Layout, #10)
}

#[test]
fn unrelated_fold_preserves_the_canvas_entity_and_handle() {
    let (mut app, h0, _h1) = raster_app();
    let e0 = find_kind(app.world_mut(), Kind::Raster).expect("the raster node exists");
    // The seed handle equals the model handle.
    assert_eq!(app.world().get::<RasterImage>(e0).unwrap().0, h0);

    // An UNRELATED fold (Tick) — the sibling text changes; the canvas must not.
    send(&mut app, Msg::Tick);

    let e1 = find_kind(app.world_mut(), Kind::Raster).unwrap();
    assert_eq!(
        e1, e0,
        "the canvas ENTITY is preserved across an unrelated fold"
    );
    assert_eq!(
        app.world().get::<RasterImage>(e1).unwrap().0,
        h0,
        "the canvas handle is untouched by an unrelated fold (no re-upload)"
    );
}

#[test]
fn a_real_handle_change_patches_in_place() {
    let (mut app, _h0, h1) = raster_app();
    let e0 = find_kind(app.world_mut(), Kind::Raster).unwrap();

    // Move the handle to the alternate — a real change.
    send(&mut app, Msg::Swap);

    let e1 = find_kind(app.world_mut(), Kind::Raster).unwrap();
    assert_eq!(
        e1, e0,
        "a handle change PATCHES the same entity (no despawn/rebuild)"
    );
    assert_eq!(
        app.world().get::<RasterImage>(e1).unwrap().0,
        h1,
        "the handle patched in place to the alternate"
    );
}
