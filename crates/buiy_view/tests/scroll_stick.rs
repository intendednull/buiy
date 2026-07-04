//! Headless **controlled stick-to-bottom** verification (spec §2.2 finding #3, no
//! GPU). The reconciler drift-asserts `ScrollOffset = max` **only while** the
//! model's `stick` intent is set; once the intent clears (the app's response to a
//! scroll-away), a further append does NOT move the offset — so the framework
//! never yanks a user reading scrollback.

mod common;

use bevy::prelude::*;
use buiy_core::BuiySet;
use buiy_core::layout::ScrollOffset;
use buiy_core::mvu::{Cmd, Envelope, Model};
use buiy_core::scroll::{ScrollExtent, update_scroll_extent};
use buiy_view::{BuiyViewAppExt, Element, Kind, find_kind, scroll_column, text};

#[derive(Component, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Chat {
    lines: Vec<String>,
    /// The model-owned stick-to-bottom intent (the view re-derives it).
    stick: bool,
}
impl Model for Chat {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    /// Append a chat line (grows the scroll content).
    Append,
    /// The app's response to a user scroll-away: clear the stick intent.
    ScrollAway,
}

fn update(s: &mut Chat, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Append => {
            let n = s.lines.len();
            s.lines.push(format!("line {n}"));
        }
        Msg::ScrollAway => s.stick = false,
    }
    Cmd::none()
}

/// A bounded-height chat pane that scrolls; the stick-to-bottom modifier is
/// applied ONLY while the model intent is set (the view re-derives it from the
/// model — the runtime-state↔pure-view contract).
fn view(s: &Chat) -> Element<Msg> {
    let lines: Vec<Element<Msg>> = s.lines.iter().map(|l| text(l.clone())).collect();
    let pane = scroll_column(lines).height(60.0);
    if s.stick {
        pane.stick_to_bottom()
    } else {
        pane
    }
}

fn chat_app() -> App {
    let mut app = common::logic_app();
    // The scroll extent cache is normally part of `ScrollInputPlugin`; add just
    // the post-layout system this behavior needs (no picking observer required).
    app.add_systems(
        Update,
        update_scroll_extent
            .after(BuiySet::Layout)
            .before(BuiySet::Input),
    );
    app.ui(
        Chat {
            lines: vec!["line 0".into()],
            stick: true,
        },
        update,
        view,
    );
    common::settle(&mut app);
    app
}

fn model_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<Chat>>();
    q.iter(world).next().expect("the model entity exists")
}

fn send(app: &mut App, m: Msg) {
    let model = model_entity(app);
    app.world_mut()
        .resource_mut::<Messages<Envelope<Chat>>>()
        .write(Envelope::user(model, m));
    // Fold, reconcile (before Layout), then a couple frames for layout + the
    // extent cache + the stick pin to settle.
    for _ in 0..4 {
        app.update();
    }
}

fn pane(app: &mut App) -> Entity {
    find_kind(app.world_mut(), Kind::Column).expect("the scroll pane exists")
}

#[test]
fn appends_while_sticking_pin_to_the_bottom_then_clear_frees_the_offset() {
    let mut app = chat_app();

    // Append enough lines to overflow the 60px pane while `stick` is set.
    for _ in 0..8 {
        send(&mut app, Msg::Append);
    }
    let sc = pane(&mut app);
    let max_y = app.world().get::<ScrollExtent>(sc).unwrap().max_offset().y;
    assert!(
        max_y > 0.0,
        "the content overflows the bounded pane (there is scroll room)"
    );
    assert_eq!(
        app.world().get::<ScrollOffset>(sc).unwrap().y,
        max_y,
        "while `stick` is set, appends keep the pane pinned to the bottom (== max)"
    );

    // The user scrolls away: the app clears the stick intent, and the runtime
    // leaves the offset where the user parked it (simulated: mid-scroll).
    send(&mut app, Msg::ScrollAway);
    let parked = 5.0_f32;
    app.world_mut().get_mut::<ScrollOffset>(sc).unwrap().y = parked;
    app.update(); // no re-pin: the stick marker is gone (intent cleared)

    // A FURTHER append must NOT yank the offset back to the bottom.
    let before_max = app.world().get::<ScrollExtent>(sc).unwrap().max_offset().y;
    send(&mut app, Msg::Append);
    let after_max = app.world().get::<ScrollExtent>(sc).unwrap().max_offset().y;
    assert!(
        after_max > before_max,
        "the append grew the content (max increased) — a real chance to yank"
    );
    assert_eq!(
        app.world().get::<ScrollOffset>(sc).unwrap().y,
        parked,
        "with the intent cleared, a further append leaves the parked offset put \
         (NOT re-pinned to the new bottom)"
    );
}
