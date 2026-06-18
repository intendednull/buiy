//! E6 Task 6 — the `TextInput` widget bundle (editing-and-ime § 2.3). It
//! composes the core editor mechanism (`TextEditState` + `SingleLine` +
//! `Placeholder`) with widget policy (sizes, focusable, a11y, focus-on-click).
//! `buiy_widgets` names ZERO cosmic types — `TextEditState::for_font_size`
//! is the seam.

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::focus::Focusable;
use buiy_core::layout::{BoxModel, Display, Edges, Length, Overflow, OverflowMode, Sizing};
use buiy_core::render::components::{Background, Border};
use buiy_core::text::edit::{Placeholder, SingleLine, TextEditState};
use buiy_core::text::{FontSize, Text};
use buiy_widgets::{TextInput, WidgetsPlugin};

/// The `#[require]` contract: spawning the **bare** `TextInput` marker (the
/// `bsn! { TextInput }` path) materializes the editor mechanism, the display
/// `Text` carrier, the layout-visible Style decomposition, paint, focus, and
/// a11y — everything `base_bundle()` assembles by hand. `SingleLine` is NOT
/// required (it is per-constructor policy added by `single_line()`).
#[test]
fn bare_text_input_marker_materializes_the_full_required_contract() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(TextInput).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(entity).is_some(), "Node");
    assert!(world.get::<Display>(entity).is_some(), "Display");
    let box_model = world.get::<BoxModel>(entity).expect("BoxModel");
    assert_eq!(box_model.width, Sizing::Length(Length::Px(200.0)));
    assert_eq!(box_model.height, Sizing::Length(Length::Px(32.0)));
    assert_eq!(box_model.padding, Edges::all(8.0));
    // The input clips its content (overflow:hidden) so auto-scroll has a
    // scroll container to pan (text_input.rs base_bundle).
    let overflow = world.get::<Overflow>(entity).expect("Overflow");
    assert_eq!(overflow.x, OverflowMode::Hidden);
    assert_eq!(overflow.y, OverflowMode::Hidden);
    assert!(world.get::<Background>(entity).is_some(), "Background");
    assert!(world.get::<Border>(entity).is_some(), "Border");
    // Editor mechanism + display carrier.
    assert!(
        world.get::<TextEditState>(entity).is_some(),
        "TextEditState"
    );
    assert!(world.get::<Text>(entity).is_some(), "display Text carrier");
    assert!(world.get::<FontSize>(entity).is_some(), "FontSize");
    assert!(world.get::<Placeholder>(entity).is_some(), "Placeholder");
    assert!(world.get::<Focusable>(entity).is_some(), "Focusable");
    // `SingleLine` is NOT part of the base contract.
    assert!(
        world.get::<SingleLine>(entity).is_none(),
        "bare TextInput is not single-line"
    );
}

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
