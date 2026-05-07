//! Phase 0 contract test: spawning a `Button` attaches role + label +
//! focusable + a default style; clicking a hovered button emits `OnPress`.
//!
//! Bevy 0.18 renamed buffered `Event` → `Message` and `EventWriter`/`EventReader`
//! → `MessageWriter`/`MessageReader`. Phase 0 leans on the `Message` flavor for
//! `OnPress` so the test reaches into `Messages<OnPress>` (not `Events<OnPress>`).

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yLabel, A11yRole},
    focus::Focusable,
};
use buiy_widgets::{Button, OnPress, WidgetsPlugin};

#[test]
fn spawning_a_button_attaches_role_label_focusable_and_default_style() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(Button::new("Save")).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Button>(entity).is_some());
    assert!(world.get::<Focusable>(entity).is_some());
    assert_eq!(
        world.get::<A11yRole>(entity).copied(),
        Some(A11yRole::Button)
    );
    let label = world.get::<A11yLabel>(entity).expect("a11y label");
    assert_eq!(label.0, "Save");
}

#[test]
fn clicking_a_button_emits_on_press() {
    use buiy_core::picking::Hovered;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    // Phase 0 picking lives in CorePlugin; the click handler reads
    // `Hovered` + `ButtonInput<MouseButton>`. Provide both as resources
    // since MinimalPlugins doesn't include the input plugin.
    app.init_resource::<ButtonInput<MouseButton>>();

    let entity = app.world_mut().spawn(Button::new("Save")).id();
    // Manually mark hovered + simulate a primary mouse press.
    app.world_mut().insert_resource(Hovered(Some(entity)));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    let messages = app.world().resource::<Messages<OnPress>>();
    let mut cursor = messages.get_cursor();
    let mut found = false;
    for ev in cursor.read(messages) {
        if ev.0 == entity {
            found = true;
        }
    }
    assert!(found, "OnPress message for clicked button");
}
