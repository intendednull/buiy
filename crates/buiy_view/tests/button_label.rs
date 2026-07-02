//! ViewSlot / dynamic button-label regression cover (#12).
//!
//! Every other test + example uses a STATIC button label, so the `set_button_label`
//! change-branch never fires there. This is the dedicated cover: when a button's
//! label is a function of the model, a fold must patch BOTH the button's visible
//! child `Text` (found via the recorded `ViewSlot.label`) AND its root `A11yLabel`
//! — IN PLACE (same button entity id, no respawn). A future refactor that dropped
//! either half of that dual write would freeze dynamic labels / accessible names
//! with every other test still green; this test reddens instead.

mod common;

use bevy::prelude::*;
use buiy_core::a11y::A11yLabel;
use buiy_core::mvu::{Cmd, Model};
use buiy_core::text::Text;
use buiy_view::{BuiyViewAppExt, Element, button, column, find_press_target};
use buiy_widgets::Button;

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct LabelApp {
    n: i32,
}
impl Model for LabelApp {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Bump,
}

fn update(s: &mut LabelApp, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Bump => s.n += 1,
    }
    Cmd::none()
}

// A button whose LABEL is a function of the model — the dynamic-label case.
fn view(s: &LabelApp) -> Element<Msg> {
    column![button(format!("Count: {}", s.n)).on_press(Msg::Bump)]
}

fn the_button(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<Button>>();
    q.iter(world).next().expect("exactly one button")
}

fn a11y_name(app: &mut App, btn: Entity) -> String {
    app.world_mut()
        .get::<A11yLabel>(btn)
        .expect("button carries A11yLabel")
        .0
        .clone()
}

/// The button's visible label child `Text` (the only `Text` node in this view).
fn visible_label(app: &mut App) -> String {
    let world = app.world_mut();
    let mut q = world.query::<&Text>();
    q.iter(world).next().expect("label Text node").0.clone()
}

fn n(app: &mut App) -> i32 {
    app.world_mut()
        .query::<&LabelApp>()
        .iter(app.world())
        .next()
        .expect("model exists")
        .n
}

#[test]
fn dynamic_button_label_patches_a11y_and_text_in_place() {
    let mut app = common::logic_app();
    app.ui(LabelApp::default(), update, view);
    common::settle(&mut app);

    let btn = the_button(&mut app);
    assert_eq!(visible_label(&mut app), "Count: 0", "seed visible label");
    assert_eq!(a11y_name(&mut app, btn), "Count: 0", "seed accessible name");

    // Fold a Bump through the REAL router (no app-authored bind system).
    let target =
        find_press_target::<LabelApp>(app.world_mut(), &Msg::Bump).expect("button routes Bump");
    common::press(&mut app, target);
    assert_eq!(n(&mut app), 1, "Bump folded");

    // The ViewSlot dual write: BOTH the visible child Text AND the accessible name
    // reflect the new model-derived label...
    assert_eq!(
        visible_label(&mut app),
        "Count: 1",
        "visible label re-patched"
    );
    assert_eq!(
        a11y_name(&mut app, btn),
        "Count: 1",
        "accessible name re-patched (the dual write)"
    );
    // ...patched IN PLACE — the button was reused, not respawned.
    assert_eq!(
        the_button(&mut app),
        btn,
        "button patched in place — same entity id, no rebuild"
    );
}
