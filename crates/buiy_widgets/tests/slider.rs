//! Wave-3 slice-2 — Slider widget: the P1d a11y bundle (role + valued range
//! `A11yValue` + orientation + focus + a11y) plus the C4 visual (the thumb
//! positioned via the `Translate` longhand, driven by `Changed<A11yValue>`) and
//! pick-through (`Pickable::IGNORE` on the decorative track/thumb/label children).
//!
//! The contract honoring (Increment/Decrement/SetValue mutate `A11yValue`) and the
//! APG keyboard (arrows/Home/End/PageUp/PageDown → value verbs) are asserted at the
//! `buiy_core` layer (`a11y_action` / `a11y_inprocess`); here the bundle shape +
//! the C4 thumb-tracks-value visual + pick-through are exercised.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yLabel, A11yOrientation, A11yRole, A11yValue, Orientation},
    components::Node,
    focus::Focusable,
    layout::{BoxModel, Length, Translate},
    render::components::{Background, Border},
    text::Text,
};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::slider::{SLIDER_THUMB_TRAVEL, Slider, SliderThumb, SliderTrack, thumb_offset};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    app
}

fn thumb_of(app: &App, sl: Entity) -> Entity {
    let world = app.world();
    world
        .get::<Children>(sl)
        .unwrap()
        .iter()
        .find(|&c| world.get::<SliderThumb>(c).is_some())
        .expect("a SliderThumb child")
}

fn thumb_x(app: &App, thumb: Entity) -> f32 {
    match app.world().get::<Translate>(thumb).unwrap().0 {
        Length::Px(px) => px,
        other => panic!("thumb translate x is not px: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The P1d bundle contract.
// ---------------------------------------------------------------------------

#[test]
fn bare_slider_marker_materializes_the_full_required_contract() {
    let mut app = app();
    let sl = app.world_mut().spawn(Slider).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(sl).is_some(), "Node");
    assert!(world.get::<BoxModel>(sl).is_some(), "BoxModel");
    assert!(world.get::<Background>(sl).is_some(), "Background");
    assert!(world.get::<Border>(sl).is_some(), "Border");
    assert!(world.get::<Focusable>(sl).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(sl).copied(),
        Some(A11yRole::Slider),
        "role defaults to Slider"
    );
    assert!(
        world.get::<A11yValue>(sl).is_some(),
        "A11yValue present (the valued range)"
    );
    assert!(
        world.get::<A11yOrientation>(sl).is_some(),
        "A11yOrientation present"
    );
    assert!(world.get::<A11yLabel>(sl).is_some(), "A11yLabel");
}

#[test]
fn slider_new_spawns_track_thumb_label_children_pick_through() {
    let mut app = app();
    let sl = app
        .world_mut()
        .spawn(Slider::new("Volume", 50.0, 0.0, 100.0, 1.0))
        .id();
    app.update();

    // The AT name + the live range are on the widget root.
    assert_eq!(
        app.world().get::<A11yLabel>(sl).map(|l| l.0.clone()),
        Some("Volume".to_string()),
        "the accessible name stays on the widget root"
    );
    let value = app.world().get::<A11yValue>(sl).unwrap();
    assert_eq!((value.now, value.min, value.max), (50.0, 0.0, 100.0));
    assert_eq!(value.step, Some(1.0));
    assert_eq!(
        app.world().get::<A11yOrientation>(sl).map(|o| o.0),
        Some(Orientation::Horizontal),
        "the catalog slider is authored Horizontal"
    );

    let children = app
        .world()
        .get::<Children>(sl)
        .expect("slider has children")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 3, "track + thumb + label children");
    for &child in &children {
        assert_eq!(
            app.world().get::<Pickable>(child).copied(),
            Some(Pickable::IGNORE),
            "decorative child carries Pickable::IGNORE (pick-through)"
        );
    }

    // Exactly one track, one thumb, one label.
    let world = app.world();
    assert_eq!(
        children
            .iter()
            .filter(|&&c| world.get::<SliderTrack>(c).is_some())
            .count(),
        1,
        "one SliderTrack child"
    );
    assert_eq!(
        children
            .iter()
            .filter(|&&c| world.get::<SliderThumb>(c).is_some())
            .count(),
        1,
        "one SliderThumb child"
    );
    let label = children
        .iter()
        .copied()
        .find(|&c| world.get::<SliderTrack>(c).is_none() && world.get::<SliderThumb>(c).is_none())
        .unwrap();
    assert_eq!(
        world.get::<Text>(label).map(|t| t.0.clone()),
        Some("Volume".to_string()),
        "the label child carries the visible pixels"
    );
}

// ---------------------------------------------------------------------------
// The C4 visual: A11yValue drives the thumb position via Changed-detection.
// ---------------------------------------------------------------------------

#[test]
fn thumb_tracks_value_now_at_min_mid_and_max() {
    let mut app = app();
    // now = min ⇒ thumb at the min end (x = 0).
    let sl = app
        .world_mut()
        .spawn(Slider::new("V", 0.0, 0.0, 100.0, 1.0))
        .id();
    app.update();
    let thumb = thumb_of(&app, sl);
    assert_eq!(
        thumb_x(&app, thumb),
        0.0,
        "now == min ⇒ thumb at the min end"
    );

    // now = max ⇒ thumb at the track end (the load-bearing C4 visual assertion).
    app.world_mut().get_mut::<A11yValue>(sl).unwrap().now = 100.0;
    app.update();
    assert_eq!(
        thumb_x(&app, thumb),
        SLIDER_THUMB_TRAVEL,
        "now == max ⇒ thumb at the track end (x = travel)"
    );

    // now = midpoint ⇒ thumb at half travel.
    app.world_mut().get_mut::<A11yValue>(sl).unwrap().now = 50.0;
    app.update();
    assert!(
        (thumb_x(&app, thumb) - SLIDER_THUMB_TRAVEL / 2.0).abs() < 1e-3,
        "now at the midpoint ⇒ thumb at half travel"
    );
}

#[test]
fn thumb_offset_clamps_out_of_range_and_handles_degenerate_span() {
    // Out-of-range `now` saturates at the bounds (matches `A11yValue` clamping).
    let over = A11yValue {
        now: 200.0,
        min: 0.0,
        max: 100.0,
        ..Default::default()
    };
    assert_eq!(
        thumb_offset(&over),
        SLIDER_THUMB_TRAVEL,
        "now > max ⇒ at end"
    );
    let under = A11yValue {
        now: -50.0,
        min: 0.0,
        max: 100.0,
        ..Default::default()
    };
    assert_eq!(thumb_offset(&under), 0.0, "now < min ⇒ at start");
    // A degenerate range (max == min) maps to 0 rather than dividing by zero.
    let degenerate = A11yValue {
        now: 5.0,
        min: 5.0,
        max: 5.0,
        ..Default::default()
    };
    assert_eq!(
        thumb_offset(&degenerate),
        0.0,
        "max == min ⇒ thumb at start"
    );
}
