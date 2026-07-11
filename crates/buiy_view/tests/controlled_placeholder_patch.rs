//! Regression (Dooduel QA Track 3, the SEPARATE correlated symptom): a controlled
//! `text_input`'s **placeholder** must update when the view returns a new one on a
//! rebuild. This is a distinct root cause from the set_value clobber
//! (`controlled_input_rebuild_clobber.rs`): the reconcile's `Kind::TextInput`
//! **patch** branch seeded the placeholder once at spawn and never re-patched it,
//! so a phase change (`Phase::Picking` → `Drawing`) left the in-game chat field
//! showing a stale prompt ("Waiting for the word…" while drawing).
//!
//! Pre-fix this FAILS: the placeholder stays the spawn-time string.

mod common;

use bevy::prelude::*;

use buiy_core::mvu::{Cmd, Model};
use buiy_core::text::edit::Placeholder;
use buiy_view::{BuiyViewAppExt, Element, Kind, column, find_kind, text_input};

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct PhForm {
    drawing: bool,
}
impl Model for PhForm {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Flip,
}

fn update(s: &mut PhForm, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Flip => s.drawing = !s.drawing,
    }
    Cmd::none()
}

fn view(s: &PhForm) -> Element<Msg> {
    // The placeholder is a pure function of the model phase — exactly the in-game
    // chat's `match s.replica.phase` prompt.
    let ph = if s.drawing {
        "Type your guess…"
    } else {
        "Waiting for the word…"
    };
    column![text_input(String::new()).placeholder(ph)]
}

fn placeholder(app: &mut App) -> String {
    let field = find_kind(app.world_mut(), Kind::TextInput).expect("input realized");
    app.world()
        .get::<Placeholder>(field)
        .expect("placeholder component exists")
        .0
        .clone()
}

#[test]
fn controlled_placeholder_updates_on_a_rebuild() {
    let mut app = common::logic_app();
    app.ui(PhForm::default(), update, view);
    common::settle(&mut app);
    assert_eq!(
        placeholder(&mut app),
        "Waiting for the word…",
        "seed: the spawn-time placeholder"
    );

    // Flip the model phase → the view returns a NEW placeholder. Trips `Changed<M>`
    // so the reconcile runs and must re-patch the placeholder.
    {
        let mut q = app.world_mut().query::<&mut PhForm>();
        let mut m = q.iter_mut(app.world_mut()).next().expect("model exists");
        m.drawing = true;
    }
    app.update();
    app.update();

    assert_eq!(
        placeholder(&mut app),
        "Type your guess…",
        "LOAD-BEARING: the controlled reconcile re-patches the placeholder when the \
         view returns a new one (not just at spawn)"
    );
}
