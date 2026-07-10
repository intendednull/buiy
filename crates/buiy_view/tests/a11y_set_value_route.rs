//! Framework-gap regression: an assistive-tech `Action::SetValue` (driven via
//! `buiy_core::a11y::set_value`, the same channel `buiy::probe::set_value` /
//! the QA-seat driver use) MUST fold into the MVU model exactly as a keystroke
//! does.
//!
//! The gap (found by the Dooduel QA-seat live gate): `honor_text_set_value`
//! (`buiy_core::a11y::contract`) mutated the editor + the a11y tree but emitted
//! NO `TextChanged`. `buiy_view`'s `route_text_input` bridge fires `on_input`
//! ONLY on `TextChanged`, so the model never folded — AND the next controlled
//! reconcile (`set_editor_value`) clobbered the un-folded edit back to the stale
//! model value. The keyboard path (`text::edit::input::apply_keyboard_edits`)
//! emits `TextChanged` after any value-changing edit; `SetValue` must mirror it.
//!
//! Pre-fix this FAILS at the first assertion (`draft` stays `""`).

mod common;

use bevy::prelude::*;

use buiy_core::a11y::{A11yRole, get_by_role, set_value};
use buiy_core::mvu::{Cmd, Model};
use buiy_core::text::edit::TextEditState;
use buiy_view::{BuiyViewAppExt, Element, Kind, column, find_kind, text_input};

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Form {
    draft: String,
}
impl Model for Form {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Typed(String),
}

fn update(s: &mut Form, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Typed(v) => s.draft = v,
    }
    Cmd::none()
}

fn view(s: &Form) -> Element<Msg> {
    column![text_input(s.draft.clone()).on_input(Msg::Typed)]
}

fn draft(app: &mut App) -> String {
    app.world_mut()
        .query::<&Form>()
        .iter(app.world())
        .next()
        .expect("model exists")
        .draft
        .clone()
}

fn editor_value(app: &mut App) -> String {
    let field = find_kind(app.world_mut(), Kind::TextInput).expect("input realized");
    app.world()
        .get::<TextEditState>(field)
        .expect("editor exists")
        .value()
}

#[test]
fn a11y_set_value_folds_into_model_and_survives_rebuild() {
    let mut app = common::logic_app();
    app.ui(Form::default(), update, view);
    common::settle(&mut app);
    assert_eq!(draft(&mut app), "", "seed: empty draft");

    // Drive the assistive-tech SetValue path exactly as the QA-seat driver does:
    // resolve the field by role, then set_value through the `honor` channel.
    let node = get_by_role(app.world_mut(), A11yRole::TextInput, None, None)
        .expect("text input resolves in the a11y tree");
    set_value(app.world_mut(), node, "ABCD").expect("set_value ok");

    // Frame N: `route_text_input` reads `TextChanged` → enqueues `Msg::Typed` →
    // the drain folds it. Frame N+1: the reconcile patches the derived tree from
    // the new draft (and must NOT clobber the already-equal editor).
    app.update();
    app.update();

    assert_eq!(
        draft(&mut app),
        "ABCD",
        "LOAD-BEARING: Action::SetValue folded into the MVU model — the framework \
         emits TextChanged on a value-changing SetValue, so route_text_input fired on_input"
    );
    assert_eq!(
        editor_value(&mut app),
        "ABCD",
        "the folded value round-trips: the controlled reconcile leaves the \
         now-equal editor untouched (no clobber back to the stale model value)"
    );
}
