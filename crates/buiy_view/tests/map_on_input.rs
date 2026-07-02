//! #17 regression: `Element::map` LIFTS `on_input` (was the P1 drop-limitation).
//!
//! Before #17, `Element::map` dropped `on_input` (a bare `fn(String)->Msg` couldn't compose into
//! a new bare fn). #17 made `on_input` an `InputHandler{Bare|Boxed}` and `map` lifts it by boxing
//! (`Boxed(move |s| f(bare(s)))`). This proves a child element carrying `on_input`, when `.map`'d
//! into a parent `Msg`, STILL routes — the lifted message reaches the parent model. If `map` still
//! dropped `on_input`, typing would route nothing and `last` would stay `None` (RED).

mod common;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;

use buiy_core::focus::FocusedEntity;
use buiy_core::mvu::{Cmd, Model};
use buiy_view::{BuiyViewAppExt, Element, Kind, column, find_kind, text_input};

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct MapApp {
    draft: String,
    last: Option<String>,
}
impl Model for MapApp {
    type Msg = ParentMsg;
}

// The CHILD component's message — the input's `on_input` produces this.
#[derive(Clone, Debug, Reflect, PartialEq)]
enum ChildMsg {
    Typed(String),
}
// The PARENT lifts the child message via `Element::map`.
#[derive(Clone, Debug, Reflect, PartialEq)]
enum ParentMsg {
    Child(ChildMsg),
}

fn update(s: &mut MapApp, m: ParentMsg) -> Cmd<ParentMsg> {
    match m {
        ParentMsg::Child(ChildMsg::Typed(v)) => {
            s.draft = v.clone();
            s.last = Some(v);
        }
    }
    Cmd::none()
}

fn view(s: &MapApp) -> Element<ParentMsg> {
    // Build a CHILD `Element<ChildMsg>` carrying `on_input`, then LIFT it with `.map`. If `map`
    // dropped `on_input` (the old behavior), typing would route nothing.
    let child: Element<ChildMsg> = text_input(s.draft.clone()).on_input(ChildMsg::Typed);
    column![child.map(ParentMsg::Child)]
}

fn last(app: &mut App) -> Option<String> {
    app.world_mut()
        .query::<&MapApp>()
        .iter(app.world())
        .next()
        .expect("model exists")
        .last
        .clone()
}

fn type_into_field(app: &mut App, s: &str) {
    let field = find_kind(app.world_mut(), Kind::TextInput).expect("input realized");
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(field);
    app.update();
    for ch in s.chars() {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character(ch.to_string().into()),
            state: ButtonState::Pressed,
            text: Some(ch.to_string().into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

#[test]
fn element_map_lifts_on_input() {
    let mut app = common::logic_app();
    app.ui(MapApp::default(), update, view);
    common::settle(&mut app);
    assert_eq!(last(&mut app), None, "seed: nothing typed");

    type_into_field(&mut app, "hi");

    assert_eq!(
        last(&mut app),
        Some("hi".to_string()),
        "LOAD-BEARING: Element::map LIFTED the child's on_input — typing routed ParentMsg::Child(Typed); the P1 map-drops-on_input gap is closed"
    );
}
