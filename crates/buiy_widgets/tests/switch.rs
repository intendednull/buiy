//! Wave-3 slice-1 — Switch widget: the P1d a11y bundle (role + binary
//! `A11yToggled` + focus + a11y) plus the C4 visual (the thumb slid via the
//! `Translate` longhand, driven by `Changed<A11yToggled>`) and pick-through
//! (`Pickable::IGNORE` on the decorative children).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yLabel, A11yRole, A11yToggled, Toggled},
    components::Node,
    focus::Focusable,
    interaction::OnPress,
    layout::{BoxModel, Length, Translate},
    render::components::{Background, Border},
    text::Text,
};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::switch::{SWITCH_THUMB_TRAVEL, Switch, SwitchThumb};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    app
}

fn press(app: &mut App, entity: Entity) {
    app.world_mut().write_message(OnPress(entity));
    app.update();
}

fn thumb_of(app: &App, sw: Entity) -> Entity {
    let world = app.world();
    world
        .get::<Children>(sw)
        .unwrap()
        .iter()
        .find(|&c| world.get::<SwitchThumb>(c).is_some())
        .expect("a SwitchThumb child")
}

fn thumb_x(app: &App, thumb: Entity) -> f32 {
    match app.world().get::<Translate>(thumb).unwrap().0 {
        Length::Px(px) => px,
        other => panic!("thumb translate x is not px: {other:?}"),
    }
}

#[test]
fn bare_switch_marker_materializes_the_full_required_contract() {
    let mut app = app();
    let sw = app.world_mut().spawn(Switch).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(sw).is_some(), "Node");
    assert!(world.get::<BoxModel>(sw).is_some(), "BoxModel");
    assert!(world.get::<Background>(sw).is_some(), "Background");
    assert!(world.get::<Border>(sw).is_some(), "Border");
    assert!(world.get::<Focusable>(sw).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(sw).copied(),
        Some(A11yRole::Switch),
        "role defaults to Switch"
    );
    assert_eq!(
        world.get::<A11yToggled>(sw).map(|t| t.0),
        Some(Toggled::False),
        "A11yToggled present, defaulting to False (off)"
    );
    assert!(world.get::<A11yLabel>(sw).is_some(), "A11yLabel");
}

#[test]
fn switch_new_spawns_label_and_thumb_children_pick_through() {
    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();

    assert_eq!(
        app.world().get::<A11yLabel>(sw).map(|l| l.0.clone()),
        Some("Wi-Fi".to_string()),
        "the accessible name stays on the widget root"
    );

    let children = app
        .world()
        .get::<Children>(sw)
        .expect("switch has children")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 2, "thumb + label children");
    for &child in &children {
        assert_eq!(
            app.world().get::<Pickable>(child).copied(),
            Some(Pickable::IGNORE),
            "decorative child carries Pickable::IGNORE (pick-through)"
        );
    }

    let world = app.world();
    let label = children
        .iter()
        .copied()
        .find(|&c| world.get::<SwitchThumb>(c).is_none())
        .unwrap();
    assert_eq!(
        world.get::<Text>(label).map(|t| t.0.clone()),
        Some("Wi-Fi".to_string()),
        "the label child carries the visible pixels"
    );

    // The thumb starts at the off position (x = 0).
    let thumb = thumb_of(&app, sw);
    assert_eq!(thumb_x(&app, thumb), 0.0, "thumb starts off (x = 0)");
}

#[test]
fn toggling_a11y_toggled_slides_the_thumb() {
    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();
    let thumb = thumb_of(&app, sw);

    assert_eq!(thumb_x(&app, thumb), 0.0, "False ⇒ thumb off");

    app.world_mut().get_mut::<A11yToggled>(sw).unwrap().0 = Toggled::True;
    app.update();
    assert_eq!(
        thumb_x(&app, thumb),
        SWITCH_THUMB_TRAVEL,
        "True ⇒ thumb slid on (x = travel)"
    );

    app.world_mut().get_mut::<A11yToggled>(sw).unwrap().0 = Toggled::False;
    app.update();
    assert_eq!(thumb_x(&app, thumb), 0.0, "False ⇒ thumb back off");
}

#[test]
fn on_press_toggles_switch_binary_and_slides_thumb() {
    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();
    let thumb = thumb_of(&app, sw);

    // Press 1: off → on.
    press(&mut app, sw);
    assert_eq!(
        app.world().get::<A11yToggled>(sw).map(|t| t.0),
        Some(Toggled::True),
        "first OnPress flips False→True"
    );
    assert_eq!(thumb_x(&app, thumb), SWITCH_THUMB_TRAVEL, "thumb slid on");

    // Press 2: on → off (binary — never Mixed).
    press(&mut app, sw);
    assert_eq!(
        app.world().get::<A11yToggled>(sw).map(|t| t.0),
        Some(Toggled::False),
        "second OnPress flips True→False (binary, no Mixed)"
    );
    assert_eq!(thumb_x(&app, thumb), 0.0, "thumb back off");
}
