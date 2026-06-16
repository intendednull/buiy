//! E6 Task 6 — the `TextInput` widget bundle (editing-and-ime § 2.3). It
//! composes the core editor mechanism (`TextEditState` + `SingleLine` +
//! `Placeholder`) with widget policy (sizes, focusable, a11y, focus-on-click).
//! `buiy_widgets` names ZERO cosmic types — `TextEditState::for_font_size`
//! is the seam.

use bevy::prelude::*;
use buiy_core::focus::Focusable;
use buiy_core::text::edit::{Placeholder, SingleLine, TextEditState};
use buiy_widgets::{TextInput, WidgetsPlugin};

#[test]
fn single_line_text_input_composes_editor_markers_and_focusable() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    app.update();

    let world = app.world();
    assert!(
        world.get::<TextEditState>(entity).is_some(),
        "has the editor"
    );
    assert!(
        world.get::<SingleLine>(entity).is_some(),
        "single-line marker"
    );
    assert!(world.get::<Focusable>(entity).is_some(), "focusable");
    let placeholder = world.get::<Placeholder>(entity).expect("placeholder");
    assert_eq!(placeholder.0, "Search…");
    assert_eq!(
        world.get::<TextEditState>(entity).unwrap().value(),
        "",
        "a fresh input is empty"
    );
}

#[test]
fn multi_line_text_input_has_no_single_line_marker() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(TextInput::multi_line("")).id();
    app.update();

    assert!(
        app.world().get::<SingleLine>(entity).is_none(),
        "multi-line ⇒ no SingleLine"
    );
    assert!(app.world().get::<TextEditState>(entity).is_some());
}

#[test]
fn clicking_a_text_input_focuses_it() {
    use buiy_core::FocusedEntity;
    use buiy_core::focus::FocusPlugin;
    use buiy_core::picking::Hovered;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    // `FocusPlugin::handle_tab` reads `Res<ButtonInput<KeyCode>>` (the resource a
    // real app gets from `InputPlugin`, absent under MinimalPlugins) — seed it so
    // the focus systems validate. `MouseButton` drives `focus_on_click`.
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();

    let entity = app.world_mut().spawn(TextInput::single_line("")).id();
    app.update();

    // Hover + mouse-down on the input.
    app.world_mut().insert_resource(Hovered(Some(entity)));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(entity),
        "click focuses the input (widget policy)"
    );
}
