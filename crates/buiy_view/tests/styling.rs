//! Headless **styling / #9** verification (no GPU).
//!
//! A model-driven style change (gap / padding / background token) **patches in
//! place** — the same container entity id, with `FlexParams` (the `FlexGap`),
//! `BoxModel` (padding), and `Background` all `set_if_neq`-updated — proving the
//! decomposed-style patching refine (spec §3 #9). This is what the prototype
//! could NOT do (it only applied container style at spawn).

mod common;

use bevy::prelude::*;
use buiy_core::layout::{BoxModel, FlexParams};
use buiy_core::mvu::{Cmd, Envelope, Model};
use buiy_core::render::components::Background;
use buiy_view::{BuiyViewAppExt, Color, Element, Kind, Radius, Space, column, text};

/// A model whose *style* depends on state (not just its text).
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Skin {
    hot: bool,
}
impl Model for Skin {
    type Msg = Toggle;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
struct Toggle;

fn update(s: &mut Skin, _m: Toggle) -> Cmd<Toggle> {
    s.hot = !s.hot;
    Cmd::none()
}

/// The container's background / gap / padding / radius are all a function of the
/// model — so a fold must re-derive and patch them.
fn view(s: &Skin) -> Element<Toggle> {
    let (bg, gap, pad, radius) = if s.hot {
        (Color::Accent, Space::Lg, Space::Lg, Radius::Lg)
    } else {
        (Color::Surface, Space::Sm, Space::Xs, Radius::Sm)
    };
    column![text("styled")]
        .background(bg)
        .gap(gap)
        .padding(pad)
        .radius(radius)
}

fn skin_app() -> App {
    let mut app = common::logic_app();
    app.ui(Skin::default(), update, view);
    app
}

/// The single realized `Kind::Column` container entity.
fn column_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Kind)>();
    q.iter(world)
        .find(|(_, k)| **k == Kind::Column)
        .map(|(e, _)| e)
        .expect("the column container exists")
}

/// The model entity (carries the `Skin` model component).
fn model_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<Skin>>();
    q.iter(world).next().expect("the model entity exists")
}

/// Flip `hot` by writing a real `Toggle` to the model's inbox, then let the
/// drain fold it and the front-of-frame reconcile (#10) patch the tree.
fn toggle(app: &mut App) {
    let model = model_entity(app);
    app.world_mut()
        .resource_mut::<Messages<Envelope<Skin>>>()
        .write(Envelope::user(model, Toggle));
    app.update(); // frame N: drain folds — model changes
    app.update(); // frame N+1: reconcile (before Layout) patches the style
}

#[test]
fn model_driven_style_patches_in_place() {
    let mut app = skin_app();
    common::settle(&mut app);

    let col = column_entity(&mut app);

    // Seed (hot == false): Surface bg, gap Sm, pad Xs.
    let bg0 = app
        .world()
        .get::<Background>(col)
        .expect("bg")
        .color
        .clone();
    let gap0 = app.world().get::<FlexParams>(col).expect("flex").gap.row;
    let pad0 = app.world().get::<BoxModel>(col).expect("box").padding;
    assert_eq!(bg0, Color::Surface.to_token(), "seed background token");

    // Fold a Toggle → hot == true.
    toggle(&mut app);

    // Same entity — the container was PATCHED, not despawned+respawned.
    assert_eq!(
        column_entity(&mut app),
        col,
        "container patched in place — same entity id"
    );

    let bg1 = app
        .world()
        .get::<Background>(col)
        .expect("bg")
        .color
        .clone();
    let gap1 = app.world().get::<FlexParams>(col).expect("flex").gap.row;
    let pad1 = app.world().get::<BoxModel>(col).expect("box").padding;

    assert_ne!(bg1, bg0, "Background token patched in place");
    assert_ne!(gap1, gap0, "FlexGap (FlexParams.gap) patched in place");
    assert_ne!(pad1, pad0, "BoxModel padding patched in place");
    assert_eq!(bg1, Color::Accent.to_token(), "patched to the accent token");
}
