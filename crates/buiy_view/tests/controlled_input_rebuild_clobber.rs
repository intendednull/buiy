//! Framework-gap regression (Dooduel QA Track 3): an assistive-tech
//! `Action::SetValue` (via `buiy_core::a11y::set_value`, the channel
//! `buiy::probe::set_value` / the QA-seat driver use) into a **controlled**
//! `text_input` must fold into the MVU model AND survive **even on a screen that
//! rebuilds every frame**.
//!
//! The gap (found by the QA-seat in-game chat, distinct from the #98 emit gap in
//! `a11y_set_value_route.rs`): the probe writes the editor + emits `TextChanged`
//! **out of band** (a direct `&mut World` dispatch, between `app.update()`s), so
//! the pending `TextChanged` sits across a frame boundary. The editor→model fold
//! (`route_text_input`, `MvuSet::Enqueue`, late in the frame) has NOT run yet
//! when the next frame's **front-of-frame** reconcile (`ViewSet::Reconcile`,
//! `.before(BuiySet::Layout)`) runs. On a screen whose model changed for an
//! unrelated reason (the in-game countdown tick → `Changed<M>` every frame), the
//! reconcile runs and unconditionally re-asserts `set_editor_value(editor,
//! model.value)` — clobbering the un-folded edit back to the STALE model value
//! **before** `route_text_input` reads the editor to fold it. The fold then folds
//! the clobbered (empty) value and the edit is permanently lost.
//!
//! The `a11y_set_value_route.rs` (#98) test passes because its model is STATIC:
//! on the settle frame the reconcile early-outs (`Changed<M>` empty), so it never
//! clobbers; it runs the frame AFTER the fold, when editor and model agree. This
//! test forces a rebuild EVERY frame (a `Tick` enqueued each update, mirroring the
//! countdown), so the reconcile runs on the settle frame and the clobber fires.
//!
//! Pre-fix this FAILS at the first assertion (`draft` stays `""`, editor clobbered
//! to `""`).

mod common;

use bevy::prelude::*;

use buiy_core::a11y::{A11yRole, get_by_role, set_value};
use buiy_core::mvu::{Cmd, Model, MvuSet, enqueue};
use buiy_core::text::edit::TextEditState;
use buiy_view::{BuiyViewAppExt, Element, Kind, column, find_kind, text_input};

/// A controlled form whose `tick` field advances **every frame** (the countdown
/// analog), so the model is `Changed<M>` every frame → the reconcile runs every
/// frame → `set_editor_value` is attempted every frame. `draft` is the controlled
/// text-input value.
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct TickForm {
    draft: String,
    tick: u64,
}
impl Model for TickForm {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Typed(String),
    Tick,
}

fn update(s: &mut TickForm, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Typed(v) => s.draft = v,
        // A per-frame model mutation on an UNRELATED field — the countdown
        // rebuild. It must not cost the in-flight `draft` edit.
        Msg::Tick => s.tick += 1,
    }
    Cmd::none()
}

fn view(s: &TickForm) -> Element<Msg> {
    column![text_input(s.draft.clone()).on_input(Msg::Typed)]
}

/// Enqueue a `Tick` every frame (in `MvuSet::Enqueue`, the funnel the real
/// countdown clock uses), so the model folds a change every frame and the
/// front-of-frame reconcile fires on every subsequent frame.
fn tick_every_frame(mut commands: Commands, q: Query<Entity, With<TickForm>>) {
    for e in &q {
        enqueue::<TickForm>(&mut commands, e, Msg::Tick);
    }
}

fn draft(app: &mut App) -> String {
    app.world_mut()
        .query::<&TickForm>()
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
fn a11y_set_value_survives_on_a_rebuilding_controlled_input() {
    let mut app = common::logic_app();
    app.ui(TickForm::default(), update, view);
    app.add_systems(Update, tick_every_frame.in_set(MvuSet::Enqueue));
    common::settle(&mut app);
    assert_eq!(draft(&mut app), "", "seed: empty draft");

    // Drive the AT SetValue path exactly as the QA-seat driver does: resolve the
    // field by role, then `set_value` OUT OF BAND (a direct `&mut World`
    // dispatch), leaving a pending `TextChanged` for the next frame's fold.
    let node = get_by_role(app.world_mut(), A11yRole::TextInput, None, None)
        .expect("text input resolves in the a11y tree");
    set_value(app.world_mut(), node, "balloon").expect("set_value ok");

    // Settle several frames — the countdown rebuild runs a reconcile on EVERY
    // frame. The edit must fold into the model AND survive (not be clobbered by a
    // same-frame re-assert running before `route_text_input` consumes it).
    for _ in 0..6 {
        app.update();
    }

    assert_eq!(
        draft(&mut app),
        "balloon",
        "LOAD-BEARING: the AT SetValue folded into the model on a screen that \
         rebuilds every frame — the front-of-frame reconcile must not clobber the \
         un-folded editor edit before route_text_input reads it"
    );
    assert_eq!(
        editor_value(&mut app),
        "balloon",
        "the editor keeps the edit (the controlled reconcile does not fight an \
         in-flight edit whose fold is still pending)"
    );
}
