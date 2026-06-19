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
    components::Node,
    focus::Focusable,
    layout::{BoxModel, Display, FlexParams, Overflow, Position},
    render::components::{Background, Border},
};
use buiy_widgets::{Button, OnPress, WidgetsPlugin};

/// The `#[require]` contract: spawning the **bare** `Button` marker (the
/// `bsn! { Button }` path) materializes the full widget contract that
/// `Button::new()` assembles by hand — the layout-visible Style
/// decomposition (so `sync_styles` sees it) plus paint, focus, and a11y.
/// Without this, `bsn! { Button }` would spawn a marker with no companions
/// and be useless (spec § 4.1a).
#[test]
fn bare_button_marker_materializes_the_full_required_contract() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(Button).id();
    app.update();

    let world = app.world();
    // Layout marker + the non-optional `sync_styles` style decomposition.
    assert!(world.get::<Node>(entity).is_some(), "Node");
    assert!(world.get::<Display>(entity).is_some(), "Display");
    assert!(world.get::<Position>(entity).is_some(), "Position");
    assert!(world.get::<FlexParams>(entity).is_some(), "FlexParams");
    assert!(world.get::<Overflow>(entity).is_some(), "Overflow");
    let box_model = world.get::<BoxModel>(entity).expect("BoxModel");
    // The canonical button box (120x32, 8px padding) — shared with
    // `Button::new()` via the `button_box_model()` initializer.
    use buiy_core::layout::{Edges, Length, Sizing};
    assert_eq!(box_model.width, Sizing::Length(Length::Px(120.0)));
    assert_eq!(box_model.height, Sizing::Length(Length::Px(32.0)));
    assert_eq!(box_model.padding, Edges::all(8.0));
    // Paint + interaction + a11y companions.
    assert!(world.get::<Background>(entity).is_some(), "Background");
    assert!(world.get::<Border>(entity).is_some(), "Border");
    assert!(world.get::<Focusable>(entity).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(entity).copied(),
        Some(A11yRole::Button),
        "a11y role defaults to Button"
    );
    assert!(world.get::<A11yLabel>(entity).is_some(), "A11yLabel");
}

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
