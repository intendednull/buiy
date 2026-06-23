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

/// Clicking a text input focuses it — C3c migrated the `focus_on_click` widget
/// policy off the legacy `Hovered` resource onto the bevy_picking `Pointer<E>`
/// layer. The driving mechanism changed (a synthetic `PointerInput` press over
/// the input's absolute box, through the real picking pipeline →
/// `Pointer<Press>` → the `focus_on_click` observer), but the asserted intent is
/// identical: a primary press on the input sets `FocusedEntity` to it.
///
/// The full Buiy backend is added explicitly so a `Pointer<Press>` fires; the
/// input is given a `ResolvedLayout` + `GlobalTransform` (the absolute basis
/// `emit_picks` reads) and the app stands up a window + `Camera2d`.
#[test]
fn clicking_a_text_input_focuses_it() {
    use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
    use bevy::picking::pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerLocation,
    };
    use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};
    use buiy_core::FocusedEntity;
    use buiy_core::ResolvedLayout;
    use buiy_core::focus::FocusPlugin;
    use buiy_core::picking::{BuiyPickingBackendPlugin, PickingPlugin};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    // `FocusPlugin::handle_tab` reads `Res<ButtonInput<KeyCode>>` (the resource a
    // real app gets from `InputPlugin`, absent under MinimalPlugins) — seed it so
    // the focus systems validate.
    app.init_resource::<ButtonInput<KeyCode>>();

    // A synthetic primary window + a Camera2d targeting it (the §3.1 camera
    // resolution the backend needs).
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));

    let entity = app.world_mut().spawn(TextInput::single_line("")).id();
    // The input's absolute box (0,0)..(200,32) — give it the absolute basis
    // `emit_picks` reads without the full layout→bridge chain.
    app.world_mut().entity_mut(entity).insert((
        ResolvedLayout {
            position: Vec2::ZERO,
            size: Vec2::new(200.0, 32.0),
        },
        GlobalTransform::IDENTITY,
    ));

    // Aim the synthetic pointer at the input's center and press.
    let target = WindowRef::Entity(window).normalize(Some(window)).unwrap();
    let location = Location {
        target: NormalizedRenderTarget::Window(target),
        position: Vec2::new(100.0, 16.0),
    };
    app.world_mut()
        .spawn((PointerId::Mouse, PointerLocation::new(location.clone())));
    app.update(); // backend emits a hit; the hover stage registers Over.
    app.world_mut()
        .write_message(bevy::picking::pointer::PointerInput {
            pointer_id: PointerId::Mouse,
            location,
            action: PointerAction::Press(PointerButton::Primary),
        });
    app.update();

    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(entity),
        "click focuses the input (widget policy)"
    );
}

/// C2 § 5 step 5 — removing the `Text`→editor content seam (the Bug-3 fix)
/// introduces NO seed regression for the empty case: a bare `TextInput`
/// (`Text("")` + `TextEditState::for_font_size`, no explicit seed verb) is `""`
/// at construction AND stays `""` after a `FontsGeneration` bump (the style-only
/// TextSync path never re-`set_text`s the empty editor buffer). `BuiyTextPlugin`
/// is added explicitly so the bump's TextSync sweep actually runs over the
/// widget's editor buffer (WidgetsPlugin alone does not register TextSync — the
/// bump would otherwise be inert and the test vacuous).
#[test]
fn bare_text_input_value_stays_empty_across_a_fonts_generation_bump() {
    use buiy_core::text::FontsGeneration;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(WidgetsPlugin);

    let entity = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<TextEditState>(entity).unwrap().value(),
        "",
        "precondition: a bare TextInput seeds \"\" (no explicit seed verb needed for the empty case)"
    );

    // Bump FontsGeneration (the runtime add_font / system-font-scan trigger) and
    // run a frame. With the § 2.1 style-only path, the empty editor buffer is
    // never set_text'd to anything else — it stays "".
    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();

    assert_eq!(
        app.world().get::<TextEditState>(entity).unwrap().value(),
        "",
        "a bare TextInput's value stays \"\" after a bump — the Text->editor seam \
         removal (Bug-3 fix) introduces NO seed regression for the empty case (§ 5 step 5)"
    );
}
