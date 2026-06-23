//! Widget scene-fns — the mergeable styled-authoring path (team-lead
//! fast-follow). A `button(label)` / `text_input_*(placeholder)` scene-fn
//! spells the widget's styling as explicit `bsn!` FIELD-patches, so a user
//! patching on top MERGES field-wise (unmentioned fields keep the widget's
//! canonical defaults) instead of the required-component-suppression gotcha
//! where a single-field `BoxModel` patch on the bare marker drops the rest.
//!
//! Upstream evidence (bevy_scene 0.19.0-rc.3 lib.rs:284-288, 313-352): for both
//! the `Clone + Default` blanket path and `FromTemplate`, "unmentioned fields
//! keep their values from earlier patches or the type's defaults, and multiple
//! patches merge rather than overwrite." Composing a scene-fn (`enemy()`) then
//! patching one field (`Health { max: 200 }`) yields the merge
//! `Health { current: 100, max: 200 }`.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::scene::{ScenePlugin, WorldSceneExt, bsn};
use buiy_core::a11y::{A11yLabel, A11yRole};
use buiy_core::components::Node;
use buiy_core::focus::Focusable;
use buiy_core::layout::{BoxModel, Display, Edges, Length, Position, Sizing};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::text::edit::{Placeholder, SingleLine, TextEditState};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::scene::{
    button, checkbox, slider, switch, text_input_multi_line, text_input_single_line,
};
use std::borrow::Cow;

/// The BSN spawn machinery + the widget plugins (so required-components are
/// registered before any spawn). No GPU.
fn scene_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);
    app
}

/// THE merge test: a user patches `BoxModel { width }` on top of `button(..)`,
/// and the widget's other canonical fields (padding, height) MERGE through —
/// they are NOT dropped. This is the property the bare-marker `#[require]` path
/// could not provide (the suppression gotcha); the scene-fn fixes it.
#[test]
fn button_scene_fn_patch_merges_keeping_canonical_fields() {
    let mut app = scene_test_app();

    let id = app
        .world_mut()
        .spawn_scene(bsn! {
            button("Save")
            BoxModel { width: { Sizing::Length(Length::Px(240.0)) } }
        })
        .expect("spawn_scene")
        .id();

    let world = app.world();
    let bm = world.get::<BoxModel>(id).expect("BoxModel present");
    // The user's patch wins on `width`…
    assert_eq!(
        bm.width,
        Sizing::Length(Length::Px(240.0)),
        "user width patch wins"
    );
    // …and the widget's canonical 8px padding + 32px height MERGE through (the
    // whole point — NOT the bare-marker suppression that would give 0/auto).
    assert_eq!(
        bm.padding,
        Edges::all(8.0),
        "canonical padding survives the merge"
    );
    assert_eq!(
        bm.height,
        Sizing::Length(Length::Px(32.0)),
        "canonical height survives the merge"
    );

    // The full widget contract is present.
    assert!(world.get::<Node>(id).is_some(), "Node");
    assert!(world.get::<Display>(id).is_some(), "Display");
    assert!(world.get::<Position>(id).is_some(), "Position");
    assert!(world.get::<Focusable>(id).is_some(), "Focusable");
    assert_eq!(world.get::<A11yRole>(id).copied(), Some(A11yRole::Button));
    assert_eq!(
        world.get::<A11yLabel>(id).expect("A11yLabel").0,
        "Save",
        "label threaded through the scene-fn"
    );
    // The canonical button surface fill is present (and unpatched here).
    assert_eq!(
        world.get::<Background>(id).expect("Background").color,
        ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
        "canonical button background"
    );
}

/// Unpatched, `button(label)` reproduces the canonical box exactly (the scene-fn
/// field-patches equal the `#[require]` initializer values).
#[test]
fn button_scene_fn_unpatched_is_the_canonical_button() {
    let mut app = scene_test_app();

    let id = app
        .world_mut()
        .spawn_scene(bsn! { button("Go") })
        .expect("spawn_scene")
        .id();

    let world = app.world();
    let bm = world.get::<BoxModel>(id).expect("BoxModel");
    assert_eq!(bm.width, Sizing::Length(Length::Px(120.0)));
    assert_eq!(bm.height, Sizing::Length(Length::Px(32.0)));
    assert_eq!(bm.padding, Edges::all(8.0));
    assert_eq!(world.get::<A11yLabel>(id).expect("A11yLabel").0, "Go");
}

/// `text_input_single_line(placeholder)` mirrors the constructor: full editor
/// contract + `SingleLine`, and a user box patch merges field-wise.
#[test]
fn text_input_single_line_scene_fn_merges_and_is_single_line() {
    let mut app = scene_test_app();

    let id = app
        .world_mut()
        .spawn_scene(bsn! {
            text_input_single_line("Search…")
            BoxModel { width: { Sizing::Length(Length::Px(400.0)) } }
        })
        .expect("spawn_scene")
        .id();

    let world = app.world();
    let bm = world.get::<BoxModel>(id).expect("BoxModel");
    assert_eq!(bm.width, Sizing::Length(Length::Px(400.0)), "width patched");
    assert_eq!(
        bm.padding,
        Edges::all(8.0),
        "canonical padding merges through"
    );
    assert!(
        world.get::<TextEditState>(id).is_some(),
        "editor mechanism present"
    );
    assert!(world.get::<SingleLine>(id).is_some(), "single-line policy");
    assert_eq!(
        world.get::<Placeholder>(id).expect("Placeholder").0,
        "Search…"
    );
}

/// `text_input_multi_line` has the editor contract but NO `SingleLine`.
#[test]
fn text_input_multi_line_scene_fn_has_no_single_line() {
    let mut app = scene_test_app();

    let id = app
        .world_mut()
        .spawn_scene(bsn! { text_input_multi_line("Body") })
        .expect("spawn_scene")
        .id();

    let world = app.world();
    assert!(world.get::<TextEditState>(id).is_some(), "editor present");
    assert!(
        world.get::<SingleLine>(id).is_none(),
        "multi-line ⇒ no SingleLine"
    );
    assert_eq!(world.get::<Placeholder>(id).expect("Placeholder").0, "Body");
}

/// `checkbox(label)` spawns the full a11y contract on the root plus the C4
/// child subtree — the check/dash mark + the visible label — both
/// `Pickable::IGNORE` (pick-through). The AT name stays `A11yLabel` on the root;
/// the label pixels live in a child `Text`.
#[test]
fn checkbox_scene_fn_builds_contract_children_and_pick_through() {
    use bevy::ecs::hierarchy::Children;
    use bevy::picking::Pickable;
    use buiy_core::a11y::{A11yToggled, Toggled};
    use buiy_core::render::components::CssVisibility;
    use buiy_core::text::Text;
    use buiy_widgets::checkbox::CheckboxMark;

    let mut app = scene_test_app();
    let id = app
        .world_mut()
        .spawn_scene(bsn! { checkbox("Done") })
        .expect("spawn_scene")
        .id();
    app.update();

    let world = app.world();
    // The a11y contract on the root.
    assert_eq!(
        world.get::<A11yRole>(id).copied(),
        Some(A11yRole::Checkbox),
        "scene-fn root is a Checkbox"
    );
    assert_eq!(
        world.get::<A11yToggled>(id).map(|t| t.0),
        Some(Toggled::False),
        "tri-state toggle present, default False"
    );
    assert_eq!(
        world.get::<A11yLabel>(id).expect("A11yLabel").0,
        "Done",
        "the AT name stays on the root"
    );

    // The C4 child subtree: mark + label, both pick-through.
    let children: Vec<Entity> = world
        .get::<Children>(id)
        .expect("children")
        .iter()
        .copied()
        .collect();
    assert_eq!(children.len(), 2, "mark + label children");
    for &c in &children {
        assert_eq!(
            world.get::<Pickable>(c).copied(),
            Some(Pickable::IGNORE),
            "decorative child is Pickable::IGNORE"
        );
    }
    let mark = children
        .iter()
        .copied()
        .find(|&c| world.get::<CheckboxMark>(c).is_some())
        .expect("a CheckboxMark child");
    assert_eq!(
        world.get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Hidden),
        "mark starts hidden (default toggle False)"
    );
    let label = children
        .iter()
        .copied()
        .find(|&c| world.get::<CheckboxMark>(c).is_none())
        .expect("a label child");
    assert_eq!(
        world.get::<Text>(label).map(|t| t.0.clone()),
        Some("Done".to_string()),
        "the label child carries the visible pixels"
    );
}

/// `switch(label)` spawns the full a11y contract plus the thumb + label children
/// (pick-through), the thumb at the off position (`Translate` x = 0).
#[test]
fn switch_scene_fn_builds_contract_children_and_pick_through() {
    use bevy::ecs::hierarchy::Children;
    use bevy::picking::Pickable;
    use buiy_core::a11y::{A11yToggled, Toggled};
    use buiy_core::layout::Translate;
    use buiy_widgets::switch::SwitchThumb;

    let mut app = scene_test_app();
    let id = app
        .world_mut()
        .spawn_scene(bsn! { switch("Wi-Fi") })
        .expect("spawn_scene")
        .id();
    app.update();

    let world = app.world();
    assert_eq!(
        world.get::<A11yRole>(id).copied(),
        Some(A11yRole::Switch),
        "scene-fn root is a Switch"
    );
    assert_eq!(
        world.get::<A11yToggled>(id).map(|t| t.0),
        Some(Toggled::False),
        "binary toggle present, default False"
    );
    assert_eq!(world.get::<A11yLabel>(id).expect("A11yLabel").0, "Wi-Fi");

    let children: Vec<Entity> = world
        .get::<Children>(id)
        .expect("children")
        .iter()
        .copied()
        .collect();
    assert_eq!(children.len(), 2, "thumb + label children");
    for &c in &children {
        assert_eq!(
            world.get::<Pickable>(c).copied(),
            Some(Pickable::IGNORE),
            "decorative child is Pickable::IGNORE"
        );
    }
    let thumb = children
        .iter()
        .copied()
        .find(|&c| world.get::<SwitchThumb>(c).is_some())
        .expect("a SwitchThumb child");
    assert_eq!(
        world.get::<Translate>(thumb).map(|t| t.0),
        Some(buiy_core::layout::Length::Px(0.0)),
        "thumb starts at the off position (x = 0)"
    );
}

/// `slider(label, now, min, max, step)` spawns the full a11y contract (role +
/// valued range + horizontal orientation) plus the track + thumb + label children
/// (pick-through), with the live `A11yValue` authored.
#[test]
fn slider_scene_fn_builds_contract_children_and_pick_through() {
    use bevy::ecs::hierarchy::Children;
    use bevy::picking::Pickable;
    use buiy_core::a11y::{A11yOrientation, A11yValue, Orientation};
    use buiy_widgets::slider::{SliderThumb, SliderTrack};

    let mut app = scene_test_app();
    let id = app
        .world_mut()
        .spawn_scene(bsn! { slider("Volume", 50.0, 0.0, 100.0, 1.0) })
        .expect("spawn_scene")
        .id();
    app.update();

    let world = app.world();
    assert_eq!(
        world.get::<A11yRole>(id).copied(),
        Some(A11yRole::Slider),
        "scene-fn root is a Slider"
    );
    let value = world.get::<A11yValue>(id).expect("A11yValue present");
    assert_eq!((value.now, value.min, value.max), (50.0, 0.0, 100.0));
    assert_eq!(value.step, Some(1.0));
    assert_eq!(
        world.get::<A11yOrientation>(id).map(|o| o.0),
        Some(Orientation::Horizontal),
        "the scene-fn authors a horizontal slider"
    );
    assert_eq!(world.get::<A11yLabel>(id).expect("A11yLabel").0, "Volume");

    let children: Vec<Entity> = world
        .get::<Children>(id)
        .expect("children")
        .iter()
        .copied()
        .collect();
    assert_eq!(children.len(), 3, "track + thumb + label children");
    for &c in &children {
        assert_eq!(
            world.get::<Pickable>(c).copied(),
            Some(Pickable::IGNORE),
            "decorative child is Pickable::IGNORE"
        );
    }
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
}
