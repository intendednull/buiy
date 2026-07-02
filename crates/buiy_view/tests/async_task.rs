//! Headless **async-through-the-surface** verification (no GPU) — the PW2 surface proof.
//!
//! The whole point of `Cmd::task` (buiy_view design §3 #15) landing in `buiy_core::mvu` is
//! that a `view`-authored app gets async **for free**: the reducer returns
//! `Cmd::task(future, Msg::Loaded)`, the drain spawns it, the poll folds the result back
//! through the funnel, and the RECONCILER re-renders the view on that async-driven model
//! change — with NO app-authored launch/poll systems (the DX-3 gap the surface deletes).
//!
//! This asserts exactly that end-to-end path against the real `ui()` install: press "Load",
//! and the label the view derives from the model flips to the async result once the task
//! completes — proving the async value reached the derived view.

mod common;

use bevy::prelude::*;
use buiy_core::interaction::OnPress;
use buiy_core::mvu::{Cmd, Model};
use buiy_core::text::Text;
use buiy_view::{BuiyViewAppExt, Element, button, column, find_press_target, text};

// --- The WHOLE app-author surface: Model + Msg + update (with async) + view ---------------

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct AsyncApp {
    loading: bool,
    data: Option<String>,
}
impl Model for AsyncApp {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Load,
    Loaded(String),
}

// The reducer launches the async effect AS A VALUE — no hand-written launch/poll systems.
fn update(s: &mut AsyncApp, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Load => {
            s.loading = true;
            Cmd::task(async { "hello".to_string() }, Msg::Loaded)
        }
        Msg::Loaded(d) => {
            s.loading = false;
            s.data = Some(d);
            Cmd::none()
        }
    }
}

// The label is a pure function of the model; the reconciler re-derives it on every change,
// including the async-driven one.
fn view(s: &AsyncApp) -> Element<Msg> {
    let status = if s.loading {
        "loading…".to_string()
    } else if let Some(d) = &s.data {
        format!("data: {d}")
    } else {
        "idle".to_string()
    };
    column![text!("{status}"), button("Load").on_press(Msg::Load)]
}

/// The single view-derived label `Text` node's content (the button carries its own label
/// `Text`; the status label is the one that starts with a non-"Load" word — match the known
/// states explicitly to avoid the button label).
fn status_label(app: &mut App) -> String {
    let world = app.world_mut();
    let mut q = world.query::<&Text>();
    q.iter(world)
        .map(|t| t.0.clone())
        .find(|s| s == "idle" || s == "loading…" || s.starts_with("data:"))
        .expect("the status label node exists")
}

#[test]
fn async_task_result_reaches_the_view() {
    let mut app = common::logic_app();
    app.ui(AsyncApp::default(), update, view);
    common::settle(&mut app);

    assert_eq!(status_label(&mut app), "idle", "seed: no data yet");

    // Press "Load" through the REAL router (no app-authored routing). The reducer flips
    // `loading` and returns `Cmd::task` — the drain spawns the effect; the poll will fold the
    // result back on a later frame.
    let load = find_press_target::<AsyncApp>(app.world_mut(), &Msg::Load).expect("Load routes");
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(load));

    // Run frames until the async result has folded AND the reconciler has re-rendered the
    // label from it. The task (`async { "hello" }`) completes near-instantly on the compute
    // pool, but the exact frame is timing-dependent — so poll a bounded loop.
    let mut reached = false;
    for _ in 0..200 {
        app.update();
        if status_label(&mut app).starts_with("data:") {
            reached = true;
            break;
        }
    }
    assert!(
        reached,
        "the async result reached the derived view within the frame budget"
    );
    assert_eq!(
        status_label(&mut app),
        "data: hello",
        "LOAD-BEARING: the view re-rendered from the async Cmd::task result — no app-authored launch/poll"
    );
    let model = app
        .world_mut()
        .query::<&AsyncApp>()
        .iter(app.world())
        .next()
        .expect("model exists")
        .clone();
    assert_eq!(
        model.data.as_deref(),
        Some("hello"),
        "the model folded the result"
    );
    assert!(!model.loading, "the result fold cleared `loading`");
}
